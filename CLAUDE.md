# Ward

Ward: cofre de credenciais offline, só de trabalho: daemon `wardd` + CLI `ward` em Rust,
app desktop em Tauri + Svelte e extensão do Chrome. Entrega credenciais ao Claude
sem que ele veja segredo. **Segurança é o requisito número um.**

- Escopo e decisões: `docs/escopo.md` (fonte de verdade).
- Missão: `/mission` (orchestrator em `.claude/skills/mission/`, agentes em
  `.claude/agents/`). Estado em `docs/mission/plan.md`.

## Regras

- Nunca toque no cofre real (`~/.local/share/ward/`), no KeePassXC, no `~/.ssh` ou
  no `~/.aws` do usuário. Desenvolvimento e testes usam só o cofre de teste em
  `.dev/` e credenciais falsas.
- Nenhum segredo real em código, teste, fixture, log, handoff ou commit.
- Cripto só por bibliotecas auditadas; nada feito à mão. Invariantes S1–S15 em
  `docs/escopo.md` › "Segurança".
- Não commite sem o usuário pedir.

## Comandos

Rodar da raiz do repositório. O Makefile fixa o `PATH` em `/usr/bin:/bin` mais
`~/.cargo/bin` (do home do passwd) e chama cargo, `cargo-audit` e `cargo-deny` pelo
caminho absoluto; nem o ambiente, nem `MAKEFLAGS`, nem `make VAR=…` trocam essas
ferramentas (`override`). Toolchain fixa em `rust-toolchain.toml`.

| O quê | Comando | Situação |
| --- | --- | --- |
| Instalar ferramentas (uma vez): `cargo-audit`, `cargo-deny` e gitleaks com sha256 conferido em `.dev/ferramentas/` | `make ferramentas` | pronto |
| Build | `make build` (`cargo build --workspace --locked`) | pronto |
| Formatar | `make fmt` | pronto |
| Checagem completa: pré-checagem `0/6` `dev/origens.py` (lista de permitidas: nenhum `.cargo/config*` no repositório nem nos pais, só chaves inofensivas no `~/.cargo/config.toml`, nenhuma variável `CARGO_*`/`RUSTC*`/`RUSTFLAGS*`/`RUSTDOC*`/`RUSTUP*` fora da lista, toolchain só do `rust-toolchain.toml`, sem `[patch]`/`[replace]`, só crates.io e path dentro do repositório), depois `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo audit`, `cargo deny check`, gitleaks (árvore, índice e histórico, mais a varredura extra de chave privada e AWS sem allowlist de caminho; binário conferido por sha256), nessa ordem. Todo alvo que chama o cargo roda a pré-checagem antes | `make check` | pronto |
| Só a pré-checagem das origens | `make origens` | pronto |
| Testes de ataque da fundação (V1.2, V1.3, V1.5) | `make teste-ataques-fundacao` | pronto |
| Chaves e senhas falsas dos serviços em `.dev/servicos/` (0700), lidas de `dev/fixtures/entradas.json` | `make dev-servicos` | pronto |
| Subir os serviços de teste (Postgres, SSH, WordPress, phish, totp, redir), só em 127.0.0.1; precisa de `make dev-servicos` antes | `make dev-servicos-subir` (ou `dev/servicos/compose.sh up -d --build`) | pronto |
| Parar os serviços de teste | `make dev-servicos-parar` (ou `dev/servicos/compose.sh down -v`) | pronto |
| Conferir os serviços (V1.4) | `make dev-servicos-conferir` | pronto |
| Estado dos serviços de teste | `make dev-servicos-estado` | pronto |
| Testes de ataque dos serviços (com os serviços de pé); começa pelo `teste-home-real` e termina com o `teste-iscas-home` | `make teste-ataques-servicos` | pronto |
| `conferir.sh` e `ataques-servicos.sh` inteiros num bwrap com iscas FIFO no home (chaves, gitconfig, docker, AWS, cofre): falha se alguma for aberta | `make teste-iscas-home` | pronto |
| Clientes de teste (ssh, sshpass, ssh-keygen, curl) sob strace, com o home coberto por tmpfs (bwrap): falha se algum abrir caminho do home do passwd; precisa de `bwrap` e `strace` | `make teste-home-real` | pronto |
| Trocar a chave de host de um SSH de teste (V4.28) | `make dev-servicos-trocar-chave-host SERVICO=ssh-senha` | pronto |
| Ler o que o phishing (8082) recebeu | `dev/servicos/compose.sh exec phish cat /registro/posts.log` | pronto |
| Criar (ou recriar) o cofre de teste `.dev/cofre-teste.ward` (0600) com as 22 entradas de `dev/fixtures/entradas.json` (D15 + `acme-totp-dev` de D28 + `delta-api-simbolos` de D59) e o corpo v3 com a geração 1 (D39) e a configuração de D30 (hash Argon2id do PIN de teste, trava 240 min, backup desligado), reaberto e conferido no fim; roda `make dev-servicos` antes (chave do deploy). `COFRE=.dev/<nome>.ward` troca o destino; fora de `.dev/` é recusado | `make dev-cofre` | pronto |
| Só abrir o cofre de teste com a senha de teste e conferir as entradas (lista nome, tipo e nível) e a configuração (o PIN de teste confere no hash) | `make dev-cofre-conferir` | pronto |
| Lista de segredos e variações para os validadores, em `.dev/variacoes.txt` (0600; um padrão por linha, nenhum com menos de 6 bytes): a tabela única de D54, a mesma da máscara — segredo inteiro (literal, URL, base64, miolos e as formas de texto), Basic, por linha (cada linha de 6+ sem `\r` e aparada, com as formas de texto, sem base64; campos de tab) e valor de linha (`CHAVE=valor`, `"chave": "valor"`, `chave: valor`, `chave : valor`, campo de tab) como segredo inteiro; formas de texto: JSON padrão e com `\/`, Go, `ensure_ascii`, PHP, aspas duplas, aspas simples e `printf %q` (D11, D36, D37, D54); oráculo independente da máscara, testado por `dev/testes/variacoes_teste.py` e pelo teste cruzado oráculo → máscara do `dev/testes/dev-cofre.sh` (V2.57); nunca imprime a lista. `VARIACOES=.dev/<nome>.txt` troca o destino | `make dev-variacoes`, depois `grep -F -f .dev/variacoes.txt <saída>` | pronto |
| Testes de V2.29, V2.52 e ataques ao `dev-cofre`/`dev-variacoes` (saída fora de `.dev/`, links, injeção, senha errada, nada em claro). Uma rodada por vez: trava (`flock`) em `.dev/.teste-dev-cofre.trava`; a 2ª espera a 1ª (até 15 min) | `make teste-dev-cofre` | pronto |
| Interface do app (Fase 29): `npm ci --ignore-scripts`, build de produção (`app/dist`) e de teste (`app/dist-teste`, com a ponte e a vitrine), `dev/testes/app-dist.py` (sem URL remota, sem inline, sem código de teste em produção) e `npm audit`. Node do nvm; troque com `NODE_DIR=…/bin` | `make app-front` | pronto |
| App Tauri inteiro: `app-front` + fmt, clippy, testes de ACL, audit, deny (`app/src-tauri/deny.toml`) e build de release com a checagem de strings (V5.6). Precisa de `libwebkit2gtk-4.1-dev` (ou `PKG_CONFIG_PATH` para os .pc) | `make app-check` | pronto |
| Ponte de teste (D7) pelo daemon de teste (D63, Fase 23a): sobe o daemon de teste com `WARD_DEV_APP` = a ponte, pede `abrir_app` (`dev/abrir-app.py`) e imprime a URL com o token, lida de `.dev/logs/ward-ponte-teste.log` (0600). A ponte é filha do daemon, confere o papel app na primeira resposta e sai quando o daemon para; executada à mão, fica agente e sai. Se o daemon já estiver no ar com outro `WARD_DEV_APP`, pede `make dev-daemon-parar`. Precisa de `make app-front` antes | `make app-ponte` | pronto |
| D63/V3.37: nenhum membro liga `modo-teste` do `wardd`/`ward-plataforma` por dependência normal (`cargo tree`), testes de unidade sem a feature e `strings` do release sem `WARD_DEV_APP` (em `target/sem-modo-teste`); fora do `make check` | `make teste-sem-modo-teste` | pronto |
| V3.3 e V3.4 pelo socket (Fase 24a): sobe um `wardd` de teste só dele (cópia do cofre de teste em `.dev/socket-ataques.*`, socket numa pasta própria de `$XDG_RUNTIME_DIR` por `WARD_DEV_SOCKET_DIR`; nunca toca no daemon de `make dev-daemon`); quadros de 2 MiB, tamanho negativo, JSON inválido, operação desconhecida, quadro cortado e 10 000 conexões caladas (`dev/testes/socket_ataque.py`), matriz operação × entrada × campo como agente sem padrão de `.dev/variacoes.txt` e app/extensão `nao_autorizado` (`dev/testes/socket_matriz.py`), log `trace` sem segredo. Precisa de `make dev-cofre` e `make dev-variacoes`; fora do `make check` (a versão curta, 70 conexões, está no `cargo test`); `CONEXOES=<n>` troca as 10 000 | `make teste-socket-ataques` | pronto |
| D46/D47 num contêiner `ubuntu:24.04` sem rede (root só dentro dele): grupos `ward-app`/`ward-nm`, binários setgid 2755, `/run/ward/1000` root:root 0755 e socket do root entregue por `LISTEN_FDS`; papel pelo egid, `ptrace` e `LD_PRELOAD` contra o setgid, servidor falso do mesmo uid, `bind` em `/run/ward/1000` (V3.5, V3.23, V3.24); lançador do daemon e partida do app de D57 com o app de teste `partida-conteiner` (ambiente montado do zero, gids, dumpable, `CLOEXEC`, reexecução, ambiente forçado, pai qualquer e pai que imita o `systemd --user`, dois `wayland-<n>` e link; V5.31); 2ª parte do V4.2 (Fase 28, D65 e): `ward` e `wardd` sem `modo-teste`, `wardd` copiado e falso não-dumpable (`dev/conteiner-plataforma/falso.c`) do uid 1000, nenhum byte para os falsos, `bind` em `/run/ward/1000/` recusado; `ward-nm` do release setgid com o `chrome-falso` em `/opt/google/chrome/chrome` e o `wardd` de teste real: papel extensão, sugerir/preencher por origem, recusa em `127.0.0.1:8081`, pai que não é o Chrome, Chrome com `LD_PRELOAD`, cópia do host e mensagem grande demais (Fase 40; V6.1, V6.2). Exige Docker, gcc e `make dev-cofre`; fora do `make check` | `make teste-docker-plataforma` | pronto |
| Fase 40 (V6.1; D5, D9, D47): `wardd` de teste só dele (cópia do cofre, socket por `WARD_DEV_SOCKET_DIR`); o `ward-nm` de teste aberto à mão fica agente e nada de `.dev/variacoes.txt` sai; com o Chrome real headless (perfil, HOME e manifesto de native messaging só em `.dev/nm.*`, extensão de teste carregada pelo `--remote-debugging-pipe`) o host é filho direto do navegador, `conferir_pai` aceita esse pai e recusa outro, e o host responde pelo Chrome. Nunca toca o perfil real do Chrome. Precisa de `make dev-cofre`, `make dev-variacoes` e do Chrome em `/opt/google/chrome`; fora do `make check` | `make teste-nm` | pronto |
| V5.32 (D48, D57, D64; Fase 29b): o WebKitGTK real com o ambiente de D57 sob `strace -f`, por um harness do app sem a partida montado só em `.dev/webkit-real/` (nada que pule a partida entra no repositório); HOME falso com iscas (plugin-marca do GStreamer, código nas pastas de plugin frei0r/LADSPA/LV2/libvisual, `/dev/shm/cuda_injection_path_shm`) e listener em `/tmp/nvidia-mps/control`; lista os `open`/`connect` em pastas graváveis. Abre uma janela na sessão Wayland por ~20 s. Sai 1 se V5.32 falhar, 2 se um canal pendente no planner carregar código. Precisa de `make app-front`, `strace`, `gcc` e `libwebkit2gtk-4.1-dev` (ou `PKG_CONFIG_PATH`); `SEM_MPS=1` mostra o ataque que D64 fecha; fora do `make check` | `make teste-webkit-real` | pronto (sai 2: canais da 29b no planner) |
| Subir o daemon de teste (`wardd` com `modo-teste`; cofre `.dev/cofre-teste.ward`, estado `.dev/cofre-teste.estado`, socket em `$XDG_RUNTIME_DIR/ward-teste/wardd.sock`, log em `.dev/logs/wardd.log`, pid em `.dev/wardd.pid`). Sobe com `env -i` e só HOME, PATH, XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR e RUST_LOG (padrão `trace`, para o V4.33; só `error`/`warn`/`info`/`debug`/`trace`) (mais `WARD_DEV_DBUS_SISTEMA`, se definida: socket do barramento vigiado para o logind e o udisks2 do backup externo; sem ela o daemon de teste não liga nenhum dos dois; e `WARD_DEV_APP`, D63: binário absoluto dentro do repositório que o `abrir_app` abre com o papel app sem o grupo; padrão do `make`: a ponte de teste), então funciona com `TMPDIR`/`LD_*` na sessão (V3.30). Sem estado, cria um com geração 0 (só o de teste). Destravar com a senha de teste; `make dev-cofre` apaga o estado velho. Com o socket de teste ocupado por outro checkout, `WARD_DEV_SOCKET_DIR=/run/user/<uid>/<pasta>` sobe este com o socket ali (o `make teste-cli` aceita a mesma variável) | `make dev-daemon` | pronto |
| V3.2 (D49) num contêiner `ubuntu:24.04` sem rede: `wardd` de teste como uid 1000; uid 4242 recebe EACCES, uid 4242 com `CAP_DAC_OVERRIDE` e o root são fechados sem nenhum byte (log com `UidRecusado`), uid 1000 é atendido. Exige Docker; fora do `make check` | `make teste-docker-uid` | pronto |
| V3.21 (Fase 24): `systemd-analyze verify` das units `dev/systemd/wardd@.socket`/`wardd@.service` (com controle) e, num contêiner `ubuntu:24.04` com systemd e sem rede (CAP_SYS_ADMIN só para remontar o cgroup dele), a primeira conexão ao `wardd@1000.socket` sobe o `wardd` como uid 1000 com `LimitCORE=0`, `LimitMEMLOCK=infinity` e sem `no_new_privs`; V3.34 (Fase 22b): com o cofre de teste destravado, `VmLck` cobre o `VmRSS` (`mlockall`), e o mesmo binário com 8 MiB de memlock sai ≠ 0; V4.38 (Fase 26): com os grupos `ward-app`/`ward-nm` e cópias setgid do cliente mínimo, `ward run acme-pg-dev -- ward-app …` (CLI de release) fica agente e o journal tem "papel app recusado". Nunca instala unit no host. Exige Docker (o build da imagem usa rede) e `make dev-cofre`; fora do `make check` | `make teste-units` | pronto |
| Parar o daemon de teste (só mata o pid se o `cmdline` for o `target/debug/wardd` daqui) | `make dev-daemon-parar` | pronto |
| CLI `ward` de teste (`modo-teste`) num pty: V4.2 com servidor falso em Python no socket de teste (recusa antes do prompt, falso com 0 bytes), V4.22, destravar com senha errada e certa sem eco, V4.1 com `lista --json` sem padrão de `.dev/variacoes.txt`; `ward run` (Fase 26): V4.3–V4.8 e V4.10 por hash contra a tabela "Entrega esperada", V4.11, V4.17, V4.19, V4.36, anexo 0600 em `<rt>tmp/` apagado, entrada padrão, sem `LD_*` nem ambiente do daemon (só comandos locais, nenhum cliente ssh/curl); `ward ssh` (Fase 27) contra os SSH de teste 2222/2223: V4.23, V4.24, V4.26 (`ssh-add` de outro processo, SIGKILL na CLI, travar), V4.28 (linha falsa no `.dev/known_hosts`, restaurado no fim), V4.31/V4.34 num pty, V4.32, V4.14 e V4.35/V4.37 pelo argv em `/proc/<pid>/cmdline` (o `wardd` recusa `strace`); salto e túnel (Fase 28) contra `ssh-salto`/`ssh-interno`/Postgres: V4.25, V4.29 (cliente `dev/testes/pg_select.py`), V4.30, V4.35 com salto (nenhum `ssh` filho de outro `ssh`), ataque à ponte do túnel e V4.33 (log do daemon com `RUST_LOG=trace` sem padrão); Fase 27a (sem precisar dos serviços): token do "Conectar" só pelo socket herdado (`--token-fd`) por um terminal falso, leitor de `/proc/*/cmdline`/`environ`/`fd` em laço sem achar o token, `--token` em argv e canal forjado (lixo, vazio, grande, pipe, fechado) recusados sem eco; sem os serviços, a seção falha com aviso; precisa de `make dev-daemon`, `make dev-variacoes` e `make dev-servicos-subir` | `make teste-cli` | pronto |

Os alvos marcados com fase ainda não existem: falham com mensagem até a fase chegar.

Serviços de teste (`dev/compose.yml`): `postgres` 55432 (`app`/`app_teste`),
`ssh-senha` 2222 (`deploy` por senha, `sftpuser` só sftp, túnel só até
`postgres:5432`), `ssh-chave` 2223 (`deploy` só por chave
`.dev/servicos/chaves/acme-deploy_ed25519`, com passphrase), `ssh-salto` 2224
(`salto`, encaminha só até `ssh-interno:22`), `ssh-interno` (sem porta no host),
`wordpress` 8081 (`wp_admin`, `wp_editor`), `phish` 8082, `totp` 8083 (`/login`,
`/cadastro`), `redir` 8084 (→ 8082). Estado em tmpfs: `down` + `up` começa limpo. Os
clientes de teste usam `-F /dev/null`, `.dev/servicos/known_hosts` e HOME falso
(`dev/servicos/comum.sh`); senha sempre por `sshpass -f <arquivo>`, nunca em argv.
O ssh acha as chaves padrão pelo home do passwd, não por `$HOME`: todo cliente ssh
de teste leva `IdentityFile=none` e `CertificateFile=/dev/null`, e todo `curl` leva
`-q`. Cliente novo vai por `comum.sh` e entra no `dev/testes/home-real.sh`; nunca rode
ssh/sshpass de teste fora deles.

Ambiente dos scripts de `dev/` (Fase 12b): todo script começa com `#!/bin/bash -p`
(sem BASH_ENV nem funções do ambiente), `export PATH=/usr/bin:/bin` e
`. dev/ambiente.sh` (git com `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1` e
`core.excludesFile`/`core.attributesFile`/`core.fsmonitor`/`core.hooksPath` fixos).
O docker só por `comum.sh`/`dev/servicos/compose.sh`: `DOCKER_CONFIG` em `.dev/docker/`,
recriado vazio a cada script, e sem `DOCKER_*`/`COMPOSE_*` herdados. O compose direto
(`docker compose -f dev/compose.yml …`) funciona, mas lê o `~/.docker` do usuário.
Não crie `.cargo/config*` no repositório: o `make check` recusa.

Cofre de teste (D15), tudo falso:
- Senha mestra de teste: `falso-mestra-ward-01`
- PIN de teste: `135790`
- Entradas: tabela "Cofre de teste" de `docs/mission/validation-contract.md`.

O gitleaks (`.gitleaks.toml`) só tem allowlist dos valores falsos fixos de D15, cada
um casado inteiro; não acrescente padrão genérico. Segredo de teste novo é gerado em
`.dev/` (fora do git), nunca escrito no repositório.
