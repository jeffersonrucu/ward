# Fase 47b — Resto fora do Rust (Missão 2, Milestone 3)

## Implementado

Parte de `master` 18f76bc (o worktree estava em b4d5e1b; `git reset --hard 18f76bc` com a
árvore limpa). Antes, `make english-check` no repositório acusava 444 achados (239 de
`dev/fixtures/entries.json`); agora só os 239 de `dev/fixtures/entries.json` (Fase 48).

- Comentários traduzidos: `.gitignore`, `app/.npmrc`, `app/src-tauri/Cargo.toml`,
  `app/vite.config.js`.
- `app/vite.config.js`: plugin local `semLinksDeErro` → `noErrorLinks` (nome do plugin
  `ward-sem-links-de-erro` → `ward-no-error-links`, parâmetro `codigo` → `chunk`). Ninguém
  procura esses nomes (grep). O modo do Vite continua `teste` e a pasta `dist-teste/`
  (strings, não acusadas; `App.svelte`, `lib/bridge` e vários scripts dependem delas).
- `app/package.json`: script `build:teste` → `build:test`. O único chamador é o alvo
  `app-front` do `Makefile`, mudado junto. `npm run build:test` rodado (exit 0) e
  `dev/tests/app-dist.py` passa (a URL `svelte.dev/e/` continua fora do pacote).
- Scripts de `dev/`: comentários que só citavam nome que muda depois foram reescritos sem
  ele (lógica, código e textos que testes procuram intactos):
  - `daemon.sh`: pasta do socket de teste, variáveis do barramento e da extensão
    (`WARD_DEV_DBUS_*`, `WARD_DEV_EXT*`), pedido `abrir_app` → "open-app request".
  - `bridge.sh`, `open-app.py`: `abrir_app` → "open-app request".
  - `tests/chrome-policy.sh`, `extension.sh`, `login.sh`, `playwright.sh`: binário
    `wardd-aprovador-teste` → "test approver"; arquivo `aprovador.modo` → "its mode
    file in `$WORK_DIR`"; `ward lista --json`, `ward travar`, `/cadastro`,
    "domínio não confere", `preencher` → descrições em inglês.
  - `tests/nm.sh`, `socket-attacks.sh`: `preencher`/`sugerir`/`nao_autorizado` →
    "fill and suggest operations" / "not authorized".
  - `tests/no-test-mode.sh`: `modo-teste` e a variável da extensão no comentário.
  - `tests/ssh-home.sh` (`/tmp/sentinela-ssh`), `units.sh` (`/opt/teste`, "papel app
    recusado"), `home-decoys.sh` (`iscas.sh`/`iscas2.sh`).
- `dev/english-check/frozen.txt`: 7 entradas novas (bloco "Phase 47b"), cada uma com o
  identificador inteiro (não a palavra solta), para não cobrir comentário:
  - `app/src-tauri/capabilities/aprovacao.json` `aprovacao` → Fase 54 (capability do Tauri);
  - `dev/daemon.sh` `WARD_DEV_DBUS_SISTEMA`, `WARD_DEV_EXTENSAO` → Fase 56;
  - `dev/tests/home-decoys.sh` `WARD_ISCAS_DENTRO`, `dev/tests/real-home.sh`
    `WARD_HOME_COBERTO`, `dev/tests/ssh-home.sh` `WARD_SSH_HOME_DENTRO` e
    `WARD_SSH_HOME_TRAB` → Fase 56 (guardas de reexecução no bwrap).

## Segurança

- Invariantes tocadas: S11/S14 só nos comentários (`Cargo.toml`, `.npmrc`,
  `vite.config.js`), sem mudar configuração: features do Tauri, `ignore-scripts`,
  `save-exact`, registro, CSP/plugin de links iguais. V5.2 (sem URL remota no pacote)
  conferido pelo `dev/tests/app-dist.py` depois do renome do plugin.
- Testes de ataque escritos: nenhum (fase só de comentário e nome; nenhuma lógica mudou).
- Dependências novas: nenhuma.

## Ficou de fora

- `dev/fixtures/entries.json` (239 achados): Fase 48, rodando noutro worktree.
- Modo `teste` do Vite e `dist-teste/`: não são acusados (strings) e mudar exige
  `app/src` e vários scripts; fica para quem renomear a ponte de teste (Fase 54/56).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 18f76bc` | 0 |
| `make english-check` antes (log `<scratchpad>/fase47b/fase47b-english-before.log`, 444 achados) | 2 |
| `make english-check` depois (log `fase47b-english-after.log`): só `dev/fixtures/entries.json` (239) | 2 (esperado) |
| `python3 -I dev/english_check.py --self-test` | 0 |
| `make compile` (log `fase47b-make-compile.log`; 48 shell, 35 Python, 7 JS, 0 erro) | 0 |
| `npm run build:test` (log `fase47b-build-test.log`) | 0 |
| `python3 -I -B dev/tests/app-dist.py` (log `fase47b-app-dist.log`) | 0 |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- O `CLAUDE.md` e handoffs antigos não citam `build:teste`; nada a mudar. O `CLAUDE.md`
  continua com nomes antigos de alvos (fase de documentação).
- `/cadastro` do serviço `totp` (8083) e o arquivo `aprovador.modo` do aprovador de
  teste são nomes pt-BR fora de comentário que o checador não acusa (strings/caminhos);
  ficam para a Fase 56 se ela quiser.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `.gitignore` | alterado |
| `Makefile` | alterado |
| `app/.npmrc` | alterado |
| `app/package.json` | alterado |
| `app/src-tauri/Cargo.toml` | alterado |
| `app/vite.config.js` | alterado |
| `dev/bridge.sh` | alterado |
| `dev/daemon.sh` | alterado |
| `dev/open-app.py` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `dev/tests/chrome-policy.sh` | alterado |
| `dev/tests/extension.sh` | alterado |
| `dev/tests/home-decoys.sh` | alterado |
| `dev/tests/login.sh` | alterado |
| `dev/tests/nm.sh` | alterado |
| `dev/tests/no-test-mode.sh` | alterado |
| `dev/tests/playwright.sh` | alterado |
| `dev/tests/socket-attacks.sh` | alterado |
| `dev/tests/ssh-home.sh` | alterado |
| `dev/tests/units.sh` | alterado |
| `docs/mission/handoffs/phase-47b.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não: PRD regra 6 (`make compile` 0; `english-check` só com `entries.json`)
- [x] Só cofre de teste e credenciais falsas usados (nenhum usado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG com lógica tocada)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
