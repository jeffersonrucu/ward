# Fase 32 — `app/core`: `form`, `approval`, `attachments`, `restore`, `tray` (Missão 2, Milestone 2)

## Implementado
- **Base**: worktree em `b4d5e1b`; com a árvore limpa, `git reset --hard 6b6255e` (master
  atual, fases 22 e 31 concluídas). Rodou em paralelo com as Fases 16, 23 e 24.
- **Arquivos** (`git mv`): `app/core/src/formulario.rs` → `form.rs`, `aprovacao.rs` →
  `approval.rs`, `anexos.rs` → `attachments.rs`, `restaurar.rs` → `restore.rs`,
  `bandeja.rs` → `tray.rs`. `lib.rs`: `pub mod` e linhas da doc do crate.
- **Comentários, locais, parâmetros, privados e testes** dos 5 arquivos em inglês
  (`mod testes` → `tests`, `ataque_…` → `attack_…`, prefixos `v5_…` do contrato ficam).
  Comentários que citavam texto da tela em pt-BR (botões "Permitir"/"Negar", frames
  `Aprovação / Expirando`, `Bandeja / Destravado`…) foram reescritos pelo sentido.
- **Renomes públicos** (lista completa no parágrafo "Fase 32" da seção 15 do glossário),
  com os usos em `app/core` (`service.rs`, `configuracoes.rs`, `primeiro_uso.rs`,
  `chave.rs`, `tests/configuracao_tauri.rs`), `app/src-tauri/src/main.rs` e
  `crates/ward-test-bridge` (`main.rs`, `http.rs`, `simulated.rs`). Principais:
  `Formulario` → `Form`, `ErroCampo` → `FieldError`, `ValorSecreto` → `SecretValue`,
  `TextoSecreto` → `SecretText`, `PedidoExibido` → `DisplayedRequest`,
  `ResultadoAprovacao` → `ApprovalResult`, `Aprovacoes` → `Approvals`, `vigiar` → `watch`,
  `SEGUNDOS_URGENTE` → `URGENT_SECONDS`, `Seletor` → `Picker` (`choose`/`choose_key`),
  `Escolha` → `Choice`, `EdicaoDosAnexos` → `AttachmentEdit`, `Anexos` → `Attachments`,
  `VersaoVista` → `VersionView`, `Restauracao` → `Restore`, `Restaurado` → `Restored`,
  `EstadoBandeja` → `TrayState`, `Aparencia` → `Appearance`, `Acao` → `Action`, `Icone` →
  `Icon`, constantes `ID_*`/`TEXTO_*` da bandeja.
- **Fio congelado** (m2:DT3): todo campo/variante `serde` renomeado leva
  `#[serde(rename = "<antigo>")]` (o `rename_all = "kebab-case"` de `ValorSecreto`/
  `ValorExtra`/`Escolha` virou `rename` por variante, com os mesmos valores); a tag
  `resultado` do `Choice` fica. Comandos Tauri e nomes dos parâmetros (`formulario`,
  `edicao`, `dados`), ids do menu (`bandeja-*`), pasta `ward-bandeja`, nome da thread
  `ward-app-aprovacao` e todos os textos exibidos ficam.
- **Teste novo** em `form.rs`, `approval.rs`, `attachments.rs` e `restore.rs`:
  `attack_new_rust_names_are_not_wire_names` — serializa/desserializa com os nomes antigos
  e recusa JSON com os nomes novos em inglês (a webview não ganha nome novo antes da Fase 54).
- **Fase 31 pendente**: helper de teste `fn trocar` do `credenciais.rs` → `exchange`.
- `src-tauri/main.rs`: só nomes vindos do `app/core` (tipos, campos `title`/`subtitle`/
  `action`/`action_enabled`/`icon`, `has_other`, `impl Picker for SeletorGtk`) e um
  comentário que citava `bandeja::acao` (agora `tray::action`). O resto do arquivo é da
  fase própria do Tauri.

## Literais alterados
- `Debug` de `SecretText`: `"TextoSecreto(••••)"` → `"SecretText(••••)"` (o valor continua
  escondido; só o nome do tipo).
- Capturas de `format!` (sem efeito na saída): `{texto}` → `{text}`, `{ruim}` → `{bad}`,
  `{chave}`/`{falta}`/`{agulha}` → `{key}`/`{missing}`/`{needle}`, `{tempo}`/`{nome}` →
  `{time}`/`{name}` em `"Trava sozinho após {time} sem uso"`, `"bandeja-{name}-{}"` e
  `"ward-anexos-{}-{name}"` (mesmo texto produzido), `{antes}`/`{depois}`/`{estado}` →
  `{before}`/`{after}`/`{state}` em mensagens de teste.
- `tests/configuracao_tauri.rs`: captura `{SEGUNDOS_URGENTE}` → `{URGENT_SECONDS}` e a
  mensagem de teste `"SEGUNDOS_URGENTE diverge do Rust"` → `"URGENT_SECONDS diverge do Rust"`.
- Literais novos: só os `rename` do `serde` (nomes antigos) e os JSON dos testes novos.
- Conferência pedida pelo orchestrator (achado da Fase 16): multiconjunto de literais de
  cada arquivo tocado contra o master por script; os `\u{…}` dos 5 arquivos (ex.: `\u{202E}`,
  `\u{1b}`, `\u{E0041}`, `\u{2192}`) estão byte a byte iguais, nenhum virou caractere.
  `service.rs`, `main.rs` do Tauri, `http.rs`/`main.rs` da ponte e `credenciais.rs`: nenhum
  literal mudou.

## Exceções novas
- Nenhuma em `allow.txt`/`frozen.txt`. Os 227 achados restantes do `english-check` nos 5
  arquivos são todos nomes de outras fases ainda não traduzidos (`ward_core::protocolo`,
  `PedidoDeAprovacao` e campos `entradas`/`ambiente`/`operacao`/`comando`/`diretorio`/
  `cadeia`/`origem`/`confirmacao`/`entrada_oculta`, `Problema { campo, posicao, motivo }`,
  `VersaoBackup { nome, conferida, fora_de_ordem }`, `Decisao`, `ErroResposta`,
  `SenhaMestra`, `destrave`, `Copiador`/`AreaTransferencia`, `Papel`, `nivel`, `novo`…).

## Segurança
- Invariantes tocadas: S1 (entrada da webview e do daemon não confiável), S10/S13 (menu da
  bandeja), S11 (segredo do formulário só no Rust), S12/D4 (aprovação e PIN). Diff só de
  nomes e comentários: sem mudança de lógica em aprovação, PIN, anexos e formulário
  (conferido no diff e pelos testes de ataque existentes, que continuam no lugar).
- `SecretText` segue sem `Debug` do valor e com `Zeroizing`; nenhum segredo em literal novo
  (o `falso-mestra-ward-01` do teste novo do `restore.rs` é a senha falsa de D15).
- Testes de ataque: `attack_new_rust_names_are_not_wire_names` em `form.rs`, `approval.rs`,
  `attachments.rs`, `restore.rs` (nome novo não vira campo aceito no fio). Escritos, não
  rodados (regra 6 do PRD); compilam (`cargo check`/`clippy --all-targets`).
- Dependências novas: nenhuma.

## Ficou de fora
- Nomes de `ward-core` e dos outros módulos do `app/core`/Tauri (fases próprias).
- `cargo test` e `make check` (regra 6 do PRD: a fase termina com `make compile`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo check -p ward-app-core -p ward-test-bridge --all-targets --all-features --locked` | 0 |
| `cargo check --all-targets --all-features --locked` (`app/src-tauri`) | 0 |
| `cargo clippy -p ward-app-core -p ward-test-bridge --all-targets --all-features -- -D warnings` | 0 |
| `cargo clippy -p ward-app-core --all-targets -- -D warnings` (sem `test-mode`) | 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` (`app/src-tauri`) | 0 |
| `cargo fmt --all --check` (raiz e `app/src-tauri`) | 0 |
| `make compile` (`.dev/logs/fase32-make-compile.log`) | 0 |
| `make english-check PATHS="<5 arquivos>"` (`.dev/logs/fase32-english-make.log`) | 2 (227 achados, todos de outras fases) |

## Problemas descobertos
- O glossário trazia `Montado` → `Mounted`; no `form.rs` o tipo é o resultado de `build`
  (partes montadas do rascunho), então ficou `Built` e a linha da tabela foi ajustada.
- O checador conta como achado o nome de campo de `ward-core` lido no `app/core`
  (`p.campo`, `r.entrada_oculta`); somem quando as fases de `ward-core` traduzirem.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/form.rs` (era `formulario.rs`) | renomeado e alterado |
| `app/core/src/approval.rs` (era `aprovacao.rs`) | renomeado e alterado |
| `app/core/src/attachments.rs` (era `anexos.rs`) | renomeado e alterado |
| `app/core/src/restore.rs` (era `restaurar.rs`) | renomeado e alterado |
| `app/core/src/tray.rs` (era `bandeja.rs`) | renomeado e alterado |
| `app/core/src/lib.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/core/src/configuracoes.rs` | alterado |
| `app/core/src/primeiro_uso.rs` | alterado |
| `app/core/src/credenciais.rs` | alterado |
| `app/core/src/chave.rs` | alterado |
| `app/core/tests/configuracao_tauri.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `crates/ward-test-bridge/src/main.rs` | alterado |
| `crates/ward-test-bridge/src/http.rs` | alterado |
| `crates/ward-test-bridge/src/simulated.rs` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-32.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6 do PRD); `make compile` 0
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (fio congelado)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
