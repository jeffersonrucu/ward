# Fase 6 — Makefile e `CLAUDE.md` (DT14, DT2) (Missão 2, Milestone 1)

## Implementado
- **Alvos do Makefile** pelo glossário (seção 6), sem alias. São 43 renomes, entre eles: `ferramentas` → `tools`, `origens` → `origins`, `gitleaks-binario` → `gitleaks-binary`, `dev-cofre` → `dev-vault`, `dev-cofre-conferir` → `dev-vault-verify`, `dev-variacoes` → `dev-variations`, `dev-servicos*` → `dev-services*` (`-up`, `-down`, `-status`, `-verify`, `-rotate-host-key`), `teste-*` → `test-*` (`test-no-test-mode`, `test-migration-guide`, `test-chrome-policy`, `test-clipboard`, `test-revealed-selection`…), `app-ponte` → `app-bridge`, `extensao`/`extensao-crx` → `extension`/`extension-crx`, `dev-daemon-parar` → `dev-daemon-stop`.
  - Foram renomeados também os pré-requisitos e o `.PHONY`. O `extension` entrou no `.PHONY`, como pedia o achado da Fase 5.
  - Conferido pela base do make (`make -pRrq`): nenhum alvo antigo existe e todos os novos existem. `LC_ALL=C make -n <antigo>` dá `No rule to make target` para os 43.
- **Variáveis do Makefile**:

  | Antiga | Nova |
  |---|---|
  | `CASA` | `PASSWD_HOME` |
  | `FERRAMENTAS_DIR` | `TOOLS_DIR` |
  | `GITLEAKS_VERSAO` | `GITLEAKS_VERSION` |
  | `CARGO_AUDIT_VERSAO` | `CARGO_AUDIT_VERSION` |
  | `CARGO_DENY_VERSAO` | `CARGO_DENY_VERSION` |
  | `GITLEAKS_OPCOES` | `GITLEAKS_OPTIONS` |
  | `COMPOSE_TESTE` | `COMPOSE_TEST` |
  | `SERVICO` | `SERVICE` |
  | `COFRE` | `VAULT` |
  | `VARIACOES` | `VARIATIONS` |
  | `WARD_DEV_SAIDA` | `WARD_DEV_OUTPUT` |
  | `APP_AUDIT_IGNORAR` | `APP_AUDIT_IGNORE` |
  | `PONTE_DE_TESTE` | `TEST_BRIDGE` |
  | `SEM_MPS` | `NO_MPS` |
  | `SEM_39C` | `NO_39C` |
  | `CONEXOES` | `CONNECTIONS` |
  | `EXTENSAO_ID` | `EXTENSION_ID` |
  | `EXTENSAO_CRX` | `EXTENSION_CRX` |
  | `CHAVE_EXTENSAO` | `EXTENSION_KEY` |
  | `SEM_PLAYWRIGHT_PYTHON` | `NO_PLAYWRIGHT_PYTHON` |

  Os `override` continuam como estavam.
- **Variáveis que o Makefile exporta e o script lê**: trocaram nos dois lados, no mesmo passo.
  - `EXTENSION_ID` e `EXTENSION_CRX`: em `install/install.sh` (a leitura e as mensagens de recusa), `install/deb.sh`, `install/crx.py`, `install/tree.txt` (comentários), `install/pack-extension.sh` (a linha impressa `EXTENSION_ID=… EXTENSION_CRX=…`) e nos testes que chamam `make install`/`deb` (`dev/tests/install.sh`, `deb.sh` e `chrome-policy.sh`, e `dev/container-install/inside.sh`), junto com as mensagens esperadas.
  - `EXTENSION_KEY`: em `install/pack-extension.sh` e `dev/tests/deb.sh`.
  - `NO_PLAYWRIGHT_PYTHON`: em `dev/tests/playwright.sh` e `playwright.py`.
- **Argumentos dos scripts chamados pelo Makefile** (glossário, seção 6):
  - `dev/daemon.sh`: `subir`, `parar` e `estado` viraram `up`, `stop` e `status`. Os chamadores mudaram junto: Makefile, `dev/tests/app-e2e.sh` e `revealed-selection.sh`.
  - `--sem-mps`/`--sem-39c` viraram `--no-mps`/`--no-39c` (`webkit-real.sh`).
  - `--trocar-chave-host` virou `--rotate-host-key`: em `dev/services/generate.py`, com o `args.rotate_host_key`, e em `attacks-services.sh`.
  - `--saida` virou `--output`: no exemplo `dev-cofre` (`main.rs` e os testes de `ler_opcoes` em `testes.rs`) e em `dev/variations.py`.
  - `--autoteste` virou `--self-test`, só no `dev/tests/log-scan.py`.
- **Pasta de build**: `target/sem-modo-teste` virou `target/no-test-mode`. Mudou no Makefile, no `CLAUDE.md` e em `dev/tests/{deb.sh,install.sh,no-test-mode.sh,skill.py}`. A pasta existente foi movida com `mv -n`, o que evita recompilar 3,1 GB.
- **Usos dos nomes de alvo no repositório**: comentários, mensagens "rode `make …`", rótulos de saída que são o nome do alvo (`test-deb: tudo ok`, `dev-services: …`, `dev-variations: …`) e `#[ignore = "… make test-…"]`. Entram aí Rust, scripts, Dockerfiles, `install/systemd/wardd@.socket` (comentário), `dev/compose.yml`, `dev/fixtures/entries.json` (`_sobre`), `README.md`, `docs/instalacao.md` e `skill/README.md`. A troca foi feita por um script com fronteira de token, e os nomes ambíguos (`dev-cofre`, `origens`, `ferramentas`, `extensao`) só mudaram depois de `make`, `make -s`, `"${MAKE[@]}"` ou `saida_fora`.
- **`dev/tests/dev-vault.sh`**: os ataques V2.29 passam as variáveis novas (`saida_fora dev-vault VAULT .ward`, `saida_fora dev-variations VARIATIONS .txt`, `saida_dentro` com `VARIATIONS=`).
- **`CLAUDE.md`**:
  - a tabela de comandos tem só os nomes novos (`VAULT=`, `VARIATIONS=`, `SERVICE=`, `CONNECTIONS=`, `NO_MPS=1`, `NO_PLAYWRIGHT_PYTHON=1`, `EXTENSION_ID`/`EXTENSION_CRX`, `target/no-test-mode`);
  - seção nova "IDs nos comentários e na documentação" (DT2): sem prefixo é da missão 1, em `docs/mission/arquivo/m1/`, e `m2:` marca a missão 2.
- **`.claude/agents/*.md` e `.claude/skills/mission/SKILL.md`**: nenhum cita alvo, variável ou caminho renomeado. Só aparece `make check`, que não muda. Nada foi alterado.
- **Glossário** (seção 6): diz quais argumentos já foram trocados nesta fase e quais ficam para as fases dos scripts, e registra a pasta `target/no-test-mode/`.
- **Literais alterados** (PRD regra 5). Todos são nome de alvo, de variável, de argumento ou de caminho:
  - Rust, exemplo `dev-cofre`:
    - `"--output"` e as mensagens `"--output sem caminho"` e `"--output repetido"`, mais a linha de uso;
    - `"… rode make dev-vault"` (4 mensagens) e `"não existe; rode make dev-services antes"`;
    - `testes.rs`: `"--output"` e `contains("make dev-services")`.
  - Rust, testes:
    - `app/src-tauri/src/area_gtk.rs`: `#[ignore = "…make test-clipboard"]`. A linha está em `strings-0.1.4.txt`, portanto entra na diferença do V6.3;
    - `crates/wardd/tests/playwright.rs`: `#[ignore = "…make test-playwright"]`.
  - `install/install.sh`:
    - `erro "EXTENSION_ID tem de ser…"`, `"EXTENSION_CRX recusado"` e `"o pacote EXTENSION_CRX é da extensão …, não de EXTENSION_ID=…"`;
    - o nome da variável em `conferir_origem`/`copiar_para`.
  - `install/pack-extension.sh`: `printf 'EXTENSION_ID=%s EXTENSION_CRX=%s'`, `EXTENSION_KEY` e `mktemp target/extension-crx.XXXXXX`.
  - Scripts de `dev/`:
    - mensagens e rótulos com o nome do alvo (DT20);
    - o `case` do `daemon.sh` e a linha de uso;
    - `argparse` `--rotate-host-key` e `--output`, e `--self-test` do `log-scan.py`;
    - as flags do `webkit-real.sh`;
    - `os.environ.get("NO_PLAYWRIGHT_PYTHON")`;
    - `SEM=…/target/no-test-mode/debug` e `ALVO=…/target/no-test-mode`.
  - `dev/fixtures/entries.json`: só o texto do `_sobre`, que cita os alvos. Nenhum valor nem entrada mudou.
- **Exceções:** nenhuma entrada nova em `allow.txt` nem em `frozen.txt`.

## Segurança
- **Invariantes tocadas e como ficaram:**
  - **S14 e S15 (V1.2, V1.3, V1.5):** as ferramentas continuam fixas por `override`. `PASSWD_HOME`, `TOOLS_DIR`, `CARGO*`, `PYTHON` e `GIT_CONFIG_*` mantêm o mesmo `override`/`export`, e o sha256 do gitleaks não mudou. `origins` continua pré-requisito de todo alvo que chama o cargo.
  - **V9.2 e D60 (instalação):** a conferência do id da extensão (`^[a-p]{32}$`), a do pacote CRX e a origem dos arquivos não mudaram. Só o nome da variável mudou, nos dois lados do mesmo passo. Quem usar o nome antigo (`sudo make install EXTENSAO_ID=…`) recebe a recusa "EXTENSION_ID tem de ser…": falha fechada, sem alias.
  - **V2.29:** os ataques de saída fora de `.dev/` passam as variáveis novas. Se ficassem com as antigas, o make as ignoraria e o ataque viraria um falso "aceitou". Por isso os dois lados mudaram juntos.
  - **S2 (Fase 5):** `make dev-vault` voltou a apagar o estado velho do daemon de teste. Ver "Problemas", item 1.
- **Testes de ataque:**
  - Nenhum novo: a fase só renomeia.
  - Os ataques existentes seguem os nomes novos: `dev-vault.sh` (V2.29), `install.sh`/`deb.sh` (recusas de `EXTENSION_ID`/`EXTENSION_CRX`/`EXTENSION_KEY`), `attacks-foundation.sh` (`make -s gitleaks-binary`, `make origins`), `attacks-services.sh` (`make -s dev-services-status` com ambiente hostil, `--rotate-host-key`).
  - Foram escritos e não rodaram (regra 6).
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Argumentos que o Makefile não passa:**
  - `--adulterado`/`--versao` (`fake_crx.py`), `--ler` (`revealed_selection.py`) e `WARD_MODO` (`compose.yml`, `ssh/entrypoint.sh`): vão com os scripts, nas Fases 40–46. O glossário registra isso.
  - `--conferir`, `--senha-stdin` e `--cruzado` do exemplo `dev-cofre`: vão com o exemplo, na Fase 43. Até lá, o Makefile chama `--output … --conferir`.
- **Variáveis locais dos scripts com o mesmo nome:** `COFRE=`, `CONEXOES=`, `SEM_MPS`/`SEM_39C` no `webkit-real.sh`, `CASA` no `origins.py` e outros, e `EXTENSAO_DIST`/`CRX_SAIDA` no `pack-extension.sh`. São identificadores dos próprios scripts (Fases 40–47), não variáveis do make.
- **Rótulos `sem-modo-teste:` do `no-test-mode.sh`:** não são nome de alvo; vão com o script.
- **Comentários e eco do Makefile em pt-BR** (`==> 0/6 origens do cargo`, que o `attacks-foundation.sh` procura): a tradução dos comentários do Makefile é da Fase 47. O `english-check` do Makefile só acusa comentários, 1029 achados e nenhum de nome.
- **`/usr/local/share/ward/extensao/` no `CLAUDE.md`:** é caminho instalado, da Fase 57.
- **`docs/escopo.md`, `docs/mission/design-nodes.md` e os docs da missão** (PRD, plano, contrato, handoffs): `escopo.md` não cita nenhum nome antigo. Os outros ficam com as Fases 59 e o histórico.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| script de troca por token, com revisão das ocorrências; grep final dos nomes antigos: só sobram as variáveis locais dos scripts | 0 |
| `make -pRrq` (base de alvos): 0 antigos e todos os 43 novos presentes; `LC_ALL=C make -n <antigo>` dá "No rule to make target" | 0 |
| `mv -n target/sem-modo-teste target/no-test-mode` | 0 |
| `make compile`: origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python e 7 JS | 0 |
| `make fmt-check` | 0 |
| `make english-check PATHS="CLAUDE.md dev/english-check"` | 0 |
| `make english-check PATHS=Makefile`: só achados de comentário (Fase 47), nenhum de nome de alvo nem de variável | 1 (esperado) |
| `python3 -I -B dev/english_check.py --self-test`: 91/91 | 0 |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos
1. **Regressão da Fase 5, corrigida aqui.**
   - A receita do `dev-cofre` apagava `${COFRE%.ward}.estado`, mas o estado do daemon de teste passou a ser `.dev/test-vault.state` (`config.rs`, `daemon.sh`). Com isso, `make dev-vault` não apagava mais o estado velho.
   - A linha agora apaga `.state`. Ela está dentro do arquivo desta fase e é caminho.
   - Era uma falha fechada (o daemon recusaria a geração), não um vazamento.
2. **`make test-webkit-real NO_MPS=1 NO_39C=1` passa `--no-mps--no-39c` grudado.** A falta de espaço já existia antes. O script só reconhece um dos dois flags de cada vez. A fase não corrigiu, porque é comportamento e não nome; fica para a Fase 45 ou 46.
3. **Locale:** em pt-BR o make diz "Sem regra para processar o alvo". O validador do V1.10 precisa de `LC_ALL=C` para achar "No rule to make target".
4. As mudanças das Fases 4 e 5 continuam não commitadas. Esta fase também não commitou.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Makefile`, `CLAUDE.md`, `README.md`, `docs/instalacao.md`, `docs/glossario.md`, `skill/README.md` | alterado |
| `install/install.sh`, `install/deb.sh`, `install/crx.py`, `install/pack-extension.sh`, `install/tree.txt`, `install/systemd/wardd@.socket` | alterado |
| `dev/daemon.sh`, `dev/bridge.sh`, `dev/open-app.py`, `dev/origins.py`, `dev/variations.py`, `dev/compose.yml`, `dev/fixtures/entries.json` | alterado |
| `dev/services/{common.sh,generate.py,verify.sh}` | alterado |
| `dev/container-{deb,install,units}/Dockerfile`, `dev/container-{deb,install,platform,uid,units}/inside.sh`, `dev/container-units/start.sh`, `dev/container-platform/fake.c` | alterado |
| `dev/tests/`: `app-e2e.sh`, `app_e2e.py`, `attacks-foundation.sh`, `attacks-services.sh`, `chrome-policy.sh`, `chrome_policy.py`, `cli.py`, `clipboard.{py,sh}`, `deb.sh`, `dev-vault.sh`, `docker-{platform,uid}.sh`, `extension.{py,sh}`, `fake_crx.py`, `gst-channels.sh`, `home-decoys.sh`, `install.sh`, `log-scan.py`, `login.{py,sh}`, `migration-guide.py`, `nm.{py,sh}`, `no-test-mode.sh`, `playwright.{py,sh}`, `real-home.sh`, `revealed-selection.sh`, `revealed_selection.py`, `skill.py`, `socket-attacks.sh`, `socket_attack.py`, `socket_matrix.py`, `ssh-home.sh`, `ssh_home.py`, `terminal-test.py`, `units.sh`, `variations_test.py`, `webkit-real.sh` | alterado |
| `crates/ward-core/{Cargo.toml,examples/dev-cofre/main.rs,examples/dev-cofre/testes.rs,src/corpo.rs,src/entrada.rs}` | alterado |
| `crates/ward-platform/{Cargo.toml,examples/container.rs,examples/pai-chrome.rs,tests/ativacao.rs,tests/identificacao.rs}`, `crates/ward-nm/examples/chrome-falso.rs`, `crates/ward-test-bridge/src/main.rs` | alterado (comentários) |
| `crates/wardd/{Cargo.toml,examples/cliente-teste.rs,src/servidor.rs,tests/app_valor.rs,tests/playwright.rs,tests/units.rs}` | alterado |
| `app/core/{examples/partida-conteiner.rs,src/credenciais.rs}`, `app/src-tauri/src/area_gtk.rs`, `app/src/lib/componentes/CampoSecreto.svelte` | alterado (comentário/`ignore`) |
| `target/sem-modo-teste/` → `target/no-test-mode/` (fora do git) | movido |
| `docs/mission/handoffs/phase-6.md` | criado |

## Procedimentos
- [ ] `make check`: não rodou, pela regra 6 do PRD. Rodaram `make compile` (0), `make fmt-check` (0), o `english-check` nos arquivos da fase e o autoteste do checador.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon iniciado).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem os nomes novos nos dois lados. Nenhum novo, porque a fase só renomeia.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase. A exceção é a correção do `.estado`/`.state` (item 1 de "Problemas").
