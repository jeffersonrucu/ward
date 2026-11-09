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
| Criar (ou recriar) o cofre de teste `.dev/cofre-teste.ward` (0600) com as 21 entradas de `dev/fixtures/entradas.json` (D15 + `acme-totp-dev` de D28) e o corpo v3 com a geração 1 (D39) e a configuração de D30 (hash Argon2id do PIN de teste, trava 240 min, backup desligado), reaberto e conferido no fim; roda `make dev-servicos` antes (chave do deploy). `COFRE=.dev/<nome>.ward` troca o destino; fora de `.dev/` é recusado | `make dev-cofre` | pronto |
| Só abrir o cofre de teste com a senha de teste e conferir as entradas (lista nome, tipo e nível) e a configuração (o PIN de teste confere no hash) | `make dev-cofre-conferir` | pronto |
| Lista de segredos e variações de D11 para os validadores, em `.dev/variacoes.txt` (0600; um padrão por linha, nenhum com menos de 6 bytes); nunca imprime a lista. `VARIACOES=.dev/<nome>.txt` troca o destino | `make dev-variacoes`, depois `grep -F -f .dev/variacoes.txt <saída>` | pronto |
| Testes de V2.29 e ataques ao `dev-cofre`/`dev-variacoes` (saída fora de `.dev/`, links, injeção, senha errada, nada em claro) | `make teste-dev-cofre` | pronto |
| Interface do app (Fase 29): `npm ci --ignore-scripts`, build de produção (`app/dist`) e de teste (`app/dist-teste`, com a ponte e a vitrine), `dev/testes/app-dist.py` (sem URL remota, sem inline, sem código de teste em produção) e `npm audit`. Node do nvm; troque com `NODE_DIR=…/bin` | `make app-front` | pronto |
| App Tauri inteiro: `app-front` + fmt, clippy, testes de ACL, audit, deny (`app/src-tauri/deny.toml`) e build de release com a checagem de strings (V5.6). Precisa de `libwebkit2gtk-4.1-dev` (ou `PKG_CONFIG_PATH` para os .pc) | `make app-check` | pronto |
| Ponte de teste (D7): serve `app/dist-teste` em 127.0.0.1 e imprime a URL com o token para o Chrome de teste; precisa de `make app-front` antes | `make app-ponte` | pronto (responde "daemon não está rodando" até o Milestone 3) |
| Subir o daemon de teste (cofre de `.dev/`, socket em `/run/user/<uid>/ward-teste/`) | `make dev-daemon` | Milestone 3 |
| Parar o daemon de teste | `make dev-daemon-parar` | Milestone 3 |

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
