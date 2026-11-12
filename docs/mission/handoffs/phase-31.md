# Fase 31 — `app/core`: `fake_daemon`, `daemon`, `client` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 3956557`
  (master atual). Rodou em paralelo com as Fases 15, 21 e 28.
- **Arquivos** (glossário seção 4, `git mv`): `app/core/src/cliente.rs` → `client.rs`,
  `daemon_falso.rs` → `fake_daemon.rs` (`daemon.rs` fica). `lib.rs`: `pub mod client`,
  `pub mod fake_daemon` e as duas linhas da doc do crate.
- **Comentários, locais, parâmetros, privados e testes** dos 3 arquivos em inglês. Os
  comentários que citavam nomes de outras fases (funções do protocolo, `MAX_ROTULO`,
  `crate::partida`, `nucleo.rs`/`uso.rs` do `wardd`, nomes de frame do design como
  `Aprovação / prod`, o caminho `ward-teste`) foram reescritos pelo sentido, sem o nome.
- **Renomes públicos** (antigo → novo), com todos os usos em `app/core`, `app/src-tauri/src`
  e `crates/ward-test-bridge` (src e `tests/attacks.rs`):
  - `daemon`: `Daemon::trocar` → `Daemon::exchange`; `EstadoCofre` → `VaultStatus`
    (`SemCofre`/`Travado`/`Destravado` → `NoVault`/`Locked`/`Unlocked`, `codigo()` →
    `code()`); `Aviso` → `Notice` (`Aprovacao`/`Destrave` → `Approval`/`Unlock`,
    `destrave_de_teste`/`destrave_hostil_de_teste` → `test_unlock`/`hostile_test_unlock`);
    `FilaAvisos` → `NoticeQueue` (`tomar`/`esperar`/`esperar_destrave`/`esperar_aprovacao`/
    `simular`/`tamanho`/`descartados` → `take`/`wait`/`wait_unlock`/`wait_approval`/
    `simulate`/`size`/`dropped`); `PRAZO_RESPOSTA` → `RESPONSE_TIMEOUT`, `MAX_AVISOS` →
    `MAX_NOTICES`; `DaemonSocket` (fica) com `padrao` → `standard`, `conectar_agora` →
    `connect_now`, `vigiar_fechamento` → `watch_close`, `avisos` → `notices`, `sistema_em`/
    `teste_em`/`de_par` → `system_at`/`test_at`/`from_pair`, `caminho` → `path`. Privados:
    `Fila` → `Queue`, `Pedinte` → `Requester`, `Transporte` → `Transport` (`novo`/`aberta`/
    `fechar`/`trocar` → `new`/`is_open`/`close`/`exchange`), `ler` → `read_loop`,
    `PRAZO_ESCRITA` → `WRITE_TIMEOUT`, `sem_conexao` → `unconnected`, `conectar` →
    `connect`; campos em inglês.
  - `client`: `estado` → `status`, `criar_cofre` → `create_vault`, `papel` → `role`,
    `tem_papel_app` → `has_app_role`, `abrir_app`/`abrir_app_como_agente` →
    `open_app`/`open_app_as_agent`, `lista` → `list`, `detalhe` → `detail`, `copiar` →
    `copy`, `revelar` → `reveal`, `emitir_token` → `issue_token`, `salvar` → `save`,
    `mudar_nivel` → `set_level`, `responder_aprovacao` → `answer_approval`, `destravar` →
    `unlock`, `configuracao` → `settings`, `trocar_pin`/`trocar_senha` →
    `change_pin`/`change_password`, `tempo_trava` → `set_lock_timeout`, `travar` → `lock`,
    `backup_externo` → `external_backup`, `pendrives` → `usb_drives`,
    `definir_backup_externo` → `set_external_backup`, `restaurar_backup` →
    `restore_backup` (`totp`, `backups` ficam); `CodigoTotp { codigo, restante }` →
    `TotpCode { code, remaining }` (não tem `serde`), `Salvamento { Salvo, Problemas }` →
    `Save { Saved, Problems }` (nome do glossário seção 15), `Respondida { Ok,
    PinIncorreto, Expirado }` → `Answered { Ok, WrongPin, Expired }`. Privados: `pedir` →
    `ask`, `erro_do_daemon` → `daemon_error`, `SegredoCore` → `CoreSecret`.
  - `fake_daemon`: `DaemonFalso` → `FakeDaemon` (`cheio`/`vazio`/`travado`/`sem_cofre`/
    `backup_plantado`/`backup_local_inseguro`/`backup_externo_ligado` → `full`/`empty`/
    `locked`/`no_vault`/`planted_backup`/`insecure_local_backup`/`external_backup_enabled`;
    `com_pendrives`/`com_backups`/`com_aprovacao`/`sem_espera` → `with_usb_drives`/
    `with_backups`/`with_approval`/`without_wait`; `nomes_dos_backups`/`quantas_entradas`/
    `erros_de_pin`/`aprovacoes_pendentes`/`nivel_de`/`pin_confere` → `backup_names`/
    `entry_count`/`pin_errors`/`pending_approvals`/`level_of`/`pin_matches`; privados
    `atender`/`destravar`/`senha_confere`/`conferir_pin`/`trocar_pin`/`restaurar`/
    `trocar_senha`/`responder_aprovacao`/`mudar_nivel`/`salvar`/`entrada` → `handle`/
    `unlock`/`password_matches`/`check_pin`/`change_pin`/`restore`/`change_password`/
    `answer_approval`/`set_level`/`save`/`entry`); `BackupFalso` → `FakeBackup`,
    `EntradasDoBackup { Teste { menos }, Guardadas }` → `BackupEntries { Test { minus },
    Stored }`, `AprovacaoSimulada` → `SimulatedApproval`, `aprovacoes_simuladas` →
    `simulated_approvals`, `backups_de_teste`/`pendrives_de_teste` →
    `test_backups`/`test_usb_drives`, `MAX_ERROS_PIN` → `MAX_PIN_ERRORS`, `VERSOES_FALSAS`
    → `FAKE_VERSIONS`, `cofre_de_teste_menos` → `test_vault_minus`, `agora`/`alterado` →
    `now_secs`/`modified`.
  - **Re-export da Fase 14**: `pub use test_vault::SECRETS as SEGREDOS;` virou
    `pub use test_vault::SECRETS;`; os usos (`aprovacao.rs`, `bandeja.rs`,
    `configuracoes.rs`, `credenciais.rs`, `restaurar.rs`, `service.rs`, `client.rs`, a ponte
    `tests/attacks.rs`) usam `SECRETS`.
  - Testes: `mod testes` → `tests`; nomes em inglês (`ataque_…` → `attack_…`, `v5_33_…`
    mantêm o prefixo do contrato); auxiliares `Resposta1` → `FixedReply`, `Base` →
    `TempBase`, `servidor_falso` → `fake_server`, `nada_recebido` → `nothing_received`,
    `mandar`/`pedido_lido`/`estado_app`/`destrave`/`aprovacao`/`fechou` →
    `send`/`read_request`/`app_status`/`unlock_notice`/`approval`/`closed`.
- **Como os usos foram trocados**: um script (scratchpad, não versionado) trocou só tokens
  de código (fora de comentários e literais) nos arquivos de `app/core/src`,
  `app/src-tauri/src` e da ponte: nomes únicos inteiros, `cliente::<fn>` qualificado,
  `FakeDaemon::<fn>`/`DaemonSocket::<fn>` qualificados, chamadas `.metodo(` dos métodos
  desta fase e `fn trocar(&mut self` das implementações de `Daemon` nos testes de outros
  módulos. `Aviso` só onde não é variante de outra enum (`Escolha::Aviso`,
  `Carregamento::Aviso` ficaram). O resto pelo `cargo check` (imports `use
  crate::daemon::Aviso`, `fila.tamanho()`, `c.codigo`/`c.restante` no `service.rs`,
  `e.codigo()` no `main.rs` do Tauri e no `http.rs` da ponte, `DaemonSocket::avisos`).
  O helper de teste `fn trocar(d: &mut FakeDaemon, …)` do `credenciais.rs` (Fase 32) ficou.
- **Conferência de lógica**: os 3 arquivos comparados token a token com o master, sem
  comentários (script `tokcmp.py` no scratchpad): só identificadores mudaram (com mapa
  consistente; os "múltiplos" são campos do fio que ficam ao lado de locais renomeados),
  mais campos abreviados que viraram `campo: local` e um par de chaves de closure que o
  rustfmt tirou. Nenhum literal mudou além das capturas de `format!` listadas abaixo.
- **Glossário**: parágrafo "Fase 31" na seção 15 (depois do da Fase 30), com a nota de que
  `trocar` do `Daemon` é "exchange" (troca de pedido e resposta), não "change".

## Literais alterados (PRD regra 5)
- Capturas de `format!`/`assert!` (V6.3, só o nome): `{nome}` → `{name}` no caminho
  temporário `"/run/user/{}/ward-app-core-{}-{name}"` (o texto fica igual);
  `{verdadeiro}` → `{real}`, `{troca}` → `{swap}`, `{modo}` → `{mode}` nos testes.
- **Ficaram**: os códigos `"sem-cofre"`/`"travado"`/`"destravado"` de
  `VaultStatus::code()` (fio da webview, Fase 54), a mensagem de log
  `ward-app: aviso do daemon descartado (fila cheia)`, os nomes de thread
  `ward-app-leitora`/`ward-app-vigia`, o `expect("transporte")`, os modos da ponte
  (`aprovacao`, `aprovacao-prod`, `aprovacao-origem*`…), `"totp_semente"`, as mensagens dos
  `assert!`, os valores falsos de D15 e todos os JSON de teste.

## Exceções novas
- `allow.txt`: `app/core/src/fake_daemon.rs aprovacao` (nome da entrada
  `acme-pg-aprovacao` do cofre de teste, citado na doc dos modos de aprovação; dado de
  usuário).
- `frozen.txt`: nenhuma.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S5/D46 (servidor conferido antes de qualquer byte): `DaemonSocket::connect` segue
    chamando `connect_server(&self.expected)` antes de criar o `Transport`, com o mesmo
    mapa `Missing|Io` → `DaemonUnavailable`, resto → `DaemonUntrusted`; nada escrito antes.
    `standard()` com os mesmos `cfg` (`ExpectedServer::system()` sem `test-mode`; o
    `WARD_DEV_SOCKET_DIR` só com a feature).
  - D47 (conexão única): `connect_now` põe `can_connect = false` antes de conectar, e
    `transport()` responde `ConnectionLost` sem reconectar (mesma ordem).
  - D65 a: a thread leitora (`read_loop`) é a única a ler; avisos vão à fila limitada
    (`MAX_NOTICES` 16, descarte do novo com log sem conteúdo); resposta sem pedido, quadro
    acima do limite ou que não decodifica no papel app fecham a conexão; prazo da resposta
    130 s e a conexão fecha sem resposta.
  - S1/S11 (`client`): `copy`/`reveal` seguem devolvendo só o `Secret` do app (zera ao
    sair); a leitura estrita (`decodificar_resposta(Papel::App, …)`) e o detalhe só da
    entrada pedida não mudaram. Quadros com PIN/senha seguem largados (`drop`) logo depois
    da troca.
  - S9/S10/D20 no daemon falso: PIN no hash Argon2id da biblioteca, 5º erro trava, senha
    mestra em tempo constante (`ctutils::CtEq`) com a espera de D20 antes de olhar, mesma
    ordem do restaurar. Diff só de nomes.
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes
  (`attack_fake_server_of_the_same_uid_is_refused_without_receiving_anything`,
  `attack_without_the_startup_connection_does_not_reconnect`, `attack_directory_*`,
  `attack_socket_open_to_others_is_refused`, `attack_regular_file_in_place_of_the_socket_is_refused`,
  `v5_33_*`, `attack_response_of_another_type_or_forged_is_refused`,
  `attack_d65_list_without_summary_or_with_secret_in_summary_is_refused`). Não rodaram
  (PRD regra 6); compilam (`cargo check`/`clippy --all-targets`, com e sem `test-mode`).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes de outras fases citados nos 3 arquivos: tipos e funções do protocolo
  (`Pedido`, `Resposta`, `ErroResposta`, `Papel`, `codificar_pedido`, campos do fio —
  Fase 17), `wc::backup::Versao`/`do_instante`/`nome_valido`/`fora_de_ordem` (Fase 15),
  `Referencia`, `SenhaMestra`, `Pin::para_o_protocolo`, `configuracoes::Configuracoes`
  (Fases 32–34). São os 919 achados que sobram no `english-check` dos 3 arquivos, todos
  identificadores (nenhum comentário, nenhum nome de arquivo).
- Comentários pt-BR de outros arquivos que citam "cliente"/"daemon falso" em prosa
  (`partida.rs`, `restaurar.rs`, `aprovacao.rs`): são das Fases 32–34.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 3956557` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| script de rename (scratchpad) + `cargo check -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked` em rodadas (logs `.dev/logs/fase31-check{1,2,3}.log`) | 0 no fim |
| `cargo check -p ward-app-core --all-targets --locked` (sem `test-mode`; `fase31-check-nofeat.log`) | 0 |
| `cargo fmt --all` / `--check` (workspace e `app/src-tauri`) | 0 |
| `cargo clippy -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked -- -D warnings` (`fase31-clippy.log`) | 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` no `app/src-tauri` (`fase31-clippy-tauri.log`) | 0 |
| `make compile` (`.dev/logs/fase31-make-compile.log`; com o `cargo check` do `app/src-tauri`) | 0 |
| `python3 -I dev/english_check.py --paths app/core/src/client.rs app/core/src/daemon.rs app/core/src/fake_daemon.rs` (`fase31-english.log`) | 1 (só identificadores de outras fases; exceção nova usada) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não no master; reset com a árvore limpa.
- **Arquivos de outras trilhas tocados** (só usos): `crates/ward-test-bridge` (Fase 29,
  concluída) e `app/src-tauri/src/main.rs` (Fase 34). Os módulos de `app/core` das Fases
  32–34 também (só usos e o `SECRETS`).
- Merge do master `efa6865` (Fases 15, 21, 28): `daemon_falso.rs` apagado e os renomes da
  Fase 15 aplicados no `fake_daemon.rs` (`Version::from_instant`/`from_name`/`name`,
  `out_of_order`, `valid_name`); glossário com os parágrafos das Fases 31, 28, 21 e 15;
  `make compile` 0 depois do merge (`.dev/logs/fase31-merge-make-compile.log`).
- `Save` (nome do glossário para `Salvamento`) é curto e genérico; ficou como o glossário
  manda, sempre qualificado como `client::Save` fora do módulo.
- Um doc comment de `client.rs` (sobre o resultado do salvar) estava solto em cima de
  `emitir_token`; foi para cima de `Save`, onde valia (só comentário).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/client.rs` (era `cliente.rs`) | renomeado e reescrito |
| `app/core/src/fake_daemon.rs` (era `daemon_falso.rs`) | renomeado e reescrito |
| `app/core/src/daemon.rs` | reescrito |
| `app/core/src/lib.rs` | alterado |
| `app/core/src/{aprovacao,backup_externo,bandeja,configuracoes,credenciais,destrave,partida,primeiro_uso,restaurar,service}.rs` | alterado (usos) |
| `app/src-tauri/src/main.rs` | alterado (usos) |
| `crates/ward-test-bridge/src/{http,main,startup}.rs`, `tests/attacks.rs` | alterado (usos) |
| `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-31.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt --check` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
