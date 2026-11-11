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
override PASSWD_HOME := $(shell /usr/bin/getent passwd "$$(/usr/bin/id -u)" | /usr/bin/cut -d: -f6)
ifeq ($(strip $(PASSWD_HOME)),)
$(error home do passwd indefinido)
endif
# PATH só do sistema; ~/.cargo/bin no fim, para o rustup (não há cargo em /usr/bin).
override PATH := /usr/bin:/bin:$(PASSWD_HOME)/.cargo/bin
export PATH
unexport BASH_ENV ENV CDPATH GLOBIGNORE
# Ferramentas pelo caminho absoluto: `cargo audit`/`cargo deny` passariam pelo
# `[alias]` de um `.cargo/config.toml`; o binário direto, não.
override CARGO := $(PASSWD_HOME)/.cargo/bin/cargo
override CARGO_AUDIT := $(PASSWD_HOME)/.cargo/bin/cargo-audit
override CARGO_DENY := $(PASSWD_HOME)/.cargo/bin/cargo-deny
override PYTHON := /usr/bin/python3 -I -B
override CARGO_HOME := $(PASSWD_HOME)/.cargo
override RUSTUP_HOME := $(PASSWD_HOME)/.rustup
export CARGO CARGO_HOME RUSTUP_HOME
# git (também o que o gitleaks chama): sem config global nem do sistema, e com
# precedência de linha de comando sobre o `.git/config`: sem ignore/attributes globais,
# sem fsmonitor (roda programa a cada `git diff`) e sem hooks. Ver dev/env.sh.
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

# Ferramentas com versão fixa (`make tools`). O gitleaks fica dentro de `.dev/`
# (fora do git): o tar.gz é conferido por sha256 antes de extrair e o binário extraído,
# por sha256, antes de cada varredura (um binário trocado em `.dev/` não roda).
override TOOLS_DIR := $(CURDIR)/.dev/tools
override GITLEAKS := $(TOOLS_DIR)/bin/gitleaks
GITLEAKS_VERSION := 8.30.1
override GITLEAKS_SHA256 := 551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
override GITLEAKS_BIN_SHA256 := 88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509
GITLEAKS_URL := https://github.com/gitleaks/gitleaks/releases/download/v$(GITLEAKS_VERSION)/gitleaks_$(GITLEAKS_VERSION)_linux_x64.tar.gz
CARGO_AUDIT_VERSION := 0.22.2
CARGO_DENY_VERSION := 0.20.2

.PHONY: check build fmt fmt-check clippy test audit deny gitleaks tools \
	origins gitleaks-binary test-attacks-foundation test-attacks-services test-home-decoys test-real-home dev-services dev-services-up \
	dev-services-down dev-services-status dev-services-verify dev-services-rotate-host-key dev-vault \
	dev-vault-verify dev-variations test-dev-vault dev-daemon dev-daemon-stop test-cli test-docker-uid test-units \
	app-front app-check app-bridge test-docker-platform test-no-test-mode test-socket-attacks \
	test-webkit-real test-clipboard test-revealed-selection test-gst-channels test-app-e2e test-app-logs test-nm test-playwright test-skill test-migration-guide \
	release install uninstall test-install extension test-extension deb extension-crx test-deb test-login \
	test-ssh-home test-chrome-policy compile english-check

## check: pré-checagem das origens, depois fmt, clippy -D warnings, testes, cargo audit,
## cargo deny e gitleaks, nessa ordem (V1.1)
check:
	@echo "==> 0/6 origens do cargo (pré-checagem: configs, ambiente, toolchain, fontes)"
	@$(MAKE) origins
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

# Todo alvo que chama o cargo depende de `origins`: nem `make clippy` sozinho roda com
# `.cargo/config*`, RUSTC_WRAPPER etc. plantados.
build: origins
	$(CARGO) build --workspace --locked

fmt: origins
	$(CARGO) fmt --all

fmt-check: origins
	$(CARGO) fmt --all --check

clippy: origins
	$(CARGO) clippy --workspace --all-targets --all-features --locked -- -D warnings

test: origins
	$(CARGO) test --workspace --all-features --locked

# --deny warnings: crate sem manutenção, retirado (yanked) ou com aviso também falha.
audit: origins
	$(CARGO_AUDIT) audit --deny warnings

deny: origins
	$(CARGO_DENY) --all-features check

# `dev/origins.py` (lista de permitidas): nenhum `.cargo/config*` no repositório nem nos
# pais, só chaves inofensivas no do CARGO_HOME, nenhuma variável CARGO_*/RUSTC*/
# RUSTFLAGS*/RUSTDOCFLAGS/RUSTUP* fora da lista, toolchain só do rust-toolchain.toml,
# sem [patch]/[replace], só crates.io e paths dentro do repositório. O cargo deny não vê
# nada disso (o lockfile continua dizendo crates.io).
origins:
	@$(PYTHON) dev/origins.py

# Três varreduras (S15):
# - árvore de trabalho, inclusive arquivos ainda não versionados; os diretórios que o
#   `.gitignore` tira do git ficam de fora (`dev/gitleaks-tree.toml`);
# - índice (`--staged`), sem exceção de diretório: pega o que foi forçado com `git add -f`;
# - histórico inteiro dos commits.
# --verbose mostra arquivo e linha de cada achado; --redact nunca imprime o valor.
#
# Depois, a varredura extra (`dev/gitleaks-extra.toml`): só chave privada e AWS, sem a
# allowlist global de caminhos das regras padrão (lockfiles, svg, png, pdf...) e
# entrando em arquivos compactados (.gz, .zip, .tar), nas mesmas três formas.
GITLEAKS_OPTIONS := --redact --verbose --no-banner --exit-code 1
GITLEAKS_EXTRA := --max-archive-depth 3

gitleaks: gitleaks-binary
	$(GITLEAKS) dir --config dev/gitleaks-tree.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) git --staged --pre-commit --config .gitleaks.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) git --config .gitleaks.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) dir --config dev/gitleaks-extra-tree.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --staged --pre-commit --config dev/gitleaks-extra.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --config dev/gitleaks-extra.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .

# Confere o sha256 do binário do gitleaks antes de usar (falha fechada).
gitleaks-binary:
	@test -x "$(GITLEAKS)" || { echo "gitleaks não encontrado em $(GITLEAKS); rode 'make tools'" >&2; exit 1; }
	@echo "$(GITLEAKS_BIN_SHA256)  $(GITLEAKS)" | sha256sum --check --quiet --strict || { echo "o binário $(GITLEAKS) não é o gitleaks $(GITLEAKS_VERSION) conferido; rode 'make tools'" >&2; exit 1; }

## tools: instala cargo-audit, cargo-deny (cargo install --locked) e o gitleaks fixo
tools: origins
	$(CARGO) install --locked cargo-audit@$(CARGO_AUDIT_VERSION) cargo-deny@$(CARGO_DENY_VERSION)
	@tmp="$$(mktemp -d)"; trap 'rm -rf "$$tmp"' EXIT; \
	curl -q -fsSL --proto '=https' --tlsv1.2 -o "$$tmp/gitleaks.tar.gz" "$(GITLEAKS_URL)"; \
	echo "$(GITLEAKS_SHA256)  $$tmp/gitleaks.tar.gz" | sha256sum --check --quiet; \
	mkdir -p "$(TOOLS_DIR)/bin"; \
	tar -xzf "$$tmp/gitleaks.tar.gz" -C "$$tmp" gitleaks; \
	echo "$(GITLEAKS_BIN_SHA256)  $$tmp/gitleaks" | sha256sum --check --quiet --strict; \
	install -m 0755 "$$tmp/gitleaks" "$(TOOLS_DIR)/bin/gitleaks"; \
	"$(TOOLS_DIR)/bin/gitleaks" version

## test-attacks-foundation: ataques de V1.2, V1.3 e V1.5 (cada um tem de falhar)
test-attacks-foundation: gitleaks-binary
	dev/tests/attacks-foundation.sh

# --- Serviços de teste (Fase 12): docker compose em dev/compose.yml ----------------------
# Pelo dev/services/compose.sh: PATH fixo, DOCKER_CONFIG vazio em .dev/docker/, sem
# DOCKER_HOST/DOCKER_CONTEXT/COMPOSE_* herdados.
COMPOSE_TEST := dev/services/compose.sh

## dev-services: gera em .dev/services/ (0700) as chaves e senhas falsas dos serviços
dev-services:
	$(PYTHON) dev/services/generate.py

## dev-services-up: gera o que falta e sobe os serviços de teste (só em 127.0.0.1)
dev-services-up: dev-services
	$(COMPOSE_TEST) up -d --build

## dev-services-down: para e apaga os contêineres e o volume do WordPress
dev-services-down:
	$(COMPOSE_TEST) down -v

## dev-services-status: lista os serviços de teste e o estado de cada um
dev-services-status:
	$(COMPOSE_TEST) ps -a --format '{{.Service}} {{.State}}'

## dev-services-verify: confere V1.4 com os serviços de pé
dev-services-verify:
	dev/services/verify.sh

## dev-services-rotate-host-key SERVICE=ssh-senha: nova chave de host e reinicia (V4.28)
SERVICE ?= ssh-senha
dev-services-rotate-host-key:
	$(PYTHON) dev/services/generate.py --rotate-host-key "$(SERVICE)"
	$(COMPOSE_TEST) restart "$(SERVICE)"

## test-attacks-services: ataques aos serviços de teste (cada um tem de falhar); depois,
## os mesmos scripts inteiros com iscas FIFO no home (test-home-decoys)
test-attacks-services:
	dev/tests/attacks-services.sh
	dev/tests/home-decoys.sh

## test-home-decoys: verify.sh e attacks-services.sh inteiros com iscas FIFO no home
## (coberto por tmpfs): nenhuma pode ser aberta (Fase 12b)
test-home-decoys:
	dev/tests/home-decoys.sh

## test-real-home: clientes de teste sob strace, com o home coberto por tmpfs (Fase 12a)
test-real-home:
	dev/tests/real-home.sh

# --- Cofre de teste (Fase 19) ------------------------------------------------------------
# O destino vai ao programa pelo ambiente (nunca interpolado no shell) e o próprio
# programa recusa qualquer caminho fora de `.dev/` (V2.29, S15). Os dois dependem de
# `dev-services`, que gera a chave do deploy em `.dev/services/keys/`.
VAULT ?= .dev/test-vault.ward
VARIATIONS ?= .dev/variations.txt

## dev-vault: cria (ou recria) .dev/test-vault.ward com as 22 entradas de
## dev/fixtures/entries.json (D15 + D28 + D59), reabre com a senha de teste e confere;
## VAULT=.dev/<nome>.ward troca o destino (só dentro de .dev/)
dev-vault: export WARD_DEV_OUTPUT = $(VAULT)
dev-vault: origins dev-services
	$(CARGO) run --quiet --locked -p ward-core --example dev-cofre -- --output "$$WARD_DEV_OUTPUT"
	@# Cofre recriado: o estado do daemon de teste (D39) era do cofre anterior.
	rm -f -- "$${WARD_DEV_OUTPUT%.ward}.state"

## dev-vault-verify: só abre o cofre de teste com a senha de teste e confere as entradas
dev-vault-verify: export WARD_DEV_OUTPUT = $(VAULT)
dev-vault-verify: origins
	$(CARGO) run --quiet --locked -p ward-core --example dev-cofre -- --output "$$WARD_DEV_OUTPUT" --conferir

## dev-variations: grava em .dev/variations.txt (0600) os segredos do cofre de teste e as
## variações da tabela única de D54 (segredo inteiro, Basic, por linha e valor de linha,
## com as formas de texto; mais D58 e D61), a mesma da máscara, um padrão por linha,
## para `grep -F -f`; VARIATIONS=.dev/<nome>.txt
dev-variations: export WARD_DEV_OUTPUT = $(VARIATIONS)
dev-variations: dev-services
	$(PYTHON) dev/variations.py --output "$$WARD_DEV_OUTPUT"

## test-dev-vault: V2.29, V2.52, V2.57, V3.29, V3.31, V3.32 e ataques ao
## dev-vault/dev-variations (cada ataque tem de falhar)
test-dev-vault: origins
	dev/tests/dev-vault.sh

# --- App desktop (Fase 29) -----------------------------------------------------------
# `app/core` (comandos, cliente do daemon, copiar) e a ponte de teste estão no
# workspace da raiz e entram no `make check`. O crate do Tauri (`app/src-tauri`) tem
# workspace e lockfile próprios: precisa das bibliotecas de desenvolvimento do WebKitGTK
# 4.1 (`libwebkit2gtk-4.1-dev`; o pkg-config do sistema ou um PKG_CONFIG_PATH herdado)
# e entra só no `make app-check`.
#
# node/npm não estão no PATH fixo. Padrão: o node mais novo do nvm do usuário; troque
# com `make app-check NODE_DIR=/caminho/do/node/bin`. O `npm ci` roda com
# `--ignore-scripts` (e o `app/.npmrc` também proíbe scripts de instalação).
NODE_DIR ?= $(lastword $(sort $(wildcard $(PASSWD_HOME)/.nvm/versions/node/v*/bin)))
override APP_NPM = PATH="$(NODE_DIR):/usr/bin:/bin" "$(NODE_DIR)/npm" --prefix "$(CURDIR)/app"
# Advisories do gtk-rs 0.18 que o Tauri 2 exige no Linux (motivo no app/src-tauri/deny.toml
# e no handoff da Fase 29). Nenhuma outra exceção.
override APP_AUDIT_IGNORE := --ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0429

## app-front: npm ci (lockfile, sem scripts), build de produção e de teste da interface,
## conferência dos pacotes (V5.2), isolamento das letras RTL (D51, Fase 39f) e npm audit
## (dependências de runtime sem nenhum aviso)
app-front:
	@test -x "$(NODE_DIR)/node" || { echo "node não encontrado; use NODE_DIR=/caminho/do/node/bin" >&2; exit 1; }
	$(APP_NPM) ci --ignore-scripts --no-audit --no-fund
	$(APP_NPM) run build
	$(APP_NPM) run build:teste
	$(PYTHON) dev/tests/app-dist.py
	PATH="$(NODE_DIR):/usr/bin:/bin" "$(NODE_DIR)/node" dev/tests/bidi.mjs
	$(PYTHON) dev/tests/bidi_icu.py
	$(APP_NPM) audit --omit=dev
	$(APP_NPM) audit --audit-level=moderate

## app-check: app-front + fmt, clippy -D warnings, testes (ACL com o runtime falso do
## Tauri), cargo audit e cargo deny do app Tauri, build de release e V5.6 (strings)
app-check: origins app-front
	cd app/src-tauri && $(CARGO) fmt --check
	cd app/src-tauri && $(CARGO) clippy --all-targets --all-features --locked -- -D warnings
	cd app/src-tauri && $(CARGO) test --all-features --locked
	cd app/src-tauri && $(CARGO_AUDIT) audit --deny warnings $(APP_AUDIT_IGNORE)
	cd app/src-tauri && $(CARGO_DENY) --all-features check
	cd app/src-tauri && $(CARGO) build --release --locked
	dev/tests/app-release.sh app/src-tauri/target/release/ward-app

## compile: one fast build and no test (m2:PRD rule 6, m2:V1.5): cargo check of the
## workspace (all targets and features), the production Vite build, cargo check of
## app/src-tauri when libwebkit2gtk-4.1-dev is there (or PKG_CONFIG_PATH), node --check
## of the extension, bash -n of the scripts and the Python parser; goes through `origins`
compile: origins
	$(CARGO) check --workspace --all-targets --all-features --locked
	@test -x "$(NODE_DIR)/node" || { echo "node não encontrado; use NODE_DIR=/caminho/do/node/bin" >&2; exit 1; }
	@test -d app/node_modules || $(APP_NPM) ci --ignore-scripts --no-audit --no-fund
	$(APP_NPM) run build
	@if /usr/bin/pkg-config --exists webkit2gtk-4.1; then \
		cd app/src-tauri && $(CARGO) check --all-targets --all-features --locked; \
	else \
		echo "==> sem libwebkit2gtk-4.1-dev: cargo check do app/src-tauri pulado"; \
	fi
	$(PYTHON) dev/syntax_check.py --node "$(NODE_DIR)/node"

## english-check: report of the pt-BR leftovers in comments, identifiers and file names
## (m2:DT17; not in `make check` until Phase 60). `PATHS="<file or folder> ..."` limits
## it to the paths of a phase. The self-test: `python3 -I dev/english_check.py --self-test`
english-check:
	$(PYTHON) dev/english_check.py $(if $(strip $(PATHS)),--paths $(PATHS))

## app-bridge: a ponte de teste (D7) aberta pelo daemon de teste (D63): sobe o daemon de
## teste com WARD_DEV_APP = a ponte (ou exige que esteja no ar com ele), pede abrir_app e
## imprime a URL com o token, lida de .dev/logs/ward-test-bridge.log
app-bridge: origins
	@test -f app/dist-teste/index.html || { echo "rode 'make app-front' antes" >&2; exit 1; }
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_APP="$(TEST_BRIDGE)" dev/daemon.sh up
	dev/bridge.sh

## test-app-logs: V5.27 (S13, Fase 39): nenhum padrão de .dev/variations.txt nos logs de
## .dev/logs/ (app, ponte, daemon de teste e o console da webview que o QA grava ali);
## antes, o autoteste com um log plantado (o ataque tem de ser achado). Nunca imprime a
## lista nem o trecho. Precisa de `make dev-variations`; fora do `make check`.
## `LOGS="<arquivo ou pasta em .dev/> ..."` troca o que é varrido
test-app-logs:
	$(PYTHON) dev/tests/log-scan.py --self-test
	$(PYTHON) dev/tests/log-scan.py $(LOGS)

## test-clipboard: Fase 39f (S2, S12; V5.4, V5.5): a área do GTK do app na sessão
## gráfica sob strace -f (teste ignorado `area_gtk::testes::sessao_real`): nenhum programa
## executado (nada de wl-copy), nenhum arquivo em /tmp, /var/tmp nem $XDG_RUNTIME_DIR com
## o valor (inotify e varredura; antes, o autoteste acha o que o wl-copy grava), wl-paste
## devolve o valor com x-kde-passwordManagerHint, some no prazo, texto do usuário fica,
## trava de fora limpa na hora. Só valores falsos; limpa a área no fim. Exige strace,
## wl-clipboard, python3-gi e libwebkit2gtk-4.1-dev (ou PKG_CONFIG_PATH); fora do `make check`
test-clipboard: origins
	dev/tests/clipboard.sh "$(CARGO)"

## test-webkit-real: V5.32 (D48, D57, D64) com o WebKitGTK real sob strace -f: harness do
## app sem a partida, montado só em .dev/webkit-real; abre uma janela na sessão Wayland
## por ~20 s. Sai 1 se V5.32 falhar (inclusive os canais da 29b, fechados na Fase 39c).
## Exige libwebkit2gtk-4.1-dev (ou PKG_CONFIG_PATH, R32), strace, gcc e `make app-front`;
## fora do `make check`. `NO_MPS=1` roda sem CUDA_MPS_PIPE_DIRECTORY e `NO_39C=1` sem
## GST_PLUGIN_LOADING_WHITELIST/__GL_APPLICATION_PROFILE (mostram os ataques)
test-webkit-real: origins
	dev/tests/webkit-real.sh "$(CARGO)" $(if $(filter 1,$(NO_MPS)),--no-mps) $(if $(filter 1,$(NO_39C)),--no-39c)

## test-app-e2e: V5.10, V5.16, V5.24 (Fase 39c) de ponta a ponta pela ponte: `wardd` de
## teste só dele (WARD_DEV_SOCKET_DIR, padrão /run/user/<uid>/ward-teste-e2e) com a ponte
## aberta pelo daemon, Chrome headless e a CLI de teste de verdade (`ward run` com a
## janela de aprovação; "Conectar" com `ward ssh --token-fd` no ssh-salto e os ataques ao
## token); varredura de logs no fim; o cofre de teste volta ao que era. Precisa de
## `make dev-vault`, `make dev-variations`, `make app-front`, dos serviços de pé e do
## Playwright do Python em PW_PYTHONPATH; fora do `make check`
test-app-e2e: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	PW_PYTHONPATH="$(PW_PYTHONPATH)" WARD_DEV_SOCKET_DIR="$(WARD_DEV_SOCKET_DIR)" dev/tests/app-e2e.sh

## test-revealed-selection: Fase 39g (S2, S12): o valor revelado no `Campo secreto` não vai
## à PRIMARY nem à CLIPBOARD por seleção, Ctrl+A, Ctrl+C, arrasto ou clique triplo, no
## WebKitGTK 4.1 de verdade (python3-gi) num Xwayland privado com cookie (a área de
## transferência da sessão não é tocada); autoteste com o valor como texto (o ataque da
## auditoria é achado). `wardd` de teste só dele (WARD_DEV_SOCKET_DIR, padrão
## /run/user/<uid>/ward-teste-selecao) com a ponte; varredura de logs no fim. Precisa de
## `make dev-vault`, `make dev-variations`, `make app-front`, Xwayland, xauth e da sessão
## Wayland; fora do `make check`
test-revealed-selection: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_SOCKET_DIR="$(WARD_DEV_SOCKET_DIR)" dev/tests/revealed-selection.sh

## test-gst-channels: canais da 29b (Fase 39c) sem o WebKitGTK: o gst-plugin-scanner do
## sistema e o EGL da GPU com o ambiente de D57 (valores do launcher.rs), HOME falso em
## .dev/gst-channels e iscas (frei0r, LADSPA, libvisual, LV2, perfis da NVIDIA). Roda antes
## sem as variáveis da 39c e exige que as iscas carreguem (o teste enxerga o ataque);
## depois com elas, nada carrega. Exige gst-inspect-1.0, strace e gcc; fora do `make check`
test-gst-channels:
	dev/tests/gst-channels.sh

# --- Plataforma em contêiner (Fase 21b) ---------------------------------------------
# D46/D47 com root só dentro de um contêiner ubuntu:24.04 descartável, sem rede: grupos
# ward-app/ward-nm, binários setgid 2755, /run/ward/1000 do root e socket criado pelo
# root e entregue por LISTEN_FDS. Nada de sudo nem de /etc ou /run do host. Exige Docker
# e gcc (a biblioteca do ataque de LD_PRELOAD); fica fora do `make check`.

## test-docker-platform: V3.5 (forma de D47), V3.23, V3.24, V5.31 (lançador e partida
## do app, D57), a 2ª parte do V4.2 (CLI e wardd sem test-mode, falso não-dumpable do
## uid 1000; Fase 28) e V6.1/V6.2 (Fase 40: ward-nm do release setgid com o "Chrome" de
## pai e o wardd de teste real; precisa de `make dev-vault`) num contêiner ubuntu:24.04
test-docker-platform: origins
	$(CARGO) build --quiet --locked -p ward-platform --features test-mode --example ward-platform-container
	$(CARGO) build --quiet --locked -p ward-app-core --example partida-conteiner
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --bin wardd --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --example chrome-falso --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --example wardd-cliente-teste
	dev/tests/docker-platform.sh target/debug/examples/ward-platform-container target/debug/examples/partida-conteiner \
		target/no-test-mode/debug/ward target/no-test-mode/debug/wardd \
		target/debug/wardd target/no-test-mode/debug/ward-nm target/no-test-mode/debug/examples/chrome-falso \
		target/debug/examples/wardd-cliente-teste

## test-docker-uid: V3.2 (D49) num contêiner ubuntu:24.04: o wardd de teste roda como
## uid 1000; clientes de outro uid (4242, também com CAP_DAC_OVERRIDE) e o root são
## recusados sem receber um byte; o uid 1000 é atendido
test-docker-uid: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --example wardd-cliente-teste
	dev/tests/docker-uid.sh target/debug/wardd target/debug/examples/wardd-cliente-teste

## test-units: V3.21 (Fase 24; D46, D57, D62): `systemd-analyze verify` das units de
## `install/systemd/` e, num contêiner ubuntu:24.04 com systemd e sem rede, a primeira
## conexão ao `wardd@1000.socket` sobe o `wardd` (sem `test-mode`) como o uid 1000,
## com LimitCORE=0, LimitMEMLOCK=infinity e sem no_new_privs
test-units: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-cliente-teste --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	dev/tests/units.sh target/no-test-mode/debug/wardd target/no-test-mode/debug/examples/wardd-cliente-teste target/no-test-mode/debug/ward

# --- Instalação (Fase 51; D18, D46, D47, D57, D60, D62, D63) --------------------------
# `make release` (como o usuário, sem root) compila os artefatos por pacote (`-p`, sem
# `test-mode`, D63); `sudo make install` só confere e copia (nunca chama o cargo como
# root) e `sudo make uninstall` desfaz. O trabalho fica em install/install.sh e a
# árvore, em install/tree.txt. Com DESTDIR=<pasta absoluta>, só copia a árvore
# (sem grupos, sem dono root, sem systemctl): staging do `.deb` (Fase 52) e testes.
PREFIX ?= /usr/local
DESTDIR ?=
WARD_UID ?= $(SUDO_UID)
EXTENSION_ID ?=
EXTENSION_CRX ?=
BIN_WARD ?= $(CURDIR)/target/release/ward
BIN_WARDD ?= $(CURDIR)/target/release/wardd
BIN_NM ?= $(CURDIR)/target/release/ward-nm
BIN_APP ?= $(CURDIR)/app/src-tauri/target/release/ward-app
# Pelo ambiente (`export`), nunca interpolado na linha do shell.
install uninstall: export DESTDIR := $(DESTDIR)
install uninstall: export PREFIX := $(PREFIX)
install uninstall: export WARD_UID := $(WARD_UID)
install uninstall: export EXTENSION_ID := $(EXTENSION_ID)
install uninstall: export EXTENSION_CRX := $(EXTENSION_CRX)
install uninstall: export BIN_WARD := $(BIN_WARD)
install uninstall: export BIN_WARDD := $(BIN_WARDD)
install uninstall: export BIN_NM := $(BIN_NM)
install uninstall: export BIN_APP := $(BIN_APP)

## release: artefatos de release para o `make install`, como o usuário: ward, wardd e
## ward-nm por pacote (`-p`, sem `test-mode`, D63) e o app (`app-front` + build do
## Tauri; precisa de libwebkit2gtk-4.1-dev ou PKG_CONFIG_PATH, R32), com o V5.6
release: origins app-front
	$(CARGO) build --release --locked -p ward --bin ward
	$(CARGO) build --release --locked -p wardd --bin wardd
	$(CARGO) build --release --locked -p ward-nm --bin ward-nm
	cd app/src-tauri && $(CARGO) build --release --locked
	dev/tests/app-release.sh app/src-tauri/target/release/ward-app

## install: `sudo make install EXTENSION_ID=<id> EXTENSION_CRX=<pacote.crx>` (depois de
## `make release`): grupos ward-app/ward-nm (D60), binários (ward-app e ward-nm 2755),
## units do sistema, /run/ward por tmpfiles.d, manifesto de native messaging, política
## do Chrome e `systemctl enable --now wardd@<SUDO_UID>.socket`. Recusa artefato com
## `test-mode` (V9.2). DESTDIR=<pasta> só copia a árvore
install:
	install/install.sh instalar

## uninstall: `sudo make uninstall`: para e desliga o wardd@*, apaga os arquivos e os
## grupos; nunca toca no cofre do usuário (V9.10)
uninstall:
	install/install.sh desinstalar

## test-install: V9.1, V9.2, V9.6, V9.10 com DESTDIR em .dev/ (árvore, modos, conteúdo,
## systemd-analyze verify, recusas) e, num contêiner ubuntu:24.04 com systemd e sem
## rede, `make install`/`uninstall` como root: grupos (D60, V9.11), units sem
## no_new_privs (D57), socket e primeira conexão (V9.7), ambiente e dumpable do wardd
## (V9.4), app aberto pelo daemon com o papel app, CLI recusa servidor falso (V9.3).
## Exige Docker (o build da imagem usa rede), gcc e `make dev-vault`; fora do `make check`
test-install: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-cliente-teste --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example partida-conteiner --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-platform --features test-mode --example ward-platform-container
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/install.sh

## test-chrome-policy: Fase 51a (V9.5, V9.6; crítico da auditoria do M9): a política
## gerada pelo install.sh (NativeMessagingUserLevelHosts false) com o Chrome real num
## bwrap com /etc/opt/chrome trocado por uma pasta em .dev/chrome-policy.*: sem ela o
## manifesto plantado no perfil vence e o ataque da auditoria tira senhas do wardd de
## teste (controle); com ela o host do perfil não roda e o do sistema sim. Nunca toca o
## perfil real do Chrome. Precisa de bwrap, openssl, `make dev-vault`, `make
## dev-variations` e do Chrome em /opt/google/chrome; fora do `make check`
test-chrome-policy: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example partida-conteiner --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	dev/tests/chrome-policy.sh target/debug/wardd target/debug/ward-nm \
		target/no-test-mode/debug/ward target/no-test-mode/debug/wardd \
		target/no-test-mode/debug/ward-nm target/no-test-mode/debug/examples/partida-conteiner \
		target/debug/wardd-aprovador-teste

# --- Pacote .deb (Fase 52; D18; V9.9) ------------------------------------------------
# `make deb` (como o usuário) monta o staging com o `install.sh` (DESTDIR) e o empacota
# por `dpkg-deb`; os binários setgid vão 0755 no pacote e o 2755 entra pelo
# `dpkg-statoverride` do preinst. `make extension-crx` empacota a extensão pelo Chrome.
DEB_DIR ?= $(CURDIR)/target/deb
EXTENSION_KEY ?= $(CURDIR)/.dev/extension/key.pem
deb: export DEB_DIR := $(DEB_DIR)
deb: export EXTENSION_ID := $(EXTENSION_ID)
deb: export EXTENSION_CRX := $(EXTENSION_CRX)
deb: export BIN_WARD := $(BIN_WARD)
deb: export BIN_WARDD := $(BIN_WARDD)
deb: export BIN_NM := $(BIN_NM)
deb: export BIN_APP := $(BIN_APP)
extension-crx: export EXTENSION_KEY := $(EXTENSION_KEY)

## deb: `make deb EXTENSION_ID=<id> EXTENSION_CRX=<pacote.crx>` (depois de `make release`),
## sem sudo: target/deb/ward_<versão>_<arq>.deb com a árvore do `make install`, Depends
## com sshpass; instalar com `sudo dpkg -i` (roteiro em docs/instalacao.md)
deb:
	install/deb.sh

## extension-crx: empacota extension/dist/ (do `make extension`) em target/extension/ward.crx
## pelo Chrome com a chave .dev/extension/key.pem (EXTENSION_KEY); recusa id ≠ ID_EXTENSAO
extension-crx:
	install/pack-extension.sh

## test-deb: V9.9 (Fase 52): `make deb` com os artefatos sem `test-mode` e um CRX3 falso
## (lista do `dpkg-deb -c` = staging, root/root, sem setgid no pacote, Depends com
## sshpass, recusas) e, num contêiner ubuntu:24.04 com systemd e sem rede, `dpkg -i`
## (falta de sshpass, grupo fora de D60, instalação, statoverride 2755, socket, primeira
## conexão, pasta SSH do usuário e /etc/ssh intactas, V9.8), reinstalar, `dpkg -r`/`-P`
## com o cofre intacto. Exige Docker (o build da imagem usa rede) e gcc; fora do `make check`
test-deb: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-cliente-teste --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example partida-conteiner --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/deb.sh

# --- Daemon de teste (Fase 22) ------------------------------------------------------
# `wardd` com a feature `test-mode`: cofre de `.dev/`, socket em
# $XDG_RUNTIME_DIR/ward-teste/. Sobe com `env -i` e uma lista fixa (HOME, PATH,
# XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR): o wardd recusa TMPDIR e LD_* (V3.30).

## dev-daemon: compila e sobe o daemon de teste em segundo plano (log em .dev/logs/), com
## WARD_DEV_APP (D63; padrão: a ponte de teste, que o `abrir_app` abre com o papel app)
TEST_BRIDGE := $(CURDIR)/target/debug/ward-test-bridge
WARD_DEV_APP ?= $(TEST_BRIDGE)
dev-daemon: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_APP="$(WARD_DEV_APP)" dev/daemon.sh up

## test-cli: CLI `ward` de teste num pty (Fase 25): V4.2 com servidor falso, V4.1 e
## V4.22 contra o daemon de teste; precisa de `make dev-daemon WARD_DEV_APP=` (sem a
## ponte: recusa no começo se o daemon de teste subiu com WARD_DEV_APP) e `make dev-variations`
test-cli: origins
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	$(PYTHON) dev/tests/cli.py target/debug/ward "$(CURDIR)"

## test-ssh-home: V4.27 (Fase 28b; R19, D13): a suíte de `ward ssh` e túnel num bwrap com
## o home do passwd e o /tmp em tmpfs, iscas na pasta do ssh do home (config com Host * e
## ProxyCommand de sentinela, known_hosts, id_ed25519, FIFOs), agente falso, inotify e o
## ssh/sshpass/sftp do daemon sob strace; controles positivos no fim. `wardd` de teste só
## dele (WARD_DEV_SOCKET_DIR próprio). Precisa de bwrap, strace, `make dev-vault`,
## `make dev-variations` e dos serviços de pé; fora do `make check`
test-ssh-home: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/ssh-home.sh target/debug/wardd target/debug/ward

## test-playwright: `ward playwright` de ponta a ponta (Fase 44; V6.12–V6.16 da parte do
## daemon) com o Chrome instalado: os ataques contra sites locais (`cargo test --ignored`)
## e, com os serviços de pé, o WordPress e o phish contra um `wardd` de teste só dele
## (WARD_DEV_SOCKET_DIR próprio). Precisa de `make dev-vault`, `make dev-variations` e do
## Playwright do Python em PW_PYTHONPATH (ou NO_PLAYWRIGHT_PYTHON=1); fora do `make check`
test-playwright: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	$(CARGO) test --quiet --locked -p wardd --all-features --test playwright -- --ignored
	PW_PYTHONPATH="$(PW_PYTHONPATH)" NO_PLAYWRIGHT_PYTHON="$(NO_PLAYWRIGHT_PYTHON)" \
		dev/tests/playwright.sh target/debug/wardd target/debug/ward target/debug/wardd-aprovador-teste

## test-no-test-mode: D63/V3.37: nenhum membro liga test-mode em dependência normal,
## testes de unidade do wardd e do ward-platform sem a feature e strings do release
## sem WARD_DEV_APP (compila em target/no-test-mode)
test-no-test-mode: origins
	dev/tests/no-test-mode.sh

## test-socket-attacks: V3.3 e V3.4 (scrutiny do M3) contra um `wardd` de teste só dele
## (cópia do cofre de teste em .dev/socket-attacks.*, socket em $XDG_RUNTIME_DIR por
## WARD_DEV_SOCKET_DIR; nunca o daemon de `make dev-daemon`): quadros ruins, 10 000
## conexões caladas, matriz operação × entrada × campo e o log sem segredo. Precisa de
## `make dev-vault` e `make dev-variations`; fora do `make check`. CONNECTIONS=<n> troca as
## 10 000
CONNECTIONS ?= 10000
test-socket-attacks: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/socket-attacks.sh target/debug/wardd "$(CONNECTIONS)"

## test-nm: Fase 40 (V6.1; D5, D9, D47): o `ward-nm` de teste aberto à mão fica agente
## e não recebe segredo; com o Chrome real (headless, perfil temporário em .dev/nm.*,
## manifesto só nele) o host é filho direto do navegador, o `verify_parent` aceita esse
## pai e recusa outro, e o host responde pelo Chrome. Daemon de teste só dele (cópia do
## cofre, socket próprio). Precisa de `make dev-vault`, `make dev-variations` e do Chrome
## em /opt/google/chrome; fora do `make check`
## test-skill: Fase 50 (V8.1, V8.2, V8.4): frontmatter da skill `ward`, comandos citados
## no `ward --help`, nenhum padrão de .dev/variations.txt em skill/, JSON do deny válido e
## com os caminhos do escopo, regras Bash sem âncora que negam cada forma de ATAQUES_BASH
## e nenhum exemplo da skill (Fase 50a); antes, o autoteste planta cada ataque numa cópia e exige
## falha. O `ward` (sem `test-mode`) vai para target/no-test-mode, fora do target/debug
## dos outros testes. Precisa de `make dev-variations`; fora do `make check`
test-skill: origins
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(PYTHON) dev/tests/skill.py target/no-test-mode/debug/ward

## test-migration-guide: Fases 53, 53b e 53d (V10.1–V10.3): ordem dos passos de docs/migracao.md,
## os itens de montagem.md um por um, Conferir e Voltar atrás em cada passo, nada para a
## conversa com o Claude, nenhum segredo em argv nem padrão de .dev/variations.txt, lista
## positiva de comandos nos blocos (chave só por `install -m 600` entre a pasta em memória
## e a pasta do ssh do home, sem `..`, find/chmod/grep/busctl limitados); antes, o
## autoteste planta cada ataque numa cópia e exige falha. Só lê o guia; não executa
## nenhum passo. Precisa de `make dev-variations`; fora do `make check`
test-migration-guide:
	$(PYTHON) dev/tests/migration-guide.py

test-nm: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward-platform --example pai-chrome
	dev/tests/nm.sh target/debug/wardd target/debug/ward-nm target/debug/examples/pai-chrome

## extension: monta a extensão do Chrome em extension/dist/ (Fase 41) com os tokens, ícones
## e fontes do app e confere o pacote e o manifesto (id fixo = ward-nm, sem
## externally_connectable/web_accessible_resources/storage, CSP estrita). Precisa de
## `make app-front` (fontes)
extension:
	$(PYTHON) dev/extension/build.py

## test-extension: Fase 41 (V6.2, V6.4–V6.7): a extensão no Chrome real (headless, perfil
## temporário em .dev/extension.*) pelo ward-nm de teste e um wardd de teste só dele com
## WARD_DEV_EXTENSAO=chrome; sugestões, preenchimento, TOTP, phishing, redir, iframe,
## DOM clobbering, campo escondido, mensagens da página, travado; screenshots em
## docs/mission/evidence/test-extension/. Precisa de `make dev-vault`, `make dev-variations`,
## `make dev-services-up` e `make app-front`; fora do `make check`
test-extension: origins extension
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward-nm --example chrome-falso
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/extension.sh target/debug/wardd target/debug/ward-nm target/debug/examples/chrome-falso target/debug/ward target/debug/wardd-aprovador-teste

## test-login: `ward login` de ponta a ponta (Fase 43; V6.3, V6.10, V6.11, V6.15, V6.16):
## a extensão no Chrome real headless, o `ward-nm` aberto pelo service worker e um `wardd`
## de teste só dele com `WARD_DEV_EXTENSAO=chrome`; fora do `make check`
test-login: origins extension
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/login.sh target/debug/wardd target/debug/ward-nm target/debug/ward target/debug/wardd-aprovador-teste

## dev-daemon-stop: para o daemon de teste (confere que o pid é mesmo o wardd daqui)
dev-daemon-stop:
	dev/daemon.sh stop
