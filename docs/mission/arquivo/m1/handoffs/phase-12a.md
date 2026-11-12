# Fase 12a — Correções da auditoria de segurança do M1 (1ª reprovação)

## Implementado
- **Alto: clientes SSH abriam as chaves padrão do home real.** O ssh procura
  identidades e certificados padrão pelo home do passwd (getpwuid), não pelo `$HOME`.
  Por isso o HOME falso de `cliente` não bastava. Os clientes sem `-i` (`ssh_senha`,
  o salto do `ProxyCommand` de `ssh_interno` e o `ssh -v` de `metodos`) abriam
  `id_rsa`, `id_ecdsa`… do usuário real.
  - Em `dev/servicos/comum.sh`, `OPCOES_SSH` ganhou `-o IdentityFile=none` (o `-i`
    continua valendo) e `-o CertificateFile=/dev/null`. Não usei `CertificateFile=none`,
    como o plano pedia: o OpenSSH 9.6 não entende `none` nessa opção e abria um arquivo
    `none` relativo ao diretório atual. Medido sob strace (ver "Problemas").
  - O `curl` também lia o `.curlrc` do home do passwd quando não achava no `$HOME`.
    Achei isso com o teste novo. Agora todo `cliente curl` começa com `-q`, e o `curl`
    do `make ferramentas` também.
  - `cliente` chama `/usr/bin/env` pelo caminho absoluto, e não pelo PATH do usuário.
- **Teste de ataque novo: `dev/testes/home-real.sh`** (`make teste-home-real`).
  - Ele se reexecuta dentro de um `bwrap` com o home do passwd coberto por tmpfs. Só
    o repositório e `/tmp` voltam por bind. Antes de rodar qualquer cliente, confere
    que o home é tmpfs e está vazio. `WARD_HOME_COBERTO=1` fora do sandbox é recusado.
  - Roda sob `strace -f` cada cliente: ssh por senha, por senha errada, por chave
    (`-i`) e pelo ProxyCommand; `ssh -v`; `ssh -W`; sftp; `ssh-keygen -y`;
    `ssh-keygen -t` como o `gerar.py`; `curl` no wp-login.
  - Falha se algum caminho do home aparecer numa chamada de arquivo. Ficam de fora o
    repositório e os diretórios do caminho até ele. Também falha com caminho relativo
    dentro do diretório atual (que tem uma isca `none`) ou com caminho relativo a um
    dirfd.
  - Os clientes que têm de entrar precisam sair 0: um trace vazio não prova nada.
  - Três controles positivos têm de ser acusados: ssh sem `IdentityFile=none`, ssh com
    `CertificateFile=none` e curl sem `-q`.
  - `ataques-servicos.sh` e `conferir.sh` rodam esse teste antes de qualquer outro
    cliente e abortam se ele falhar.
- **Médio: varredura extra de chave privada e AWS, sem allowlist global de caminho.**
  - `dev/gitleaks-extra.toml`, sem `useDefault`, tem três regras: chave privada,
    Access Key ID e Secret Access Key (com ou sem o prefixo `aws_`). Não tem nenhuma
    allowlist.
  - `dev/gitleaks-extra-arvore.toml` só tira os diretórios ignorados pelo git, como a
    `gitleaks-arvore.toml`.
  - `make gitleaks` roda a extra na árvore, no índice e no histórico, com
    `--max-archive-depth 3` (entra em .gz/.zip/.tar).
- **Médio: origens do cargo.** `dev/origens.py` (só biblioteca padrão, com `tomllib`)
  roda no passo 5/6, depois do `cargo deny`. O rótulo virou
  `5/6 cargo deny check (e origens do cargo)`, e a ordem dos seis passos de V1.1 é a
  mesma. Ele recusa:
  - `.cargo/config` ou `.cargo/config.toml` que o cargo leria, da raiz até `/` e o do
    CARGO_HOME, com `source`, `patch`, `registries`, `registry`, `paths`, `include` ou
    `credential-alias`. Vale em tabela ou em chave pontilhada. TOML inválido também
    é recusado;
  - variáveis `CARGO_SOURCE_*`, `CARGO_REGISTRIES_*`, `CARGO_REGISTRY_*` e
    `CARGO_PATCH_*`;
  - `[patch]` ou `[replace]` em qualquer manifesto do workspace;
  - pacote sem fonte (path) fora do repositório, ou em `target/` ou `.dev/`;
  - pacote de fonte que não é o crates.io.

  `make origens` roda só esse script.
- **Baixo:**
  - `CARGO :=` e `GITLEAKS :=`: variável de ambiente não troca mais a ferramenta.
  - Alvo `gitleaks-binario`: confere o sha256 do binário extraído
    (`88f91962…c32e5509`) antes de toda varredura e de `teste-ataques-fundacao`.
    `make ferramentas` também confere esse sha256 antes de instalar. Calculei o valor
    baixando de novo o tar.gz oficial, que bateu com o sha256 já fixado. O binário
    extraído deu o mesmo hash do que está em `.dev/`.
  - Nota no topo de `phase-11.md` dizendo que o `phase-11-ajuste.md` o substitui na
    allowlist.
- `dev/servicos/ssh/entrada.sh`: `PerSourcePenalties no` nos sshd de teste (ver
  "Problemas").
- `CLAUDE.md` › "Comandos": `make check` atualizado, `make teste-home-real` e a regra
  dos clientes (`IdentityFile=none`, `CertificateFile=/dev/null`, `curl -q`, todo
  cliente novo por `comum.sh` e `home-real.sh`).

## Segurança
- Invariantes tocadas:
  - **R19/V1.5**: nenhum cliente de teste abre caminho do home do usuário. Prova feita
    na ordem pedida:
    1. Sob strace, dentro do sandbox do auditor (`bwrap --dev-bind / / --tmpfs <home>
       --bind <repo> <repo> --bind /tmp /tmp`), rodei o `st.sh` do auditor (saída
       redirecionada para `f12a/aud-st-*.txt`, sem sobrescrever a dele): 0 acessos à
       pasta de chaves SSH ou de credenciais AWS do home (antes eram dezenas).
    2. Ainda no sandbox e sob strace, rodei `conferir.sh` e `ataques-servicos.sh`
       inteiros: 0 acessos a essas pastas. Usei uma cópia sem a chamada ao
       `home-real.sh`, porque strace não roda dentro de strace.
    3. `home-real.sh` passou no sandbox. Só então rodei os alvos fora dele.

    Os acessos ao home que sobram nos traces completos vêm de ferramentas do host,
    não dos clientes de teste (ver "Problemas").
  - **S15**: a extra acha chave, segredo AWS e Access Key ID em lockfiles, `go.sum`,
    `gitleaks.toml`, svg, png (inclusive png real com texto no fim), pdf, woff2,
    `vendor/`, `.gz` e `.zip`, na árvore e no índice. Passa com `--redact`, e a saída
    não repete o segredo.
  - **S14**: substituição de fonte por diretório vendorizado e adulterado (o cenário
    do `v13.sh`) agora faz o `make check` falhar no passo 5/6. Antes passava inteiro.
    `[patch]`, `[replace]`, registro alternativo e path fora do repositório também
    falham. O binário do gitleaks é conferido por sha256 a cada varredura.
- Testes de ataque escritos:
  - `dev/testes/home-real.sh`. Cada cliente: `ssh_senha (2222, deploy)`,
    `ssh_senha com senha errada (2224)`, `ssh_chave (2223, -i)`, `ssh_interno
    (ProxyCommand pelo salto)`, `ssh -v sem credencial (2223)`, `ssh -W (túnel até o
    postgres)`, `sftp do sftpuser`, `ssh-keygen -y`, `ssh-keygen -t ed25519 -f`,
    `curl no wp-login`. Controles: `controle: ssh sem IdentityFile=none é acusado`,
    `controle: arquivo relativo ao diretório atual é acusado`,
    `controle: curl sem -q é acusado`.
  - `dev/testes/ataques-fundacao.sh` › `extra_sem_allowlist_de_caminho`: 3 tipos × 12
    caminhos, na árvore e no índice, mais a checagem de que a saída não repete o
    segredo.
  - `dev/testes/ataques-fundacao.sh` › `origens_do_cargo`, uma checagem por caso:
    - `make check` com `replace-with` para `vendor/`;
    - `source` em chave pontilhada;
    - `.cargo/config` com `[patch]`;
    - `[registries]`;
    - config em diretório pai;
    - TOML inválido;
    - `CARGO_REGISTRIES_MAU_INDEX`;
    - `[patch]` e `[replace]` no `Cargo.toml`;
    - path absoluto fora do repositório;
    - path com `..` para fora;
    - controle: a cópia limpa passa.
  - `dev/testes/ataques-fundacao.sh` › `ferramentas_fixas`:
    - `GITLEAKS=/bin/true` no ambiente não desliga o gitleaks;
    - `CARGO=false` no ambiente não troca o cargo;
    - gitleaks trocado por script é recusado;
    - gitleaks com 1 byte a mais é recusado.
  - Todas as cópias desses testes ficam em `mktemp -d` e copiam o gitleaks para o
    `.dev/` delas.
- Dependências novas: nenhuma. Ferramentas do host passam a ser exigidas pelos testes
  de serviço: `bwrap` (bubblewrap) e `strace`, as duas já instaladas. Sem elas, o
  `home-real.sh` falha fechado.

## Ficou de fora
- Nenhum item do plano. Os pontos abaixo ficam registrados e não foram corrigidos,
  porque estão fora do escopo da fase.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `st.sh` do auditor dentro do sandbox, sob strace (saída em `f12a/`) | 0 (0 acessos às pastas de chave/AWS) |
| `conferir.sh` e `ataques-servicos.sh` sem a guarda, dentro do sandbox e sob strace | 0 e 0 (0 acessos) |
| `dev/testes/home-real.sh` dentro do sandbox (`WARD_HOME_COBERTO=1`) | 0 |
| `dev/testes/home-real.sh` fora (reexecuta em bwrap) | 0 |
| `WARD_HOME_COBERTO=1 dev/testes/home-real.sh` fora do sandbox (tem de recusar) | 1 |
| `probe.sh` / `probe-idx.sh` do auditor (estado antes) | 0 (7 casos não detectados) |
| `f12a/probe-extra.sh` (extra, 48 casos) | 0 (todos detectados) |
| `f12a/v13b.sh` (v13 do auditor com os arquivos novos) | vendor adulterado: `make check` 2 no passo 5/6; path fora: `make deny` 2 |
| `make dev-servicos-subir` (rebuild do SSH com `PerSourcePenalties no`) | 0 |
| `make dev-servicos-conferir` | 0 |
| `make teste-ataques-servicos` (3×, depois do rebuild) | 0, 0, 0 (53 ok) |
| `make check` | 0 |
| `make teste-ataques-fundacao` | 0 (101 ok) |

## Problemas descobertos
- **`CertificateFile=none` não funciona no OpenSSH 9.6**: abre `./none` relativo ao
  diretório atual. Um arquivo `none` plantado ali viraria certificado. Troquei por
  `CertificateFile=/dev/null`, que diverge do texto do plano. O teste acusa o caso de
  `none` como controle.
- **`curl` lia o `.curlrc` do home do passwd**, mesmo com HOME falso. Esse achado é
  novo, não estava na auditoria. Corrigido com `-q` em todo `cliente curl`.
- **Penalidades por origem do sshd 10 (`PerSourcePenalties`)**: depois das senhas
  erradas de `autenticacao`, o sshd de teste derrubava as conexões seguintes do
  gateway do docker por 16 s.
  - O túnel positivo `deploy@2222 → postgres` falhou em 2 de 3 rodadas.
  - Os testes "recusado" podiam passar pela queda, não pela autenticação.
  - Desliguei a opção em `dev/servicos/ssh/entrada.sh`, arquivo da Fase 12. É só o
    servidor de teste, preso em 127.0.0.1.
  - Sugestão para outra fase: os testes "recusado" conferirem "Permission denied" na
    saída.
- **Ferramentas do host ainda olham o home real** (fora dos clientes de teste):
  - o shell que chama os scripts procura `docker`, `git`, `python3`… no PATH do
    usuário, que inclui `~/.local/bin` e o nvm;
  - `docker` lê `~/.docker/config.json` e procura credential helpers;
  - `git` lê `~/.gitconfig`.

  Nada disso toca a pasta de chaves SSH nem a de credenciais AWS: o trace completo deu
  0. Um binário plantado no PATH do usuário, porém, rodaria nos scripts. Se o
  orchestrator quiser fechar isso, o caminho é PATH fixo no topo dos scripts e
  `HOME`/`DOCKER_CONFIG` isolados para o docker.
- `conferir.sh` (V1.4) passou a exigir `bwrap` e `strace`, porque roda a guarda antes
  de qualquer cliente. O `bwrap` precisa de user namespaces sem privilégio.
- `dev/origens.py` proíbe também `registry`, `paths`, `include` e `credential-alias`,
  além do que o plano lista, e lê o config do CARGO_HOME (`~/.cargo`, dentro da
  exceção do Makefile). Hoje esse arquivo não existe. Se o usuário criar um
  `~/.cargo/config.toml` com `[registries]`, o `make check` deste projeto falha, de
  propósito.
- Um segredo AWS de 40 caracteres sozinho, sem nome de variável, continua sem
  detecção. A extra exige o nome (`aws_secret_access_key`, `secret_access_key`…).
- A varredura da árvore continua pulando `dist/` e `node_modules/` aninhados, como o
  `.gitignore`. O índice e o histórico não pulam nada, então um `git add -f` é pego.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/servicos/comum.sh` | alterado (`IdentityFile=none`, `CertificateFile=/dev/null`, `/usr/bin/env`, `curl -q`) |
| `dev/servicos/conferir.sh` | alterado (guarda `home-real.sh`, `curl -q`) |
| `dev/servicos/ssh/entrada.sh` | alterado (`PerSourcePenalties no`) |
| `dev/testes/home-real.sh` | criado |
| `dev/testes/ataques-servicos.sh` | alterado (guarda `home-real.sh`, `curl -q`) |
| `dev/testes/ataques-fundacao.sh` | alterado (3 ataques novos; cópia leva gitleaks e arquivos novos) |
| `dev/gitleaks-extra.toml` | criado |
| `dev/gitleaks-extra-arvore.toml` | criado |
| `dev/origens.py` | criado |
| `Makefile` | alterado (`:=`, sha256 do binário, extra, origens, `teste-home-real`, `curl -q`) |
| `CLAUDE.md` | alterado ("Comandos") |
| `docs/mission/handoffs/phase-11.md` | alterado (nota no topo) |
| `docs/mission/handoffs/phase-12a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny + origens, gitleaks + extra): 0
- [x] Só cofre de teste e credenciais falsas usados (chaves e segredos gerados na hora
  em `mktemp -d`; nenhum cliente ssh rodou fora do sandbox antes da prova)
- [x] Teste de ataque para cada (SEG) tocado (V1.2/V1.3/V1.5, S14, S15, R19)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (exceto `PerSourcePenalties` em arquivo da Fase
  12, necessário para `teste-ataques-servicos` sair 0 de forma estável; registrado)
