# Fase 7 — `ward-platform`: `launcher`, `child`, `clock`, `testing`, `lib.rs` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Arquivos** (`git mv`, glossário seção 4): `lancador.rs` → `launcher.rs`, `filho.rs` →
  `child.rs`, `relogio.rs` → `clock.rs`, `teste.rs` → `testing.rs`. Módulos em `lib.rs`:
  `launcher`, `child`, `clock`, `testing` (este só com `test-mode`). `mod testes` →
  `mod tests` nos quatro.
- **Comentários** dos cinco arquivos em inglês ("Fase N" → "Phase N"; IDs da missão 1 sem
  prefixo). Os textos pt-BR que os comentários citavam ("aprovação indisponível",
  "encerrado") foram descritos em inglês; o nome de operação `abrir_app` saiu dos
  comentários (fica "the open-app operation"), para não precisar de `frozen.txt`.
- **Renomes** (antigo → novo), todos com os usos ajustados no repositório inteiro:
  - `launcher.rs`
    - constantes: `PATH_DO_APP` → `APP_PATH`, `PLUGINS_GST_DO_SISTEMA` → `SYSTEM_GST_PLUGINS`,
      `PLUGINS_GST_PERMITIDOS` → `ALLOWED_GST_PLUGINS`, `VARIAVEIS_FIXAS` → `FIXED_VARIABLES`,
      `PASTA_MPS_INEXISTENTE` → `MISSING_MPS_DIR`, `VARIAVEIS_DE_LOCALE` → `LOCALE_VARIABLES`,
      `MAX_DIGITOS_WAYLAND` → `MAX_WAYLAND_DIGITS`, `NOME_APP_DE_TESTE` → `TEST_APP_NAME`,
      `PATH_DE_TESTE` → `TEST_PATH`;
    - `AmbienteApp` → `AppEnvironment` (`variaveis()` → `variables()`, `valor()` → `value()`);
    - `ErroSessao` → `SessionError`: `RuntimeNaoConfiavel` → `UntrustedRuntime`,
      `SemWayland` → `NoWayland`, `VariosWayland` → `SeveralWayland`,
      `WaylandNaoConfiavel` → `UntrustedWayland`, `BarramentoNaoConfiavel` → `UntrustedBus`,
      `HomeIndisponivel` → `HomeUnavailable`, `Faltando` → `Missing`, `AMais` → `Extra`,
      `Diferente` → `Different`, `Repetida` → `Repeated`; `sessao_indisponivel()` →
      `session_unavailable()`;
    - `ErroLancador` → `LauncherError`: `JaAberto` → `AlreadyOpen`, `Sessao` → `Session`,
      `BinarioNaoConfiavel` → `UntrustedBinary`, `Io`;
    - `Lancador` → `Launcher`: `novo` → `new`, `de_teste` → `for_test`, `de` →
      `from_origin`, `aberto` → `is_open`, `e_o_app_lancado` → `is_launched_app`,
      `tomar_saida` → `take_exit_status`, `abrir` → `open`, `pid`; campos `origem` →
      `origin`, `filho` → `child`, `saida` → `exit_status`; enum privado `Origem` →
      `Origin` (`Instalado { binario, gid_app }` → `Installed { binary, app_gid }`, `Teste`
      → `Test`);
    - `ErroBinarioDeTeste` → `TestBinaryError` (`Caminho` → `Path`, `ForaDoRepositorio` →
      `OutsideRepo`, `Link`, `NaoArquivo` → `NotFile`, `OutroDono` → `OtherOwner`,
      `Gravavel` → `Writable`; `mensagem()` → `message()`);
    - `BinarioDeTeste` → `TestBinary` (`novo` → `new`, `caminho` → `path`, `nome` → `name`,
      `conferir` → `verify`, `ambiente` → `environment`, `abrir_log` → `open_log`, `lancar`
      → `launch`; campos `caminho`/`raiz` → `path`/`root`);
    - funções: `ambiente_do_app` → `app_environment`, `conferir_ambiente_do_app` →
      `verify_app_environment`, `locale_valido` → `valid_locale`, `lancar_para_teste` →
      `launch_for_test`, e as privadas `pasta_da_sessao` → `session_dir`, `montar` →
      `build`, `conferir_pasta_da_sessao` → `verify_session_dir`, `socket_do_uid` →
      `uid_socket`, `nome_wayland` → `wayland_name`, `socket_wayland_unico` →
      `single_wayland_socket`, `nome_conhecido` → `known_name`, `conferir_em` →
      `verify_in`, `conferir_binario_do_app` → `verify_app_binary`, `lancar` → `launch`.
  - `child.rs`
    - `ErroComando` → `CommandError` (`Vazio` → `Empty`, `ByteNulo` → `NulByte`,
      `NomeInvalido` → `InvalidName`);
    - `ErroLancar` → `LaunchError` (`Sistema` → `System`, `Preparo` → `Preparation`,
      `Diretorio` → `Directory`, `Programa` → `Program`; `codigo_de_saida` → `exit_code`,
      `mensagem` → `message`);
    - `Comando` → `Command` (`novo` → `new`, `argumento` → `argument`, `variavel` →
      `variable`, `caminhos` → `paths`, `lancar` → `launch`, `lancar_com_depuracao` →
      `launch_with_debugging`, `lancar_em_pty` → `launch_in_pty`, `lancar_com` →
      `launch_with`; campo `ambiente` → `env`); `c_de` → `c_bytes`; `PATH_PADRAO` →
      `DEFAULT_PATH`;
    - `Pipes { entrada, saida, erro }` → `Pipes { stdin, stdout, stderr }`;
      `PipeDepuracao { comandos, respostas }` → `DebugPipe { commands, responses }`;
      `Pty { mestre }` → `Pty { master }` (`definir_janela` → `set_window_size`);
    - `Fim { Codigo, Sinal }` → `End { Code, Signal }` (`codigo_de_saida` → `exit_code`);
    - `Grupo` → `Group` (`esperar_fim` → `wait_for_end`, `matar` → `kill`,
      `matar_com_sessoes_filhas` → `kill_with_child_sessions`, `sobraram` → `remaining`,
      `colher` → `collect`; campos `colhido` → `collected`, `sobras` → `leftovers`);
    - `pronto_para_ler`/`pronto_para_ler_fd` → `ready_to_read`/`ready_to_read_fd`.
  - `clock.rs`: `tempo_desde_boot` → `time_since_boot`.
  - `testing.rs`: `executar_com_socket` → `run_with_socket`.
  - `lib.rs`: `uid_efetivo` → `effective_uid`, `pasta_pessoal` → `home_dir`,
    `conta_do_usuario` → `user_account`.
  - Testes de unidade dos quatro arquivos: nomes em inglês (ex.:
    `ataque_so_o_pid_do_lancador_e_o_app` → `attack_only_the_launcher_pid_is_the_app`,
    `ataque_filho_com_sessao_propria_morre_com_matar_com_sessoes_filhas` →
    `attack_child_with_own_session_dies_with_kill_with_child_sessions`). Nenhum script
    chama teste por nome.
- **Usos ajustados** (só os usos): `wardd` (`execucao.rs`, `ponte.rs`, `playwright.rs`,
  `nucleo.rs`, `ssh.rs`, `cdp.rs`, `agente_ssh.rs`, `lancamento.rs`, `config.rs`,
  `arquivos.rs`, `externo.rs`, `servidor.rs`, `app-teste/`, `aprovador-teste/`, testes),
  `app/core` (`partida.rs`, `bandeja.rs`), `ward-platform` (`app.rs`, comentário de
  `descendentes.rs`, `examples/container.rs`, `tests/{ativacao,identificacao,socket,subreaper}.rs`).
  Desestruturação de `Pipes` com os nomes locais mantidos (`stdin: entrada, …`): os
  locais são dos arquivos do `wardd`, das fases deles. Em `tests/subreaper.rs` o
  `std::process::Command` passou a ser escrito por extenso (o `use` dele colidia com
  `child::Command`).
- **Scripts e Makefile que liam os nomes** (mesma fase, PRD regra 3/4):
  - `dev/tests/gst-channels.sh` e `dev/tests/webkit-real.sh` leem as constantes do
    arquivo-fonte: caminho `launcher.rs` e o bloco `pub const FIXED_VARIABLES`.
  - `dev/tests/no-test-mode.sh` (V3.37): as marcas procuradas no `wardd` de release
    trocaram para `TestBinary launch_for_test`. Sem isso, o teste procuraria nomes que
    não existem mais e passaria sempre (ver Segurança).
  - `dev/container-platform/inside.sh`: as saídas por `Debug` do exemplo do contêiner
    (`lancador: segundo AlreadyOpen`, `Session(SeveralWayland) indisponivel=true`,
    `Session(UntrustedWayland) indisponivel=true`, `UntrustedBinary`).
  - Comentários: `Makefile` (`launcher.rs`), `dev/tests/attacks-foundation.sh`
    (`TestBinary`).
- **Glossário**: linha "saída(s)" com `exit_status` (como o processo saiu) e as pontas
  `stdin`/`stdout`/`stderr`; linha nova "pronto (para ler)" → `ready_to_read`.
- **Exceções**: `allow.txt` ganhou `crates/ward-platform/src/launcher.rs frei` (o
  `frei0r` é nome de plugin do GStreamer). `frozen.txt`: nada.
- **Literais alterados** (PRD regra 5; nenhum texto exibido ao usuário mudou):
  - formato com captura de variável renomeada (o texto impresso é o mesmo):
    `"unix:path={rt_texto}/bus"` → `{rt_text}` (este está em
    `dev/fixtures/legacy/strings-0.1.4.txt`, então entra na diferença do V6.3);
    nos testes: `"{nome}.log"`, `"/run/user/{uid}/ward-lancador-teste-{}-{nome}"`,
    `"/run/user/{}/ward-lancador-repo-{}-{nome}"` (`{name}`), `"{proibida}"`/`"{proibido} …"`
    (`{forbidden}`), `"{nome}={valor}"`, `"{ruim:?}"`, `"#!/bin/sh\n{corpo}\n"`,
    `"{texto}"`, `"/proc/{neto}"`/`"/proc/{neto}/stat"` (`{grandchild}`),
    `"boottime {c} × uptime {seg}"` (`{secs}`);
  - `testing.rs`: `"executar_com_socket: {erro}"` → `"run_with_socket: {error}"` (é o
    nome da função; só `test-mode`, nenhum script lê).
  - Nos scripts: `"pub const FIXED_VARIABLES"`, o caminho `launcher.rs`, as marcas do
    `no-test-mode.sh` e as quatro saídas esperadas do `inside.sh`.
  - Ficaram de propósito: os prefixos de pasta de teste `ward-lancador-teste`/`-repo`, o
    `Debug` de `Command` (`"Comando({} argumentos, {} variáveis, ••••)"`) e o prefixo
    `lancador:` que o exemplo do contêiner imprime (o `inside.sh` procura por ele).

## Segurança
- **Invariantes tocadas e como ficaram:**
  - S1/S8 e D57/D63 (lançador do app, pidfd, ambiente montado do zero), S2/S3/S8 e D60
    (filho do `ward run`: ambiente exato em `Zeroizing`, sem shell, sessão e grupo
    próprios, matar a sessão inteira), V2.3 (papel app só para o pid lançado e vivo):
    diff só de nomes e comentários. Conferido por um comparador de tokens (sem
    comentários, identificadores trocados por posição, vírgula final e chaves de braço do
    `rustfmt` normalizadas): `launcher.rs` 10 200 tokens e `child.rs` 7 582 tokens com
    0 diferenças fora de identificadores; `clock.rs` 0; `testing.rs` só o literal acima;
    os 30 arquivos de usos com 0 diferenças, salvo `execucao.rs`/`ponte.rs` (os campos
    `stdin:`/`stdout:`/`stderr:` na desestruturação) e `subreaper.rs` (ordem dos `use` e
    `std::process::Command` por extenso).
  - V3.37 (`make test-no-test-mode`): as marcas procuradas acompanharam o rename. Se
    ficassem `BinarioDeTeste lancar_para_teste`, o teste nunca acharia nada e um release
    com `test-mode` passaria.
  - V5.31/D57 (`make test-docker-platform`): as saídas esperadas do contêiner
    acompanharam o `Debug` novo; senão o ataque viraria falso "falhou".
  - `make test-webkit-real`/canais da 39c: os scripts continuam lendo os valores fixos de
    D57/D64 da mesma fonte (`FIXED_VARIABLES`); se o nome não acompanhasse, o script sairia
    com erro (falha fechada), não com valor errado.
- **Testes de ataque**: nenhum novo (a fase só renomeia); os existentes foram renomeados
  junto (`attack_*` em `launcher.rs` e `child.rs`). Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Nomes de `sys`, `processo`, `descendentes`, `papel`, `endurecimento`, `memoria`,
  `servidor`, `socket`, `terminal`, `conectar` que estes arquivos usam ou reexportam
  (`CAPACIDADE_PIPE_SAIDA`, `PontasPai`, `PreparoExec`, `ErroExec`, `ETAPA_*`,
  `descendentes::Relatorio`, `BinarioPermitido`, `Instalacao`, os `pub use` do `lib.rs`…):
  são das Fases 8 e 9. Por isso o `english-check` destes cinco arquivos ainda não está
  limpo (ver Problemas).
- Os locais e nomes próprios dos arquivos do `wardd` e do `app/core` que usam estes tipos
  (ex.: variáveis `grupo`, `comando`, `lancador`, `AppLancado`): fases deles.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard b1c96ff` (o worktree nasceu em `5114a25`, árvore limpa) | 0 |
| `cargo fmt --all` (e `--check` no `app/src-tauri`) | 0 |
| `make compile` (2×; a segunda depois dos ajustes de comentário) | 0 |
| `make english-check PATHS="crates/ward-platform/src/{launcher,child,clock,testing,lib}.rs"` | 1 (227 achados, todos nomes das Fases 8–9; 0 problemas nas exceções) |
| `make check` | não rodado (PRD regra 6: as fases dos Milestones 1–5 não rodam suíte) |

## Problemas descobertos
- O worktree desta fase foi criado em `5114a25` (0.1.4), não em `b1c96ff` como o pedido
  dizia; com a árvore limpa, fiz `git reset --hard b1c96ff` antes de começar.
- **Checador não limpo nos arquivos da fase**: a ordem do plano põe `launcher`/`child`
  antes de `sys`/`processo`/`descendentes` (Fase 8) e de `papel`/`endurecimento`/
  `servidor`/`socket`/`terminal`/`conectar`/`memoria` (Fase 9). Os 227 achados que sobram
  em `launcher.rs`, `child.rs`, `clock.rs`, `testing.rs` e `lib.rs` são todos usos ou
  reexportações desses nomes (conferido um a um pela lista); somem com as Fases 8 e 9. A
  Fase 9 deve rodar o checador nestes cinco arquivos também.
- **Literal em `strings-0.1.4.txt`**: renomear uma variável capturada em `format!` muda o
  literal (`"unix:path={rt_texto}/bus"` → `{rt_text}`). Vai acontecer em todas as fases
  da trilha R; o V6.3 deve aceitar a diferença quando só o nome da captura muda.
- **`Debug` em log**: o `wardd` registra `executar: o filho não foi criado ({e:?})`; o
  log passa a mostrar `Program(2)`/`Directory(…)` em vez de `Programa(2)`/`Diretorio(…)`.
  Nenhum script nem teste procura esses nomes (grep em `dev/`, `crates/`, `app/`).
- **Trilhas paralelas**: esta fase mexeu em scripts de `dev/` (trilha S: `inside.sh`,
  `gst-channels.sh`, `webkit-real.sh`, `no-test-mode.sh`, `attacks-foundation.sh`) e numa
  linha de comentário do `Makefile`, porque leem nomes do Rust. Se a trilha S renomear
  esses arquivos, o merge precisa levar as linhas trocadas aqui.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-platform/src/launcher.rs` (era `lancador.rs`) | renomeado e alterado |
| `crates/ward-platform/src/child.rs` (era `filho.rs`) | renomeado e alterado |
| `crates/ward-platform/src/clock.rs` (era `relogio.rs`) | renomeado e alterado |
| `crates/ward-platform/src/testing.rs` (era `teste.rs`) | renomeado e alterado |
| `crates/ward-platform/src/lib.rs` | alterado |
| `crates/ward-platform/src/app.rs`, `src/descendentes.rs` | alterado |
| `crates/ward-platform/examples/container.rs` | alterado |
| `crates/ward-platform/tests/{ativacao,identificacao,socket,subreaper}.rs` | alterado |
| `crates/wardd/src/{agente_ssh,arquivos,cdp,config,execucao,externo,lancamento,nucleo,playwright,ponte,servidor,ssh}.rs` | alterado |
| `crates/wardd/app-teste/main.rs`, `crates/wardd/aprovador-teste/main.rs` | alterado |
| `crates/wardd/tests/{app_teste,app_valor,externo,nucleo,run,socket,trava}.rs` | alterado |
| `app/core/src/{partida,bandeja}.rs` | alterado |
| `dev/container-platform/inside.sh` | alterado |
| `dev/tests/{gst-channels,webkit-real,no-test-mode,attacks-foundation}.sh` | alterado |
| `Makefile` | alterado (comentário) |
| `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-7.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não, por PRD
  regra 6; rodou `make compile` (0) e `cargo fmt`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes renomeados; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
