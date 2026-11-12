# Fase 19 — `wardd`: `main`, `lib`, `config`, `log`, `launch`, `files`, `usage`, `lock` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 440416c`
  (master de então). Durante a fase a Fase 17 (protocolo) e a 33 entraram no master
  (`3356893`): merge, conflitos resolvidos pegando a versão do master nos arquivos de
  outras fases e reaplicando por script só os usos dos meus nomes; nos meus arquivos, os
  nomes do protocolo da Fase 17 (`Operation`, `Role`, `Request`, `ResponseError`,
  `Decision`, `StdinKind`, `x.entry`, `x.stdin_kind`, `t.hidden()`) entraram por script.
  O commit da fase fica direto sobre `3356893`.
- **Arquivos** (glossário seção 4, `git mv`): `src/lancamento.rs` → `launch.rs`,
  `src/arquivos.rs` → `files.rs`, `src/uso.rs` → `usage.rs`, `src/trava.rs` → `lock.rs`
  (`main`, `lib`, `config`, `log` ficam). `lib.rs`: `pub mod files/launch/lock/usage`.
  Os `mod testes` → `mod tests`. Os oito arquivos traduzidos por inteiro (comentários,
  locais, parâmetros, campos, nomes de teste).
- **Renomes (antigo → novo)**: lista completa no parágrafo "Fase 19" do glossário (seção
  15, antes de "Funções e métodos…"). Os principais:
  - `log`: macros `aviso!`/`depurar!` → `warning!`/`debug!` (não `warn!`, para não ficar
    ambíguo com o atributo `#[warn]`), `configurar` → `configure`, `detalhado` →
    `detailed`.
  - `config`: `Config { caminhos, diretorio_socket_teste, app_de_teste,
    extensao_de_teste, rt }` → `{ paths, test_socket_dir, test_app, test_extension, rt }`;
    `do_ambiente`/`de` → `from_env`/`from_vars`, `caminhos_de_teste` → `test_paths`,
    `absoluto_limpo` → `clean_absolute`.
  - `launch`: `AbridorReal` → `RealOpener`, `AppLancado` → `LaunchedApp`, `AppDeTeste` →
    `TestApp`, `ExtensaoDeTeste` → `TestExtension` (`e_o_app`/`classificar`/`do_chrome`
    → `is_the_app`/`classify`/`from_chrome`), `APP_INSTALADO` → `INSTALLED_APP`,
    `descrever_saida` → `describe_exit`.
  - `files`: `Geracoes { vista, queimada }` → `Generations { seen, burned }`,
    `ArquivoEstado { geracoes, falhas_agente }` → `StateFile { generations,
    agent_failures }`, `ErroEstado { Ausente, Invalido }` → `StateError { Missing,
    Invalid }`, `PastaAberta` → `OpenDir` (`caminho`/`pasta` → `path`/`backup_dir`), o
    alias `Identidade` sai (`FileIdentity` direto), `ler_estado(_completo)`/
    `gravar_estado(_completo)` → `read_state`/`read_full_state`/`write_state`/
    `write_full_state`, `ler_cofre`/`cofre_existe`/`identidade` → `read_vault`/
    `vault_exists`/`identity`, `conferir_arquivo` → `verify_file`, `MAX_ARQUIVO_COFRE` →
    `MAX_VAULT_FILE`.
  - `usage`: `Autorizacao { SemPerguntar, Aprovado }` → `Authorization { WithoutAsking,
    Approved }`, `Daemon::autorizar_uso`/`responder_aprovacao` → `authorize_use`/
    `answer_approval`, `mais_restritivo` → `most_restrictive`, `origem_a_confirmar` →
    `origin_to_confirm`, `cadeia_para_exibir` → `display_chain`.
  - `lock`: `conta_como_uso` → `counts_as_use`, `venceu` → `expired`, `Evento` → `Event`
    (`TelaBloqueada`/`Suspensao`/`Acordou` → `ScreenLocked`/`Suspend`/`WokeUp`),
    `barramento_do_daemon` → `daemon_bus`, `caminho_do_barramento` → `bus_path`,
    `conectar` → `connect`, `vigiar_logind`/`vigiar_inatividade` → `watch_logind`/
    `watch_inactivity`, `BARRAMENTO_SISTEMA`/`INTERVALO_INATIVIDADE`/
    `ESPERA_MAXIMA_BARRAMENTO`/`PRAZO_CHAMADA` → `SYSTEM_BUS`/`INACTIVITY_INTERVAL`/
    `MAX_BUS_WAIT`/`CALL_DEADLINE`.
- **Usos ajustados** (só os nomes desta fase, por script, nos arquivos de outras fases):
  `wardd/src/{nucleo,aprovacao,server,execution,ssh,ssh_agent,bridge,login,playwright,cdp,external,udisks}.rs`
  (macros e imports `warning`/`debug`, `files::…`, `Generations`/`StateFile`/
  `FileIdentity`/`OpenDir`, `.backup_dir()`, `authorize_use`/`answer_approval`,
  `lock::expired`/`counts_as_use`/`connect`/`MAX_BUS_WAIT`, `TestApp`/`LaunchedApp`/
  `TestExtension` e os métodos no `server.rs`; dois links de doc: `crate::usage` no
  `nucleo.rs`, `crate::lock::daemon_bus` no `udisks.rs`) e os testes `approval.rs`,
  `app_value_binary.rs`, `backup.rs`, `common/mod.rs`, `core.rs`, `d62.rs`, `lock.rs`,
  `login.rs`, `socket.rs` (só usos; arquivos da Fase 26 tocados só nisso).
- **V3.15** (`tests/approval.rs`, achado das Fases 23/25): a lista dos módulos que podem
  ler o ambiente passa de `"trava.rs"` a `"lock.rs"`, a do caminho da aprovação de
  `"uso.rs"` a `"usage.rs"`, e o caso de ataque `("uso.rs", …test-mode…)` a
  `("usage.rs", …)`. Os ganchos (`com_chrome`, `com_gancho_depois_de_gravar`…) e
  `"nucleo.rs"`/`"aprovacao.rs"` são da Fase 20.
- **Exceções**: em `frozen.txt` (parágrafo "Phase 19"), fase 56: `WARD_DEV_EXTENSAO` citado
  nos comentários de `config.rs`, `launch.rs` e `main.rs`, `WARD_DEV_DBUS_SISTEMA` no
  `lock.rs` e o diretório `ward-teste` no `config.rs`. Nada em `allow.txt`: o comentário
  de `config.rs` deixou de citar `cofre.ward`/`estado` (nomes no disco que nunca mudam,
  PRD regra 11; o literal no código continua).

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo (léxico próprio, fora de comentários), comparado
com `3356893`:
- Só capturas de `format!` (valor igual): `config.rs` `{ruim}`/`{ruim:?}` → `{bad}`/
  `{bad:?}`; `files.rs` `{CABECALHO_ESTADO}` → `{STATE_HEADER}` (o texto gravado no
  estado, `ward-estado 1\nvista …\nqueimada …\nfalhas_agente …`, é o mesmo),
  `.{nome}.tmp-` → `.{name}.tmp-`, `{modo:o}`/`pasta {modo:o}` → `{mode:o}`, `{ruim:?}` →
  `{bad:?}`; `lock.rs` `{motivo}: cofre travado` → `{reason}: …`, `{ruim}` → `{bad}`.
- `tests/approval.rs`: `"trava.rs"` → `"lock.rs"`, `"uso.rs"` (×2) → `"usage.rs"` (nomes de
  arquivo do V3.15).
- **Ficaram**: todas as mensagens de log (as que os scripts procuram: `"logind: vigiando
  bloqueio de tela e suspensão"`, `"pronto"`, `"WARD_DEV_DBUS_SISTEMA recusado…"`,
  `"tela bloqueada"`…), os nomes de thread (`wardd-vigia-app`, `wardd-logind`,
  `wardd-inatividade`), os textos do estado em disco, os nomes de variável `WARD_DEV_*`,
  os caminhos (`cofre.ward`, `estado`, `test-vault.*`, `ward-teste`) e os valores dos testes.
  Nos outros arquivos tocados nenhum literal mudou.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S7/D39/D53/D62 (`files`): `open_read` com `O_NOFOLLOW|O_NONBLOCK|O_NOCTTY`,
    `verify_file`, `read_vault` com identidade do mesmo fd, estado estrito
    (`parse_state`), gravação atômica (`write_full_state`: `create_new` 0600, fsync, rename,
    fsync da pasta), `clean_state_temp_files` e `OpenDir`/`open_backup_dir` iguais.
    Formato do estado congelado (mesmo cabeçalho e chaves).
  - S1/S9/S10/D2/D19/D51/D68 (`usage`): mesma ordem (destrave, entrada bloqueada =
    inexistente, nível combinado, liberação de 1 hora, origem local, entrada padrão oculta),
    mesmas conferências antes do PIN e mesma reconferência do pendente com o estado preso.
  - S8/R23/D30 (`lock`): `counts_as_use` com os mesmos braços, `expired` com falha fechada,
    eventos do logind e inibidor soltado só depois de travar, endereço fixo no release e
    `WARD_DEV_DBUS_SISTEMA` só com `test-mode`.
  - D7/D57/D63 (`config`, `launch`): `clean_absolute`, `WARD_DEV_EXTENSAO` só `"chrome"`,
    `TestApp`/`TestExtension` sem valor possível sem `test-mode` (`Infallible`).
  - S13 (`log`): mesmas macros e mesmo formato de linha (`wardd: `, `wardd[debug]: `).
  - V3.15: a lista de arquivos acompanha os nomes novos (sem isso o teste deixaria de
    cobrir `lock.rs`/`usage.rs`).
- **Conferência**: (1) multiconjunto de literais (acima); (2) sequência de tokens dos oito
  arquivos com todo identificador trocado por `ID` e sem comentários, comparada com
  `3356893`: só diferenças de ordem dos `use`, de campos escritos por extenso ou abreviados
  (`Roles { install, app, … }`, `Description { … }`, `PedidoDeAprovacao { confirmacao:
  confirmation }`) e do alias `as Identidade` removido; (3) `cargo clippy -p wardd -D
  warnings` com todas as features e sem features (release).
- **Testes de ataque**: nenhum novo (só rename); os existentes (FIFO no lugar, known_hosts
  frouxo/link, estado estrito, caminho de barramento estranho, lista/sugerir não contam,
  V3.15) acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Identificadores de outras fases que o checador ainda acusa nos oito arquivos (173
  achados de identificador): `nucleo` (`Caminhos { cofre, estado }`, `Aberto`, `Daemon::
  novo`/`com_*`/`travar`/`travado`/`conferir_pin`/`gravar_ou_desfazer`, `RelogioReal`…),
  `aprovacao` (`Motor`, `Cliente`, `AbridorApp::abrir`, `ErroAbrirApp`, `UMA_HORA`,
  `esperar_destrave`, `pedir_aprovacao`…; Fase 20) e `protocol` (`Resposta`,
  `PedidoDeAprovacao`/`PedidoDeDestrave` e campos; Fase 18). Dois achados de comentário em
  `lib.rs` são os links de módulo [`nucleo`] e [`aprovacao`]: a Fase 20 renomeia os
  módulos e deve trocar os links junto.
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 440416c` (worktree nasceu em `b4d5e1b`) | 0 |
| `git merge 3356893` (Fases 17 e 33; conflitos resolvidos como acima) | conflitos resolvidos |
| `cargo fmt -p wardd` | 0 |
| `cargo check -p wardd --all-targets --all-features --locked` (log `.dev/logs/fase19-check-wardd.log`) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (log `fase19-clippy.log`) | 0 |
| `cargo clippy -p wardd --lib --bins --locked -- -D warnings` (release, log `fase19-clippy-release.log`) | 0 |
| `make compile` (log `.dev/logs/fase19-make-compile.log`; 0 warnings) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/src/{main,lib,config,log,launch,files,usage,lock}.rs` (log `fase19-english-check.log`) | 1 (173 identificadores de outras fases e os 2 links de módulo do `lib.rs`) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não no master; reset com a árvore limpa.
- O `git mv` virou apagar+criar no índice (os arquivos foram reescritos por inteiro), então
  o merge com a Fase 17 deu modify/delete em `uso.rs`/`trava.rs`; os nomes do protocolo
  foram reaplicados nos arquivos novos por script e conferidos pela sequência de tokens.
  Quem fizer merge de outra fase que ainda mexa em `lancamento.rs`/`arquivos.rs`/`uso.rs`/
  `trava.rs` vai ver o mesmo conflito: levar a mudança para `launch.rs`/`files.rs`/
  `usage.rs`/`lock.rs`.
- O script de usos troca `\bIdentidade\b`; em `nucleo.rs` isso pegou três comentários pt-BR
  (palavra comum, não o tipo), que voltei à mão. Fica o aviso para a Fase 20.
- Fase 20: o `lib.rs` tem os links [`nucleo`] e [`aprovacao`] na doc do crate; e o V3.15
  (`tests/approval.rs`) cita `"nucleo.rs"`/`"aprovacao.rs"` e os ganchos pelo nome.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/launch.rs` (era `lancamento.rs`) | renomeado e reescrito |
| `crates/wardd/src/files.rs` (era `arquivos.rs`) | renomeado e reescrito |
| `crates/wardd/src/usage.rs` (era `uso.rs`) | renomeado e reescrito |
| `crates/wardd/src/lock.rs` (era `trava.rs`) | renomeado e reescrito |
| `crates/wardd/src/{main,lib,config,log}.rs` | reescrito |
| `crates/wardd/src/{nucleo,aprovacao,server,execution,ssh,ssh_agent,bridge,login,playwright,cdp,external,udisks}.rs` | alterado (usos) |
| `crates/wardd/tests/{approval,app_value_binary,backup,core,d62,lock,login,socket}.rs`, `tests/common/mod.rs` | alterado (usos; V3.15) |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-19.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -p wardd -D warnings` com e sem features (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
