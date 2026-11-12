# Fase 17 — `ward-core`: `protocol` (+ `fields`, `errors`, `json`, `request`, `frame`) (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 6db0d00`
  (master na hora). Durante a fase o master andou (Fases 23 e 25, `440416c`): o merge fica
  para o orchestrator (ver "Problemas").
- **Arquivos** (glossário seção 4, `git mv`): `src/protocolo.rs` → `src/protocol.rs`,
  `protocolo/campos.rs` → `protocol/fields.rs`, `erros.rs` → `errors.rs`, `json.rs` →
  `json.rs`, `pedido.rs` → `request.rs`, `quadro.rs` → `frame.rs`; e, só de caminho (o
  módulo mudou de pasta), `protocolo/resposta.rs` → `protocol/response.rs` e
  `protocolo/testes.rs` → `protocol/tests.rs` (conteúdo da Fase 18; aqui só os usos dos
  nomes desta fase). `lib.rs`: `pub mod protocol`. Os seis arquivos da fase traduzidos
  por inteiro (comentários, locais, parâmetros, campos privados, a macro `table!`).
- **Renomes** (lista completa no parágrafo "Fase 17" do glossário, seção 15), com todos os
  usos no repositório:
  - `protocol.rs`: `VERSAO` → `VERSION`; `Papel { Agente, App, Extensao }` → `Role { Agent,
    App, Extension }` (`TODOS`/`nome`/`de_nome` → `ALL`/`name`/`from_name`); `Papeis` →
    `Roles` (`agent`/`app`/`extension`, `has`); constantes da tabela `TODOS`/`AGENTE_E_APP`/
    `SO_AGENTE`/`SO_APP`/`SO_EXTENSAO`/`APP_E_EXTENSAO` → `ALL`/`AGENT_AND_APP`/
    `AGENT_ONLY`/`APP_ONLY`/`EXTENSION_ONLY`/`APP_AND_EXTENSION`; macro `tabela!` → `table!`
    (`TODAS`/`nome`/`papeis`/`de_nome`/`permitida` → `ALL`/`name`/`roles`/`from_name`/
    `allowed`); `Operacao` → `Operation` e `TipoResposta` → `ResponseType` com as variantes
    pelos nomes v2 da seção 10 (`Estado` → `Status`, `EntradaPadrao` → `Stdin`, `Janela` →
    `Resize`, `Saida` → `Output`, `Fim` → `Exit`, `Preenchimento` → `Fill`…);
    `carrega_credencial` → `carries_credential`; `ErroCodificar` → `EncodeError`
    (`NotAllowedForRole`, `TooLarge`, `Invalid`); `RespostaInvalida` → `InvalidResponse`;
    `codificar_pedido`/`decodificar_pedido`/`codificar_resposta`/`decodificar_resposta`/
    `erro_do_quadro` → `encode_request`/`decode_request`/`encode_response`/
    `decode_response`/`frame_error`; privados `Envelope`, `ErroEnvelope` →
    `EnvelopeError`, `ler_envelope` → `read_envelope`.
  - `errors.rs`: `ErroResposta` → `ResponseError` (33 variantes pelos códigos v2:
    `Unauthorized`, `Locked`, `LockedTimedOut`, `InvalidRequest`, …, `Internal`),
    `SIMPLES`/`codigo`/`mensagem`/`de_codigo`/`de_validacao` → `SIMPLE`/`code`/`message`/
    `from_code`/`from_validation`; `Problema { campo, posicao, motivo }` → `Problem {
    field, position, reason }`; `MSG_RETROCESSO`/`MSG_COFRE_MUDOU`/`MSG_GERACAO_ESGOTADA` →
    `MSG_ROLLBACK`/`MSG_VAULT_CHANGED`/`MSG_GENERATION_EXHAUSTED`.
  - `request.rs`: `Pedido` → `Request` (variantes como as de `Operation`, campos pela
    tabela de campos da seção 10: `senha` → `password`, `entrada` → `entry`, `janela` →
    `window_size`, `tipo_entrada` → `stdin_kind`, `rascunho` → `draft`, `manter_anexos` →
    `keep_attachments`, `entrada_confirmada` → `confirmed_entry`, `atual`/`novo`/`nova` →
    `current`/`new`, `ligado` → `enabled`…; `operacao`/`conferir`/`escrever` →
    `operation`/`check`/`write`); `PedidoExecutar` → `RunRequest` (`entry`, `env`,
    `stdin_kind`); `Variavel` → `Variable`; `TipoEntradaPadrao` → `StdinKind` (`Null`,
    `File`, `Other`; `hidden`); `Janela` → `WindowSize` (`rows`, `cols`); `Alvo` → `Target`
    (`Field`, `Extra`; `secret_in`); `Decisao` → `Decision` (`Deny`, `AllowOnce`,
    `AllowOneHour`, `AllowAlways`, `Confirm`; `requires_pin`); `ResultadoLogin` →
    `LoginResult` (`Submitted`, `Refused`, `NoForm`, `TabChanged`, `Failure`);
    `id_login_ok` → `login_id_ok`; `juntar_anexos_mantidos` → `merge_kept_attachments`;
    `CAMPOS_SECRETOS` → `SECRET_FIELDS`; privados (`CampoRascunho` → `DraftField`,
    `campos_do_rascunho`/`ler_rascunho`/`escrever_rascunho`/`ler_dados` → `draft_fields`/
    `read_draft`/`write_draft`/`read_data`, conferências `*_ok`).
  - `frame.rs`: `BLOCO`/`CABECALHO` → `BLOCK`/`HEADER`, `limite_do_quadro`/
    `LIMITE_RESPOSTA` → `frame_limit`/`RESPONSE_LIMIT`, `ErroQuadro` → `FrameError`
    (`Closed`, `Truncated`, `Empty`, `TooLarge`, `Io`), `ler_cabecalho`/`validar_tamanho`/
    `ler_quadro`/`escrever_quadro` → `read_header`/`validate_size`/`read_frame`/
    `write_frame`.
  - `fields.rs` (crate): `Campos` → `Fields` e métodos, `texto_de`/`bytes_de`/`segredo_de`/
    `lista_de`/`textos_de` → `text_of`/`bytes_of`/`secret_of`/`list_of`/`texts_of`.
  - `json.rs` (crate): `Valor` → `Value` (`Null`, `Bool`, `Integer`, `Text`, `List`,
    `Object`), `JsonInvalido` → `InvalidJson`, `Leitor` → `Reader`, `Escritor` → `Writer`
    e métodos, `ler`/`codificar` → `read`/`encode`, `de_base64`/`tamanho_base64` →
    `from_base64`/`base64_len`, `MAX_PROFUNDIDADE`/`MAX_NOS`/`MAX_NOS_RESPOSTA`/
    `MAX_CHAVES` → `MAX_DEPTH`/`MAX_NODES`/`MAX_RESPONSE_NODES`/`MAX_KEYS`.
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`lib.rs`, `wire_v1.rs`,
  `tests/corpo_memoria.rs`, `protocol/response.rs`, `protocol/tests.rs`), `wardd` (`src/`
  `aprovacao`, `execution`, `login`, `nucleo`, `playwright`, `servidor`, `ssh`, `trava`,
  `udisks`, `uso`; `app-teste/`, `aprovador-teste/`, `examples/test-client.rs`; 20 testes
  de `tests/`), `ward` (`client`, `commands`, `tests`, `text`), `ward-nm` (`lib`, `main`,
  `tests`) e `app/core` (`aprovacao`, `backup_externo`, `client`, `credenciais`, `daemon`,
  `destrave`, `fake_daemon`, `formulario`, `partida`, `restaurar`, `service`). Feito por
  script (scratchpad, não versionado): um léxico de Rust que só mexe no código (comentário e
  literal intactos), com caminhos `ward_core::protocolo`/`protocolo::`/`quadro::`,
  variantes qualificadas (`Pedido::Estado` → `Request::Status`…) e os tipos e funções pelo
  nome inteiro (nunca depois de `X::` de outro tipo, para não pegar `ErroPartida::Papel`);
  depois os métodos e campos pelos diagnósticos do `cargo check` (E0599, E0609, E0026,
  E0559, E0560, só quando o tipo da mensagem é desta fase). Campo abreviado em padrão ou
  construção dos arquivos de outras fases virou `novo: local` (`password: senha`), para não
  renomear local de outra fase; onde o local já era o nome novo, o `x: x` virou `x`
  (lint do rustc e `clippy::redundant_field_names`). À mão: o `use … {…, quadro}` de
  `execution.rs` e `partida.rs`, o alias `StdinKind as T` (`T::Nula`/`T::Outra`/
  `T::Arquivo`/`T::TODOS`) de `ward/src/tests.rs`, a variante `ErroPartida::Papel`
  (o script tinha trocado a declaração; voltou, o tipo dentro é `Role`) e os links de doc
  `[`super::codificar_resposta`]`/`[`super::TipoResposta::carrega_credencial`]` de
  `response.rs` e `[`ErroResposta::CofreMudou`]` de `nucleo.rs`.
- **`dev/tests/socket_matrix.py`** (achado da Fase 45): lê `crates/ward-core/src/protocol.rs`,
  corta em `"Operation, "` … `"table!"` e `AGENT_ROLES = {"ALL", "AGENT_AND_APP",
  "AGENT_ONLY"}`. Conferido rodando o mesmo recorte: 36 operações, os mesmos nomes de fio
  na mesma ordem do original.
- **Glossário**: parágrafo "Fase 17" depois do da Fase 16 (seção 15).
- **Exceções**: 5 linhas novas em `frozen.txt` (seção "Phase 17"): as chaves do envelope v1
  citadas no exemplo do doc do módulo (`dados`, `tipo`, `erro`, `codigo`, `mensagem`; Fase
  50). Nada em `allow.txt`.

## Literais alterados (PRD regra 5)
Conferido por multiconjunto de literais (script na scratchpad, comentários fora) em todos os
arquivos tocados, contra o `6db0d00`:
- **Nenhum literal de fio mudou**: operações, tipos de resposta, códigos de erro, papéis
  (`"agente"`, `"extensao"`), campos (`"dados"`, `"senha"`, `"tipo_entrada"`, `"janela"`…),
  valores (`"nula"`, `"uma_vez"`, `"enviado"`…), mensagens pt-BR de erro e de `Display`.
- `protocol.rs`: os textos de doc das variantes passados à macro `table!` (`$doc`/`$vdoc`,
  viram `#[doc]`) foram traduzidos, como os outros comentários; os nomes de fio (`$txt`) não.
- `json.rs`: o `Debug` de `Value` acompanha o nome (`"Nulo"`/`"Logico"`/`"Inteiro"`/
  `"Texto(••••)"`/`"Lista"`/`"Objeto"` → `"Null"`/`"Bool"`/`"Integer"`/`"Text(••••)"`/
  `"List"`/`"Object"`), como o `"Mask(••••)"` da Fase 16; continua sem conteúdo (S13).
  Nenhum script nem teste procura esses textos.
- `protocol/tests.rs`: os `include_str!` do V3.15 acompanham os arquivos (`"../protocolo.rs"`,
  `"quadro.rs"`, `"campos.rs"`, `"pedido.rs"`, `"resposta.rs"`, `"erros.rs"` → `"../protocol.rs"`,
  `"frame.rs"`, `"fields.rs"`, `"request.rs"`, `"response.rs"`, `"errors.rs"`): o teste
  continua lendo os sete fontes do protocolo.
- Os `expect`/`assert!` pt-BR do `json.rs` (`"o texto realocou"`, `"as passadas do escritor
  divergiram"`, `"o escritor realocou"`) ficam.
- Nenhum `\uXXXX` foi decodificado (a conferência mostra só as diferenças acima; o
  `'\u{8}'`/`'\u{c}'` e o `b"\\u"` do `json.rs` estão byte a byte).
- **Conferido por forma** (tokens com identificadores normalizados, sem comentários, com
  `x: y` → `x` e vírgula final normalizados): `fields.rs`, `json.rs`, `frame.rs` iguais;
  `protocol.rs`, `errors.rs` e `request.rs` só diferem na ordem do `use` (rustfmt), chaves
  de braço de `match` e vírgulas (rustfmt). Nos arquivos de outras fases a forma só muda pelos
  campos abreviados (`password: senha` ↔ `password`) e pelo rustfmt.
- **JSON byte a byte**: rodei, além da compilação, `cargo test -p ward-core --lib --
  protocol wire_v1` (58 testes: os 48 do protocolo e os 10 de fio, que comparam a
  serialização com `dev/fixtures/legacy/protocol-v1.jsonl`): todos passam.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - D5/S1/S9/S10/V3.15: a mesma tabela operação × papel e tipo × papel (mesmas linhas, mesmos
    papéis), a mesma asserção de compilação (nenhuma resposta com credencial ao agente;
    aprovar, nível, revelar, copiar, token, preencher, login, backups e configuração fora do
    agente), a mesma ordem de leitura JSON → versão → operação → **papel** → campos, a
    mesma recusa de lista com nível/resumo para quem não é o app (D50, D65 c) e de `estado`
    com papel de outra conexão (D63). Sem feature, variável ou argumento (V3.15 continua
    lendo os sete fontes).
  - V3.3/V3.4: os mesmos limites de quadro por papel antes de ler e alocar, blocos de 64 KiB,
    tamanho 0 e "negativo" recusados, erros sem bytes do quadro.
  - S8/S13: o mesmo JSON estrito (profundidade, nós, chaves, chave repetida, inteiros
    canônicos, substitutos), `Zeroizing` de capacidade exata no leitor e no escritor, base64
    do `base64ct` de tempo constante para todo segredo, `Debug` de `Value` sem conteúdo,
    erros de texto fixo.
  - D39: nenhum campo de geração aceito ou enviado; `Rollback`/`VaultChanged`/
    `GenerationExhausted` com as mesmas mensagens fixas.
- **Testes de ataque**: nenhum novo (só rename); os existentes (V3.3 para toda operação de
  outro papel, V3.4 quadros, V3.15 fontes, campos inventados, chave repetida, segredos fora do
  agente) acompanham os nomes e passaram (ver acima).
- **Dependências novas**: nenhuma.

## Ficou de fora
- `protocol/response.rs` e `protocol/tests.rs` (Fase 18): só mudaram de caminho e nos usos
  dos nomes desta fase; comentários, locais e os nomes de lá (`Resposta`, `ItemLista`,
  `PedidoDeAprovacao`, `VersaoBackup`, `Fluxo`, `MAX_NOS_RESUMO`, `ler_cabecalho` próprio…)
  ficam para ela. Os 29 achados que sobram no `english-check` desta fase são todos esses
  identificadores da Fase 18 usados em `protocol.rs`/`json.rs` (`Resposta`, `tipo()`,
  `conferir`, `escrever`, `nivel`, `resumo`, `papel`, `ler`, `MAX_NOS_RESUMO`…).
- Locais e comentários pt-BR dos arquivos que usam os nomes novos ficam para as fases deles
  (`wardd` 19–27, `app/core` 32–34; `ward`/`ward-nm` já traduzidos na Fase 28 esperavam estes
  nomes). Comentários de outras fases que citam os nomes antigos sem link de doc (ex.:
  `codificar_resposta` em `response.rs`) ficam.
- O `docs/mission/plan.md` ainda descreve a Fase 18 como `protocolo/resposta.rs`,
  `protocolo/testes.rs`: agora são `protocol/response.rs` e `protocol/tests.rs` (não mexi no
  plano).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 6db0d00` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked --message-format=json` (logs `.dev/logs/fase17-check{1..10}.json`; 10 rodadas com o corretor de diagnósticos) | 0 na última |
| `cargo fmt --all` / `cargo fmt --all --check` | 0 / 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (logs `fase17-clippy*.{log,json}`; 1 rodada para os `x: x`) | 0 |
| `cargo test -p ward-core --lib --locked -- protocol wire_v1` (extra; log `fase17-test-protocol.log`) | 0 (58 passaram) |
| `python3 -I dev/english_check.py --paths <os 6 arquivos da fase>` (= `make english-check PATHS=…`; log `fase17-english.log`) | 1 (29 achados, todos identificadores da Fase 18; 0 de comentário; 0 problema nas exceções) |
| `make compile` (log `fase17-make-compile.log`; inclui o `cargo check` do `app/src-tauri`, build do Vite e `syntax_check.py`) | 0 |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `6db0d00`; reset com a árvore limpa.
- **O master andou durante a fase** (`440416c`: Fases 23 e 25). O branch parte de
  `6db0d00`; o merge vai ter conflito nos arquivos que essas fases renomearam (ex.:
  `wardd/src/servidor.rs`, testes `aprovacao`/`trava`/`nucleo`/`socket`/`datas`/`memoria`/
  `comum`): lá vale aplicar de novo só os usos desta fase (o script e o corretor de
  diagnósticos estão descritos acima).
- **Arquivos de fases paralelas tocados (só usos)**: Fase 23 (`wardd/src/servidor.rs`,
  `udisks.rs`; testes `comum/mod.rs`, `memoria.rs`, `socket.rs`, `datas.rs`), Fase 27
  (testes `playwright.rs`, `extensao.rs`, `login.rs`, `app_valor.rs`, `valor_app.rs`,
  `app_teste.rs`, `anexos_edicao.rs`) e Fase 32 (`app/core` `formulario.rs`,
  `aprovacao.rs`, `restaurar.rs`): só imports, tipos, variantes, métodos e campos desta
  fase; nenhum local renomeado. O `Pedido` próprio de `wardd/tests/playwright.rs` (struct
  do servidor HTTP falso) e o `Papeis` próprio de `wardd/src/servidor.rs` não mudaram.
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 16) e
  `dev/english-check/frozen.txt` (seção nova no fim).
- **Ferramenta**: o ambiente recusa comando composto com `git`/heredoc longo no worktree
  isolado; scripts foram escritos na scratchpad e rodados à parte.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocol.rs` (era `protocolo.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/fields.rs` (era `protocolo/campos.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/errors.rs` (era `protocolo/erros.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/json.rs` (era `protocolo/json.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/request.rs` (era `protocolo/pedido.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/frame.rs` (era `protocolo/quadro.rs`) | renomeado e reescrito |
| `crates/ward-core/src/protocol/response.rs` (era `protocolo/resposta.rs`) | renomeado; usos |
| `crates/ward-core/src/protocol/tests.rs` (era `protocolo/testes.rs`) | renomeado; usos e `include_str!` |
| `crates/ward-core/src/{lib,wire_v1}.rs`, `crates/ward-core/tests/corpo_memoria.rs` | alterado (usos) |
| `crates/wardd/src/{aprovacao,execution,login,nucleo,playwright,servidor,ssh,trava,udisks,uso}.rs` | alterado (usos) |
| `crates/wardd/{app-teste,aprovador-teste}/main.rs`, `crates/wardd/examples/test-client.rs` | alterado (usos) |
| `crates/wardd/tests/{anexos_edicao,app_teste,app_valor,aprovacao,backup,configuracoes,d62,datas,extensao,externo,login,memoria,nucleo,playwright,run,socket,ssh,trava,valor_app}.rs`, `crates/wardd/tests/comum/mod.rs` | alterado (usos) |
| `crates/ward/src/{client,commands,tests,text}.rs` | alterado (usos) |
| `crates/ward-nm/src/{lib,main,tests}.rs` | alterado (usos) |
| `app/core/src/{aprovacao,backup_externo,client,credenciais,daemon,destrave,fake_daemon,formulario,partida,restaurar,service}.rs` | alterado (usos) |
| `dev/tests/socket_matrix.py` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-17.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt`, `cargo clippy -D warnings` (0) e os 58 testes de protocolo e de fio (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase

## Merge com o master `46138ad` e `2e23f1d` (Fases 23, 25, 26, 27, 32 e 33)
- Dois merges (o primeiro, com `46138ad`, foi commitado antes de o master andar para
  `2e23f1d` e só compila junto com o segundo). Nos arquivos dessas fases ficou a versão do
  master (conflito de conteúdo) e os antigos que voltaram (modify/delete: `servidor.rs`,
  testes `comum/`, `aprovacao`, `trava`, `nucleo`, `externo`, `configuracoes`, `datas`,
  `memoria`, `extensao`, `app_valor`, `valor_app`, `anexos_edicao`; `app/core`
  `formulario`, `aprovacao`, `restaurar`, `backup_externo`, `credenciais`, `partida`)
  foram apagados. Os usos desta fase foram reaplicados pelo mesmo script e pelo corretor de
  diagnósticos (9 rodadas do `cargo check`), à mão o `use … frame` de `app/core/src/startup.rs`;
  os `x: x` pelo clippy (`app/core/src/client.rs`).
- O `StartupError::Role(Role)` da Fase 33 é dela e ficou. O V3.15 de `wardd/tests/approval.rs`
  só lê `wardd/src`: nada desta fase.
- Literais conferidos de novo por script contra `2e23f1d`: só os `include_str!` de
  `protocol/tests.rs`; os arquivos da fase iguais à conferência de antes. Glossário com as
  partes de todas as fases, sem marca de conflito.
- `cargo check` 0, `cargo clippy -D warnings` 0, `cargo fmt --check` 0,
  `cargo test -p ward-core --lib -- protocol wire_v1` 58 ok (log
  `fase17-merge-test-protocol.log`), `make compile` 0 (log `fase17-merge-make-compile.log`).
