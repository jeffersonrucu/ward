# Ward - development commands. Always run from the repository root.
#
# Rule (V1.5, S15): no target here reads or writes outside the repository, except the
# build tools (~/.cargo). Development data only in `.dev/`.

# `bash -p`: does not read BASH_ENV/ENV nor import functions exported by the environment.
SHELL := /bin/bash
.SHELLFLAGS := -p -eu -o pipefail -c
.DEFAULT_GOAL := check
MAKEFLAGS += --no-print-directory

# Fixed environment (Phase 12b). `override`: neither the environment, nor an inherited
# `MAKEFLAGS='CARGO=...'`, nor `make VAR=...` swap a tool, PATH or the git config.
#
# Home from passwd, not $HOME (a HOME=/tmp/x would swap cargo and CARGO_HOME).
override PASSWD_HOME := $(shell /usr/bin/getent passwd "$$(/usr/bin/id -u)" | /usr/bin/cut -d: -f6)
ifeq ($(strip $(PASSWD_HOME)),)
$(error home do passwd indefinido)
endif
# PATH only from the system; ~/.cargo/bin at the end, for rustup (there is no cargo in /usr/bin).
override PATH := /usr/bin:/bin:$(PASSWD_HOME)/.cargo/bin
export PATH
unexport BASH_ENV ENV CDPATH GLOBIGNORE
# Tools by absolute path: `cargo audit`/`cargo deny` would go through the `[alias]` of a
# `.cargo/config.toml`; the binary itself does not.
override CARGO := $(PASSWD_HOME)/.cargo/bin/cargo
override CARGO_AUDIT := $(PASSWD_HOME)/.cargo/bin/cargo-audit
override CARGO_DENY := $(PASSWD_HOME)/.cargo/bin/cargo-deny
override PYTHON := /usr/bin/python3 -I -B
override CARGO_HOME := $(PASSWD_HOME)/.cargo
override RUSTUP_HOME := $(PASSWD_HOME)/.rustup
export CARGO CARGO_HOME RUSTUP_HOME
# git (also the one gitleaks calls): no global nor system config, and with command line
# precedence over `.git/config`: no global ignore/attributes, no fsmonitor (runs a
# program on every `git diff`) and no hooks. See dev/env.sh.
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

# Tools with a fixed version (`make tools`). gitleaks lives inside `.dev/` (out of git):
# the tar.gz is verified by sha256 before extracting, and the extracted binary by sha256
# before each scan (a binary swapped in `.dev/` does not run).
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

## check: origins pre-check, then fmt, clippy -D warnings, tests, cargo audit,
## cargo deny and gitleaks, in this order (V1.1)
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

# Every target that calls cargo depends on `origins`: not even `make clippy` alone runs
# with a planted `.cargo/config*`, RUSTC_WRAPPER etc.
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

# --deny warnings: an unmaintained or yanked crate, or one with a warning, also fails.
audit: origins
	$(CARGO_AUDIT) audit --deny warnings

deny: origins
	$(CARGO_DENY) --all-features check

# `dev/origins.py` (allowlist): no `.cargo/config*` in the repository nor in its parents,
# only harmless keys in the one of CARGO_HOME, no CARGO_*/RUSTC*/RUSTFLAGS*/RUSTDOCFLAGS/
# RUSTUP* variable outside the list, toolchain only from rust-toolchain.toml, no
# [patch]/[replace], only crates.io and paths inside the repository. cargo deny sees none
# of this (the lockfile still says crates.io).
origins:
	@$(PYTHON) dev/origins.py

# Three scans (S15):
# - working tree, including files not versioned yet; the dirs that `.gitignore` keeps out
#   of git are skipped (`dev/gitleaks-tree.toml`);
# - index (`--staged`), with no dir exception: catches what was forced with `git add -f`;
# - the whole commit history.
# --verbose shows the file and line of each finding; --redact never prints the value.
#
# Then the extra scan (`dev/gitleaks-extra.toml`): only private keys and AWS, without the
# global path allowlist of the default rules (lockfiles, svg, png, pdf...) and going into
# compressed files (.gz, .zip, .tar), in the same three forms.
GITLEAKS_OPTIONS := --redact --verbose --no-banner --exit-code 1
GITLEAKS_EXTRA := --max-archive-depth 3

gitleaks: gitleaks-binary
	$(GITLEAKS) dir --config dev/gitleaks-tree.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) git --staged --pre-commit --config .gitleaks.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) git --config .gitleaks.toml $(GITLEAKS_OPTIONS) .
	$(GITLEAKS) dir --config dev/gitleaks-extra-tree.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --staged --pre-commit --config dev/gitleaks-extra.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .
	$(GITLEAKS) git --config dev/gitleaks-extra.toml $(GITLEAKS_OPTIONS) $(GITLEAKS_EXTRA) .

# Verifies the sha256 of the gitleaks binary before using it (fail closed).
gitleaks-binary:
	@test -x "$(GITLEAKS)" || { echo "gitleaks não encontrado em $(GITLEAKS); rode 'make tools'" >&2; exit 1; }
	@echo "$(GITLEAKS_BIN_SHA256)  $(GITLEAKS)" | sha256sum --check --quiet --strict || { echo "o binário $(GITLEAKS) não é o gitleaks $(GITLEAKS_VERSION) conferido; rode 'make tools'" >&2; exit 1; }

## tools: installs cargo-audit, cargo-deny (cargo install --locked) and the pinned gitleaks
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

## test-attacks-foundation: attacks of V1.2, V1.3 and V1.5 (each one must fail)
test-attacks-foundation: gitleaks-binary
	dev/tests/attacks-foundation.sh

# --- Test services (Phase 12): docker compose in dev/compose.yml -------------------------
# Through dev/services/compose.sh: fixed PATH, empty DOCKER_CONFIG in .dev/docker/, no
# inherited DOCKER_HOST/DOCKER_CONTEXT/COMPOSE_*.
COMPOSE_TEST := dev/services/compose.sh

## dev-services: generates in .dev/services/ (0700) the fake keys and passwords of the services
dev-services:
	$(PYTHON) dev/services/generate.py

## dev-services-up: generates what is missing and starts the test services (only on 127.0.0.1)
dev-services-up: dev-services
	$(COMPOSE_TEST) up -d --build

## dev-services-down: stops and deletes the containers and the WordPress volume
dev-services-down:
	$(COMPOSE_TEST) down -v

## dev-services-status: lists the test services and the state of each one
dev-services-status:
	$(COMPOSE_TEST) ps -a --format '{{.Service}} {{.State}}'

## dev-services-verify: verifies V1.4 with the services up
dev-services-verify:
	dev/services/verify.sh

## dev-services-rotate-host-key SERVICE=<service>: new host key and restart (V4.28;
## default: the password SSH service)
SERVICE ?= ssh-senha
dev-services-rotate-host-key:
	$(PYTHON) dev/services/generate.py --rotate-host-key "$(SERVICE)"
	$(COMPOSE_TEST) restart "$(SERVICE)"

## test-attacks-services: attacks on the test services (each one must fail); then the
## same scripts, whole, with FIFO decoys in the home (test-home-decoys)
test-attacks-services:
	dev/tests/attacks-services.sh
	dev/tests/home-decoys.sh

## test-home-decoys: verify.sh and attacks-services.sh, whole, with FIFO decoys in the
## home (covered by tmpfs): none of them may be opened (Phase 12b)
test-home-decoys:
	dev/tests/home-decoys.sh

## test-real-home: test clients under strace, with the home covered by tmpfs (Phase 12a)
test-real-home:
	dev/tests/real-home.sh

# --- Test vault (Phase 19) --------------------------------------------------------------
# The destination goes to the program through the environment (never interpolated in the
# shell) and the program itself refuses any path outside `.dev/` (V2.29, S15). Both
# depend on `dev-services`, which generates the deploy key in `.dev/services/keys/`.
VAULT ?= .dev/test-vault.ward
VARIATIONS ?= .dev/variations.txt

## dev-vault: creates (or recreates) .dev/test-vault.ward with the 22 entries of
## dev/fixtures/entries.json (D15 + D28 + D59), reopens it with the test password and
## verifies; VAULT=.dev/<name>.ward changes the destination (only inside .dev/)
dev-vault: export WARD_DEV_OUTPUT = $(VAULT)
dev-vault: origins dev-services
	$(CARGO) run --quiet --locked -p ward-core --example dev-vault -- --output "$$WARD_DEV_OUTPUT"
	@# Vault recreated: the state of the test daemon (D39) belonged to the previous vault.
	rm -f -- "$${WARD_DEV_OUTPUT%.ward}.state"

## dev-vault-verify: only opens the test vault with the test password and verifies the entries
dev-vault-verify: export WARD_DEV_OUTPUT = $(VAULT)
dev-vault-verify: origins
	$(CARGO) run --quiet --locked -p ward-core --example dev-vault -- --output "$$WARD_DEV_OUTPUT" --verify

## dev-variations: writes to .dev/variations.txt (0600) the secrets of the test vault and
## the variations of the single table of D54 (whole secret, Basic, per line and line value,
## with the text forms; plus D58 and D61), the same as the mask's, one pattern per line,
## for `grep -F -f`; VARIATIONS=.dev/<name>.txt
dev-variations: export WARD_DEV_OUTPUT = $(VARIATIONS)
dev-variations: dev-services
	$(PYTHON) dev/variations.py --output "$$WARD_DEV_OUTPUT"

## test-dev-vault: V2.29, V2.52, V2.57, V3.29, V3.31, V3.32 and attacks on
## dev-vault/dev-variations (each attack must fail)
test-dev-vault: origins
	dev/tests/dev-vault.sh

# --- Desktop app (Phase 29) -------------------------------------------------------------
# `app/core` (commands, daemon client, copy) and the test bridge are in the root
# workspace and go into `make check`. The Tauri crate (`app/src-tauri`) has its own
# workspace and lockfile: it needs the WebKitGTK 4.1 development libraries
# (`libwebkit2gtk-4.1-dev`; the system pkg-config or an inherited PKG_CONFIG_PATH) and
# only goes into `make app-check`.
#
# node/npm are not in the fixed PATH. Default: the newest node of the user's nvm; change
# it with `make app-check NODE_DIR=/path/to/node/bin`. `npm ci` runs with
# `--ignore-scripts` (and `app/.npmrc` also forbids install scripts).
NODE_DIR ?= $(lastword $(sort $(wildcard $(PASSWD_HOME)/.nvm/versions/node/v*/bin)))
override APP_NPM = PATH="$(NODE_DIR):/usr/bin:/bin" "$(NODE_DIR)/npm" --prefix "$(CURDIR)/app"
# Advisories of gtk-rs 0.18, which Tauri 2 requires on Linux (reason in
# app/src-tauri/deny.toml and in the Phase 29 handoff). No other exception.
override APP_AUDIT_IGNORE := --ignore RUSTSEC-2024-0370 --ignore RUSTSEC-2024-0429

## app-front: npm ci (lockfile, no scripts), production and test builds of the interface,
## package check (V5.2), isolation of the RTL letters (D51, Phase 39f) and npm audit
## (runtime dependencies without any warning)
app-front:
	@test -x "$(NODE_DIR)/node" || { echo "node não encontrado; use NODE_DIR=/caminho/do/node/bin" >&2; exit 1; }
	$(APP_NPM) ci --ignore-scripts --no-audit --no-fund
	$(APP_NPM) run build
	$(APP_NPM) run build:test
	$(PYTHON) dev/tests/app-dist.py
	PATH="$(NODE_DIR):/usr/bin:/bin" "$(NODE_DIR)/node" dev/tests/bidi.mjs
	$(PYTHON) dev/tests/bidi_icu.py
	$(APP_NPM) audit --omit=dev
	$(APP_NPM) audit --audit-level=moderate

## app-check: app-front + fmt, clippy -D warnings, tests (ACL with Tauri's fake runtime),
## cargo audit and cargo deny of the Tauri app, release build and V5.6 (strings)
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

## app-bridge: the test bridge (D7) opened by the test daemon (D63): starts the test
## daemon with WARD_DEV_APP = the bridge (or requires it to be up with it), asks the
## daemon to open the app and prints the URL with the token, read from
## .dev/logs/ward-test-bridge.log
app-bridge: origins
	@test -f app/dist-teste/index.html || { echo "rode 'make app-front' antes" >&2; exit 1; }
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_APP="$(TEST_BRIDGE)" dev/daemon.sh up
	dev/bridge.sh

## test-app-logs: V5.27 (S13, Phase 39): no pattern of .dev/variations.txt in the logs of
## .dev/logs/ (app, bridge, test daemon and the webview console the QA writes there);
## before that, the self-test with a planted log (the attack must be found). Never prints
## the list nor the excerpt. Needs `make dev-variations`; out of `make check`.
## `LOGS="<file or dir in .dev/> ..."` changes what is scanned
test-app-logs:
	$(PYTHON) dev/tests/log-scan.py --self-test
	$(PYTHON) dev/tests/log-scan.py $(LOGS)

## test-clipboard: Phase 39f (S2, S12; V5.4, V5.5): the app's GTK clipboard in the
## graphical session under strace -f (the ignored GTK clipboard session test): no program
## executed (no wl-copy), no file in /tmp, /var/tmp nor $XDG_RUNTIME_DIR with the value
## (inotify and scan; before that, the self-test finds what wl-copy writes), wl-paste
## returns the value with x-kde-passwordManagerHint, it goes away on time, the user's
## text stays, a lock from outside clears it at once. Fake values only; clears the
## clipboard at the end. Needs strace, wl-clipboard, python3-gi and libwebkit2gtk-4.1-dev
## (or PKG_CONFIG_PATH); out of `make check`
test-clipboard: origins
	dev/tests/clipboard.sh "$(CARGO)"

## test-webkit-real: V5.32 (D48, D57, D64) with the real WebKitGTK under strace -f: an app
## harness without the startup, built only in .dev/webkit-real; opens a window in the
## Wayland session for ~20 s. Exits 1 if V5.32 fails (including the 29b channels, closed
## in Phase 39c). Needs libwebkit2gtk-4.1-dev (or PKG_CONFIG_PATH, R32), strace, gcc and
## `make app-front`; out of `make check`. `NO_MPS=1` runs without
## CUDA_MPS_PIPE_DIRECTORY and `NO_39C=1` without
## GST_PLUGIN_LOADING_WHITELIST/__GL_APPLICATION_PROFILE (they show the attacks)
test-webkit-real: origins
	dev/tests/webkit-real.sh "$(CARGO)" $(if $(filter 1,$(NO_MPS)),--no-mps) $(if $(filter 1,$(NO_39C)),--no-39c)

## test-app-e2e: V5.10, V5.16, V5.24 (Phase 39c) end to end through the bridge: a test
## `wardd` of its own (WARD_DEV_SOCKET_DIR, default /run/user/<uid>/ward-teste-e2e) with
## the bridge opened by the daemon, headless Chrome and the real test CLI (`ward run`
## with the approval window; the app's Connect button with `ward ssh --token-fd` on the
## jump SSH service and the attacks on the token); log scan at the end; the test vault
## goes back to what it was.
## Needs `make dev-vault`, `make dev-variations`, `make app-front`, the services up and
## the Python Playwright in PW_PYTHONPATH; out of `make check`
test-app-e2e: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	PW_PYTHONPATH="$(PW_PYTHONPATH)" WARD_DEV_SOCKET_DIR="$(WARD_DEV_SOCKET_DIR)" dev/tests/app-e2e.sh

## test-revealed-selection: Phase 39g (S2, S12): the value revealed in the secret field
## does not go to PRIMARY nor CLIPBOARD by selection, Ctrl+A, Ctrl+C, drag or triple
## click, in the real WebKitGTK 4.1 (python3-gi) on a private Xwayland with a cookie (the
## session clipboard is not touched); self-test with the value as text (the audit attack
## is found). A test `wardd` of its own (WARD_DEV_SOCKET_DIR, default
## /run/user/<uid>/ward-teste-selecao) with the bridge; log scan at the end. Needs
## `make dev-vault`, `make dev-variations`, `make app-front`, Xwayland, xauth and the
## Wayland session; out of `make check`
test-revealed-selection: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_SOCKET_DIR="$(WARD_DEV_SOCKET_DIR)" dev/tests/revealed-selection.sh

## test-gst-channels: the 29b channels (Phase 39c) without WebKitGTK: the system
## gst-plugin-scanner and the GPU EGL with the D57 environment (values of launcher.rs),
## fake HOME in .dev/gst-channels and decoys (frei0r, LADSPA, libvisual, LV2, NVIDIA
## profiles). Runs first without the 39c variables and requires the decoys to load (the
## test sees the attack); then with them, nothing loads. Needs gst-inspect-1.0, strace
## and gcc; out of `make check`
test-gst-channels:
	dev/tests/gst-channels.sh

# --- Platform in a container (Phase 21b) ----------------------------------------------
# D46/D47 with root only inside a disposable ubuntu:24.04 container, without network:
# ward-app/ward-nm groups, setgid 2755 binaries, root's /run/ward/1000 and a socket
# created by root and handed over by LISTEN_FDS. No sudo and nothing of the host's /etc
# or /run. Needs Docker and gcc (the library of the LD_PRELOAD attack); stays out of
# `make check`.

## test-docker-platform: V3.5 (D47 form), V3.23, V3.24, V5.31 (launcher and startup of
## the app, D57), the 2nd part of V4.2 (CLI and wardd without test-mode, a non-dumpable
## fake of uid 1000; Phase 28) and V6.1/V6.2 (Phase 40: release ward-nm setgid with the
## "Chrome" as parent and the real test wardd; needs `make dev-vault`) in an
## ubuntu:24.04 container
test-docker-platform: origins
	$(CARGO) build --quiet --locked -p ward-platform --features test-mode --example ward-platform-container
	$(CARGO) build --quiet --locked -p ward-app-core --example startup-container
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --bin wardd --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --example fake-chrome --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --example wardd-test-client
	dev/tests/docker-platform.sh target/debug/examples/ward-platform-container target/debug/examples/startup-container \
		target/no-test-mode/debug/ward target/no-test-mode/debug/wardd \
		target/debug/wardd target/no-test-mode/debug/ward-nm target/no-test-mode/debug/examples/fake-chrome \
		target/debug/examples/wardd-test-client

## test-docker-uid: V3.2 (D49) in an ubuntu:24.04 container: the test wardd runs as
## uid 1000; clients of another uid (4242, also with CAP_DAC_OVERRIDE) and root are
## refused without getting a byte; uid 1000 is served
test-docker-uid: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --example wardd-test-client
	dev/tests/docker-uid.sh target/debug/wardd target/debug/examples/wardd-test-client

## test-units: V3.21 (Phase 24; D46, D57, D62): `systemd-analyze verify` of the units of
## `install/systemd/` and, in an ubuntu:24.04 container with systemd and without network,
## the first connection to `wardd@1000.socket` starts `wardd` (without `test-mode`) as
## uid 1000, with LimitCORE=0, LimitMEMLOCK=infinity and without no_new_privs
test-units: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-test-client --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	dev/tests/units.sh target/no-test-mode/debug/wardd target/no-test-mode/debug/examples/wardd-test-client target/no-test-mode/debug/ward

# --- Install (Phase 51; D18, D46, D47, D57, D60, D62, D63) ------------------------------
# `make release` (as the user, without root) builds the artifacts per package (`-p`,
# without `test-mode`, D63); `sudo make install` only verifies and copies (never calls
# cargo as root) and `sudo make uninstall` undoes it. The work is in install/install.sh
# and the tree in install/tree.txt. With DESTDIR=<absolute dir>, it only copies the tree
# (no groups, no root owner, no systemctl): staging of the `.deb` (Phase 52) and tests.
PREFIX ?= /usr/local
DESTDIR ?=
WARD_UID ?= $(SUDO_UID)
EXTENSION_ID ?=
EXTENSION_CRX ?=
BIN_WARD ?= $(CURDIR)/target/release/ward
BIN_WARDD ?= $(CURDIR)/target/release/wardd
BIN_NM ?= $(CURDIR)/target/release/ward-nm
BIN_APP ?= $(CURDIR)/app/src-tauri/target/release/ward-app
# Through the environment (`export`), never interpolated in the shell line.
install uninstall: export DESTDIR := $(DESTDIR)
install uninstall: export PREFIX := $(PREFIX)
install uninstall: export WARD_UID := $(WARD_UID)
install uninstall: export EXTENSION_ID := $(EXTENSION_ID)
install uninstall: export EXTENSION_CRX := $(EXTENSION_CRX)
install uninstall: export BIN_WARD := $(BIN_WARD)
install uninstall: export BIN_WARDD := $(BIN_WARDD)
install uninstall: export BIN_NM := $(BIN_NM)
install uninstall: export BIN_APP := $(BIN_APP)

## release: release artifacts for `make install`, as the user: ward, wardd and ward-nm
## per package (`-p`, without `test-mode`, D63) and the app (`app-front` + Tauri build;
## needs libwebkit2gtk-4.1-dev or PKG_CONFIG_PATH, R32), with V5.6
release: origins app-front
	$(CARGO) build --release --locked -p ward --bin ward
	$(CARGO) build --release --locked -p wardd --bin wardd
	$(CARGO) build --release --locked -p ward-nm --bin ward-nm
	cd app/src-tauri && $(CARGO) build --release --locked
	dev/tests/app-release.sh app/src-tauri/target/release/ward-app

## install: `sudo make install EXTENSION_ID=<id> EXTENSION_CRX=<package.crx>` (after
## `make release`): ward-app/ward-nm groups (D60), binaries (ward-app and ward-nm 2755),
## system units, /run/ward by tmpfiles.d, native messaging manifest, Chrome policy and
## `systemctl enable --now wardd@<SUDO_UID>.socket`. Refuses an artifact with
## `test-mode` (V9.2). DESTDIR=<dir> only copies the tree
install:
	install/install.sh install

## uninstall: `sudo make uninstall`: stops and disables wardd@*, deletes the files and the
## groups; never touches the user's vault (V9.10)
uninstall:
	install/install.sh uninstall

## test-install: V9.1, V9.2, V9.6, V9.10 with DESTDIR in .dev/ (tree, modes, content,
## systemd-analyze verify, refusals) and, in an ubuntu:24.04 container with systemd and
## without network, `make install`/`uninstall` as root: groups (D60, V9.11), units
## without no_new_privs (D57), socket and first connection (V9.7), environment and
## dumpable of wardd (V9.4), app opened by the daemon with the app role, CLI refuses a
## fake server (V9.3). Needs Docker (the image build uses the network), gcc and
## `make dev-vault`; out of `make check`
test-install: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-test-client --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example startup-container --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-platform --features test-mode --example ward-platform-container
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/install.sh

## test-chrome-policy: Phase 51a (V9.5, V9.6; critical finding of the M9 audit): the
## policy generated by install.sh (NativeMessagingUserLevelHosts false) with the real
## Chrome in a bwrap with /etc/opt/chrome swapped for a dir in .dev/chrome-policy.*:
## without it the manifest planted in the profile wins and the audit attack takes
## passwords from the test wardd (control); with it the profile host does not run and the
## system one does. Never touches the real Chrome profile. Needs bwrap, openssl,
## `make dev-vault`, `make dev-variations` and Chrome in /opt/google/chrome; out of
## `make check`
test-chrome-policy: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example startup-container --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	dev/tests/chrome-policy.sh target/debug/wardd target/debug/ward-nm \
		target/no-test-mode/debug/ward target/no-test-mode/debug/wardd \
		target/no-test-mode/debug/ward-nm target/no-test-mode/debug/examples/startup-container \
		target/debug/wardd-aprovador-teste

# --- .deb package (Phase 52; D18; V9.9) -------------------------------------------------
# `make deb` (as the user) builds the staging with `install.sh` (DESTDIR) and packs it
# with `dpkg-deb`; the setgid binaries go 0755 in the package and the 2755 comes from
# the `dpkg-statoverride` of the preinst. `make extension-crx` packs the extension with
# Chrome.
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

## deb: `make deb EXTENSION_ID=<id> EXTENSION_CRX=<package.crx>` (after `make release`),
## without sudo: target/deb/ward_<version>_<arch>.deb with the tree of `make install`,
## Depends with sshpass; install it with `sudo dpkg -i` (guide in docs/instalacao.md)
deb:
	install/deb.sh

## extension-crx: packs extension/dist/ (from `make extension`) into
## target/extension/ward.crx with Chrome and the key .dev/extension/key.pem
## (EXTENSION_KEY); refuses an id other than the extension id constant of ward-nm
extension-crx:
	install/pack-extension.sh

## test-deb: V9.9 (Phase 52): `make deb` with the artifacts without `test-mode` and a
## fake CRX3 (`dpkg-deb -c` list = staging, root/root, no setgid in the package, Depends
## with sshpass, refusals) and, in an ubuntu:24.04 container with systemd and without
## network, `dpkg -i` (missing sshpass, group outside D60, install, statoverride 2755,
## socket, first connection, the user's SSH dir and /etc/ssh untouched, V9.8),
## reinstall, `dpkg -r`/`-P` with the vault untouched. Needs Docker (the image build uses
## the network) and gcc; out of `make check`
test-deb: origins
	$(CARGO) build --quiet --locked -p wardd --bin wardd --example wardd-test-client --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-nm --bin ward-nm --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p ward-app-core --example startup-container --target-dir target/no-test-mode
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/deb.sh

# --- Test daemon (Phase 22) ----------------------------------------------------------
# `wardd` with the `test-mode` feature: vault from `.dev/`, socket in
# $XDG_RUNTIME_DIR/ward-teste/. Starts with `env -i` and a fixed list (HOME, PATH,
# XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR): wardd refuses TMPDIR and LD_* (V3.30).

## dev-daemon: builds and starts the test daemon in the background (log in .dev/logs/),
## with WARD_DEV_APP (D63; default: the test bridge, which the open-app request opens with
## the app role)
TEST_BRIDGE := $(CURDIR)/target/debug/ward-test-bridge
WARD_DEV_APP ?= $(TEST_BRIDGE)
dev-daemon: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-test-bridge --features test-mode --bin ward-test-bridge
	WARD_DEV_APP="$(WARD_DEV_APP)" dev/daemon.sh up

## test-cli: test `ward` CLI in a pty (Phase 25): V4.2 with a fake server, V4.1 and
## V4.22 against the test daemon; needs `make dev-daemon WARD_DEV_APP=` (without the
## bridge: refuses at the start if the test daemon came up with WARD_DEV_APP) and
## `make dev-variations`
test-cli: origins
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	$(PYTHON) dev/tests/cli.py target/debug/ward "$(CURDIR)"

## test-ssh-home: V4.27 (Phase 28b; R19, D13): the `ward ssh` and tunnel suite in a
## bwrap with the passwd home and /tmp on tmpfs, decoys in the home's ssh dir (config
## with Host * and a sentinel ProxyCommand, known_hosts, id_ed25519, FIFOs), a fake
## agent, inotify and the daemon's ssh/sshpass/sftp under strace; positive controls at
## the end. A test `wardd` of its own (its own WARD_DEV_SOCKET_DIR). Needs bwrap, strace,
## `make dev-vault`, `make dev-variations` and the services up; out of `make check`
test-ssh-home: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/ssh-home.sh target/debug/wardd target/debug/ward

## test-playwright: `ward playwright` end to end (Phase 44; V6.12–V6.16, the daemon
## part) with the installed Chrome: the attacks against local sites
## (`cargo test --ignored`) and, with the services up, WordPress and phish against a test
## `wardd` of its own (its own WARD_DEV_SOCKET_DIR). Needs `make dev-vault`,
## `make dev-variations` and the Python Playwright in PW_PYTHONPATH (or
## NO_PLAYWRIGHT_PYTHON=1); out of `make check`
test-playwright: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	$(CARGO) test --quiet --locked -p wardd --all-features --test playwright -- --ignored
	PW_PYTHONPATH="$(PW_PYTHONPATH)" NO_PLAYWRIGHT_PYTHON="$(NO_PLAYWRIGHT_PYTHON)" \
		dev/tests/playwright.sh target/debug/wardd target/debug/ward target/debug/wardd-aprovador-teste

## test-no-test-mode: D63/V3.37: no member turns on test-mode in a normal dependency,
## unit tests of wardd and ward-platform without the feature and release strings without
## WARD_DEV_APP (builds in target/no-test-mode)
test-no-test-mode: origins
	dev/tests/no-test-mode.sh

## test-socket-attacks: V3.3 and V3.4 (M3 scrutiny) against a test `wardd` of its own
## (copy of the test vault in .dev/socket-attacks.*, socket in $XDG_RUNTIME_DIR by
## WARD_DEV_SOCKET_DIR; never the daemon of `make dev-daemon`): bad frames, 10 000 silent
## connections, operation × entry × field matrix and the log without secrets. Needs
## `make dev-vault` and `make dev-variations`; out of `make check`. CONNECTIONS=<n>
## changes the 10 000
CONNECTIONS ?= 10000
test-socket-attacks: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	dev/tests/socket-attacks.sh target/debug/wardd "$(CONNECTIONS)"

## test-nm: Phase 40 (V6.1; D5, D9, D47): the test `ward-nm` opened by hand stays an
## agent and gets no secret; with the real Chrome (headless, temporary profile in
## .dev/nm.*, manifest only in it) the host is a direct child of the browser,
## `verify_parent` accepts that parent and refuses another, and the host answers through
## Chrome. Test daemon of its own (copy of the vault, its own socket). Needs
## `make dev-vault`, `make dev-variations` and Chrome in /opt/google/chrome; out of
## `make check`
## test-skill: Phase 50 (V8.1, V8.2, V8.4): frontmatter of the `ward` skill, commands
## cited in `ward --help`, no pattern of .dev/variations.txt in skill/, valid deny JSON
## with the scope paths, unanchored Bash rules that deny each form of BASH_ATTACKS and no
## example of the skill (Phase 50a); before that, the self-test plants each attack in a
## copy and requires a failure. `ward` (without `test-mode`) goes to target/no-test-mode,
## out of the other tests' target/debug. Needs `make dev-variations`; out of `make check`
test-skill: origins
	$(CARGO) build --quiet --locked -p ward --bin ward --target-dir target/no-test-mode
	$(PYTHON) dev/tests/skill.py target/no-test-mode/debug/ward

## test-migration-guide: Phases 53, 53b and 53d (V10.1–V10.3): order of the steps of
## docs/migracao.md, the items of the setup list one by one, verify and go-back sections
## in each step, nothing for the conversation with Claude, no secret in argv nor pattern
## of .dev/variations.txt, allowlist of commands in the blocks (key only by
## `install -m 600` between the in-memory dir and the home's ssh dir, no `..`,
## find/chmod/grep/busctl limited); before that, the self-test plants each attack in a
## copy and requires a failure. Only reads the guide; runs no step. Needs
## `make dev-variations`; out of `make check`
test-migration-guide:
	$(PYTHON) dev/tests/migration-guide.py

test-nm: origins
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward-platform --example chrome-parent
	dev/tests/nm.sh target/debug/wardd target/debug/ward-nm target/debug/examples/chrome-parent

## extension: builds the Chrome extension in extension/dist/ (Phase 41) with the tokens,
## icons and fonts of the app and verifies the package and the manifest (fixed id =
## ward-nm, no externally_connectable/web_accessible_resources/storage, strict CSP).
## Needs `make app-front` (fonts)
extension:
	$(PYTHON) dev/extension/build.py

## test-extension: Phase 41 (V6.2, V6.4–V6.7): the extension in the real Chrome
## (headless, temporary profile in .dev/extension.*) through the test ward-nm and a test
## wardd of its own with WARD_DEV_EXTENSAO=chrome; suggestions, filling, TOTP, phishing,
## redir, iframe, DOM clobbering, hidden field, page messages, locked; screenshots in
## docs/mission/evidence/test-extension/. Needs `make dev-vault`, `make dev-variations`,
## `make dev-services-up` and `make app-front`; out of `make check`
test-extension: origins extension
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward-nm --example fake-chrome
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/extension.sh target/debug/wardd target/debug/ward-nm target/debug/examples/fake-chrome target/debug/ward target/debug/wardd-aprovador-teste

## test-login: `ward login` end to end (Phase 43; V6.3, V6.10, V6.11, V6.15, V6.16):
## the extension in the real headless Chrome, `ward-nm` opened by the service worker and
## a test `wardd` of its own with `WARD_DEV_EXTENSAO=chrome`; out of `make check`
test-login: origins extension
	$(CARGO) build --quiet --locked -p wardd --features test-mode --bin wardd --bin wardd-aprovador-teste
	$(CARGO) build --quiet --locked -p ward-nm --features test-mode --bin ward-nm
	$(CARGO) build --quiet --locked -p ward --features test-mode --bin ward
	dev/tests/login.sh target/debug/wardd target/debug/ward-nm target/debug/ward target/debug/wardd-aprovador-teste

## dev-daemon-stop: stops the test daemon (verifies the pid really is the wardd from here)
dev-daemon-stop:
	dev/daemon.sh stop
