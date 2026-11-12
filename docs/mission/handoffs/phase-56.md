# Fase 56 — Variáveis internas e de teste e marca de `test-mode` (DT12)

## Implementado
- **Variáveis (DT12, glossário §8)**, com todos os usos (Rust, scripts, Makefile):
  `WARD_DEV_EXTENSAO`→`WARD_DEV_EXTENSION`, `WARD_DEV_DBUS_SISTEMA`→`WARD_DEV_SYSTEM_DBUS`,
  `WARD_GERENTE_PROGRAMA`→`WARD_MANAGER_PROGRAM`, `WARD_CAMINHO`/`WARD_ESPERADO`/
  `WARD_ATIVACAO`→`WARD_PATH`/`WARD_EXPECTED`/`WARD_ACTIVATION`,
  `WARD_PLATAFORMA_FILHO`→`WARD_PLATFORM_CHILD`, `WARD_PAI_CHROME_SOCK`→
  `WARD_CHROME_PARENT_SOCK`, `WARD_EXIGIR_OATHTOOL`→`WARD_REQUIRE_OATHTOOL`,
  `WARD_HOME_COBERTO`/`WARD_ISCAS_DENTRO`/`WARD_SSH_HOME_DENTRO`/`WARD_SSH_HOME_TRAB`
  (os três scripts do bwrap de uma vez) → `WARD_HOME_COVERED`/`WARD_DECOYS_INSIDE`/
  `WARD_SSH_HOME_INSIDE`/`WARD_SSH_HOME_WORK`, `WARD_TESTE_GRAVACAO_*`→`WARD_TEST_WRITE_*`,
  `WARD_TESTE_BACKUP_*`→`WARD_TEST_BACKUP_*`, `WARD_TESTE_MANTER_PERFIL`→
  `WARD_TEST_KEEP_PROFILE`, `WARD_TESTE_PG_SENHA_ARQUIVO`→`WARD_TEST_PG_PASSWORD_FILE`,
  `WARD_CONTROLE`→`WARD_CONTROL`, `ATQ_*`→`ATK_*`, `EXTENSAO_DIST`/`CRX_SAIDA`→
  `EXTENSION_DIST`/`CRX_OUTPUT`. `WARD_ESTADO`→`WARD_STATE` veio igual da Fase 58 (merge sem conflito).
- **Binários e pastas de teste**: `crates/wardd/app-teste/` → `test-app/` (bin
  `wardd-test-app`), `aprovador-teste/` → `test-approver/` (bin `wardd-test-approver`);
  logs passam a `.dev/logs/wardd-test-app.log`/`wardd-test-approver.log` (saem do nome do
  binário). Arquivo de modo do aprovador `aprovador.modo` → `approver.mode`.
- **Pasta do socket de teste** `ward-teste` → `ward-test` (`TEST_DIR_NAME`, daemon.sh,
  bridge.sh, cli.py, contêineres uid/plataforma, app/core), `ward-teste-e2e` →
  `ward-test-e2e`, `ward-teste-selecao` → `ward-test-selection`; outros nomes
  `ward-teste-*` de teste (memória, nm, chaves geradas, `/ward-teste/` das páginas do CDP).
- **Ponte de teste** (achado da 54): mensagens/uso `ward-ponte-teste:` →
  `ward-test-bridge:` (o `dev/bridge.sh` aceita as duas), caminho `/ponte/` →
  `/test-bridge/`, `&tela=` → `&screen=` e telas `credentials/generator/settings/approval`
  (App.svelte, SideNav, app_e2e.py), `--daemon-falso`/`--anexos` → `--fake-daemon`/
  `--attachments` e os 20 modos (lista no glossário §9), códigos locais
  `unknown-command`/`unknown-notice`/`failure`/`bridge-unavailable`/`bridge-invalid`,
  thread `ward-bridge`. Ids da bandeja `bandeja-*` → `tray-title/subtitle/open/action/quit`.
- **Achado da 47b**: `/cadastro` do totp 8083 → `/registration` (server.py, extension.py,
  `wardd/tests/extension.rs`, CLAUDE.md).
- **Achado da Fase 9**: log do `wardd` `(UidRecusado)`/`(SemIdentidade)` →
  `(UidRefused)`/`(NoIdentity)` com o `dev/container-uid/inside.sh`; `pai=NoIdentity(`.
- **Marca de `test-mode`**: `install/install.sh` recusa (lista `TEST_MODE_MARKS`) as marcas
  novas **e** as da 0.1.x (`ward-test-bridge`/`ward-ponte-teste`, `wardd-test-approver`/
  `wardd-aprovador-teste`, `wardd-test-app`/`wardd-app-teste`, `ward-teste`/`ward-test`),
  mais qualquer `WARD_DEV_*`; mensagem "foi compilado com modo-teste" → "foi compilado com
  test-mode" (testes de instalação, `.deb` e contêiner de instalação ajustados).
  `no-test-mode.sh`: marcas novas e antigas, `WARD_DEV_` e pasta do socket nos binários,
  autoteste que planta cada marca numa cópia do `wardd` de release, prefixo `no-test-mode:`.
  `app-release.sh` (V5.6) e `app-dist.py`: nomes novos e antigos, `WARD_DEV_`.
- `frozen.txt`: tiradas todas as entradas da Fase 56 (no merge, a 57 tirou as dela: o
  arquivo ficou só com o cabeçalho; a Fase 60 o apaga). `CLAUDE.md` (variáveis, socket,
  log, `/registration`, linhas de `test-no-test-mode` e instalação) e glossário (§9 com as
  marcas e a tabela "Nomes internos e de teste trocados na Fase 56") atualizados.
- Merge do master `4fc010d` (Fases 55, 57, 58): só conflito em `frozen.txt` (ambos
  esvaziaram), resolvido com o cabeçalho.

### Literais mudados (motivo)
Todos são nomes (variável, binário, caminho, modo, código, id) da tabela acima ou textos
que os citam: mensagens `ward-test-bridge:`, `no-test-mode:`, `app-release:`, "foi
compilado com test-mode"; `panic!("test_mode={m}…")` no teste do `hardening.rs`; nome do
Server do serviço web de teste `ward-test`. Comparação por script: nenhuma linha do diff
contra o master tem escape `\u`.

## Segurança
- Invariantes: S8 / m2:V5.4, m2:V5.5 (marca de `test-mode`), S11 (ponte e ACL só de
  nomes), S5/V3.2 (log do uid recusado). Nenhuma lógica de segurança mudou: só nomes; as
  listas de recusa ganharam as formas novas sem perder as antigas. `WARD_DEV_*` só são
  lidos em código `cfg(feature = "test-mode")` (sem mudança).
- Testes de ataque escritos:
  - `dev/tests/install.sh` › bloco "m2:V5.4": cada marca nova e antiga plantada num
    `wardd` de release é recusada antes de qualquer arquivo, e o `wardd` de teste 0.1.4 de
    `.dev/baseline/` também (pulado com aviso se a linha de base faltar).
  - `dev/tests/no-test-mode.sh` › autoteste "m2:V5.5": cada marca plantada é acusada.
  - `crates/ward-test-bridge/tests/attacks.rs` › `attack_old_command_path_is_refused`
    (`/ponte/<comando>` com token e origem certos: nada roda, área vazia; controle no novo).
  - `app/core/src/tray.rs` › `attack_forged_id_in_the_menu_does_nothing` com os ids
    `bandeja-*` antigos e as variações dos novos.
  - `crates/wardd/tests/approval.rs` › V3.15 proíbe também `test_approver`,
    `test-approver`, `WARD_APPROV`, `auto_approv` em `src/`.
- Conferência à mão (sem suíte de daemon): a função `verify_no_test_mode` do
  `install.sh`, extraída num harness, recusa cada marca nova e antiga com a mensagem da
  marca mais longa, recusa os `ward`/`wardd` de teste 0.1.4 da linha de base e aceita os
  de release 0.1.4 e os instalados (`/usr/local/bin/ward`, `wardd`, `ward-app`, só lidos).
  `strings` dos `ward`/`wardd`/`ward-nm` de release desta árvore: nenhuma marca.
- Dependências novas: nenhuma.

## Ficou de fora
- Nomes do Docker (projeto `ward-teste` do compose, imagens `ward-teste/*:1`) e caminhos
  dentro dos contêineres (`/opt/teste/*`, `/etc/sudoers.d/ward-teste`, `falso-ward-teste`):
  trocar o projeto do compose deixaria os contêineres de pé com o nome velho prendendo as
  portas; não são marca nem identificador. Registrado no glossário.
- Valor `/run/user/1000/ward-teste/...` do `dev/fixtures/protocol-v2.jsonl` e do
  `protocol/tests.rs` (dado igual à linha de base v1) e o caminho falso do exemplo
  `dev-vault`.
- Palavras de modo do aprovador (`confirmar`, `negar`, `uma_vez`, `uma_vez_ciente`):
  literais das suítes; só o arquivo mudou de nome (o achado era o arquivo).
- Modo `teste` do Vite e `app/dist-teste/` (fora da lista da fase; mudar o modo do Vite
  mexe no que entra no pacote de produção).
- Tela própria para `incompatible-version`: a missão 2 não muda tela.
- `docs/mission/design-nodes.md` cita `--daemon-falso cheio --anexos`: fica para a Fase 59.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 1abe42a` (worktree nasceu em `b4d5e1b`) | 0 |
| `git merge master` (`4fc010d`; conflito só em `frozen.txt`) | 0 após resolver |
| `make fmt` | 0 |
| `make compile` (log `scratchpad/fase56/fase56-compile.log`; 52 shell, 36 Python, 7 JS) | 0 |
| `make english-check` (repositório inteiro) | 0 |
| `cargo clippy --workspace --all-targets --all-features -D warnings` | 0 |
| `cargo test -p ward-app-core --lib tray::` (10) | 0 |
| `cargo test -p ward-app-core --features test-mode --lib fake_daemon::` (3) | 0 |
| `cargo test -p ward-test-bridge --features test-mode --test attacks -- attack_old` (2) | 0 |
| `npm run build:test` + `dev/tests/app-dist.py` | 0 |
| `make test-no-test-mode` (sem daemon; com o autoteste novo) | 0 |
| harness de `verify_no_test_mode` (`scratchpad/fase56/vt.sh`) | 0 |
| `make check` | não rodado (PRD regra 6: inclui testes com daemon) |

## Problemas descobertos
- `crates/wardd/tests/run.rs` (~linha 1149) espera do aprovador de teste
  `operacao=executar … decisao=uma_vez`, mas `Decision::name()` e `Operation::name()` já
  são do protocolo v2 (`once`, `run`): o teste deve falhar no M6. Não corrigi (fora da fase).
- Depois do merge, quem tiver o daemon de teste no ar com a pasta `ward-teste` precisa
  `make dev-daemon-stop` com o checkout antigo; logs `.dev/logs/wardd-*-teste.log` velhos
  ficam órfãos (podem ser apagados).
- `frozen.txt` está só com o cabeçalho; a Fase 60 o apaga.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/test-app/main.rs`, `crates/wardd/test-approver/main.rs` | renomeado (de `app-teste/`, `aprovador-teste/`) |
| `crates/wardd/Cargo.toml`, `src/{config,launch,lock,main,server,udisks}.rs` | alterado |
| `crates/wardd/tests/{approval,app_value_binary,extension,lock,run,socket,test_app}.rs` | alterado |
| `crates/ward-platform/src/{hardening,server,socket}.rs`, `tests/{activation,hardening}.rs`, `examples/{chrome-parent,container}.rs` | alterado |
| `crates/ward-core/src/{atomic_write,backup,totp}/tests.rs`, `tests/body_memory.rs` | alterado |
| `crates/ward-test-bridge/src/{http,main,simulated}.rs`, `tests/attacks.rs` | alterado |
| `app/core/src/{daemon,fake_daemon,tray}.rs` | alterado |
| `app/src/App.svelte`, `lib/components/SideNav.svelte`, `lib/bridge/{index,test-bridge}.js` | alterado |
| `install/install.sh` (só `verify_no_test_mode`), `install/pack-extension.sh` | alterado |
| `dev/bridge.sh`, `dev/daemon.sh`, `dev/services/{generate.py,web/server.py}` | alterado |
| `dev/container-{install,platform,uid}/inside.sh` | alterado |
| `dev/tests/{app-dist.py,app_e2e.py,app-e2e.sh,app-release.sh,chrome_policy.py,chrome-policy.sh,cli.py,deb.sh,extension.py,extension.sh,home-decoys.sh,install.sh,login.py,login.sh,nm.py,no-test-mode.sh,pg_select.py,playwright.py,playwright.sh,real-home.sh,revealed-selection.sh,ssh_home.py,ssh-home.sh}` | alterado |
| `dev/english-check/frozen.txt`, `Makefile`, `CLAUDE.md`, `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-56.md` | criado |

Nos arquivos das Fases 55/57 (`install/install.sh`, `dev/tests/install.sh`, `deb.sh`,
`dev/container-install/inside.sh`): só a função de marcas, o texto da mensagem
conferido, `EXTENSION_DIST`/`CRX_OUTPUT` e o bloco de recusas m2:V5.4.

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não (regra 6); rodados `make compile`, `make english-check`, clippy e testes sem daemon
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica (nenhum texto ou medida de tela mudou)
- [x] Ficou dentro do escopo da fase
