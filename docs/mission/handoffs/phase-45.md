# Fase 45 — `real-home.sh`, `webkit-real.sh`, `origins.py`, serviços web, `nm.py`, ataques ao socket, `generate.py`, `deb.sh`, `compose.yml` (Missão 2, trilha S)

## Implementado

O worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard 2cb1305` (master). A troca foi 1:1 pelo `docs/glossario.md`. Nos Python, renomeei só os tokens `NAME`, por `tokenize` e com um mapa por arquivo, e conferi por AST. Nos shell, reescrevi à mão e conferi por diff de tokens. Comentários e docstrings passaram para o inglês ("Fase" → "Phase", IDs da missão 1 sem prefixo). As mensagens de saída (`ok …`, `FALHA …`, `origens: …`, `dev-services: …`, `webkit-real: …`, `test-deb: …`) continuam em pt-BR e iguais (DT20).

- **`dev/origins.py`** (pré-checagem de segurança): só nomes e comentários. Renomes: `RAIZ` → `ROOT`, `CASA` → `PASSWD_HOME`, `PERMITIDAS_HOME`/`ENV_PERMITIDAS`/`TOOLCHAIN_PERMITIDAS` → `HOME_ALLOWED`/`ENV_ALLOWED`/`TOOLCHAIN_ALLOWED`, `ENV_DO_CARGO` → `CARGO_ENV`, `CANAL_FIXO` → `FIXED_CHANNEL`, `NOME_SIMPLES` → `SIMPLE_NAME`, `PULAR` → `SKIP`, `erros` → `errors`, `dentro` → `inside`, `conferir_*` → `verify_*`, `configs_no/fora_do_repositorio` → `configs_in/outside_repo`, `terminar` → `finish`, e os locais.
  - Pela AST, nenhuma constante, string ou operador mudou.
  - Os textos que o `attacks-foundation.sh` procura (`origens: `, `fonte fora do crates.io`, `.cargo/config* no repositório é proibido`, `toolchain ativo`, `config`) estão iguais. Por isso nenhum `grep` daquele script precisou mudar.
- **`dev/services/web/server.py`**: `pagina` → `page`, `responder` → `respond`, `ler_corpo` → `read_body`, `formulario` → `form`, `tela_wp`/`tela_totp`/`tela_cadastro` → `wp_screen`/`totp_screen`/`signup_screen`, `cadastrar`/`entrar` → `sign_up`/`log_in`, `totp_confere` → `totp_matches`, `LIMITE_*` → `*_LIMIT`, `TRAVA_*` → `*_LOCK`, `CONTAS_CARREGADAS` → `LOADED_ACCOUNTS` …
  - Variáveis de ambiente: `WARD_REGISTRO`/`WARD_CONTAS`/`WARD_DESTINO_REDIR`/`WARD_PORTA` → `WARD_RECORD`/`WARD_ACCOUNTS`/`WARD_REDIR_TARGET`/`WARD_LISTEN_PORT`, pelo glossário, seção 8. Só o `compose.yml` define uma delas (`WARD_LISTEN_PORT`), e mudou junto.
  - HTML, campos do formulário, chaves do `posts.log` e do `totp-contas.json` e caminhos do contêiner ficaram iguais.
- **`dev/services/generate.py`**: `RAIZ` → `ROOT`, `DESTINO`/`SEGREDOS`/`CHAVES` → `DEST`/`SECRETS`/`KEYS`, `SERVICOS_SSH` → `SSH_SERVICES`, `falhar` → `fail`, `gravar` → `write_file`, `diretorio` → `ensure_dir`, `gerar_*` → `generate_*`, `gravar_*` → `write_*` …
  - O `--help` usava a 1ª linha da docstring (`__doc__.splitlines()[0]`). Agora usa a constante `DESCRIPTION`, com o mesmo texto pt-BR byte a byte (conferido contra o original).
- **`dev/compose.yml`**:
  - âncoras `x-seguranca`/`seguranca` → `x-security`/`security` e `wp-ambiente` → `wp-env`;
  - rede `interno` → `internal`;
  - `WARD_MODO: senha|chave|salto|interno` → `WARD_MODE: password|key|jump|internal`, com o `case` e a leitura em `dev/services/ssh/entrypoint.sh` e o comentário do `Dockerfile` (uso 1:1, glossário seção 6). A imagem é refeita pelo `make dev-services-up` (`--build`);
  - comentários em inglês.
- **`dev/tests/nm.py`, `socket_attack.py`, `socket_matrix.py`**: renomes pelo padrão das Fases 40–42 (`ROOT`, `WORK_DIR`, `SOCKET_PATH`, `failures`/`fail`/`verify`, `read_exact`, `BAD_FRAMES`, `SLOTS`, `operations`, `session` …).
  - No `socket_matrix.py`, duas expressões `r.get('tipo')` dentro de f-string viraram a variável `reply_kind`, calculada antes. O texto impresso é o mesmo.
- **`dev/tests/webkit-real.sh`**: `RAIZ` → `ROOT`, `LANCADOR` → `LAUNCHER`, `valor_fixo` → `fixed_value`, `SAIDA` → `OUT_DIR`, `CASA` → `FAKE_HOME`, `MARCA` → `MARKS`, `isca` → `decoy`, `limpar` → `cleanup`, `LPID` → `LISTENER_PID`, `nao` → `not_ok`, e o Python embutido. As strings que ele procura no `main.rs` do app (`partida::iniciar()`, `tomar_daemon`, `vigiar`, `na_vigia`, `ErroApp::ConexaoPerdida`) e no `launcher.rs` (`FIXED_VARIABLES`) foram conferidas contra o master e ficaram iguais.
- **Defeito do plano (achado da Fase 6), corrigido**: `make test-webkit-real NO_MPS=1 NO_39C=1` passava `--no-mps--no-39c` grudado. Agora:
  - o Makefile passa as duas opções com espaço;
  - o script lê todos os argumentos depois do cargo (`--no-mps`, `--no-39c`, juntos ou não);
  - argumento desconhecido é recusado e o script sai 2. Antes ele era ignorado sem aviso, e agora falha fechado.
  - Conferido isolando o laço de argumentos: nenhum, um, os dois nas duas ordens, grudado (recusa) e lixo (recusa).
- **`dev/tests/real-home.sh`**: `CASA` → `PASSWD_HOME`, `rastrear`/`rastrear_ok` → `trace_run`/`trace_ok`, `acusados` → `flagged`, `conferir` → `verify`, `falhou`/`falhas` → `fail`/`failures`, `opcoes_mutadas` → `mutated_options`, `mutacao` → `mutation`, `controle_*` → `control_*`, e o Python embutido (`inside`, `parent_of`, `cwd_of`, `flagged` …).
  - Os rótulos internos dos traces também mudaram: `senha`/`errada`/`chave`/`interno`/`metodos`/`canal`/`keygen-novo`/`controle-*`/`m-sem-os-dois` → `password`/`wrong`/`key`/`internal`/`methods`/`channel`/`keygen-new`/`control-*`/`m-without-both`.
  - O diff de tokens mostra só esses pares, com mapeamento consistente.
- **`dev/tests/deb.sh`**: `SO_HOST` → `HOST_ONLY` (opção interna `--so-host` → `--host-only`, que ninguém mais passa), `SEM` → `NO_TM`, `CLIENTE` → `CLIENT`, `NOME` → `CONTAINER_NAME`, `IMAGEM` → `IMAGE`, `confere`/`falhou`/`recusa` → `verify`/`fail`/`refusal` …
  - Nomes temporários em `$TMP`: `extraido` → `extracted`, `conteiner` → `container`, `outra/aberta.pem` → `other/open.pem`, `outro.crx` → `other.crx`, `saida/` → `out/`, `plantado` → `planted`.
  - Os textos procurados em saídas de `install/` (`modo-teste`, `não de EXTENSION_ID`, `não do ID_EXTENSAO`, `garantir_grupo` …), os caminhos de staging (`$D/raiz`) e os do contêiner (`/opt/teste`, `bin/cliente`) ficaram iguais.
- **`allow.txt`**: os quatro serviços `ssh-senha`, `ssh-chave`, `ssh-salto` e `ssh-interno` são nomes de host gravados nas entradas do cofre de teste (`entries.json` e as fixtures legadas 0.1.4) e nomes dos arquivos de segredo em `.dev/services/`, então são dado e ficam.
- **Glossário**: entrou um parágrafo da Fase 45 na seção 4, com os nomes recorrentes, as variáveis trocadas antes da Fase 56 e o que ficou para as Fases 46 e 56.

### Literais alterados (PRD regra 5)
- Nomes de variável de ambiente: `WARD_MODE` e os valores do modo, `WARD_LISTEN_PORT`, `WARD_RECORD`, `WARD_ACCOUNTS`, `WARD_REDIR_TARGET`. Junto mudaram as mensagens `WARD_MODE não definido`/`desconhecido` do `entrypoint.sh`.
- `webkit-real.sh`:
  - linha de uso `[--no-mps] [--no-39c]` e a mensagem nova `opção desconhecida`;
  - caminhos em `.dev/webkit-real/` (`saida/` → `out/`, `casa` → `home`, `marcas` → `marks`);
  - nomes das iscas (`*marca*.so` → `*mark*.so`, `urn:ward:mark`) e o C gerado (`mark()`, macro `LABEL`).
- `real-home.sh`: os rótulos dos traces (acima) e `$TMP/wrong-password`/`$TMP/new-key`.
- `deb.sh`: `--host-only` e os nomes em `$TMP` (acima).
- `generate.py`: o exemplo do uso na docstring passou a ser `<ssh service>` (a docstring não é impressa; o `--help` usa `DESCRIPTION`).

## Segurança
- **Invariantes tocadas**:
  - **S14** (`origins.py`): a lógica não mudou. Pela AST, as listas de permitidas, as regex, a ordem dos passos (nada roda antes dos passos 1–4) e as mensagens são as mesmas.
  - **S2/S15** (`generate.py`): continua com gravação atômica `O_EXCL|O_NOFOLLOW`, modos 0600/0644/0700, passphrase só pelo pty e nada fora do repositório. Pela AST, nada mudou.
  - **S1/S5** (`socket_matrix.py`, `socket_attack.py`): os mesmos ataques, limites e expectativas.
  - **D47/V6.1** (`nm.py`): igual.
  - **V5.32/D64** (`webkit-real.sh`): as conferências são as mesmas. Com a correção do defeito, `NO_MPS=1 NO_39C=1` passa a tirar as duas proteções juntas, como o Makefile descreve.
  - **R13** (`real-home.sh`): o detector e as mutações são os mesmos.
- **`WARD_HOME_COBERTO` ficou de propósito.** Ele é a guarda de reexecução no bwrap, e o `ssh-home.sh` (Fase 44, em paralelo) e o `home-decoys.sh` o tiram com `env -u`. Se eu trocasse só aqui, o `env -u` deles viraria no-op para o nome novo. Pelo glossário, ele vai para a Fase 56, nos três arquivos juntos.
- **Testes de ataque**: nenhum novo (a fase é de nomes, mais a correção do argumento). Escritos, não rodados (PRD regra 6): `make test-real-home`, `test-webkit-real`, `test-nm`, `test-socket-attacks`, `test-deb` e `test-attacks-foundation` rodam no M6.
- **Dependências novas**: nenhuma.

## Ficou de fora
- **Contrato do `dev/services/common.sh` (Fase 46)**: `RAIZ` em `real-home.sh` e `deb.sh` (o `common.sh` lê `$RAIZ` de quem o carrega), `OPCOES_SSH`, `IMAGEM_PG`, `DS`, `SEG` e as funções `exigir`, `cliente`, `ssh_senha`, `ssh_chave`, `ssh_interno`, `wp_login`, `psql_app`. São os únicos achados que sobram do `english-check` nos meus arquivos, mais o `WARD_HOME_COBERTO`.
- **Fase 56**: `WARD_HOME_COBERTO`, `WARD_TESTE_PG_SENHA_ARQUIVO` e `WARD_PAI_CHROME_SOCK` (literais), mais `EXTENSAO_DIST`/`CRX_SAIDA` passados ao `pack-extension.sh` (Fase 47/56).
- **Literais de fio e de outros arquivos**: `destravar`, `lista`, `preencher`, `nao_autorizado` …, o `protocolo.rs` e `Operacao,`/`tabela!` lidos pelo `socket_matrix.py`, os exemplos `pai-chrome`, `partida-conteiner` e `wardd-cliente-teste`, e os textos de `install/`.
- **Suítes e Docker**: Milestone 6. A validade do `compose.yml` foi conferida só por leitura, porque o `docker compose config` e o PyYAML ficaram de fora.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 2cb1305` (árvore limpa) | 0 |
| conferência por AST dos 6 Python (original × novo, sem docstrings; colisões só de `os.path`/kwarg `file`/`mode`/atributo `name`) | 0 problemas reais |
| diff de tokens dos shell (`real-home.sh`, `deb.sh`) e revisão do diff do `webkit-real.sh` | só os pares listados |
| `python3 -I dev/english_check.py --paths` dos 8 arquivos limpos + `allow.txt` | 0 (0 achados) |
| `english-check` em `real-home.sh` e `deb.sh` | 1 (só `RAIZ`, `OPCOES_SSH`, `IMAGEM_PG` e `WARD_HOME_COBERTO`; Fases 46/56) |
| `python3 -I -B dev/english_check.py --self-test` | 0 (91/91) |
| `make -n test-webkit-real NO_MPS=1 NO_39C=1` → `--no-mps --no-39c` | 0 |
| `make compile` (log em `.dev/fase45/fase45-make-compile.log`) | 0 (origens, `cargo check` do workspace e do `app/src-tauri`, Vite, sintaxe de 48 shell, 35 Python e 7 JS) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- **Conflitos previsíveis de merge**:
  - a Fase 43 renomeia os exemplos `partida-conteiner`/`wardd-cliente-teste`/`pai-chrome` "com todos os usos": as linhas `APP=`/`CLIENT=` do `deb.sh` e a docstring do `nm.py` daqui também mudaram;
  - a Fase 44 mexe no `ssh-home.sh`, que usa `WARD_HOME_COBERTO`, igual aqui.
- **Fase 17 (protocolo)**: o `socket_matrix.py` lê `crates/ward-core/src/protocolo.rs` e procura `Operacao, `, `tabela!` e os papéis `TODOS`/`AGENTE_E_APP`/`SO_AGENTE`. Quem renomear isso tem de ajustar o script.
- **Fase 46**: ao renomear `RAIZ` no `common.sh`, ajustar `real-home.sh`, `deb.sh` e os outros que fazem `source`. Ao renomear `OPCOES_SSH`/`IMAGEM_PG`/`DS`/`SEG`, ajustar o `declare -p` do `real-home.sh`.
- **Fase de app (main.rs)**: o `webkit-real.sh` procura `partida::iniciar()`, `tomar_daemon`, `vigiar`, `na_vigia` e `ErroApp::ConexaoPerdida`. Quem renomear isso tem de ajustar o heredoc junto, senão o teste aborta com "main.rs do app mudou".

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/origins.py` | alterado |
| `dev/services/web/server.py` | alterado |
| `dev/services/generate.py` | alterado |
| `dev/services/ssh/entrypoint.sh`, `dev/services/ssh/Dockerfile` | alterado (só `WARD_MODE`) |
| `dev/compose.yml` | alterado |
| `dev/tests/nm.py`, `socket_attack.py`, `socket_matrix.py` | alterado |
| `dev/tests/real-home.sh`, `webkit-real.sh`, `deb.sh` | alterado |
| `Makefile` | alterado (receita do `test-webkit-real`) |
| `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-45.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram `make compile` (0), o `english-check` e o autoteste.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon, nenhum Docker).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem iguais, conferidos por AST e por tokens.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase. A exceção é o uso 1:1 de `WARD_MODE` no `entrypoint.sh`/`Dockerfile` (Fase 46).
