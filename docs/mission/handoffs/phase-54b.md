# Fase 54b — Nomes reservados na leitura

## Implementado
- `crates/ward-core/src/entry/validation.rs`: `Draft::validate` passa a ser
  `validate_from(Origin::Save)`; novo `Draft::validate_stored` (`pub(crate)`, só o leitor do
  corpo chama) com `Origin::Read`. Na leitura, e só nela, são aceitos os nomes que a 0.2.0
  reserva e a 0.1.x deixava livres (m2:DT11): extra `ENTRY`/`GROUP`/`ENVIRONMENT`/`FILE`/`FILE_*`
  (em qualquer tipo) e `PROTOCOL` em ftp/sftp; anexo `KEY` em server e ftp/sftp. O resto
  das regras é igual nos dois caminhos: os nomes que a 0.1.x já reservava (`URL`, `USER`,
  `PASS`, `TOTP`, `HOST`, `PORT`, `PASSPHRASE`, `TOKEN`) e os nomes repetidos continuam
  recusados também na leitura. Funções `reserved_extra`/`reserved_attachment` (a regra da
  gravação, sem mudança) e `has_reserved_name(entry)`.
- `crates/ward-core/src/body.rs`: o 2º passe usa `validate_stored`. Um cofre v4 ou v5 com
  esses nomes abre, e o v4 é regravado como v5 com os mesmos bytes do v5 equivalente (migra).
- `crates/ward-core/src/delivery.rs`: nova `DeliveryError::ReservedName`. O `build` recusa a
  entrada inteira **antes de qualquer chamada a `Resources`** (sem arquivo temporário da chave
  ou de anexo, sem TOTP, sem túnel) quando algum extra ou anexo tem nome reservado pela regra
  da gravação. A conferência final (`check`) ficou como estava: recusa nome repetido, `LD_*`,
  nome fora do formato e byte nulo.
- `crates/ward-core/src/mask.rs`: `stored_entry_cost` (`pub(crate)`) para a leitura: a entrada
  com nome reservado (entrega recusada) conta só a própria entrada no orçamento D29, que é o
  que `ward ssh`, o túnel e o salto montam; `entry_cost` (gravação) segue recusando com
  `NoDelivery`.
- `crates/ward-core/src/entry.rs`: reexporta `has_reserved_name` no crate.
- No `wardd`, nada muda: o `execution.rs` já trata qualquer erro de entrega que não seja
  `TypeWithoutDelivery` como `DeliveryFailed` (log com o texto fixo, sem valor). A edição
  (`core.rs`) chama `Draft::validate`, então salvar a entrada sem renomear é recusado com
  "vira uma variável que o tipo já entrega" no campo.

## Segurança
- S1 (m2:V4.19): nenhum extra nem anexo troca ou imita uma variável entregue. Fui além da
  conferência final: a entrega recusa a entrada mesmo quando o nome reservado não colide
  (extra `FILE_X` sem anexo `x`; anexo `key` num server com senha, que se passaria por
  `WARD_FILE_KEY`), e recusa antes de criar qualquer arquivo temporário (S2). O `check` final
  continua recusando nome repetido.
- S6/S13 (m2:V4.1): a leitura continua recusando o corpo inteiro com o erro genérico em
  qualquer outra violação; a folga é uma lista fechada de nomes, só no caminho de leitura.
  A gravação (`validate`) não mudou.
- S8/S13: nenhum valor novo em `Debug`/`Display`; o texto do erro novo é fixo.
- Testes de ataque (todos rodados e passando):
  - `body/tests.rs` › `m2_54b_names_reserved_since_v5_open_and_migrate`: 12 casos (cada nome
    reservado, `File-Key`, `file_app_env` igual ao anexo `app.env`, vários de uma vez,
    `PROTOCOL` e anexo `key` no sftp, anexo `key`/`KEY` no server com chave e com senha, nota)
    em v4 e v5: abre, migra com os bytes do v5, a entrega é `ReservedName` com 0 chamadas a
    recursos, e as outras entradas não têm nome reservado.
  - `body/tests.rs` › `m2_54b_other_reserved_names_still_refused_on_read`: nomes já
    reservados na 0.1.x e repetidos (extra e anexo) recusados na leitura, v4 e v5, sem valor
    no erro.
  - `entry/tests.rs` › `m2_54b_reserved_since_v5_accepted_on_read_refused_on_save`: aceito em
    `validate_stored`, recusado em `validate` com o motivo no campo; os estritos e os
    repetidos recusados nos dois; extra vazio recusado na leitura.
  - `delivery/tests.rs` › `attack_m2_54b_stored_reserved_name_does_not_replace_a_variable`:
    16 entradas lidas (login, banco com extra secreto, sftp, server com chave, server com
    senha, genérico com `FILE_APP_ENV` + anexo `app.env`): `ReservedName`, 0 recursos, erro
    sem valor, `entry_cost` = `NoDelivery` e `stored_entry_cost` ok.
  - `delivery/tests.rs` › `m2_54b_saved_entries_have_no_reserved_name`: nenhuma entrada
    aceita na gravação é afetada.
  - Mutação conferida: com o `body.rs` de volta ao `validate`, o teste de abrir/migrar falha;
    sem a guarda do `build`, os três testes de ataque falham.
- Dependências novas: nenhuma.
- Segredos de teste: só valores de D15 (`falso-…`) e textos não secretos
  (`valor-de-teste`, `conteudo-de-teste`); gitleaks no diff: nada.

## Ficou de fora
- Suítes que falam com daemon (regra 6): `crates/wardd/tests/run.rs`, `dev/tests/cli.py`.
  O `ward run` da entrada foi provado no `ward-core` (`delivery::build`), que é o que o
  `wardd` chama.
- Mensagem própria para o usuário: o `ward run` dessa entrada responde `DeliveryFailed`
  (genérico). Ver Problemas.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test --locked -p ward-core --lib -- body:: entry:: delivery:: mask::` | 0 (153 testes) |
| `cargo test --locked -p ward-core --lib -- m2_54b` (mutações, revertidas) | 101 (esperado) |
| `cargo clippy --locked -p ward-core -p wardd --all-targets -- -D warnings` | 0 |
| `cargo fmt --all --check` | 0 |
| `make compile` (log `.dev/logs/fase54b-compile.log`) | 0 |
| `make english-check` (log `.dev/logs/fase54b-english-check.log`) | 0 (0 achado) |
| `gitleaks detect --no-git` no diff da fase | 0 |

`make check` não rodado: o `cargo test` do workspace tem testes que falam com daemon
(regra 6 e Fase 54a).

## Problemas descobertos
- `body/tests.rs` › `v2_31_forged_fields` estava quebrado desde a Fase 53 (caso "anexo do
  servidor chamado chave": `chave` deixou de ser reservado e o corpo passou a abrir; a 53
  rodou só `delivery::`/`entry::`). Troquei o caso por um extra `HOST` no server, que segue
  recusado na leitura.
- Decisão de segurança além do pedido: a entrega recusa qualquer entrada com nome reservado
  pela regra da gravação, mesmo sem colisão de fato (ver Segurança). Só alcança entradas lidas
  de um cofre 0.1.x; depois de renomear o extra/anexo no app, a entrega volta.
- O usuário não fica sabendo o motivo: o `ward run` responde o erro genérico de entrega e o
  log do daemon diz "campo extra ou anexo com nome reservado". Sugiro, na Fase 59/66
  (`docs/instalacao.md`, roteiro de atualização), avisar que extras chamados `ENTRY`, `GROUP`,
  `ENVIRONMENT`, `FILE`/`FILE_*`, `PROTOCOL` (ftp/sftp) e anexos `key` (server/ftp/sftp) têm de
  ser renomeados no app; ou um código de erro próprio no protocolo (fora desta fase).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/body.rs` | alterado |
| `crates/ward-core/src/body/tests.rs` | alterado |
| `crates/ward-core/src/delivery.rs` | alterado |
| `crates/ward-core/src/delivery/tests.rs` | alterado |
| `crates/ward-core/src/entry.rs` | alterado |
| `crates/ward-core/src/entry/tests.rs` | alterado |
| `crates/ward-core/src/entry/validation.rs` | alterado |
| `crates/ward-core/src/mask.rs` | alterado |
| `docs/mission/handoffs/phase-54b.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não (regra 6); fmt,
  clippy, `make compile`, `make english-check` e os testes de unidade do `ward-core` rodados
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
