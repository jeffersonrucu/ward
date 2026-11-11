# Fase 46 — O resto de `dev/` e o contrato do `common.sh` (Missão 2, trilha S)

## Implementado

O worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard 5efa7f5` (master). A troca foi 1:1 pelo `docs/glossario.md`, no padrão das Fases 40–45. Em todos os arquivos, comentários e docstrings foram para o inglês ("Fase" → "Phase", IDs da missão 1 sem prefixo). As mensagens impressas (`ok`, `FALHA`, erros, uso) continuam em pt-BR e iguais byte a byte.

O trabalho foi dividido em quatro grupos de arquivos disjuntos. Fiz o contrato e o grupo D. Os grupos A, B e C foram feitos em paralelo, no mesmo worktree, por forks deste worker. Revisei os diffs deles.

### Contrato de `dev/env.sh` e `dev/services/common.sh` (só nomes e comentários)

**Quem carrega:** define `ROOT` antes do `.`. Os `RAIZ="$ROOT"` do `install.sh` e do `attacks-services.sh` saíram. O `real-home.sh` e o `deb.sh` usavam `RAIZ` como raiz própria e agora usam `ROOT`.

| Antigo | Novo |
| --- | --- |
| `RAIZ` | `ROOT` |
| `CASA_PASSWD` (`env.sh`) | `PASSWD_HOME` |
| `DS` | `SERVICES_DIR` |
| `SEG` | `SECRETS_DIR` |
| `DOCKER_VAZIO` | `DOCKER_EMPTY` |
| `IMAGEM_PG` | `PG_IMAGE` |
| `OPCOES_SSH` | `SSH_OPTIONS` |
| `exigir` | `require` |
| `cliente` | `client` |
| `montar_opcoes_ssh` | `build_ssh_options` |
| `ssh_senha` | `ssh_password` |
| `ssh_chave` | `ssh_key` |
| `ssh_interno` | `ssh_internal` |
| `esperar_wordpress` | `wait_wordpress` |
| locais `arq`/`porta`/`usuario`/`salto` | `file`/`port`/`user`/`jump` |

Ficam como estão: `psql_app`, `wp_login` e `COMPOSE`.

**Usos ajustados** em `verify.sh`, `compose.sh`, `real-home.sh`, `deb.sh`, `attacks-services.sh`, `install.sh`, `attacks-foundation.sh` (`CASA_PASSWD`), `dev-vault.sh` (`CASA_PASSWD`), `home-decoys.sh`, `ssh-home.sh`, `docker-platform.sh`, `docker-uid.sh` e `units.sh`.
- No `real-home.sh`, o `declare -p` que leva o ambiente do `common.sh` ao bash rastreado passou a `SSH_OPTIONS MUT COMPOSE PG_IMAGE SERVICES_DIR SECRETS_DIR ROOT TMP`. É a mesma lista de antes, com os nomes novos.
- As linhas `CASA="$CASA_PASSWD"` (`home-decoys.sh`, `ssh-home.sh`) saíram, e os usos passaram a `PASSWD_HOME`.
- Conferi por grep: nenhum `RAIZ`, `CASA_PASSWD`, `OPCOES_SSH`, `IMAGEM_PG`, `$DS`, `$SEG`, `exigir`, `cliente <comando>`, `ssh_senha`, `ssh_chave`, `ssh_interno`, `esperar_wordpress` ou `montar_opcoes_ssh` sobra em `dev/`.

### Grupo D (eu)

- **`services/verify.sh`**:
  - `falhas`/`falhou` → `failures`/`failed`, `par`/`usuario`/`arq`/`cab` → `pair`/`user`/`file`/`headers`.
  - Os comentários citam a porta (2222/2223/2224) em vez do nome do serviço (`ssh-salto` …, que são dados do cofre de teste).
- **`services/compose.sh`**: só `ROOT` e o comentário.
- **`services/ssh/entrypoint.sh`**: `modo` → `mode`, `segredos` → `secrets`, `definir_senha` → `set_password`, `so_chave` → `key_only`, `autorizar_chave` → `authorize_key`. O comentário do `Dockerfile` também foi para o inglês.
- **`services/wordpress/install.sh`**: `banco_pronto` → `db_ready`.
- **`tests/clipboard.sh`**: `linha`/`alvo` → `line`/`target`, no Python embutido.
- **`tests/app-e2e.sh`** e **`tests/revealed-selection.sh`**: `GUARDA` → `BACKUP` e `limpar` → `cleanup`.
- **`tests/gst-channels.sh`**, no padrão do `webkit-real.sh` da Fase 45:
  - `LANCADOR` → `LAUNCHER`, `valor_fixo` → `fixed_value`, `LISTA` → `ALLOWLIST`;
  - `isca` → `decoy`, `rot` → `label`, `casa` → `home`, `marca(s)` → `marks`, `canal` → `channel`;
  - `falhou` → `failed`, `nao` → `not_ok`, `CASA` → `GST_HOME`;
  - o C gerado passou a `mark()`, com as macros `MARKS`/`LABEL`.

### Grupo A (fork)

- **`daemon.sh`**: `subir`/`parar` → `start`/`stop_daemon`, `pid_vivo` → `live_pid`, `COFRE`/`ESTADO` → `VAULT`/`STATE`, `MARCA_APP` → `APP_MARK`, `NIVEL_LOG` → `LOG_LEVEL`, `UID_ATUAL` → `CURRENT_UID`, `CASA` → `PASSWD_HOME` (local; o script não carrega o `env.sh`), `PASTA_SOCKET` → `SOCKET_DIR` (local). Os subcomandos `up`/`stop`/`status` são os mesmos.
- **`bridge.sh`**: `antes` → `before`, `novo` → `new_output`.
- **`open-app.py`**: `ler_exato` → `read_exact`, `resposta` → `reply`, `tipo` → `kind` … Conferido por AST.
- **`socket-attacks.sh`, `nm.sh`, `extension.sh`, `login.sh`, `playwright.sh`**:
  - comuns: `COFRE` → `VAULT`, `LISTA` → `PATTERN_LIST`, `TRAB` → `WORK_DIR`, `limpar` → `cleanup`, `falhas` → `failures`;
  - por arquivo: `CONEXOES` → `CONNECTIONS` (variável local, não a do Makefile), `SONDA` → `PROBE`, `USO` → `USAGE`, `FALSO` → `FAKE`, `APROVADOR` → `APPROVER`.

### Grupo B (fork)

- **`chrome-policy.sh`**: `POLITICA` → `POLICY`, `rodada` → `run_round`, `sem`/`com` → `without`/`with`.
- **`ssh-home.sh`**: `COFRE`/`LISTA`/`TRAB` como no grupo A.
- **`home-decoys.sh`**: `ABERTAS` → `OPENED`, `FASE` → `STAGE`, `escritores` → `writers`, `iscas` → `decoys`, `rodar` → `run_stage`.
- **`docker-platform.sh`, `docker-uid.sh`, `units.sh`**: `IMAGEM` → `IMAGE`, `CLIENTE` → `CLIENT`, `NOME` → `CONTAINER_NAME`, `CHROME_FALSO` → `FAKE_CHROME`, `COFRE_TESTE` → `TEST_VAULT`.
- **`no-test-mode.sh`**: `ALVO` → `TARGET_DIR`, `com_modo_teste` → `with_test_mode`, `conferir` → `check_strings`, `TEXTOS` → `MARKS`.
- **`app-release.sh`**: `BINARIO` → `BINARY`, `TEXTOS` → `STRINGS_FILE`.

### Grupo C (fork)

- **`terminal-test.py`**: `PRAZO`/`MARCA` → `TIMEOUT`/`MARK`, `sair` → `die`, `registrar` → `record`, `mestre`/`escravo` → `master`/`slave`, `varrer_proc` → `scan_proc` …
  - Entrou o helper `yes_no(flag)` (devolve `"sim"`/`"nao"`), que tira os literais de dentro das f-strings. A linha de log é a mesma (precedente: `reply_kind`, Fase 45).
- **`pg_select.py`**: `Conexao` → `Connection`, `falhar` → `fail`, e os nomes do SCRAM (`salt`, `salted`, `client_key`, `stored_key`, `proof`, `signature`).
- **`fake_crx.py`**: só variáveis. As opções `--adulterado`/`--versao` ficam, porque o `install.sh` as passa.
- **`fake_v6.py`**: `PORTA`/`SAIDA` → `PORT`/`OUTPUT`, `Pagina` → `Page`, `Servidor` → `Server`.
- **`apport_list.py`**: o uso imprimia `__doc__`. Agora imprime a constante `USAGE`, com o texto pt-BR igual (conferido por AST). O rótulo ignorado/não ignorado saiu da f-string para `verdict`.
- **`app-dist.py`**: `SO_NO_TESTE` → `TEST_ONLY`, `conferir` → `check`.
- **`log-scan.py`**: `varrer` → `scan`, `autoteste` → `self_test`, `limpo`/`sujo` → `clean`/`dirty`.
- **`bidi_icu.py`**: `EXIBICAO` → `DISPLAY_RS`, `faixas_js`/`faixas_rust` → `js_ranges`/`rust_ranges`.
- **`bidi.mjs`**: `igual` → `equal`, `falhas` → `failures`. As letras hebraicas do comentário viraram escapes.

### Conferências

**Textos procurados em fontes e saídas, todos casam com o master:**
- `FIXED_VARIABLES` e as constantes do `launcher.rs` (`gst-channels.sh`);
- `const RTL` e `bidiParts` do `bidi.js`, e `pub fn invisible` do `display.rs` (`bidi_icu.py`);
- `papel extensão de teste` (`servidor.rs`) e `login acme-wp-prod: formulário enviado` (`login.rs`);
- `^ward-ponte-teste:` (log da ponte);
- as marcas de `test-mode` do `no-test-mode.sh`;
- os caminhos `/opt/teste/...` dos `inside.sh`.

**Outros ajustes:**
- `allow.txt`: entrou `dev/tests/gst-channels.sh frei` (frei0r é nome de plugin, como no `launcher.rs`).
- Glossário: entrou um parágrafo da Fase 46 na seção 4, com o contrato e os nomes recorrentes.

### Literais alterados (PRD regra 5; todos são nomes internos do próprio script)

- **Pastas temporárias do socket:**
  - `ward-socket-ataques-*` → `ward-socket-attacks-*`
  - `ward-nm-teste-*` → `ward-nm-test-*`
  - `ward-extensao-teste-*` → `ward-extension-test-*`
  - `ward-login-teste-*` → `ward-login-test-*`
  - Os scripts Python recebem esse caminho por argumento.
- **Cópia do cofre de teste:** `.dev/e2e-guarda` → `.dev/e2e-backup`, `.dev/selecao-guarda` → `.dev/selection-backup`.
- **`gst-channels.sh`:**
  - nomes das iscas (`*marca*.so` → `*mark*.so`, `urn:ward:mark`) e o C gerado (`mark()`, `MARKS`, `LABEL`);
  - subpastas `casa`/`marcas` → `home`/`marks`;
  - rótulos `gst-sem`/`gst-com`/`egl-sem`/`egl-com` → `*-without`/`*-with`;
  - `.saida` → `.out`.
- **`home-decoys.sh`:** `$TMP/abertas`/`fase` → `opened`/`stage`, e os rótulos de etapa `inicio`/`conferir`/`ataques`/`controle` → `start`/`verify`/`attacks`/`control`, com o `grep -qx` junto. As mensagens "controle: …" não mudaram.
- **`ssh-home.sh`:** `$TRAB/envolver` → `$WORK_DIR/wrapper`.
- **`chrome-policy.sh`:** staging `$TRAB/raiz` → `$WORK_DIR/root`.
- **`units.sh`:** `$TMP/controle` → `control`, e no controle negativo `LimitCORE=nao-e-numero`/`DiretivaInventada` → `not-a-number`/`InventedDirective`, com o `grep` junto.
- **`no-test-mode.sh`:** `$TMP/arvore`/`forcado`/`textos` → `tree`/`forced`/`strings.txt`.
- **`log-scan.py --self-test`:** `limpo.log`/`sujo.log` → `clean.log`/`dirty.log`.
- **`real-home.sh`:** os rótulos `verify "ssh_senha …"`, `"ssh_chave …"`, `"ssh_interno …"` e a mensagem `mutação não casou com OPCOES_SSH` citam o nome da função ou variável e passaram aos nomes novos.

## Segurança

**Invariantes tocadas:**
- **R19/R13, Fase 12a/12b** (`common.sh`, `env.sh`, `real-home.sh`):
  - o diff do `common.sh`/`env.sh` é só de nomes e comentários;
  - as opções do ssh (`IdentityFile=none`, `CertificateFile=/dev/null`, `-F /dev/null` …), o `env -i` do `client`, o `-q` do curl, o `DOCKER_CONFIG` vazio, a limpeza de `DOCKER_*`/`COMPOSE_*`/`GIT_*` e os `GIT_CONFIG_*` fixos são os mesmos;
  - as mutações do `real-home.sh` andam pelo array `SSH_OPTIONS` e devem acusar as mesmas proteções.
- **S2/S15**:
  - senhas dos serviços SSH continuam pelo stdin (`chpasswd`), e as do WordPress por `--prompt`;
  - `sshpass -f` sempre;
  - `PGPASSWORD` pelo ambiente, sem valor em argv.
- **S1/S5, D47, V5.27, V6.x** (testes do grupo A):
  - continuam iguais: `env -i` do `daemon.sh` com a mesma lista, conferência do pid pelo `cmdline`, `umask`/0700 e varredura dos logs com `.dev/variations.txt`;
  - as variáveis renomeadas são atribuídas antes do uso, então nada herdado do ambiente entra.
- **V5.32/D64** (`gst-channels.sh`): o mesmo controle positivo, as mesmas iscas e o mesmo leitor do `launcher.rs`.
- **`pg_select.py`**: o SCRAM e o `hmac.compare_digest` são iguais por AST.

**Testes de ataque:** nenhum novo, porque a fase só renomeia. Os existentes foram escritos de novo e não rodados (PRD regra 6), e rodam no M6:
- `make dev-services-verify`, `test-attacks-services`, `test-real-home`, `test-home-decoys`;
- `test-socket-attacks`, `test-nm`, `test-extension`, `test-login`, `test-playwright`, `test-chrome-policy`;
- `test-docker-platform`, `test-docker-uid`, `test-units`, `test-no-test-mode`;
- `test-clipboard`, `test-revealed-selection`, `test-app-e2e`, `test-gst-channels`, `test-app-logs`, `app-front`.

**Dependências novas:** nenhuma.

## Ficou de fora

- **`english-check` em `dev/`: 64 achados restantes, nenhum é identificador desta fase** (`dev/fixtures/entries.json` e `dev/gitleaks-*.toml` à parte):
  - **Fase 56:** `WARD_HOME_COBERTO`, `WARD_ISCAS_DENTRO`, `WARD_SSH_HOME_DENTRO`, `WARD_SSH_HOME_TRAB`, `WARD_DEV_EXTENSAO`, `WARD_DEV_DBUS_SISTEMA`.
  - **Comentários que citam nomes congelados ou de outras fases:**
    - operações e erros do fio: `abrir_app`, `preencher`, `sugerir`, `nao_autorizado`;
    - subcomandos `ward lista`/`ward travar`;
    - o binário `wardd-aprovador-teste`, `aprovador.modo` e `modo-teste`;
    - `ward-teste`, `/tmp/sentinela-ssh`, `/cadastro`;
    - as mensagens "domínio não confere" e "papel app recusado";
    - o teste `configuracao_tauri`;
    - os `iscas.sh` da auditoria.
  - Somem quando as fases donas renomearem, ou na Fase 60. Não pus nada no `frozen.txt`: a Fase 45 também não pôs, e uma entrada sem uso quebraria as fases paralelas.
- **Fase 48:** `dev/fixtures/entries.json`.
- **Fase 47:** `dev/gitleaks-*.toml`.
- **Literais compartilhados que ficaram:**
  - `terminal-teste.*` e os modos (`terminal-test.py` × `app_e2e.py`/`app-e2e.sh`);
  - `aprovador.modo`/`confirmar` (lido pelo `crates/wardd/aprovador-teste`);
  - `--adulterado`/`--versao` do `fake_crx.py` (passadas pelo `install.sh`);
  - os cenários e as chaves JSON entre `chrome-policy.sh` e `chrome_policy.py`;
  - `tracos` (`ssh-home.sh` × `ssh_home.py`);
  - `ward-teste`, `ward-teste-e2e`, `ward-teste-selecao` (pasta do socket, Fase 56);
  - os caminhos internos dos contêineres e dos serviços SSH (`/etc/ssh/autorizadas`, `senha-*`, `autorizada.pub`, `ward-ssh-entrada`, `/srv/sftp/arquivos`);
  - o estado `ward-estado 1` (Fase 49).
- **Suítes e Docker:** ficam para o Milestone 6.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 5efa7f5` (árvore limpa) | 0 |
| `bash -n` / `sh -n` dos shell alterados, `py_compile` dos Python, `node --check dev/tests/bidi.mjs` | 0 |
| conferência por AST dos Python (forks; scripts em `fase46-c/` e `fase46-astcmp.py` no scratchpad) | 0 diferenças de lógica (só as três extrações listadas) |
| `python3 -I -B dev/english_check.py --paths dev` | 1 (64 achados, todos de outras fases ou literais congelados; ver acima) |
| `python3 -I -B dev/english_check.py --paths dev/services dev/env.sh` e nos arquivos do grupo D | 0 |
| `make compile` (log em `.dev/fase46/fase46-make-compile.log`) | 0 (origens, `cargo check` do workspace e do `app/src-tauri`, Vite, sintaxe de 48 shell, 35 Python e 7 JS) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- **Conflitos previsíveis de merge:**
  - A Fase 47 mexe em `install/` e nos comentários do Makefile. Daqui, o Makefile não mudou. O `deb.sh` e o `install.sh` daqui ainda procuram textos de `install/` (mensagens `modo-teste`, `ID_EXTENSAO` …), que conferi contra o master.
  - A Fase 12 (trilha R) não toca em `dev/`.
- **Fase 56:** ao trocar `WARD_HOME_COBERTO`, `WARD_ISCAS_DENTRO`, `WARD_SSH_HOME_*`, `WARD_DEV_EXTENSAO`, `WARD_DEV_DBUS_SISTEMA`, `ward-teste*` e `wardd-aprovador-teste`, ajustar os scripts desta fase. Ali estão o `daemon.sh`, o `bridge.sh` (prefixo `ward-ponte-teste:`) e os `*.sh` de `dev/tests/`.
- **Fase 34:** o comentário do `clipboard.sh` já não cita `area_gtk::testes::sessao_real`/`Copiador`. O `clipboard.py` ainda roda esse teste pelo nome, e quem o renomear tem de ajustar o `clipboard.py`.
- **`daemon.sh` × `install/`:** o `daemon.sh` grava `ward-estado 1 / vista 0 / queimada 0` quando falta o estado. Na Fase 49 (estado v2) o script muda junto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/env.sh`, `dev/services/common.sh`, `dev/services/compose.sh`, `dev/services/verify.sh` | alterado |
| `dev/services/ssh/entrypoint.sh`, `dev/services/ssh/Dockerfile`, `dev/services/wordpress/install.sh` | alterado |
| `dev/daemon.sh`, `dev/bridge.sh`, `dev/open-app.py` | alterado |
| `dev/tests/{app-e2e,app-release,chrome-policy,clipboard,docker-platform,docker-uid,extension,gst-channels,home-decoys,login,nm,no-test-mode,playwright,revealed-selection,socket-attacks,ssh-home,units}.sh` | alterado |
| `dev/tests/{attacks-foundation,attacks-services,deb,dev-vault,install,real-home}.sh` (usos do contrato) | alterado |
| `dev/tests/{terminal-test,pg_select,bidi_icu,log-scan,app-dist,apport_list,fake_v6,fake_crx}.py`, `dev/tests/bidi.mjs` | alterado |
| `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-46.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram o `make compile` (0), o `english-check` e as conferências de sintaxe e AST.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon, nenhum Docker).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem iguais, conferidos por AST e por diff.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
