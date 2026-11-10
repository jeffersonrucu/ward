# Fase 9 — `ward-platform`: socket, papel, conectar, terminal, endurecimento, servidor, app, pasta e testes (Missão 2, Milestone 2, trilha R)

## Implementado
- **Arquivos** (`git mv`, glossário seção 4): `src/papel.rs` → `role.rs`,
  `src/conectar.rs` → `connect.rs`, `src/endurecimento.rs` → `hardening.rs`,
  `src/pasta.rs` → `dir.rs`, `src/servidor.rs` → `server.rs` (`socket.rs`,
  `terminal.rs`, `app.rs` ficam); `tests/ativacao.rs` → `activation.rs`,
  `tests/endurecimento.rs` → `hardening.rs`, `tests/identificacao.rs` →
  `identification.rs`, `tests/comum/mod.rs` → `tests/common/mod.rs` (`socket.rs`,
  `subreaper.rs` ficam). Módulos em `lib.rs`: `app`, `child`, `clock`, `connect`,
  `descendants`, `dir`, `hardening`, `launcher`, `memory`, `process`, `role`, `server`,
  `socket`, `sys`, `terminal`, `testing`.
- **Comentários** de todos os arquivos de `src/` e `tests/` do crate em inglês, inclusive
  os `// SAFETY:` e os comentários do `Cargo.toml` (a `description` fica). Locais,
  parâmetros, privados e nomes de teste em inglês (`ataque_*` → `attack_*`).
- **Renomes** (antigo → novo), com todos os usos ajustados no repositório:
  - `role.rs`: `GRUPO_APP` → `APP_GROUP`, `GRUPO_EXTENSAO` → `EXTENSION_GROUP`,
    `Instalacao` → `Install` (`from_system`, `for_test`, `app_gid`, `extension_gid`,
    `chrome`), `ErroInstalacao` → `InstallError` (`MissingGroup`, `GroupWithMembers`,
    `InvalidGid`, `GidInSubgid`, `Read`), `PoliticaKernel` → `KernelPolicy` (`read`,
    `simulated`, `verify`); privados `GID_NAO_MAPEADO` → `UNMAPPED_GID`,
    `MAX_ARQUIVO_GRUPOS` → `MAX_GROUPS_FILE`, `ler_limitado` → `read_limited`,
    `gids_proprios` → `own_gids`, `gid_do_grupo` → `group_gid`, `gid_no_subgid` →
    `gid_in_subgid`.
  - `connect.rs`: `CanalConectar` → `ConnectChannel` (`new`, `number`, `prepare`),
    `ErroCanal` → `ChannelError` (`Number`, `NotSocket`, `OtherUser`, `Empty`,
    `TooLarge`, `Deadline`, `Io`; `message()`), `ler_token_herdado` →
    `read_inherited_token`, `MAX_TOKEN`, `PRAZO_LEITURA` → `READ_DEADLINE`,
    `MAX_NUMERO_FD` → `MAX_FD_NUMBER`.
  - `socket.rs`: `NOME_DIRETORIO`(`_TESTE`) → `DIR_NAME`/`TEST_DIR_NAME`, `NOME_SOCKET` →
    `SOCKET_NAME`, `DIRETORIO_SISTEMA` → `SYSTEM_DIR`, `diretorio_runtime` →
    `runtime_dir`, `caminho_socket_sistema` → `system_socket_path`, `ErroDiretorio` →
    `DirError`, `DiretorioSocket` → `SocketDir` (`path`, `verify`, `listen`),
    `preparar_diretorio` → `prepare_dir`, `ErroEscuta` → `ListenError`, `Escuta` →
    `Listener` (`path`, `listener`, `accept`), `ErroAtivacao` → `ActivationError`,
    `escutar_de_listen_fds` → `listen_from_listen_fds`, `RecusaCaminho` → `PathRefusal`,
    `Conexao { stream, par }` → `Connection { stream, peer }`, `ErroAceitar` →
    `AcceptError`, `conferir_conexao` → `verify_connection`, `par_fechou` →
    `peer_closed`; privados (`conferir_caminho_sistema` → `verify_system_path`, …).
  - `server.rs`: `ServidorEsperado` → `ExpectedServer` (`system`, `system_at`, `test`,
    `test_at`, `path`), `RecusaServidor` → `ServerRefusal`, `conectar_servidor` →
    `connect_server`, `conferir_servidor` → `verify_server`.
  - `hardening.rs`: `ErroInicio` → `StartError`, `Memoria` → `Memory` (`Locked`,
    `NotLocked`), `endurecer_processo` → `harden_process`, `conferir_ambiente_proprio` →
    `verify_own_environment`, `iniciar_endurecido` → `start_hardened`,
    `nome_para_mensagem` → `name_for_message`.
  - `terminal.rs`: `ErroTerminal` → `TerminalError` (`NoTerminal`, `Canceled`,
    `TooLong`, `Io`), `ler_senha` → `read_password`, `ModoCru` → `RawMode`, `e_terminal`
    → `is_terminal`, `tamanho` → `size`.
  - `app.rs`: `largar_grupo` → `release_group`, `PaiDiferente` → `ParentMismatch`, `Pai`
    → `Parent` (`atual` → `current`, `continua_pai` → `still_parent`,
    `conferir_parece_o_daemon` → `verify_looks_like_daemon`), `valor_dumpable` →
    `dumpable_value`, `tem_cloexec` → `has_cloexec`,
    `NOME_DAEMON` → `DAEMON_NAME`, `reexecutar` → `reexec`, `esperar_fechamento` →
    `wait_for_close`.
  - `dir.rs`: `renomear_sem_sobrescrever` → `rename_no_replace`, `apagar_conteudo`(`_em`)
    → `delete_contents`(`_at`), `apagar_item` → `delete_item`.
  - `tests/common/mod.rs`: `DirTeste { caminho }` → `TestDir { path }` (`novo` → `new`),
    `Filho` → `TestChild` (`esperar` → `wait_for`, `encerrar` → `finish`), `CLIENTE_PY` →
    `CLIENT_PY`, `python_real` → `real_python`, `permitido` → `allowed`,
    `comando_limpo` → `clean_command`, `iniciar` → `start`, `subir_com_nova_tentativa` →
    `spawn_with_retry`, `copiar_binario` → `copy_binary`, `cliente_python` →
    `python_client`.
  - Variantes de todos os enums: parágrafo novo na seção 15 do glossário.
- **Checador também nos arquivos da Fase 7/8** (achado da Fase 7): os nomes da Fase 9
  que sobravam em `process.rs`, `launcher.rs`, `child.rs`, `testing.rs`, `lib.rs` e
  `app.rs` (`PoliticaKernel`, `nome_para_mensagem`, `crate::descendentes`,
  `por_no_fd_ativacao`, `partida`) foram trocados.
- **Usos ajustados**: `wardd` (`main`, `servidor`, `agente_ssh`, `config`, `playwright`,
  `execucao`, `externo`, `lancamento`; `tests/{externo,playwright,run,socket,ssh,trava}.rs`),
  `ward` (`args`, `cliente`, `comandos`, `testes`), `ward-nm` (`main.rs`), `app/core`
  (`partida.rs`, `daemon.rs`, `terminal.rs`, `examples/partida-conteiner.rs`) e
  `ward-platform/examples/container.rs`. Os nomes próprios desses crates (ex.:
  `ler_senha` do trait do `ward`, `PRAZO_LEITURA`/`NOME_SOCKET` do `wardd`/`ward`/
  `ward-nm`, `ErroPartida::Pai`) ficam para as fases deles.
- **Glossário**: parágrafo com as variantes fixadas na Fase 9 (seção 15).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
- `connect.rs`: `Debug` manual `"CanalConectar(fd {})"` → `"ConnectChannel(fd {})"` e
  captura `{saida}` → `{output}` (texto impresso igual).
- `terminal.rs`: `Debug` manual `"ModoCru"` → `"RawMode"`; captura de teste `{tela:?}` →
  `{screen:?}`.
- `socket.rs`: capturas `{modo:o}` → `{mode:o}` (Display de `DirError::WrongMode`, texto
  igual) e `"{DIRETORIO_SISTEMA}/{}/{NOME_SOCKET}"` → `"{SYSTEM_DIR}/{}/{SOCKET_NAME}"`
  (mesmo caminho); `dir.rs` `{rotulo}` → `{label}`; `{ruim}` → `{bad}` em vários testes.
- Testes: os nomes `filho_auxiliar` → `helper_child` (também nas strings `--exact` e no
  `sh -c` do `attack_more_than_one_fd`); as saídas esperadas do `Debug` de
  `ActivationError` em `tests/activation.rs` (`UntrustedDir`, `PathMismatch`,
  `NotActivated`, `PidMismatch`, `FdCount`); mensagem de assert `"a Escuta apaga…"` →
  `"o Listener apaga…"`; capturas `{arquivo}`, `{lidas:?}`, `{esperado}`, `{MARCA}`,
  `{modo}`, `{nome}`, `{politica:?}`, `{esperada:?}` com o nome novo.
- `dev/container-platform/inside.sh`: `servidor=PastaNaoConfiavel` →
  `servidor=UntrustedDir` e `servidor=Ausente` → `servidor=Missing` (saída por `Debug` de
  `ServerRefusal` no `examples/container.rs`).
- `crates/wardd/tests/externo.rs`: o teste que procura no fonte o texto
  `"renomear_sem_sobrescrever(destino.as_fd()"` passa a procurar
  `"rename_no_replace(destino.as_fd()"`.
- **Ficaram de propósito**: todos os `Display` (pt-BR) dos erros; as variáveis de ambiente
  dos testes (`WARD_ATIVACAO`, `WARD_CAMINHO`, `WARD_ESPERADO`,
  `WARD_PLATAFORMA_FILHO`; mudam na Fase 56); os modos (`"ativar"`, `"receber"`,
  `"endurecer"`, `"iniciar"`…) e marcas (`"RESULTADO:"`, `"ok-sem-travar:"`,
  `"conectado"`); o texto do log do `wardd` `"conexão recusada (UidRecusado)…"` e
  `"(SemIdentidade)"`, que o `dev/container-uid/inside.sh` procura (agora não é mais o
  nome da variante; ver Problemas); o `pai=SemIdentidade(...)` impresso pelo
  `examples/pai-chrome.rs` (literal, não `Debug`).
- **Conferido** por comparação de literais, números e operadores (sem comentários) entre
  cada arquivo antigo e o novo de `src/` e `tests/`: números e operadores iguais em
  todos; strings só com as trocas listadas acima.

## Segurança
- **Invariantes tocadas e como ficaram (V2.3):**
  - S5/D46 (pasta 0700, socket 0600, `LISTEN_FDS` com `LISTEN_PID`, pasta do root,
    conferência do servidor antes de qualquer byte, `SO_PEERCRED` uid 0, endereço do par):
    diff só de nomes e comentários.
  - D47/D60/D63 (papel pelo egid, `/etc/group`, `subgid`, política do kernel
    `ptrace_scope`/`suid_dumpable`): só nomes e comentários.
  - S8/D48/D62 (`PR_SET_DUMPABLE`, `RLIMIT_CORE`, lista de variáveis inseguras da glibc,
    nome cortado na mensagem, `mlockall` e `LimitMEMLOCK`): só nomes e comentários.
  - D3/Fase 27a (canal do token do "Conectar": socket herdado, mesmo uid, prazo, tamanho):
    só nomes e comentários.
  - D57 (partida do app: grupo largado, pai = daemon não-dumpable, `CLOEXEC`, reexecução):
    só nomes e comentários.
  - `make test-docker-platform`: as saídas `Debug` esperadas acompanharam as variantes
    novas; sem isso, o V3.23 viraria falso "falhou".
- **Testes de ataque**: nenhum novo (a fase só renomeia); os existentes renomeados junto
  (ex.: `attack_fake_server_of_the_same_uid_is_refused_before_any_byte`,
  `attack_v3_25_forged_name_comes_out_cut`). Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- `examples/container.rs` e `examples/pai-chrome.rs` (466 e 50 achados no checador): o
  nome e o conteúdo dos exemplos ficam para a trilha S (Fase 43, com os scripts que os
  chamam); aqui só os usos da API foram ajustados (`.verify()`, `.listen(`,
  `app_gid()`/`extension_gid()`…).
- Nomes próprios de `wardd`, `ward`, `ward-nm` e `app/core` que coincidem com os antigos
  da plataforma (`ler_senha`, `PRAZO_LEITURA`, `NOME_SOCKET`, `ErroPartida::Pai`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 7a102ff` (o worktree nasceu em `5114a25`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (até ficar limpo) | 0 |
| `cargo fmt --all` | 0 |
| `make compile` | 0 |
| `make english-check PATHS="crates/ward-platform/src crates/ward-platform/tests crates/ward-platform/Cargo.toml"` | 0 (0 achados) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `5114a25`, não em `7a102ff`; com a árvore limpa, fiz
  `git reset --hard 7a102ff`.
- **Log do `wardd` com o nome antigo da variante**: `"conexão recusada (UidRecusado)"` e
  `"(SemIdentidade)"` ficaram porque o `dev/container-uid/inside.sh` (V3.2) os procura.
  Quem traduzir o `wardd/src/servidor.rs` (ou a Fase 56) decide se troca o texto junto
  com o script; não troquei para não mexer num literal de log de outra trilha.
- **Trilha S**: esta fase mexeu em `dev/container-platform/inside.sh` (2 linhas). Se a
  trilha S renomear o arquivo, o merge precisa levar as linhas trocadas.
- `strings-0.1.4.txt` pode ter `"CanalConectar"`/`"ModoCru"` (nomes de tipo no `Debug`
  manual): o V6.3 deve aceitar a troca, como `"BufferTravado"` da Fase 8.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-platform/src/{role,connect,hardening,dir,server}.rs` (eram `papel`, `conectar`, `endurecimento`, `pasta`, `servidor`) | renomeado e alterado |
| `crates/ward-platform/src/{socket,terminal,app,lib,process,launcher,child,testing}.rs` | alterado |
| `crates/ward-platform/tests/{activation,hardening,identification}.rs`, `tests/common/mod.rs` (eram `ativacao`, `endurecimento`, `identificacao`, `comum/mod`) | renomeado e alterado |
| `crates/ward-platform/tests/{socket,subreaper}.rs` | alterado |
| `crates/ward-platform/Cargo.toml` | alterado (comentários) |
| `crates/ward-platform/examples/container.rs` | alterado (usos da API) |
| `crates/wardd/src/{agente_ssh,config,execucao,externo,lancamento,main,playwright,servidor}.rs` | alterado |
| `crates/wardd/tests/{externo,playwright,run,socket,ssh,trava}.rs` | alterado |
| `crates/ward/src/{args,cliente,comandos,testes}.rs`, `crates/ward-nm/src/main.rs` | alterado |
| `app/core/src/{daemon,partida,terminal}.rs`, `app/core/examples/partida-conteiner.rs` | alterado |
| `dev/container-platform/inside.sh` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-9.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0) e `cargo fmt`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes renomeados; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
