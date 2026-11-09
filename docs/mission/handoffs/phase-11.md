# Fase 11 — Fundação: workspace, toolchain, deny, gitleaks, Makefile e "Comandos"

> **Nota (Fase 12a):** na allowlist do gitleaks, este handoff foi substituído pelo
> `phase-11-ajuste.md`: a exceção do id do Figma descrita abaixo foi removida e a
> `.gitleaks.toml` tem só os valores de D15. A Fase 12a também mudou `GITLEAKS`/`CARGO`
> para `:=`, passou a conferir o sha256 do binário do gitleaks a cada varredura e
> acrescentou a varredura extra e `dev/origens.py` (ver `phase-12a.md`).

## Implementado
- Workspace Cargo (`Cargo.toml`, resolver 3, edição 2024, `rust-version = 1.99`,
  `publish = false`) com seis crates vazios em `crates/`: `ward-core` (lib),
  `ward-plataforma` (lib), `wardd` (bin), `ward` (bin), `ward-nm` (bin) e
  `ward-ponte-teste` (lib, com a feature `modo-teste` declarada e vazia). Os binários
  saem com código 1 e "ainda não implementado", em vez de fingir sucesso.
- Lints do workspace: `unsafe_code = "deny"` e `unsafe_op_in_unsafe_fn = "deny"`;
  `clippy::undocumented_unsafe_blocks`, `dbg_macro` e `todo` como `deny`. Todos os
  crates, menos `ward-plataforma`, têm `#![forbid(unsafe_code)]`. Perfil release com
  `overflow-checks`, LTO e `strip`.
- `rust-toolchain.toml` fixo em `1.99.1` com `rustfmt` e `clippy`. `Cargo.lock`
  gerado (versão 4) e fora do `.gitignore`.
- `deny.toml`: advisories (`yanked = "deny"`, `unmaintained = "all"`, sem `ignore`),
  licenças permissivas numa lista fechada, `wildcards = "deny"` (só path do workspace
  liberado), `unknown-registry` e `unknown-git` = `deny`, só o crates.io permitido.
- `.gitleaks.toml`: regras padrão + allowlist só dos valores falsos fixos de D15
  (cada um casado inteiro, `^…$`) e uma entrada separada para o id público do Figma
  (ver "Problemas"). `dev/gitleaks-arvore.toml` herda essa config e só acrescenta os
  diretórios que o `.gitignore` tira do git (`.dev/`, `target/`, `node_modules/`,
  `dist/`), para a varredura da árvore de trabalho.
- `Makefile`: `make check` roda, nessa ordem, `cargo fmt --check`,
  `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`,
  `cargo test --workspace --all-features --locked`, `cargo audit --deny warnings`,
  `cargo deny --all-features check` e gitleaks em três varreduras (árvore com arquivos
  não versionados; índice `--staged`, sem exceção de diretório; histórico), sempre
  com `--redact --verbose` (mostra arquivo e linha, nunca o valor). Alvos `build`,
  `fmt`, `ferramentas` (instala `cargo-audit@0.22.2`, `cargo-deny@0.20.2` e o gitleaks
  8.30.1 em `.dev/ferramentas/bin/`, conferindo o sha256 antes de extrair) e
  `teste-ataques-fundacao`. Alvos das fases seguintes (`dev-servicos`, `dev-cofre`,
  `dev-variacoes`, `dev-daemon`, `dev-daemon-parar`) existem e falham com mensagem
  dizendo a fase que os implementa.
- `CLAUDE.md` › "Comandos": build, `make check`, `make ferramentas`, testes de ataque,
  `make dev-servicos`, `docker compose -f dev/compose.yml up -d`/`down`,
  `make dev-cofre`, `make dev-variacoes`, `make dev-daemon`/`dev-daemon-parar`, senha
  mestra e PIN de teste, cada um com a fase em que fica pronto (V1.6).
- `.gitignore`: `__pycache__/` e `*.pyc`, como pedido.

## Segurança
- Invariantes tocadas:
  - **S14**: `cargo audit --deny warnings` e `cargo deny check` no `make check`;
    origens restritas ao crates.io; `Cargo.lock` gerado e não ignorado; `--locked`
    em build, clippy e testes (lockfile desatualizado falha).
  - **S15**: gitleaks em árvore, índice e histórico; allowlist só com valores exatos
    de D15 (nada de `falso-.*`); a exceção de diretório vale só na árvore, então um
    arquivo de `.dev/` forçado para o git é achado; `--redact` em todas as varreduras.
  - V1.5: `.dev/` ignorado; Makefile e `dev/` não citam cofre real, KeePassXC, pasta
    de chaves SSH, de credenciais AWS, `ssh_config` nem `SSH_AUTH_SOCK`. O Makefile só
    acrescenta `~/.cargo/bin` ao `PATH`.
- Testes de ataque (`dev/testes/ataques-fundacao.sh`, via
  `make teste-ataques-fundacao`; todos barrados no momento da execução):
  - `v1_2`: arquivo `vazamento.txt` com chave OpenSSH ed25519 gerada na hora e
    `aws_secret_access_key = <40 aleatórios>` → `make check` sai 2 e aponta o arquivo;
    a saída não repete o segredo; sem o arquivo, volta a 0.
  - `allowlist_ancorada`: `api_secret = "falso-WP-senha-01<aleatório>"` continua sendo
    achado (a allowlist não vira porta de saída).
  - `dev_forcado_no_indice`: chave em `.dev/` adicionada com `git add -f` (numa cópia
    do workspace, com `git init` próprio) é achada pela varredura do índice.
  - `v1_3_advisory`: crate de teste com `time = "=0.1.43"` → `make check` falha no
    passo 4/6 (`cargo audit`) com RUSTSEC-2020-0071; `make deny` também recusa.
  - `v1_3_git`: dependência de git (`file://` local, sem rede) → `make check` falha no
    passo 5/6 (`cargo deny`) com `source-not-allowed`.
  - `v1_3_lockfile`: `Cargo.lock` existe e `git check-ignore` não o ignora.
  - `v1_5`: `.dev/qualquer`, `.dev/cofre-teste.ward` e o gitleaks local são ignorados;
    grep dos padrões proibidos no Makefile e em `dev/` não acha nada (os padrões são
    montados por partes para o script não casar consigo mesmo).
  - Os ataques de V1.3 rodam numa cópia do workspace em `mktemp -d`; o único arquivo
    criado no repositório real é `vazamento.txt`, apagado por `trap` mesmo em erro.
- Dependências novas: nenhuma dependência de crate. Ferramentas de desenvolvimento
  (fora do grafo do projeto): `cargo-audit 0.22.2` e `cargo-deny 0.20.2`
  (`cargo install --locked`, em `~/.cargo/bin`) e gitleaks 8.30.1 (binário oficial do
  GitHub, sha256 `551f6fc8…f2470eb` conferido contra o `checksums.txt` da release), em
  `.dev/ferramentas/bin/`. Motivo: são as ferramentas que V1.1 exige.

## Ficou de fora
- `app/` (Tauri) e `extensao/` não foram criados: não são crates vazios e chegam no
  M5/M6. O `src-tauri` entra no workspace quando a fase do app o criar.
- `make dev-servicos`, `dev-cofre`, `dev-variacoes`, `dev-daemon`, `dev-daemon-parar`:
  só o alvo que falha com mensagem; implementação nas Fases 12, 19 e no M3.
- Ligação entre `wardd` e `ward-ponte-teste` pela feature `modo-teste`: fica para a
  fase da ponte (D7), que decide se é dependência opcional do daemon ou binário à parte.
- Os testes de ataque não entram no `make check` (V1.1 fixa os seis passos e V1.2 roda
  o próprio `make check`); ficam em `make teste-ataques-fundacao`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo install --locked cargo-audit cargo-deny` | 0 |
| download do gitleaks 8.30.1 + `sha256sum` contra `checksums.txt` | 0 |
| `cargo build` | 0 |
| `make check` (antes do arquivo da Fase 10d aparecer) | 0 |
| `make teste-ataques-fundacao` | 0 (13 ok, 0 falhas) |
| `make check` (final) | 2 — só `docs/mission/evidence/fase-10d/v0-auditoria.json`, ver "Problemas" |
| gitleaks da árvore sem esse único arquivo (config temporária no scratchpad) | 0 |

## Problemas descobertos
- **`make check` final sai 2 por um arquivo da Fase 10d, que roda em paralelo**:
  `docs/mission/evidence/fase-10d/v0-auditoria.json`, linha ~492, contém a string
  `falso-exemplo-9f3k` sob uma chave que o `generic-api-key` trata como segredo. Não é
  valor de D15, então não entrou na allowlist (regra da fase), e o arquivo é da
  10d, então não o alterei. Saídas: a 10d evita gravar a string (ou grava só um
  hash), ou o orchestrator decide pôr esse valor exato na allowlist com motivo. Sem
  esse arquivo, todos os seis passos passam.
- **Id do Figma na allowlist**: `bzu9PxxFur2tU3vlNCHvsY` aparece em três evidências do
  M0 (`fase-1-inspiracoes.json`, `fase-4a/…`, `fase-10/v0-21-inspiracoes.json`) e o
  `generic-api-key` o acusa. Não é credencial nem valor de D15; entrou como exceção
  separada, casada inteira, com comentário. Se a regra "allowlist só de D15" for
  estrita, a alternativa é tirar o id dessas evidências.
- `--pre-commit --staged` e a varredura do histórico rodam sem commits hoje (0 commits);
  passam a valer quando o usuário commitar.
- `cargo audit` e `cargo deny` baixam o banco de advisories pela rede a cada
  `make check`; sem rede o passo falha (falha fechada, não passa em silêncio).
- Os nomes `make dev-daemon` e `make dev-daemon-parar` são proposta desta fase; o plano
  não os nomeia. A fase do daemon de teste pode trocá-los e atualizar o `CLAUDE.md`.
- O `extend.path` de `dev/gitleaks-arvore.toml` é relativo ao diretório atual (o
  gitleaks não resolve relativo ao arquivo); por isso o Makefile sempre roda da raiz.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Cargo.toml` | criado |
| `Cargo.lock` | criado |
| `rust-toolchain.toml` | criado |
| `deny.toml` | criado |
| `.gitleaks.toml` | criado |
| `dev/gitleaks-arvore.toml` | criado |
| `dev/testes/ataques-fundacao.sh` | criado |
| `Makefile` | criado |
| `crates/ward-core/{Cargo.toml,src/lib.rs}` | criado |
| `crates/ward-plataforma/{Cargo.toml,src/lib.rs}` | criado |
| `crates/wardd/{Cargo.toml,src/main.rs}` | criado |
| `crates/ward/{Cargo.toml,src/main.rs}` | criado |
| `crates/ward-nm/{Cargo.toml,src/main.rs}` | criado |
| `crates/ward-ponte-teste/{Cargo.toml,src/lib.rs}` | criado |
| `CLAUDE.md` | alterado (seção "Comandos") |
| `.gitignore` | alterado (`__pycache__/`, `*.pyc`) |
| `docs/mission/handoffs/phase-11.md` | criado |
| `.dev/ferramentas/bin/gitleaks` | criado (fora do git) |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): passou com 0
  antes; o final sai 2 só pelo arquivo da Fase 10d (ver "Problemas")
- [x] Só cofre de teste e credenciais falsas usados (chaves geradas na hora em
  diretório temporário e apagadas)
- [x] Teste de ataque para cada (SEG) tocado (V1.2, V1.3, V1.5)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
