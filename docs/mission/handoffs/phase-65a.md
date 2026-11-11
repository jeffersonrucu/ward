# Fase 65a — Correção da Bateria E (guarda + contêiner)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei
  `git reset --hard ce9a14a` (master atual).
- **Preparação** (`.dev/` do worktree):
  - copiados do checkout principal: `.dev/tools` (sha256 do gitleaks conferido:
    `88f91962…`), `.dev/baseline/` (`sha256sum -c SHA256SUMS` ok), `.dev/extension/key.pem`
    (0600), `.dev/pw-lib` e `.dev/services`;
  - `.dev/services` foi **copiado, não gerado**: os serviços de pé são os do checkout
    principal (projeto `ward-teste` do compose), que montam as chaves de host e os
    segredos de lá. Chaves novas não bateriam com o `known_hosts`. O `make dev-services`
    depois da cópia só confere o que falta;
  - `make dev-services`, `make dev-vault` e `make dev-variations` (282 padrões): 0.
- **(1) Guarda** (feita primeiro, antes de abrir qualquer Chrome):
  - `dev/tests/chrome_guard.py check [--root <pasta>]` sai 3 quando o Chrome veria o Ward
    instalado:
    - em `/etc/opt/chrome/policies/{managed,recommended}` e nas do Chromium: `ward.json`,
      qualquer arquivo com o id da extensão (mesmo JSON inválido ou por link) ou com
      `NativeMessagingUserLevelHosts`;
    - em `native-messaging-hosts`: `br.ward.nm.json`, outro manifesto com o id ou que aponta
      para um programa `ward*`;
    - o que não dá para ler também recusa (fail closed).
  - Só diz o que achou e onde, nunca o conteúdo. O id sai de `crates/ward-nm/src/lib.rs`.
  - `--self-test` (alvo `make test-chrome-guard`): planta numa cópia em
    `.dev/chrome-guard.*` 19 casos. Entre eles uma cópia do `ward.json` do host, com o nome
    dele e com outro nome. A árvore limpa, a política alheia e o manifesto alheio passam, e
    os outros 16 são recusados.
  - Onde a guarda roda:
    - no começo de `nm.sh`, `extension.sh`, `login.sh`, `chrome-policy.sh` e
      `playwright.sh`;
    - antes do Chrome em `nm.py`, `extension.py` e `login.py`;
    - na 1ª linha dos alvos `test-playwright` (antes do `cargo test --ignored`, que abre
      Chrome) e `test-chrome-policy`;
    - dentro do contêiner (`inside.sh`);
    - dentro do bwrap do `chrome-isolated.sh`.
  - No host desta máquina (política 0.1.x instalada), cada um desses sai 3 antes de
    qualquer Chrome (`fase65a-chrome-guard.log`).
- **(2) Contêiner das suítes da extensão**:
  - `dev/tests/browser-container.sh <nm|extension|login> <binários>`, que os alvos
    `test-nm`, `test-extension` e `test-login` agora chamam;
  - imagem `ward-teste/browser:1` (`dev/container-browser/Dockerfile`): `ubuntu:24.04` pelo
    digest dos outros testes, `google-chrome-stable` do repositório do Google por HTTPS,
    com a chave conferida pela impressão digital `EB4C1BFD…B4796`, e python3. O build falha
    se a imagem tiver política, manifesto do sistema, `/etc/chromium` ou
    `/usr/local/bin/ward-nm`. O build usa a rede;
  - o teste roda com `--network none`, o uid/gid do usuário, `--cap-drop ALL`,
    `no-new-privileges`, `/tmp` e `/run/user/<uid>` em tmpfs, memlock sem limite (o
    `mlockall` do `wardd`), `--pids-limit 4096`, `--memory 6g` e `--shm-size 1g`;
  - montagens: o repositório só leitura, mais `.dev/`, `docs/mission/evidence/` e a pasta
    do relay em leitura e escrita. Nada do `/etc`, `/usr/local`, `/run` ou home do host;
  - `dev/tests/service_relay.py` liga o contêiner aos serviços sem rede:
    - no host, sockets Unix numa pasta 0700 de `$XDG_RUNTIME_DIR` (`ward-browser-relay.*`)
      repassam para `127.0.0.1:8081–8084`;
    - dentro, escuta só em `127.0.0.1` (IPv4; o `[::1]` fica livre para os ataques das
      suítes);
    - as duas leituras fixas que as suítes faziam com `compose.sh exec` (o `posts.log` do
      phish e o `zxcvbn.min.js` do WordPress) passam por `service_relay.py cat`, com lista
      fechada. Fora do contêiner, ele chama o `compose.sh` como antes;
    - nunca sobe, para ou recria os serviços.
  - Chrome no contêiner com `--no-sandbox`, só quando `WARD_TEST_IN_CONTAINER=1` **e** existe
    `/.dockerenv` (ver Problemas). Ele é filho direto do navegador, e o `verify_parent` aceita
    `/opt/google/chrome/chrome` (dono root) como no host.
- **(3) `test-app-e2e`**:
  - `app_e2e.py › run_in_background` com `stdin=subprocess.DEVNULL`;
  - o Chrome do Playwright passa por `dev/tests/chrome-isolated.sh`: bwrap com `/etc/opt`,
    `/usr/local` e, se existirem, `/etc/chromium` e `/run/ward` em tmpfs, com a guarda
    dentro antes de abrir o Chrome. Depois do launch, o teste exige zero alvos
    `chrome-extension://` (`Target.getTargets`) antes de qualquer página, e sai 3 se achar.
    O `chrome-isolated.sh --version` rodou: a guarda passou dentro do bwrap
    (`fase65a-chrome-isolated.log`).
  - **A suíte não rodou**: a permissão negou o comando (Problema 1).
- **(4) `test-webkit-real`**:
  - o harness troca `tray::session_icons_dir()` por `tray::icons_dir(<.dev/webkit-real/tray-runtime>, <uid>)`;
  - o teste falha se o strace mostrar `$RT/ward-tray` e exige a pasta do harness (controle
    positivo);
  - rodou com 0. A lista de `/run/user/<uid>` só tem `dconf`.
- **(5) Marca "fora de ordem"** (achados 4 da 64 e 5 da 65). Decisão: **fica como está**,
  só registrado, sem mudança no produto.
  - Lendo `ward_core::backup::out_of_order`: uma cópia de mesma geração com nome mais velho
    não é marcada, e a versão verdadeira de geração menor que vem depois dela pela data sai
    marcada.
  - Motivos para não mudar:
    - a cópia restaura exatamente uma gravação real (a mesma geração), então não há
      retrocesso;
    - a versão marcada tem mesmo geração menor do que uma versão com data anterior, ou seja,
      a pasta está incoerente, e o aviso é só aviso (S8/D39 seguem na restauração);
    - marcar os dois lados pediria um texto novo ("gravação mais nova do que a data diz"),
      e a regra 2 do PRD proíbe texto novo na missão 2.
  - Fica para o orchestrator ou o usuário reabrir numa missão futura.
- **CLAUDE.md e Makefile**:
  - linha nova `make test-chrome-guard` e linha nova `make test-app-e2e`;
  - `test-nm`, `test-extension` e `test-login` agora rodam no contêiner;
  - `test-chrome-policy` e `test-playwright` são recusados pela guarda num host com o Ward;
  - `test-webkit-real` tem a pasta da bandeja própria;
  - comentários dos alvos atualizados, e `test-chrome-guard` entrou no `.PHONY`.

### Resultado

| Suíte | Resultado | Log (`docs/mission/evidence/M6/fase-65a/`) |
| --- | --- | --- |
| `make test-chrome-guard` (autoteste) | 0 (19 casos) | `fase65a-test-chrome-guard.log`, `fase65a-chrome-guard.log` |
| guarda no host (`check`, `nm.sh`/`extension.sh`/`login.sh` diretos, `make test-playwright`, `make test-chrome-policy`) | 3 / 3 / 3 / 3 / 2 (make sobre o 3) / 2: recusados antes do Chrome | `fase65a-chrome-guard.log` |
| `make test-nm` (contêiner, Chrome 157) | **0**: 33 ok, inclusive m2:V5.10 pelo `ward-nm` e pelo Chrome real (`fill_v1`/`suggest_v1` → só `unknown_version`), D47 (`verify_parent`, filho direto, dumpable) | `fase65a-test-nm.log` |
| `make test-extension` (contêiner) | **0**: 102 ok. V6.2, V6.4 (iframe do phishing), V6.5 (8082/8084, `posts.log` igual, nada no perfil), V6.7 (8081 e 8083 com TOTP), ataque `[::1]`, Fase 42 (salvar login), 44c (janela), medidas do popup e capturas | `fase65a-test-extension.log`, `test-extension/` |
| `make test-login` (contêiner) | **0**: 48 ok. `ward login acme-wp-prod` chega ao painel (V6.6, V6.10), phishing e `delta-redir` recusam, `action`/`formaction`/GET/target/duas senhas sem envio, `[::1]`, `posts.log` igual, logs sem padrão | `fase65a-test-login.log`, `test-login/` |
| `make test-app-e2e PW_PYTHONPATH=…` | **negado** pela permissão (Problema 1) | — |
| `make test-webkit-real` | **0** (V5.32 ok; `ward-tray` do runtime não tocado) | `fase65a-test-webkit-real.log` |
| `make compile` | 0 (55 shell, 39 Python, 7 JS; 0 erro) | `fase65a-compile.log` |
| `make english-check` | 0 (0 achados) | `fase65a-english-check.log` |
| `make check` | 0 (fmt, clippy, testes, audit, deny, gitleaks: "tudo limpo") | — |

PNGs (telas da extensão no Chrome real, no contêiner):
- `docs/mission/evidence/M6/fase-65a/test-extension/sugestoes-do-dominio-localhost-8081.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/sugestoes-do-dominio-localhost-8083.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/sugestoes-do-dominio-sem-entrada-127.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/sugestoes-do-dominio-sem-campo.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/cofre-travado-localhost-8081.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/popup/salvar-login-novo-localhost-8083.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/popup/salvar-login-novo-rolado.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/popup/salvar-login-novo-erro-nome.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/popup/salvar-login-novo-tema-claro.png`
- `docs/mission/evidence/M6/fase-65a/test-extension/popup/cofre-travado-tema-claro.png`
- `docs/mission/evidence/M6/fase-65a/test-login/v6-10-painel-wordpress.png`

## Segurança

- **Invariantes tocadas**:
  - S1: nenhuma extensão de teste alcança o `ward-nm` do sistema nem o `wardd` instalado, e
    as suítes de extensão verificam o domínio, o phishing e o redir;
  - S13: os logs do daemon, do Chrome/`ward-nm`, do console, da CLI e da janela de teste
    foram varridos pelas suítes, sem padrão;
  - S8: a pasta da bandeja do app instalado não é tocada.
- **Testes de ataque escritos**:
  - `dev/tests/chrome_guard.py › self_test`: 16 formas de a política ou o manifesto do Ward
    chegarem ao Chrome, todas recusadas, e 3 controles liberados;
  - `fase65a-chrome-guard.log`: cada entrada do host recusada com a política real
    instalada;
  - `dev/tests/app_e2e.py › main`: Chrome com qualquer alvo `chrome-extension://` aborta
    antes da 1ª página (não rodou, Problema 1);
  - `dev/tests/webkit-real.sh`: falha se algum processo do app tocar
    `/run/user/<uid>/ward-tray`, com controle positivo da pasta do harness.
- **Evidência**: `grep -rlF -f .dev/variations.txt docs/mission/evidence/M6/fase-65a/` não
  acha nada (exit 1). Nos logs, o home e o caminho do worktree viraram `<home>` e `<repo>`.
- **Dependências novas**: nenhuma em Rust ou npm. A imagem de teste instala
  `google-chrome-stable`, `python3` e `ca-certificates`. `curl` e `gnupg` só entram no
  build e saem no fim. Motivo: rodar o Chrome real fora do alcance da política do host
  (decisão do usuário).
- **Não tocados**: o cofre real, o `wardd` instalado, `/etc` (a política só foi lida),
  `/usr/local`, o perfil real do Chrome, `~/.config/google-chrome/`, `~/.ssh` e `~/.aws`.
  Os serviços de teste e o daemon de teste do checkout principal não foram parados nem
  recriados. Nenhum `sudo`.

## Ficou de fora

- `make test-app-e2e`: a chamada foi negada pela permissão (Problema 1). A correção do
  `stdin` e o Chrome isolado estão prontos, mas não foram conferidos de ponta a ponta.
- `test-chrome-policy` e `test-playwright` agora são recusados nesta máquina (pedido da
  fase). Rodá-los aqui pediria um contêiner próprio, o que não estava nesta fase
  (Problema 3).
- `extension-crx` (Chrome `--pack-extension`) ficou sem guarda (Problema 4).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard ce9a14a650f6bacd876c19d137fbeb72de2c1ad2` | 0 |
| cópia de `.dev/{tools,baseline,services,pw-lib,extension/key.pem,known_hosts}` do checkout principal; `sha256sum -c SHA256SUMS` | 0 |
| `make dev-services && make dev-vault && make dev-variations` | 0 |
| `python3 -I -B dev/tests/chrome_guard.py --self-test` / `check` (host) | 0 / 3 |
| `systemd-run --user --scope -p TasksMax=1000 make app-front` | 0 |
| `make extension` | 0 |
| `docker build -t ward-teste/browser:1 dev/container-browser` | 0 |
| `systemd-run … -p TasksMax=1500 make test-nm` | 0 |
| `systemd-run … -p TasksMax=1500 make test-extension` | 0 |
| `systemd-run … -p TasksMax=1500 make test-login` | 0 |
| roteiro da guarda no host (`fase65a-chrome-guard.log`) | ver tabela acima |
| `ls .dev/pw-lib/; ls /run/user/1000 \| grep -i ward; systemd-run --user --scope -q -p TasksMax=1500 make test-app-e2e PW_PYTHONPATH=$PWD/.dev/pw-lib/site > docs/mission/evidence/M6/fase-65a/fase65a-test-app-e2e.log 2>&1 < /dev/null; …` | **negado** pela permissão; não insisti |
| `systemd-run … -p TasksMax=1500 make test-webkit-real` | 0 |
| `dev/tests/chrome-isolated.sh --version` | 0 |
| laço `sed -i` para trocar caminhos nos logs + `grep` | **negado**; troquei os caminhos com a ferramenta de edição e rodei o `grep -rlF -f .dev/variations.txt` sozinho (exit 1: nada) |
| `make english-check` | 0 |
| `make test-chrome-guard` | 0 |
| `systemd-run … -p TasksMax=1000 make compile` | 0 |
| `systemd-run … -p TasksMax=1500 make check` | 0 |
| `rm -rf target app/src-tauri/target app/node_modules .dev/pw-lib .dev/docker` | 0 |

`df -h /`: 12 GB livres no começo, 4–6 GB no meio (Rust, imagem de 1,34 GB e o harness do
WebKit, apagado depois). No fim, depois da limpeza: 7,9 GB. Nenhum contêiner, Chrome, daemon ou relay meu ficou de pé. A imagem `ward-teste/browser:1` (1,34 GB) ficou no Docker, como as outras `ward-teste/*:1`; a primeira montagem, de antes de trocar o nome do ARG, já não existe.

## Problemas descobertos

1. **`make test-app-e2e` negado pela permissão** (comando exato na tabela). Para rodar,
   com os serviços de pé e nenhum daemon de teste deste worktree no ar:
   `PW_PYTHONPATH=<repo>/.dev/pw-lib/site make test-app-e2e < /dev/null`.
   O esperado é 10/10 mais o passo novo "Chrome sem nenhuma extensão".
2. **Chrome com `--no-sandbox` no contêiner**. O sandbox do Chrome precisa de user
   namespaces, e o contêiner (sem capacidades, seccomp padrão do Docker, AppArmor) não os
   dá. Abrir o seccomp ou dar `CAP_SYS_ADMIN` enfraqueceria o isolamento do host. Por isso o
   `--no-sandbox` só entra com `WARD_TEST_IN_CONTAINER=1` e `/.dockerenv`. As páginas são as
   de teste, o contêiner não tem rede e só vê o repositório (só leitura), `.dev/` e a
   evidência.
3. **`test-chrome-policy` e `test-playwright` recusados nesta máquina**:
   - o `test-chrome-policy` já escondia `/etc/opt` no bwrap, mas a fase pede a recusa;
   - o `test-playwright` usa `--disable-extensions`, mas o `ward playwright` (produto) abre
     o Chrome pelo caminho fixo, sem como isolar.
   - Para a validação final, os dois precisam de um contêiner como o da extensão ou de uma
     máquina sem o Ward. A rodada 3 do `test-chrome-policy` já era pulada aqui.
4. **`make extension-crx`** abre o Chrome (`--pack-extension`, perfil temporário) sem
   guarda. Deixei assim de propósito: a Fase 66 precisa empacotar nesta máquina, e o
   empacotamento não abre página nem fala com native messaging. Vale o orchestrator
   decidir.
5. **`test-webkit-real` sem guarda**: ele usa o WebKitGTK, não o Chrome, e não lê política
   do Chrome. A correção dele é a da bandeja (item 4).
6. **Chrome do contêiner (157) ≠ Chrome do host (156)**: o contêiner pega o stable do dia em
   que a imagem é montada. A imagem fica como `ward-teste/browser:1` e só é refeita se o
   `Dockerfile` mudar.
7. **`/run/user/<uid>/ward-tray` já existe** (de rodadas anteriores do harness, Fase 65, ou
   do app instalado). Não mexi.
8. **V6.5 (texto)**, complemento do achado 8 da Fase 65: no `test-login`, o
   `ward login` na aba do phishing (8082) sai com "domínio não confere". O
   `ward login delta-redir` também recusa, mas o teste só confere que é recusa por origem
   (sai 1), não o texto. O texto do `ward playwright` continua o da 0.1.4 (Fase 66).

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `dev/tests/chrome_guard.py` | criado |
| `dev/tests/service_relay.py` | criado |
| `dev/tests/browser-container.sh` | criado |
| `dev/tests/chrome-isolated.sh` | criado |
| `dev/container-browser/Dockerfile` | criado |
| `dev/container-browser/inside.sh` | criado |
| `dev/tests/nm.sh`, `extension.sh`, `login.sh`, `chrome-policy.sh`, `playwright.sh` | alterados (guarda) |
| `dev/tests/nm.py`, `extension.py`, `login.py` | alterados (guarda, `--no-sandbox` só no contêiner, leituras pelo relay) |
| `dev/tests/app_e2e.py` | alterado (`stdin=DEVNULL`, Chrome isolado, sem alvo de extensão) |
| `dev/tests/webkit-real.sh` | alterado (pasta da bandeja do harness, teste do `ward-tray`) |
| `Makefile` | alterado (três alvos no contêiner, guarda em `test-playwright`/`test-chrome-policy`, `test-chrome-guard`, comentários) |
| `CLAUDE.md` | alterado (linhas dos alvos) |
| `docs/mission/evidence/M6/fase-65a/` | criado (logs `fase65a-*`, `test-extension/`, `test-login/`) |
| `docs/mission/handoffs/phase-65a.md` | criado |

## Procedimentos
- [x] `make check` rodado (0, tudo limpo)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (guarda com autoteste, `ward-tray` no strace, alvo de extensão no e2e)
- [x] (UI) Conferido: as medidas e capturas do popup são conferidas pelo `test-extension` contra os frames 17332/17392
- [x] Ficou dentro do escopo da fase
