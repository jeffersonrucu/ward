# Fase 54 — (UI) Comandos e eventos do Tauri e `serde` da webview (missão 2)

## Implementado
- **Comandos do Tauri** (`app/core/src/commands.rs`, `app/src-tauri/src/main.rs`): os 34
  comandos trocados 1:1 pelo glossário §13 (`estado_do_cofre` → `vault_status`,
  `copiar_campo` → `copy_field`, `listar_credenciais` → `list_credentials`,
  `detalhe_credencial` → `credential_detail`, `gerar_senha` → `generate_password`,
  `revelar_campo` → `reveal_field`, `salvar_credencial` → `save_credential`,
  `mudar_nivel` → `set_level`, `ouvir_avisos` → `listen_notices`,
  `pedido_de_aprovacao` → `approval_request`, `responder_aprovacao` → `answer_approval`,
  `destravar` → `unlock`, `travar` → `lock`…). Funções `#[tauri::command]`, parâmetros
  (`entry`, `field`, `password`, `channel`, `size`, `lowercase`, `uppercase`, `digits`,
  `symbols`, `previous`, `form`, `level`, `edit`, `data`, `minutes`, `enabled`,
  `decision`, `acknowledged`) e a lista do `generate_handler!`.
- **Janelas, eventos e threads**: rótulos `principal` → `main`, `aprovacao` → `approval`;
  eventos `estado-mudou` → `status-changed`, `pedido-de-aprovacao` → `approval-request`,
  `pedido-de-destrave` → `unlock-request`; threads `ward-app-*` em inglês; pasta dos
  ícones da bandeja `ward-tray`.
- **Capabilities**: `capabilities/principal.json` → `main.json` (identifier/janela
  `main`, 32 permissões `allow-<comando>` novas) e `aprovacao.json` → `approval.json`
  (só `allow-listen-notices`, `allow-approval-request`, `allow-answer-approval`);
  `tauri.conf.json` com `["main", "approval"]`. As permissões continuam geradas pelo
  `build.rs` a partir de `COMMANDS` (não há pasta `permissions/` escrita à mão).
- **`serde` da webview** (app/core): tirados os `#[serde(rename = "<antigo>")]` em que o
  campo Rust já é o nome de fio; ficou só `rename = "type"` dos campos `kind`. Valores em
  inglês: códigos de `AppError` (inclui `versao-incompativel` → `incompatible-version`),
  estado do cofre `no-vault/locked/unlocked`, força `weak/fair/strong/very-strong`,
  `tag = "result"` com `attached/loaded/canceled/notice`, valores de segredo
  `empty/keep/generated/loaded/new`, aprovação `answered/pending/blocked/wrong-pin/expired`,
  decisões `deny/once/one_hour/always/confirm`, campos de erro `password/confirmation/
  pin_confirmation/current/new`, ids de erro do formulário (`name/group/environment/
  project/username/password/.../auth/attachments/form`).
- **`app/src`**: `call`/`invoke`/`listen` com os nomes novos, leitura por chave entre
  aspas e por ponto, ids de formulário v2 (`password`, `username`, `totp_seed`,
  `private_key`, `engine`, `port`, `db_name`, `tunnel`, `jump`, `variable`, `region`,
  `security`, `protocol`, `text`) — achado da Fase 51; operações do pedido pelo nome v2
  (`run`, `fill`). Comentários "frozen until Phase 54" trocados por `m2:Phase 54`.
- **Ponte de teste** (`crates/ward-test-bridge/src/http.rs`, `app/src/lib/bridge/*`):
  argumentos e respostas em inglês (`{ok, value}` / `{ok:false, code, message}`), mesmos
  comandos da lista. Ficam: o caminho `/ponte/<comando>`, os nomes de tela do `&tela=`
  (`aprovacao`, `credenciais`, `gerador`, `configuracoes`), os modos `--daemon-falso`
  (Fase 56) e os códigos locais da ponte (`comando-desconhecido`, `ponte-indisponivel`…).
- **V5.6**: lista de marcas do `dev/tests/app-release.sh` ganhou `ward_test_bridge`.
- **`frozen.txt`**: tiradas todas as 219 entradas da Fase 54 (comandos/parâmetros do
  `main.rs`, `serde` das Fases 32–34, campos da W nas Fases 35–38, comentários da 30) e os
  cabeçalhos que ficaram vazios.
- **Glossário**: `versao-incompativel` → `incompatible-version` na linha dos códigos de
  `AppError` (§13).

### Literais mudados (`git diff -U0` filtrado por aspas) e motivo
- Nomes de comando, permissões `allow-*`, rótulos de janela, eventos: troca 1:1 do §13.
- Chaves JSON de pedido/resposta da webview e da ponte, ids de campo e valores
  enumerados listados acima: nomes de fio do §13 (antes congelados até esta fase).
- Nomes de thread e `ICONS_DIR`: §13.
- Literais de teste: JSONs de entrada/saída nos nomes novos; listas de "nomes antigos"
  nos testes de ataque novos (de propósito em pt).
- **Não mudaram** (revertidos à mão depois da troca automática): textos de tela
  (`entrada/entradas`, `grupo`, `minutos`, `montado`, `fraca`, `forte`, `bloqueado`,
  `segredo`), `name="versao"`/`name="pendrive"` de inputs locais, nomes de tela do
  `&tela=`, os nomes do daemon em `Problem.field` (`nome`, `senha`, `entrada`,
  `autenticação`…, só o lado direito do mapa vira inglês), quadros v1 dos testes,
  nomes de pasta temporária dos testes e o texto do `expecting` (`texto`).
- Escapes `\u{202E}` conferidos por script no diff (4 linhas, iguais dos dois lados).

## Segurança
- Invariantes: S11 / V5.x (ACL do Tauri) e m2:V4.22–V4.24. Cada janela continua só com
  os comandos dela: `main` sem `approval_request`/`answer_approval`; `approval` só com
  `listen_notices`, `approval_request`, `answer_approval`. Nenhum comando novo; nenhum
  alias para nome antigo. Diff da lógica de segurança só em nomes.
- Testes de ataque escritos:
  - `app/core/src/commands.rs` › `attack_old_command_names_are_refused` (os 34 nomes
    antigos fora de `COMMANDS` e sem `from_name`).
  - `app/core/tests/tauri_config.rs` › `attack_old_command_names_are_gone` (nenhum
    `call('<antigo>'`, `invoke('<antigo>'` nem `"allow-<antigo>"` em `app/src`,
    capabilities e `tauri.conf.json`; conf sem `principal`/`aprovacao`).
  - `app/src-tauri/src/main.rs` › `attack_old_command_names_and_window_labels_are_denied`
    (nome antigo negado pela ACL nas duas janelas, erro de ACL e não de serviço; janela
    com rótulo antigo não ganha capability).
  - `crates/ward-test-bridge/tests/attacks.rs` ›
    `attack_old_command_names_and_keys_are_refused` (nome antigo 404, chave antiga →
    `invalid-request`, sem `codigo`/`valor`, nada na área de transferência).
  - `app/core/src/{credentials,unlock,first_use,attachments,settings,form}.rs` ›
    `attack_old_wire_names_are_*` (chave/valor antigo recusado ou ausente da saída).
- Dependências novas: nenhuma.

## Ficou de fora
- Tela própria para `incompatible-version`: a missão 2 não muda tela (registrado em
  Problemas).
- Testes que falam com daemon/ACL do `main.rs` e da ponte: escritos, **não rodados**
  (regra 6 do PRD). Rodei só os sem daemon (abaixo).
- Screenshots: nenhum texto ou medida visível mudou (os textos tocados pela troca
  automática foram revertidos), então não houve conferência contra o FigCraft.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make fmt` + `cargo fmt` do `app/src-tauri` | 0 |
| `make compile` (com `cargo check` do `app/src-tauri` e build do Vite) | 0 |
| `cargo check --all-targets` do `app/src-tauri` | 0 |
| `cargo clippy --all-targets -D warnings` (`ward-app-core`, `ward-test-bridge`, `app/src-tauri`) | 0 |
| `cargo test -p ward-app-core --test tauri_config` (8 testes, só arquivos) | 0 |
| `cargo test -p ward-app-core --lib` filtrado em `commands::`, `form::`, `credentials::`, `first_use::`, `generator::` (só serde) | 0 |
| `make app-front` | 0 (npm audit: 2 low, já conhecidos) |
| `make english-check` (repositório inteiro) | 0 |

`make check` não rodado: roda o `cargo test` inteiro, que inclui testes com daemon
(regra 6 desta fase).

## Problemas descobertos
- O plano pede "`frozen.txt` vazio e apagado", mas ainda há entradas das Fases 56 e 57
  (variáveis `WARD_DEV_*`, pasta `ward-teste`, funções do `.deb`, binários
  `wardd-*-teste`). Tirei só as da Fase 54; o arquivo fica para a Fase 60.
- `incompatible-version` (antes `versao-incompativel`) continua sem tela própria: cai na
  mensagem genérica de erro. Precisa de decisão de escopo/design numa missão futura.
- Os ids do menu da bandeja (`bandeja-*`) e os códigos locais da ponte de teste
  (`comando-desconhecido`, `aviso-desconhecido`, `falha`, `ponte-indisponivel`,
  `ponte-invalida`) não estão no §13 e ficaram em pt.
- O app `dev/tests/app_e2e.py`/`revealed_selection.py` só usam ids de elemento e nomes
  de tela, que não mudaram; não rodados (precisam de daemon).

- Merge com o master (54a/54b): conflito em `crates/ward-test-bridge/tests/attacks.rs`
  resolvido mantendo a guarda `compile_error!` sem `test-mode` e os valores de D15. Com
  a chave em inglês (`"password"`), o gitleaks acusa as senhas erradas fora de D15
  (`falso-mestra-ward-00`/`-02`), que com a chave `"senha"` passavam; trocadas por
  `falso-WP-senha-01` (D15, errada para o cofre). Árvore e índice do gitleaks limpos; o
  histórico ainda tem 12 achados, todos de commits anteriores do master (nenhum deste
  branch).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/src/{approval,attachments,commands,credentials,daemon,error,external_backup,first_use,form,generator,key,level,restore,service,settings,tray,unlock}.rs` | alterado |
| `app/core/tests/tauri_config.rs` | alterado |
| `app/src-tauri/capabilities/main.json` | criado (era `principal.json`) |
| `app/src-tauri/capabilities/approval.json` | criado (era `aprovacao.json`) |
| `app/src-tauri/capabilities/principal.json`, `aprovacao.json` | apagado |
| `app/src-tauri/src/main.rs`, `app/src-tauri/tauri.conf.json` | alterado |
| `app/src/App.svelte`, `app/src/lib/bridge/{commands,index,test-bridge}.js` | alterado |
| `app/src/lib/components/{GeneratorPopover,LevelDialog}.svelte` | alterado |
| `app/src/screens/*.svelte` e `*.js` (ApprovalWindow, ChangePassword, ChangePin, ChooseUsbDrive, Credentials, EmergencyKit, FirstUse, Form, PasswordGenerator, RestoreBackup, Settings, Unlock, approval, credentials, first-use, form, restore, settings) | alterado |
| `crates/ward-test-bridge/src/http.rs`, `crates/ward-test-bridge/tests/attacks.rs` | alterado |
| `dev/english-check/frozen.txt`, `dev/tests/app-release.sh` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-54.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6: testes com daemon); `make compile`, clippy,
  english-check e testes sem daemon rodados
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Nada visível mudou; sem conferência no FigCraft
- [x] Ficou dentro do escopo da fase
