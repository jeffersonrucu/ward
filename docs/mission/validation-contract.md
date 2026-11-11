# Contrato de validação

Missão 2: código em inglês. Escrito a partir de `docs/mission/prd.md` antes de qualquer
código. O contrato da missão 1 está em `docs/mission/arquivo/m1/validation-contract.md`:
as asserções dele continuam valendo, com os nomes novos, e são reexecutadas pelas suítes
citadas aqui.

**Quando se confere** (pedido do usuário, 2026-11-10; PRD regra 6): as asserções ficam
agrupadas pelo milestone que implementa o que elas cobrem, mas **todas** são conferidas
só no **Milestone 6** (bateria única). Os Milestones 1–5 fecham com `make compile` e
`make english-check PATHS=…` limpos em cada fase. O orchestrator não chama os
validadores (scrutiny, security-auditor, QA) antes do Milestone 6. O que falhar no M6
vira fase de correção.

Marcas: **(SEG)** o security-auditor executa o ataque; **(UI)** o QA confere pela
interface. Nos comentários de código, os IDs deste contrato levam o prefixo `m2:`
(PRD DT2).

## Cofre de teste (D15)

Senha mestra `falso-mestra-ward-01`, PIN `135790`. Todos os valores são falsos. Tipos e
níveis com os nomes do corpo v5 (PRD DT4); o app e a tabela da CLI mostram os rótulos
em pt-BR. O `make test-dev-vault` lê esta tabela (nome, tipo e nível).

| Entrada | Tipo | Grupo | Ambiente | Nível | Dados |
|---|---|---|---|---|---|
| `acme-wp-dev` | login | acme | dev | approval | URL `http://localhost:8081/wp-login.php`, usuário `wp_admin`, senha `falso-WP-senha-01`, TOTP semente `JBSWY3DPEHPK3PXP` |
| `acme-wp-prod` | login | acme | prod | free | URL `http://localhost:8081/wp-login.php`, usuário `wp_editor`, senha `falso-WP-senha-02`, TOTP semente `JBSWY3DPEHPK3PXP` |
| `acme-pg-dev` | database (postgres) | acme | dev | free | `127.0.0.1:55432`, base `app_teste`, usuário `app`, senha `falso-PG:s3nha/01@#` |
| `acme-pg-aprovacao` | database (postgres) | acme | stage | approval | igual a `acme-pg-dev` |
| `acme-pg-prod` | database (postgres) | acme | prod | approval | igual a `acme-pg-dev` |
| `acme-pg-tunel` | database (postgres) | acme | stage | free | host `postgres`, porta 5432, túnel por `acme-ssh-senha-stage`, mesmo usuário/senha |
| `acme-ssh-senha-stage` | server | acme | stage | free | `127.0.0.1:2222`, usuário `deploy`, senha `falso-SSH-senha-01` |
| `acme-ssh-chave-prod` | server | acme | prod | free | `127.0.0.1:2223`, usuário `deploy`, chave ed25519 gerada em `.dev/`, passphrase `falso-passphrase-01` (servidor só aceita chave) |
| `acme-ssh-salto` | server | acme | prod | approval | `127.0.0.1:2224`, usuário `salto`, senha `falso-SALTO-01` |
| `acme-ssh-salto-livre` | server | acme | prod | free | igual a `acme-ssh-salto` |
| `acme-ssh-interno` | server | acme | prod | free | host `ssh-interno`, porta 22 (só na rede do compose), usuário `deploy`, chave de `acme-ssh-chave-prod`, salto `acme-ssh-salto-livre` |
| `acme-ssh-interno-aprov` | server | acme | prod | free | igual a `acme-ssh-interno`, salto `acme-ssh-salto` |
| `beta-github-api` | api | beta | dev | free | URL `https://api.github.com`, token `falso_token_github_01`, variável `GITHUB_TOKEN` |
| `beta-aws-dev` | aws | beta | dev | free | access key `FALSOACCESSKEY01`, secret `falso-aws-secret-01`, região `sa-east-1`, session token `falso-aws-session-01`, role `arn:aws:iam::000000000000:role/teste` |
| `beta-smtp-stage` | smtp | beta | stage | free | `smtp.teste.invalid:587`, usuário `mailer`, senha `falso-SMTP-01`, segurança `starttls` |
| `beta-sftp-dev` | ftp-sftp | beta | dev | free | `sftp`, `127.0.0.1:2222`, usuário `sftpuser`, senha `falso-SFTP-01` |
| `gama-nota` | secure-note | gama | dev | free | texto `falso-nota-01` |
| `gama-generico` | generic | gama | stage | free | extras `REGIAO=r-01` (não segredo), `CHAVE_LICENCA=falso-licenca-01` (segredo); anexo `app.env` com `FALSO_ENV=falso-anexo-01` |
| `gama-bloqueada` | login | gama | prod | blocked | URL `https://bloqueada.invalid`, usuário `x`, senha `falso-BLOQ-01` |
| `delta-redir` | login | delta | dev | free | URL `http://localhost:8084/login` (redireciona para o phishing), usuário `u`, senha `falso-REDIR-01` |
| `acme-totp-dev` | login | acme | dev | free | URL `http://localhost:8083/login`, usuário `wp_admin`, senha `falso-WP-senha-01`, TOTP semente `JBSWY3DPEHPK3PXP` |
| `delta-api-simbolos` | api | delta | dev | free | URL `https://api.teste.invalid`, token `falso-Ab3$k&<9>Zq!'/ç-01`, variável `SIMBOLOS_TOKEN`; extra secreto `SEGREDO_ASCII=falso-Ab3$k&<9>Zq!'` |

Nomes de entrada, grupos, nomes de extra e de anexo são dados do usuário: não mudam.
Serviços de teste: os da missão 1 (`dev/compose.yml`, portas 55432, 2222, 2223, 2224,
8081, 8082, 8083, 8084).

## Entrega esperada (PRD DT11), usada em V4 e V6

| Entrada | Variáveis e conteúdo esperado |
|---|---|
| `acme-wp-prod` | `WARD_URL=http://localhost:8081/wp-login.php`, `WARD_USER=wp_editor`, `WARD_PASS=falso-WP-senha-02`, `WARD_TOTP=<6 dígitos iguais a oathtool --totp -b JBSWY3DPEHPK3PXP no mesmo período>` |
| `acme-pg-dev` | `PGHOST=127.0.0.1`, `PGPORT=55432`, `PGDATABASE=app_teste`, `PGUSER=app`, `PGPASSWORD=falso-PG:s3nha/01@#`, `DATABASE_URL=postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste` |
| `acme-pg-tunel` | `PGHOST=127.0.0.1`, `PGPORT=<porta local do túnel, ≠5432>`, demais como `acme-pg-dev`, `DATABASE_URL` com o mesmo host/porta locais |
| `acme-ssh-senha-stage` | `WARD_HOST=127.0.0.1`, `WARD_PORT=2222`, `WARD_USER=deploy`, `WARD_PASS=falso-SSH-senha-01` |
| `acme-ssh-chave-prod` | `WARD_HOST=127.0.0.1`, `WARD_PORT=2223`, `WARD_USER=deploy`, `WARD_FILE_KEY=<rt>tmp/…` (arquivo 0600 com a chave privada), `WARD_PASSPHRASE=falso-passphrase-01` |
| `beta-github-api` | `WARD_URL=https://api.github.com`, `WARD_TOKEN=falso_token_github_01`, `GITHUB_TOKEN=falso_token_github_01` |
| `beta-aws-dev` | `AWS_ACCESS_KEY_ID=FALSOACCESSKEY01`, `AWS_SECRET_ACCESS_KEY=falso-aws-secret-01`, `AWS_SESSION_TOKEN=falso-aws-session-01`, `AWS_REGION=sa-east-1`, `AWS_DEFAULT_REGION=sa-east-1`, `AWS_ROLE_ARN=arn:aws:iam::000000000000:role/teste` |
| `beta-smtp-stage` | `SMTP_HOST=smtp.teste.invalid`, `SMTP_PORT=587`, `SMTP_USER=mailer`, `SMTP_PASS=falso-SMTP-01`, `SMTP_SECURITY=starttls` |
| `beta-sftp-dev` | `WARD_PROTOCOL=sftp`, `WARD_HOST=127.0.0.1`, `WARD_PORT=2222`, `WARD_USER=sftpuser`, `WARD_PASS=falso-SFTP-01` |
| `gama-nota` | nenhuma; `ward run` sai com erro "tipo sem entrega" e não executa o comando |
| `gama-generico` | `WARD_REGIAO=r-01`, `WARD_CHAVE_LICENCA=falso-licenca-01`, `WARD_FILE_APP_ENV=<rt>tmp/…` (conteúdo `FALSO_ENV=falso-anexo-01`) |
| todas | `WARD_ENTRY=<nome>`, `WARD_GROUP=<grupo>`, `WARD_ENVIRONMENT=<ambiente>` |
| nenhuma | nenhuma variável `WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`, `WARD_PROTOCOLO`, `WARD_ARQUIVO_*` |
| smtp (qualquer) | `SMTP_SECURITY` com o nome v5 da segurança (DT4): `none`, `starttls` ou `tls`; a 0.1.x entregava `nenhuma` no lugar de `none` (achado da Fase 48, registrado na Fase 53) |

## Milestone 0 — Design no FigCraft

Pulado (resposta 1 do usuário, 2026-11-10; escopo "Missão 2 — Código em inglês"):
nenhuma tela muda. Sem asserções. `Ward.figcraft` e `docs/mission/design-nodes.md`
ficam como estão.

## Milestone 1 — Base da tradução

- [ ] V1.1 — Dado `docs/glossario.md`, quando o validador procura cada termo pt-BR de
  identificador do repositório (módulos, arquivos, crates, pastas, alvos, variáveis,
  subcomandos, operações, campos e erros do protocolo, nomes gravados, comandos do
  Tauri), então cada um tem uma única tradução, e as tabelas de DT4, DT8, DT11, DT12,
  DT13, DT14 e DT15 do PRD estão no glossário sem conflito.
- [ ] V1.2 — Dado o checador (`dev/english_check.py`), quando o autoteste planta numa
  cópia do repositório um identificador pt-BR e um comentário pt-BR em cada linguagem
  (Rust, JS, Svelte, CSS, shell, Python, Makefile, unit do systemd, TOML, YAML,
  Dockerfile, C) e um arquivo e uma pasta com nome pt-BR, então ele falha em cada
  plantio, citando arquivo e linha; e com um literal pt-BR (`"senha incorreta"`) ou um
  comentário em inglês com "de" ou "a" soltos, ele não acusa.
- [ ] V1.3 — Dada uma entrada de `allow.txt` ou `frozen.txt` que não casa com nada,
  quando o checador roda, então ele falha ("exceção sem uso").
- [ ] V1.4 — (SEG) Dado o checador, quando o validador lê o código, então ele só usa a
  biblioteca padrão (nenhum `pip`, nenhuma dependência nova em `Cargo.lock` nem em
  `package-lock.json`), roda com `python3 -I` e não segue link simbólico para fora do
  repositório. Cobre S14.
- [ ] V1.5 — Dado `make compile`, quando roda num repositório limpo, então faz
  `cargo check` do workspace (todos os alvos e features), `cargo check` do
  `app/src-tauri` (se houver WebKitGTK), o build do Vite, `node --check` da extensão,
  `bash -n` dos scripts e `py_compile` do Python, passa pela pré-checagem das origens e
  não roda teste nenhum.
- [ ] V1.6 — Dado `dev/fixtures/legacy/`, quando o validador confere o `SHA256SUMS` e
  abre `vault-0.1.4.ward` com o `wardd` 0.1.4 de `.dev/baseline/` e a senha de teste,
  então os arquivos batem e o cofre tem as 22 entradas, corpo v4; `protocol-v1.jsonl`
  tem uma linha para cada operação e resposta da v1.
- [ ] V1.7 — (SEG) Dado o repositório depois da Fase 3, quando o gitleaks varre árvore,
  índice e histórico (`make gitleaks`), então nada é acusado, e `dev/fixtures/legacy/`
  não tem valor fora dos falsos de D15. Grep dos valores falsos de D15 no
  `vault-0.1.4.ward` e nos backups não acha nada (estão cifrados). Cobre S15 e S2.
- [ ] V1.8 — Dados os renomes estruturais (DT13), quando o validador lista o
  repositório, então não existem `crates/ward-plataforma`, `crates/ward-ponte-teste`,
  `app/nucleo`, `instalacao/`, `extensao/`, `dev/testes/`, `dev/servicos/`,
  `dev/conteiner-*` nem `dev/fixtures/entradas.json`, e `cargo metadata` lista
  `ward-platform`, `ward-test-bridge`, `ward-app-core` e a feature `test-mode` (nenhuma
  `modo-teste`).
- [ ] V1.9 — (SEG) Dada a chave da extensão movida para `.dev/extension/key.pem`, quando
  o validador calcula o id dela, então é igual ao `EXTENSION_ID` do `ward-nm` e ao id da
  0.1.4; o arquivo é 0600, do usuário, não é link, e `.dev/extensao/chave.pem` não
  existe. Nenhuma chave nova foi gerada (o id não mudou). Cobre S15.
- [ ] V1.10 — Dado o Makefile, quando o validador roda `make <alvo antigo>` para cada
  alvo da coluna "Antigo" de DT14, então o make falha com "No rule to make target"; e
  cada alvo novo existe. O `CLAUDE.md` lista só os nomes novos.

## Milestone 2 — Rust em inglês (fio congelado)

- [ ] V2.1 — Dado cada crate do workspace e `app/src-tauri`, quando o checador roda em
  `crates/` e `app/core`, `app/src-tauri`, então não há identificador, comentário,
  arquivo nem módulo em pt-BR (só as exceções de `allow.txt`, cada uma com motivo).
- [ ] V2.2 — (SEG) Dado o commit do fim do Milestone 3 (o último antes da Fase 48, num
  `git worktree` em `.dev/`), quando o validador roda nele os testes de fio da Fase 3,
  então cada pedido e resposta serializados são iguais, byte a byte, a
  `dev/fixtures/legacy/protocol-v1.jsonl`, e o corpo serializado do cofre de teste é
  igual ao de `vault-0.1.4.ward` (v4). Os commits das Fases 7–47 não alteram esses
  testes nem as fixtures (`git log -- <arquivos>`). Cobre RM1 e S1.
- [ ] V2.3 — (SEG) Dado um rename na conferência de papel (`ward-platform`
  `role`/`peer`), quando o validador compara a lógica das funções de conferência (papel
  pelo egid, `SO_PEERCRED`, pai do `ward-nm`, servidor instalado) com a da 0.1.4, então
  o diff é só de nomes e comentários; e o ataque da missão 1 (`make
  test-docker-platform`: `ptrace` e `LD_PRELOAD` contra o setgid, servidor falso do
  mesmo uid, `bind` em `/run/ward/1000`) continua falhando. Cobre S5 e S8.
- [ ] V2.4 — (SEG) Dado o cofre de teste, quando um cliente de papel agente (Python, pelo
  socket de teste) manda cada operação da matriz operação × entrada × campo do
  `make test-socket-attacks`, então nenhuma resposta tem padrão de
  `.dev/variations.txt`, e app/extensão sem o grupo recebem `unauthorized`. Quadros de
  2 MiB, tamanho negativo, JSON inválido e 10 000 conexões caladas não derrubam o
  daemon. Cobre S1 e S5.
- [ ] V2.5 — (SEG) Dada a máscara com os nomes novos, quando `ward run acme-pg-dev --`
  imprime `$PGPASSWORD` (inteiro), a senha quebrada em dois `write` no mesmo pedaço,
  `falso-PG%3As3nha%2F01%40%23` (URL-encoded), o base64 da senha e `$DATABASE_URL`,
  então a saída devolvida tem `••••` no lugar de cada um e o `grep -F -f
  .dev/variations.txt` não acha nada. A escrita byte a byte com pausa deixa em claro só
  o começo que já saiu (risco aceito em S4). Cobre S4.
- [ ] V2.6 — (SEG) Dado o daemon de release, quando o validador o inicia com
  `LD_PRELOAD=/tmp/x.so` (e `LD_AUDIT`, `LD_LIBRARY_PATH`), então ele recusa iniciar;
  rodando, `/proc/<pid>/status` mostra dumpable 0 e um `gdb -p`/`strace -p` do mesmo
  uid falha; `kill -SEGV` não deixa core em lugar nenhum. Cobre S8.
- [ ] V2.7 — (SEG) Dado o daemon destravado com um `ward run` em curso e um `ward ssh`
  aberto, quando `ward lock` roda, então os dois são mortos (inclusive um descendente
  que fez `setsid`), a CLI sai 137 com "o cofre travou; o comando foi encerrado", e um
  dump da memória do daemon (como root, no contêiner do `test-units`) depois da trava
  não tem padrão de `.dev/variations.txt`. Cobre S8.
- [ ] V2.8 — (SEG) Dado o daemon com `RUST_LOG=trace`, quando toda a bateria do M6 roda,
  então o `.dev/logs/wardd.log` e os logs do `ward-nm`, do app, da ponte e da CLI não
  têm padrão de `.dev/variations.txt` (`dev/tests/log-scan.py`). Cobre S13.

## Milestone 3 — Web e scripts em inglês (fio congelado)

- [ ] V3.1 — Dados `app/src`, `extension/`, `dev/`, `install/`, o Makefile, as units e
  os arquivos de configuração, quando o checador roda, então não há identificador,
  comentário, arquivo nem pasta em pt-BR (fora de `allow.txt`).
- [ ] V3.2 — (UI) Dado o app no Chrome em modo de teste (ponte de teste), quando o QA
  percorre destravar, primeiro uso, Credenciais vazia e cheia, filtros e busca, o
  formulário de cada um dos 9 tipos (novo e edição, anexos, TOTP), nível de acesso com
  PIN, gerador, Configurações, trocar PIN, trocar senha, restaurar backup, escolher
  pendrive, kit de emergência e a janela de aprovação (normal, `prod`, PIN errado, PIN
  bloqueado, expirando), então cada tela tem os mesmos textos pt-BR, estados e layout
  das capturas de `docs/mission/arquivo/m1/evidence/final/` e dos nodes de
  `docs/mission/design-nodes.md` (tolerância de antialiasing; nenhuma diferença de
  texto, cor, espaçamento ou ordem).
- [ ] V3.3 — (UI) Dado o app, quando o QA navega só pelo teclado (Tab, Shift+Tab,
  Enter, Esc, setas nas listas e nos selects), então a ordem de foco, o foco visível e
  os atalhos são os da missão 1, e o Esc fecha diálogos e a janela de aprovação nega.
- [ ] V3.4 — (UI)(SEG) Dado o app com o cofre de teste aberto, quando o QA (e o
  `make test-revealed-selection`) varre o DOM, os atributos, o `title`, o
  `localStorage`/`sessionStorage` e as mensagens da ponte em cada tela, então nenhum
  padrão de `.dev/variations.txt` aparece fora do campo depois de "revelar", e "ocultar"
  o tira do DOM. Cobre S11.
- [ ] V3.5 — (UI) Dado o popup da extensão no Chrome real headless (`make
  test-extension`), quando o QA abre as sugestões do domínio, salvar login novo e cofre
  travado, então os textos pt-BR e as medidas batem com os frames 17332/17392 de
  `design-nodes.md`, como na missão 1.
- [ ] V3.6 — Dados os tokens visuais (`app/src/lib/style/tokens.css`) com nomes novos,
  quando o validador compara os valores computados (cor, tamanho, raio, espaçamento)
  de cada token da 0.1.4 com o novo correspondente pelo glossário, então são iguais, e
  `extension/dist/` usa os mesmos tokens.
- [ ] V3.7 — Dados os scripts de `dev/`, quando o validador lê os cabeçalhos, então todo
  script continua com `#!/bin/bash -p`, `export PATH=/usr/bin:/bin` e `. dev/env.sh`
  (o nome novo de `ambiente.sh`), e todo cliente ssh/curl de teste passa pelo
  `dev/services/common.sh` com `IdentityFile=none`, `CertificateFile=/dev/null` e
  `-q`.

## Milestone 4 — Formato e protocolo novos

Formato e cripto:

- [ ] V4.1 — (SEG) Dado `vault-0.1.4.ward` (v4), quando o `wardd` 0.2.0 tenta abrir com a
  senha errada, então falha com `wrong_password`, não grava nada (mtime, inode e bytes do
  arquivo e da pasta iguais) e não cria temporário. O mesmo vale para um cofre v5. Cobre
  S6.
- [ ] V4.2 — (SEG) Dado `vault-0.1.4.ward` e um cofre v5, quando o validador altera 1 byte
  em cada posição (cabeçalho, sal, parâmetros, nonce, chave embrulhada, corpo cifrado,
  tag; teste exaustivo nas posições do cabeçalho e por amostra de 4 KiB no resto), então
  toda abertura falha, nenhuma migra e nada é gravado. Cobre S6.
- [ ] V4.3 — (SEG) Dado `vault-0.1.4.ward` destravado pela primeira vez no daemon 0.2.0,
  quando o validador mata o daemon com SIGKILL em cada ponto da gravação da migração
  (antes do temporário, depois do write, depois do fsync, depois do rename, antes do
  fsync da pasta, antes do estado), então em todo caso o arquivo do cofre é a v4
  original (byte a byte) ou a v5 completa e abrível, nunca outra coisa. A próxima
  abertura funciona e migra se ainda for v4. Cobre S7.
- [ ] V4.4 — (SEG) Dada a migração em curso, quando o validador observa a pasta do cofre
  (`inotifywait`/`strace -f -e trace=openat,write,rename` no contêiner do
  `test-units`) e lê cada arquivo criado (temporário, versão do backup, estado), então
  nenhum tem padrão de `.dev/variations.txt` nem os nomes de tipo em claro; só bytes
  cifrados com a magia `WARDCOFR` e o estado sem segredo. Cobre S2.
- [ ] V4.5 — Dado o cofre migrado, quando o validador compara pelo protocolo (papel app
  da ponte de teste: `detail`, `reveal` de cada campo, `totp`, `config`) com o mesmo
  pelo `wardd` 0.1.4 em `vault-0.1.4.ward`, então as 22 entradas têm os mesmos nomes,
  grupos, ambientes, projeto, tags, campos, extras, anexos (bytes), sementes TOTP
  (mesmos códigos no mesmo período), datas, níveis (pela tabela de DT4), e a mesma
  configuração (tempo de trava 240 min, backup externo desligado; o PIN `135790`
  confere).
- [ ] V4.6 — Dado o cofre migrado, quando o validador o abre de novo e trava e destrava
  três vezes, então ele não é regravado (inode, mtime e geração iguais) e o corpo diz
  versão 5.
- [ ] V4.7 — (SEG) Dado o cofre migrado (geração g+1), quando um processo do usuário
  troca só o arquivo do cofre pela cópia v4 (geração g) com o daemon parado e o
  destrava, então o daemon recusa com `rollback` (o estado guarda g+1), como na missão
  1. Com o daemon rodando, o resultado depende do estado dele: **destravado**, a
  gravação seguinte dá `vault_changed` e não sobrescreve nada; **travado**, o destrave
  relê o arquivo e dá `rollback` (ajuste da Fase 66, achado da Fase 64). Cobre a
  conferência de geração (D39/D53).
- [ ] V4.8 — (SEG) Dado um corpo v4 montado no teste com um nome desconhecido (tipo
  `banco2`, nível `livre ` com espaço, nível em inglês `free` num corpo v4, tipo
  `database` num corpo v4), quando o núcleo o lê, então recusa com `invalid_vault`, sem
  migrar nem gravar; o mesmo para um corpo v5 com nome pt-BR (`banco`). Cobre RM2.
- [ ] V4.9 — Dados os vetores conhecidos (Argon2id e XChaCha20-Poly1305 da missão 1,
  `ward-core` `crypto`), quando os testes rodam, então dão os mesmos bytes; e o
  cabeçalho de um cofre v5 novo tem os mesmos campos e tamanhos que o v4 (magia,
  versão do formato 1, parâmetros ≥ 256 MiB / 3 passes). Cobre S6.
- [ ] V4.10 — Dado `backup-0.1.4/` (v4) mais versões v5 geradas depois da migração,
  quando o app restaura cada uma, então a v4 restaurada abre com os dados dela e é
  gravada como v5 com geração nova; a rotação local e a do pendrive contam v4 e v5
  conferidas pela geração selada (30 mais novas), e lixo com nome de versão, versão de
  outro cofre e cópia renomeada não ocupam lugar. Cobre S7 e a rotação ("Backup").
- [ ] V4.11 — Dado o estado `state-0.1.4` (`ward-estado 1`), quando o daemon 0.2.0 sobe,
  então lê as gerações e as senhas erradas do agente com o mesmo valor e, na próxima
  gravação, grava `ward-state 2`, de forma atômica e 0600, com o nome `estado` no disco
  da release. Estado com link, FIFO ou dono errado é recusado, como antes.
- [ ] V4.12 — (SEG) Dado o `wardd` 0.1.4 de `.dev/baseline/`, quando abre o cofre já
  migrado (v5), então recusa sem gravar nada (com o estado `ward-estado 1`, a recusa
  do 0.1.4 é `cofre_invalido`, "o conteúdo do cofre é inválido", log "corpo
  inválido"; com o estado `ward-state 2`, `retrocesso`; ajuste da Fase 66, achado da
  Fase 64: o binário 0.1.4 não muda), e o arquivo continua abrindo na 0.2.0 (DT7).

Protocolo v2:

- [ ] V4.13 — (SEG) Dado o daemon 0.2.0, quando um cliente manda cada linha de pedido de
  `dev/fixtures/legacy/protocol-v1.jsonl` (v1), então toda resposta é `error`
  `unknown_version`, sem campo de dado, e a conexão fecha. Com o cofre destravado,
  nenhuma resposta tem padrão de `.dev/variations.txt`. Cobre S1 e RM3.
- [ ] V4.14 — (SEG) Dado o daemon 0.2.0, quando um cliente v2 manda um pedido com um nome
  de campo antigo (`entrada`, `entrada_padrao`, `confirmacao…`, `nivel`), um campo
  extra desconhecido ou a operação antiga (`executar`, `revelar`, `destravar`), então o
  daemon recusa (`invalid_request`/`unknown_operation`) e não executa nada; um
  `run` com o tipo de entrada padrão omitido ou com o nome antigo não é tratado como
  terminal. Cobre RM1 e RA11.
- [ ] V4.15 — (SEG) Dado o protocolo v2, quando um teste lista todas as operações,
  respostas, campos, códigos de erro e papéis, então nenhum nome está em pt-BR (pela
  lista do checador) e a matriz de papel de cada operação é a da v1 (agente não tem
  `reveal`, `copy`, `detail`, `fill`, `suggest`, `set_level`, `restore_backup`…). Cobre
  S1 e S9.
- [ ] V4.16 — (SEG) Dada a CLI 0.2.0 diante de um servidor que responde v1 (o `wardd`
  0.1.4 de teste no socket de teste e um servidor falso em Python), quando o usuário
  roda `/usr/local/bin/ward unlock` (no teste, o binário de teste num pty), então a CLI
  para com mensagem pt-BR de versão incompatível **antes** do prompt da senha e não
  envia byte de senha (captura do servidor falso vazia). O mesmo vale para o app
  (`ward-app-core`). Cobre S5.
- [ ] V4.17 — (SEG) Dada a CLI 0.1.4 diante do daemon 0.2.0, quando roda `lista`,
  `run` (a operação v1 `executar`) e `destravar` (este último com a senha de teste num
  pty), então recebe erro de versão ou "ward: resposta inválida do daemon" (a CLI 0.1.4
  não lê o quadro v2 `unknown_version` da DT8; ajuste da Fase 66, achado da Fase 64),
  nenhum comando executa, o daemon continua travado e registra `unknown_version` (a
  senha mandada pela CLI velha não destrava o cofre). Cobre RM3.
- [ ] V4.18 — Dado `ward list --json`, quando o agente o lê, então as chaves e os valores
  são os de DT4/DT8 em inglês (`name`, `type`, `group`, `environment`, `project`,
  `address`), sem chave de nível (`level` nem `nivel`: D50 tira o nível da lista do
  agente, como na 0.1.4; ajuste da Fase 60, achado da Fase 52); nenhuma chave nem valor
  pt-BR; a entrada `blocked` não aparece; nenhum padrão de `.dev/variations.txt`; e
  `ward list` (tabela) mostra os rótulos pt-BR de tipo ("banco", "servidor").

Tipos e entrega (PRD DT11):

- [ ] V4.19 — Dado o cofre de teste, quando `ward run <entrada> -- env -0` roda para cada
  linha de "Entrega esperada", então o filho recebe exatamente as variáveis da tabela
  (conferidas por hash, sem imprimir segredo), `WARD_ENTRY`/`WARD_GROUP`/
  `WARD_ENVIRONMENT` em todas e nenhuma das formas antigas.
- [ ] V4.20 — (SEG) Dado `ward run gama-generico -- sh -c 'stat -c %a "$WARD_FILE_APP_ENV"'`
  e o mesmo com `acme-ssh-chave-prod` e `WARD_FILE_KEY`, quando roda (e quando o
  comando falha ou leva SIGKILL), então o arquivo é 0600 em `/run/user/<uid>/ward/tmp/`
  e não existe mais depois do fim. Cobre S2.
- [ ] V4.21 — (SEG) Dado qualquer `ward run`/`ward ssh` da bateria, quando um leitor em
  laço lê `/proc/*/cmdline` durante a execução, então nenhum argv (do daemon, do
  `sshpass`, do `ssh`, da CLI, do filho) tem padrão de `.dev/variations.txt`. Cobre S3.

Comandos do Tauri e capabilities:

- [ ] V4.22 — (SEG) Dado o app de release, quando o validador lê
  `app/src-tauri/capabilities/*.json` e as permissões, então só os comandos que o
  front-end chama estão liberados (lista igual à da 0.1.4 pelo glossário, nenhum a
  mais), a CSP é a da 0.1.4, `dev/tests/app-dist.py` não acha URL remota, inline nem
  código de teste em produção, e o `strings` do release (V5.6 da missão 1) não tem os
  nomes de teste novos nem antigos. Cobre S11.
- [ ] V4.23 — (SEG) Dado o app, quando a webview chama um comando Tauri com o nome antigo
  (pt-BR) ou um que não está na capability (pela ponte de teste e por
  `window.__TAURI__` no app de teste), então a chamada é recusada sem efeito. Cobre S11.
- [ ] V4.24 — (UI)(SEG) Dado o "copiar" de um segredo no app, quando 30 s passam, então a
  área de transferência está vazia (e o `make test-clipboard` passa). Cobre S12.

## Milestone 5 — Nomes externos e documentação

- [ ] V5.1 — Dada a CLI 0.2.0, quando o usuário roda `ward list`, `ward list --json`,
  `ward lock`, `ward unlock`, `ward help`, `ward --help` e `ward --version`, então
  funcionam como os equivalentes da 0.1.4, com mensagens pt-BR; `ward --version` diz
  0.2.0; e `ward lista`, `ward travar`, `ward destravar` e `ward ajuda` saem 2 com
  "comando desconhecido", sem falar com o daemon.
- [ ] V5.2 — Dado `ward --help`, quando o validador compara com
  `dev/fixtures/legacy/help-0.1.4.txt`, então a diferença é só a dos nomes de
  subcomando (e a versão), com o texto pt-BR igual.
- [ ] V5.3 — (SEG) Dado `skill/settings-deny.json`, quando o `make test-skill` aplica
  cada forma da lista de ataques da missão 1 com `ward unlock` (inteiro e por
  subcomando, com caminho completo, `env`, `sh -c`, curingas, `python3 -c`) e as mesmas
  formas com `ward destravar`, então todas são negadas; e nenhum exemplo da skill
  (`ward list`, `ward run`, `ward ssh`, `ward login`, `ward playwright`, `ward lock`) é
  negado. A skill não cita subcomando antigo nem `--token-fd`. Cobre a ameaça 1
  ("Segurança" › "Modelo de ameaça").
- [ ] V5.4 — (SEG) Dado o `install/install.sh`, quando recebe como artefato um binário
  com a marca de `test-mode` nova, um com a marca `modo-teste` antiga (o `wardd` de
  teste 0.1.4 de `.dev/baseline/`), um não-ELF ou um link, então recusa cada um antes de
  mudar qualquer coisa. Cobre S8 (DT12).
- [ ] V5.5 — (SEG) Dado o `make test-no-test-mode`, quando roda, então nenhum membro liga
  `test-mode` por dependência normal e o `strings` dos binários de release não tem
  `WARD_DEV_APP`, `WARD_DEV_EXTENSION` nem `WARD_DEV_EXTENSAO`, nem as outras variáveis
  de teste, novas ou antigas. Cobre S8.
- [ ] V5.6 — Dado `make install DESTDIR=.dev/<pasta>`, quando o validador compara com
  `install/tree.txt`, então a árvore tem `usr/local/share/ward/extension/` e
  `usr/local/lib/ward/created-groups` (com o motivo do arquivo), nenhum caminho
  `extensao`/`grupos-criados`, e a política do Chrome aponta para
  `file:///usr/local/share/ward/extension/update.xml`.
- [ ] V5.7 — (SEG) Dado o contêiner do `make test-install` com a 0.1.4 instalada (pelo
  `.deb` de `.dev/baseline/` e, em outra rodada, pelo `make install` da 0.1.4) e o
  `wardd@1000` rodando com o cofre de teste destravado, quando a 0.2.0 é instalada por
  cima, então o `wardd` antigo é parado antes da troca (cofre travado), os caminhos
  antigos somem, `created-groups` tem os grupos que a 0.1.4 criou, os grupos não
  mudam de gid, o setgid 2755 está nos binários novos, o socket religa e a primeira
  conexão sobe o `wardd` 0.2.0. O cofre não é tocado até destravar e migra no destrave.
  Cobre S5 e S7.
- [ ] V5.8 — (SEG) Dado o uninstall da 0.2.0 sobre uma instalação feita pela 0.1.4 (só
  `grupos-criados`) e sobre uma feita pela 0.2.0, quando roda, então apaga só os grupos
  da marca (nunca um grupo `ward-app` que já existia antes), apaga as duas marcas e não
  toca em `~/.local/share/ward/`.
- [ ] V5.9 — Dado o `.deb` 0.2.0 (`make deb` e `make test-deb`), quando o validador lista
  o conteúdo e o instala por cima do `.deb` 0.1.4 no contêiner, então `dpkg -c` não tem
  caminho antigo, `dpkg -L ward` depois da atualização não tem caminho antigo, o
  statoverride 2755 vale para os binários novos, e `dpkg -r`/`-P` deixam o cofre
  intacto.
- [ ] V5.10 — (SEG) Dada a extensão 0.1.4 (protocolo v1) ainda instalada no perfil de
  teste, quando ela fala com o `ward-nm` 0.2.0 (`make test-nm` com a extensão da linha
  de base), então recebe `unknown_version` e nenhum usuário, senha ou TOTP; a extensão
  0.2.0 (versão do manifesto 0.2.0, mesmo id) preenche normalmente. Cobre S1.
- [ ] V5.11 — Dados `docs/`, o README, o `CLAUDE.md`, `skill/` e `.claude/`, quando o
  validador procura os nomes antigos de DT8–DT15 (subcomandos, alvos, variáveis,
  caminhos, crates, pastas), então só aparecem em `docs/mission/arquivo/`, na nota de
  atualização de `docs/instalacao.md`, no histórico do escopo e nestes lugares que os
  citam de propósito (ajuste da Fase 60, achado da Fase 59): `docs/glossario.md` (as
  tabelas antigo → novo); os documentos da missão 2 em `docs/mission/` (PRD, plano,
  este contrato, handoffs) e a tabela "antes → depois" do fim de
  `docs/mission/design-nodes.md`; em `skill/README.md` e `skill/settings-deny.json`, as
  regras de deny que negam as formas da 0.1.x (`ward destravar`, `WARD_ARQUIVO_*`; DT10,
  Fase 58); no `CLAUDE.md`, as marcas de teste da 0.1.x que o `test-no-test-mode` e o
  `install.sh` recusam (`WARD_DEV_EXTENSAO`, `ward-ponte-teste`…), a marca de grupos e a
  pasta da extensão da 0.1.x que a instalação apaga, e os nomes que o glossário mantém
  (imagens Docker `ward-teste/*:1`, a base `app_teste` do app de teste); e as palavras
  pt-BR comuns usadas como texto (como "ferramentas", "origens", "livre", "revelar"),
  que não são o nome antigo. O `CLAUDE.md` registra a
  convenção de IDs (DT2); o `docs/instalacao.md` tem a atualização 0.1.x → 0.2.0 e a
  volta pelo backup v4 (DT7); e o `make test-migration-guide` passa.

## Milestone 6 — Validação final

- [ ] V6.1 — Dado o repositório final, quando `make check` roda, então a etapa
  `english-check` (com o autoteste) roda dentro dele, sem achado, e `frozen.txt` não
  existe; `make check` passa inteiro (fmt, clippy `-D warnings`, testes, `cargo audit`,
  `cargo deny`, gitleaks, origens).
- [ ] V6.2 — Dado o repositório final, quando o validador procura cada nome antigo das
  tabelas DT8–DT15 do PRD (como palavra inteira, em todos os arquivos do git; as
  palavras pt-BR comuns que também eram nome de fio, como `estado`, `lista`, `erro`,
  `banco` e `livre`, só como literal exato entre aspas, e os subcomandos como
  `ward <nome>`, para não contar a prosa pt-BR; a procura da Fase 60), então só acha
  nestes lugares (ajuste da Fase 66c, ressalva do scrutiny do M6; a tabela com os
  arquivos está no handoff da Fase 60):
  - `docs/mission/arquivo/` e os documentos da missão (`docs/mission/*`, handoffs,
    evidências, `design-nodes.md`), as notas de atualização e as tabelas antigo → novo
    de `docs/` (`instalacao.md`, `glossario.md`, `escopo.md`) e o `CLAUDE.md`;
  - `dev/fixtures/legacy/`;
  - a tabela de conversão v4/v1 do núcleo (`ward-core/src/body/legacy_v4.rs`,
    `wire_v1.rs` e a tabela v1 → v2 dos testes de protocolo);
  - os testes de recusa em que o nome antigo é o ataque: subcomando, operação, campo,
    comando Tauri, nível ou variável da 0.1.x que tem de ser recusado ou que não pode
    aparecer na saída nem no ambiente;
  - as regras de deny e o `skill/README.md` com as formas da 0.1.x (DT10);
  - a recusa da marca antiga de `test-mode` (instalador, `no-test-mode.sh`, testes de
    instalação e de release, `dev/bridge.sh`) e os caminhos da 0.1.x que a atualização
    apaga, com a marca antiga que o uninstall lê (`install/upgrade.sh`, `groups.sh`,
    `deb.sh`; DT15);
  - os rótulos pt-BR exibidos ("banco", "servidor", "livre", "bloqueado"), que não são o
    nome gravado, e o nome do arquivo de estado no disco (`estado`, DT6);
  - o protocolo de linhas do teste da área de transferência (glossário §9), chaves
    internas e nomes de pasta de teste que não são fio do Ward, e a prosa pt-BR dos logs
    e dos textos (prefixos `executar:`, "ferramentas", "origens").
- [ ] V6.3 — Dados os literais do produto (`english_check.py --literals`), quando o
  validador compara com `dev/fixtures/legacy/strings-0.1.4.txt`, então a diferença é
  só de nomes trocados por esta missão (subcomandos, operações, variáveis, caminhos),
  cada um listado no handoff da Fase 60. Nenhum texto exibido mudou, fora as mensagens
  novas de propósito, que são comportamento novo desta missão e não tradução (ressalva
  da Fase 66c; lista no handoff da Fase 60): as da migração do cofre para o corpo v5, as
  de versão do protocolo (DT8), a de nome reservado de campo extra ou anexo (Fase 54b)
  e o estado `ward-state 2` (DT6).
- [ ] V6.4 — (SEG) Dada a bateria de ataques da missão 1 com os nomes novos (`make
  test-attacks-foundation`, `test-attacks-services`, `test-home-decoys`,
  `test-real-home`, `test-socket-attacks`, `test-cli`, `test-ssh-home`,
  `test-docker-uid`, `test-docker-platform`, `test-units`, `test-install`,
  `test-deb`, `test-chrome-policy`, `test-nm`, `test-extension`, `test-login`,
  `test-playwright`, `test-webkit-real`), quando roda, então cada suíte passa (o
  `test-webkit-real` sai como na missão 1: 0 ou 2, com os canais de D64 já registrados).
  Cobre S1–S13 pela reexecução dos ataques da missão 1.
- [ ] V6.5 — (SEG) Dado o cofre de teste e o serviço `phish` (8082), quando
  `ward login delta-redir` e `ward playwright delta-redir` rodam e a extensão é aberta
  no 8082, então todos recusam e o `posts.log` do phishing não muda (`ward login` e a
  extensão dizem "domínio não confere"; `ward playwright` diz "a origem da página não
  bate com a da entrada", o mesmo texto da 0.1.4; ajuste da Fase 66, achado da Fase
  65); numa origem `http` (8081) o preenchimento passa pela janela de aprovação
  mesmo no nível `free`. Cobre S1 e a ameaça 4.
- [ ] V6.6 — Dados os serviços de teste, quando `ward run acme-pg-dev -- psql -c 'select
  1'`, `ward run acme-pg-tunel -- dev/tests/pg_select.py`, `ward ssh
  acme-ssh-senha-stage -- true`, `ward ssh acme-ssh-chave-prod -- true`, `ward ssh
  acme-ssh-interno -- true` (salto), `ward login acme-wp-prod` (chega ao painel do
  WordPress 8081) e `ward playwright acme-wp-prod` (o Playwright abre `/wp-admin/` com o
  storage state) rodam, então todos funcionam e a saída devolvida não tem padrão de
  `.dev/variations.txt`.
- [ ] V6.7 — (SEG) Dados os níveis, quando o agente roda `ward list` e `ward run
  gama-bloqueada -- true`, então `gama-bloqueada` não aparece e o run dá "entrada
  inexistente"; `ward run acme-pg-aprovacao -- true` abre a janela de aprovação (com a
  ponte de teste) e `ward run acme-pg-dev -- true` não abre. Cobre os níveis ("Claude
  (agente)" › "Nível de acesso por entrada").
- [ ] V6.8 — (UI)(SEG) Dada a janela de aprovação de `acme-pg-prod`, quando o QA confere,
  então ela mostra entrada, ambiente `prod` em destaque, o comando exato e a cadeia de
  processos; tem os 4 botões **Negar**, **Permitir uma vez**, **Permitir por 1 hora** e
  **Permitir sempre** (iguais em `dev`, `stage` e `prod`); os três de permitir pedem o
  PIN; "Permitir por 1 hora" deixa novos pedidos passarem sem janela até 1 h e depois
  volta a abrir; "Permitir sempre" muda o nível para `free`; fechar nega; sem resposta
  em 2 min o pedido expira e a CLI sai com "expirou". Com a entrada padrão em pipe, os
  botões só liberam depois da confirmação a mais, e o daemon recusa sem ela. Cobre S10.
- [ ] V6.9 — (UI)(SEG) Dada a tela de nível de `acme-pg-dev`, quando o QA muda o nível,
  então o PIN é pedido; 5 PINs errados seguidos (na janela de aprovação ou no nível)
  travam o cofre. Cobre S9.
- [ ] V6.10 — (SEG) Dado o daemon de teste destravado, quando há bloqueio de tela
  (logind falso pelo `WARD_DEV_SYSTEM_DBUS`), suspensão, 4 h sem uso (relógio de teste)
  ou `ward lock`, então o cofre trava; travado, um `ward run` do agente abre o pedido de
  destrave e espera até 2 min (sai 125 aos 120 s ± 2 s) e é solto por `ward unlock`.
  Cobre S8 ("Destravar e travar").
- [ ] V6.11 — (SEG) Dada a árvore final, quando o gitleaks varre árvore, índice e
  histórico, então nada é acusado; o `.gitleaks.toml` só tem a allowlist dos valores
  falsos de D15, cada um casado inteiro (nenhum padrão genérico, nenhum caminho novo
  além dos já existentes com o nome novo), com uma exceção registrada: a forma base64
  de dois desses valores (a senha mestra de teste e a do WordPress), que a fixture do
  protocolo v2 (`dev/fixtures/protocol-v2.jsonl`, m2:DT8) carrega na chave `password`,
  também casada inteira (ajuste da Fase 66c); `cargo audit` e `cargo deny` (workspace e
  `app/src-tauri`) estão limpos; o `npm audit` não tem aviso de severidade moderate ou
  superior, e cada aviso low sem correção publicada fica registrado no handoff (hoje,
  os 2 de `deepmerge`, por `@sveltejs/vite-plugin-svelte`, dependência de build); e os
  lockfiles não têm dependência nova em relação à 0.1.4. Cobre S14 e S15.
- [ ] V6.12 — (UI) Dado o app nativo (Tauri) de release, quando o usuário segue
  `docs/mission/roteiro-usuario.md`, então a janela, a bandeja ("Travar agora") e a
  janela de aprovação são as da 0.1.4.
- [ ] V6.13 — Dados os artefatos da Fase 66, quando o validador confere, então
  `target/release/{ward,wardd,ward-nm}` e o app dizem 0.2.0 e não têm `test-mode`;
  `target/extension/ward.crx` tem o id de sempre e o manifesto 0.2.0; o `.deb`
  `ward_0.2.0_<arq>.deb` passa no `make test-deb`; e o roteiro do usuário cobre instalar
  por cima da 0.1.4, o primeiro destrave (migração), atualizar a skill e o deny em
  `~/.claude` e conferir a janela nativa.
