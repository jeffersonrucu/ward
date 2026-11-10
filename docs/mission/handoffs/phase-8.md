# Fase 8 — `ward-platform`: `sys`, `process`, `descendants`, `memory` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Arquivos** (`git mv`, glossário seção 4): `processo.rs` → `process.rs`,
  `descendentes.rs` → `descendants.rs`, `memoria.rs` → `memory.rs` (`sys.rs` fica). Módulos
  em `lib.rs`: `process`, `descendants`, `memory`. `mod testes` → `mod tests` nos três que
  têm testes.
- **Comentários** dos quatro arquivos em inglês, inclusive os `// SAFETY:` ("Fase N" →
  "Phase N"; IDs da missão 1 sem prefixo). Locais, parâmetros, itens privados e nomes de
  teste em inglês.
- **Renomes** (antigo → novo), com todos os usos ajustados no repositório:
  - `sys.rs` (só `pub(crate)`, todos chamados como `sys::…`): `erro_os` → `os_error`,
    `pidfd_vivo` → `pidfd_alive`, `renameat_sem_sobrescrever` → `renameat_no_replace`,
    `listar_pasta` → `list_dir`, `desligar_dumpable` → `disable_dumpable`,
    `valor_dumpable` → `dumpable_value`, `zerar_rlimit_core` → `zero_rlimit_core`,
    `travar_toda_memoria` → `lock_all_memory`, `tamanho_pagina` → `page_size`,
    `mapear_anonimo` → `map_anonymous`, `desmapear` → `unmap`, `aconselhar` → `advise`,
    `FD_ATIVACAO`(`_TOMADO`) → `ACTIVATION_FD`(`_TAKEN`), `duplicar_fd_ativacao` →
    `dup_activation_fd`, `fechar_fd_ativacao` → `close_activation_fd`,
    `por_no_fd_ativacao` → `put_on_activation_fd`, `largar_grupo_efetivo` →
    `release_effective_group`, `tem_cloexec` → `has_cloexec`, `abrir_pty` → `open_pty`,
    `definir_janela` → `set_window_size`, `ler_janela` → `read_window_size`, `modo_cru`
    → `make_raw`, `tirar_cloexec` → `clear_cloexec`, `esperar_fechamento` →
    `wait_for_close`, `fechamento` → `is_closed`, `home_do_uid` → `uid_home`,
    `conta_do_uid` → `uid_account`, `fechar_fds_no_exec` → `close_fds_on_exec`,
    `herdar_so_no_exec` → `inherit_only_on_exec`, `tomar_socket_herdado` →
    `take_inherited_socket`, `relogio_boot` → `boot_clock`, `CAPACIDADE_PIPE_SAIDA` →
    `OUTPUT_PIPE_CAPACITY` (pública, reexportada por `child`), `ampliar_pipe` →
    `widen_pipe`, `capacidade_pipe` → `pipe_capacity`, `acima_de(_2)` → `above(_2)`,
    `dev_null_acima_de_4` → `dev_null_above_4`, `PRIMEIRO_FD_LIVRE_DEPURACAO` →
    `FIRST_FREE_DEBUG_FD`, `PreparoExec { caminhos, … }` → `ExecPreparation { paths, … }`,
    `Stdio::Depuracao` → `Stdio::DebugPipe`, `PontasPai` → `ParentEnds` (`Pipes { stdin,
    stdout, stderr }`, `Pty`, `DebugPipe { commands, responses }`), `FilhoCru { pid,
    pontas }` → `RawChild { pid, ends }`, `ErroExec` → `ExecError` (`System`, `Child {
    step, errno }`), `ETAPA_*` → `STEP_PREPARATION`/`STEP_DIRECTORY`/`STEP_EXEC`,
    `filho_depois_do_fork` → `child_after_fork`, `esperar_sem_colher` →
    `wait_without_collecting`, `colher` → `collect`, `esperar_leitura` → `wait_readable`,
    `matar_grupo` → `kill_group`, `parar_grupo` → `stop_group`, `sinal_grupo` →
    `signal_group`, `virar_subreaper` → `become_subreaper`, `pid_e_sessao` →
    `pid_and_session`, `pidfd_sinal` → `pidfd_signal`, `colher_se_saiu` →
    `collect_if_exited`.
  - `process.rs`: `Recusa` → `Refusal` (variantes: `UidMismatch`, `ProcessVanished`,
    `NoAccess`, `Read`, `ExeNotAllowed`, `ExeSwapped`, `UntrustedInstall`, `Debugger`,
    `LdVariable`, `LargeEnvironment`, `UntrustedLibrary`, `ParentNotAllowed`,
    `CommonGroup`, `KernelPolicy`, `AppParentNotAllowed`); `MAX_AMBIENTE` →
    `MAX_ENVIRONMENT`, `MAX_MAPAS` → `MAX_MAPS`, `MAX_CADEIA` → `MAX_CHAIN`,
    `MAX_NOME_CADEIA` → `MAX_CHAIN_NAME`; `BinarioPermitido` → `AllowedBinary` (`new`,
    `path`); `CaminhoInvalido` (a struct deste arquivo) → `InvalidPath`;
    `CredenciaisPar` → `PeerCredentials`; `ProcessoPar` → `PeerProcess` (`from_socket`,
    `credentials`, `identify`, `parent_pid`, `verify_parent`, `classify`,
    `classify_test_app`, `classify_test_extension_host`, `reverify_no_debugger`, `chain`);
    `ClassePar` → `PeerClass` (`App`, `ExtensionHost`, `Common { reason }`);
    `ElementoCadeia { pid, nome }` → `ChainElement { pid, name }`; `cadeia_de_processos`
    → `process_chain`; `nome_ld_para_log` → `ld_name_for_log`, `ler_tracer_pid` →
    `read_tracer_pid`, `caminho_absoluto_simples` → `simple_absolute_path`,
    `conferir_arquivo_do_root` → `verify_root_file`; privados (`Alvo` → `Target`, `Vista`
    → `View`, `Mapa` → `MapLine`, `identificar_imagem` → `identify_image`…).
  - `descendants.rs`: `virar_subreaper` → `become_subreaper`, `e_subreaper` →
    `is_subreaper`, `registro` → `record`, `Relatorio { mortos, sobraram }` → `Report {
    killed, remaining }`, `matar_sessao` → `kill_session`, `matar_orfaos` →
    `kill_orphans`; privados `LIDERES` → `LEADERS`, `MAX_RODADAS` → `MAX_ROUNDS`,
    `PRAZO_MORTE` → `KILL_DEADLINE`, `Proc { inicio, zumbi }` → `{ start, zombie }`,
    `alvos` → `targets`, `sinal_conferido` → `verified_signal`, `colher_orfaos` →
    `collect_orphans`…
  - `memory.rs`: `MAX_BUFFER_TRAVADO` → `MAX_LOCKED_BUFFER`, `BufferTravado` →
    `LockedBuffer` (`new`, `zero`; campos `start`, `mapped`), `travar_regiao` →
    `lock_region`.
- **Usos ajustados** (só os usos; locais dos arquivos de outras fases mantidos, com
  `Common { reason: motivo }` na desestruturação): `ward-platform` (`child`, `launcher`,
  `lib`, `app`, `clock`, `conectar`, `endurecimento`, `papel`, `pasta`, `servidor`,
  `socket`, `terminal`, `testing`, `examples/{container,pai-chrome}.rs`,
  `tests/{comum/mod,identificacao,socket,subreaper}.rs`), `wardd` (`main`, `servidor`,
  `nucleo`, `execucao`, `lancamento`, `agente_ssh`, `tests/aprovacao.rs`), `ward`
  (`cliente.rs`), `ward-nm` (`main.rs`), `app/core` (`partida.rs`,
  `examples/partida-conteiner.rs`).
- **Scripts que leem os nomes** (mesma fase, PRD regra 4):
  - `dev/container-platform/inside.sh`: as saídas por `Debug` do exemplo do contêiner
    (`papel=agente motivo=CommonGroup`/`AppParentNotAllowed`/`ParentNotAllowed`).
  - `dev/tests/nm.py`: `pai=ParentNotAllowed` (saída por `Debug` do `pai-chrome`) e o texto
    `verify_parent`.
  - `dev/tests/no-test-mode.sh` (V3.37): as marcas procuradas no `wardd` de release
    passam a `classify_test_app classify_test_extension_host`.
  - Comentários/textos: `dev/tests/nm.sh`, `Makefile` e `CLAUDE.md` (`verify_parent`).
- **Glossário**: parágrafo novo na seção 15 com as variantes fixadas nesta fase
  (`Refusal`, `PeerClass`, `ExecError`, `ParentEnds`, `Stdio::DebugPipe`, `Report`,
  `record()`). Os tipos já estavam na tabela.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
- `process.rs`: só nome de captura em `format!`/`write!`: `"variável {nome} no ambiente"`
  → `{name}` (Display de `Refusal::LdVariable`; o texto impresso é o mesmo),
  `"/proc/{atual}/status"` → `{current}`; nos testes `"{ruim}"` → `"{bad}"` e
  `"1-2 r-xp 0 00:01 {} {caminho}\n"` → `{path}` (2×).
- `memory.rs`: `debug_struct("BufferTravado")` → `"LockedBuffer"` (nome do tipo no
  `Debug`; está em `dev/fixtures/legacy/strings-0.1.4.txt`, entra na diferença do V6.3;
  nenhum script nem log o procura).
- `tests/subreaper.rs`: mensagens de assert `"matar_orfaos: …"` → `"kill_orphans: …"`
  (nome da função).
- Scripts: as saídas esperadas do `inside.sh` e do `nm.py` e as marcas do
  `no-test-mode.sh` acima.
- Ficaram de propósito: todos os textos `Display` de `Refusal` e `InvalidPath`, as
  mensagens de `io::Error` do `sys.rs` ("PR_SET_DUMPABLE não ficou em 0" etc.), o rótulo
  `motivo=` e `pai=` dos exemplos (os scripts procuram por eles) e o
  "tamanho do buffer travado fora do limite".
- Conferido por comparação de tokens (sem comentários, identificadores comparados por
  posição): `sys.rs` 8 454 tokens e `descendants.rs` 2 389 com 0 diferenças fora de
  identificadores; `process.rs` 5 523 tokens, só as 5 capturas acima; `memory.rs` 1 075,
  só o `"LockedBuffer"`. Os mapas antigo → novo de identificador são consistentes (as
  colisões que aparecem são só de nomes do `std`, como `read`, `new`, `path`).

## Segurança
- **Invariantes tocadas e como ficaram:**
  - S5/S1 e D5/D47/D57/D63 (identificação do par: `SO_PEERCRED`, `SO_PEERPIDFD`, papel
    pelo egid, pai do app = daemon, pai do `ward-nm` = Chrome instalado, `TracerPid`,
    `LD_*`, `maps`, falha fechada em agente): diff só de nomes e comentários (V2.3,
    comparação de tokens acima).
  - S8 (dumpable, `RLIMIT_CORE`, `mlockall`, subreaper, fim de sessão com `SIGSTOP` +
    `SIGKILL` por `pidfd` conferido pelo início, órfãos sem dono; memória travada com
    `MADV_DONTDUMP`/`WIPEONFORK` e `zeroize`): só nomes e comentários (V2.6, V2.7).
  - S2/S3 (`fork_exec`: ambiente exato, sem shell, `close_range`, sessão própria,
    `PR_SET_PDEATHSIG`): só nomes e comentários.
  - V3.37 (`make test-no-test-mode`): as marcas do release acompanharam o rename; sem
    isso o teste procuraria nomes que não existem e passaria sempre.
  - `make test-docker-platform`/`make test-nm`: as saídas `Debug` esperadas acompanharam
    as variantes novas; senão o ataque viraria um falso "falhou"/"passou".
- **Testes de ataque**: nenhum novo (a fase só renomeia); os existentes renomeados junto
  (ex.: `attack_process_with_u202e_in_name_reaches_chain_whole`). Não rodaram (PRD
  regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Nomes da Fase 9 que estes arquivos usam: `papel` (`Instalacao`, `PoliticaKernel`,
  `gid_extensao`, `conferir`), por isso o checador de `process.rs` ainda acusa 10 achados
  (todos esses nomes). Somem na Fase 9.
- Locais dos arquivos de outras fases que desestruturam os tipos novos (ex.: `motivo` em
  `wardd/src/servidor.rs`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard c31e3d8` (o worktree nasceu em `5114a25`, árvore limpa) | 0 |
| `cargo fmt --all` | 0 |
| `make compile` | 0 |
| `make english-check PATHS="…/descendants.rs …/memory.rs"` | 0 (0 achados) |
| `make english-check PATHS="…/sys.rs …/process.rs …/descendants.rs …/memory.rs"` | 1 (10 achados, todos nomes da Fase 9 em `process.rs`; 0 problemas nas exceções) |
| `make english-check PATHS="…/{launcher,child,clock,testing,lib}.rs"` (os da Fase 7) | 1 (65 achados, eram 227; os que sobram são da Fase 9) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `5114a25` (0.1.4), não em `c31e3d8`; com a árvore limpa, fiz
  `git reset --hard c31e3d8`.
- **`Debug` em saída de teste**: `motivo={reason:?}` do `examples/container.rs` e
  `pai={m:?}` do `examples/pai-chrome.rs` imprimem a variante de `Refusal`; os scripts
  foram ajustados. O log do `wardd` usa o `Display` (pt-BR), que não mudou.
- **Trilha S**: esta fase mexeu em `dev/container-platform/inside.sh`, `dev/tests/nm.py`,
  `dev/tests/nm.sh`, `dev/tests/no-test-mode.sh`, uma linha do `Makefile` e do
  `CLAUDE.md`. Se a trilha S renomear esses arquivos, o merge precisa levar as linhas
  trocadas aqui.
- `strings-0.1.4.txt` tem `"BufferTravado"`: o V6.3 deve aceitar a troca (nome de tipo
  no `Debug`), como as capturas de `format!` da Fase 7.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-platform/src/process.rs` (era `processo.rs`) | renomeado e alterado |
| `crates/ward-platform/src/descendants.rs` (era `descendentes.rs`) | renomeado e alterado |
| `crates/ward-platform/src/memory.rs` (era `memoria.rs`) | renomeado e alterado |
| `crates/ward-platform/src/sys.rs` | alterado |
| `crates/ward-platform/src/{lib,child,launcher,app,clock,conectar,endurecimento,papel,pasta,servidor,socket,terminal,testing}.rs` | alterado |
| `crates/ward-platform/examples/{container,pai-chrome}.rs` | alterado |
| `crates/ward-platform/tests/{comum/mod,identificacao,socket,subreaper}.rs` | alterado |
| `crates/wardd/src/{agente_ssh,execucao,lancamento,main,nucleo,servidor}.rs`, `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/ward/src/cliente.rs`, `crates/ward-nm/src/main.rs` | alterado |
| `app/core/src/partida.rs`, `app/core/examples/partida-conteiner.rs` | alterado |
| `dev/container-platform/inside.sh`, `dev/tests/{nm.py,nm.sh,no-test-mode.sh}` | alterado |
| `Makefile`, `CLAUDE.md` | alterado (texto `verify_parent`) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-8.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0) e `cargo fmt`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes renomeados; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
