# Fase 30 — `app/core`: `service`, `secret`, `error`, `lib` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 88a9a48`.
  Depois do commit da fase, a pedido do orchestrator, `git merge 3775e98` (Fase 29 na
  ponte de teste), com os renomes desta fase aplicados de novo sobre a ponte nova.
- **Arquivos** (glossário seção 4): `app/core/src/servico.rs` → `service.rs`,
  `segredo.rs` → `secret.rs`, `erro.rs` → `error.rs` (`git mv`); `lib.rs`:
  `pub mod service`, `pub mod secret`, `mod error`, `pub use error::AppError`. Os outros
  `mod` (`anexos`, `cliente`…) são das Fases 31–34 e ficaram.
- **Comentários, locais, parâmetros, privados e testes** dos 4 arquivos em inglês (o
  `lib.rs` só tem comentários desta fase; os nomes de módulo citados nele são das
  Fases 31–34).
- **Renomes públicos** (antigo → novo), com todos os usos no repositório:
  - `ErroApp` → `AppError` (`codigo()` → `code()`), variantes pelos códigos da seção 13
    do glossário: `DaemonIndisponivel` → `DaemonUnavailable`, `DaemonNaoConfiavel` →
    `DaemonUntrusted`, `ConexaoPerdida` → `ConnectionLost`, `PedidoInvalido` →
    `InvalidRequest`, `Travado` → `Locked`, `AreaTransferencia` → `Clipboard`,
    `EntradaInexistente` → `EntryNotFound`, `CampoInexistente` → `FieldNotFound`,
    `NaoAutorizado` → `Unauthorized`, `Recusado` → `Refused`, `RespostaInvalida` →
    `InvalidResponse`, `AleatorioIndisponivel` → `RandomUnavailable`, `SemSenhaGerada` →
    `NoGeneratedPassword`, `PinIncorreto` → `WrongPin`, `SeletorIndisponivel` →
    `PickerUnavailable`, `TerminalIndisponivel` → `TerminalUnavailable`, `NaoEServidor` →
    `NotAServer`, `CofreJaExiste` → `VaultAlreadyExists`, `CofreNaoGravado` →
    `VaultNotWritten`, `SenhaIncorreta` → `WrongPassword`, `CofreRetrocesso` →
    `VaultRollback`, `PendrivesIndisponiveis` → `UsbDrivesUnavailable`,
    `PendriveNaoConectado` → `UsbDriveNotConnected`, `BackupInexistente` →
    `BackupNotFound`, `BackupIndisponivel` → `BackupUnavailable`, `CofreMudou` →
    `VaultChanged`.
  - `Segredo` (do `app/core`) → `Secret` (`novo` → `new`, `expor` → `expose`).
  - `Servico` → `Service` (`novo` → `new`); métodos pelos comandos da seção 13:
    `estado_do_cofre` → `vault_status`, `criar_cofre` → `create_vault`, `destravar` →
    `unlock`, `travar` → `lock`, `listar_credenciais` → `list_credentials`,
    `detalhe_credencial` → `credential_detail`, `copiar_campo` → `copy_field`,
    `gerar_senha` → `generate_password`, `copiar_senha_gerada` →
    `copy_generated_password`, `revelar_senha_gerada` → `reveal_generated_password`,
    `descartar_senha_gerada` → `discard_generated_password`, `revelar_campo` →
    `reveal_field`, `codigo_totp` → `totp_code`, `copiar_codigo_totp` → `copy_totp_code`,
    `usar_senha_gerada` → `use_generated_password`, `descartar_formulario` →
    `discard_form`, `salvar_credencial` → `save_credential`, `mudar_nivel` → `set_level`,
    `escolher_anexo` → `choose_attachment`, `remover_anexo` → `remove_attachment`,
    `carregar_chave` → `load_key`, `conectar` → `connect`, `configuracoes` → `settings`,
    `trocar_pin` → `change_pin`, `trocar_senha_mestra` → `change_master_password`,
    `definir_tempo_trava` → `set_lock_timeout`, `backup_externo` → `external_backup`,
    `listar_pendrives` → `list_usb_drives`, `definir_backup_externo` →
    `set_external_backup`, `listar_backups` → `list_backups`, `restaurar_backup` →
    `restore_backup`, `pedido_de_aprovacao` → `approval_request`, `responder_aprovacao` →
    `answer_approval`; e `pedido_de_destrave` → `unlock_request`, `info_do_pedido` →
    `request_info`, `aprovacoes` → `approvals`, `negar_aprovacoes_pendentes` →
    `deny_pending_approvals`, `com_seletor`/`com_terminal` → `with_picker`/
    `with_terminal`, `estado_da_bandeja` → `tray_state`, `na_passagem_da_bandeja` →
    `on_tray_transition`, `ver_passagem` → `check_transition`, `ao_sair` → `on_exit`.
  - `ResultadoSalvar { salvo, erros }` → `SaveResult { saved, errors }`,
    `ResultadoCriar { criado, erros }` → `CreateResult { created, errors }`,
    `CodigoTotp { codigo, restante }` (o do `service`) → `TotpCode { code, remaining }`,
    cada campo com `#[serde(rename = "<antigo>")]` (fio da webview congelado até a Fase 54).
  - Privados: `FontesDoServico` → `ServiceSources`, `CAMPOS_GERAVEIS` →
    `GENERATABLE_FIELDS`, `backup_externo_de` → `external_backup_of`, campos do `Service`
    (`copier`, `generator`, `used`, `loaded`, `attachments`, `picker`, `approvals`,
    `pending_unlock`), `*_travado(s)` → `lock_*`, `base_da_edicao` → `edit_base`.
  - Testes: `mod testes` → `tests`, `testes_destravar` → `unlock_tests`, nomes dos testes
    traduzidos (`ataque_…` → `attack_…`), auxiliares (`servico` → `service`,
    `SeletorFixo`/`TerminalFixo`/`Espiao`/`Anotador`/`Contador`/`SemArea`/`Fora` →
    `FixedPicker`/`FixedTerminal`/`Spy`/`Recorder`/`Counter`/`NoClipboard`/`Down`…).
- **Usos ajustados** (só os nomes desta fase): os outros módulos do `app/core` (src),
  `app/src-tauri/src/{main,area_gtk}.rs`, `crates/ward-test-bridge` (src e
  `tests/attacks.rs`), o heredoc de `dev/tests/webkit-real.sh` (procura
  `partida.tomar_daemon().ok_or(AppError::ConnectionLost)` no `main.rs`) e os comentários
  que citam esses nomes (`app/core/Cargo.toml`, docs dos módulos). Os renomes de método
  fora dos 4 arquivos foram feitos pelo compilador: um script (scratchpad, não versionado)
  lê os diagnósticos JSON do `cargo check` e troca só no span do erro (método de
  `Service`, `expor` do `Secret`, `codigo` do `AppError`, campos de `SaveResult`/
  `CreateResult`/`TotpCode`). Os nomes das funções `#[tauri::command]` e os parâmetros
  delas não mudaram.
- **Glossário**: parágrafo "Fase 30" na seção 15 (depois do da Fase 29).
- **Exceções novas**: `frozen.txt` (seção "Phase 30"): `livre` (48), `carregada` (54) e
  `chave` (53, nome reservado `CHAVE` = `WARD_ARQUIVO_CHAVE`) citados em comentários de
  teste do `service.rs`. `allow.txt`: `falso`, `mestra`, `anexo`, `generico` no
  `service.rs` (valores e nomes falsos do cofre de teste nos comentários).

## Literais alterados (PRD regra 5)
- `secret.rs`: `Debug` `"Segredo(••••)"` → `"Secret(••••)"` (o valor continua `••••`;
  nenhum script nem teste procura o texto antigo; está em `strings-0.1.4.txt`, diferença
  aceita pelo V6.3 como nas Fases 8–12).
- `service.rs`: novos `#[serde(rename = "salvo"|"erros"|"criado"|"codigo"|"restante")]`
  (o JSON para a webview fica igual); capturas de `format!` renomeadas com a variável
  (`{onde}` → `{place}`, `{nome}` → `{name}`, `{rotulo}`/`{miolo}` → `{label}`/`{body}`,
  `{depois:?}` → `{after:?}` etc.), inclusive na pasta temporária de teste
  `ward-servico-34-{}-{name}` (o prefixo literal ficou).
- `dev/tests/webkit-real.sh`: os dois textos do heredoc acompanham `AppError::ConnectionLost`.
- **Ficaram**: os códigos de `AppError::code()` (`"pedido-invalido"`…, fio até a Fase 54),
  todas as mensagens pt-BR do `Display`, os resultados da aprovação (`"respondido"`,
  `"expirado"`, `"pin-incorreto"`, `"bloqueado"`), `GENERATABLE_FIELDS = ["senha"]` (id de
  campo do fio) e todo literal fora desses arquivos (conferido por multiconjunto de
  literais entre linhas removidas e adicionadas do diff: zero diferença fora do acima).
- **Debug derivado**: `AppError` deriva `Debug`; ele só aparece em `assert`s de teste
  (`{e:?}` sem o PIN). Para a interface vão `code()` e `to_string()` (iguais).

## Segurança
- **Invariantes tocadas** (diff só de nomes e comentários):
  - S1/S11: nenhum método passou a devolver o `Secret`; `Secret` continua sem `Clone`,
    `Display`, `Serialize`; `expose` continua `pub(crate)`. As respostas
    (`SaveResult`, `CreateResult`, `TotpCode`) serializam o mesmo JSON (`rename`).
  - S8/S13: `Secret` zera ao sair (`Zeroizing`), `Debug` só `Secret(••••)`; o teste
    `attack_debug_does_not_show_the_value` acompanha. Mensagens de erro fixas, iguais.
  - S9/S10/S12: `answer_approval`, `set_level`, `change_pin`, `restore_backup`, `lock`,
    `on_tray_transition`, `check_transition`, `on_exit`: mesmas expressões, mesma ordem
    (conferido no diff; só renomes de locais e campos).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os módulos e tipos das Fases 31–34 citados nos 4 arquivos (`cliente`, `anexos`,
  `Copiador`, `EstadoCofre`, `SenhaMestra`, `Carregamento`…, e campos de structs deles,
  como `.nome`, `.anexos`): são os ~860 achados que sobram no `english-check` dos 4
  arquivos (todos identificadores dessas fases, de `ward_core::protocolo` — Fase 17 — ou
  links de doc para eles).
- Comandos Tauri, parâmetros, `serde` da webview e códigos de erro: Fase 54.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 88a9a48` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| scripts de rename (scratchpad) + `cargo check --workspace --all-targets --all-features --locked --message-format=json` em rodadas (logs `.dev/logs/fase30-fix*.log`, `fase30-tfix*.log`) | 0 no fim |
| `git merge --no-edit 3775e98` (pedido do orchestrator; conflitos na ponte e no glossário, resolvidos com a versão da Fase 29 + os renomes desta fase) | conflito → resolvido |
| `cargo fmt --all` (workspace e `app/src-tauri`) / `cargo fmt --all --check` | 0 |
| `cargo clippy -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked -- -D warnings` e o do `app/src-tauri` (extra; logs `fase30-clippy*.log`) | 0 |
| `make compile` (log `.dev/logs/fase30-make-compile.log`; com o `cargo check` do `app/src-tauri`) | 0 |
| `python3 -I dev/english_check.py --paths app/core/src/service.rs app/core/src/secret.rs app/core/src/error.rs app/core/src/lib.rs` (log `fase30-english.log`) | 1 (só nomes das Fases 17 e 31–34; nenhum problema nas exceções) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `88a9a48`; reset com a árvore limpa.
- **Arquivos da trilha paralela tocados**: `crates/ward-test-bridge/*` usa `Servico`,
  `ErroApp` e os métodos; sem mudar a ponte o workspace não compila. Depois do merge de
  `3775e98` os renomes foram reaplicados sobre a ponte da Fase 29 (só usos).
- `dev/tests/webkit-real.sh` (trilha S) mudou junto, só o texto do heredoc.
- Os homônimos `cliente::CodigoTotp`, `Comando::CodigoTotp` e `Resposta::CodigoTotp` não
  mudaram (outras fases); o primeiro passe de regex os pegou e foi desfeito.
- `chave` em `frozen.txt` para o `service.rs` também cala o identificador do módulo
  `chave` (Fase 33) nesse arquivo; ao renomear o módulo, a entrada continua valendo pelo
  comentário do `CHAVE`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/service.rs`, `secret.rs`, `error.rs` (eram `servico.rs`, `segredo.rs`, `erro.rs`) | renomeado e reescrito |
| `app/core/src/lib.rs` | alterado |
| `app/core/src/{anexos,aprovacao,area_transferencia,backup_externo,bandeja,chave,cliente,comandos,configuracoes,credenciais,daemon,daemon_falso,destrave,formulario,gerador,nivel,partida,primeiro_uso,restaurar,terminal}.rs`, `app/core/Cargo.toml` | alterado (usos) |
| `app/src-tauri/src/main.rs`, `area_gtk.rs` | alterado (usos) |
| `crates/ward-test-bridge/src/{http,main,simulated,wl_clipboard}.rs`, `tests/attacks.rs` | alterado (usos, sobre a Fase 29) |
| `dev/tests/webkit-real.sh` | alterado |
| `dev/english-check/frozen.txt`, `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-30.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt --check` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
