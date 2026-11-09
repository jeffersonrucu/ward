# Ward — comandos de desenvolvimento. Rodar sempre da raiz do repositório.
#
# Regra (V1.5, S15): nenhum alvo daqui lê ou escreve fora do repositório, a não ser
# as ferramentas de build (~/.cargo). Dados de desenvolvimento só em `.dev/`.

# `bash -p`: não lê BASH_ENV/ENV nem importa funções exportadas do ambiente.
SHELL := /bin/bash
.SHELLFLAGS := -p -eu -o pipefail -c
.DEFAULT_GOAL := check
MAKEFLAGS += --no-print-directory

# Ambiente fixo (Fase 12b). `override`: nem o ambiente, nem `MAKEFLAGS='CARGO=…'`
# herdado, nem `make VAR=…` trocam ferramenta, PATH ou config do git.
#
# Home do passwd, não $HOME (um HOME=/tmp/x trocaria o cargo e o CARGO_HOME).
override CASA := $(shell /usr/bin/getent passwd "$$(/usr/bin/id -u)" | /usr/bin/cut -d: -f6)
ifeq ($(strip $(CASA)),)
$(error home do passwd indefinido)
endif
# PATH só do sistema; ~/.cargo/bin no fim, para o rustup (não há cargo em /usr/bin).
override PATH := /usr/bin:/bin:$(CASA)/.cargo/bin
export PATH
unexport BASH_ENV ENV CDPATH GLOBIGNORE
# Ferramentas pelo caminho absoluto: `cargo audit`/`cargo deny` passariam pelo
# `[alias]` de um `.cargo/config.toml`; o binário direto, não.
override CARGO := $(CASA)/.cargo/bin/cargo
override CARGO_AUDIT := $(CASA)/.cargo/bin/cargo-audit
override CARGO_DENY := $(CASA)/.cargo/bin/cargo-deny
override PYTHON := /usr/bin/python3 -I -B
override CARGO_HOME := $(CASA)/.cargo
override RUSTUP_HOME := $(CASA)/.rustup
export CARGO CARGO_HOME RUSTUP_HOME
# git (também o que o gitleaks chama): sem config global nem do sistema, e com
# precedência de linha de comando sobre o `.git/config`: sem ignore/attributes globais,
# sem fsmonitor (roda programa a cada `git diff`) e sem hooks. Ver dev/ambiente.sh.
override GIT_CONFIG_GLOBAL := /dev/null
override GIT_CONFIG_NOSYSTEM := 1
override GIT_CONFIG_COUNT := 4
override GIT_CONFIG_KEY_0 := core.excludesFile
override GIT_CONFIG_VALUE_0 := /dev/null
override GIT_CONFIG_KEY_1 := core.attributesFile
override GIT_CONFIG_VALUE_1 := /dev/null
override GIT_CONFIG_KEY_2 := core.fsmonitor
override GIT_CONFIG_VALUE_2 := false
override GIT_CONFIG_KEY_3 := core.hooksPath
override GIT_CONFIG_VALUE_3 := /dev/null
export GIT_CONFIG_GLOBAL GIT_CONFIG_NOSYSTEM GIT_CONFIG_COUNT GIT_CONFIG_KEY_0 GIT_CONFIG_VALUE_0 \
	GIT_CONFIG_KEY_1 GIT_CONFIG_VALUE_1 GIT_CONFIG_KEY_2 GIT_CONFIG_VALUE_2 GIT_CONFIG_KEY_3 GIT_CONFIG_VALUE_3
unexport GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
	GIT_EXEC_PATH GIT_SSH GIT_SSH_COMMAND GIT_ASKPASS GIT_EXTERNAL_DIFF GIT_PAGER GIT_CONFIG \
	GIT_CONFIG_PARAMETERS GIT_TEMPLATE_DIR GIT_CEILING_DIRECTORIES GIT_DISCOVERY_ACROSS_FILESYSTEM

# Ferramentas com versão fixa (`make ferramentas`). O gitleaks fica dentro de `.dev/`
# (fora do git): o tar.gz é conferido por sha256 antes de extrair e o binário extraído,
# por sha256, antes de cada varredura (um binário trocado em `.dev/` não roda).
override FERRAMENTAS_DIR := $(CURDIR)/.dev/ferramentas
override GITLEAKS := $(FERRAMENTAS_DIR)/bin/gitleaks
GITLEAKS_VERSAO := 8.30.1
override GITLEAKS_SHA256 := 551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
override GITLEAKS_BIN_SHA256 := 88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509
GITLEAKS_URL := https://github.com/gitleaks/gitleaks/releases/download/v$(GITLEAKS_VERSAO)/gitleaks_$(GITLEAKS_VERSAO)_linux_x64.tar.gz
CARGO_AUDIT_VERSAO := 0.22.2
CARGO_DENY_VERSAO := 0.20.2

.PHONY: check build fmt fmt-check clippy test audit deny gitleaks ferramentas \
	origens gitleaks-binario teste-ataques-fundacao teste-ataques-servicos teste-iscas-home teste-home-real dev-servicos dev-servicos-subir \
	dev-servicos-parar dev-servicos-estado dev-servicos-conferir dev-servicos-trocar-chave-host dev-cofre \
	dev-cofre-conferir dev-variacoes teste-dev-cofre dev-daemon dev-daemon-parar teste-cli teste-docker-uid teste-units \
	app-front app-check app-ponte teste-docker-plataforma

## check: pré-checagem das origens, depois fmt, clippy -D warnings, testes, cargo audit,
## cargo deny e gitleaks, nessa ordem (V1.1)
check:
	@echo "==> 0/6 origens do cargo (pré-checagem: configs, ambiente, toolchain, fontes)"
	@$(MAKE) origens
	@echo "==> 1/6 cargo fmt --check"
	@$(MAKE) fmt-check
	@echo "==> 2/6 cargo clippy --all-targets -- -D warnings"
	@$(MAKE) clippy
	@echo "==> 3/6 cargo test"
	@$(MAKE) test
	@echo "==> 4/6 cargo audit"
	@$(MAKE) audit
	@echo "==> 5/6 cargo deny check"
	@$(MAKE) deny
	@echo "==> 6/6 gitleaks"
	@$(MAKE) gitleaks
	@echo "==> make check: tudo limpo"

# Todo alvo que chama o cargo depende de `origens`: nem `make clippy` sozinho roda com
# `.cargo/config*`, RUSTC_WRAPPER etc. plantados.
build: origens
	$(CARGO) build --workspace --locked

fmt: origens
	$(CARGO) fmt --all

fmt-check: origens
	$(CARGO) fmt --all --check

clippy: origens
	$(CARGO) clippy --workspace --all-targets --all-features --locked -- -D warnings

test: origens
	$(CARGO) test --workspace --all-features --locked

# --deny warnings: crate sem manutenção, retirado (yanked) ou com aviso também falha.
audit: origens
	$(CARGO_AUDIT) audit --deny warnings

deny: origens
	$(CARGO_DENY) --all-features check

# `dev/origens.py` (lista de permitidas): nenhum `.cargo/config*` no repositório nem nos
# pais, só chaves inofensivas no do CARGO_HOME, nenhuma variável CARGO_*/RUSTC*/
# RUSTFLAGS*/RUSTDOCFLAGS/RUSTUP* fora da lista, toolchain só do rust-toolchain.toml,
# sem [patch]/[replace], só crates.io e paths dentro do repositório. O cargo deny não vê
# nada disso (o lockfile continua dizendo crates.io).
origens:
	@$(PYTHON) dev/origens.py

# Três varreduras (S15):
# - árvore de trabalho, inclusive arquivos ainda não versionados; os diretórios que o
#   `.gitignore` tira do git ficam de fora (`dev/gitleaks-arvore.toml`);
# - índice (`--staged`), sem exceção de diretório: pega o que foi forçado com `git add -f`;
# - histórico inteiro dos commits.
# --verbose mostra arquivo e linha de cada achado; --redact nunca imprime o valor.
#
# Depois, a varredura extra (`dev/gitleaks-extra.toml`): só chave privada e AWS, sem a
# allowlist global de caminhos das regras padrão (lockfiles, svg, png, pdf...) e
# entrando em arquivos compactados (.gz, .zip, .tar), nas mesmas três formas.
GITLEAKS_OPCOES := --redact --verbose --no-banner --exit-code 1
GITLEAKS_EXTRA := --max-archive-depth 3

gitleaks: gitleaks-binario
	$(GITLEAKS) dir --config dev/gitleaks-arvore.toml $(GITLEAKS_OPCOES) .
	$(GITLEAKS) git --staged --pre-commit --config .gitleaks.toml $(GITLEAKS_OPCOES) .
	$(GITLEAKS) git --config .gitleaks.toml $(GITLEAKS_OPCOES) .
	$(GITLEAKS) dir --config dev/gitleaks-extra-arvore.toml $(GITLEAKS_OPCOES) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --staged --pre-commit --config dev/gitleaks-extra.toml $(GITLEAKS_OPCOES) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --config dev/gitleaks-extra.toml $(GITLEAKS_OPCOES) $(GITLEAKS_EXTRA) .

# Confere o sha256 do binário do gitleaks antes de usar (falha fechada).
gitleaks-binario:
	@test -x "$(GITLEAKS)" || { echo "gitleaks não encontrado em $(GITLEAKS); rode 'make ferramentas'" >&2; exit 1; }
	@echo "$(GITLEAKS_BIN_SHA256)  $(GITLEAKS)" | sha256sum --check --quiet --strict || { echo "o binário $(GITLEAKS) não é o gitleaks $(GITLEAKS_VERSAO) conferido; rode 'make ferramentas'" >&2; exit 1; }

## ferramentas: instala cargo-audit, cargo-deny (cargo install --locked) e o gitleaks fixo
ferramentas: origens
	$(CARGO) install --locked cargo-audit@$(CARGO_AUDIT_VERSAO) cargo-deny@$(CARGO_DENY_VERSAO)
	@tmp="$$(mktemp -d)"; trap 'rm -rf "$$tmp"' EXIT; \
	curl -q -fsSL --proto '=https' --tlsv1.2 -o "$$tmp/gitleaks.tar.gz" "$(GITLEAKS_URL)"; \
	echo "$(GITLEAKS_SHA256)  $$tmp/gitleaks.tar.gz" | sha256sum --check --quiet; \
	mkdir -p "$(FERRAMENTAS_DIR)/bin"; \
	tar -xzf "$$tmp/gitleaks.tar.gz" -C "$$tmp" gitleaks; \
	echo "$(GITLEAKS_BIN_SHA256)  $$tmp/gitleaks" | sha256sum --check --quiet --strict; \
	install -m 0755 "$$tmp/gitleaks" "$(FERRAMENTAS_DIR)/bin/gitleaks"; \
	"$(FERRAMENTAS_DIR)/bin/gitleaks" version

## teste-ataques-fundacao: ataques de V1.2, V1.3 e V1.5 (cada um tem de falhar)
teste-ataques-fundacao: gitleaks-binario
	dev/testes/ataques-fundacao.sh

# --- Serviços de teste (Fase 12): docker compose em dev/compose.yml ----------------------
# Pelo dev/servicos/compose.sh: PATH fixo, DOCKER_CONFIG vazio em .dev/docker/, sem
# DOCKER_HOST/DOCKER_CONTEXT/COMPOSE_* herdados.
COMPOSE_TESTE := dev/servicos/compose.sh

## dev-servicos: gera em .dev/servicos/ (0700) as chaves e senhas falsas dos serviços
dev-servicos:
	$(PYTHON) dev/servicos/gerar.py

## dev-servicos-subir: gera o que falta e sobe os serviços de teste (só em 127.0.0.1)
dev-servicos-subir: dev-servicos
	$(COMPOSE_TESTE) up -d --build

## dev-servicos-parar: para e apaga os contêineres e o volume do WordPress
dev-servicos-parar:
	$(COMPOSE_TESTE) down -v

## dev-servicos-estado: lista os serviços de teste e o estado de cada um
dev-servicos-estado:
	$(COMPOSE_TESTE) ps -a --format '{{.Service}} {{.State}}'

## dev-servicos-conferir: confere V1.4 com os serviços de pé
dev-servicos-conferir:
	dev/servicos/conferir.sh

## dev-servicos-trocar-chave-host SERVICO=ssh-senha: nova chave de host e reinicia (V4.28)
SERVICO ?= ssh-senha
dev-servicos-trocar-chave-host:
	$(PYTHON) dev/servicos/gerar.py --trocar-chave-host "$(SERVICO)"
	$(COMPOSE_TESTE) restart "$(SERVICO)"

## teste-ataques-servicos: ataques aos serviços de teste (cada um tem de falhar); depois,
## os mesmos scripts inteiros com iscas FIFO no home (teste-iscas-home)
teste-ataques-servicos:
	dev/testes/ataques-servicos.sh
	dev/testes/iscas-home.sh

## teste-iscas-home: conferir.sh e ataques-servicos.sh inteiros com iscas FIFO no home
## (coberto por tmpfs): nenhuma pode ser aberta (Fase 12b)
teste-iscas-home:
	dev/testes/iscas-home.sh

## teste-home-real: clientes de teste sob strace, com o home coberto por tmpfs (Fase 12a)
teste-home-real:
	dev/testes/home-real.sh

# --- Cofre de teste (Fase 19) ------------------------------------------------------------
# O destino vai ao programa pelo ambiente (nunca interpolado no shell) e o próprio
# programa recusa qualquer caminho fora de `.dev/` (V2.29, S15). Os dois dependem de
# `dev-servicos`, que gera a chave do deploy em `.dev/servicos/chaves/`.
COFRE ?= .dev/cofre-teste.ward
VARIACOES ?= .dev/variacoes.txt

## dev-cofre: cria (ou recria) .dev/cofre-teste.ward com as 22 entradas de
## dev/fixtures/entradas.json (D15 + D28 + D59), reabre com a senha de teste e confere;
## COFRE=.dev/<nome>.ward troca o destino (só dentro de .dev/)
dev-cofre: export WARD_DEV_SAIDA = $(COFRE)
dev-cofre: origens dev-servicos
	$(CARGO) run --quiet --locked -p ward-core --example dev-cofre -- --saida "$$WARD_DEV_SAIDA"
	@# Cofre recriado: o estado do daemon de teste (D39) era do cofre anterior.
	rm -f -- "$${WARD_DEV_SAIDA%.ward}.estado"

## dev-cofre-conferir: só abre o cofre de teste com a senha de teste e confere as entradas
dev-cofre-conferir: export WARD_DEV_SAIDA = $(COFRE)
dev-cofre-conferir: origens
	$(CARGO) run --quiet --locked -p ward-core --example dev-cofre -- --saida "$$WARD_DEV_SAIDA" --conferir

## dev-variacoes: grava em .dev/variacoes.txt (0600) os segredos do cofre de teste e as
## variações da tabela única de D54 (segredo inteiro, Basic, por linha e valor de linha,
## com as formas de texto; mais D58 e D61), a mesma da máscara, um padrão por linha,
## para `grep -F -f`; VARIACOES=.dev/<nome>.txt
dev-variacoes: export WARD_DEV_SAIDA = $(VARIACOES)
dev-variacoes: dev-servicos
	$(PYTHON) dev/variacoes.py --saida "$$WARD_DEV_SAIDA"

## teste-dev-cofre: V2.29, V2.52, V2.57, V3.29, V3.31, V3.32 e ataques ao
## dev-cofre/dev-variacoes (cada ataque tem de falhar)
teste-dev-cofre: origens
	dev/testes/dev-cofre.sh

# --- App desktop (Fase 29) -----------------------------------------------------------
# `app/nucleo` (comandos, cliente do daemon, copiar) e a ponte de teste estão no
# workspace da raiz e entram no `make check`. O crate do Tauri (`app/src-tauri`) tem
# workspace e lockfile próprios: precisa das bibliotecas de desenvolvimento do WebKitGTK
# 4.1 (`libwebkit2gtk-4.1-dev`; o pkg-config do sistema ou um PKG_CONFIG_PATH herdado)
# e entra só no `make app-check`.
#
# node/npm não estão no PATH fixo. Padrão: o node mais novo do nvm do usuário; troque
# com `make app-check NODE_DIR=/caminho/do/node/bin`. O `npm ci` roda com
# `--ignore-scripts` (e o `app/.npmrc` também proíbe scripts de instalação).
NODE_DIR ?= $(lastword $(sort $(wildcard $(CASA)/.nvm/versions/node/v*/bin)))
override APP_NPM = PATH="$(NODE_DIR):/usr/bin:/bin" "$(NODE_DIR)/npm" --prefix "$(CURDIR)/app"
# Advisories do gtk-rs 0.18 que o Tauri 2 exige no Linux (motivo no app/src-tauri/deny.toml
# e no handoff da Fase 29). Nenhuma outra exceção.
override APP_AUDIT_IGNORAR := --ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0429

## app-front: npm ci (lockfile, sem scripts), build de produção e de teste da interface,
## conferência dos pacotes (V5.2) e npm audit (dependências de runtime sem nenhum aviso)
app-front:
	@test -x "$(NODE_DIR)/node" || { echo "node não encontrado; use NODE_DIR=/caminho/do/node/bin" >&2; exit 1; }
	$(APP_NPM) ci --ignore-scripts --no-audit --no-fund
	$(APP_NPM) run build
	$(APP_NPM) run build:teste
	$(PYTHON) dev/testes/app-dist.py
	$(APP_NPM) audit --omit=dev
	$(APP_NPM) audit --audit-level=moderate

## app-check: app-front + fmt, clippy -D warnings, testes (ACL com o runtime falso do
## Tauri), cargo audit e cargo deny do app Tauri, build de release e V5.6 (strings)
app-check: origens app-front
	cd app/src-tauri && $(CARGO) fmt --check
	cd app/src-tauri && $(CARGO) clippy --all-targets --all-features --locked -- -D warnings
	cd app/src-tauri && $(CARGO) test --all-features --locked
	cd app/src-tauri && $(CARGO_AUDIT) audit --deny warnings $(APP_AUDIT_IGNORAR)
	cd app/src-tauri && $(CARGO_DENY) --all-features check
	cd app/src-tauri && $(CARGO) build --release --locked
	dev/testes/app-release.sh app/src-tauri/target/release/ward-app

## app-ponte: sobe a ponte de teste (D7) servindo app/dist-teste; imprime a URL com o
## token para o Chrome de teste. Precisa do daemon de teste (Milestone 3) para responder.
app-ponte: origens
	@test -f app/dist-teste/index.html || { echo "rode 'make app-front' antes" >&2; exit 1; }
	$(CARGO) run --quiet --locked -p ward-ponte-teste --features modo-teste -- --dist app/dist-teste

# --- Plataforma em contêiner (Fase 21b) ---------------------------------------------
# D46/D47 com root só dentro de um contêiner ubuntu:24.04 descartável, sem rede: grupos
# ward-app/ward-nm, binários setgid 2755, /run/ward/1000 do root e socket criado pelo
# root e entregue por LISTEN_FDS. Nada de sudo nem de /etc ou /run do host. Exige Docker
# e gcc (a biblioteca do ataque de LD_PRELOAD); fica fora do `make check`.

## teste-docker-plataforma: V3.5 (forma de D47), V3.23, V3.24 e V5.31 (lançador e partida
## do app, D57) num contêiner ubuntu:24.04
teste-docker-plataforma: origens
	$(CARGO) build --quiet --locked -p ward-plataforma --features modo-teste --example ward-plataforma-conteiner
	$(CARGO) build --quiet --locked -p ward-app-nucleo --example partida-conteiner
	dev/testes/docker-plataforma.sh target/debug/examples/ward-plataforma-conteiner target/debug/examples/partida-conteiner

## teste-docker-uid: V3.2 (D49) num contêiner ubuntu:24.04: o wardd de teste roda como
## uid 1000; clientes de outro uid (4242, também com CAP_DAC_OVERRIDE) e o root são
## recusados sem receber um byte; o uid 1000 é atendido
teste-docker-uid: origens
	$(CARGO) build --quiet --locked -p wardd --features modo-teste --bin wardd --example wardd-cliente-teste
	dev/testes/docker-uid.sh target/debug/wardd target/debug/examples/wardd-cliente-teste

## teste-units: V3.21 (Fase 24; D46, D57, D62): `systemd-analyze verify` das units de
## `dev/systemd/` e, num contêiner ubuntu:24.04 com systemd e sem rede, a primeira
## conexão ao `wardd@1000.socket` sobe o `wardd` (sem `modo-teste`) como o uid 1000,
## com LimitCORE=0, LimitMEMLOCK=infinity e sem no_new_privs
teste-units: origens
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-cliente-teste --target-dir target/sem-modo-teste
	dev/testes/units.sh target/sem-modo-teste/debug/wardd target/sem-modo-teste/debug/examples/wardd-cliente-teste

# --- Daemon de teste (Fase 22) ------------------------------------------------------
# `wardd` com a feature `modo-teste`: cofre de `.dev/`, socket em
# $XDG_RUNTIME_DIR/ward-teste/. Sobe com `env -i` e uma lista fixa (HOME, PATH,
# XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR): o wardd recusa TMPDIR e LD_* (V3.30).

## dev-daemon: compila e sobe o daemon de teste em segundo plano (log em .dev/logs/)
dev-daemon: origens
	$(CARGO) build --quiet --locked -p wardd --features modo-teste --bin wardd
	dev/daemon.sh subir

## teste-cli: CLI `ward` de teste num pty (Fase 25): V4.2 com servidor falso, V4.1 e
## V4.22 contra o daemon de teste; precisa de `make dev-daemon` e `make dev-variacoes`
teste-cli: origens
	$(CARGO) build --quiet --locked -p ward --features modo-teste --bin ward
	$(PYTHON) dev/testes/cli.py target/debug/ward "$(CURDIR)"

## dev-daemon-parar: para o daemon de teste (confere que o pid é mesmo o wardd daqui)
dev-daemon-parar:
	dev/daemon.sh parar
