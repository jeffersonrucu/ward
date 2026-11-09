# Fase 12b — Achados médios e baixos da 2ª auditoria do M1

## Implementado
- **`origens` virou lista de permitidas e roda como pré-checagem.** O `make check` tem
  agora um passo `==> 0/6 origens do cargo (pré-checagem…)` antes do passo 1. Os seis
  passos de V1.1 continuam na mesma ordem, e o rótulo do 5/6 voltou a ser
  `cargo deny check`. Os alvos `build`, `fmt`, `fmt-check`, `clippy`, `test`, `audit`,
  `deny` e `ferramentas` dependem de `origens`, então nem `make clippy` sozinho roda com
  config plantado. `dev/origens.py` recusa:
  1. qualquer `.cargo/config*` dentro do repositório, em qualquer pasta (pula só `.git`,
     `target` e `node_modules`), e `.cargo` que seja link simbólico;
  2. `.cargo/config*` em diretório pai da raiz que não seja o CARGO_HOME;
  3. no config do CARGO_HOME, chave fora de `PERMITIDAS_HOME`. A lista tem `term.*`,
     `net.retry`/`offline`, `http.timeout`/`low-speed-limit`/`multiplexing`,
     `build.jobs`/`incremental`, `cargo-new.vcs`, `future-incompat-report.frequency` e
     `cache.auto-clean-frequency`, todas só com valor simples. TOML inválido também é
     recusado;
  4. variável `CARGO*`/`RUSTC*`/`RUSTDOC*`/`RUSTFLAGS*`/`RUSTUP*` fora de `ENV_PERMITIDAS`.
     `CARGO_HOME`, `RUSTUP_HOME` e `CARGO` só passam com o valor do home do passwd. As
     de aparência e rede (`CARGO_TERM_*`, `CARGO_NET_RETRY`, `CARGO_HTTP_TIMEOUT`,
     `CARGO_BUILD_JOBS`, `CARGO_INCREMENTAL`) passam com qualquer valor;
  5. `rust-toolchain.toml` com chave fora de `channel/components/profile/targets` (ex.:
     `path`), canal que não seja versão fixa, ou um `rust-toolchain` sem extensão. Também
     recusa quando o toolchain ativo do rustup não vem desse arquivo (`rustup override set`);
  6. o que já recusava antes: `[patch]`/`[replace]`, path fora do repositório e fonte
     fora do crates.io.

  Os itens 1 a 5 rodam sem executar cargo nem rustc, e o script para ali se achar algo.
  O motivo: o próprio `cargo metadata` rodaria um `build.rustc` plantado.
- **Ferramentas pelo caminho absoluto, sem troca possível.** O Makefile tira o home do
  passwd com `/usr/bin/getent` e não usa o `$HOME`. Ele fixa:
  - `CARGO`, `CARGO_AUDIT`, `CARGO_DENY` e `PYTHON` com caminho absoluto;
  - `audit` como `cargo-audit audit …` e `deny` como `cargo-deny --all-features check`,
    sem passar pelo `[alias]`;
  - `CARGO_HOME`, `RUSTUP_HOME` e `CARGO`, exportados;
  - `PATH=/usr/bin:/bin:<passwd>/.cargo/bin`;
  - `GITLEAKS` e os sha256.

  Tudo isso com `override`. Achei que `MAKEFLAGS='CARGO=…'` herdado do ambiente vencia
  o `:=` da Fase 12a (ver "Problemas"). Com `override`, nem o ambiente, nem `MAKEFLAGS`,
  nem `make VAR=…` trocam esses valores.
- **Shell sem nada do ambiente.** O Makefile usa `.SHELLFLAGS := -p …` e
  `unexport BASH_ENV ENV CDPATH GLOBIGNORE`. Todos os scripts bash de `dev/` passaram de
  `#!/usr/bin/env bash` a `#!/bin/bash -p`, que não lê BASH_ENV nem importa funções do
  ambiente. Na primeira linha útil eles fazem `export PATH=/usr/bin:/bin`. `gerar.py` e
  `origens.py` passaram a `#!/usr/bin/python3 -IB`, e o `gerar.py` fixa o próprio PATH.
- **`dev/ambiente.sh`** (novo), carregado por todos os scripts:
  - PATH fixo;
  - tira BASH_ENV/ENV/CDPATH/GLOBIGNORE e todo `GIT_*` herdado;
  - git com `GIT_CONFIG_GLOBAL=/dev/null` e `GIT_CONFIG_NOSYSTEM=1`;
  - por `GIT_CONFIG_COUNT` (precedência de linha de comando, que vence também o
    `.git/config`), fixa `core.excludesFile=/dev/null`, `core.attributesFile=/dev/null`,
    `core.fsmonitor=false` e `core.hooksPath=/dev/null`.

  O Makefile exporta as mesmas variáveis do git, que valem também para o git chamado
  pelo gitleaks.
- **Docker isolado** (`dev/servicos/comum.sh` e o novo `dev/servicos/compose.sh`):
  - tira do ambiente todo `DOCKER_*`, `COMPOSE_*`, `BUILDX_*` e `BUILDKIT_*` herdado;
  - `DOCKER_CONFIG=.dev/docker/`, apagado e recriado só com `{}` (0600, pasta 0700) a
    cada script. Um plugin, contexto ou builder plantado ali não sobrevive;
  - o `COMPOSE_TESTE` do Makefile agora é `dev/servicos/compose.sh`;
  - alvo novo `make dev-servicos-estado`.
- **`dev/testes/home-real.sh`:**
  - traça também `git check-ignore`, `docker compose ps` e o `docker run` do
    `psql_app`;
  - o detector acompanha o diretório atual de cada processo (chdir, clone/fork). Depois
    de um `fchdir` acusa tudo, porque o diretório fica desconhecido;
  - mutações das opções de `OPCOES_SSH`, cada uma tem de ser acusada:
    - sem `IdentityFile=none`;
    - `CertificateFile=/dev/null` trocado por `none`;
    - sem os dois (aparecem os `*-cert.pub`);
    - sem `-F /dev/null`;
    - sem `UserKnownHostsFile`.
  - "`CertificateFile=/dev/null` segura os `*-cert.pub` mesmo sem `IdentityFile=none`"
    é conferido à parte;
  - controles de git e docker sem o isolamento.
- **`dev/testes/iscas-home.sh`** (novo, `make teste-iscas-home`): é o vetor
  `iscas.sh`/`iscas2.sh` da auditoria.
  - Roda num bwrap com o home do passwd em tmpfs e planta 28 iscas FIFO nesse tmpfs:
    chaves e config do ssh, gitconfig e `~/.config/git/*`, `.docker/config.json`,
    `.curlrc`, `.netrc`, AWS, o cofre do Ward e `.cargo/config.toml`.
  - Roda `conferir.sh` e `ataques-servicos.sh` inteiros. Falha se alguma isca for
    aberta.
  - Os controles (ssh, git e docker sem isolamento) têm de abrir iscas.
  - `make teste-ataques-servicos` roda esse teste no fim.
- `CLAUDE.md` › "Comandos": pré-checagem, `make origens`, `dev-servicos-estado`,
  `teste-iscas-home`, `compose.sh` e a regra do ambiente dos scripts. Inclui "não crie
  `.cargo/config*` no repositório".

## Segurança
- Invariantes tocadas:
  - **S14** (build confiável): nenhum vetor da auditoria passa pelo `make check`. A lista
    é `[alias]`, `build.rustc`, `rustc-wrapper`, `rustc-workspace-wrapper`, `linker`,
    `rustflags`, `runner`, `[env]` e as variáveis `RUSTC*`/`CARGO_*`. Todos param no
    passo 0/6 sem que o programa plantado rode (marca ausente). Rodei os repros do
    auditor (`v13-alias.sh`, `ataque-config.sh` com `wrap.sh`/`linker.sh`/`rustc.sh`)
    em cópias no meu scratchpad (`f12b/r2/`), sem tocar nos arquivos do auditor. Todos
    deram `rc=2` e `wrapper-rodou=nao`.
  - **S15**: o gitleaks continua conferido por sha256, e o script de ataque não aceita
    mais `GITLEAKS` do ambiente.
  - **R19/V1.5**: git e docker não leem mais o home real (strace e iscas). O ssh mantém
    as proteções da 12a, agora com mutação para cada uma.
- Testes de ataque escritos:
  - `dev/testes/ataques-fundacao.sh` › `origens_lista_permitida`, todos barrados antes
    do passo 1 e sem marca:
    - configs: `[alias]` para o falso-ok com RUSTSEC-2020-0071 (v13-alias); `[alias]`
      `= "version"`; `build.rustc`; `build.rustc-wrapper` (wrap.sh);
      `rustc-workspace-wrapper`; `target.<triplo>.linker`; `build.rustflags` com
      `-C linker`; `target.<triplo>.runner`; `[env]`; chave pontilhada;
    - qualquer `.cargo/config*` no repositório: `[term]` inofensivo, `config` sem
      extensão, subpasta `crates/ward-core`, `config.bak`; mais `.cargo` como link
      simbólico e `config` inofensivo em diretório pai;
    - 18 variáveis: `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`,
      `CARGO_BUILD_RUSTC_WRAPPER`, `CARGO_BUILD_RUSTC`, `RUSTFLAGS`,
      `CARGO_BUILD_RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTDOCFLAGS`, `RUSTDOC`,
      `CARGO_TARGET_…_LINKER`, `CARGO_TARGET_…_RUNNER`, `CARGO_ALIAS_AUDIT`,
      `CARGO_TARGET_DIR`, `RUSTC_BOOTSTRAP`, `RUSTUP_TOOLCHAIN`, `RUSTUP_DIST_SERVER`,
      `CARGO_UNSTABLE_BUILD_STD`;
    - `rust-toolchain.toml` com `path`, com `nightly` e `rust-toolchain` legado;
    - `cargo-audit` pelo caminho absoluto ignora o alias e acha RUSTSEC-2020-0071;
    - `CARGO_HOME`/`RUSTUP_HOME`/`CARGO` do ambiente não valem (`make build` passa sem
      marca), e o `origens.py` sozinho recusa `CARGO_HOME` de fora;
    - controles: `CARGO_TERM_COLOR` passa; sem a pré-checagem, o rustc-wrapper plantado
      roda num `cargo build` direto (prova de que o vetor é real).
  - `ataques-fundacao.sh` › `config_do_cargo_home`. Roda num bwrap com home em tmpfs, e
    só `~/.rustup` e `~/.cargo/bin` voltam, somente leitura. São recusados `[alias]`,
    `build.rustc-wrapper`, `build.rustc`, `target.linker`, `build.rustflags`, `[env]`,
    `[source]`, `[registries]`, `http.proxy`, `net.git-fetch-with-cli`, `include`, valor
    não simples e TOML inválido. Sem config e com chaves da lista, passa. O
    `rustup override` de diretório, simulado por um `settings.toml` montado por cima, é
    recusado.
  - `ataques-fundacao.sh` › `ambiente_do_make`: um `make check` inteiro numa cópia com
    ambiente hostil sai 0, sem marca. O ambiente:
    - PATH com cargo/cargo-audit/cargo-deny/python3/git/bash/sh/sha256sum/gitleaks/
      docker/rustc/make falsos na frente;
    - `MAKEFLAGS` trocando CARGO/GITLEAKS/CARGO_AUDIT/PYTHON;
    - `BASH_ENV`/`ENV`, HOME falso, `GIT_CONFIG_GLOBAL` com `core.fsmonitor` e ignore
      global.

    Controle: o fsmonitor roda num `git diff` sem o isolamento. `core.fsmonitor` no
    `.git/config` local não roda no `make gitleaks`.
  - `ataques-fundacao.sh` › `git_sem_config_do_usuario`: o ignore global não faz
    `check-ignore` passar. Controle: sem o isolamento, faz.
  - `ataques-fundacao.sh` › `v1_3_git`: a origem git agora para na pré-checagem, e o
    `cargo-deny` direto também a recusa (ver "Problemas").
  - `dev/testes/ataques-servicos.sh` › `docker_sem_ambiente_do_usuario`. Ambiente
    hostil: plugin `docker-compose` plantado num DOCKER_CONFIG (com `credsStore` e
    `cliPluginsExtraDirs`), `DOCKER_HOST`, `DOCKER_CONTEXT`, `COMPOSE_FILE`,
    `COMPOSE_PROJECT_NAME`, `BASH_ENV` e PATH com `docker` falso. Com ele, `compose.sh`
    e `make dev-servicos-estado` acham os serviços reais sem rodar nada. Um plugin
    plantado em `.dev/docker/cli-plugins` é apagado antes do uso. Controle: o plugin
    roda sem o isolamento.
  - `dev/testes/home-real.sh`:
    - clientes traçados: `git check-ignore`, `docker compose ps` e `docker run do
      psql_app`;
    - mutações: `sem IdentityFile=none`, `CertificateFile=none no lugar de /dev/null`,
      `sem IdentityFile=none e sem CertificateFile=/dev/null`, `sem -F /dev/null`,
      `sem UserKnownHostsFile próprio`, mais a conferência
      `CertificateFile=/dev/null tira os *-cert.pub…`;
    - controles: `git sem o isolamento` e `docker sem DOCKER_CONFIG`.
  - `dev/testes/iscas-home.sh`: `conferir.sh inteiro…` e `ataques-servicos.sh inteiro…`
    não abrem iscas; controles de `id_ed25519`, `.gitconfig` e `.docker/config.json`.
- Dependências novas: nenhuma. Nenhum crate e nenhuma ferramenta nova (`bwrap`,
  `strace` e `mkfifo` já eram exigidos ou vêm do sistema).

## Ficou de fora
- Nada do plano. Os pontos abaixo ficaram registrados e não foram corrigidos.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make teste-home-real` (antes de qualquer cliente ssh/curl) | 0 |
| Experimentos de ssh/strace só dentro de `bwrap --tmpfs <home>` | 0 |
| Repros do auditor em cópia (`f12b/r2/v13-alias.sh`, `ataque-config.sh` × 9 vetores) | `make check` 2 em todos, wrapper não rodou |
| `make check` | 0 |
| `make teste-ataques-fundacao` (165 ok) | 0 |
| `make dev-servicos-subir` (pelo `compose.sh`) | 0 |
| `make dev-servicos-conferir` | 0 |
| `make teste-ataques-servicos` (71 ok, inclui `teste-iscas-home`) | 0 |

## Problemas descobertos
- **`MAKEFLAGS` do ambiente vencia o `:=` da Fase 12a.** O handoff 12a dizia que
  "variável de ambiente não troca mais a ferramenta". Mas `MAKEFLAGS='CARGO=/x' make`
  entra como variável de linha de comando e vence o `:=`. Corrigi com `override` e
  escrevi teste.
- **V1.3 × pré-checagem.** O texto de V1.3 diz "falha em `cargo audit` ou `cargo deny`".
  A dependência de git agora falha antes, no passo 0/6 (`origens: pacote falso-git:
  fonte fora do crates.io`). Escolhi isso por segurança: sem a pré-checagem, o
  `clippy` do passo 2 baixaria e compilaria o crate (com `build.rs`) antes do deny. O
  `cargo deny` sozinho continua recusando, e o teste confere as duas coisas. O advisory
  (`time 0.1.43`) continua falhando no passo 4/6. O orchestrator decide se o contrato
  ganha uma nota.
- **`CertificateFile=none` sozinho é perigoso.** Ele abre `./none` relativo ao diretório
  atual, como a 12a disse. A mutação agora prova isso de verdade: a 12a só acrescentava
  `none` por cima do `/dev/null`. Já o `CertificateFile=/dev/null` é segunda barreira:
  sem `IdentityFile=none` ele ainda impede a busca dos `*-cert.pub`. Sem os dois, o ssh
  procura os `*-cert.pub`.
- **`.git/config` local ainda é vetor parcial.** Fechei `core.fsmonitor` e `hooksPath`
  por `GIT_CONFIG_COUNT`. Um `diff.<driver>.textconv` no `.git/config` junto com um
  `.gitattributes` versionado ainda pode rodar programa no `git log -p` do gitleaks. Não
  testei. Sugestão para outra fase: o `origens` (ou um irmão) recusar `.git/config` com
  chaves que executam programa.
- **O ambiente do próprio `make` continua confiável por suposição.** `MAKEFILES=…`,
  `MAKEFLAGS=--eval=…` e `LD_PRELOAD` rodam código antes de qualquer checagem do
  Makefile. Não há como barrar isso de dentro dele. Quem controla esse ambiente já
  executa código como o usuário.
- `dev/origens.py` lê `~/.cargo/config*` e roda `rustup show active-toolchain`. Se o
  usuário criar um `~/.cargo/config.toml` com chave fora da lista (ex.: `[alias]` ou
  `http.proxy`), o `make check` deste projeto falha, de propósito. A mensagem diz qual
  chave.
- O `CARGO_HOME`/`RUSTUP_HOME` do ambiente agora é ignorado pelo Makefile, que sempre
  usa `~/.cargo` e `~/.rustup` do passwd. Quem usa outro CARGO_HOME precisa mudar o
  Makefile.
- `.dev/docker/` é recriado por cada script que carrega `comum.sh`. Dois scripts de
  serviço rodando ao mesmo tempo podem apagar o DOCKER_CONFIG um do outro. Hoje eles
  rodam em sequência.
- Com o DOCKER_CONFIG vazio, o `make dev-servicos-subir` recria os 4 contêineres SSH a
  cada execução: o build gera um manifest de atestação novo e o compose acha que a
  imagem mudou. O estado fica em tmpfs e as chaves de host em `.dev/`, então só se perde
  o estado em memória. Não conferi se isso já acontecia antes desta fase.
- `docker compose -f dev/compose.yml …` direto (fora do `compose.sh`) continua lendo o
  `~/.docker` do usuário. O `CLAUDE.md` avisa.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/ambiente.sh` | criado |
| `dev/servicos/compose.sh` | criado |
| `dev/testes/iscas-home.sh` | criado |
| `dev/origens.py` | alterado (reescrito: lista de permitidas, CARGO_HOME, ambiente, toolchain) |
| `Makefile` | alterado (pré-checagem 0/6, `override`, PATH fixo, `bash -p`, binários absolutos, git isolado, `compose.sh`, `dev-servicos-estado`, `teste-iscas-home`) |
| `dev/servicos/comum.sh` | alterado (`ambiente.sh`, DOCKER_CONFIG vazio, sem DOCKER_*/COMPOSE_*) |
| `dev/servicos/conferir.sh` | alterado (`#!/bin/bash -p`, PATH fixo) |
| `dev/servicos/gerar.py` | alterado (shebang, PATH fixo) |
| `dev/testes/ataques-fundacao.sh` | alterado (ambiente, gitleaks fixo, 4 funções novas, V1.3 git e vendor na pré-checagem) |
| `dev/testes/ataques-servicos.sh` | alterado (`#!/bin/bash -p`, PATH, `docker_sem_ambiente_do_usuario`) |
| `dev/testes/home-real.sh` | alterado (git/docker, cwd por processo, mutações, controles) |
| `CLAUDE.md` | alterado ("Comandos") |
| `docs/mission/handoffs/phase-12b.md` | criado |

## Procedimentos
- [x] `make check` rodado (pré-checagem, fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados. Nenhum `.cargo/` criado no
  repositório real, todos os vetores rodaram em cópias `mktemp -d`. O config do
  CARGO_HOME e o settings do rustup só foram escritos em tmpfs dentro do bwrap.
- [x] Teste de ataque para cada vetor dos repros (v13-alias, ataque-config, wrap.sh,
  iscas) e para cada item do plano
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase. Fiz alguns extras no mesmo espírito e os registrei
  acima: `override` contra MAKEFLAGS, `bash -p`/BASH_ENV, toolchain e rustup override,
  fsmonitor/hooksPath do git e o teste de iscas.
