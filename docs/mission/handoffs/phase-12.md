# Fase 12 — `ward-core`: `entry`, `entry/secret`, `entry/validation` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard a8dde4c`.
  No meio da fase, a pedido do orchestrator, `git merge 118d248` (fast-forward; a Fase 43
  moveu o exemplo para `crates/ward-core/examples/dev-vault/{main,tests}.rs`), e os
  renomes desta fase foram aplicados também nesses arquivos.
- **Arquivos** (glossário seção 4): `src/entrada.rs` → `entry.rs`,
  `entrada/segredo.rs` → `entry/secret.rs`, `entrada/validacao.rs` →
  `entry/validation.rs`; `entrada/testes.rs` → `entry/tests.rs` (só `git mv` e os usos dos
  nomes desta fase; a tradução do arquivo é da Fase 13). `lib.rs`: `pub mod entry`.
- **Comentários, locais, parâmetros e privados** dos 3 arquivos em inglês.
- **Renomes públicos e `pub(crate)`** (antigo → novo), com todos os usos no repositório:
  - `Segredo` → `Secret` (`de_texto`/`de_bytes`/`expor`/`expor_texto` → `from_text`/
    `from_bytes`/`expose`/`expose_text`).
  - `Ambiente` → `Environment`; `NivelAcesso` → `AccessLevel` (`Bloqueado`/`ComAprovacao`/
    `Livre` → `Blocked`/`Approval`/`Free`, `mais_restritivo` → `most_restrictive`);
    `TipoEntrada` → `EntryType` (`Banco`/`Servidor`/`NotaSegura`/`Generico` → `Database`/
    `Server`/`SecureNote`/`Generic`); `Motor` → `Engine`; `SegurancaSmtp` →
    `SmtpSecurity` (`Nenhuma` → `None`); `Protocolo` → `Protocol`. Em todos: `TODOS`/`TODAS`
    → `ALL`, `nome`/`de_nome`/`rotulo`/`porta_padrao` → `name`/`from_name`/`label`/
    `default_port`.
  - `Autenticacao { Senha, Chave { chave_privada, passphrase } }` → `Auth { Password, Key {
    private_key, passphrase } }`; structs `Login`, `Banco` → `Database`, `Servidor` →
    `Server`, `Api`, `Aws`, `Smtp`, `FtpSftp`, `NotaSegura` → `SecureNote` com os campos
    `usuario`/`senha`/`totp_semente`/`motor`/`porta`/`base`/`tunel`/`autenticacao`/`salto`/
    `variavel`/`regiao`/`seguranca`/`protocolo`/`texto` → `username`/`password`/`totp_seed`/
    `engine`/`port`/`db_name`/`tunnel`/`auth`/`jump`/`variable`/`region`/`security`/
    `protocol`/`text`.
  - `Dados` → `Data` (`tipo()` → `kind()`); `ValorExtra { Texto, Segredo }` → `ExtraValue {
    Text, Secret }`; `CampoExtra` → `ExtraField` (`nome`/`valor`/`e_segredo` → `name`/
    `value`/`is_secret`); `Anexo` → `Attachment` (`conteudo` → `content`);
    `OrigemSegredo { Campo, Extra, Anexo }` → `SecretOrigin { Field, Extra, Attachment }`;
    `SegredoDaEntrada { origem, valor }` → `EntrySecret { origin, value }`; `Datas {
    criada_em, alterada_em }` → `Dates { created_at, modified_at }` (`NENHUMA`, `instante`,
    `de_partes`, `carimbo_novo`, `alterado` → `NONE`, `instant`, `from_parts`, `new_stamp`,
    `modified`).
  - `Entrada` → `Entry`: `nome`, `grupo`, `ambiente`, `projeto`, `tags`, `nivel`,
    `definir_nivel`, `datas`, `definir_datas`, `tipo`, `dados`, `extras`, `anexos`,
    `segredos`, `substituir_por`, `referencia`, `conferir_no_cofre` → `name`, `group`,
    `environment`, `project`, `tags`, `level`, `set_level`, `dates`, `set_dates`, `kind`,
    `data`, `extras`, `attachments`, `secrets`, `replace_with`, `reference`,
    `verify_in_vault`.
  - `validation`: `ErroValidacao { campo, posicao, motivo }` → `ValidationError { field,
    position, reason }`; `ErrosValidacao` → `ValidationErrors` (`lista`/`tem`/`tem_motivo`
    → `list`/`has`/`has_reason`); `Rascunho { nome, grupo, ambiente, projeto, tags, dados,
    extras, anexos }` → `Draft { name, group, environment, project, tags, data, extras,
    attachments }` (`validar` → `validate`); `RascunhoDados` → `DataDraft` (mesmos
    variantes e campos do `Data`, mais `chave_privada` → `private_key`); `RascunhoExtra {
    nome, valor, segredo }` → `ExtraDraft { name, value, secret }`; `RascunhoAnexo { nome,
    conteudo }` → `AttachmentDraft { name, content }`; `GRANDE_PARA_MASCARA` →
    `TOO_LARGE_FOR_MASK`; `nome_variavel` → `variable_name`; `origem_da_url` →
    `url_origin`; `origem_local_da_url` → `local_url_origin`; `url_valida` → `valid_url`;
    `separar_host_porta` → `split_host_port`; `VARIAVEIS_COMUNS` → `COMMON_VARIABLES`;
    `variaveis_do_tipo`/`anexos_do_tipo` → `type_variables`/`type_attachments`. Privados:
    `Coletor` → `Collector`, constantes de mensagem (`OBRIGATORIO` → `REQUIRED`…),
    predicados `*_valido(a)` → `valid_*`, `motivo_linha`/`motivo_segredo` →
    `line_reason`/`secret_reason`, `caractere_de_exibicao` → `display_character`,
    `host_local` → `local_host`, `chave_parece_privada` → `looks_like_private_key`.
- **Usos ajustados** (só os nomes desta fase; 71 arquivos): `ward-core` (corpo e testes,
  `cofre_teste`, entrega e testes, mascara e testes, protocolo `campos`/`erros`/`pedido`/
  `resposta`/`testes`, `origin`, `generator`, `totp`, `atomic_write`, `limits/tests`,
  `backup/testes`, `wire_v1.rs`, `tests/corpo_memoria.rs`, `examples/dev-vault`), `wardd`
  (src e testes), `ward`, `ward-nm/src/testes.rs` e `app/core`. Feito pelo compilador: um
  script (scratchpad, não versionado) lê os diagnósticos JSON do `cargo check` e troca o
  identificador só no span de cada erro causado pelo rename (tipo novo citado na
  mensagem); campo abreviado vira `novo: antigo`, para manter o nome do local. Os
  `redundant_field_names` que isso gerou no `dev-vault` (locais já em inglês pela Fase 43)
  e no `limits/tests.rs` voltaram à forma abreviada. Links de doc (`[`…`]`) para os nomes
  desta fase em `corpo.rs`, `protocolo/{erros,pedido,campos,resposta}.rs`,
  `corpo/testes.rs`, `entrega.rs`, `wardd/src/{uso,nucleo}.rs` acompanharam.
- **Homônimos que não mudaram**: o `Segredo` do `app/core/src/segredo.rs` (e o `Debug`
  `"Segredo(••••)"` dele), o `Motor` do `wardd/src/aprovacao.rs`, o `ValorExtra` do
  `app/core/src/formulario.rs`, os `Anexo` de `wardd/src/playwright.rs` e
  `app/core/src/credenciais.rs`, `Resposta::Segredo`/`TipoResposta::Segredo`,
  `CampoRascunho::Segredo` (Fase 17), `TipoEntradaPadrao` (Fase 17).
- **Glossário**: parágrafo "Fase 12" na seção 15.
- **Exceções**: 11 linhas em `frozen.txt` (seção "Phase 12"): os nomes gravados
  `bloqueado`/`com-aprovacao`/`livre` citados no doc de `AccessLevel::name` (Fase 48) e as
  variáveis entregues `WARD_AMBIENTE`, `WARD_PROTOCOLO`, `WARD_ARQUIVO_*`, `WARD_ENTRADA`,
  `WARD_GRUPO`, `WARD_ARQUIVO_CHAVE` citadas nos comentários (Fase 53). Nenhuma em
  `allow.txt`.

## Literais alterados (PRD regra 5)
- `entry/secret.rs`: `Debug` `"Segredo(••••)"` → `"Secret(••••)"` (o valor continua
  `••••`); o teste `entry/tests.rs` que confere esse `Debug` acompanha.
- `entry/validation.rs`: `Debug` manuais `debug_struct("RascunhoExtra")` → `"ExtraDraft"`
  (campos `"nome"`, `"valor"`, `"segredo"` → `"name"`, `"value"`, `"secret"`),
  `"RascunhoAnexo"` → `"AttachmentDraft"` (`"nome"`, `"conteudo"` → `"name"`,
  `"content"`), `"RascunhoDados({})"` → `"DataDraft({})"`, `debug_struct("Rascunho")` →
  `"Draft"` com o campo `"tipo"` → `"kind"`. Nenhum script nem teste procura esses textos.
  O `{}` do `DataDraft` continua o `name()` do tipo (nome gravado).
- **Ficaram**: todos os literais de `name()` (nomes gravados no cofre: `banco`,
  `servidor`, `nota-segura`, `generico`, `bloqueado`, `com-aprovacao`, `livre`, `nenhuma`…,
  Fase 48) e de `label()` (texto exibido); todas as mensagens de validação pt-BR (só os
  nomes das constantes mudaram); os rótulos de campo das mensagens (`"nome"`, `"usuário"`,
  `"porta"`, `"túnel SSH"`…) e de `SecretOrigin::Field` (`"senha"`, `"chave privada"`…);
  `"entrada inválida: "`; os sufixos `WARD_*` (`"PROTOCOLO"`, `"ENTRADA"`, `"ARQUIVO"`,
  `"CHAVE"`…).
- **Fora dos 3 arquivos**: nenhum literal mudou (conferido por multiconjunto de literais
  entre linhas removidas e adicionadas do `git diff`, por arquivo: zero diferença), exceto
  a expectativa do `Debug` acima.
- **Debug derivado**: os `#[derive(Debug)]` de `Entry`, `Data`, `AccessLevel` etc. agora
  imprimem os nomes ingleses; procurei `ComAprovacao`, `NotaSegura`, `TipoEntrada`,
  `NivelAcesso` etc. em scripts e testes: nenhum uso (só `strings-0.1.4.txt`, que o V6.3
  deve aceitar, como nas Fases 8–11). Nenhum `{:?}` desses tipos vai para texto exibido.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S8/S13 (`Secret`): zera ao sair (`Drop` + `ZeroizeOnDrop`), sem `Clone`/`PartialEq`/
    `Hash`/`Deref`, `Debug`/`Display` só `••••`; `ZeroizeOnDrop` de `Entry`, `Data`,
    `ExtraField`, `Attachment`, `Auth` igual. Erros de validação só com `&'static str`.
  - S9: `Draft::validate` cria com `AccessLevel::Approval` (o default, antigo
    `ComAprovacao`); `replace_with` mantém o nível; `set_level` continua só para o daemon.
    A ordem de `most_restrictive` (Blocked 0 < Approval 1 < Free 2) é a mesma.
  - Domínio/origem (D9, D41, D55, Fase 44e): `valid_url`, `url_origin`,
    `local_url_origin`, `local_host`, `valid_url_host`, `split_host_port` com a mesma
    lógica, só nomes.
  - Listas de variáveis proibidas (D25, D42) e reservadas (D10) iguais.
  - A forma dos 3 arquivos foi revista à mão contra os originais (mesmas expressões, mesma
    ordem; renomes de locais só).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- `entry/tests.rs`: só o `git mv` e os usos; comentários e nomes do arquivo são da Fase 13.
- Os 12 achados que sobram no `english-check` são identificadores da Fase 16
  (`mascara`, `ErroMascara::{GrandeDemais, SemEntrega}`, `custo_da_entrada`).
- Locais e comentários pt-BR dos outros arquivos que usam os nomes novos ficam para as
  fases deles (`pedido.rs:351` ainda diz `RascunhoDados::Login` num comentário comum).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard a8dde4c` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `git merge --no-edit 118d248` (pedido do orchestrator; fast-forward) | 0 |
| script de rename por diagnóstico (`cargo check --workspace --all-targets --all-features --locked --message-format=json`, várias rodadas até 0 erro; logs `.dev/logs/fase12-fix*.log`) | 0 |
| `cargo fmt --all` / `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra; log `fase12-clippy.log`) | 0 |
| `make compile` (log `fase12-make-compile.log`; inclui o `cargo check` do `app/src-tauri`) | 0 |
| `python3 -I dev/english_check.py --paths crates/ward-core/src/entry.rs crates/ward-core/src/entry/secret.rs crates/ward-core/src/entry/validation.rs` (log `fase12-english.log`) | 1 (12 achados, todos nomes da Fase 16; 0 problema nas exceções) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `a8dde4c`; reset com a árvore limpa.
- **Arquivo da trilha S tocado** (pedido do orchestrator): `crates/ward-core/examples/
  dev-vault/{main,tests}.rs` (Fase 43), só os usos dos nomes desta fase.
- **Arquivos compartilhados**: `dev/english-check/frozen.txt` (11 linhas no fim) e
  `docs/glossario.md` (parágrafo depois do da Fase 11). Nenhum script de `dev/` tocado.
- `SmtpSecurity::None` é um variante chamado `None`: só aparece qualificado; um
  `use SmtpSecurity::*` faria sombra ao `Option::None`. Nenhum hoje.
- `entry/tests.rs` confere o `Debug` do `Secret` (`"Secret(••••)"`); o do `Segredo` do
  `app/core` continua `"Segredo(••••)"` até a Fase 30.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/entry.rs`, `entry/secret.rs`, `entry/validation.rs` (eram `entrada.rs`, `entrada/segredo.rs`, `entrada/validacao.rs`) | renomeado e reescrito |
| `crates/ward-core/src/entry/tests.rs` (era `entrada/testes.rs`) | renomeado (usos) |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/{atomic_write,corpo,entrega,generator,mascara,origin,totp,wire_v1}.rs`, `atomic_write/tests.rs`, `backup/testes.rs`, `corpo/{cofre_teste,testes}.rs`, `entrega/testes.rs`, `generator/tests.rs`, `limits/tests.rs`, `mascara/testes.rs`, `totp/tests.rs`, `protocolo/{campos,erros,pedido,resposta,testes}.rs` | alterado (usos) |
| `crates/ward-core/tests/corpo_memoria.rs`, `crates/ward-core/examples/dev-vault/{main,tests}.rs` | alterado (usos) |
| `crates/wardd/src/{agente_ssh,cdp,execucao,login,nucleo,playwright,ponte,servidor,ssh,uso}.rs`, `crates/wardd/app-teste/main.rs`, `crates/wardd/aprovador-teste/main.rs`, `crates/wardd/examples/test-client.rs` | alterado (usos) |
| `crates/wardd/tests/{anexos_edicao,app_valor,aprovacao,backup,datas,extensao,login,nucleo,ssh,valor_app}.rs` | alterado (usos) |
| `crates/ward/src/{cliente,comandos,testes,texto}.rs`, `crates/ward-nm/src/testes.rs` | alterado (usos) |
| `app/core/src/{aprovacao,cliente,credenciais,daemon,daemon_falso,destrave,formulario,gerador,nivel,servico,terminal}.rs` | alterado (usos) |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-12.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
