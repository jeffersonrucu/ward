# Fase 43 — Scripts de contêiner e exemplos (trilha S)

## Implementado

Comentários, docstrings e identificadores em inglês (troca 1:1 pelo `docs/glossario.md`);
as mensagens de saída pt-BR (`ok:`, `FALHOU:`, descrições das conferências) ficam.

- **Exemplos renomeados, com todos os usos** (`[[example]]` dos Cargo.toml, Makefile,
  `CLAUDE.md`, `dev/tests/*.sh`, `nm.py`):
  - `wardd-cliente-teste` (`examples/cliente-teste.rs`) → `wardd-test-client` (`examples/test-client.rs`)
  - `pai-chrome` → `chrome-parent`; `chrome-falso` → `fake-chrome`
  - `dev-cofre` (`examples/dev-cofre/{main,testes}.rs`) → `dev-vault` (`examples/dev-vault/{main,tests}.rs`)
  - `partida-conteiner` → `startup-container` (`app/core/examples/startup-container.rs`)
- **`dev-vault`** (`main.rs`, `tests.rs`): identificadores próprios em inglês (`Error`,
  `Options`, `Reader`, `Obj`, `Value`, `Source`, `FakeResources`, constantes `MAX_*`,
  `DEFAULT_OUTPUT` …) e nomes dos testes em inglês. A API do `ward-core` usada
  (`Configuracao`, `CofreAberto`, `RascunhoDados`, `gravar_entradas`, `limites::*`,
  `cofre_teste.rs` …) fica: é das fases R.
- **`container.rs`, `chrome-parent.rs`, `fake-chrome.rs`, `test-client.rs`,
  `startup-container.rs`**: comentários e identificadores em inglês; API de
  `ward_app_core::partida` e `ward_core::protocolo` fica.
- **`dev/container-*`** (`inside.sh` de plataforma, install, deb, units e uid;
  `fake.c`, `mark.c`, os três Dockerfiles, `start.sh`): comentários em inglês e renomes
  de funções e variáveis (`confere` → `check`, `nao_tem` → `lacks`, `igual` → `equal`,
  `falhou` → `failed`, `falhas` → `failures`, `como_1000` → `as_1000`, `instalar` →
  `install_ward`, `caso_grupo` → `group_case`, `retrato_ssh` → `ssh_snapshot`,
  `REL`/`REG`/`FALSO`/`COFRE_T`/`USUARIO` → `REPORT`/`RECORD`/`FAKE`/`VAULT_T`/`TEST_USER` …;
  tabela completa no glossário, seção 6, parágrafo da Fase 43). Cabeçalhos
  `#!/bin/bash -p` e `export PATH=…` intactos.
- **`dev/tests/clipboard.py`**: docstring, comentários e nomes em inglês (`Vigia` →
  `Watcher`, `Teste` → `AppTest`, `TESTE` → `TEST_NAME`, `rodada_x11` → `x11_round` …).
- **Glossário**: parágrafo "Feitos na Fase 43" na seção 6.

### Literais mudados (e por quê)

| Onde | Antes → depois | Motivo |
| --- | --- | --- |
| `dev-vault` (uso, mensagens, `dev-vault.sh`, Makefile) | `--conferir`/`--senha-stdin`/`--cruzado` → `--verify`/`--password-stdin`/`--cross`; prefixo `dev-cofre:` → `dev-vault:` | opções e nome do programa (glossário; pendência da Fase 6) |
| `dev-vault.sh` | ataque de senha em argv `--senha` → `--password` (o `tests.rs` recusa as duas e as três opções velhas, sem alias) | nome novo da opção |
| `tests.rs` | prefixo temporário `ward-teste-dev-cofre-` → `ward-test-dev-vault-` | nome de arquivo temporário |
| `fake.c` + `container-platform/inside.sh` | `--ativar`/`--conectar` → `--activate`/`--connect` | glossário seção 6 |
| `container.rs` + `inside.sh` de plataforma e install | subcomandos `ativar`, `servidor`, `cliente`, `prender`, `lancar-app`, `escutar`, `ambiente` → `activate`, `server`, `client`, `attach`, `launch-app`, `listen`, `environment`; opções `--lancar`, `--abrir-app`, `--abrir-app-com`, `--sistema-em`, `--pausar` → `--launch`, `--open-app`, `--open-app-with`, `--system-at`, `--pause` | interface interna de teste, só entre arquivos desta fase |
| `chrome-parent.rs` + `nm.py` | subcomando `servir` → `serve` | idem |
| `docker-platform.sh` + `container-platform/inside.sh` | cópias `/opt/teste/chrome-falso`, `/opt/teste/wardd-cliente-teste` → `/opt/teste/fake-chrome`, `/opt/teste/wardd-test-client` | nomes dos exemplos |
| Dockerfiles `units`/`install`/`deb` | `/usr/local/sbin/iniciar` → `/usr/local/sbin/start` | o arquivo copiado já é `start.sh` |
| usos (`docker-uid.sh`, `units.sh`, `nm.sh`, `extension.sh`, `deb.sh`, `install.sh`) | textos de uso com os nomes novos dos exemplos | nomes dos exemplos |
| `clipboard.py` | código Python embutido: variável `alvos` → `targets`; mensagem "aceitou/recusou" montada numa variável antes do `print` (mesmo texto) | identificador dentro de string executada |

### Literais que ficam

Marcadores impressos e procurados pelos scripts (`servidor: pronto`, `papel=…`,
`lancador:`, `forcado:`, `gerente:`, `app-filho:`, `prender:`, `inicio`/`depois`/
`erro`/`queda`, `saida=`, `resposta:`, `pai=…`), subcomandos do `wardd-test-client`
(`estado`/`lista`/`destravar` = nomes de operação do protocolo, Fase 51), JSON do
protocolo v1, Debug dos erros conferidos (`UntrustedDir`, `CommonGroup`,
`AppParentNotAllowed`, `AlreadyOpen`, `Session(SeveralWayland)` …), log
`conexão recusada (UidRecusado)`, variáveis `WARD_GERENTE_PROGRAMA`/
`WARD_PAI_CHROME_SOCK` (Fase 56), nome do teste `area_gtk::testes::sessao_real` e o
protocolo de linhas do `clipboard.py` (vão com o teste do `app/src-tauri`), nomes de
dentro do contêiner ditos pelos scripts de fora (`/opt/teste`, `cliente`, `plataforma`,
`falso`, `ward-servidor`, `libmarca.so`, `/tmp/ward-app-relatorio.txt`) e o formato do
arquivo de estado (`ward-estado 1` …). Os comentários não citam esses literais pt
(descrevem em inglês); nenhuma entrada nova em `allow.txt` nem em `frozen.txt`.

## Segurança

- Invariantes tocadas: nenhuma lógica mudou; só nomes, comentários e literais de
  interface de teste listados acima. Senha de teste continua só por stdin
  (`--password-stdin`, `wardd-test-client destravar`); o ataque "senha em argv" segue no
  `dev-vault.sh` e no `tests.rs` (agora também recusa `--senha` e as opções velhas).
- Conferência: comparação de tokens Rust antigo × novo (nomes novos mapeados de volta,
  sem comentários) em todos os exemplos: só sobram os literais da tabela acima. Python:
  `tokenize` de strings/números/operadores do `clipboard.py` igual ao original fora das
  docstrings e do código embutido. Shell: renome por mapa só em posição de código e
  `$var`, revisado palavra por palavra; checagem de variáveis usadas e não definidas
  (nenhuma nova); `bash -n` pelo `syntax_check.py`; `fake.c`/`mark.c` compilam com
  `-Wall -Werror`.
- Testes de ataque: os existentes, renomeados (`attack_*` em `dev-vault/tests.rs`); nenhum
  teste novo de SEG (fase de nomes).
- Dependências novas: nenhuma.

## Ficou de fora

- Testes não rodados (PRD regra 6) nem Docker (pedido do orchestrator); `make check` não
  rodado.
- Nomes da API do `ward-core`/`ward-app-core` (`partida`, `iniciar`, `tomar_daemon`,
  `vigiar`, `Pedido`, `Segredo` …): 375 achados do `english-check` em `dev-vault/*`,
  `test-client.rs` e `startup-container.rs`, todos de API — fases R.
- Nomes de dentro do contêiner e marcadores de saída (lista acima): fases 44–46/51/56.
- Comentários pt dos scripts de fora (`docker-platform.sh`, `install.sh`, `deb.sh`,
  `nm.*`, `units.sh`, `docker-uid.sh`, `extension.sh`, `dev-vault.sh`): só os nomes
  foram trocados; o resto é das Fases 44–46.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `cargo fmt --all --check` (raiz e `app/src-tauri`) | 0 |
| `make compile` (log `fase43-make-compile.log`) | 0 (0 warnings) |
| `cargo clippy -p ward-core -p wardd -p ward-platform -p ward-nm -p ward-app-core --examples --all-features -- -D warnings` | 0 |
| `make english-check PATHS="dev/container-* dev/tests/clipboard.py container.rs chrome-parent.rs fake-chrome.rs"` | 0 (0 achados) |
| `make english-check PATHS="crates/ward-core/examples/dev-vault test-client.rs startup-container.rs"` | 2 (375 achados, todos nomes de API do `ward-core`/`ward-app-core`) |
| `python3 -I -B dev/english_check.py --self-test` | 0 (91/91) |
| `gcc -Wall -Werror -fsyntax-only fake.c mark.c` | 0 |

## Problemas descobertos

- **Merge com a Fase 10 (R)**: ela renomeia `limites` (`MAX_ANEXOS_SOMADOS`,
  `MINUTOS_TRAVA_PADRAO`) e outros nomes usados em `dev-cofre/main.rs`; aqui o arquivo
  foi movido para `dev-vault/main.rs` e reescrito. O git pode não ver o rename e dar
  conflito modify/delete: aplicar os renomes da Fase 10 em `dev-vault/main.rs` e
  `dev-vault/tests.rs`.
- As imagens `ward-teste/*-systemd:1` mudam de conteúdo (`/usr/local/sbin/start`); o
  `docker build` dos scripts refaz a camada sozinho, sem trocar a tag.
- `container.rs`: o marcador `prender:` (saída) ficou pt enquanto o subcomando virou
  `attach`; o script não procura esse marcador.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/core/examples/startup-container.rs` | renomeado (de `partida-conteiner.rs`) e alterado |
| `crates/ward-core/examples/dev-vault/main.rs` | renomeado (de `dev-cofre/main.rs`) e alterado |
| `crates/ward-core/examples/dev-vault/tests.rs` | renomeado (de `dev-cofre/testes.rs`) e alterado |
| `crates/ward-nm/examples/fake-chrome.rs` | renomeado (de `chrome-falso.rs`) e alterado |
| `crates/ward-platform/examples/chrome-parent.rs` | renomeado (de `pai-chrome.rs`) e alterado |
| `crates/wardd/examples/test-client.rs` | renomeado (de `cliente-teste.rs`) e alterado |
| `crates/ward-platform/examples/container.rs` | alterado |
| `crates/ward-core/Cargo.toml`, `crates/wardd/Cargo.toml` | alterado |
| `Makefile`, `CLAUDE.md`, `docs/glossario.md` | alterado |
| `dev/container-platform/{inside.sh,fake.c,mark.c}` | alterado |
| `dev/container-install/{inside.sh,Dockerfile}` | alterado |
| `dev/container-deb/{inside.sh,Dockerfile}` | alterado |
| `dev/container-units/{inside.sh,Dockerfile,start.sh}` | alterado |
| `dev/container-uid/inside.sh` | alterado |
| `dev/tests/clipboard.py` | alterado |
| `dev/tests/{deb.sh,dev-vault.sh,docker-platform.sh,docker-uid.sh,extension.sh,install.sh,nm.py,nm.sh,units.sh}` | alterado (só usos dos exemplos) |
| `docs/mission/handoffs/phase-43.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não: a missão 2 fecha a fase com `make compile` e `english-check` (regra 6)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG mudou; ataques existentes mantidos)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
