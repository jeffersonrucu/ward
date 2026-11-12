# Fase 34 — `app/core`: `unlock`, `first_use`, `generator`, `clipboard`, `level`; `app/src-tauri`: `main.rs`, `gtk_clipboard.rs`, `build.rs` (Missão 2, Milestone 2)

## Implementado
- **Base**: worktree a partir do master da época; rodou em paralelo com as Fases 17, 19 e
  33. Merges feitos antes do commit: `2e23f1d` (Fase 33) e `3356893` (Fase 17, master
  atual). Conflitos do `3356893`: `client.rs` (versão do master + os nomes desta fase) e
  `destrave.rs` modify/delete (apagado; as mudanças da Fase 17 — `ward_core::protocol::
  PedidoDeDestrave` e `p.operacao.name()` — foram levadas para o `unlock.rs`).
- **Arquivos** (`git mv`): `app/core/src/destrave.rs` → `unlock.rs`, `primeiro_uso.rs` →
  `first_use.rs`, `gerador.rs` → `generator.rs`, `area_transferencia.rs` →
  `clipboard.rs`, `nivel.rs` → `level.rs`, `app/src-tauri/src/area_gtk.rs` →
  `gtk_clipboard.rs`. `lib.rs`: `pub mod` em ordem e links da doc do crate.
- **Comentários, locais, privados e testes** dos 8 arquivos em inglês (`mod testes` →
  `tests`, `ataque_…` → `attack_…`). Comentários que citavam nome de fio ou texto da tela
  em pt-BR (comandos Tauri, avisos `pedido-de-…`, rótulos de janela, itens da bandeja,
  códigos de erro, nomes de frame) foram reescritos pelo sentido, como nas Fases 30–33.
- **Renomes públicos** (lista completa no parágrafo "Fase 34" da seção 15 do glossário),
  com os usos em `app/core` (`service.rs`, `client.rs`, `settings.rs` — inclusive os
  links de rustdoc para `crate::first_use::password_errors` e `Typed` —, `approval.rs`,
  `restore.rs`, `external_backup.rs`, `lib.rs`, `tests/configuracao_tauri.rs`) e
  `crates/ward-test-bridge` (`http.rs`, `main.rs`, `wl_clipboard.rs`,
  `tests/attacks.rs`). Principais: `SenhaMestra` → `MasterPassword`, `InfoPedido` →
  `RequestInfo`, `PedidoPendente` → `PendingRequest`, `vigiar` → `watch`, `Digitado` →
  `Typed`, `NovoCofre` → `NewVault`, `erros_da_senha` → `password_errors`, `Opcoes` →
  `Options`, `Forca` → `Strength`, `Gerada`/`Revelada`/`Gerador` → `Generated`/
  `Revealed`/`Generator`, `AreaTransferencia { escrever, ler, limpar }` → `Clipboard {
  write, read, clear }`, `Copiador` → `Copier`, `PRAZO_LIMPEZA` → `CLEAR_DELAY`,
  `NivelMudado` → `LevelChanged`, `nivel_de_nome` → `level_from_name`, `AreaGtk` →
  `GtkClipboard`.
- **`main.rs` do Tauri**: tipos e funções próprios em inglês (`Falha` → `Failure`,
  `RespostaEstado` → `StatusResponse`, `AvisoTela` → `ScreenNotice`, `Canais` →
  `Channels`, `Estado` → `AppState`, `Bandeja` → `Tray`, `JANELA_PRINCIPAL`/
  `JANELA_APROVACAO` → `MAIN_WINDOW`/`APPROVAL_WINDOW`, `navegacao_permitida` →
  `navigation_allowed`, `com_comandos` → `with_commands` etc.); locais de `main`
  `partida`/`na_aprovacao`/`na_vigia` → `app_startup`/`for_approval`/`for_watcher`.
  **Ficam** (fio, até a Fase 54): os nomes das funções `#[tauri::command]` (são os nomes
  dos comandos) e os parâmetros que vêm do JSON (`entrada`, `campo`, `senha`, `canal`,
  `tamanho`, `minusculas`, `maiusculas`, `digitos`, `simbolos`, `anterior`, `formulario`,
  `nivel`, `edicao`, `dados`, `minutos`, `ligado`, `uuid`, `id`, `decisao`, `pin`,
  `ciente`); os `State`/`WebviewWindow` não vêm do JSON e mudaram (`estado` → `state`,
  `janela` → `window`). Rótulos `principal`/`aprovacao`, avisos `pedido-de-destrave`/
  `estado-mudou`/`pedido-de-aprovacao`, nomes de thread (`ward-app-destrave`,
  `ward-app-negar`, `ward-app-bandeja`, `ward-app-travar`, `ward-limpeza`) e todos os
  textos mostrados ficam.
- **Fio congelado** (m2:DT3): todo campo `serde` renomeado leva `#[serde(rename =
  "<antigo>")]`: `RequestInfo { restante, entrada, operacao, comando }`, `NewVault {
  senha, confirmacao, confirmacao_pin }` (com `deny_unknown_fields`), `LevelChanged {
  entrada, nivel }`, e no `main.rs` `Failure { codigo, mensagem }`, `StatusResponse {
  estado, pedido }`, `ScreenNotice { aviso }`, `GeneratedResponse { tamanho, forca,
  simbolos }`, `RevealedResponse { valor }`, `UsedResponse { tamanho }`.
- **`dev/tests/webkit-real.sh`** (heredoc que tira a partida do `main.rs`): procura agora
  `let mut app_startup = startup::start()`, `app_startup.take_daemon()…`,
  `app_startup.watch(move || {` com `for_watcher` dentro e troca por
  `drop(for_watcher);`; a checagem final recusa também `app_startup` restante (o `\b`
  não pegava `startup` dentro de `app_startup`). Conferido rodando o trecho Python do
  heredoc contra o `main.rs` novo (scratchpad): as três trocas acontecem uma vez e não
  sobra `app_startup`.
- **`dev/tests/clipboard.py`**: `TEST_NAME = "gtk_clipboard::tests::real_session"`;
  `dev/tests/clipboard.sh`: comentário com o módulo novo. O protocolo de linhas do teste
  (`prazo`, `copiar`, `copiar-fora`, `limpar-fora`, `sair`, `ward39f:`) fica.
- **`app/core/tests/configuracao_tauri.rs`**: o padrão procurado no `main.rs` passou a
  `.on_navigation(navigation_allowed)`.
- **`dev/english-check/frozen.txt`**: 52 entradas da Fase 54 para o `main.rs` (34 nomes de
  comando Tauri e 18 parâmetros do JSON).
- **Testes novos** `attack_new_rust_names_are_not_wire_names` em `unlock.rs`,
  `first_use.rs` (o JSON com as chaves novas em inglês é recusado), `level.rs` e no
  `main.rs` (as seis respostas da webview serializam com as chaves antigas).

## Literais alterados
- `Debug` com o nome do tipo (o valor continua `••••`, precedente das Fases 30 e 32):
  `"SenhaMestra(••••)"` → `"MasterPassword(••••)"`, `"Digitado(••••)"` →
  `"Typed(••••)"`, `"Revelada(••••)"` → `"Revealed(••••)"` (2).
- Capturas de `format!` (mesmo texto produzido): `{texto}` → `{text}`, `{mau}`/`{ruim}`/
  `{ruim:?}` → `{bad}`/`{bad:?}`, `{proibido:?}` → `{forbidden:?}`, `{comando}` →
  `{command}`, `{falha}` → `{failure}`, `{atual} {novo}` → `{current} {new}`,
  `{minutos}` → `{minutes}`, `{corpo}` → `{body}`, `{versao:?}` → `{version:?}`,
  `{rotulo}` → `{label}`, `{motivo}` → `{reason}`.
- `dev/tests/clipboard.py` (nome do teste), `webkit-real.sh` (padrões acima) e
  `configuracao_tauri.rs` (padrão acima).
- Literais novos: só os `rename` do `serde` (nomes antigos) e os JSON/valores falsos dos
  testes novos (`acme-pg-dev`, `acme-pg-aprovacao`, `falso-valor`, `livre`, `travado`…).
- Conferência por script (scratchpad): multiconjunto de literais de cada `.rs` tocado
  contra o `3356893` só difere nos itens acima; os escapes (`\u{…}`, `\x..`, `\0`) estão
  byte a byte iguais em todos. Tokenizador sem comentários: as sequências só diferem em
  identificadores (mapa consistente), nos atributos `serde` novos, na forma curta de
  campo (`size: tamanho` → `size`), na ordem dos `use` e em vírgulas finais do rustfmt.

## Segurança
- Invariantes tocadas: S1/S11 (entrada da webview não confiável: `MasterPassword`, `Pin`,
  `Typed`, `NewVault` com `deny_unknown_fields`, `Options::validate`; capabilities e ACL
  por janela/origem no `main.rs`), S2/S12 (área de transferência no processo, limpeza em
  30 s só se o valor ainda for o copiado, comparação em tempo constante `ctutils`), S8/
  S13 (`Debug` sem valor, `Zeroizing`), S9 (PIN no `mudar_nivel`/aprovação), S10
  (janela de aprovação, negar ao fechar), D47/D57 (partida antes do GTK no `main`).
  **Destrave, primeiro uso e gerador: diff só de nomes, comentários e atributos `serde`**
  (tokenizador): nenhuma conferência, limite, ordem de passo, RNG, alfabeto, comparação
  ou mensagem de erro mudou. No `main.rs`, a ordem `startup::start()` → `take_daemon()` →
  `watch()` antes do Tauri, o `on_navigation`, o `devtools` só com `test-mode`, o
  `disable_drag_drop_handler` e a conferência de rótulo da janela de aprovação ficam
  iguais.
- Testes de ataque: os existentes ficam (renomeados `attack_…`); novos
  `attack_new_rust_names_are_not_wire_names` em `app/core/src/unlock.rs`,
  `app/core/src/first_use.rs`, `app/core/src/level.rs` e `app/src-tauri/src/main.rs`
  (nome novo não vira campo do fio; `NewVault` recusa as chaves em inglês). Rodados:
  `cargo test -p ward-app-core -p ward-test-bridge` (199 + 7 ok) e `cargo test` do
  `app/src-tauri` (22 ok, 1 ignorado: a sessão gráfica real).
- Dependências novas: nenhuma.

## Ficou de fora
- `tests/configuracao_tauri.rs` → `tests/tauri_config.rs` e o resto dele: não é arquivo
  desta fase; só o padrão do `main.rs` mudou.
- Nomes de `ward_core::protocol` usados no `unlock.rs` (`PedidoDeDestrave { restante,
  entrada, operacao, comando }`; 15 achados do `english-check`): são de outra fase.
- Nomes de comando Tauri e parâmetros do JSON: Fase 54 (entradas no `frozen.txt`).
- `make test-clipboard` e `make test-webkit-real` (precisam da sessão gráfica); o
  heredoc foi simulado e o teste `real_session` compila.
- `make check` (regra 6 do PRD: a fase termina com `make compile`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo fmt --all` e `cargo fmt` (`app/src-tauri`), depois `--check` nos dois | 0 |
| `make compile` (`<scratchpad>/compile-fase34.log`) | 0 (a 1ª rodada deu 2: dois usos de campo renomeado nos testes do `service.rs`, corrigidos) |
| `cargo clippy -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked -- -D warnings` (`clippy-fase34.log`) | 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` (`app/src-tauri`; `clippy-tauri-fase34.log`) | 0 |
| `cargo test -p ward-app-core -p ward-test-bridge --locked` (`test-core-fase34.log`) | 0 (a 1ª deu 101: padrão `navegacao_permitida` no `configuracao_tauri.rs`, corrigido) |
| `cargo test --locked` (`app/src-tauri`; `test-tauri-fase34.log`) | 0 |
| `python3 -I dev/english_check.py --paths <8 arquivos>` (`english-fase34.log`) | 1 (15 achados, todos de `ward_core::protocol`) |
| `python3 -I dev/english_check.py` (repositório inteiro; `english-full-fase34.log`) | 1 (achados de outras fases; 0 problema nas exceções) |

## Problemas descobertos
- `dev/fixtures/legacy/strings-0.1.4.txt` (linha de base antiga) ainda traz
  `SenhaMestra(••••)`, `Digitado(••••)` e `Revelada(••••)`; é fixture legada com
  `SHA256SUMS` e ficou como está (mesma decisão da Fase 32).
- O `frozen.txt` ganhou 52 entradas da Fase 54 para o `main.rs`; a Fase 54 tem de
  tirá-las ao renomear os comandos (senão o checador acusa exceção sem uso).
- Os logs ficaram no scratchpad da sessão (o worktree não tem `.dev/`), com "fase34" no
  nome.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/unlock.rs` (era `destrave.rs`) | renomeado e alterado |
| `app/core/src/first_use.rs` (era `primeiro_uso.rs`) | renomeado e alterado |
| `app/core/src/generator.rs` (era `gerador.rs`) | renomeado e alterado |
| `app/core/src/clipboard.rs` (era `area_transferencia.rs`) | renomeado e alterado |
| `app/core/src/level.rs` (era `nivel.rs`) | renomeado e alterado |
| `app/core/src/lib.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/core/src/settings.rs` | alterado |
| `app/core/src/approval.rs` | alterado |
| `app/core/src/restore.rs` | alterado |
| `app/core/src/external_backup.rs` | alterado |
| `app/core/src/client.rs` | alterado |
| `app/core/tests/configuracao_tauri.rs` | alterado |
| `app/src-tauri/src/gtk_clipboard.rs` (era `area_gtk.rs`) | renomeado e alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/build.rs` | alterado |
| `crates/ward-test-bridge/src/http.rs` | alterado |
| `crates/ward-test-bridge/src/main.rs` | alterado |
| `crates/ward-test-bridge/src/wl_clipboard.rs` | alterado |
| `crates/ward-test-bridge/tests/attacks.rs` | alterado |
| `dev/tests/webkit-real.sh` | alterado |
| `dev/tests/clipboard.py` | alterado |
| `dev/tests/clipboard.sh` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-34.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6 do PRD); `make compile` 0, clippy e testes dos
  crates tocados 0
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (fio congelado)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
