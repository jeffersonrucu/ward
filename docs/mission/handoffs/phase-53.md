# Fase 53 — Variáveis entregues (DT11)

## Implementado
- `crates/ward-core/src/delivery.rs`: `WARD_ENTRADA`→`WARD_ENTRY`, `WARD_GRUPO`→`WARD_GROUP`,
  `WARD_AMBIENTE`→`WARD_ENVIRONMENT`, `WARD_PROTOCOLO`→`WARD_PROTOCOL`,
  `WARD_ARQUIVO_<NOME>`→`WARD_FILE_<NAME>`, `WARD_ARQUIVO_CHAVE`→`WARD_FILE_KEY`. Só os nomes
  mudaram: zeroização, máscara, percent-encoding, ordem e conferência final (`check`) iguais. O
  nome fixo do arquivo temporário da chave (`"chave"`, só o nome do arquivo em `<rt>tmp/`) ficou.
- `crates/ward-core/src/entry/validation.rs`: nomes reservados acompanham a entrega
  (`COMMON_VARIABLES` = `ENTRY`/`GROUP`/`ENVIRONMENT`; `PROTOCOL` no ftp/sftp; extra `FILE` e
  `FILE_*` reservados; anexo `KEY` reservado em server e ftp/sftp). Comentários em inglês.
- `crates/ward-core/src/entry.rs`: comentários com os nomes novos.
- `dev/english-check/frozen.txt`: saem as 9 exceções da Fase 53 (8 de `entry.rs`/`validation.rs`
  e a do `app/core/src/service.rs`). `make english-check` no repositório: 0 achado.
- Testes (escritos; só os de unidade do `ward-core`/`app/core` rodados, sem daemon):
  - `delivery/tests.rs`: expectativas pela tabela "Entrega esperada"; novos
    `m2_v4_19_no_old_variable_name` e `m2_smtp_security_with_the_v5_name` (`none`/`starttls`/`tls`).
  - `entry/tests.rs`: reservados com os nomes novos (inclui `File-Key` → `FILE_KEY` e anexo `key`);
    novo `m2_dt11_old_names_are_not_reserved`; `WARD_ENTRY` na lista de variável perigosa da api.
  - `crates/wardd/tests/run.rs` (não rodado, fala com daemon de teste): nomes novos e os nomes da
    0.1.x na lista de proibidos do ambiente do filho.
  - `app/core/src/service.rs`: o anexo reservado do teste passa de `chave` para `key`.
  - `dev/tests/cli.py` (não rodado): tabela com `WARD_PROTOCOL`, `WARD_FILE_KEY`,
    `WARD_FILE_APP_ENV` e as comuns novas; para cada entrada da tabela, `ward run <e> -- env -0`
    confere que os `WARD_*` são exatamente os da tabela e que nenhum nome da 0.1.x aparece (só os
    nomes são lidos; a saída passa pelo `no_variation`).
- `app/src/screens/form.js`: textos de ajuda e `attachmentVariable` com `WARD_PROTOCOL`,
  `WARD_FILE_KEY` e `WARD_FILE_<NOME>` (literais só; senão o formulário mostraria nomes que não
  são mais entregues).
- `docs/mission/validation-contract.md`: linha nova na tabela "Entrega esperada" para o
  `SMTP_SECURITY` com o nome v5 (`none` no lugar de `nenhuma`), achado da Fase 48.
- `make dev-vault`, `make dev-vault-verify`, `make dev-variations` e `variations_test.py` passam com
  o `entries.json` atual. O oráculo (`dev/variations.py`), o parser do `dev-vault.sh` e o
  `login.py` não citam nome de variável entregue: nada a mudar neles (o parser e as chaves do
  `entries.json` já tinham mudado na Fase 48).

## Segurança
- S2/S3/S4/S8 (m2:V4.19–V4.21): a lógica da entrega não mudou (diff só nos nomes); os valores
  seguem em `Zeroizing`, sem `Debug` com valor; a conferência final ainda recusa nome repetido,
  `LD_*` e byte nulo, então um extra que colida com um nome do tipo falha mesmo que a validação
  deixasse passar.
- Reservados: o que a entrega produz é exatamente o que a validação reserva
  (`type_variables_match_the_reserved_ones` continua passando com o prefixo `FILE_`).
- Testes de ataque: `delivery/tests.rs` › `m2_v4_19_no_old_variable_name`;
  `entry/tests.rs` › `attack_extra_or_attachment_overrides_type_variable` (nomes novos),
  `attack_dangerous_variable_in_api` (`WARD_ENTRY`); `crates/wardd/tests/run.rs` (proibidos da
  0.1.x no ambiente do filho); `dev/tests/cli.py` (m2:V4.19, nenhum nome antigo e conjunto exato).
- Nenhum segredo novo: só valores de D15 (`falso-SMTP-01`).
- Dependências novas: nenhuma.

## Ficou de fora
- `skill/ward/SKILL.md`, `skill/README.md`, `skill/settings-deny.json` e `dev/tests/skill.py`
  (ataques com `$WARD_ARQUIVO_CHAVE`): Fase 58. O deny deveria negar `WARD_FILE_KEY` nas mesmas
  formas e manter `WARD_ARQUIVO_CHAVE` para a 0.1.x.
- `docs/escopo.md` (`WARD_ARQUIVO_<NOME>` na linha 79) e `docs/glossario.md`: Fase 59.
- `dev/fixtures/legacy/strings-0.1.4.txt`: linha de base, não muda.
- Nenhuma suíte que fala com daemon foi rodada (regra 6): `wardd` `tests/run.rs` e
  `dev/tests/cli.py` ficam para o Milestone 6.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make compile` | 0 |
| `cargo fmt --all --check` (depois de `cargo fmt --all`) | 0 |
| `cargo clippy --locked -p ward-core -p wardd -p ward-app-core --all-targets -- -D warnings` | 0 |
| `cargo test --locked -p ward-core --lib -- delivery:: entry::` | 0 (57 testes) |
| `cargo test --locked -p ward-app-core --lib -- --exact service::tests::attack_large_empty_attachment_and_forged_id` | 0 |
| `make dev-vault` | 0 |
| `make dev-vault-verify` | 0 |
| `make dev-variations` | 0 |
| `python3 -I -B dev/tests/variations_test.py` | 0 (43 testes) |
| `make english-check` | 0 (0 achado) |

`make check` não rodado: inclui o `cargo test` do workspace, que tem testes que falam com daemon
(regra 6 e Fase 54a).

## Problemas descobertos
- **Cofre que deixa de abrir.** A leitura do corpo valida cada entrada (`body.rs`, 2ª passada:
  `Draft::validate`), e um corpo com uma entrada inválida não abre. Com os reservados novos, um
  cofre 0.1.x (v4) ou v5 com um extra chamado `ENTRY`, `GROUP`, `ENVIRONMENT`, `FILE` ou `FILE_*`
  (ou `PROTOCOL` num ftp/sftp), ou com um anexo `key` num server/ftp/sftp, passa a ser recusado
  inteiro no destrave da 0.2.0. Na 0.1.x esses nomes eram livres. Não mexi: decidir entre
  (a) aceitar na leitura e recusar só na gravação (a conferência da entrega já recusa nome
  repetido), (b) avisar no roteiro de atualização (Fase 66 / `docs/instalacao.md`) ou (c) recusar
  como hoje é do orchestrator/planner. Os nomes antigos (`ENTRADA`, `ARQUIVO_*`, anexo `chave`…)
  passam a ser livres (teste `m2_dt11_old_names_are_not_reserved`), então nada que abria deixa de
  abrir por eles.
- O nome fixo do arquivo temporário da chave privada continua `chave` (`<rt>tmp/<n>-chave`). É
  literal e não é variável; se a Fase 60 quiser trocar por `key`, muda `delivery.rs`, o teste
  `v2_14_expected_delivery_of_the_contract` e nada mais.
- O texto de validação "deve ser nenhuma, starttls ou tls" (achado da Fase 48) segue igual.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/delivery.rs` | alterado |
| `crates/ward-core/src/delivery/tests.rs` | alterado |
| `crates/ward-core/src/entry.rs` | alterado |
| `crates/ward-core/src/entry/validation.rs` | alterado |
| `crates/ward-core/src/entry/tests.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/src/screens/form.js` | alterado |
| `dev/tests/cli.py` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/mission/validation-contract.md` | alterado |
| `docs/mission/handoffs/phase-53.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não (regra 6); `make compile`, fmt, clippy e os testes de unidade do `delivery`/`entry` rodados
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase (mais os textos do `form.js`, ver Implementado)
