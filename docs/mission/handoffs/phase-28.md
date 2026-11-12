# Fase 28 — Clientes Rust: `crates/ward` e `crates/ward-nm` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 66eb87b`.
  Fase adiantada em paralelo com as Fases 14, 29 e 30 (pedido do orchestrator): só os
  nomes próprios de `ward` e `ward-nm` mudaram; os de `ward-core`/`ward-platform` (`Pedido`,
  `Resposta`, `ErroResposta`, `Papel`, `quadro`, `TipoEntradaPadrao`, campos `entrada`/
  `senha`/`tipo_entrada`…) ficaram como no master e são usados com o nome atual.
- **Arquivos** (`git mv`, glossário seção 4): `crates/ward/src/{cliente,comandos,texto,testes}.rs`
  → `{client,commands,text,tests}.rs`; `crates/ward-nm/src/testes.rs` → `tests.rs`;
  `crates/ward-nm/tests/binario.rs` → `binary.rs`. `examples/fake-chrome.rs` já estava em
  inglês (Fase 43).
- **Renomes** (antigo → novo; lista completa no parágrafo "Fase 28" do glossário, seção 15):
  - `ward`: `principal` → `main`; `Comando` → `Command` (`Lista`/`Travar`/`Destravar`/`Ajuda`/
    `Versao` → `List`/`Lock`/`Unlock`/`Help`/`Version`; campos `entrada`/`comando` →
    `entry`/`command`); `ErroUso` → `UsageError` (variantes `NoCommand`, `UnknownCommand`,
    `UnexpectedArgument`, `NotUtf8`, `RunUsage`, `SshUsage`, `TokenInArgv`,
    `PlaywrightUsage`, `LoginUsage`; `mensagem()` → `message()`); `AJUDA` → `HELP`;
    `interpretar`/`interpretar_run`/`interpretar_ssh`/`numero_fd` → `parse`/`parse_run`/
    `parse_ssh`/`fd_number` (`token_ok` fica).
  - `client`: `ErroConexao` → `ConnectionError` (`Missing`, `Untrusted`, `NoResponse`,
    `Closed`, `Io`, `InvalidResponse`, `InvalidRequest`); `Conexao` → `Connection`
    (`conectar`/`conectar_com`/`conectar_sem_extra`/`reconferir`/`conferir_extra`/`pedir`/
    `pedir_com_prazo`/`abrir_sessao` → `connect`/`connect_with`/`connect_without_extra`/
    `reverify`/`verify_extra`/`request`/`request_with_timeout`/`open_session`; campo
    `exigir_nao_dumpable` → `require_non_dumpable`); `Sessao`/`EscritaSessao` → `Session`/
    `SessionWriter` (`receber`/`entrada`/`janela` → `receive`/`send_stdin`/`send_window`);
    `servidor_esperado` → `expected_server`; `PRAZO_LEITURA`/`PRAZO_ESCRITA` →
    `READ_TIMEOUT`/`WRITE_TIMEOUT`; `erro_de_io`/`erro_do_quadro` → `io_error`/`frame_error`.
  - `commands`: `Entorno` → `Env` (métodos `connect`, `harden`, `read_password`, `stdout`,
    `stderr`, `environment`, `current_dir`, `stdin`, `stdin_kind`, `terminal`,
    `terminal_size`, `raw_mode`, `read_token`); `Sistema { esperado, … }` → `System {
    expected, … }`; `executar` → `execute` (não `run`, que é o handler do `ward run`);
    `SAIDA_*` → `EXIT_OK`/`EXIT_ERROR`/`EXIT_USAGE`/`EXIT_RUN_ERROR`; `MSG_RUN_GRANDE`/
    `MSG_RUN_CAMPO`/`MSG_RUN_TRAVADO`/`MSG_TOKEN_FORMATO` → `MSG_RUN_TOO_LARGE`/
    `MSG_RUN_FIELD`/`MSG_RUN_LOCKED`/`MSG_TOKEN_FORMAT`; `PEDACO_ENTRADA` → `STDIN_CHUNK`;
    `PASSO_JANELA` → `WINDOW_STEP`; `PRAZO_PLAYWRIGHT` → `PLAYWRIGHT_TIMEOUT`;
    `lista`/`travar`/`destravar` → `list`/`lock`/`unlock`; `pedido_run`/`pedido_ssh` →
    `run_request`/`ssh_request`; `falhar`/`falhar_run` → `fail`/`fail_run`;
    `mensagem_do_daemon`/`mensagem_do_login` → `daemon_message`/`login_message`;
    `variavel_mandada` → `variable_sent`; `repassar_entrada`/`receber_sessao` →
    `forward_stdin`/`receive_session`; `token_do_canal` → `token_from_channel`;
    `acompanhar_janela` → `track_window`; `janela_de` → `window_from`;
    `tipo_da_entrada_padrao`/`classificar_entrada` → `detect_stdin_kind`/`classify_stdin`.
  - `text`: `invisivel`/`neutralizar`/`json_texto`/`json_opcional`/`lista_json`/
    `lista_tabela` → `invisible`/`neutralize`/`json_text`/`json_optional`/`list_json`/
    `list_table`.
  - `ward-nm`: `ID_EXTENSAO` → `EXTENSION_ID` (valor igual); `MAX_MENSAGEM_ENTRADA`/`SAIDA`
    → `MAX_MESSAGE_IN`/`OUT`; `OPERACOES` → `OPERATIONS`; `resposta_cabe` →
    `response_fits`; `origem_da_extensao_valida` → `valid_extension_origin`; `ErroMensagem`
    → `MessageError` (`End`, `Truncated`, `Empty`, `TooLarge`, `Io`); `ler_mensagem`/
    `escrever_mensagem` → `read_message`/`write_message`; `FalhaDaemon` → `DaemonFailure`
    (`Unavailable`, `Conversation`; `texto()` → `text()`); `Daemon::trocar` → `exchange`;
    `erro` → `error_body`; `atender`/`servir` → `handle`/`serve`; no `main.rs`,
    `servidor_esperado`/`conferir_extra`/`PRAZO_*` → `expected_server`/`verify_extra`/
    `READ_TIMEOUT`/`WRITE_TIMEOUT`.
  - Testes: nomes, ajudantes e locais em inglês (`Pasta` → `TestDir`, `Visto` → `Seen`,
    `Teste` → `TestEnv`, `servidor` → `fake_server`, `com_daemon_falso` →
    `with_fake_daemon`, `Falso` → `FakeDaemon`, `quadro` → `frame` etc.). Nenhum script
    nem alvo do Makefile roda teste destes crates pelo nome (procurado).
- **Leitores do `EXTENSION_ID`** (achados das Fases 39 e 40, mais os que achei): o regex e
  as duas mensagens de `dev/extension/build.py`; a leitura de `pub const` em
  `dev/tests/extension.py`, `dev/tests/login.py`, `dev/tests/chrome_policy.py`; o `sed` e as
  duas mensagens de `install/pack-extension.sh` (não estava no plano) com a mensagem que o
  `dev/tests/deb.sh` procura; `CLAUDE.md` e `docs/instalacao.md`.
- **Comentários** (inclusive `Cargo.toml` dos dois crates) em inglês; "Fase" → "Phase", IDs
  da missão 1 sem prefixo (DT2). Onde o comentário citava nome de fio ou subcomando pt-BR
  (`destravar`, `estado`, `interno`, `nao_autorizado`, `executar`/`saida`/`fim`, o botão
  "Conectar", a pasta `ward-teste`), a frase foi reescrita sem o nome, sem exceção nova.
- **Glossário**: parágrafo "Fase 28" depois do da Fase 13 (seção 15).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
- Rust (`--literals` antes × depois em `crates/ward` e `crates/ward-nm`): só capturas de
  `format!` com nome novo (V6.3): `{codigo}` → `{code}`, `{entrada}` → `{entry}` (em
  `"login enviado: {entry} (aba ativa do Chrome)"`, saída igual), `{MSG_RUN_CAMPO}`/
  `{MSG_RUN_GRANDE}`/`{MSG_RUN_TRAVADO}` → `{MSG_RUN_FIELD}`/`{MSG_RUN_TOO_LARGE}`/
  `{MSG_RUN_LOCKED}`, `{ruim:?}` → `{bad:?}`, `{tipo:?}` → `{kind:?}`. Nenhum texto exibido,
  subcomando, ajuda, mensagem de native messaging nem nome de fio mudou (o `HELP`, os
  `"lista"`/`"travar"`/`"destravar"`/`"ajuda"`, as mensagens pt-BR e a resposta pronta
  `{"v":1,"tipo":"erro",…}` do `ward-nm` são os mesmos bytes). Nomes de thread
  `ward-entrada`/`ward-janela` e os eventos de teste `"conectar"`/`"endurecer"`/`"ler_senha"`
  ficaram (literais).
- Scripts (nome de constante): `dev/extension/build.py` (`r'pub const EXTENSION_ID: …'`,
  `"EXTENSION_ID não achado em crates/ward-nm/src/lib.rs"`, `"… ≠ EXTENSION_ID do ward-nm"`),
  `install/pack-extension.sh` (o `sed`, `"EXTENSION_ID não achado …"`, `"… não do
  EXTENSION_ID $EXPECTED …"`), `dev/tests/deb.sh` (procura `"não do EXTENSION_ID"`),
  `dev/tests/{extension,login,chrome_policy}.py` (`"pub const EXTENSION_ID"`).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S5/D46/V4.2 (CLI): `connect_with` confere o servidor por `connect_server` antes do
    primeiro byte; `verify_extra` (só `test-mode`) exige `/proc/<pid>/exe` com `EACCES` e
    `reverify_no_debugger`; `reverify` reconfere no mesmo socket antes da senha. Ordem do
    `unlock` igual: conecta → estado → `harden` → `read_password` → `reverify` → manda.
  - S3/Fase 27a (`--token-fd`): `parse_ssh` recusa `--token`/`--token=` antes de tudo;
    `fd_number` só dígitos, 3..=`MAX_FD_NUMBER`; `token_from_channel` endurece antes de ler
    e confere `token_ok`; nada conecta antes.
  - D13/Fase 26a (ambiente do filho): `variable_sent` continua tirando `LD_*`, `=` e NUL;
    limites D52 iguais; a montagem do ambiente do filho é do daemon (não mudou aqui).
  - S1/S8/S13 (`ward-nm`): `valid_extension_origin` só aceita o `EXTENSION_ID`; `handle`
    recusa operação fora de `OPERATIONS`, confere `response_fits` e o papel da resposta de
    estado (D63); limites `MAX_MESSAGE_IN`/`OUT` e `Zeroizing` iguais; stderr só textos
    fixos. O pai Chrome e o papel pelo egid são conferidos no `ward-platform`/daemon (não
    tocados).
  - Conferência mecânica: um script (scratchpad, não versionado) troca todo identificador
    por `I`, todo literal por `S`, tira comentários e compara a sequência de tokens de cada
    arquivo antigo com o novo: `text.rs`, `lib.rs`, `main.rs` (dos dois crates), `ward-nm`
    `tests.rs` e `tests/binary.rs` iguais; em `client.rs`, `commands.rs`, `args.rs` e
    `tests.rs` só aparecem `campo: local` no lugar do atalho `campo` (campos do `ward-core`
    com o local renomeado) e vírgulas/chaves que o `rustfmt` mudou na quebra de linha.
- **Testes de ataque**: nenhum novo (só rename); os existentes (V4.2 servidor falso no
  release e no `test-mode`, socket trocado enquanto digita, resposta com credencial, token
  em argv, canal do token forjado, argumento sem eco, origem de outra extensão, operações
  do app/agente, resposta fora do papel, mensagem grande/vazia/cortada) acompanham os
  nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes de `ward-core` (protocolo: `Pedido`, `Resposta`, `ErroResposta`, `Papel`,
  `Operacao`, `TipoResposta`, `quadro`, `codificar_*`/`decodificar_*`, `LIMITE_RESPOSTA`,
  `TipoEntradaPadrao`, `Janela`, `ItemLista` e campos) usados por estes crates: ficam para
  as Fases 17–18, que ajustam os usos aqui. Por isso o `english-check` destes caminhos
  ainda tem 763 achados, **todos** identificadores do `ward-core` (conferido pela lista
  única de nomes); nenhum comentário nem identificador próprio dos dois crates.
- Subcomandos (Fase 55), protocolo v2 e `list --json` em inglês (Fase 51).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 66eb87b` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo fmt --all` | 0 |
| `cargo check -p ward -p ward-nm --all-targets --all-features --locked` (log `.dev/logs/fase28-check.log`) | 0 |
| `cargo clippy -p ward -p ward-nm --all-targets --all-features --locked -- -D warnings` (log `fase28-clippy.log`) | 0 |
| `cargo clippy -p ward -p ward-nm --all-targets --locked -- -D warnings` (sem `test-mode`; log `fase28-clippy-sem-feature.log`) | 0 |
| `make compile` (log `fase28-make-compile.log`; `npm ci`, Vite, `cargo check` do `app/src-tauri`, `syntax_check.py`: 0 erro) | 0 |
| `make english-check PATHS="crates/ward crates/ward-nm"` (log `fase28-english.log`) | 1 (763 achados, só nomes do `ward-core`; ver "Ficou de fora") |
| `make english-check PATHS="<scripts e Cargo.toml tocados>"` (log `fase28-english-scripts.log`) | 1 (0 achado; 1 "exceção sem uso" `install/* instalacao` só porque o `--paths` pegou um arquivo de `install/` sozinho) |
| `python3 -I dev/english_check.py --literals --paths crates/ward crates/ward-nm` antes × depois | só capturas de `format!` |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `66eb87b`; reset com a árvore limpa.
- `install/pack-extension.sh` também lê `pub const ID_EXTENSAO` (o plano só citava o
  `build.py` e o `extension.py`); trocado junto, com a mensagem que o `dev/tests/deb.sh`
  procura. `dev/tests/login.py` e `dev/tests/chrome_policy.py` também liam.
- `make english-check PATHS=…` com um único arquivo de `install/` acusa a exceção
  permanente `install/* instalacao` como sem uso (comportamento do `--paths`, não desta
  fase).
- **Arquivos compartilhados** com as fases paralelas: `docs/glossario.md` (parágrafo novo
  depois do da Fase 13; a Fase 14 deve pôr o dela no mesmo ponto, conflito de merge
  esperado e trivial), `CLAUDE.md` (duas células da tabela, só `ID_EXTENSAO`).
- Os arquivos renomeados aparecem no `git diff` como apagar + criar (o texto mudou muito);
  o `git mv` foi feito.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward/src/client.rs` (era `cliente.rs`) | renomeado e reescrito |
| `crates/ward/src/commands.rs` (era `comandos.rs`) | renomeado e reescrito |
| `crates/ward/src/text.rs` (era `texto.rs`) | renomeado e reescrito |
| `crates/ward/src/tests.rs` (era `testes.rs`) | renomeado e reescrito |
| `crates/ward/src/{args,lib,main}.rs`, `crates/ward/Cargo.toml` | alterado |
| `crates/ward-nm/src/tests.rs` (era `testes.rs`) | renomeado e reescrito |
| `crates/ward-nm/tests/binary.rs` (era `binario.rs`) | renomeado e reescrito |
| `crates/ward-nm/src/{lib,main}.rs`, `crates/ward-nm/Cargo.toml` | alterado |
| `dev/extension/build.py`, `dev/tests/{extension,login,chrome_policy}.py`, `dev/tests/deb.sh`, `install/pack-extension.sh` | alterado (`EXTENSION_ID`) |
| `CLAUDE.md`, `docs/instalacao.md` | alterado (`EXTENSION_ID`) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-28.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt`, `cargo clippy -D warnings` com e sem `test-mode` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou (conferido por tokens)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
