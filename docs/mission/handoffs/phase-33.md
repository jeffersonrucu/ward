# Fase 33 — `app/core`: `commands`, `settings`, `external_backup`, `terminal`, `key`, `startup`, `credentials` (Missão 2, Milestone 2)

## Implementado
- **Base**: worktree em `b4d5e1b`; com a árvore limpa, `git reset --hard a8262a8` (master
  atual, Fase 32 concluída). Rodou em paralelo com as Fases 17, 23 e 25.
- **Arquivos** (`git mv`): `app/core/src/comandos.rs` → `commands.rs`, `configuracoes.rs`
  → `settings.rs`, `backup_externo.rs` → `external_backup.rs`, `chave.rs` → `key.rs`,
  `partida.rs` → `startup.rs`, `credenciais.rs` → `credentials.rs` (`terminal.rs` fica).
  `lib.rs`: `pub mod` (o `#[cfg]` continua só no `fake_daemon`) e a doc do crate.
- **Comentários, locais, parâmetros, privados e testes** dos 7 arquivos em inglês
  (`mod testes` → `tests`, `pelo_servico` → `via_service`, `ataque_…` → `attack_…`; prefixos
  `v5_22_`, `v7_9_`, `d65_` do contrato ficam). Comentários que citavam nome de fio em
  pt-BR (comandos Tauri, operações e campos do protocolo, valores `carregada`, texto da
  CLI) foram reescritos pelo sentido; o valor de cada constante de comando continua
  mostrando o nome do fio.
- **Renomes públicos** (lista completa no parágrafo "Fase 33" da seção 15 do glossário),
  com os usos em `app/core` (`service.rs`, `client.rs`, `daemon.rs`, `lib.rs`,
  `tests/configuracao_tauri.rs`, `examples/startup-container.rs`), `app/src-tauri`
  (`src/main.rs`, `build.rs`) e `crates/ward-test-bridge` (`http.rs`, `simulated.rs`).
  Principais: `Comando` → `Command` (variantes e `from_name`), constantes de comando
  (`VAULT_STATUS`…, `COMMANDS`, `MAIN_COMMANDS`, `APPROVAL_COMMANDS`, `SHARED_COMMANDS`),
  `permissao` → `permission`, `Referencia` → `Reference`, `Configuracoes` → `Settings`,
  `ResultadoTroca` → `ChangeResult`, `TrocaPin`/`TrocaSenha` → `PinChange`/
  `PasswordChange`, `PendriveVisto`/`BackupExternoVisto`/`EscolhaDoPendrive` →
  `UsbDriveView`/`ExternalBackupView`/`UsbDriveChoice`, `TerminalDoSistema`/`SemTerminal`/
  `Abertura` → `SystemTerminal`/`NoTerminal`/`Opening`, `Terminal::abrir` → `open`,
  `Carregamento` → `Loading`, `ler_chave` → `read_key`, `ErroPartida`/`Partida` →
  `StartupError`/`Startup` (`iniciar`/`tomar_daemon`/`vigiar`/`reexecutar_ou_sair` →
  `start`/`take_daemon`/`watch`/`reexec_or_exit`), `ItemCredencial`/`DetalheCredencial`/
  `CampoTexto`/`CampoSecreto`/`Anexo` → `CredentialItem`/`CredentialDetail`/`TextField`/
  `SecretField`/`Attachment`.
- **Fio congelado** (m2:DT3): todo campo/variante `serde` renomeado leva
  `#[serde(rename = "<antigo>")]`; no `Loading` o `rename_all = "kebab-case"` virou
  `rename` por variante (`carregada`, `cancelado`, `aviso`; tag `resultado` fica).
  Comandos Tauri, nomes dos parâmetros (`dados`, `campo`, `minutos`, `ligado`…), valores
  das constantes de comando, ids de campo nos erros (`atual`, `novo`, `nova`,
  `confirmacao`) e todos os textos exibidos ficam.
- **`dev/tests/webkit-real.sh`** (heredoc que tira a partida do `main.rs`): procura agora
  `startup::start()`, `partida.take_daemon()`, `partida.watch(move || {` e
  `use ward_app_core::startup;`, e a checagem final recusa `partida`/`startup` restante.
  Simulado numa cópia do `main.rs` no scratchpad: as asserções passam e o resultado tem a
  partida removida. A variável local `partida` do `main.rs` do Tauri é da Fase 34.
- **Testes novos** `attack_new_rust_names_are_not_wire_names` em `settings.rs`,
  `external_backup.rs`, `key.rs` e `credentials.rs`: serializam com os nomes antigos e
  (nos `Deserialize`) recusam JSON com os nomes novos em inglês.

## Literais alterados
- Capturas de `format!` (mesmo texto produzido): `{nome}` → `{name}` (`commands.rs`;
  `"ward-chave-{}-{name}"` em `key.rs`), `{ruim:?}`/`{mau}`/`{texto}`/`{agulha}`/
  `{forjado}`/`{corpo}` → `{bad:?}`/`{bad}`/`{text}`/`{needle}`/`{forged}`/`{body}`,
  `"-----END {label}-----"` e `"{a}{label}-----\n{body}\n{b}{label}-----\n"` (`key.rs`),
  `"1 {one}"`/`"{n} {many}"` e `r#""criada_em":{created}"#` (`credentials.rs`).
- `key.rs`: `rename_all = "kebab-case"` sai; entram os `rename` por variante com os mesmos
  valores.
- `tests/configuracao_tauri.rs`: mensagem de teste `"comandos.js diverge de COMANDOS"` →
  `"commands.js diverge de COMMANDS"` (nome do arquivo JS da Fase 35 e da constante).
- `dev/tests/webkit-real.sh`: os padrões do heredoc acima e a mensagem
  `"main.rs do app mudou: vigiar"` → `"…: watch"`.
- Literais novos: só os `rename` do `serde` (nomes antigos) e os JSON dos testes novos.
- Conferência por script (tokenizador Rust no scratchpad): para cada arquivo tocado, a
  sequência de tokens sem comentários contra o master só difere em identificadores, nos
  atributos `serde` novos, em forma curta de campo (`size: size` → `size`) e na ordem dos
  `use` (rustfmt); o multiconjunto de literais só perde os listados acima. Os escapes
  (`\u{202E}`, `\u{200B}`, `\u{2066}`, `\u{FEFF}`, `\u{1b}`, `\u{e9}`, `\0` e o `\u001b`
  dentro do `r#"…"#` do `external_backup.rs`) estão byte a byte iguais; nenhum caractere
  invisível cru nos fontes (`grep -P`).

## Exceções novas
- Nenhuma em `allow.txt`/`frozen.txt`. O `english-check` dos 7 arquivos dá 190 achados,
  todos de nomes de outras fases ainda não traduzidos: `ward_core::protocolo` (`Papel`,
  `Pedido`, `Resposta`, `quadro`, `Pendrive { rotulo, montado }`, `ItemLista`,
  `DetalheEntrada`, `ResumoItem` e campos `nome`/`tipo`/`campos`/`segredos`/`anexos`/
  `datas`…, `MAX_ROTULO`, `MAX_PENDRIVES`), e do `app/core` da Fase 34 (`primeiro_uso`,
  `Digitado`, `erros_da_senha`, `pin_valido`, `PIN_INVALIDO`, `PIN_DIFERENTE`,
  `SenhaMestra`, `area_transferencia`/`AreaTransferencia`/`Copiador` e métodos `ler`/
  `escrever`/`limpar`, `nivel`, `novo`/`nova`). Os achados de comentário que sobram
  (`settings.rs` linhas 9 e 17) são o link `crate::primeiro_uso::erros_da_senha` e
  `[`Digitado`]`, nomes da Fase 34.

## Segurança
- Invariantes tocadas: S1 (entrada da webview e resposta do daemon não confiáveis:
  `Reference`, `UsbDriveChoice`, `Token::from_response`, `entry_for_argv`), S2/S11/S13
  (`key.rs`: leitura da chave, avisos fixos), S8/D47/D57/D63 (`startup.rs`: partida setgid,
  papel, reexecução), S9 (troca de PIN), S11 (lista de comandos e capabilities). Diff só
  de nomes, comentários e atributos `serde` (conferido pelo tokenizador): nenhuma
  conferência, ordem de passos da partida, flag de `open` (`O_NOFOLLOW|O_NONBLOCK|
  O_NOCTTY`), limite ou regex mudou. O token do "Conectar" continua só no canal (fd), nunca
  em argv/ambiente; `Token` e `Secret` seguem sem `Debug` do valor.
- Testes de ataque: os existentes ficam (renomeados `attack_…`); novos
  `attack_new_rust_names_are_not_wire_names` em `settings.rs`, `external_backup.rs`,
  `key.rs`, `credentials.rs` (nome novo não vira campo do fio). Escritos, não rodados
  (regra 6 do PRD); compilam (`cargo clippy --all-targets`, com e sem `test-mode`).
- Dependências novas: nenhuma.

## Ficou de fora
- `tests/configuracao_tauri.rs` → `tests/tauri_config.rs` (glossário) e o resto dele: não
  é arquivo desta fase; só os usos foram ajustados.
- `main.rs`/`build.rs` do Tauri: só os nomes vindos destes módulos e o caminho
  `ward_app_core::startup`/`commands::COMMANDS` em comentário; o resto é da Fase 34.
- Nomes de `ward-core` e dos módulos do `app/core` da Fase 34 (ficam como no master).
- `cargo test` e `make check` (regra 6 do PRD: a fase termina com `make compile`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo check -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked` | 0 |
| `cargo clippy -p ward-app-core -p ward-test-bridge --all-targets --all-features -- -D warnings` (`.dev/logs/fase33-clippy.log`) | 0 |
| `cargo clippy -p ward-app-core --all-targets -- -D warnings` (sem `test-mode`; `.dev/logs/fase33-clippy-notest.log`) | 0 |
| `cargo fmt --all --check` (raiz e `app/src-tauri`) | 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` (`app/src-tauri`; `.dev/logs/fase33-tauri-clippy.log`) | 0 |
| `make compile` (`.dev/logs/fase33-make-compile.log`) | 0 |
| `make english-check PATHS="<7 arquivos>"` (`.dev/logs/fase33-english.log`) | 2 (190 achados, todos de outras fases) |

## Problemas descobertos
- Corrida com a Fase 17 (paralela): `startup.rs`, `credentials.rs`, `external_backup.rs`
  e `client.rs` usam nomes de `ward_core::protocolo` (`Papel`, `Pedido::Estado`,
  `Resposta::Estado { travado, cofre_existe, papel }`, `quadro::ler_quadro`,
  `Pendrive { rotulo, montado }`, `ItemLista`, `DetalheEntrada`…). Quem fizer o merge da
  segunda das duas ajusta esses usos.
- Os comentários de `settings.rs` citam `crate::primeiro_uso::erros_da_senha` e
  `Digitado` (link de rustdoc): a Fase 34 precisa atualizá-los ao renomear, senão o link
  quebra só no `cargo doc`.
- O `webkit-real.sh` procura a variável local `partida` do `main.rs` (`partida.take_daemon`,
  `partida.watch`): se a Fase 34 renomear essa variável, o heredoc muda junto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/commands.rs` (era `comandos.rs`) | renomeado e alterado |
| `app/core/src/settings.rs` (era `configuracoes.rs`) | renomeado e alterado |
| `app/core/src/external_backup.rs` (era `backup_externo.rs`) | renomeado e alterado |
| `app/core/src/key.rs` (era `chave.rs`) | renomeado e alterado |
| `app/core/src/startup.rs` (era `partida.rs`) | renomeado e alterado |
| `app/core/src/credentials.rs` (era `credenciais.rs`) | renomeado e alterado |
| `app/core/src/terminal.rs` | alterado |
| `app/core/src/lib.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/core/src/client.rs` | alterado |
| `app/core/src/daemon.rs` | alterado |
| `app/core/tests/configuracao_tauri.rs` | alterado |
| `app/core/examples/startup-container.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/build.rs` | alterado |
| `crates/ward-test-bridge/src/http.rs` | alterado |
| `crates/ward-test-bridge/src/simulated.rs` | alterado |
| `dev/tests/webkit-real.sh` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-33.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6 do PRD); `make compile` 0
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (fio congelado)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
