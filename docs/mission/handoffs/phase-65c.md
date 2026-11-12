# Fase 65c — Correção: `make test-app-e2e` para no primeiro passo

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei
  `git reset --hard 0c1c46b` (master atual).
- **Preparação** (`.dev/` do worktree):
  - copiados do checkout principal: `.dev/tools` (sha256 do gitleaks conferido contra o
    `GITLEAKS_BIN_SHA256` do Makefile: SUCESSO), `.dev/pw-lib`, `.dev/services` e
    `.dev/known_hosts`;
  - `make dev-vault`, `make dev-variations` (282 padrões) e `make app-front`: 0.
- **Causa do timeout em `#master-password`**: o pacote de teste `app/dist-teste` do
  checkout principal era velho. Ele foi gerado em 2026-11-10 14:42, antes das fases 54/56
  da missão 2, e ainda era o pacote da 0.1.x (`assets/ponte-teste-*.js`, com
  `codigo`/`mensagem` e o caminho antigo `/ponte/<comando>`).
  - O alvo refazia os binários (`wardd`, `ward-test-bridge` e `ward` às 16:03), mas não o
    pacote. A ponte atual só atende `POST /test-bridge/<comando>`, então a resposta não
    vinha em JSON e a tela de destravar mostrava "resposta inválida da ponte de teste" no
    lugar do campo da senha. A captura da falha original (`antes-falha-0.png`) mostra isso.
  - O `app-e2e.sh` só conferia se o `index.html` existia.
  - Não era o bwrap, a porta, a guarda nem D68: com um pacote novo, a suíte passou 29/29 de
    primeira, com o mesmo `chrome-isolated.sh` e a mesma guarda.
  - **Reprodução**: troquei o `app/dist-teste` do worktree pelo pacote velho do checkout
    principal e rodei `dev/tests/app-e2e.sh`. A falha foi a mesma, agora com a mensagem nova
    (`fase65c-bundle-velho.log`). Depois devolvi o pacote certo.
- **Correção** (o isolamento e a guarda não mudaram):
  - `Makefile › test-app-e2e`: depois dos três `cargo build`, refaz o pacote de teste com
    `npm run build:test`. O `npm ci --ignore-scripts` só roda se faltar `app/node_modules`,
    como no `compile`. Assim o pacote fica sempre do mesmo build que os binários.
  - `dev/tests/app-e2e.sh`: recusa (sai 1, antes de subir o daemon) um `app/dist-teste`
    mais velho que qualquer arquivo de `app/src`, `app/index.html`, `app/vite.config.js`,
    `app/package.json` ou `app/package-lock.json` (`find -newer`). Isso cobre quem roda o
    script direto. Testado: com `touch` num `.svelte` o script sai 1 e diz o arquivo.
  - `dev/tests/app_e2e.py › unlock`: se o campo não aparece e a tela mostra um erro da ponte
    de teste, a falha diz que o pacote é de outro build, em vez de um timeout seco.
  - `CLAUDE.md`: a linha do `make test-app-e2e` cita a Fase 65c.

### Resultado

| Suíte | Resultado | Evidência (`docs/mission/evidence/M6/fase-65c/`) |
| --- | --- | --- |
| `PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null` | **0**: 29/29. Chrome sem extensão (guarda da 65a), destravar, V5.10, V5.16 (negar, fechar, **uma vez**, **1 hora** com travar/destravar, **sempre**, a entrada fica Livre), V5.24 (**Conectar com token** no salto, `/proc` sem o token, reuso, outra entrada e token vencido, todos negados). Logs sem padrão | `fase65c-test-app-e2e.log`, `test-app-e2e/qa.json` e PNGs |
| `app-e2e.sh` com o pacote velho do checkout principal | 1, com a mensagem nova (reprodução da causa) | `fase65c-bundle-velho.log` |
| `app-e2e.sh` com o pacote mais velho que `app/src` | 1, antes do daemon | — |
| `make compile` | 0 (55 shell, 39 Python, 7 JS; 0 erro) | — |
| `make english-check` | 0 (0 achados) | — |
| `make check` | 0 ("tudo limpo"; gitleaks sem vazamento) | — |

PNGs (app no Chrome isolado pela ponte, 1280×800; aprovação em 560×760):
- `docs/mission/evidence/M6/fase-65c/antes-falha-0.png`: a falha original, com o erro da
  ponte na tela de destravar
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/formulario-novo-vazio.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/formulario-novo-erro-no-campo.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/aprovacao-ward-run-real.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/conectar-acme-ssh-salto.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/conectar-reuso-pede-aprovacao.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/conectar-outra-entrada-pede-aprovacao.png`
- `docs/mission/evidence/M6/fase-65c/test-app-e2e/sempre-entrada-livre.png`

## Segurança

- **Invariantes tocadas**:
  - V6.8 e V6.9: o Chrome continua no bwrap do `chrome-isolated.sh`, com a guarda dentro e
    a exigência de zero alvos `chrome-extension://`. Nada disso mudou, e o passo passou
    (`ok - Chrome sem nenhuma extensão`).
  - V3.2: o daemon de teste é só da suíte (socket em `/run/user/<uid>/ward-test-e2e`).
  - S13: a varredura dos logs (V5.27) deu 0 arquivos com segredo em todas as rodadas.
- **Testes de ataque**: a suíte roda os ataques ao token do Conectar (`/proc/*/cmdline` e
  `environ`, reuso, outra entrada, depois de 30 s), todos verdes. Teste novo da fase: um
  pacote de teste fora do build atual é recusado (`app-e2e.sh`) ou explicado (`app_e2e.py`).
  Assim a suíte não pode rodar contra uma interface que não é a do código.
- **Evidência**:
  - `grep -rlF -f .dev/variations.txt docs/mission/evidence/M6/fase-65c/` não acha nada
    (exit 1);
  - nos logs, o home e o caminho do worktree viraram `<home>` e `<repo>`.
- **Dependências novas**: nenhuma.
- **Não tocados**: o cofre real, o `wardd` instalado, `/etc`, `/usr/local`, o perfil real
  do Chrome e os serviços de teste do checkout principal (nem parados nem recriados).
  Nenhum Chrome fora do `chrome-isolated.sh`. Nenhum `sudo`.

## Ficou de fora

- `make test-app-e2e` com `systemd-run --user --scope -p TasksMax=1500` e redireção para
  log: **negado** pela permissão. Rodei a forma simples pedida
  (`PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null`, e a mesma com
  `> .dev/fase65c-test-app-e2e.log 2>&1`). A suíte não faz fork em rajada (um Chrome, um
  `ward run` por vez).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 0c1c46b013366cef4a54c3e4515bece5cacfcde7` | 0 |
| cópia de `.dev/{tools,pw-lib,services,known_hosts}`; `sha256sum --check --strict` do gitleaks | 0 |
| `systemd-run --user --scope -q -p TasksMax=1000 make dev-vault` | 0 |
| `make dev-variations` | 0 |
| `systemd-run --user --scope -q -p TasksMax=1000 make app-front` | 0 |
| `systemd-run --user --scope -q -p TasksMax=1500 make test-app-e2e PW_PYTHONPATH=$PWD/.dev/pw-lib/site WARD_DEV_SOCKET_DIR=/run/user/1000/ward-test-e2e-65c > …/fase65c-e2e-1.log 2>&1 < /dev/null; …` | **negado** pela permissão |
| `PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null` (antes da correção, pacote novo) | 0 (29/29) |
| `touch app/src/App.svelte; PW_PYTHONPATH=… dev/tests/app-e2e.sh < /dev/null` | 1 (recusa do pacote velho) |
| `app-e2e.sh` com o `dist-teste` do checkout principal | 1 (reprodução, `fase65c-bundle-velho.log`) |
| `PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null > .dev/fase65c-test-app-e2e.log 2>&1` | 0 (29/29) |
| `make english-check` | 0 |
| `make compile` | 0 |
| `make check` | 0 |
| `rm -rf target app/src-tauri/target app/node_modules .dev/pw-lib .dev/docker` | 0 |

`df -h /`: 18 GB livres no começo. No fim, depois da limpeza: 21 GB. Nenhum Chrome,
daemon de teste ou processo meu ficou de pé.

## Problemas descobertos

1. **O mesmo pacote velho afeta outros alvos**:
   - `make app-bridge` e `make test-revealed-selection` (`dev/tests/revealed-selection.sh`)
     também só conferem se `app/dist-teste/index.html` existe;
   - o `make test-extension` copia os tokens de `app/src`, não do pacote, e não é afetado.

   No checkout principal, o `app/dist-teste` ainda é o da 0.1.x: o próximo
   `make test-app-e2e` o refaz, e `make app-front` também. Não mexi nos outros alvos porque
   estão fora da fase; vale aplicar a mesma checagem neles.
2. **Permissão**: o `systemd-run … make test-app-e2e … > log` foi negado (comando exato na
   tabela). A forma simples passou.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `Makefile` | alterado (`test-app-e2e` refaz o pacote de teste) |
| `dev/tests/app-e2e.sh` | alterado (recusa pacote mais velho que as fontes) |
| `dev/tests/app_e2e.py` | alterado (mensagem do destravar com erro da ponte) |
| `CLAUDE.md` | alterado (linha do `test-app-e2e`) |
| `docs/mission/evidence/M6/fase-65c/` | criado (`fase65c-*.log`, `antes-falha-0.png`, `test-app-e2e/`) |
| `docs/mission/handoffs/phase-65c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (ataques do token do Conectar verdes; pacote fora do build recusado)
- [x] (UI) Conferido: as capturas do app saem do Chrome isolado
- [x] Ficou dentro do escopo da fase
