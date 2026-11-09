# Contrato de validação

Escrito a partir de `docs/mission/prd.md`, antes de qualquer código. Os validadores
conferem contra este arquivo, não contra o que foi implementado. Marcas: **(UI)** o QA
confere pela interface (Chrome em modo de teste, D7); **(SEG)** o security-auditor
executa o ataque descrito e ele tem de falhar. "Dado X, quando Y, então Z".

Regras gerais:
- Em todo milestone a partir do M1, `make check` passa limpo (S14, S15).
- Ataques rodam só contra o cofre de teste e o daemon de teste. Onde o texto diz
  `<rt>`, leia `/run/user/<uid>/ward-teste/` no daemon de teste (D7) e
  `/run/user/<uid>/ward/` no instalado (M9), onde ficam os temporários; o socket do
  instalado fica em `/run/ward/<uid>/wardd.sock` (D46).
- "Contém segredo" = contém qualquer valor secreto da tabela abaixo **ou** qualquer
  variação de D11 (URL-encoded, base64, Basic auth, `DATABASE_URL`), mais as linhas e
  valores de D36, as formas escapadas de D37 e a tabela única de D54. O validador gera a lista de variações
  com `make dev-variacoes` (Fase 19, atualizado pela Fase 19d) e faz `grep -F -f`.
- Comparar conteúdo de variável sem expor o valor: `ward run <e> -- sh -c 'printf %s
  "$VAR" | sha256sum'` contra o sha256 calculado localmente do valor esperado.

## Cofre de teste (D15)

Senha mestra `falso-mestra-ward-01`, PIN `135790`. Todos os valores são falsos.

| Entrada | Tipo | Grupo | Ambiente | Nível | Dados |
|---|---|---|---|---|---|
| `acme-wp-dev` | login | acme | dev | com aprovação | URL `http://localhost:8081/wp-login.php`, usuário `wp_admin`, senha `falso-WP-senha-01`, TOTP semente `JBSWY3DPEHPK3PXP` |
| `acme-wp-prod` | login | acme | prod | livre | URL `http://localhost:8081/wp-login.php`, usuário `wp_editor`, senha `falso-WP-senha-02`, TOTP semente `JBSWY3DPEHPK3PXP` |
| `acme-pg-dev` | banco (postgres) | acme | dev | livre | `127.0.0.1:55432`, base `app_teste`, usuário `app`, senha `falso-PG:s3nha/01@#` |
| `acme-pg-aprovacao` | banco (postgres) | acme | stage | com aprovação | igual a `acme-pg-dev` |
| `acme-pg-prod` | banco (postgres) | acme | prod | com aprovação | igual a `acme-pg-dev` |
| `acme-pg-tunel` | banco (postgres) | acme | stage | livre | host `postgres`, porta 5432, túnel por `acme-ssh-senha-stage`, mesmo usuário/senha |
| `acme-ssh-senha-stage` | servidor | acme | stage | livre | `127.0.0.1:2222`, usuário `deploy`, senha `falso-SSH-senha-01` |
| `acme-ssh-chave-prod` | servidor | acme | prod | livre | `127.0.0.1:2223`, usuário `deploy`, chave ed25519 gerada em `.dev/`, passphrase `falso-passphrase-01` (servidor só aceita chave) |
| `acme-ssh-salto` | servidor | acme | prod | com aprovação | `127.0.0.1:2224`, usuário `salto`, senha `falso-SALTO-01` |
| `acme-ssh-salto-livre` | servidor | acme | prod | livre | igual a `acme-ssh-salto` |
| `acme-ssh-interno` | servidor | acme | prod | livre | host `ssh-interno`, porta 22 (só na rede do compose), usuário `deploy`, chave de `acme-ssh-chave-prod`, salto `acme-ssh-salto-livre` |
| `acme-ssh-interno-aprov` | servidor | acme | prod | livre | igual a `acme-ssh-interno`, salto `acme-ssh-salto` |
| `beta-github-api` | api | beta | dev | livre | URL `https://api.github.com`, token `falso_token_github_01`, variável `GITHUB_TOKEN` |
| `beta-aws-dev` | aws | beta | dev | livre | access key `FALSOACCESSKEY01`, secret `falso-aws-secret-01`, região `sa-east-1`, session token `falso-aws-session-01`, role `arn:aws:iam::000000000000:role/teste` |
| `beta-smtp-stage` | smtp | beta | stage | livre | `smtp.teste.invalid:587`, usuário `mailer`, senha `falso-SMTP-01`, segurança `starttls` |
| `beta-sftp-dev` | ftp/sftp | beta | dev | livre | `sftp`, `127.0.0.1:2222`, usuário `sftpuser`, senha `falso-SFTP-01` |
| `gama-nota` | nota segura | gama | dev | livre | texto `falso-nota-01` |
| `gama-generico` | genérico | gama | stage | livre | extras `REGIAO=r-01` (não segredo), `CHAVE_LICENCA=falso-licenca-01` (segredo); anexo `app.env` com `FALSO_ENV=falso-anexo-01` |
| `gama-bloqueada` | login | gama | prod | bloqueado | URL `https://bloqueada.invalid`, usuário `x`, senha `falso-BLOQ-01` |
| `delta-redir` | login | delta | dev | livre | URL `http://localhost:8084/login` (redireciona para o phishing), usuário `u`, senha `falso-REDIR-01` |
| `acme-totp-dev` | login | acme | dev | livre | URL `http://localhost:8083/login`, usuário `wp_admin`, senha `falso-WP-senha-01`, TOTP semente `JBSWY3DPEHPK3PXP` (D28; mesma conta que o serviço `totp` já aceita) |
| `delta-api-simbolos` | api | delta | dev | livre | (a partir da Fase 22a, D59) URL `https://api.teste.invalid`, token `falso-Ab3$k&<9>Zq!'/ç-01`, variável `SIMBOLOS_TOKEN`; extra secreto `SEGREDO_ASCII=falso-Ab3$k&<9>Zq!'`; nenhum serviço do compose a usa |

Serviços do `docker compose` de teste (Fase 12): `postgres` (55432), `ssh-senha`
(2222; `deploy` por senha, `sftpuser` só sftp), `ssh-chave` (2223; só chave),
`ssh-salto` (2224), `ssh-interno` (sem porta no host), `wordpress` + `mariadb` (8081),
`phish` (8082; clone da tela de login do WordPress que grava todo POST em
`/registro/posts.log`), `totp` (8083; formulário de login com campo de código e
cadastro novo), `redir` (8084; redireciona para 8082).

## Entrega esperada (D10), usada em V2 e V4

| Entrada | Variáveis e conteúdo esperado |
|---|---|
| `acme-wp-prod` | `WARD_URL=http://localhost:8081/wp-login.php`, `WARD_USER=wp_editor`, `WARD_PASS=falso-WP-senha-02`, `WARD_TOTP=<6 dígitos iguais a oathtool --totp -b JBSWY3DPEHPK3PXP no mesmo período>` |
| `acme-pg-dev` | `PGHOST=127.0.0.1`, `PGPORT=55432`, `PGDATABASE=app_teste`, `PGUSER=app`, `PGPASSWORD=falso-PG:s3nha/01@#`, `DATABASE_URL=postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste` |
| `acme-pg-tunel` | `PGHOST=127.0.0.1`, `PGPORT=<porta local do túnel, ≠5432>`, demais como `acme-pg-dev`, `DATABASE_URL` com o mesmo host/porta locais |
| `acme-ssh-senha-stage` | `WARD_HOST=127.0.0.1`, `WARD_PORT=2222`, `WARD_USER=deploy`, `WARD_PASS=falso-SSH-senha-01` |
| `acme-ssh-chave-prod` | `WARD_HOST=127.0.0.1`, `WARD_PORT=2223`, `WARD_USER=deploy`, `WARD_ARQUIVO_CHAVE=<rt>tmp/…` (arquivo 0600 com a chave privada), `WARD_PASSPHRASE=falso-passphrase-01` |
| `beta-github-api` | `WARD_URL=https://api.github.com`, `WARD_TOKEN=falso_token_github_01`, `GITHUB_TOKEN=falso_token_github_01` |
| `beta-aws-dev` | `AWS_ACCESS_KEY_ID=FALSOACCESSKEY01`, `AWS_SECRET_ACCESS_KEY=falso-aws-secret-01`, `AWS_SESSION_TOKEN=falso-aws-session-01`, `AWS_REGION=sa-east-1`, `AWS_DEFAULT_REGION=sa-east-1`, `AWS_ROLE_ARN=arn:aws:iam::000000000000:role/teste` |
| `beta-smtp-stage` | `SMTP_HOST=smtp.teste.invalid`, `SMTP_PORT=587`, `SMTP_USER=mailer`, `SMTP_PASS=falso-SMTP-01`, `SMTP_SECURITY=starttls` |
| `beta-sftp-dev` | `WARD_PROTOCOLO=sftp`, `WARD_HOST=127.0.0.1`, `WARD_PORT=2222`, `WARD_USER=sftpuser`, `WARD_PASS=falso-SFTP-01` |
| `gama-nota` | nenhuma; `ward run` sai com erro "tipo sem entrega" e não executa o comando |
| `gama-generico` | `WARD_REGIAO=r-01`, `WARD_CHAVE_LICENCA=falso-licenca-01`, `WARD_ARQUIVO_APP_ENV=<rt>tmp/…` (conteúdo `FALSO_ENV=falso-anexo-01`) |
| todas | `WARD_ENTRADA=<nome>`, `WARD_GRUPO=<grupo>`, `WARD_AMBIENTE=<ambiente>` |

## Milestone 0 — Design no FigCraft

Arquivo `/home/jeffersonoliveira/documentos/Ward.figcraft` (D22), conferido pelo MCP
do FigCraft (`open_file`, `inspect_document`, `get_node`, `screenshot`) a partir da
versão salva em disco. Frames na página Layout, componentes, estilos e variables na
página Design System; ids em `docs/mission/design-nodes.md`. Nomes exatos; frames
adicionais são permitidos. O Figma `bzu9PxxFur2tU3vlNCHvsY` é só histórico: nenhuma
asserção V0.* é conferida nele, exceto a `1:3` em V0.21.

- [x] V0.1 — Dado o arquivo, quando o QA lista as coleções de variables, então existem
  `Cores` (um único modo, `Escuro`; nenhum modo `Claro`), `Tipografia`, `Espaçamento`,
  `Raio` (com os valores arredondados de D23) e `Largura`, e as cores semânticas cobrem fundo, superfície, texto, borda, primária, perigo, aviso, sucesso e
  os três ambientes (`dev`, `stage`, `prod`).
- [x] V0.2 — Dado qualquer frame do Layout, quando o QA inspeciona preenchimentos,
  bordas, textos, espaçamentos e raios, então todos estão ligados a variables (nenhuma
  cor ou medida solta).
- [x] V0.3 — Dada a página Design System, quando o QA lista os componentes, então
  existem com estes nomes e variantes: `Botão` (primário, secundário, perigo, fantasma;
  padrão, foco, desabilitado), `Campo` (padrão, foco, erro, desabilitado),
  `Campo secreto` (oculto, revelado, copiado; com ações revelar e copiar), `Select`,
  `Badge ambiente` (dev, stage, prod), `Badge tipo` (os 9 tipos), `Item de lista`,
  `Modal`, `Toast` (sucesso, erro, aviso), `Campo PIN`.
- [x] V0.4 — Dado o Layout, quando o QA percorre os frames, então todo botão, campo,
  select, badge, item de lista, modal e toast é **instância** de componente do Design
  System (nenhum desanexado).
- [x] V0.5 — Existem `Primeiro uso / Criar cofre`, `Primeiro uso / Senha mestra` e
  `Primeiro uso / PIN` (PIN de 4 a 6 dígitos, explicando que o PIN não destrava).
- [x] V0.6 — Existem `Destravar / Padrão`, `Destravar / Senha errada` e
  `Destravar / Pedido do agente` (mostra a entrada e o comando que esperam, com
  contagem de 2 min).
- [x] V0.7 — Existem `Credenciais / Vazia`, `Credenciais / Cheia` (agrupada por
  grupo, com badge de tipo e de ambiente em cada item) e `Credenciais / Filtrada`
  (filtro por tipo e ambiente e busca ativos), sem nenhum valor secreto visível.
- [x] V0.8 — Existem os 18 frames `Formulário / <tipo> / Novo` e
  `Formulário / <tipo> / Edição` para `login`, `banco`, `servidor`, `api`, `aws`,
  `smtp`, `ftp/sftp`, `nota segura`, `genérico`, com os campos de "Tipos prontos (v1)",
  `grupo` (com autocompletar) e `ambiente` obrigatórios, `projeto`, tags, campos
  extras com chave "segredo sim/não" e o nível de acesso.
- [x] V0.9 — Os frames de formulário mostram anexos (adicionar, listar, remover) em
  pelo menos `servidor` e `genérico`, e TOTP (código com contagem; semente como campo
  secreto) em `login`.
- [x] V0.10 — `Formulário / servidor / Edição` tem o botão `Conectar` (D3).
- [x] V0.11 — Existem `Nível de acesso / Escolher` (bloqueado, com aprovação, livre,
  com explicação curta de cada), `Nível de acesso / PIN` e `Nível de acesso / PIN
  errado`, iguais para os três ambientes (R16).
- [x] V0.12 — Existem `Gerador de senha / Avulso` e `Gerador de senha / No formulário`
  (tamanho, minúsculas, maiúsculas, dígitos, símbolos, gerar de novo, copiar).
- [x] V0.13 — Existem `Configurações / Geral` (PIN, senha mestra, tempo de trava,
  backup externo **desligado**) e `Configurações / Backup externo ligado` (pendrive
  escolhido).
- [x] V0.14 — Existem `Restaurar backup / Versões` (lista datada) e
  `Restaurar backup / Confirmar` (pede a senha mestra da versão).
- [x] V0.15 — Existe `Kit de emergência / Impressão` em tamanho A4, com onde está o
  cofre, onde estão os backups e o pendrive, e uma linha em branco para a senha mestra
  escrita à mão (nenhum campo preenchido).
- [x] V0.16 — Existem `Aprovação / Normal`, `Aprovação / prod`, `Aprovação / PIN
  errado`, `Aprovação / PIN bloqueado` e `Aprovação / Expirando`. Cada um mostra
  entrada, ambiente, comando exato e cadeia de processos; os quatro botões **Negar**,
  **Permitir uma vez**, **Permitir por 1 hora**, **Permitir sempre** aparecem em todos,
  inclusive `prod` (R16), com o ambiente em destaque em `prod` e o foco inicial em
  **Negar**.
- [x] V0.17 — Existem `Bandeja / Destravado` (abrir, travar agora, sair) e
  `Bandeja / Travado` (abrir, destravar).
- [x] V0.18 — Existem `Extensão / Sugestões do domínio`, `Extensão / Salvar login novo`
  (tipo, grupo, ambiente) e `Extensão / Cofre travado`.
- [x] V0.19 — Dado qualquer frame do Layout (todos os grupos: destravar e primeiro uso;
  credenciais e formulários; aprovação; gerador e configurações; bandeja e extensão),
  quando o QA inspeciona o modo de variables aplicado, então é `Escuro`; nenhum frame
  ou componente tem variante clara nem controle de alternância de tema, e
  `Configurações / *` não tem opção de tema (D1).
- [x] V0.20 — Todo texto dos frames está em português, e todo valor secreto de exemplo
  aparece como `••••` ou é obviamente falso.
- [x] V0.21 — Dadas as listas de filhos das Inspirações salvas antes de desenhar (a da
  `1:3` do Figma, em `docs/mission/evidence/fase-1-inspiracoes.json`, e a da página
  Inspirações do `Ward.figcraft`, se existir, em
  `docs/mission/evidence/figcraft-inspiracoes.json`, Fase 4a), quando o QA compara
  cada uma com a lista atual, então nada foi criado, movido, editado ou apagado em
  nenhuma delas.
- [x] V0.22 — `docs/mission/design-nodes.md` liga cada frame e componente acima ao seu
  id no FigCraft, e, reabrindo `Ward.figcraft` do disco (`open_file`), todo id existe
  (prova de que cada fase salvou com `save_file`).
- [x] V0.23 — O usuário aprovou o `Ward.figcraft` no FigCraft, incluindo o ícone
  (registro em `plan.md`).

## Milestone 1 — Fundação

- [x] V1.1 — Dado o repositório limpo, quando roda `make check`, então executa
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
  `cargo audit`, `cargo deny check` e `gitleaks`, nessa ordem, e sai com 0.
- [x] V1.2 — (SEG) Dado um arquivo `vazamento.txt` com uma chave privada OpenSSH gerada
  na hora e uma linha `aws_secret_access_key = <40 caracteres aleatórios>`, quando roda
  `make check`, então sai ≠ 0 apontando o arquivo; removido o arquivo, volta a 0 —
  cobre S15.
- [x] V1.3 — (SEG) Dado um crate de teste que adiciona uma dependência com advisory
  conhecido (ex.: `time = "=0.1.43"`, RUSTSEC-2020-0071) ou vinda de git/registro não
  permitido, quando roda `make check`, então falha em `cargo audit` ou `cargo deny`;
  `Cargo.lock` está versionado — cobre S14.
- [x] V1.4 — Dado `docker compose -f dev/compose.yml up -d`, quando o validador testa,
  então: `psql` em 55432 entra com `app`; `ssh-senha` (2222) aceita `deploy` por senha;
  `ssh-chave` (2223) recusa senha; `ssh-salto` (2224) aceita `salto`; `ssh-interno` não
  responde do host e responde a partir do `ssh-salto`; o WordPress em 8081 aceita
  `wp_admin`; `phish` (8082), `totp` (8083) e `redir` (8084) respondem.
- [x] V1.5 — (SEG) Dado `.gitignore`, quando `git check-ignore .dev/qualquer`, então é
  ignorado; nenhum comando do Makefile ou script de teste cita
  `~/.local/share/ward`, `~/.ssh`, `~/.aws` ou o KeePassXC — cobre S15.
- [x] V1.6 — `CLAUDE.md` › "Comandos" tem: build, `make check`, `make dev-cofre`, subir
  e parar o daemon de teste, `docker compose` de teste, senha mestra e PIN de teste.

## Milestone 2 — Núcleo: formato, cripto, tipos e máscara

- [x] V2.1 — (SEG) Dado o cofre de teste, quando abre com `falso-mestra-ward-02`, então
  falha com o mesmo erro genérico de arquivo adulterado e nada é decifrado — cobre S6.
- [x] V2.2 — (SEG) Dado um cofre pequeno, quando o teste troca um bit de **cada** byte
  do arquivo (um arquivo por posição), então toda abertura falha sem pânico — cobre S6.
- [x] V2.3 — (SEG) Dado o mesmo cofre, quando o teste o trunca em cada tamanho de 0 a
  n−1 e quando acrescenta 1 byte, então toda abertura falha — cobre S6.
- [x] V2.4 — (SEG) Dado o cabeçalho, quando o teste o lê, então registra Argon2id com
  memória ≥ 262144 KiB, passes ≥ 3 e paralelismo; quando o atacante reescreve o
  cabeçalho com memória de 8 MiB (rebaixamento), então a abertura falha (cabeçalho é
  dado autenticado); quando um arquivo bem formado declara parâmetros abaixo do
  mínimo, então é recusado — cobre S6.
- [x] V2.5 — (SEG) Dados os vetores do RFC 9106 (Argon2id) e do draft
  `irtf-cfrg-xchacha` (XChaCha20-Poly1305), quando passam pelas mesmas funções do
  formato, então os resultados batem — cobre S6.
- [x] V2.6 — (SEG) Dados dois cofres criados com a mesma senha e o mesmo conteúdo,
  quando comparados, então sal, nonce, chave embrulhada e texto cifrado são todos
  diferentes (chave de dados aleatória) — cobre S6.
- [x] V2.7 — (SEG) Dado `cargo tree -p ward-core`, quando o auditor confere, então a
  cripto vem só de `argon2`, `chacha20poly1305`, `rand_core`/`getrandom`, `zeroize` e das
  transitivas do RustCrypto (a comparação em tempo constante vem de `ctutils`/`cmov`,
  que substituíram o `subtle` nas versões atuais; qualquer um dos dois serve), e o código
  do Ward não implementa primitiva, modo, KDF nem comparação de tag à mão (`grep` por
  `==` ou `ct_eq` sobre tag e chave no `ward-core` não acha nada) — cobre S6, S14.
- [x] V2.8 — Dado o cofre, quando troca a senha mestra, então a senha antiga falha, a
  nova abre e o conteúdo é o mesmo.
- [x] V2.9 — (SEG) Dado um processo filho que grava o cofre, quando é morto com SIGKILL
  em 200 pontos aleatórios da gravação (e por injeção de falha entre escrita, fsync e
  rename), então o arquivo abre sempre, com a versão anterior inteira ou a nova
  inteira — cobre S7.
- [x] V2.10 — (SEG) Dado `strace -f -e trace=openat,write,fsync,rename,renameat2` de uma
  gravação, quando o auditor lê, então a ordem é escrever o temporário no mesmo
  diretório → fsync do temporário → rename → fsync do diretório — cobre S7.
- [x] V2.11 — (SEG) Dado `strace -f -e write -s 100000` de uma gravação do cofre de
  teste, quando procura os segredos, então nenhum buffer escrito em arquivo contém
  segredo, e `grep -a` no cofre e nos temporários também não acha — cobre S2.
- [x] V2.12 — Dado cada tipo, quando salva sem campo obrigatório, sem `grupo`, sem
  `ambiente`, com ambiente fora de `dev|stage|prod`, `servidor` com senha **e** chave ou
  sem nenhuma, ou `banco` com motor fora da lista, então a validação recusa com
  mensagem em português.
- [x] V2.13 — Dado cada tipo, quando o teste lista os campos secretos, então são:
  `login` senha e semente TOTP; `banco` senha; `servidor` senha, chave e passphrase;
  `api` token; `aws` secret e session token; `smtp` senha; `ftp/sftp` senha, chave e
  passphrase; `nota segura` texto; extras marcados como segredo; todos os anexos.
- [x] V2.14 — Dadas as entradas da tabela "Entrega esperada", quando o núcleo monta o
  ambiente de cada uma (sem daemon), então nomes e valores batem exatamente, e
  `gama-nota` devolve "tipo sem entrega".
- [x] V2.15 — Dado `banco` com motores `mysql`, `redis`, `mongo` e `sqlserver` (host
  `db.teste.invalid`, porta padrão, usuário `u`, senha `falso:DB/01`, base `b`), quando
  o núcleo monta o ambiente, então saem as variáveis de D10 de cada motor e as URLs
  trazem usuário e senha com percent-encoding.
- [x] V2.16 — Dado um extra `api-key` e um anexo `chave acesso.pem`, quando o núcleo
  monta o ambiente, então saem `WARD_API_KEY` e `WARD_ARQUIVO_CHAVE_ACESSO_PEM`; dois
  extras que normalizam para o mesmo nome são recusados ao salvar.
- [x] V2.17 — (SEG) Dada a máscara de `acme-pg-dev`, quando a saída é
  `x falso-PG:s3nha/01@# y`, então sai `x •••• y` — cobre S4.
- [x] V2.18 — (SEG) Dado cada ponto de corte k do segredo, quando a saída chega em dois
  pedaços `[..k]` e `[k..]`, em três pedaços e byte a byte, então sai `••••` e nenhum
  pedaço do segredo — cobre S4.
- [x] V2.19 — (SEG) Dada a saída com o segredo em percent-encoding maiúsculo
  (`falso-PG%3As3nha%2F01%40%23`), minúsculo (`%3a…%2f…`) e em forma de formulário,
  então todos viram `••••` — cobre S4.
- [x] V2.20 — (SEG) Dada a saída com o segredo em base64 padrão e URL-safe, com e sem
  `=`, e com `base64("app:falso-PG:s3nha/01@#")` (Basic auth), então todos viram `••••`
  — cobre S4.
- [x] V2.21 — (SEG) Dada a saída com o `DATABASE_URL` inteiro de `acme-pg-dev`, então
  nem o URL nem pedaços da senha (crua ou codificada) sobram — cobre S4.
- [x] V2.22 — (SEG) Dados dois segredos em que um contém o outro, quando os dois
  aparecem colados, então nada de nenhum dos dois sobra — cobre S4.
- [x] V2.23 — (SEG) Dado o anexo `app.env` de `gama-generico`, quando a saída traz
  o conteúdo inteiro ou só a linha `FALSO_ENV=falso-anexo-01`, então vira `••••` (D11)
  — cobre S4.
- [x] V2.24 — Dada uma saída sem segredo (texto, UTF-8 inválido e binário de 1 MiB),
  quando passa pela máscara, então sai byte a byte igual, inclusive o fim que ficou no
  buffer.
- [ ] V2.25 — Dados os vetores do RFC 6238 (SHA1, T=59 → `94287082` em 8 dígitos) e a
  semente `JBSWY3DPEHPK3PXP`, quando o núcleo gera o código, então bate com o RFC e com
  `oathtool --totp -b` no mesmo período.
- [x] V2.26 — Dado o gerador com tamanho de 4 a 128 e cada combinação de conjuntos,
  quando gera 10 000 senhas, então nenhuma tem caractere fora dos conjuntos marcados,
  todas têm pelo menos um de cada conjunto marcado e a fonte é o RNG do sistema
  (`OsRng`/`getrandom`); sem símbolos não sai nenhum símbolo.
- [x] V2.27 — (SEG) Dada uma entrada de cada tipo, quando o teste formata com `{:?}` e
  `{}` e serializa os erros, então nenhuma saída contém segredo (os tipos de segredo
  imprimem `••••`) — cobre S13.
- [x] V2.28 — (SEG) Dados os tipos que guardam segredo e a chave de dados, quando o
  teste confere em tempo de compilação, então implementam `ZeroizeOnDrop` e não
  implementam `Clone` sem zerar; a lista inclui `formato::CofreAberto` e
  `formato::SenhaMestra`, além de `cripto::Chave` e dos tipos de `entrada` — cobre S8.
- [x] V2.29 — Dado `make dev-cofre`, quando roda duas vezes, então
  `.dev/cofre-teste.ward` tem as 21 entradas da tabela (com `acme-totp-dev`, D28; 22
  a partir da Fase 22a, com `delta-api-simbolos`, conferido no V3.29), abre
  com a senha de teste, e o comando recusa caminho de saída fora de `.dev/`.
- [x] V2.30 — (SEG) Dadas as 21 entradas do cofre de teste (22 a partir da Fase 22a,
  V3.29), mais uma de cada tipo com
  projeto, 16 tags, 64 extras e anexos que somam 256 KiB (o limite de D29), com bytes de
  0 a 255, dentro do orçamento da máscara (D29), quando
  o núcleo serializa o corpo (D26), sela, grava por `gravar_cofre`, reabre e
  desserializa, então cada entrada volta igual campo a campo (segredos comparados por
  sha256), com o **mesmo nível** (as três bloqueada/com aprovação/livre de D15), a
  seção de configuração volta igual (D30) e serializar de novo dá os mesmos bytes; um
  cofre sem entradas (só a configuração) também faz ida e volta; e um anexo vazio é
  recusado ao salvar e no corpo forjado (D29) — cobre S2, S9.
- [x] V2.31 — (SEG) Dado o corpo decifrado do cofre de teste (o AEAD barra quem não tem
  a chave; aqui o teste chama o decodificador direto, como um corpo autêntico gravado
  com defeito ou por versão hostil), quando o corpo é truncado em cada tamanho, ganha 1
  byte a mais, traz versão de corpo desconhecida, tipo ou nível fora da lista, o nome de
  uma entrada repetido, um salto para entrada inexistente ou para um servidor que tem
  salto, `-oProxyCommand=…` no host, U+202E no grupo ou nulo num segredo, então o
  decodificador recusa o corpo inteiro com o mesmo erro genérico, sem pânico
  (`catch_unwind`), sem devolver nenhuma entrada e sem o valor no `Display`/`Debug` do
  erro; e com um bit trocado em cada byte e 100 000 mutações aleatórias (semente fixa) o
  resultado é sempre corpo recusado ou entradas que passam por `Rascunho::validar` e
  `conferir_no_cofre`, nunca pânico nem entrada inválida — cobre S6, S13.
- [x] V2.32 — (SEG) Dado um corpo forjado que declara comprimento `u32::MAX` num campo,
  4 GiB num anexo, anexos que somam 256 KiB + 1 byte, 2^32−1 entradas, 65 extras,
  33 anexos, 17 tags ou corpo acima de 64 MiB, quando o decodificador lê, então recusa em menos de 100 ms sem alocar
  proporcional ao declarado (alocador de contagem do teste: nenhum pedido acima do
  tamanho real do buffer) e sem pânico; e o codificador recusa montar corpo acima do
  limite — cobre S8, S13.
- [x] V2.33 — (SEG) Dado um alocador global de teste que, ao liberar ou realocar um
  bloco, procura nele as marcas dos segredos falsos, quando o teste serializa,
  desserializa, sela e abre o cofre de teste e solta tudo, então nenhum bloco liberado
  contém segredo (os buffers zeram antes de liberar e nenhuma realocação deixa cópia);
  uma mutação de controle (um `Vec` comum no lugar do `Zeroizing`) é acusada — cobre S8.
- [x] V2.34 — (SEG) Dado um crate fora do `ward-core` (doctest `compile_fail` ou teste em
  `wardd`), quando chama `ward_core::gravacao::gravar(destino, bytes)` com um cabeçalho
  válido e corpo arbitrário, então não compila; a única gravação pública é a que parte
  de um `CofreAberto` (`gravar_cofre` e a troca de senha), e o auditor confere por
  `grep` que nenhum `pub fn` do `ward-core` grava bytes sem selar (D27) — cobre S2, S7.
- [x] V2.35 — Dado o `ward-core`, quando o validador procura os números de D25 e D29
  (64 KiB, 1 MiB, 256 KiB, 64, 32, 16, 10 000, 64 MiB, 4 MiB de orçamento) fora de
  `ward_core::limites`, então não acha definição repetida em `entrada`, `corpo` nem
  `mascara`; e um teste muda uma constante numa cópia de trabalho e vê validação,
  leitura do corpo e orçamento mudarem juntos (D32).
- [x] V2.36 — (SEG) Dada uma entrada `genérico` com um anexo de 256 KiB + 1 byte, outra
  com dois anexos de 200 KiB e uma `api` com 64 extras secretos de 64 KiB (acima do
  orçamento de 4 MiB), quando salva e quando o corpo forjado as traz, então as três são
  recusadas, a última com "entrada grande demais para a máscara do `ward run`"; e para
  a maior entrada aceita de cada tipo com entrega, mais o par banco + `servidor` de
  túnel (D19), o `Gerador` da máscara monta sem passar de 8 MiB, ou seja, nenhuma
  entrada aceita pelo cofre fica sem `ward run` — cobre S4.
- [x] V2.37 — (SEG) Dado `banco` com base `b?x=1`, `b#x`, `../b`, `b.c`, `b c`, `b%41`,
  64 caracteres, ou `redis` com base `abc`, quando salva, então a validação recusa com
  mensagem em português; `acme_db-1` (postgres) e `3` (redis) passam e saem iguais em
  `PGDATABASE` e no `/3` do `REDIS_URL` (D33).
- [x] V2.38 — (SEG) Dado o anexo `app.env` com as linhas `PORT=8080`, `DEBUG=1` e
  `FALSO_ENV=falso-anexo-01`, quando a saída traz `porta 8080 debug 1`, então ela sai
  intacta; quando traz `falso-anexo-01` sozinho ou a linha `PORT=8080` inteira, então
  vira `••••`; e um segredo de campo de 4 caracteres segue mascarado (D34, D11) —
  cobre S4.
- [x] V2.39 — (SEG) Dado o cofre de teste gerado por `make dev-cofre` depois da Fase
  19b, quando o auditor decifra o corpo com a senha de teste, então a seção de
  configuração traz um hash PHC `$argon2id$` com sal e parâmetros, o tempo de trava 240
  e o backup externo desligado; `135790` não aparece em claro no arquivo, no corpo
  decifrado nem nos temporários (`grep -a`); um corpo v1, um corpo sem a seção e um
  corpo sem hash de PIN são recusados com o erro genérico (D30) — cobre S2, S9.
- [x] V2.40 — (SEG) Dado o repositório, quando o auditor procura `unsafe` e
  `allow(unsafe_code)` fora de `crates/ward-plataforma/`, então só acha
  `crates/ward-core/tests/corpo_memoria.rs`, com `// SAFETY:` em cada bloco e
  `alloc_zeroed` no lugar de `alloc`; nenhum crate de produção tem `unsafe` fora do
  `ward-plataforma` (D31).

Correções da auditoria de segurança do M2 (Fase 19c, D36–D45). As repros do auditor
estão em `scratchpad/ataque/src/bin/` (`mascara.rs`, `validacao.rs`, `gravar.rs`); cada
asserção abaixo vira teste de ataque no `ward-core`.

- [x] V2.41 — (SEG) Dada a máscara de `acme-ssh-chave-prod` (chave em campo, não
  anexo), quando a saída traz a chave inteira com CRLF, `head -n 4` da chave, a chave
  sem as linhas `-----BEGIN`/`-----END` (`grep -v -- -----`) e cada linha da chave
  sozinha, então nenhuma linha da chave com 6 caracteres ou mais sobra, com ou sem `\r`
  (o maior trecho que sobra é uma linha curta do fim do base64); e dado um `genérico`
  com o extra secreto `GCP` = JSON de conta de serviço de várias linhas, quando a saída
  traz o JSON inteiro, a linha `"private_key": "falso-chave-de-conta-de-servico-0123456789"`
  e só o valor (`jq -r .private_key`), então tudo vira `••••`; o mesmo para a linha
  `token: falso-token-kube-01` num anexo YAML (D36) — cobre S4.
- [x] V2.42 — (SEG) Dado um `banco` com a senha `Ab3$k&<9>Zq!`, quando a saída traz
  `{"Env":["PGPASSWORD=Ab3$k\u0026\u003c9\u003eZq!"]}` (JSON do Go) e
  `declare -x PGPASSWORD="Ab3\$k&<9>Zq!"`, então nenhum pedaço da senha sobra; dada a
  senha `senha-ação-01`, quando a saída traz `{"p": "senha-a\u00e7\u00e3o-01"}`
  (`json.dumps`), então vira `••••`; dada `falso-😀-01`, quando traz
  `falso-\ud83d\ude00-01`, então vira `••••`; e a saída sem segredo que contém
  `\u0026` segue intacta (D37) — cobre S4.
- [x] V2.43 — (SEG) Dadas a máscara com D36 e D37 e as mesmas entradas do V2.36 (maior
  entrada aceita de cada tipo com entrega, com chave privada de 64 KiB em várias
  linhas no `servidor` e no `ftp/sftp` e um extra secreto de várias linhas, e o par
  banco + `servidor` de túnel de D19), quando o `Gerador` monta, então não passa de 8
  MiB e o custo medido pela validação é ≥ o real; `make dev-cofre` aceita as 21
  entradas de D15 sem mudança; e uma entrada que passou do orçamento só por causa de
  D36/D37 é recusada com "entrada grande demais para a máscara do `ward run`" (D29) —
  cobre S4.
- [x] V2.44 — (SEG) (Reescrita pela Fase 19e, D53; a aprovação anterior valia para "segue
  com n".) Dado o cofre de teste com a geração n, quando grava por
  `gravar_entradas` e quando troca a senha, então o arquivo relido tem n+1 e depois
  n+2; quando a gravação falha antes do rename (injeção do V2.9), então o arquivo
  segue com n e o `CofreAberto` fica com n+1 (geração queimada), e a gravação seguinte
  grava n+2;
  quando o teste copia o arquivo da geração n por cima do da n+2 (rollback com versão
  autêntica) e abre com `geracao_minima` = n+2, então a abertura falha com
  `Retrocesso`, sem entrada devolvida e sem valor no erro; um corpo v2 e um corpo com
  geração 0 são recusados com o erro genérico; um cofre na geração `u64::MAX` recusa
  gravar de novo sem tocar no disco; e um crate de fora que tenta mudar a
  geração por `configuracao_mut()` não compila (doctest `compile_fail`) (D39) — cobre
  S7, S9.
- [x] V2.45 — (SEG) Dado um `login` com `grupo` = `acme` + c + `prod`, para c em
  U+0600, U+1D173, U+13430, U+1BCA0, U+3164, U+115F, U+FFA0, U+2800, U+FE0F, U+180B,
  U+E0100, U+034F, U+200B, U+202E, U+00A0, U+3000, `a` + U+0301 e um emoji, quando
  salva e quando o corpo forjado traz o mesmo texto em `projeto`, tag, nome de extra,
  nome de anexo e `usuário` do `login`, então tudo é recusado com "caractere não
  permitido", sem o valor; `São Paulo`, `ação`, `Müller` e `Ñandú` passam; e uma senha
  com U+200B e U+3164 passa, porque segredo não é texto de exibição (D38) — cobre S13.
- [x] V2.46 — (SEG) Dado o `ward-core`, quando compila, então uma asserção de
  compilação prende `size_of::<cripto::Chave>()` a `size_of::<usize>()` (a chave de
  dados e a KEK são `Box<Zeroizing<[u8; 32]>>`); quando o auditor procura por `grep`,
  então nenhuma função nem campo recebe ou guarda `Zeroizing<[u8; 32]>` por valor fora
  do `Box`, o `buf` de `ler_phc_pin` é `Zeroizing`, a saída recalculada em
  `conferir_pin` fica num `Zeroizing` comparado por `ctutils` e nenhum código de
  produção chama `hash_password`/`verify_password`; e o V2.33 segue com zero achados
  (D40) — cobre S8.
- [x] V2.47 — (SEG) Dado um `login` e um `api`, quando a URL é
  `https://cliente.com.br@evil.example/`, `https://evil.example\@cliente.com.br/`,
  `https://evil.example%2f@cliente.com.br`, `http://evil.example`,
  `https://x.example:99999`, `https://x.example:0`, `https://x.example:080`,
  `HTTPS://x.example`, `https://X.example`, `https://сliente.com.br` (с cirílico),
  `https://-x.example`, `https://x.example/a\b`, `https://x.example/%zz`,
  `https://x..example` ou `https://010.0.0.1`, então a validação recusa com mensagem em
  português; `https://cliente.com.br/wp-login.php?x=1#y`, `https://xn--clente-9ya.com.br`,
  `https://[::1]:8443/`, `http://localhost:8083/login` e `http://127.0.0.1:8081/` passam
  (D41, D9) — cobre S13.
- [x] V2.48 — (SEG) Dado um `api`, quando a variável é `GIT_PROXY_COMMAND`,
  `GIT_EXTERNAL_DIFF`, `PAGER`, `GIT_PAGER`, `EDITOR`, `LESSOPEN`, `PS0`,
  `PROMPT_COMMAND`, `BROWSER`, `JAVA_TOOL_OPTIONS`, `OPENSSL_CONF`, `SSLKEYLOGFILE`,
  `HTTPS_PROXY`, `LC_ALL`, `TZ`, `SSHPASS`, `LD_PRELOAD`, `WARD_X_TOKEN` ou
  `GIT_ASKPASS`, então a validação recusa, e o corpo forjado com ela também; e
  `GITHUB_TOKEN`, `GH_TOKEN`, `NPM_TOKEN`, `OPENAI_API_KEY`, `STRIPE_SECRET_KEY`,
  `DOCKER_PASSWORD` e `NPM_CONFIG__AUTH` passam e saem no `ward run` com o token (D42)
  — cobre S3.
- [x] V2.49 — (SEG) Dado um `servidor` e um `ftp/sftp`, quando o usuário (ou o do salto)
  é `u;id`, `u'$(id)'`, `u%h`, `-l`, `-deploy@x`, `u v`, `u\x` ou `usuário`, então a
  validação recusa, e o corpo forjado também; e `deploy`, `app_user`, `j.silva`,
  `www-data` e `deploy@cliente.com.br` passam (D43; `@` aceito por decisão do usuário);
  e, num `banco` e num `smtp`, o usuário `-oProxyCommand=id`, `-l`, `de ploy` ou
  `usuário` é recusado e `app_user` e `alertas@acme.com.br` passam — cobre S3, S13.
- [x] V2.50 — (SEG) Dado um crate fora do `ward-core` (doctest `compile_fail`), quando
  chama `ward_core::gravacao::gravar_cofre(destino, &cofre, corpo)` ou
  `cofre.selar(corpo)` com um corpo sem hash de PIN, então não compila; e o auditor
  confere por `grep` que os únicos `pub fn` do `ward-core` que gravam são
  `gravar_entradas` e `gravacao::trocar_senha`, os dois com corpo serializado ou
  desserializado antes de selar (D44, D27) — cobre S2, S7.
- [x] V2.51 — Dadas as escolhas confirmadas em D45, quando os testes rodam, então:
  trava 0 e 1441 recusadas e 1 e 1440 aceitas, sem valor que signifique "nunca";
  `HashPin::criar` usa 64 MiB/3/1, e a leitura recusa abaixo disso e acima de 256
  MiB/16/4; UUID vazio, de 37 caracteres ou com `-` na ponta recusado; texto PHC não
  canônico recusado; base `redis` `00` e `03` recusadas e `0` aceita; e uma mutação
  que muda `MAX_CORPO` ou `MAX_ENTRADAS` numa cópia de trabalho faz reprovar o teste da
  mensagem de `ErroCorpo::GrandeDemais` (ou a mensagem acompanha a constante) (D45, D32).
- [x] V2.52 — (SEG) Dado o `dev/variacoes.py` (oráculo independente da máscara), quando
  o teste dele recebe valores falsos montados no próprio teste, então a lista traz: cada
  linha aparada com 6+ caracteres de um segredo de várias linhas e o valor (com e sem
  aspas, 6+) de `CHAVE=valor`, `"chave": "valor"` e `chave: valor` (D36); para
  `falso-Ab3$k&<9>Zq!`, a forma `falso-Ab3$k\u0026\u003c9\u003eZq!` (JSON do Go) e
  `falso-Ab3\$k&<9>Zq!` (aspas duplas do shell); para `senha-ação-01`,
  `senha-a\u00e7\u00e3o-01`; para `falso-😀-01`, `falso-\ud83d\ude00-01`; para o
  valor `falso` + U+2028 + `linha01`, a forma `falso\u2028linha01`; e nenhuma
  dessas formas quando o valor não tem o caractere (`falso-simples-01` não ganha padrão
  novo); quando roda `make teste-dev-cofre`, então `make dev-variacoes`
  continua determinístico, 0600, sem linha vazia nem padrão com menos de 6 bytes, e
  sem segredo na saída (D36, D37, V2.29) — cobre S4.

Reauditoria do M2 (Fase 19e, D53–D55). Repros em `scratchpad/ataque/src/bin/`
(`geracao.rs`, `desvio.rs`, `url.rs`); cada asserção vira teste de ataque no
`ward-core` (e no teste do `variacoes.py`, para a V2.57).

- [x] V2.53 — (SEG) Dado um cofre na geração n num diretório temporário, quando o
  ataque do `geracao.rs` tira a leitura do diretório (0300) para o fsync do diretório
  falhar depois do rename, grava a mudança A (uma entrada a menos), devolve 0700 e
  grava a mudança B (a entrada de volta), então a gravação A devolve erro com o
  `CofreAberto` em n+1, B grava n+2, e a cópia do arquivo A posta no lugar de B abre
  com `geracao_minima` = n+2 → `Retrocesso`, sem entrada e sem valor; nenhum par de
  arquivos selados nessa sequência tem a mesma geração; quando o teste reabre o
  arquivo A com `geracao_minima` = n e `geracao_queimada` = n+2, então o `CofreAberto`
  fica em n+2 e a gravação seguinte grava n+3; e `trocar_senha` com
  `geracao_queimada` = n+5 sobre um arquivo em n+3 grava n+6 (D53) — cobre S7, S9.
- [x] V2.54 — (SEG) Dado um `api` com o token `falso-tok-linha-um-01\nfalso-tok-linha-dois-02`
  e a máscara montada só por `ConstrutorMascara::new().entrega(&g).construir()`, quando
  a saída traz `head -n1 <<<"$GITHUB_TOKEN"` (bash real), cada linha sozinha, a linha
  com `\r` e a linha aparada, então nenhum trecho de 6+ caracteres do token sobra; o
  mesmo com `Mascara::da_entrada` (D54) — cobre S4.
- [x] V2.55 — (SEG) Dado um `genérico` com o extra secreto
  `linha-um-ação-01\nlinha-dois-ação-02\nlinha-tres-Ab3$k&<9>Zq!`, quando a saída é a
  de `python3 -I -c 'json.dumps(os.environ["NOTA"].splitlines())'` (linhas em
  `ensure_ascii`), de `json.dumps({"l": linha1})`, de `L=$(sed -n 3p <<<"$NOTA");
  declare -p L` e da linha 3 no JSON do Go, então nenhum trecho de 6+ caracteres de
  nenhuma linha sobra, comparando também com a forma decodificada; e a saída sem
  segredo com `\u00e7` segue intacta (D54, formas de texto por linha) — cobre S4.
- [x] V2.56 — (SEG) Dado o `desvio.rs` contra o código novo, quando a saída traz: `printf
  '%q\n' "$NOTA"` de `Ab3$k&<9>Zq!` (bash real); `echo "${NOTA@Q}"`, `set | grep
  ^NOTA=` e `set -x` com `falso-it's-01x`; `falso\/a\u00e7\u00e3o\/01` (`json_encode`
  do PHP) para `falso/ação/01`; e, num extra secreto de várias linhas, as linhas
  `senha2 : falso-yaml-esp-02`, `senha:\tfalso-yaml-tab-01`, `"k":"falso-json-colado-03"`,
  `"private key": "falso-valor-com-espaco-na-chave-01"` e `"x/y":
  "falso-valor-barra-chave-02"`, cada uma inteira e só o valor (`jq -r`), e a linha
  `falso-um-linha\tcom-tab` cortada por `cut -f1`, então tudo vira `••••` e nenhum
  trecho de 6+ caracteres sobra; e, para os casos que D54 manda para RA3 (`printf %q`
  de valor com controle, `expand` da linha com tab, base64 de uma linha solta), o
  teste registra o resultado sem exigir máscara; e o V2.43 refeito com a tabela de D54
  segue ≤ 8 MiB, com o custo da validação ≥ o real e as 21 entradas de D15 aceitas
  (D54, D29) — cobre S4.
- [x] V2.57 — (SEG) Dado o `dev/variacoes.py` e o cofre de teste, quando o teste cruzado
  (rodado por `dev/testes/dev-cofre.sh` em `make teste-dev-cofre`) põe cada padrão de
  `make dev-variacoes` sozinho numa saída e passa pela máscara da entrada de onde ele
  veio, então todo padrão sai `••••`; e o teste do `variacoes.py` confere, com valores
  falsos montados nele, que uma linha de segredo de várias linhas ganha as formas de
  texto (JSON padrão, `\/`, Go, `ensure_ascii`, PHP, aspas duplas, aspas simples,
  `printf %q`) e os campos de tab, e **não** ganha base64 nem miolos; o oráculo continua
  sem importar nem chamar o código da máscara, determinístico, 0600, sem linha vazia
  nem padrão com menos de 6 bytes (D54, V2.52) — cobre S4.
- [x] V2.58 — (SEG) Dado um `login` e um `api`, quando a URL é `https://0x7f000001/`,
  `https://0x7f.0.0.1/`, `https://1.2.3.0x4/`, `https://x.123/`, `https://x.0x10/` ou
  `https://x.example.9z/`, então a validação recusa com a mensagem de URL inválida, e o
  corpo forjado com ela também; e `https://cliente.com.br/`, `https://x1.example/`,
  `https://1password.com/`, `https://x.example2/` e os casos aceitos da V2.47 passam
  (D55, D41) — cobre S13.

## Milestone 3 — Daemon e protocolo

- [x] V3.1 — (SEG) Dado o daemon de teste no ar, quando `stat` em `<rt>` e no socket,
  então o diretório é 0700 e o socket 0600, do usuário; quando `<rt>` já existe com 0755
  ou de outro dono, então o daemon não sobe — cobre S5.
- [x] V3.2 — (SEG) Dado um contêiner `ubuntu:24.04` que não monta nada do `/run/user`
  do host, com o `wardd` de teste subido pelo root do contêiner como uid 1000
  (`setpriv --reuid 1000 --regid 1000 --clear-groups`) em `/run/user/1000/ward-teste/`
  (D49), quando conecta: (a) um cliente de uid 4242 sem capacidade → o `connect` falha
  com EACCES; (b) um cliente de uid 4242 com `CAP_DAC_OVERRIDE` ambiente (`setpriv
  --reuid 4242 --inh-caps +dac_override --ambient-caps +dac_override`) que manda
  `lista` → o daemon fecha sem mandar nenhum byte e o log traz `UidRecusado` com uid
  4242; (c) um cliente uid 0 → igual a (b); e depois um cliente do uid 1000 recebe
  resposta normal. Roda por `make teste-docker-uid` — cobre S5.
- [x] V3.3 — (SEG) Dado um cliente em Python que fala direto com o socket
  (`ataque_socket.py`), quando manda cada operação do protocolo com cada entrada do
  cofre de teste e com campos extras inventados (`"revelar": true`, `"campo":
  "senha"`), então nenhuma resposta contém segredo e as operações de app e extensão
  respondem "não autorizado" — cobre S1.
- [x] V3.4 — (SEG) Dado o mesmo cliente, quando manda quadro de 2 MiB, tamanho negativo,
  JSON inválido, operação desconhecida e 10 000 conexões abertas sem falar, então o
  daemon recusa ou fecha cada uma e continua respondendo a um cliente normal.
- [x] V3.5 — (SEG) Dado o binário de teste do app, quando é copiado para
  `/tmp/ward-app` e pede `revelar`, quando o binário permitido sobe com
  `LD_PRELOAD=<libqualquer>.so`, e quando sobe sob `strace -f` (TracerPid ≠ 0), então
  `revelar` e `copiar` respondem "não autorizado" nos três casos — cobre S1 (D5).
  Com D47 (confirmada em 2026-11-08), o mesmo ataque vale com o binário setgid num contêiner: a cópia perde
  o setgid e vira agente, o `LD_PRELOAD` é ignorado pelo `ld.so` (`AT_SECURE`) e o
  `strace -f` não consegue se prender; o que se confere é o papel, não o `/proc`.
- [x] V3.6 — (SEG) Dado o `wardd` no ar, quando outro processo do usuário tenta
  `cat /proc/<pid>/mem`, `cat /proc/<pid>/environ`, `gdb -p <pid>` e
  `strace -p <pid>`, então todos falham por permissão; `/proc/<pid>/limits` mostra core
  0; e `kill -SEGV` no daemon não deixa core em `coredumpctl` nem no disco — cobre S8.
- [x] V3.7 — (SEG) Dado `LD_PRELOAD`, `LD_AUDIT` ou `LD_LIBRARY_PATH` definidos, ou o
  daemon iniciado sob `strace`, quando o `wardd` sobe, então sai ≠ 0 com mensagem e
  sem criar socket — cobre S8.
- [x] V3.8 — (SEG) Dado o daemon destravado, quando trava, então o estado não guarda
  mais entradas nem a chave de dados, e o teste com observador do buffer confirma que
  a memória da chave foi zerada antes de liberar — cobre S8.
- [x] V3.9 — (SEG) Dado o socket, quando um cliente tenta destravar com 10 senhas
  erradas seguidas, então cada tentativa demora mais que a anterior (espera crescente
  até 30 s, D20) e nenhuma resposta diz quantas faltam.
- [x] V3.10 — Dado o daemon destravado, quando recebe `travar`, então qualquer operação
  seguinte responde "travado" até destravar de novo.
- [x] V3.11 — (SEG) Dado `mudar_nivel` de `acme-pg-dev` para bloqueado, quando vem do
  papel app sem PIN ou com PIN errado, então recusa e o arquivo não muda; quando vem do
  papel agente com o PIN certo, então responde "não autorizado" — cobre S9.
- [x] V3.12 — (SEG) Dado o papel app, quando erra o PIN 5 vezes seguidas (inclusive
  reconectando entre tentativas), então o cofre trava; 4 erros e 1 acerto zeram a
  contagem — cobre S9.
- [x] V3.13 — (SEG) Dado o cofre de teste aberto pelo núcleo, quando o auditor procura
  o PIN, então ele só existe como hash Argon2id na seção de configuração do corpo v2,
  dentro da parte cifrada (D30), e a comparação é em tempo constante — cobre S9.
- [x] V3.14 — (SEG) Dado o motor de aprovação com relógio injetado e o aprovador em
  processo (D8), quando: não há resposta por 120 s → negado; a janela fecha → negado;
  "Permitir uma vez" → só aquele pedido passa, o seguinte pergunta de novo; "Permitir
  por 1 hora" → pedidos até 59:59 passam sem perguntar, em 60:01 pergunta, e travar
  encerra a liberação (D2); "Permitir sempre" → a entrada vira livre no arquivo; os
  três "Permitir" sem PIN válido → negado — cobre S10, S9.
- [x] V3.15 — (SEG) Dado o protocolo de release, quando o auditor lista as operações por
  papel, então nenhuma de aprovar, responder aprovação ou mudar nível é aceita do papel
  agente, e não há feature, variável ou argumento que ligue um aprovador no socket —
  cobre S10 (D8).
- [x] V3.16 — Dadas `acme-pg-prod` (com aprovação) e `acme-wp-prod` (livre), quando o
  motor avalia, então `prod` livre passa sem perguntar e o pedido de `prod` com
  aprovação oferece os quatro botões, inclusive "Permitir sempre" (R16).
- [x] V3.17 — (SEG) Dado `gama-bloqueada`, quando o papel agente pede `lista`, então ela
  não aparece; quando pede `run` com ela, então a resposta é byte a byte igual à de uma
  entrada inexistente — cobre S1.
- [x] V3.18 — Dado o cofre travado, quando chega um pedido do agente, então o daemon
  pede a abertura do app e segura o pedido; destravado em 60 s → o pedido segue;
  passados 120 s → "travado: tempo esgotado" (D16).
- [x] V3.19 — Dado o daemon de teste destravado, quando roda `loginctl lock-session`,
  então em até 2 s está travado; dado o sinal `PrepareForSleep(true)` simulado no teste,
  então trava; dado o relógio injetado, então 3h59 sem uso não trava, 4h00 trava e um
  uso às 3h zera a contagem.
- [x] V3.20 — (SEG) Dada a suíte de integração do M3 com `RUST_LOG=trace`, quando o
  auditor procura segredos em stdout, stderr, journal e `.dev/logs/`, então não acha
  nenhum — cobre S13.
- [x] V3.21 — Dadas as units de teste `wardd@.socket` e `wardd@.service` do sistema
  (D46), quando o auditor lê, então o `.socket` tem `ListenStream=/run/ward/%i/wardd.sock`,
  `SocketUser=%i` e `SocketMode=0600`, e o `.service` tem `User=%i`, `LimitCORE=0` e
  nenhum `Environment=LD_*`; e, num contêiner com systemd, a primeira conexão ao socket
  sobe o `wardd`.
- [x] V3.22 — (SEG) Dado o daemon de teste com o cofre na geração n+2 e uma cópia
  autêntica da geração n (backup de teste), quando, com o daemon travado, o atacante faz
  `cp` da cópia por cima de `.dev/cofre-teste.ward` e destrava, então o destravar
  recusa com "o cofre no disco é mais antigo que o último aberto", o cofre segue
  travado e o arquivo não é regravado; quando o atacante apaga o arquivo de estado e
  destrava, então recusa igual; quando, com o daemon destravado, o atacante troca o
  arquivo por `mv` (inode novo) ou o reescreve no lugar (`mtime` novo) e o agente pede
  uma operação que grava, então o daemon não grava por cima, trava e avisa "o arquivo
  do cofre mudou fora do Ward"; o arquivo de estado é 0600, sem segredo, e o log não
  traz valor (D39) — cobre S7, S9, S13.
- [x] V3.23 — (SEG) Dado um servidor falso do mesmo uid escutando no caminho do socket
  de teste (um `socat` ou script Python), quando a conferência do cliente da
  `ward-plataforma` roda num build sem `modo-teste`, então recusa antes de mandar
  qualquer byte (o uid do par não é 0, ou a pasta não é do root); dado o listener
  criado pelo root de um contêiner em `/run/ward/1000/` (root:root 0755, socket 0600 do
  uid 1000) e entregue ao `wardd` de teste por `LISTEN_FDS`, então a conferência aceita
  e o `wardd` atende; quando o atacante (uid 1000) mata o `wardd` e tenta `bind` em
  `/run/ward/1000/`, então recebe EACCES — cobre S5 (D46).
- [x] V3.24 — (SEG) Dado `ward-app` e `ward-nm` de teste instalados no contêiner como
  root:`ward-app`/root:`ward-nm` 2755, quando cada um conecta, então recebe o papel
  pelo grupo; quando o processo que os lançou faz `PTRACE_ATTACH`/`PTRACE_SEIZE` logo
  depois do `exec`, com o filho parado antes de conectar, então recebe EPERM; quando o
  pai já está preso a ele desde antes do `exec` (`PTRACE_TRACEME`), então o kernel não
  aplica o setgid e o processo conecta como agente (D60); quando sobem com `LD_PRELOAD=<lib>` que grava um arquivo-marca, então a
  marca não existe; quando um processo do uid de teste com outro gid (ou num namespace
  de usuário) conecta, então fica agente (com o userns sem privilégio bloqueado no
  sistema, o caso diz isso na saída e passa, D60); e com `ptrace_scope` 0 simulado no
  teste, o daemon recusa o papel app — cobre S1, S8 (D47).
- [x] V3.25 — (SEG) Dado o `wardd` subindo com `GLIBC_TUNABLES`, `GCONV_PATH` ou um
  `LD_` de nome inventado, então sai ≠ 0 com o nome da variável na mensagem,
  higienizado (`[A-Za-z0-9_]`, até 64 caracteres; um nome com `\n`, `\x1b` ou 200
  caracteres sai cortado e só com esses caracteres), e sem o valor; o mesmo com
  `TMPDIR` e `MALLOC_ARENA_MAX` (D48, D60) — cobre S8.
- [x] V3.26 — (SEG) Dado o papel agente, quando pede `lista` (texto e `--json`), então
  nenhuma entrada traz o nível; dado o papel app, então traz (D50) — cobre S1.
- [x] V3.27 — (SEG) Dado o papel app, quando cria uma entrada sem nível, então ela nasce
  "com aprovação"; quando cria com "livre" sem PIN, então recusa; quando remove
  `acme-pg-aprovacao` e a recria com o mesmo nome, então ela volta "com aprovação"
  (D50) — cobre S9.
- [x] V3.28 — (SEG) Dado o daemon de teste destravado na geração n, com a gravação do
  cofre forçada a falhar depois do rename (fsync do diretório, injeção de teste),
  quando o papel app grava a mudança A e depois a B, então o arquivo de estado traz
  `queimada` = n+1 antes do rename de A, A responde `falha_ao_gravar`, B grava n+2 e o
  estado fica com `vista` = `queimada` = n+2; quando o atacante põe a cópia de A no
  lugar do cofre e destrava, então recusa com "o cofre no disco é mais antigo que o
  último aberto"; quando o daemon é morto e sobe de novo entre A e B, então B ainda
  grava um número maior que `queimada`; quando a escrita do estado falha, então o cofre
  não é gravado; e quando `trocar_senha` falha depois de selar, então o cofre trava
  (D53) — cobre S7, S9.
- [x] V3.29 — (SEG) Dada a Fase 22a, quando `make dev-cofre` e `make teste-dev-cofre`,
  então o cofre de teste tem 22 entradas, `delta-api-simbolos` abre igual à tabela
  (D59) e cabe no orçamento de D29; e quando o teste cruzado põe sozinho na saída cada
  padrão do oráculo dela (literal, base64, percent, aspas simples com `'\''`, aspas
  duplas do shell, `printf %q` do `SEGREDO_ASCII`, JSON do Go, PHP e o JSON do Go com
  `ensure_ascii`, `falso-Ab3$k\u0026\u003c9\u003eZq!'/\u00e7-01`, de D58), então cada
  um sai `••••`; o mesmo para o Go com `ensure_ascii` num segredo de várias linhas,
  linha a linha; e uma linha binária de anexo continua mascarada pelo literal, sem
  formas de texto (D54) — cobre S4.
- [x] V3.30 — Dado `TMPDIR=/tmp` e `LD_LIBRARY_PATH=/x` na sessão, quando `make
  dev-daemon`, então o daemon de teste sobe e atende `ward lista` (ambiente limpo por
  `env -i` e lista fixa, D60; com qualquer uma das duas ele recusaria subir, V3.25).
- [x] V3.31 — (SEG) Dadas entradas `genérico` criadas no teste (fora do cofre de D15)
  com o anexo `client.json` de uma linha
  `{"installed":{"client_id":"x.apps","client_secret":"falso-GOCSPX-minificado-01"}}`,
  o extra secreto `{"user":"x","token":"falso-json-uma-linha-01"}` e um extra de várias
  linhas com `"db": {"senha": "falso-json-aninhado-02"},` e
  `"a": "x", "b": "falso-json-duas-chaves-03"`, quando a máscara da entrada recebe a
  saída real de `python3 -I -c 'print(json.loads(...)[...])'` de cada valor (e de um
  valor com `\u00e7` e `\/` no JSON, impresso decodificado), então nenhum trecho de 6+
  caracteres de nenhum dos valores sai em claro; e o `dev/variacoes.py` gera os mesmos
  padrões, que o teste cruzado põe sozinhos na saída e saem `••••` (D61) — cobre S4.
- [x] V3.32 — (SEG) Dado um extra secreto de várias linhas com
  `senha: falso-yaml-coment-01 # comentario`, `private key: falso-yaml-chave-espaco-02`,
  `aws/secret: falso-yaml-chave-barra-03`, `SENHA_DB="falso-env-coment-05" # dotenv` e
  `tok: 'falso-it''s-yaml-06'`, quando a máscara recebe cada valor como o `yq` e o
  `source` do bash o imprimem (`falso-it's-yaml-06` no último), então nenhum sai em
  claro; dado um anexo com a linha Latin-1
  `SENHA=falso-latin\xe7-01<TAB>TOKEN=falso-campo-tab-02` e
  `nome\xe7<TAB>falso-campo-tsv-03<TAB>falso-campo-tsv-04`, quando a saída é a do
  `cut -f2` e do `cut -f3`, então sai `••••`; e o custo das duas entradas cabe no
  orçamento de D29, com as 22 de D15 ainda aceitas (V2.36, V2.43). `iconv` e `repr()`
  ficam em RA3 e não são cobrados aqui (D61) — cobre S4.
- [x] V3.33 — (SEG) Dado o daemon de teste travado, quando um cliente do papel agente
  erra a senha 10 vezes, depois o atacante mata o `wardd` (`kill -9`), o daemon sobe de
  novo e o agente tenta outra senha, então essa tentativa ainda espera 30 s (contador
  do agente no arquivo de estado, gravado antes da resposta de erro); com o agente no
  meio das esperas, o papel app destrava com a senha certa sem esperar; depois do
  destravar certo, os dois contadores zeram; o arquivo de estado no formato antigo é
  lido como contador 0; e o arquivo de estado não tem senha, PIN nem segredo (D62) —
  cobre S9.
- [x] V3.34 — (SEG) Dado o `wardd` de release no contêiner com `RLIMIT_MEMLOCK`
  infinito, quando destrava o cofre de teste e roda uma operação, então `VmLck` no
  `/proc/<pid>/status` é maior que zero e cobre o `VmRSS` (`MCL_ONFAULT`), e o app
  lançado tem `VmLck: 0 kB`; dado o mesmo binário com `RLIMIT_MEMLOCK` de 8 MiB, então
  sai ≠ 0 com mensagem e sem criar socket; com `modo-teste` e 8 MiB, sobe com aviso no
  log; e as units de teste da Fase 24 têm `LimitMEMLOCK=infinity` (D62) — cobre S8.
- [x] V3.35 — (SEG) Dado um estado com `vista` = `queimada` = 7 e nenhum cofre, quando
  o papel app cria o cofre, então ele nasce na geração 8 e o estado fica 8/8; quando o
  atacante põe no lugar a cópia de um cofre anterior na geração 5 e destrava, então
  recusa com "o cofre no disco é mais antigo que o último aberto"; e dado um gancho de
  teste que troca o arquivo logo depois do rename do `gravar_entradas`, quando o daemon
  grava de novo, então responde "o arquivo do cofre mudou fora do Ward", trava e não
  grava (identidade lida do fd do temporário, D62) — cobre S7, S13.
- [x] V3.36 — (SEG) Dado o contêiner da 21b com `ward-app` de teste setgid, quando o
  lançador do daemon o abre, então ele recebe o papel app, e depois de se reexecutar
  (mesmo pid) continua app; quando um processo com egid `ward-app` e `PPid` igual ao
  daemon que **não** veio do lançador conecta (no teste do classificador, um pid que
  não é o do pidfd; na Fase 26, V4.38), então fica agente; e quando o app lançado já
  morreu e o pidfd não vale mais, então nenhum par com aquele pid é app (D63) — cobre
  S1.
- [x] V3.37 — (SEG) Dado `make dev-daemon` com `WARD_DEV_APP` da ponte, quando `make
  app-ponte`, então a ponte é aberta pelo daemon, o `estado` dela diz `app` e o QA
  destrava, cria uma entrada e muda o nível com o PIN de teste pelo Chrome; quando
  conectam um cliente Python, a `ward-ponte-teste` executada à mão, uma cópia dela ou um
  `ward-app` de teste aberto fora do lançador, então todos ficam agente e `revelar` e
  `mudar_nivel` respondem "não autorizado"; quando um cliente manda `abrir_app` com
  caminho, argumentos ou ambiente no pedido, então o daemon abre só o `WARD_DEV_APP`,
  com o ambiente fixo; no build sem `modo-teste`, um par sem egid aberto pelo lançador
  fica agente; e `strings` no `wardd` e no `ward-plataforma` de release não acha
  `WARD_DEV_APP` (D63, D7, D8) — cobre S1, S10, S11.
- [x] V3.38 — (SEG) Dado o daemon de teste, quando um cliente pede `estado`, então a
  resposta traz o papel (`agente`, `app` ou `extensao`) e nada mais de novo; quando o
  app de teste ou a ponte recebem `agente` na primeira resposta (aberto fora do
  lançador ou com o pai trocado), então saem com mensagem antes de mostrar qualquer
  tela de credencial; e o log do daemon registra o código de saída (ou o sinal) do app
  que lançou, só o número (D63) — cobre S1.

- [x] V3.39 — (SEG) Dado o daemon de teste destravado, quando um cliente Python no papel
  agente (e outro simulando extensão, fora do pai Chrome) pede `revelar`, `copiar` e
  `totp` de `acme-pg-dev` e de `gama-bloqueada`, então todos recebem "não autorizado" e
  nenhuma resposta contém segredo; quando o app de teste (papel app pelo lançador, D63)
  pede `revelar` da senha de `acme-pg-dev` e da de `gama-bloqueada`, então recebe o valor
  sem PIN e sem janela de aprovação; travado, recebe "travado"; campo inexistente, "campo
  inexistente"; e o log do daemon tem `revelar <entrada> <campo>` sem nenhum padrão de
  `.dev/variacoes.txt` (D4, D65) — cobre S1, S13.
- [x] V3.40 — (SEG) Dado o daemon de teste, quando o app de teste pede `lista`, então
  cada item traz `resumo` (campos de texto, ids dos segredos, nº de extras e de anexos)
  e nenhum segredo; quando um cliente agente pede `lista`, então nenhum item traz
  `resumo` nem usuário; um quadro forjado com `resumo` decodificado no papel agente é
  recusado, e `codificar_resposta` recusa mandar ao agente um item com `resumo` (D50,
  D65) — cobre S1.
- [x] V3.41 — (SEG) Dado o corpo v4 (D65), quando o app cria uma entrada, edita, muda o
  nível e o daemon reabre o cofre, então `criada_em` não muda, `alterada_em` sobe a cada
  gravação (relógio injetado) e as duas chegam só no `detalhe` do papel app; um
  `salvar_entrada` com data no rascunho é recusado pelo decodificador; um cofre v3 é
  recusado como versão não suportada sem tocar no arquivo; e 1 byte alterado nas datas
  faz a abertura falhar como em V2 — cobre S1, S6.

## Milestone 4 — CLI e integração Claude (run/ssh)

- [ ] V4.1 — Dado o cofre destravado, quando `ward lista`, então aparecem as 21
  entradas visíveis (as 22 do cofre de teste menos `gama-bloqueada`) com nome, tipo,
  grupo, ambiente, projeto e URL/host; `ward lista --json` tem só esses campos e nenhum
  segredo.
- [ ] V4.2 — (SEG) Duas partes (D65). (1) Build de teste: dado o daemon parado e um
  servidor falso em Python no socket de teste que grava tudo o que recebe, quando o
  usuário roda `ward destravar` e digita a senha (via `expect`), então a CLI aborta com
  "servidor não é o wardd" **antes** de ler ou enviar a senha, e o registro do falso não
  contém a senha. (2) Build sem `modo-teste`, no contêiner de `make
  teste-docker-plataforma`: dado o `wardd` verdadeiro copiado para `/tmp` e, à parte,
  um falso que chama `prctl(PR_SET_DUMPABLE, 0)`, os dois rodando como uid 1000 e
  escutando em pastas desse uid, quando o uid 1000 roda `ward destravar`, `ward lista` e
  `ward travar` e digita a senha, então a CLI sai ≠ 0 com "o daemon não está rodando" ou
  "servidor não é o wardd" sem pedir a senha, nenhum dos dois falsos recebe byte algum,
  e o `bind` do uid 1000 em `/run/ward/1000/` falha — cobre S5.
- [ ] V4.3 — Dado `acme-wp-prod` (livre), quando
  `ward run acme-wp-prod -- sh -c '<sha256 de cada variável>'`, então os hashes batem
  com "Entrega esperada" e `WARD_TOTP` bate com `oathtool`.
- [ ] V4.4 — Dado `acme-pg-dev`, quando `ward run` com os hashes, então as seis
  variáveis batem com "Entrega esperada".
- [ ] V4.5 — Dado `acme-ssh-senha-stage` e `acme-ssh-chave-prod`, quando `ward run` com
  os hashes, então as variáveis batem e `WARD_ARQUIVO_CHAVE` aponta para arquivo 0600
  em `<rt>tmp/` cujo hash é o da chave privada de `.dev/`.
- [ ] V4.6 — Dado `beta-github-api`, quando `ward run` com os hashes, então
  `WARD_TOKEN` e `GITHUB_TOKEN` batem.
- [ ] V4.7 — Dado `beta-aws-dev`, quando `ward run` com os hashes, então as seis
  variáveis AWS batem.
- [ ] V4.8 — Dado `beta-smtp-stage`, quando `ward run` com os hashes, então as cinco
  `SMTP_*` batem.
- [ ] V4.9 — Dado `beta-sftp-dev`, quando `ward run beta-sftp-dev -- bash -c
  'SSHPASS="$WARD_PASS" sshpass -e sftp -P 2222 sftpuser@127.0.0.1 <<< ls'` (ou `sh -c`
  com `printf 'ls\n' | SSHPASS="$WARD_PASS" sshpass -e sftp …`: o `sh` não tem `<<<`),
  então conecta, e os hashes das `WARD_*` batem.
- [ ] V4.10 — Dado `gama-generico`, quando `ward run` com os hashes, então `WARD_REGIAO`,
  `WARD_CHAVE_LICENCA` e o conteúdo do arquivo de `WARD_ARQUIVO_APP_ENV` batem.
- [ ] V4.11 — Dado `gama-nota`, quando `ward run gama-nota -- touch .dev/marca`, então
  sai ≠ 0 com "tipo sem entrega" e `.dev/marca` não existe.
- [ ] V4.12 — Dado o Postgres de teste, quando `ward run acme-pg-dev -- psql -tAc
  'select current_user'`, então imprime `app` e sai 0; `ward run acme-pg-dev -- sh -c
  'echo $DATABASE_URL'` imprime só `••••`.
- [ ] V4.13 — (SEG) Dado `ward run acme-pg-dev -- sleep 30` em segundo plano, quando
  outro processo do usuário lê `/proc/<pid>/environ` e `<rt>tmp/*` de todos os
  processos e arquivos, então nenhum segredo aparece (RA1; reprovar aqui vai para o
  usuário) — cobre S2, S3.
  **Risco aceito pelo usuário (2026-11-09):** RA1 foi para "Fora do modelo de ameaça"
  no escopo; a asserção não bloqueia o M4. O teste `ra1_…` só registra o estado.
- [ ] V4.14 — (SEG) Dado `ward run` com `sleep 5` de cada entrada visível e `ward ssh` de
  `acme-ssh-senha-stage` e `acme-ssh-chave-prod`, quando um laço lê
  `/proc/*/cmdline` a cada 50 ms, então nenhum segredo aparece em argv — cobre S3.
- [ ] V4.15 — (SEG) Dado `gama-generico`, quando o comando termina com 0, termina com 1,
  quando o filho leva SIGKILL e quando a CLI leva SIGKILL, então o anexo some de
  `<rt>tmp/` em até 1 s; quando o `wardd` leva SIGKILL e sobe de novo, então `<rt>tmp/`
  é esvaziado na partida — cobre S2.
- [ ] V4.16 — (SEG) Dada uma marca de tempo antes da suíte do M4, quando o auditor roda
  `find ~ /tmp /var/tmp -newer <marca> -type f` (sem `.dev/` e o repositório) e procura
  segredos, então não acha nenhum arquivo — cobre S2.
- [ ] V4.17 — (SEG) Dado `acme-pg-dev`, quando o comando imprime `$PGPASSWORD` puro, em
  base64, com `urllib.parse.quote`, em dois `printf` separados por `sleep 0.2` (corte no
  `:`), e tudo isso também em stderr, então a saída devolvida tem só `••••` no lugar —
  cobre S4.
- [ ] V4.18 — (SEG) Dado cada subcomando e opção de `ward --help`, quando o auditor os
  roda com cada entrada visível (sem comando que imprima variável), então nenhuma saída,
  erro ou código de saída contém segredo — cobre S1.
- [ ] V4.19 — Dado `ward run acme-pg-dev -- sh -c 'exit 7'`, então a CLI sai 7; dado um
  filho morto por SIGTERM, então sai 143.
- [ ] V4.20 — Dado `acme-pg-aprovacao` sem app disponível, quando `ward run
  acme-pg-aprovacao -- touch .dev/marca`, então sai ≠ 0 com "aprovação indisponível"
  (D21) e `.dev/marca` não existe.
- [ ] V4.21 — Dado o cofre travado, quando `ward run acme-pg-dev -- true` e o usuário
  roda `ward destravar` em outro terminal dentro de 2 min, então o pedido segue e sai 0;
  sem destravar, sai ≠ 0 em 120 s ± 2 s.
- [ ] V4.22 — Dado `ward travar`, então `ward lista` responde "travado".
- [ ] V4.23 — Dado o SSH de teste por senha, quando `ward ssh acme-ssh-senha-stage
  whoami`, então imprime `deploy`; e o `ssh` filho rodou com `sshpass -e`,
  `PubkeyAuthentication=no` e `IdentityAgent=none` (conferido em `/proc/<pid>/cmdline`).
- [ ] V4.24 — Dado o SSH de teste só por chave, quando `ward ssh acme-ssh-chave-prod
  whoami`, então imprime `deploy`.
- [ ] V4.25 — Dado o salto, quando `ward ssh acme-ssh-interno hostname`, então imprime o
  hostname do `ssh-interno`.
- [ ] V4.26 — (SEG) Dado `ward ssh acme-ssh-chave-prod 'sleep 20'`, quando outro processo
  do usuário acha o socket em `<rt>ssh-*/agent.sock` e roda `SSH_AUTH_SOCK=<sock>
  ssh-add -L`, `ssh-add <chave qualquer>` e `ssh-add -D`, então a conexão é recusada nas
  três; ao fim (normal, e com SIGKILL na CLI) o socket e o diretório somem; travar o
  cofre durante a sessão também os remove (D12) — cobre S1.
- [ ] V4.27 — (SEG) Dado `HOME` de teste com `~/.ssh/config` contendo `Host *` e
  `ProxyCommand touch /tmp/sentinela-ssh`, um `known_hosts` e um `id_ed25519` de
  sentinela, `inotifywait -m ~/.ssh` rodando e `SSH_AUTH_SOCK` apontando para um agente
  falso que registra conexões, quando roda toda a suíte de `ward ssh` e túnel, então
  `/tmp/sentinela-ssh` não existe, o `inotifywait` não registra nenhum evento e o agente
  falso não recebe conexão (R19).
- [ ] V4.28 — (SEG) Dado `ward ssh acme-ssh-senha-stage true` já feito uma vez, quando a
  chave de host do `ssh-senha` é trocada, então o próximo `ward ssh` recusa com aviso de
  chave de host mudada (D13).
- [ ] V4.29 — Dado `acme-pg-tunel`, quando `ward run acme-pg-tunel -- psql -tAc 'select
  1'`, então imprime `1`, `PGHOST=127.0.0.1` com porta local, e o túnel fecha ao fim.
- [ ] V4.30 — Dado `acme-ssh-interno-aprov` (livre, com salto `acme-ssh-salto` com
  aprovação) sem app, quando `ward ssh acme-ssh-interno-aprov hostname`, então nega
  com "aprovação indisponível" (vale o nível mais restritivo, D19).
- [ ] V4.31 — Dado `ward ssh acme-ssh-senha-stage` interativo num pty (`expect`), quando
  digita `whoami` e `exit`, então vê `deploy` e a CLI sai 0.
- [ ] V4.32 — (SEG) Dado `ward ssh --token-fd <n> acme-pg-aprovacao` com um token
  inventado entregue pelo socket herdado no fd `<n>` (o token nunca vai em argv; D3,
  Fase 27a), quando roda sem app, então o token é ignorado e vale o nível ("aprovação
  indisponível"; com o app de teste que nega, "pedido negado") (D3).
- [ ] V4.33 — (SEG) Dada a suíte do M4 com `RUST_LOG=trace`, quando o auditor procura
  segredos nos logs do daemon e da CLI, então não acha — cobre S13.
- [ ] V4.34 — (SEG) Dado `ward ssh acme-ssh-senha-stage` interativo num pty (`expect`),
  quando o servidor imprime um prompt que termina num prefixo da senha (ex.: `fals`) e
  fica parado, então em até 1 s o pty mostra `••••` no lugar do prefixo, nunca o
  prefixo em claro; e quando em seguida chega o resto da senha, então nenhum pedaço
  dela aparece; com saída que diverge do segredo, o resto sai normal (D34) — cobre S4.
- [ ] V4.35 — (SEG) Dado `strace -f -e trace=execve` de `ward ssh acme-ssh-senha-stage
  whoami` e de `ward ssh acme-ssh-interno whoami` (com salto), quando o auditor lê o argv
  do `ssh`, então o usuário vem só como argumento próprio depois de `-l`, o host vem
  depois de `--`, não há `usuário@host` nem `-o User=`, nenhum `sh -c` aparece na cadeia
  e o `ProxyJump` traz só usuário e host validados (D43, D13); o mesmo com uma entrada
  de usuário `deploy@cliente.com.br`, que chega inteiro depois de `-l` — cobre S3.
- [ ] V4.36 — Dado `ward run acme-pg-dev -- true` com uma variável de 800 KiB no
  ambiente, quando roda, então a CLI sai ≠ 0 com a mensagem do limite de 750 KiB, sem
  valor, e nada é executado (D52).
- [ ] V4.37 — (SEG) Dado `strace -f -e trace=execve` de uma entrega por `sftp` de
  `beta-sftp-dev` e de uma entrada `ftp/sftp` de teste com usuário
  `deploy@cliente.com.br`, quando o auditor lê o argv do `sftp` (e do `scp`, se a Fase
  27 o usar), então não há `-l`, o usuário vem como `-o` seguido de
  `User=<usuário>` inteiro num só argumento, há `-F /dev/null`, o host vem depois de
  `--`, sem `usuário@host` nem `sh -c`; e `sftpuser` entra no SSH de teste (2222) por
  esse argv; no `ssh` segue o V4.35 (D56, D43) — cobre S3.
- [ ] V4.38 — (SEG) Dado o contêiner com `ward-app` e `ward-nm` de teste setgid e o
  daemon destravado, quando o agente roda `ward run <entrada livre> --
  /usr/local/bin/ward-app` com o ambiente de D57 copiado igual (e o mesmo com o
  `ward-nm`), então o filho conecta como agente, `revelar` responde "não autorizado" e
  nenhum segredo sai na saída mascarada (D63) — cobre S1.

## Milestone 5 — App desktop

Cada asserção (UI) cita o frame do Layout; o id do FigCraft está em
`docs/mission/design-nodes.md`.
O QA usa o Chrome em modo de teste (D7) e confere a aparência contra o frame.

- [ ] V5.1 — (SEG) Dado o app de teste com devtools, quando o auditor chama
  `__TAURI__.core.invoke('plugin:fs|read_text_file', …)`,
  `invoke('plugin:shell|execute', …)`, `invoke('plugin:http|fetch', …)` e um comando
  inventado, então todos são negados; o arquivo de capabilities lista só os comandos
  próprios do app — cobre S11.
- [ ] V5.2 — (SEG) Dado o app, quando o auditor injeta `<img
  src="https://example.com/x">`, `<script src="https://example.com/x.js">` e
  `fetch('https://example.com')`, então a CSP bloqueia os três e nenhuma conexão sai
  (sem requisição no log de rede); `grep -rE 'https?://' dist/` só acha os namespaces
  XML que o Svelte embute (`http://www.w3.org/2000/svg`, `http://www.w3.org/1999/xhtml`,
  e outros `http://www.w3.org/…` de namespace, se houver), que são nomes e não
  endereços; qualquer outra URL reprova, como confere o `dev/testes/app-dist.py`
  (handoff 29) — cobre S11.
- [ ] V5.3 — (SEG)(UI) Dado o app destravado, quando o QA passa por todas as telas e
  abre todas as entradas sem clicar em "revelar", então `outerHTML`, os `value` dos
  inputs, `localStorage`, `sessionStorage`, IndexedDB e as respostas da ponte não
  contêm segredo; depois de revelar, passados 30 s ou ao sair da tela, o valor sai do
  DOM — cobre S11 (D4).
- [ ] V5.4 — (SEG) Dado "copiar" na senha de `acme-pg-dev`, quando o auditor registra a
  IPC/ponte, então nenhuma resposta ao frontend contém o valor, e `wl-paste` devolve o
  valor — cobre S11 (D4).
- [ ] V5.5 — (SEG) Dado "copiar", quando passam 30 s, então `wl-paste` não devolve mais o
  valor; quando o usuário copia outro texto antes dos 30 s, então esse texto continua lá
  — cobre S12.
- [ ] V5.6 — (SEG) Dado o build de release, quando o auditor procura, então
  `strings` nos binários não acha a ponte nem `WARD_DEV_DIR`, e `WARD_DEV_DIR=.dev
  wardd` ignora a variável (D7) — cobre S11.
- [ ] V5.7 — (UI) `Primeiro uso / *`: cria o cofre, senha mestra com confirmação, PIN
  com 3 ou 7 dígitos e com letras recusados; Tab segue a ordem visual e Enter avança.
- [ ] V5.8 — (UI) `Destravar / Padrão`, `/ Senha errada` e `/ Pedido do agente`: senha
  errada mostra o erro sem limpar o foco; com um `ward run` travado esperando, a tela
  mostra entrada, comando e contagem; destravar libera o pedido.
- [ ] V5.9 — (UI) `Credenciais / Vazia`, `/ Cheia` e `/ Filtrada`: agrupada por grupo
  (acme, beta, delta, gama), filtros de tipo e ambiente e busca combinados; setas
  navegam, Enter abre, a busca tem atalho de teclado.
- [ ] V5.10 — (UI) `Formulário / <tipo> / Novo` e `/ Edição` dos 9 tipos: campos do
  tipo, `grupo` com autocompletar, `ambiente` obrigatório, extras com "segredo
  sim/não"; salvar sem obrigatório mostra o erro no campo.
- [ ] V5.11 — (UI) `Campo secreto`: revelar mostra o valor, oculta de novo em 30 s ou ao
  sair; copiar mostra o `Toast` de sucesso; nenhum segredo aparece nos frames sem
  revelar.
- [ ] V5.12 — (UI)(SEG) Anexos em `servidor` e `genérico`: adicionar, listar com nome e
  tamanho, remover; o conteúdo do anexo nunca vai para o DOM; escolher um arquivo de
  300 KiB (ou passar de 256 KiB somados) avisa antes de salvar, e o arquivo não é
  enviado; arquivo vazio é recusado; a recusa do orçamento da máscara aparece no campo
  (D29) — cobre S11.
- [ ] V5.13 — (UI) TOTP em `acme-wp-dev`: o código bate com `oathtool` e a contagem
  reinicia a cada 30 s; a semente só aparece com revelar; colar um `otpauth://` com
  `algorithm=SHA256`, `digits=8` ou `period=60` é recusado ao salvar com "parâmetros de
  TOTP não suportados" (D35).
- [ ] V5.14 — (UI) `Gerador de senha / No formulário`: gera, respeita tamanho e
  conjuntos, e preenche o campo de senha.
- [ ] V5.15 — (UI)(SEG) `Nível de acesso / *`: mudar o nível pede PIN; PIN errado mostra o
  erro e o nível não muda; o 5º erro seguido leva ao `Destravar / Padrão` com o cofre
  travado; os três níveis aparecem também em `prod` — cobre S9.
- [ ] V5.16 — (UI)(SEG) `Aprovação / Normal`: `ward run acme-pg-aprovacao -- touch
  .dev/marca` abre a janela com entrada, ambiente, comando exato e cadeia de processos.
  Negar → a CLI sai ≠ 0 e a marca não existe. Fechar → igual. Sem resposta 120 s →
  `Aprovação / Expirando` nos últimos segundos e depois negado. "Permitir uma vez" com
  PIN 135790 → executa; o pedido seguinte abre a janela de novo. "Permitir por 1 hora"
  → o segundo pedido não abre janela; `ward travar` e destravar → abre de novo.
  "Permitir sempre" → a entrada passa a "livre" no formulário — cobre S10.
- [ ] V5.17 — (UI) `Aprovação / prod`: com `acme-pg-prod`, o ambiente aparece em destaque
  e os quatro botões estão lá, inclusive "Permitir sempre" (R16).
- [ ] V5.18 — (UI)(SEG) `Aprovação / PIN errado` e `/ PIN bloqueado`: PIN errado mostra
  erro e o pedido continua pendente; o 5º erro trava o cofre, nega o pedido e mostra
  bloqueado — cobre S9.
- [ ] V5.19 — (UI)(SEG) Na janela de aprovação o foco inicial está em "Negar", Enter sem
  PIN não permite nada, e Esc nega — cobre S10.
- [ ] V5.20 — (SEG) Dado um pedido pendente, quando o cliente Python do papel agente
  manda "responder aprovação: permitir uma vez" com o PIN certo, então recebe "não
  autorizado" e o pedido continua pendente — cobre S10.
- [ ] V5.21 — (UI) `Gerador de senha / Avulso`: tamanho, conjuntos, sem símbolos, gerar
  de novo e copiar (o copiar respeita V5.5); os símbolos mostrados são os de D35, sem
  espaço, aspas, crase e `\`; o tamanho vai de 8 a 64 e o comando recusa 7 e 65, no
  avulso e no formulário; a força segue os limiares de D65.
- [ ] V5.22 — (UI) `Configurações / Geral`: trocar PIN pede o PIN atual; trocar senha
  mestra pede a atual; tempo de trava padrão 4 h e alterável; backup externo aparece
  desligado.
- [ ] V5.23 — (UI) `Bandeja / Destravado` e `/ Travado`: "Travar agora" trava (o app vai
  para destravar e `ward lista` responde "travado"); "Abrir" mostra a janela.
- [ ] V5.24 — (UI)(SEG) "Conectar" em `Formulário / servidor / Edição` de
  `acme-ssh-salto` (com aprovação): abre o terminal já conectado, sem janela nem PIN.
  Ataques: reusar o token visto em `/proc/<pid>/cmdline` → nega; usar depois de 30 s →
  vale o nível; usar o token de `acme-ssh-salto` em `acme-pg-aprovacao` → vale o nível
  (D3).
- [ ] V5.25 — (UI) Teclado e foco em todas as telas: foco visível, Tab na ordem visual,
  Esc fecha modal, nenhum trecho só alcançável por mouse.
- [ ] V5.26 — (UI) Nenhum texto em inglês na interface; só tema escuro: com o sistema em
  tema claro, o app continua escuro, e nenhuma tela (inclusive Configurações) oferece
  alternância de tema (D1).
- [ ] V5.27 — (SEG) Dado o app em uso pela suíte do M5, quando o auditor procura segredos
  nos logs do app, do daemon e no console da webview, então não acha — cobre S13.
- [ ] V5.28 — `docs/mission/roteiro-usuario.md` traz os passos da janela nativa (app
  abre, janela de aprovação aparece em primeiro plano no COSMIC, bandeja funciona,
  copiar limpa em 30 s) e o usuário confirmou.
- [ ] V5.29 — (UI)(SEG) Dado `ward run acme-pg-aprovacao -- sh -c` com argumento que
  traz U+202E, U+2066, largura zero, `\x1b[2J` e `\n` falso imitando outro comando,
  e com um processo pai de nome `claude\u202E` na cadeia, quando a janela abre, então
  cada caractere aparece como escape visível, cada argumento separado e entre aspas,
  nada é reordenado nem apagado, e o que executa é o comando exato (D51) — cobre S10.
- [ ] V5.30 — (SEG) Dado o app (RA7), quando o auditor roda `cargo deny check` e
  `cargo audit` na raiz, então nenhum advisory é ignorado; em `app/src-tauri/`, os
  único ignorado é RUSTSEC-2024-0370, com motivo, mais a RUSTSEC-2024-0429 só se o
  `cargo audit` em `app/src-tauri/` ainda a detectar (D64); e
  `grep -rn 'Variant' app/` não acha uso de `glib::Variant` — cobre S14.
- [ ] V5.31 — (SEG) Dado o `ward-app` de teste instalado setgid no contêiner (Fase 29a),
  quando o lançador do daemon (D57) o abre, então ele conecta ao socket antes do GTK,
  `prctl(PR_GET_DUMPABLE)` dentro do processo (impresso pelo app de teste) dá ≠ 1 e o
  `status` mostra `TracerPid` 0 desde o `exec` (dumpable 2 com `suid_dumpable=2`,
  D60), e depois da partida o processo tem o gid real, dumpable 0 e `TracerPid` 0
  (D64); o ambiente do processo tem só as variáveis de D57, com os valores
  fixos (um `GTK_MODULES`, `GST_PLUGIN_PATH` ou `GIO_EXTRA_MODULES` posto pelo
  atacante não chega ao WebKitGTK; não há `DISPLAY` nem `LISTEN_*`); o fd do socket
  tem `CLOEXEC`. Ataques, cada um tem de falhar em dar o papel app:
  - lançado por um processo qualquer do uid de teste, ele não recebe o papel e sai;
  - lançado por um processo que imita o gerente (`Name: systemd`, `cmdline`
    `/usr/lib/systemd/systemd --user`, `PPid: 1`, no `init.scope` de
    `user@<uid>.service`), como uma unit de `~/.config/systemd/user/` faria, ele não
    recebe o papel e sai;
  - aberto pelo daemon, mas com `WAYLAND_DISPLAY=/tmp/proxy.sock`,
    `WAYLAND_DISPLAY=../x`, `DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus` ou
    `XDG_RUNTIME_DIR=/tmp/x` postos à força no ambiente (teste do conferidor do app),
    então o app não usa o papel e sai com mensagem;
  - com dois sockets `wayland-<n>` em `/run/user/<uid>`, ou um link, então o lançador
    não abre o app e responde "aprovação indisponível" (D21);
  - um `systemctl --user set-environment WAYLAND_DISPLAY=<proxy>` (ou o mesmo no
    ambiente do pedido `abrir_app`) não muda o ambiente do app aberto pelo daemon
  (D47, D48, D57) — cobre S1, S8.
- [ ] V5.32 — (SEG) Dado o `ward-app` com o ambiente de D57 sob `strace -f` (WebKitGTK
  real), quando abre e desenha a janela, então o ambiente tem
  `GST_PLUGIN_SYSTEM_PATH`, `NO_AT_BRIDGE=1`, `WEBKIT_A11Y_BUS_ADDRESS` e
  `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps`; nenhum processo do app faz `open` nem
  `connect` em `/tmp/nvidia-mps`; com um listener do atacante em
  `/tmp/nvidia-mps/control`, ninguém conecta nele; um plugin-marca em
  `~/.local/share/gstreamer-1.0/plugins` não carrega; e a lista de `open`/`connect` em
  `/tmp`, `/var/tmp`, `/dev/shm` e `/run/user/<uid>` (fora do `wayland-<n>` e do `bus`)
  sai vazia ou cada item está registrado com decisão (D64) — cobre S8.

- [ ] V5.33 — (SEG) Dado o app de teste contra um daemon falso que intercala avisos,
  quando o daemon manda `pedido_de_aprovacao` e `pedido_de_destrave` entre um pedido e a
  resposta, então a resposta chega a quem pediu e os avisos vão para a fila, nunca no
  lugar dela; quando manda 1000 avisos seguidos, então a fila não passa de 16, o
  excedente é descartado com log sem conteúdo e o app continua respondendo; quando manda
  uma resposta sem pedido, um quadro acima do limite ou um tipo do papel agente, então o
  app fecha a conexão sem mostrar nada (D65, D47) — cobre S1, S11.
- [ ] V5.34 — (UI) Rodapé do detalhe em `Credenciais / Cheia`: "Criada em … · Alterada em
  …" de uma entrada criada pelo app, com as datas do daemon; uma entrada sem datas não
  mostra o texto (D65).
- [ ] V5.35 — (UI) Design (Fase 37a): no `Ward.figcraft`, a descrição de "Símbolos" em
  `Gerador de senha / Avulso` e no popover do formulário é o conjunto de D35; existem o
  frame `Credenciais / Sem resultado` e o cartão de nível bloqueado do detalhe, com
  instâncias dos componentes e tokens como variables; as contagens batem com a fixture
  (D65).

## Milestone 6 — Extensão, `ward login` e `ward playwright`

- [ ] V6.1 — (SEG) Dado `ward-nm` executado direto por um processo do usuário (`printf
  '<mensagem preencher acme-wp-dev origem http://localhost:8081>' | ward-nm
  chrome-extension://<id>/`), quando pede preencher, então o daemon recusa (pai não é
  o Chrome) e nada de segredo sai no stdout — cobre S1 (D5).
- [ ] V6.2 — (SEG)(UI) Dada a aba em `http://127.0.0.1:8081/wp-login.php` (origem
  diferente), quando o QA abre o popup, então `acme-wp-*` não é sugerida; quando o
  content script força a mensagem de preencher para essa aba, então o daemon recusa —
  cobre S1 (D9).
- [ ] V6.3 — (SEG) Dada a aba ativa no phishing `http://localhost:8082`, quando `ward
  login acme-wp-prod`, então sai ≠ 0 com "domínio não confere" e `posts.log` fica vazio.
- [ ] V6.4 — (SEG) Dada uma página da origem certa com um iframe do phishing, quando
  preenche, então só os campos do documento da origem certa recebem valor e o iframe
  não recebe nada.
- [ ] V6.5 — (SEG) Dado o uso da extensão na suíte, quando o auditor inspeciona
  `chrome.storage`, o DOM do popup e as mensagens trocadas, então segredo só aparece na
  resposta de preencher para a origem certa, e nunca fica guardado — cobre S1.
- [ ] V6.6 — (SEG) Dada uma página qualquer, quando chama
  `chrome.runtime.sendMessage('<id>', …)` ou faz `postMessage` para o content script
  pedindo preencher, então nada acontece (sem `externally_connectable`, mensagens da
  página ignoradas).
- [ ] V6.7 — (UI) `Extensão / Sugestões do domínio`: em `localhost:8081` aparecem
  `acme-wp-dev` e `acme-wp-prod`; clicar em `acme-wp-prod` preenche usuário e senha; em
  `localhost:8083` com campo de código, `acme-totp-dev` (D28) preenche usuário, senha e
  TOTP, e o login entra.
- [ ] V6.8 — (UI) `Extensão / Salvar login novo`: enviar um login novo em
  `localhost:8083` oferece salvar, pede tipo, grupo e ambiente, e a entrada aparece no
  `ward lista` com nível "com aprovação".
- [ ] V6.9 — (UI) `Extensão / Cofre travado`: com o cofre travado, o popup mostra o
  estado e o botão abre o app no destravar. Com o Chrome e o sistema em tema claro, os
  três estados do popup continuam escuros e não há alternância de tema (D1).
- [ ] V6.10 — Dada a aba em `localhost:8081/wp-login.php`, quando `ward login
  acme-wp-prod`, então a aba chega ao painel do WordPress logado; numa tentativa com
  senha recusada pelo site, os campos ficam vazios depois do envio.
- [ ] V6.11 — (UI) Dado `ward login acme-wp-dev` (com aprovação), então abre a janela de
  aprovação; negar → nada é preenchido.
- [ ] V6.12 — Dado `ward playwright acme-wp-prod`, então imprime um caminho em
  `<rt>playwright/acme-wp-prod.json` (0600), e um teste Playwright em Python com esse
  `storage_state` abre `/wp-admin/` já logado.
- [ ] V6.13 — (SEG) Dado o storage state e o perfil temporário, quando o auditor procura a
  senha de `acme-wp-prod` e suas variações, então não acha; o perfil some ao fim e
  `ward travar` apaga os storage states (D6) — cobre S2.
- [ ] V6.14 — (SEG) Durante `ward playwright`, `ss -ltnp` não mostra o Chrome escutando
  em porta, o Chrome tem `--remote-debugging-pipe`, e nenhum argv tem segredo — cobre
  S3.
- [ ] V6.15 — (SEG) Dado `ward playwright delta-redir` e `ward login delta-redir` (8084
  redireciona para 8082), então os dois recusam por origem e `posts.log` fica vazio
  (D9).
- [ ] V6.16 — (SEG) Dada a suíte do M6, quando o auditor procura segredos nos logs do
  `ward-nm`, do daemon e no console da extensão, então não acha — cobre S13.

## Milestone 7 — Backup

- [x] V7.1 — Dado o cofre de teste, quando faz 31 gravações, então existem 30 arquivos
  `ward-AAAAMMDD-HHMMSS.ward` em `.dev/backups/` e o mais antigo saiu.
- [x] V7.2 — (SEG) Dado um backup, quando abre com senha errada ou com 1 byte alterado
  (cada posição), então falha; `grep -a` não acha segredo — cobre S6.
- [x] V7.3 — (SEG) Dada a gravação do backup morta com SIGKILL em 100 pontos, então os
  backups anteriores continuam abrindo — cobre S7.
- [x] V7.4 — Dado um cofre novo, quando um pendrive de teste (imagem em loop montada via
  `udisksctl`) é conectado, então nada é copiado (externo desligado por padrão).
- [x] V7.5 — Dado o externo ligado com o UUID do pendrive A, quando A é conectado, então
  as versões vão para `<montagem>/ward-backup/`; quando o pendrive B é conectado, então
  nada é copiado.
- [x] V7.6 — (SEG) Dados os arquivos escritos no pendrive, quando o auditor procura
  segredos, então não acha — cobre S2.
- [x] V7.7 — (UI) `Restaurar backup / Versões` e `/ Confirmar`: escolhe uma versão, pede
  a senha daquela versão, o atual vira backup e as entradas da versão aparecem.
- [x] V7.8 — (SEG) Dado o papel agente, quando pede restaurar ou ligar o backup
  externo pelo socket, então "não autorizado".
- [x] V7.9 — (UI) `Configurações / Backup externo ligado`: escolher o pendrive, ligar e
  desligar.
- [x] V7.10 — (UI)(SEG) `Kit de emergência / Impressão`: mostra onde estão o cofre, os
  backups e o pendrive e a linha em branco para a senha; imprimir em PDF e procurar
  segredos e a senha mestra no PDF e no DOM não acha nada.
- [x] V7.11 — (SEG) Dado o cofre de teste na geração n+2 e o backup da geração n, quando
  o usuário restaura esse backup pelo app, então o cofre restaurado tem geração n+3, o
  arquivo de estado também, o daemon destrava com ele e as entradas são as da versão
  n; quando o mesmo backup é copiado à mão por cima do arquivo, então cai no V3.22
  (D39, D17) — cobre S7, S9.

## Milestone 8 — Skill `ward`

- [x] V8.1 — `skill/ward/SKILL.md` usa só `ward lista/run/ssh/login/playwright/travar`,
  segue "listar, cruzar grupo, ambiente e tipo com a tarefa, avisar qual entrada vai
  usar", e não cita KeePassXC nem `cofre-*`.
- [x] V8.2 — (SEG) O trecho de deny para `~/.claude/settings.json` cobre
  `~/.local/share/ward/**`, `~/.config/ward/**`, `/run/user/*/ward/**` e
  `/media/**/ward-backup/**`, e a skill manda nunca ler esses caminhos nem imprimir
  variável de credencial ou conteúdo de anexo, em nenhuma codificação (base64, hex),
  porque a máscara não cobre essas formas (RA3).
- [x] V8.3 — Dados três pedidos de exemplo ("conecta no banco de teste do acme", "entra
  no servidor de stage do acme", "faz login no WordPress de produção"), quando o QA
  segue a skill com o cofre de teste, então ela escolhe `acme-pg-dev`,
  `acme-ssh-senha-stage` e `acme-wp-prod`, avisa antes de usar e usa o comando certo.
- [x] V8.4 — A skill diz que SSH com chave do `~/.ssh` (ex.: `git push`) é feito direto,
  sem o Ward (R18).

## Milestone 9 — Instalação

- [ ] V9.1 — Dado `make install DESTDIR=<staging>`, então a árvore tem exatamente os
  caminhos de D18, e instalada com `sudo` todo arquivo é root:root sem escrita para
  grupo e outros.
- [ ] V9.2 — (SEG) Dado um artefato com `modo-teste`, quando `make install`, então recusa
  (D7).
- [ ] V9.3 — (SEG) Dado o `wardd` instalado, quando um `wardd` copiado para `~/bin`
  sobe e escuta onde puder (`/run/user/<uid>/`, `~/.local/`, um caminho em
  `WARD_SOCKET`), então `ward destravar` recusa antes de pedir a senha, porque só aceita
  o socket em `/run/ward/<uid>/` com pasta do root e par de uid 0 (D46) — cobre S5.
- [ ] V9.4 — (SEG) Dadas as units instaladas (D46), quando o atacante roda `systemctl
  --user set-environment LD_PRELOAD=<lib>` e força o `wardd` a subir de novo (mata o
  processo e conecta), então o `wardd` do sistema não herda a variável
  (`/proc/<pid>/environ` lido como root no roteiro não a traz) e a lib não é carregada;
  e o `wardd` sobe com dumpable 0 e core 0 — cobre S8.
- [ ] V9.5 — (SEG) Dada a política do Chrome instalada, então a extensão aparece como
  instalada pela organização e não pode ser desativada nem removida, e carregar outra
  extensão com o mesmo ID pelo modo desenvolvedor é bloqueado (roteiro do usuário).
- [ ] V9.6 — (SEG) O manifesto de native messaging tem `allowed_origins` só com o ID da
  extensão e `path` para `/usr/local/lib/ward/ward-nm`, dono root.
- [ ] V9.7 — Dado o boot e o login do usuário, então `wardd@<uid>.socket` está ativo,
  `/run/ward/<uid>/` é root:root 0755 com `wardd.sock` 0600 do usuário, a primeira
  conexão sobe o `wardd` como o usuário, `/run/user/<uid>/ward/` (temporários) é 0700, e
  o uso diário não pede `sudo` (roteiro do usuário; D46).
- [ ] V9.8 — (SEG) Depois de instalar, `~/.ssh`, `/etc/ssh/ssh_config*` e o
  `SSH_AUTH_SOCK` de `systemctl --user show-environment` estão como antes, e `git push`
  com a chave pessoal funciona como antes (R19; roteiro do usuário).
- [ ] V9.9 — O `.deb` tem a mesma lista de arquivos do staging (`dpkg -c`) e declara a
  dependência de `sshpass`; instala e desinstala num contêiner `ubuntu:24.04`.
- [ ] V9.10 — `sudo make uninstall` remove os arquivos instalados e não toca em
  `~/.local/share/ward/`.
- [ ] V9.11 — (SEG) Dado `make install` num contêiner `ubuntu:24.04`, então `ward-app`
  e `ward-nm` são grupos de sistema (gid < 1000), fora das faixas do `/etc/subgid`,
  sem membros, com `!` no `gshadow`; quando o grupo já existe com membro, com senha no
  `gshadow` ou com gid ≥ 1000, então a instalação recusa e não instala o binário
  setgid; quando um usuário sem o grupo tenta `newgrp ward-app`, então falha; e as
  units do sistema não têm `NoNewPrivileges=`, `RestrictSUIDSGID=` nem opção que os
  implique (`systemd-analyze security` e `systemctl show -p NoNewPrivileges` dão `no`),
  e o app aberto pelo daemon recebe o papel (D57, D60) — cobre S1, S8.

## Milestone 10 — Migração e saída do KeePassXC

- [ ] V10.1 — `docs/migracao.md` traz, nesta ordem: cadastrar as credenciais à mão no
  Ward; mover as chaves SSH pessoais do KeePassXC para o `~/.ssh` (agente do sistema)
  e as de servidor para entradas `servidor`; conferir com `ward ssh` cada servidor;
  devolver o chaveiro ao gnome-keyring, inclusive "Chrome Safe Storage"; desfazer cada
  item de `~/.claude/skills/cofre/references/montagem.md` (override do keyring,
  ativação D-Bus, autostart, PAM), um por um; conferir as senhas do Chrome; instalar a
  skill `ward` e remover a `cofre` e os scripts antigos; só então desinstalar o
  KeePassXC.
- [ ] V10.2 — (SEG) Nenhum passo do guia pede para colar segredo na conversa com o
  Claude, e todo passo que toca `~/.ssh` ou o chaveiro é executado pelo usuário, não
  pela missão (R19).
- [ ] V10.3 — O guia tem, para cada passo, como conferir que deu certo e como voltar
  atrás.
- [ ] V10.4 — O usuário executou o guia e confirmou que o Chrome manteve as senhas antes
  de desinstalar o KeePassXC (registro em `plan.md`).
