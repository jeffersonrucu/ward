#!/bin/bash -p
# Testes de ataque da Fundação (Fase 11). Cada ataque tem de FALHAR, isto é, ser pego
# por `make check`. Cobre V1.2 (S15), V1.3 (S14) e V1.5 (S15), mais os achados da
# auditoria do M1 (Fase 12a): varredura extra sem allowlist de caminho, origens do
# cargo (source/patch/registries/path fora) e binário/variáveis das ferramentas; e da
# 2ª auditoria (Fase 12b): origens como lista de permitidas e pré-checagem (alias,
# rustc-wrapper, linker, rustflags, build.rustc, runner, [env], variáveis RUSTC*/
# CARGO_*), config do CARGO_HOME, toolchain, PATH/MAKEFLAGS/BASH_ENV do usuário e git
# sem a config do usuário.
#
# Só usa material gerado na hora em diretório temporário: chave ed25519 nova, segredo
# aleatório, crate de teste. Nada do usuário é lido. O que é criado no repositório
# (`vazamento.txt`) é apagado ao sair, mesmo em erro.
#
# Uso: dev/testes/ataques-fundacao.sh   (ou `make teste-ataques-fundacao`)

set -euo pipefail

# PATH fixo antes de qualquer comando (Fase 12b): nada do PATH do usuário roda aqui.
export PATH=/usr/bin:/bin
RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=dev/ambiente.sh
. "$RAIZ/dev/ambiente.sh"
# O cargo do rustup (não há cargo em /usr/bin), no fim do PATH.
export PATH="$PATH:$CASA_PASSWD/.cargo/bin"
# O gitleaks é sempre o de `.dev/` (não uma variável herdada), conferido por sha256.
GITLEAKS="$RAIZ/.dev/ferramentas/bin/gitleaks"
(cd "$RAIZ" && make -s gitleaks-binario)
# Fase 28d: as cópias saíram do `/tmp`. O `cargo test` da cópia sobe o daemon de teste com
# `WARD_DEV_APP`, e o `BinarioDeTeste` recusa (de propósito) app sob pasta gravável por
# outros, como o `/tmp` (1777): numa cópia em `/tmp` esse teste falha sempre.
# Fase 28f: e saíram também do repositório. Em `.dev/`, os `.cargo/config*` plantados nas
# cópias eram achados pelo `dev/origens.py` da raiz (que varre o repositório inteiro, de
# propósito): durante a rodada todo `make` do checkout falhava em `origens`, e o próprio
# V1.2 (`make check` na raiz) saía 2 sem citar `vazamento.txt`. O `origens.py` continua
# sem exceção para `.dev/`; as cópias vão para `~/.cache/` (home do passwd), numa pasta
# 0700 do `mktemp`, e a cadeia de pastas até `/` é conferida antes (só do usuário ou do
# root, sem escrita de grupo/outros, sem link).
pasta_privada() {
    local p="$1" dono modo
    [ "$(realpath -e -- "$p")" = "$p" ] || { echo "pasta das cópias: $p é link ou não existe" >&2; exit 1; }
    while :; do
        read -r dono modo < <(stat -c '%u %a' -- "$p")
        if { [ "$dono" != "$(id -u)" ] && [ "$dono" != 0 ]; } || (( 8#$modo & 8#022 )); then
            echo "pasta das cópias: $p é de outro dono ou gravável por outros" >&2
            exit 1
        fi
        [ "$p" = / ] && break
        p="$(dirname -- "$p")"
    done
}
CACHE_COPIAS="$CASA_PASSWD/.cache"
[ -L "$CACHE_COPIAS" ] && { echo "pasta das cópias: $CACHE_COPIAS é link" >&2; exit 1; }
mkdir -p -m 0700 "$CACHE_COPIAS"
pasta_privada "$CACHE_COPIAS"
TMP="$(mktemp -d "$CACHE_COPIAS/ward-ataques-fundacao.XXXXXX")"
pasta_privada "$TMP"
case "$TMP/" in
    "$RAIZ"/*) echo "pasta das cópias dentro do repositório: $TMP" >&2; exit 1 ;;
esac
VAZAMENTO="$RAIZ/vazamento.txt"
limpar() { rm -rf "$TMP"; rm -f "$VAZAMENTO"; }
trap limpar EXIT

falhas=0
ok() { printf 'ok    %s\n' "$1"; }
falhou() { printf 'FALHA %s\n' "$1" >&2; falhas=$((falhas + 1)); }

# Copia os arquivos que o git rastreia (`git ls-files`, com o conteúdo da árvore de
# trabalho), sem `.dev/`, `target/` nem `node_modules/`, que o git ignora. Fase 28d: antes
# copiava só `crates/` e alguns arquivos soltos, mas o workspace tem membro em `app/`
# (`app/nucleo`) e os testes leem `dev/fixtures/` por `include_str!`; a cópia quebrava no
# clippy antes dos ataques. Com a lista do git, o que o workspace referencia vai junto.
# O gitleaks vai para o `.dev/` da cópia: o Makefile só usa o dele (`GITLEAKS :=`) e
# confere o sha256 do binário.
copiar_workspace() {
    local destino="$1"
    mkdir -p "$destino/.dev/ferramentas/bin"
    (cd "$RAIZ" && git ls-files -z --cached) \
        | (cd "$RAIZ" && tar --null --no-recursion -T - -cf -) \
        | tar -C "$destino" -xf -
    # Um repositório git vazio na cópia corta a descoberta: o `git`/`gitleaks git` da cópia
    # nunca vê um repositório de cima (o home pode ser um; antes da 28f era o próprio Ward).
    (cd "$destino" && git init -q)
    local essencial
    for essencial in Cargo.toml Cargo.lock Makefile dev/origens.py app/nucleo/Cargo.toml \
        dev/fixtures/entradas.json; do
        [ -f "$destino/$essencial" ] || { echo "copiar_workspace: falta $essencial" >&2; exit 1; }
    done
    cp "$GITLEAKS" "$destino/.dev/ferramentas/bin/gitleaks"
}

# Acrescenta um crate de teste `crates/ataque` com a dependência dada.
adicionar_crate_ataque() {
    local destino="$1" dependencia="$2"
    mkdir -p "$destino/crates/ataque/src"
    cat >"$destino/crates/ataque/Cargo.toml" <<EOF
[package]
name = "ataque"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
$dependencia
EOF
    printf '//! Crate de teste do ataque de dependência.\n' >"$destino/crates/ataque/src/lib.rs"
    sed -i 's|^    "crates/ward-ponte-teste",$|    "crates/ward-ponte-teste",\n    "crates/ataque",|' \
        "$destino/Cargo.toml"
    grep -q '"crates/ataque"' "$destino/Cargo.toml"
    (cd "$destino" && cargo generate-lockfile --quiet)
}

# --- V1.2 (S15): chave privada OpenSSH e segredo AWS num arquivo novo ----------------
v1_2() {
    ssh-keygen -q -t ed25519 -N '' -C 'ward-teste-vazamento' -f "$TMP/chave" </dev/null
    local segredo
    segredo="$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 40)"
    [ "${#segredo}" -eq 40 ]
    {
        cat "$TMP/chave"
        printf 'aws_secret_access_key = %s\n' "$segredo"
    } >"$VAZAMENTO"
    rm -f "$TMP/chave" "$TMP/chave.pub"

    local saida rc=0
    saida="$(cd "$RAIZ" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'vazamento.txt' <<<"$saida"; then
        ok "V1.2 make check sai $rc e aponta vazamento.txt"
    else
        falhou "V1.2 make check não pegou vazamento.txt (exit $rc)"
    fi
    # O relatório não pode repetir o segredo (--redact).
    if grep -qF "$segredo" <<<"$saida"; then
        falhou "V1.2 a saída do make check repete o segredo"
    else
        ok "V1.2 a saída do make check não repete o segredo"
    fi

    rm -f "$VAZAMENTO"
    if (cd "$RAIZ" && make check >"$TMP/v1_2-limpo.log" 2>&1); then
        ok "V1.2 sem o arquivo, make check volta a 0"
    else
        falhou "V1.2 sem o arquivo, make check continua falhando (ver $TMP/v1_2-limpo.log)"
        tail -20 "$TMP/v1_2-limpo.log" >&2
    fi
}

# --- allowlist de D15 casa o valor inteiro: "falso-…" + segredo real é achado -------
allowlist_ancorada() {
    local dir="$TMP/ancora"
    copiar_workspace "$dir"
    printf 'api_secret = "falso-WP-senha-01%s"\n' \
        "$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 24)" >"$dir/burla.txt"
    local rc=0
    (cd "$dir" && "$GITLEAKS" dir --config dev/gitleaks-arvore.toml --redact --no-banner \
        --exit-code 1 . >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then
        ok "allowlist: valor falso com sufixo ainda é achado"
    else
        falhou "allowlist: 'falso-WP-senha-01<segredo>' passou pelo gitleaks"
    fi
}

# --- arquivo de `.dev/` forçado para o índice (git add -f) é achado ----------------
dev_forcado_no_indice() {
    local dir="$TMP/indice"
    copiar_workspace "$dir"
    mkdir -p "$dir/.dev"
    ssh-keygen -q -t ed25519 -N '' -C 'ward-teste-indice' -f "$dir/.dev/chave" </dev/null
    (cd "$dir" && git init -q && git add -f .dev/chave)
    local saida rc=0
    saida="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '.dev/chave' <<<"$saida"; then
        ok "índice: chave em .dev/ forçada com git add -f é achada"
    else
        falhou "índice: chave forçada em .dev/ passou pelo gitleaks (exit $rc)"
    fi
}

# --- V1.3 (S14): dependência com advisory conhecido -------------------------------
v1_3_advisory() {
    local dir="$TMP/advisory"
    copiar_workspace "$dir"
    adicionar_crate_ataque "$dir" 'time = "=0.1.43"'
    local saida rc=0
    saida="$(cd "$dir" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 4/6 cargo audit' <<<"$saida" \
        && grep -q 'RUSTSEC-2020-0071' <<<"$saida"; then
        ok "V1.3 make check falha em cargo audit com RUSTSEC-2020-0071"
    else
        falhou "V1.3 make check não falhou em cargo audit pelo advisory (exit $rc)"
        tail -30 <<<"$saida" >&2
    fi
    # Segunda barreira: cargo deny sozinho também recusa.
    rc=0
    saida="$(cd "$dir" && make deny 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'RUSTSEC-2020-0071' <<<"$saida"; then
        ok "V1.3 cargo deny também recusa RUSTSEC-2020-0071"
    else
        falhou "V1.3 cargo deny aceitou o advisory (exit $rc)"
    fi
}

# --- V1.3 (S14): dependência vinda de git não permitido ---------------------------
v1_3_git() {
    local repo="$TMP/repo-git"
    mkdir -p "$repo/src"
    cat >"$repo/Cargo.toml" <<'EOF'
[package]
name = "falso-git"
version = "0.1.0"
edition = "2024"
license = "MIT"
EOF
    printf '//! Crate falso servido por git.\n' >"$repo/src/lib.rs"
    (cd "$repo" && git init -q && git add -A \
        && git -c user.name=teste -c user.email=teste@teste.invalid commit -q -m teste)

    local dir="$TMP/git"
    copiar_workspace "$dir"
    adicionar_crate_ataque "$dir" "falso-git = { git = \"file://$repo\" }"
    local saida rc=0
    saida="$(cd "$dir" && make check 2>&1)" || rc=$?
    # Fase 12b: a origem git já é recusada na pré-checagem (antes de o clippy compilar o
    # crate, com build.rs, no passo 2); o cargo deny continua recusando também.
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$saida" && ! grep -q '==> 1/6' <<<"$saida" \
        && grep -q "origens: pacote falso-git: fonte fora do crates.io" <<<"$saida"; then
        ok "V1.3 make check recusa a origem git na pré-checagem, antes do passo 1"
    else
        falhou "V1.3 make check não recusou a dependência de git antes do passo 1 (exit $rc)"
        tail -30 <<<"$saida" >&2
    fi
    rc=0
    saida="$(cd "$dir" && "$CASA_PASSWD/.cargo/bin/cargo-deny" --all-features check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q "source-not-allowed" <<<"$saida" && grep -q "falso-git" <<<"$saida"; then
        ok "V1.3 cargo deny também recusa a origem git (source-not-allowed)"
    else
        falhou "V1.3 cargo deny aceitou a dependência de git (exit $rc)"
    fi
}

# --- V1.3 (S14): Cargo.lock existe e não é ignorado pelo git -----------------------
v1_3_lockfile() {
    if [ -f "$RAIZ/Cargo.lock" ] && ! (cd "$RAIZ" && git check-ignore -q Cargo.lock); then
        ok "V1.3 Cargo.lock existe e não é ignorado"
    else
        falhou "V1.3 Cargo.lock ausente ou ignorado pelo git"
    fi
}

# --- V1.5 (S15): .dev/ ignorado; Makefile e scripts não citam pastas do usuário ----
v1_5() {
    local alvo
    for alvo in .dev/qualquer .dev/cofre-teste.ward .dev/ferramentas/bin/gitleaks; do
        if (cd "$RAIZ" && git check-ignore -q "$alvo"); then
            ok "V1.5 $alvo é ignorado"
        else
            falhou "V1.5 $alvo NÃO é ignorado pelo git"
        fi
    done

    # Padrões montados por partes para este arquivo não casar consigo mesmo.
    local padroes=(
        '\.local/share/w''ard'
        '\.s''sh(/|\b)'
        '\.a''ws(/|\b)'
        'keepass''xc'
        'ssh_con''fig'
        'SSH_AUTH''_SOCK'
    )
    local arquivos=("$RAIZ/Makefile")
    while IFS= read -r -d '' f; do arquivos+=("$f"); done \
        < <(find "$RAIZ/dev" -type f -print0)
    local p achou=0
    for p in "${padroes[@]}"; do
        if grep -nEi -- "$p" "${arquivos[@]}" >&2; then
            achou=1
        fi
    done
    if [ "$achou" -eq 0 ]; then
        ok "V1.5 Makefile e dev/ não citam pasta proibida"
    else
        falhou "V1.5 Makefile ou dev/ cita pasta proibida (linhas acima)"
    fi
}

# --- Fase 12a (S15): chave/AWS em caminho que a allowlist global do gitleaks pula --------
# As regras padrão do gitleaks ignoram lockfiles, imagens, svg, pdf, fontes, vendor e
# `gitleaks.toml`, e não abrem arquivo compactado. A varredura extra tem de achar em
# todos, na árvore e no índice.
extra_sem_allowlist_de_caminho() {
    local chave segredo akid
    ssh-keygen -q -t ed25519 -N '' -C 'ward-teste-extra' -f "$TMP/chave-extra" </dev/null
    chave="$(cat "$TMP/chave-extra")"
    rm -f "$TMP/chave-extra" "$TMP/chave-extra.pub"
    segredo="$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 40)"
    akid="AKIA$(head -c 96 /dev/urandom | base64 | tr -dc 'A-Z2-7' | head -c 16)"
    local base="$TMP/extra-base"
    copiar_workspace "$base"
    local f tipo conteudo dir rc saida
    for tipo in chave aws id; do
        case "$tipo" in
            chave) conteudo="$chave" ;;
            aws) conteudo="aws_secret_access_key = $segredo" ;;
            id) conteudo="aws_access_key_id = $akid" ;;
        esac
        for f in app/package-lock.json app/pnpm-lock.yaml Cargo.lock.bak go.sum gitleaks.toml \
            docs/.gitleaks.toml docs/icone.svg docs/foto.png docs/manual.pdf docs/fonte.woff2 \
            vendor/x/README docs/pacote.txt.gz; do
            dir="$TMP/extra"
            rm -rf "$dir"
            cp -a "$base" "$dir"
            mkdir -p "$(dirname "$dir/$f")"
            if [[ "$f" == *.gz ]]; then
                printf '%s\n' "$conteudo" | gzip >"$dir/$f"
            else
                printf '%s\n' "$conteudo" >"$dir/$f"
            fi
            rc=0
            saida="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
            if [ "$rc" -eq 0 ] || ! grep -q "$f" <<<"$saida"; then
                falhou "extra: $tipo em $f passou pela árvore (exit $rc)"
            elif grep -qF -e "$segredo" -e "$akid" <<<"$saida"; then
                falhou "extra: a saída repete o segredo ($f)"
            else
                ok "extra: $tipo em $f é achado na árvore"
            fi
            # Índice: o mesmo arquivo forçado com git add -f.
            (cd "$dir" && git init -q && git add -f "$f")
            rc=0
            (cd "$dir" && "$GITLEAKS" git --staged --pre-commit --config dev/gitleaks-extra.toml \
                --redact --no-banner --max-archive-depth 3 --exit-code 1 . >/dev/null 2>&1) || rc=$?
            if [ "$rc" -ne 0 ]; then ok "extra: $tipo em $f é achado no índice"; else falhou "extra: $tipo em $f passou pelo índice"; fi
        done
    done
    rm -rf "$TMP/extra" "$base"
}

# --- Fase 12a (S14): origens do cargo que o cargo deny não vê ---------------------------
# origem_recusada <descrição> <diretório da cópia>: `make origens` tem de falhar.
origem_recusada() {
    local rc=0 saida
    saida="$(cd "$2" && env "${@:3}" make origens 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '^origens: ' <<<"$saida"; then
        ok "origens: $1 é recusado"
    else
        falhou "origens: $1 passou (exit $rc)"
    fi
}
origens_do_cargo() {
    local dir
    # Substituição de fonte por diretório vendorizado (o ataque da auditoria). Aqui pelo
    # `make check` inteiro: tem de falhar na pré-checagem, antes do passo 1.
    dir="$TMP/vendor"
    copiar_workspace "$dir"
    mkdir -p "$dir/.cargo" "$dir/vendor"
    printf '[source.crates-io]\nreplace-with = "v"\n\n[source.v]\ndirectory = "vendor"\n' >"$dir/.cargo/config.toml"
    local saida rc=0
    saida="$(cd "$dir" && make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$saida" && ! grep -q '==> 1/6' <<<"$saida" \
        && grep -q '.cargo/config.toml: .cargo/config\* no repositório é proibido' <<<"$saida"; then
        ok "origens: make check falha na pré-checagem (antes do passo 1) com replace-with para vendor/"
    else
        falhou "origens: make check aceitou a substituição de fonte (exit $rc)"
        tail -20 <<<"$saida" >&2
    fi
    # Chave pontilhada (sem cabeçalho de tabela `[source`).
    dir="$TMP/pontilhada"; copiar_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf 'source.crates-io.replace-with = "v"\nsource.v.directory = "vendor"\n' >"$dir/.cargo/config.toml"
    origem_recusada "source em chave pontilhada" "$dir"
    # `.cargo/config` sem extensão, com [patch].
    dir="$TMP/config-antigo"; copiar_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[patch.crates-io]\nitoa = { path = "x" }\n' >"$dir/.cargo/config"
    origem_recusada ".cargo/config com [patch]" "$dir"
    # Registro alternativo.
    dir="$TMP/registro"; copiar_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[registries.mau]\nindex = "sparse+https://example.invalid/index/"\n' >"$dir/.cargo/config.toml"
    origem_recusada "[registries]" "$dir"
    # Config num diretório pai da raiz (o cargo também lê).
    mkdir -p "$TMP/pai/.cargo"
    printf '[source.crates-io]\nreplace-with = "v"\n[source.v]\ndirectory = "/nao/existe"\n' >"$TMP/pai/.cargo/config.toml"
    dir="$TMP/pai/filho"; copiar_workspace "$dir"
    origem_recusada "source num .cargo/config.toml de diretório pai" "$dir"
    # TOML inválido não passa em silêncio.
    dir="$TMP/invalido"; copiar_workspace "$dir"; mkdir -p "$dir/.cargo"
    printf '[source\n' >"$dir/.cargo/config.toml"
    origem_recusada ".cargo/config.toml inválido" "$dir"
    # Variável de ambiente.
    dir="$TMP/ambiente"; copiar_workspace "$dir"
    origem_recusada "CARGO_REGISTRIES_MAU_INDEX no ambiente" "$dir" CARGO_REGISTRIES_MAU_INDEX=sparse+https://example.invalid/
    # [patch] e [replace] no Cargo.toml do workspace.
    dir="$TMP/patch"; copiar_workspace "$dir"
    printf '\n[patch.crates-io]\nward-falso = { path = "crates/ward-core" }\n' >>"$dir/Cargo.toml"
    origem_recusada "[patch] no Cargo.toml" "$dir"
    dir="$TMP/replace"; copiar_workspace "$dir"
    printf '\n[replace]\n"itoa:1.0.15" = { path = "crates/ward-core" }\n' >>"$dir/Cargo.toml"
    origem_recusada "[replace] no Cargo.toml" "$dir"
    # Dependência por path fora do repositório.
    mkdir -p "$TMP/fora/src"
    printf '[package]\nname = "fora"\nversion = "0.1.0"\nedition = "2024"\nlicense = "MIT"\n' >"$TMP/fora/Cargo.toml"
    printf '//! Crate fora do repositório.\n' >"$TMP/fora/src/lib.rs"
    dir="$TMP/path-fora"; copiar_workspace "$dir"
    adicionar_crate_ataque "$dir" "fora = { path = \"$TMP/fora\" }"
    origem_recusada "dependência por path fora do repositório" "$dir"
    # Path relativo que sai da raiz por `..`.
    dir="$TMP/path-rel"; copiar_workspace "$dir"
    adicionar_crate_ataque "$dir" 'fora = { path = "../../../fora" }'
    origem_recusada "path relativo com .. para fora do repositório" "$dir"
    # Controle: a cópia limpa passa.
    dir="$TMP/limpa"; copiar_workspace "$dir"
    if (cd "$dir" && make origens >/dev/null 2>&1); then ok "origens: cópia limpa passa"; else falhou "origens: cópia limpa recusada"; fi
}

# --- Fase 12a: ferramenta trocada (binário do gitleaks e variáveis de ambiente) ----------
ferramentas_fixas() {
    local dir="$TMP/ferramentas" rc saida
    copiar_workspace "$dir"
    ssh-keygen -q -t ed25519 -N '' -C 'ward-teste-ferr' -f "$dir/vazou" </dev/null
    rm -f "$dir/vazou.pub"
    # GITLEAKS no ambiente não troca o gitleaks do Makefile: o vazamento continua achado.
    rc=0
    (cd "$dir" && GITLEAKS=/bin/true make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "GITLEAKS=/bin/true no ambiente não desliga o gitleaks"; else falhou "GITLEAKS do ambiente trocou o gitleaks"; fi
    # CARGO no ambiente não troca o cargo: com CARGO=false o fmt-check ainda roda e passa.
    if (cd "$dir" && CARGO=false make fmt-check >/dev/null 2>&1); then ok "CARGO=false no ambiente não troca o cargo"; else falhou "CARGO do ambiente trocou o cargo"; fi
    # Binário do gitleaks adulterado em .dev/: recusado antes de varrer.
    rm -f "$dir/vazou"
    printf '#!/bin/sh\nexit 0\n' >"$dir/.dev/ferramentas/bin/gitleaks"
    rc=0
    saida="$(cd "$dir" && make gitleaks 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'não é o gitleaks' <<<"$saida"; then
        ok "gitleaks trocado em .dev/ é recusado pelo sha256"
    else
        falhou "gitleaks trocado em .dev/ rodou (exit $rc)"
    fi
    cp "$GITLEAKS" "$dir/.dev/ferramentas/bin/gitleaks"
    printf 'x' >>"$dir/.dev/ferramentas/bin/gitleaks"
    rc=0
    (cd "$dir" && make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "gitleaks com um byte a mais é recusado"; else falhou "gitleaks com um byte a mais rodou"; fi
}

# --- Fase 12b (S14): origens como lista de permitidas, antes do passo 1 ---------------------
# Os vetores da 2ª auditoria (scratchpad/r2: v13-alias.sh, ataque-config.sh, wrap.sh): um
# `.cargo/config.toml` ou uma variável de ambiente que troca o que o cargo executa. Cada um
# roda o `make check` inteiro numa cópia e tem de falhar na pré-checagem `0/6`, antes do
# passo 1, sem que o programa plantado rode (a marca não pode aparecer).
MARCAS="$TMP/marcas"
preparar_iscas_cargo() {
    mkdir -p "$TMP/iscas"
    # wrap.sh da auditoria: troca o fonte do ward-core que o rustc compila.
    cat >"$TMP/iscas/wrap.sh" <<EOF
#!/bin/bash
echo "wrapper \$*" | head -c 80 >>"$MARCAS"; echo >>"$MARCAS"
args=()
for a in "\$@"; do
    if [[ "\$a" == *crates/ward-core/src/lib.rs ]]; then
        printf '//! adulterado\npub fn porta_dos_fundos() -> u8 { 42 }\n' >"$TMP/iscas/lib.rs"; a="$TMP/iscas/lib.rs"
    fi
    args+=("\$a")
done
exec "\${args[@]}"
EOF
    printf '#!/bin/bash\necho rustc >>"%s"\nexec "%s/.cargo/bin/rustc" "$@"\n' "$MARCAS" "$CASA_PASSWD" >"$TMP/iscas/rustc.sh"
    printf '#!/bin/bash\necho linker >>"%s"\nexec cc "$@"\n' "$MARCAS" >"$TMP/iscas/linker.sh"
    printf '#!/bin/bash\necho programa >>"%s"\nexit 0\n' "$MARCAS" >"$TMP/iscas/programa.sh"
    chmod +x "$TMP/iscas/"*.sh
    # falso-ok do v13-alias.sh: um "audit"/"deny" que só diz que está tudo limpo.
    mkdir -p "$TMP/falso-ok/src"
    printf '[package]\nname = "falso-ok"\nversion = "0.1.0"\nedition = "2021"\n[workspace]\n' >"$TMP/falso-ok/Cargo.toml"
    printf 'fn main() { println!("audit/deny falsos: tudo limpo"); }\n' >"$TMP/falso-ok/src/main.rs"
}

# barrado_antes <descrição> <cópia> [VAR=valor...]: `make check` sai ≠ 0 na pré-checagem,
# antes do passo 1, e nada plantado rodou.
barrado_antes() {
    local d="$1" dir="$2" rc=0 saida
    shift 2
    rm -f "$MARCAS"
    saida="$(cd "$dir" && env "$@" make check 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q '==> 0/6 origens' <<<"$saida" && ! grep -q '==> 1/6' <<<"$saida" \
        && grep -q '^origens: ' <<<"$saida" && [ ! -e "$MARCAS" ]; then
        ok "pré-checagem: $d é barrado antes do passo 1"
    else
        falhou "pré-checagem: $d passou ou rodou o programa plantado (exit $rc, marca: $([ -e "$MARCAS" ] && echo sim || echo não))"
        grep -E '==>|^origens' <<<"$saida" | tail -5 >&2
    fi
}

# config_na_copia <nome> <conteúdo> [arquivo relativo]: cópia com aquele config.
config_na_copia() {
    local dir="$TMP/cfg-$1"
    rm -rf "$dir"
    copiar_workspace "$dir"
    local arq="${3:-.cargo/config.toml}"
    mkdir -p "$(dirname "$dir/$arq")"
    printf '%s\n' "$2" >"$dir/$arq"
    printf '%s' "$dir"
}

origens_lista_permitida() {
    preparar_iscas_cargo
    local dir alvo="x86_64-unknown-linux-gnu"
    # v13-alias.sh: alias de `audit`/`deny` com uma dependência de advisory conhecido.
    dir="$(config_na_copia alias-run "[alias]
audit = [\"run\", \"-q\", \"--manifest-path\", \"$TMP/falso-ok/Cargo.toml\", \"--\"]
deny = [\"run\", \"-q\", \"--manifest-path\", \"$TMP/falso-ok/Cargo.toml\", \"--\"]")"
    adicionar_crate_ataque "$dir" 'time = "=0.1.43"'
    barrado_antes "[alias] audit/deny para um falso-ok (v13-alias), com RUSTSEC-2020-0071" "$dir"
    # O audit e o deny vão pelo binário: mesmo sem a pré-checagem, o alias não pegaria.
    local rc=0 saida
    saida="$(cd "$dir" && "$CASA_PASSWD/.cargo/bin/cargo-audit" audit --deny warnings 2>&1)" || rc=$?
    if [ "$rc" -ne 0 ] && grep -q 'RUSTSEC-2020-0071' <<<"$saida" && ! grep -q 'falsos' <<<"$saida"; then
        ok "cargo-audit pelo caminho absoluto ignora o [alias] e acha RUSTSEC-2020-0071"
    else
        falhou "cargo-audit pelo caminho absoluto passou pelo alias (exit $rc)"
    fi
    barrado_antes "[alias] audit/deny = version" "$(config_na_copia alias "[alias]
audit = \"version\"
deny = \"version\"")"
    barrado_antes "build.rustc" "$(config_na_copia build-rustc "[build]
rustc = \"$TMP/iscas/rustc.sh\"")"
    barrado_antes "build.rustc-wrapper (wrap.sh)" "$(config_na_copia rustc-wrapper "[build]
rustc-wrapper = \"$TMP/iscas/wrap.sh\"")"
    barrado_antes "build.rustc-workspace-wrapper" "$(config_na_copia ws-wrapper "[build]
rustc-workspace-wrapper = \"$TMP/iscas/wrap.sh\"")"
    barrado_antes "target.<triplo>.linker" "$(config_na_copia linker "[target.$alvo]
linker = \"$TMP/iscas/linker.sh\"")"
    barrado_antes "build.rustflags com -C linker" "$(config_na_copia rustflags "[build]
rustflags = [\"-C\", \"linker=$TMP/iscas/linker.sh\"]")"
    barrado_antes "target.<triplo>.runner" "$(config_na_copia runner "[target.$alvo]
runner = \"$TMP/iscas/programa.sh\"")"
    barrado_antes "[env]" "$(config_na_copia env "[env]
RUSTC_BOOTSTRAP = \"1\"")"
    barrado_antes "chave pontilhada build.rustc-wrapper" "$(config_na_copia pontilhada "build.rustc-wrapper = \"$TMP/iscas/wrap.sh\"")"
    # Qualquer `.cargo/config*` no repositório, mesmo com chave inofensiva.
    barrado_antes ".cargo/config.toml só com [term]" "$(config_na_copia term "[term]
color = \"never\"")"
    barrado_antes ".cargo/config sem extensão" "$(config_na_copia antigo "[term]
color = \"never\"" .cargo/config)"
    barrado_antes ".cargo/config.toml numa subpasta (crates/ward-core)" \
        "$(config_na_copia sub "[term]
color = \"never\"" crates/ward-core/.cargo/config.toml)"
    barrado_antes ".cargo/config.bak (qualquer config*)" "$(config_na_copia bak "x" .cargo/config.bak)"
    dir="$TMP/cfg-link"; rm -rf "$dir"; copiar_workspace "$dir"
    mkdir -p "$TMP/fora-cargo"; printf '[build]\nrustc-wrapper = "%s"\n' "$TMP/iscas/wrap.sh" >"$TMP/fora-cargo/config.toml"
    ln -s "$TMP/fora-cargo" "$dir/crates/.cargo"
    barrado_antes ".cargo como link simbólico para fora" "$dir"
    # Config num diretório pai, mesmo inofensivo.
    mkdir -p "$TMP/pai2/.cargo"; printf '[term]\ncolor = "never"\n' >"$TMP/pai2/.cargo/config.toml"
    dir="$TMP/pai2/filho"; copiar_workspace "$dir"
    barrado_antes ".cargo/config.toml inofensivo em diretório pai" "$dir"

    # Variáveis de ambiente (ataque-config.sh com VAR=valor).
    dir="$TMP/env"; rm -rf "$dir"; copiar_workspace "$dir"
    local v
    for v in "RUSTC=$TMP/iscas/rustc.sh" "RUSTC_WRAPPER=$TMP/iscas/wrap.sh" \
        "RUSTC_WORKSPACE_WRAPPER=$TMP/iscas/wrap.sh" "CARGO_BUILD_RUSTC_WRAPPER=$TMP/iscas/wrap.sh" \
        "CARGO_BUILD_RUSTC=$TMP/iscas/rustc.sh" "RUSTFLAGS=-Clinker=$TMP/iscas/linker.sh" \
        "CARGO_BUILD_RUSTFLAGS=-Clinker=$TMP/iscas/linker.sh" \
        "CARGO_ENCODED_RUSTFLAGS=-Clinker=$TMP/iscas/linker.sh" "RUSTDOCFLAGS=-Zunstable-options" \
        "RUSTDOC=$TMP/iscas/programa.sh" "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$TMP/iscas/linker.sh" \
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER=$TMP/iscas/programa.sh" "CARGO_ALIAS_AUDIT=version" \
        "CARGO_TARGET_DIR=$TMP/alvo" "RUSTC_BOOTSTRAP=1" "RUSTUP_TOOLCHAIN=stable" \
        "RUSTUP_DIST_SERVER=https://example.invalid" "CARGO_UNSTABLE_BUILD_STD=true"; do
        barrado_antes "${v%%=*} no ambiente" "$dir" "$v"
    done
    # Neutralizadas pelo Makefile (override + export): não são recusadas, mas não valem.
    rm -f "$MARCAS"
    mkdir -p "$TMP/home-cargo"
    printf '[build]\nrustc-wrapper = "%s"\n' "$TMP/iscas/wrap.sh" >"$TMP/home-cargo/config.toml"
    rc=0
    (cd "$dir" && CARGO_HOME="$TMP/home-cargo" RUSTUP_HOME="$TMP/home-rustup" CARGO="$TMP/iscas/programa.sh" \
        make build >/dev/null 2>&1) || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARCAS" ]; then
        ok "CARGO_HOME/RUSTUP_HOME/CARGO do ambiente não valem (o Makefile fixa os do passwd)"
    else
        falhou "CARGO_HOME/RUSTUP_HOME/CARGO do ambiente mudaram o build (exit $rc)"
    fi
    # E o origens recusa o valor diferente quando chamado sem o Makefile.
    rc=0
    (cd "$dir" && CARGO_HOME="$TMP/home-cargo" /usr/bin/python3 -I -B dev/origens.py >/dev/null 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then ok "origens.py sozinho recusa CARGO_HOME diferente do esperado"; else falhou "origens.py aceitou CARGO_HOME de fora"; fi
    # Controle: variável inofensiva da lista passa.
    if (cd "$dir" && CARGO_TERM_COLOR=never make origens >/dev/null 2>&1); then
        ok "pré-checagem: CARGO_TERM_COLOR (da lista) passa"
    else
        falhou "pré-checagem: CARGO_TERM_COLOR recusado"
    fi

    # rust-toolchain.toml: `path` aponta o rustc para qualquer pasta; canal tem de ser fixo.
    dir="$TMP/tc-path"; rm -rf "$dir"; copiar_workspace "$dir"
    printf '[toolchain]\npath = "%s"\n' "$TMP/iscas" >"$dir/rust-toolchain.toml"
    barrado_antes "rust-toolchain.toml com path" "$dir"
    dir="$TMP/tc-canal"; rm -rf "$dir"; copiar_workspace "$dir"
    sed -i 's/^channel = .*/channel = "nightly"/' "$dir/rust-toolchain.toml"
    barrado_antes "rust-toolchain.toml com canal móvel (nightly)" "$dir"
    dir="$TMP/tc-legado"; rm -rf "$dir"; copiar_workspace "$dir"
    printf 'nightly\n' >"$dir/rust-toolchain"
    barrado_antes "rust-toolchain sem extensão" "$dir"

    # Controle: o vetor é real. Sem a pré-checagem, `cargo build` direto na cópia com o
    # rustc-wrapper roda o wrap.sh (a marca aparece).
    rm -f "$MARCAS"
    dir="$TMP/cfg-rustc-wrapper"
    (cd "$dir" && rm -rf crates/ataque && cargo build --quiet --offline >/dev/null 2>&1) || true
    if [ -s "$MARCAS" ]; then
        ok "controle: sem a pré-checagem o rustc-wrapper plantado roda (o vetor é real)"
    else
        falhou "controle: o rustc-wrapper plantado não rodou nem sem a pré-checagem (teste sem efeito)"
    fi
}

# --- Fase 12b (S14): config do CARGO_HOME só com chaves inofensivas -------------------------
# Num bwrap com o home do passwd coberto por tmpfs (o ~/.cargo real nunca é tocado): só o
# rustup e os binários de ~/.cargo/bin voltam, somente leitura, e o `$TMP` deste script
# (Fases 28d/28f: fica em `~/.cache/`, no home). O `config.toml` do CARGO_HOME
# é escrito no tmpfs. Workspace mínimo sem dependências (o cargo metadata não precisa do
# registro). Também prova a checagem do toolchain ativo com um `rustup override` simulado.
config_do_cargo_home() {
    local m="$TMP/minimo"
    mkdir -p "$m/dev" "$m/a/src"
    cp "$RAIZ/dev/origens.py" "$m/dev/"
    cp "$RAIZ/rust-toolchain.toml" "$m/"
    printf '[workspace]\nmembers = ["a"]\nresolver = "3"\n' >"$m/Cargo.toml"
    printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2024"\n' >"$m/a/Cargo.toml"
    : >"$m/a/src/lib.rs"
    (cd "$m" && cargo generate-lockfile --quiet --offline)
    cat >"$TMP/no-home.sh" <<'EOF'
#!/bin/bash -p
# Dentro do bwrap: confere o tmpfs, escreve o config do CARGO_HOME e roda o origens.
set -u
casa="$1" m="$2" conteudo="$3"
[ "$(stat -f -c %T "$casa")" = tmpfs ] || { echo "home não coberto" >&2; exit 9; }
mkdir -p "$casa/.cargo"
[ -n "$conteudo" ] && printf '%s\n' "$conteudo" >"$casa/.cargo/config.toml"
exec /usr/bin/python3 -I -B "$m/dev/origens.py"
EOF
    no_home() { # conteúdo [settings.toml do rustup]
        local extra=()
        [ -n "${2:-}" ] && extra=(--ro-bind "$2" "$CASA_PASSWD/.rustup/settings.toml")
        /usr/bin/env -i PATH=/usr/bin:/bin bwrap --die-with-parent --dev-bind / / --tmpfs "$CASA_PASSWD" \
            --ro-bind "$CASA_PASSWD/.rustup" "$CASA_PASSWD/.rustup" "${extra[@]}" \
            --ro-bind "$CASA_PASSWD/.cargo/bin" "$CASA_PASSWD/.cargo/bin" --bind /tmp /tmp \
            --bind "$TMP" "$TMP" \
            -- /bin/bash -p "$TMP/no-home.sh" "$CASA_PASSWD" "$m" "$1"
    }
    local c rc
    for c in '[alias]
audit = "version"' "[build]
rustc-wrapper = \"$TMP/iscas/wrap.sh\"" "[build]
rustc = \"$TMP/iscas/rustc.sh\"" "[target.x86_64-unknown-linux-gnu]
linker = \"$TMP/iscas/linker.sh\"" '[build]
rustflags = ["-Cdebuginfo=0"]' '[env]
RUSTC_BOOTSTRAP = "1"' '[source.crates-io]
replace-with = "v"' '[registries.mau]
index = "sparse+https://example.invalid/"' '[http]
proxy = "http://127.0.0.1:9"' '[net]
git-fetch-with-cli = true' 'include = "outro.toml"' '[term]
color = ["never"]' '[source'; do
        rc=0
        no_home "$c" >"$TMP/no-home.out" 2>&1 || rc=$?
        if [ "$rc" -eq 1 ] && grep -q '^origens: .*config' "$TMP/no-home.out"; then
            ok "CARGO_HOME: config com '$(tr '\n' ' ' <<<"$c" | sed 's/ $//')' é recusado"
        else
            falhou "CARGO_HOME: config com '$(tr '\n' ' ' <<<"$c" | sed 's/ $//')' passou (exit $rc)"
        fi
    done
    # Controles: sem config e com chaves da lista, passa.
    for c in '' '[term]
color = "never"
progress.when = "never"
[net]
retry = 3
[build]
jobs = 4'; do
        rc=0
        no_home "$c" >"$TMP/no-home.out" 2>&1 || rc=$?
        local desc="ausente"
        [ -n "$c" ] && desc="só com chaves permitidas (term, net.retry, build.jobs)"
        if [ "$rc" -eq 0 ]; then ok "CARGO_HOME: config $desc passa"; else falhou "CARGO_HOME: config permitido recusado"; cat "$TMP/no-home.out" >&2; fi
    done
    # `rustup override set` para a pasta: o toolchain ativo deixa de ser o do arquivo.
    printf 'version = "12"\ndefault_toolchain = "stable-x86_64-unknown-linux-gnu"\nprofile = "minimal"\n\n[overrides]\n"%s" = "stable-x86_64-unknown-linux-gnu"\n' \
        "$m" >"$TMP/settings.toml"
    rc=0
    no_home '' "$TMP/settings.toml" >"$TMP/no-home.out" 2>&1 || rc=$?
    if [ "$rc" -eq 1 ] && grep -q 'toolchain ativo' "$TMP/no-home.out"; then
        ok "rustup override de diretório é recusado"
    else
        falhou "rustup override de diretório passou (exit $rc)"
    fi
}

# --- Fase 12b: ambiente do usuário não troca o que o make check executa ---------------------
# PATH com cargo/python3/git/bash/sha256sum falsos na frente, MAKEFLAGS com CARGO/GITLEAKS
# trocados (vence `:=`, não vence `override`), BASH_ENV, HOME falso e um ~/.gitconfig com
# core.fsmonitor (roda programa a cada `git diff`, inclusive no gitleaks). O `make check`
# inteiro numa cópia tem de passar sem nenhuma marca.
ambiente_do_make() {
    local falsos="$TMP/falsos" p
    mkdir -p "$falsos"
    for p in cargo cargo-audit cargo-deny python3 git bash sh sha256sum gitleaks docker rustc make; do
        printf '#!/bin/sh\necho "%s" >>"%s"\nexit 0\n' "$p" "$MARCAS" >"$falsos/$p"
        chmod +x "$falsos/$p"
    done
    # o próprio `make` tem de ser o do sistema: chamado pelo caminho absoluto abaixo.
    printf 'echo bash_env >>"%s"\n' "$MARCAS" >"$TMP/bash_env"
    printf '[core]\n\tfsmonitor = %s\n' "$TMP/iscas/programa.sh" >"$TMP/gitconfig-mau"
    mkdir -p "$TMP/xdg/git"; printf '*\n' >"$TMP/xdg/git/ignore"
    local dir="$TMP/amb"
    copiar_workspace "$dir"
    (cd "$dir" && git init -q && git add -A)
    rm -f "$MARCAS"
    local rc=0 saida
    saida="$(cd "$dir" && /usr/bin/env PATH="$falsos:$PATH" BASH_ENV="$TMP/bash_env" ENV="$TMP/bash_env" \
        MAKEFLAGS="CARGO=$falsos/cargo GITLEAKS=$falsos/gitleaks CARGO_AUDIT=$falsos/cargo-audit PYTHON=$falsos/python3" \
        HOME="$TMP/home-falso" GIT_CONFIG_GLOBAL="$TMP/gitconfig-mau" XDG_CONFIG_HOME="$TMP/xdg" \
        /usr/bin/make check 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARCAS" ] && grep -q '==> make check: tudo limpo' <<<"$saida"; then
        ok "PATH/MAKEFLAGS/BASH_ENV/HOME/gitconfig do usuário não trocam nada no make check"
    else
        falhou "o ambiente do usuário mudou o make check (exit $rc, marcas: $(tr '\n' ' ' <"$MARCAS" 2>/dev/null))"
        tail -15 <<<"$saida" >&2
    fi
    # Controle: o fsmonitor plantado roda num `git diff` comum (o vetor é real).
    rm -f "$MARCAS"
    (cd "$dir" && /usr/bin/env -u GIT_CONFIG_COUNT GIT_CONFIG_GLOBAL="$TMP/gitconfig-mau" /usr/bin/git diff --staged --quiet) >/dev/null 2>&1 || true
    if [ -s "$MARCAS" ]; then ok "controle: core.fsmonitor do gitconfig roda sem o isolamento"; else falhou "controle: fsmonitor não rodou nem sem isolamento"; fi
    # O mesmo pelo .git/config local: a linha de comando (GIT_CONFIG_COUNT) vence.
    rm -f "$MARCAS"
    (cd "$dir" && /usr/bin/git config core.fsmonitor "$TMP/iscas/programa.sh")
    rc=0
    (cd "$dir" && make gitleaks >/dev/null 2>&1) || rc=$?
    if [ "$rc" -eq 0 ] && [ ! -e "$MARCAS" ]; then
        ok "core.fsmonitor no .git/config não roda no make gitleaks"
    else
        falhou "core.fsmonitor do .git/config rodou no make gitleaks (exit $rc)"
    fi
}

# --- Fase 12b: git dos testes sem a config do usuário -------------------------------------
# dev/ambiente.sh: um ignore global (~/.config/git/ignore) faria `git check-ignore` do V1.5
# passar sem o .gitignore do repositório.
git_sem_config_do_usuario() {
    local dir="$TMP/git-isolado" rc=0
    mkdir -p "$dir"
    (cd "$dir" && /usr/bin/git init -q)
    printf '[core]\n\texcludesFile = %s/xdg/git/ignore\n' "$TMP" >"$TMP/gitconfig-ignora"
    /usr/bin/env GIT_CONFIG_GLOBAL="$TMP/gitconfig-ignora" XDG_CONFIG_HOME="$TMP/xdg" HOME="$TMP" \
        /bin/bash -p -c '. "$1/dev/ambiente.sh"; cd "$2" && git check-ignore -q qualquer.txt' _ "$RAIZ" "$dir" || rc=$?
    if [ "$rc" -eq 1 ]; then
        ok "git dos testes ignora o ignore global do usuário (check-ignore depende só do repositório)"
    else
        falhou "git dos testes leu o ignore global do usuário (exit $rc)"
    fi
    # Controle: sem o ambiente.sh, o mesmo git diz que está ignorado.
    rc=0
    (cd "$dir" && /usr/bin/env -u GIT_CONFIG_COUNT -u GIT_CONFIG_GLOBAL XDG_CONFIG_HOME="$TMP/xdg" HOME="$TMP" \
        /usr/bin/git check-ignore -q qualquer.txt) || rc=$?
    if [ "$rc" -eq 0 ]; then ok "controle: sem o isolamento o ignore global vale"; else falhou "controle: ignore global não valeu nem sem isolamento"; fi
}

# --- Fase 28f: as cópias com `.cargo/config*` plantado não sujam o `origens` da raiz ------
# Com as cópias da rodada ainda no disco (só os `target/` saem), o `make origens` do
# checkout tem de continuar limpo. Antes da 28f elas ficavam em `.dev/` e todo `make` do
# repositório falhava em `origens` enquanto o teste rodava.
origens_da_raiz_durante_a_rodada() {
    local plantados rc=0 saida
    plantados="$(find "$TMP" -path '*/.cargo/config*' 2>/dev/null | wc -l)"
    if [ "$plantados" -eq 0 ]; then
        falhou "28f: nenhuma cópia com .cargo/config* plantado no disco (checagem sem efeito)"
        return
    fi
    saida="$(cd "$RAIZ" && make -s origens 2>&1)" || rc=$?
    if [ "$rc" -eq 0 ]; then
        ok "28f: make origens da raiz limpo com $plantados .cargo/config* plantados nas cópias"
    else
        falhou "28f: make origens da raiz falhou durante a rodada (exit $rc)"
        tail -10 <<<"$saida" >&2
    fi
}

# Fase 28d: cada cópia compila o workspace inteiro no próprio `target/`; apaga os
# `target/` das cópias depois de cada ataque para não somar vários GB no disco.
rodar() {
    "$1"
    rm -rf "$TMP"/*/target
}

rodar v1_2
rodar allowlist_ancorada
rodar dev_forcado_no_indice
rodar v1_3_advisory
rodar v1_3_git
rodar v1_3_lockfile
rodar v1_5
rodar extra_sem_allowlist_de_caminho
rodar origens_do_cargo
rodar ferramentas_fixas
rodar origens_lista_permitida
rodar config_do_cargo_home
rodar origens_da_raiz_durante_a_rodada
rodar ambiente_do_make
rodar git_sem_config_do_usuario
rodar origens_da_raiz_durante_a_rodada

if [ "$falhas" -ne 0 ]; then
    printf '\n%d ataque(s) passaram: corrigir antes de seguir.\n' "$falhas" >&2
    exit 1
fi
printf '\nTodos os ataques foram barrados.\n'
