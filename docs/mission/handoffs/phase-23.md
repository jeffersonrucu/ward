# Fase 23 — `wardd`: `server`, `external`, `udisks` e testes `common`, `units`, `memory`, `socket`, `dates` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 6b6255e`
  (master atual).
- **Arquivos** (`git mv`): `src/servidor.rs` → `src/server.rs`, `src/externo.rs` →
  `src/external.rs`, `tests/comum/mod.rs` → `tests/common/mod.rs`, `tests/memoria.rs` →
  `tests/memory.rs`, `tests/datas.rs` → `tests/dates.rs` (`udisks.rs`, `units.rs`,
  `socket.rs` ficam, glossário seção 4). Os oito arquivos traduzidos por inteiro
  (comentários, locais, parâmetros, campos, nomes de teste, `mod testes` → `mod tests`).
- **Renomes** (antigo → novo; a lista completa está no parágrafo "Fase 23" do glossário,
  seção 15):
  - `server`: `MAX_CONEXOES`/`MAX_CONEXOES_APP` → `MAX_CONNECTIONS`/`MAX_APP_CONNECTIONS`,
    `PRAZO_LEITURA`/`PRAZO_ESCRITA` → `READ_DEADLINE`/`WRITE_DEADLINE`; `Vaga`/`Vagas` →
    `Slot`/`Slots` (`reservar`/`marcar`/`soltar` → `reserve`/`mark`/`release`), `Soltar` →
    `ReleaseOnDrop`; `Papeis { instalacao, app, app_de_teste, extensao_de_teste }` →
    `Roles { install, app, test_app, test_extension }` (`papel` → `role`);
    `papel_da_classe` → `role_from_class`; `responder` → `answer`; `AssinanteSocket` →
    `SocketSubscriber`; `ClienteSocket` → `SocketClient`; `atender_conexao` →
    `serve_connection`; `servir` → `serve`.
  - `external`: `PASTA` → `BACKUP_DIR`, `PREFIXO_TEMPORARIO` → `TEMP_PREFIX`; `Visto {
    rotulo, montagem, dispositivo }` → `Seen { label, mount, device }`; `FontePendrives::
    pendrives` → `UsbDriveSource::usb_drives`; `ErroExterno` → `ExternalError` (variantes
    `NoUdisks`/`NotConnected`/`Ambiguous`/`NotMounted`/`Swapped`/`UnsafeDir`/
    `NoLocalBackup`/`Io`; `codigo` → `code`); `Copia { copiadas, apagadas }` →
    `BackupCopy { copied, deleted }`; `escolher`/`copiar`/`rodar` → `choose`/`copy`/
    `rotate` e os privados.
  - `udisks`: constantes, `Objetos` → `Objects`, `montagem_valida`/`rotulo_limpo`/
    `ler_objetos`/`registrar` → `valid_mount`/`clean_label`/`read_objects`/`record`,
    `Udisks::novo` → `new`, `vigiar_copia`/`vigiar_udisks` → `watch_copy`/`watch_udisks`,
    `ESPERA_RAJADA`/`INTERVALO_COPIA` → `BURST_WAIT`/`COPY_INTERVAL`.
  - `tests/common`: `SENHA`/`PIN_ERRADO`/`SEGREDOS` → `PASSWORD`/`WRONG_PIN`/`SECRETS`,
    `RelogioFalso` → `FakeClock`, `Dir::novo`/`caminhos` → `new`/`paths`, `daemon_em` →
    `daemon_at`, `ped`/`pedir` → `req`/`request`, `erro` → `error`, `destravar` →
    `unlock`, `servidor`/`banco` → `server`/`database`, `mudar_nivel` → `set_level`,
    `cofre_de_teste` → `test_vault`, `codificado` → `encoded`, `contem` → `contains`,
    `geracoes`/`geracao_no_disco` → `generations`/`generation_on_disk`, `com_janela` →
    `with_window` (`login`, `ok`, `b64`, `PIN`, `Dir` ficam).
  - Testes: `socket.rs` (`DirSocket` → `SocketDir`, `subir`/`conectar`/`mandar`/
    `receber`/`pedir_por`/`senha_json`/`protocolo_erro`/`outro_processo_le` →
    `start`/`connect`/`send`/`receive`/`request_on`/`password_json`/`protocol_error`/
    `other_process_reads`), `dates.rs` (`RelogioDeParede` → `WallClock`), `memory.rs`
    (`Observador` → `Observer`, `procurar`/`procurando`/`chave_de_dados` → `search`/
    `searching`/`data_key`), `units.rs` (`valores` → `values`, `PROIBIDAS` → `FORBIDDEN`).
    Nomes de teste: `d63_sem_o_grupo_o_pid_do_lancador_nao_da_papel_app` →
    `d63_without_the_group_the_launcher_pid_gives_no_app_role`,
    `d63_par_filho_sem_egid_fica_agente` → `d63_child_peer_without_egid_stays_agent`,
    `ataque_conexoes_de_agente_nao_despejam_o_app` →
    `attack_agent_connections_do_not_evict_the_app`,
    `assinante_escreve_quadro_inteiro_e_falha_fechado` →
    `subscriber_writes_a_whole_frame_and_fails_closed`,
    `ataque_montagem_estranha_conta_como_nao_montado` →
    `attack_strange_mount_counts_as_not_mounted`, `rotulo_sem_controle_e_cortado` →
    `label_without_control_and_cut`, `v3_21_socket_do_sistema` → `v3_21_system_socket`,
    `v3_21_servico_com_user_limitcore_e_sem_ld` →
    `v3_21_service_with_user_limitcore_and_no_ld`,
    `d57_ataque_nenhuma_diretiva_que_liga_no_new_privs` →
    `d57_attack_no_directive_that_turns_on_no_new_privs`,
    `v3_8_travar_zera_a_chave_e_os_segredos_antes_de_liberar` →
    `v3_8_lock_zeroes_the_key_and_the_secrets_before_freeing`,
    `v3_30_ambiente_com_tmpdir_ou_ld_recusa_subir` →
    `v3_30_env_with_tmpdir_or_ld_refuses_to_start`,
    `v3_20_ciclo_pelo_socket_sem_segredo_no_log` →
    `v3_20_cycle_through_the_socket_without_secret_in_the_log`,
    `v3_33_kill_9_entre_as_senhas_erradas` → `v3_33_kill_9_between_the_wrong_passwords`,
    `v3_6_wardd_no_ar_nega_proc_ptrace_e_core` →
    `v3_6_running_wardd_denies_proc_ptrace_and_core`, `v3_41_datas_pelo_relogio_do_daemon`
    → `v3_41_dates_from_the_daemon_clock`, `ataque_v3_41_datas_nao_vao_ao_agente` →
    `attack_v3_41_dates_do_not_go_to_the_agent`,
    `ataque_v3_41_data_vinda_do_cliente_recusada` →
    `attack_v3_41_date_from_the_client_refused`, `relogio_ilegivel_grava_sem_data_nova`
    → `unreadable_clock_writes_without_a_new_date`, `restaurar_mantem_as_datas` →
    `restore_keeps_the_dates`. Nenhum script, Makefile nem `Cargo.toml` cita esses nomes
    (procurado em `dev/`, `install/`, `Makefile`, `crates/`, `app/`).
- **Usos ajustados** (só os nomes desta fase): `src/lib.rs` (`pub mod server`/`external`,
  doc), `src/main.rs` (`server::{self, Roles}`, `Roles { install: instalacao, app,
  test_app: app_de_teste, test_extension: extensao_de_teste }`, `server::serve`,
  `Udisks::new`, `watch_copy`, `watch_udisks`), `src/nucleo.rs` (import, `UsbDriveSource`,
  `BackupCopy`, `ExternalError::NoUdisks`/`NoLocalBackup`, `external::choose`/`copy`,
  `.usb_drives()` nas duas consultas, `v.mount`/`v.label`, link de doc para
  `watch_copy`), e os 16 testes que usam o `common` (`mod common;`, `use common::…`,
  `common::X`, chamadas sem caminho onde há `use common::*`, `Dir::new`,
  `dir.paths()`). A troca nos testes foi feita por um script que só toca identificadores
  fora de string e comentário, só chamada (não campo `r.erro` do `run.rs`/`ssh.rs`, não
  o `destravar: bool` do `app_valor.rs`, não as funções locais de mesmo nome:
  `aprovacao.rs::com_janela`, `app_teste.rs::pedir`), e a forma do código foi conferida
  depois (abaixo). `tests/externo.rs`: `wardd::external::{ExternalError, Seen,
  UsbDriveSource}`, campos `label`/`mount`/`device` só nos valores `Seen` (o `Objeto` do
  udisks2 falso é da Fase 26 e ficou), variantes, `copied`/`deleted`, `udisks::…` novos.
  `tests/aprovacao.rs`: `role_from_class`. Três comentários de outros testes citavam
  `comum`/`comum::servidor`/`comum::cofre_de_teste` e acompanham.
- **Glossário**: parágrafo "Fase 23" antes do da Fase 21 (seção 15). Escolha registrada:
  `teto` → `cap` (como a tabela de termos; a Fase 15 usou `ceiling_of` no privado dela).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Conferido por multiconjunto de literais por arquivo (HEAD × worktree, script
`scratchpad/f23/shape.py`; também confirma que nenhum escape `\u{…}` virou caractere, e
não há caractere de controle cru em `crates/wardd/`):
- Capturas de `format!` (V6.3), texto igual: `{motivo}` → `{reason}` nos dois `aviso!` do
  `server` (log igual), `{PREFIXO_TEMPORARIO}` → `{TEMP_PREFIX}` (nome no disco igual,
  `.ward-tmp-…`), e nos testes `{rotulo}`/`{dados}`/`{nome}`/`{tunel}`/`{nivel}`/
  `{decisao}`/`{criar:?}`/`{ruim:?}`/`{texto}`/`{esperas:?}`/`{arquivo}`/`{saida:?}`/
  `{anterior}`/`{versao}`/`{outra:?}`/`{papel:?}`/`{forjado}` → `{label}`/`{data}`/
  `{name}`/`{tunnel}`/`{level}`/`{decision}`/`{create:?}`/`{bad:?}`/`{text}`/`{waits:?}`/
  `{file}`/`{exit_status:?}`/`{previous}`/`{version}`/`{other:?}`/`{role:?}`/`{forged}`
  (o JSON montado é o mesmo).
- Pastas temporárias de teste: `"/run/user/{}/wardd-servidor-d63-{}"` →
  `"wardd-server-d63-{}"`; rótulos de `Dir::new`/`SocketDir::new`: `"memoria"` →
  `"memory"`, `"amb"`/`"amb2"`/`"amb.log"`/`"amb2.log"`/`"ciclo"` → `"env"`/`"env2"`/
  `"env.log"`/`"env2.log"`/`"cycle"`, `"datas-v3-41"`/`"datas-agente"`/`"datas-cliente"`/
  `"datas-sem-relogio"`/`"datas-restaurar"` → `"dates-v3-41"`/`"dates-agent"`/
  `"dates-client"`/`"dates-no-clock"`/`"dates-restore"`.
- **Nomes de arquivo que os testes procuram** (mudam com o arquivo, senão a conferência
  enfraquece): `tests/aprovacao.rs` (V3.15) `"servidor.rs"` → `"server.rs"` na lista dos
  arquivos onde `test-mode` é proibido e no caso de ataque; `tests/externo.rs`
  `include_str!("../src/externo.rs")` → `"../src/external.rs"`, mensagem `"… em
  external.rs"` e o texto procurado `"rename_no_replace(destino.as_fd()"` →
  `"rename_no_replace(destination.as_fd()"` (a variável do `write_version` mudou de nome;
  a chamada é a mesma).
- **Ficaram**: todas as mensagens de `aviso!`/`depurar!` (inclusive `UidRecusado` e
  `SemIdentidade`, que o `dev/container-uid/inside.sh` procura, e os textos de
  `ExternalError::code`), os nomes das threads (`wardd-conexao`, `wardd-externo`,
  `wardd-udisks`), o nome da pasta `ward-backup` e o prefixo `.ward-tmp-`, nomes de
  interface e propriedade do D-Bus, todo JSON do protocolo (operações e campos pt-BR,
  fio congelado), os valores falsos (`falso-…`, `falso-errada-01`, `falso-errada-k9`),
  os nomes de entrada de teste (`t-aprov`, `t-memoria`, `t-datas`…), `WARD_CONTROLE` e os
  textos de `assert!`/`expect`/`panic!`.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários). Forma do código
  conferida com os identificadores trocados por um marcador e sem comentários
  (`shape.py`): nos oito arquivos da fase só as capturas e os rótulos acima e os padrões
  abreviados `PeerClass::Common { reason: motivo }` → `{ reason }`; nos outros 19 arquivos
  tocados, nenhuma diferença de forma (só o `Roles { install: instalacao, … }` do
  `main.rs`, por campo com nome novo).
  - S5 / V3.2 / D47 / D63 (`server`): `verify_connection` antes de qualquer byte, recusa de
    uid/sem identidade fechando sem escrever, `role_from_class` (único ponto: app só para o
    pid que o lançador abriu e vive), exceções de teste só com `test-mode`, reconferência
    de depurador antes de pedido de app/extensão, resposta que o papel não pode receber →
    `Interno`: mesma lógica e mesma ordem.
  - V3.4 (vagas): limites 64/8, despejo só da ociosa mais antiga do grupo do resto, app
    nunca despejado: mesmo código.
  - D17/D30/S2 (`external`): `O_NOFOLLOW | O_DIRECTORY`, `st_dev` × dispositivo do
    udisks2 e segunda consulta depois de aberto, pasta do usuário no mesmo dispositivo,
    `create_new` 0600 + `sync_all` + `renameat2(RENAME_NOREPLACE)`, só versões seladas
    pela chave do cofre aberto, fora de ordem não vai, rotação só do que confere e com
    data até agora: mesma lógica. O teste que proíbe `link(2)` no arquivo segue lendo o
    arquivo certo (`external.rs`).
  - `udisks`: as mesmas conferências de UUID, rótulo (sem controle, corte em
    `MAX_ROTULO`), montagem (absoluta, sem NUL, sem `.`/`..`, até 4096) e limite de
    objetos.
  - V3.15 (aprovação sem `test-mode`): a lista de arquivos do teste agora tem `server.rs`
    (sem isso o arquivo renomeado sairia da conferência).
- **Testes de ataque**: nenhum novo (só rename); os existentes (conexões de agente não
  despejam o app, montagem estranha, D57 sem `no_new_privs`, V3.8 memória zerada, V3.30
  `TMPDIR`/`LD_*`, V3.6 `/proc`/ptrace/core, V3.41 datas, V3.15) acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Identificadores de outras fases que aparecem nestes arquivos (checador: 465 achados de
  identificador, todos usos): `protocolo`/`Papel`/`Pedido`/`Resposta`/`ErroResposta`/
  `quadro`/`MAX_ROTULO` (Fase 17), `aprovacao::{Assinante, Cliente}` e métodos
  (`entregar`, `cadeia`, `saiu`), `nucleo::{Daemon, Caminhos, Relogio}` e métodos
  (`motor`, `assinar`, `atender_de`, `sincronizar_externo`, `cutucar_externo`, `travar`,
  `dormir`, `segundos_unix`…), `lancamento::{AppDeTeste, AppLancado, ExtensaoDeTeste}`
  (`e_o_app`, `classificar`, `do_chrome`), `arquivos::*`, `trava::{conectar,
  ESPERA_MAXIMA_BARRAMENTO}`, `config::caminhos_de_teste`, macros `aviso!`/`depurar!`
  (Fases 19–20). Os locais do `main.rs`/`nucleo.rs` (`papeis`, `instalacao`, `fonte`,
  `vistos`…) são das Fases 19–20.
- 9 achados de comentário ficam de propósito: `UidRecusado` (texto de log que o script
  procura), `WARD_DEV_EXTENSAO`/`WARD_DEV_DBUS_SISTEMA` (variáveis, Fase 56), links para
  `MAX_ROTULO`, `Daemon::sincronizar_externo`, `crate::trava::barramento_do_daemon` (as
  fases desses nomes acertam o link) e os nomes de entrada de teste `t-aprov`/`t-livre`/
  `t-bloq` (dados).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo fmt -p wardd`, `cargo fmt --all --check` | 0 |
| `cargo check -p wardd --all-targets --all-features --locked` (log `check-fase23.log`) | 0 |
| `make compile` (log `scratchpad/f23/compile-fase23.log`; 0 warning) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (extra, log `clippy-fase23.log`) | 0 |
| `make english-check PATHS="crates/wardd/src/server.rs crates/wardd/src/external.rs crates/wardd/src/udisks.rs crates/wardd/tests/common crates/wardd/tests/units.rs crates/wardd/tests/memory.rs crates/wardd/tests/socket.rs crates/wardd/tests/dates.rs"` (log `english-check-fase23.log`) | 2 (465 identificadores de outras fases, 9 comentários de propósito; 0 arquivo, 0 problema nas exceções) |

`make check` não rodou: pelo plano (PRD regra 6), as fases dos Milestones 1–5 terminam
com `make compile` e o checador; a bateria fica para o Milestone 6.

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `6b6255e`; reset com a árvore limpa.
- **Conflito com a Fase 24 (paralela)**: `tests/ssh.rs` e `tests/run.rs` usam o `common`
  (≈ 300 linhas com `pedir`/`erro`/`cofre_de_teste`/`SENHA`… trocadas aqui). Se a Fase 24
  reescrever esses arquivos, o merge precisa reaplicar os nomes do `common` (lista acima
  e no glossário); o `erro` de campo (`r.erro`) e o `let erro` local do `ssh.rs` não são
  do `common`. Também tocados (só usos), das Fases 25–27 ainda não feitas: `aprovacao.rs`,
  `trava.rs`, `nucleo.rs`, `externo.rs`, `backup.rs`, `configuracoes.rs`, `d62.rs`,
  `extensao.rs`, `login.rs`, `playwright.rs`, `app_teste.rs`, `app_valor.rs`,
  `valor_app.rs`, `anexos_edicao.rs`; e `src/nucleo.rs`/`main.rs`/`lib.rs` (Fases 19–20).
- O teste `externo_nao_usa_link` (`tests/externo.rs`) procura o texto
  `rename_no_replace(destination.as_fd()` no código: quem renomear essa variável no
  `external.rs` precisa mudar o teste junto (é a conferência de "sem `link(2)`").
- O teste V3.15 do `tests/aprovacao.rs` lista arquivos de `src/` por nome
  (`aprovacao.rs`, `uso.rs`, `server.rs`, `nucleo.rs`): as Fases 19/20 que renomearem
  `aprovacao.rs`/`uso.rs`/`nucleo.rs` precisam atualizar a lista, senão a conferência
  deixa de cobrir o arquivo.
- `docs/mission/plan.md` ainda cita `servidor.rs`, `externo.rs`, `comum/`, `memoria.rs`,
  `datas.rs` na linha da Fase 23 e `externo.rs` etc. nas Fases 25–27 (o plano é do
  orchestrator).
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo antes do da Fase 21;
  conflito de merge trivial com as Fases 16/24/32).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/servidor.rs` → `crates/wardd/src/server.rs` | renomeado e alterado |
| `crates/wardd/src/externo.rs` → `crates/wardd/src/external.rs` | renomeado e alterado |
| `crates/wardd/src/udisks.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/comum/mod.rs` → `crates/wardd/tests/common/mod.rs` | renomeado e alterado |
| `crates/wardd/tests/memoria.rs` → `crates/wardd/tests/memory.rs` | renomeado e alterado |
| `crates/wardd/tests/datas.rs` → `crates/wardd/tests/dates.rs` | renomeado e alterado |
| `crates/wardd/tests/units.rs` | alterado |
| `crates/wardd/tests/socket.rs` | alterado |
| `crates/wardd/tests/{anexos_edicao,app_teste,app_valor,aprovacao,backup,configuracoes,d62,extensao,externo,login,nucleo,playwright,run,ssh,trava,valor_app}.rs` | alterados (só usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-23.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não, pela regra 6
  do PRD; rodaram `make compile`, `cargo clippy -p wardd -D warnings` e `fmt --check`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes, renomeados; não rodaram
  (regra 6)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase

## Merge com o master `a8262a8` (Fases 16, 24, 27 e 32)
- Conflitos em `tests/{app_value_binary,app_value_core,extension,login,playwright,run,ssh,test_app}.rs`
  resolvidos com a versão do master (Fases 24/27) e os nomes do `common` reaplicados pelo
  mesmo script (só identificadores fora de string e comentário; a função local
  `test_app.rs::request` ficou de fora). `anexos_edicao.rs` (apagado no master) removido; o
  `attachment_edit.rs` novo recebeu os nomes do `common`. Em `aprovacao.rs`, `trava.rs` e
  `nucleo.rs` (Fase 25, em paralelo) só usos do `common` e o literal `"server.rs"` do V3.15.
- Literais conferidos por script contra `a8262a8`: só mudam `"server.rs"` (V3.15) e os três
  de `tests/externo.rs` (`../src/external.rs`, mensagem, `destination.as_fd()`); nenhum
  `\u{…}` virou caractere, nenhum caractere de controle cru em `crates/wardd/`.
- Glossário com as partes das Fases 16, 23, 24, 27 e 32, sem marcador de conflito.
- `make compile` depois do merge: 0 (log `scratchpad/f23/compile-merge-fase23.log`).
