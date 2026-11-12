# Fase 12 — Serviços de teste (`dev/compose.yml`), `make dev-servicos` e fixtures

## Implementado
- `dev/compose.yml` (projeto `ward-teste`) com `postgres` (55432), `ssh-senha` (2222),
  `ssh-chave` (2223), `ssh-salto` (2224), `ssh-interno` (sem porta), `mariadb`,
  `wordpress` (8081), `wpcli` (serviço que roda uma vez: instala o WordPress e cria
  `wp_admin`/`wp_editor`), `phish` (8082), `totp` (8083), `redir` (8084).
- SSH: uma imagem (`dev/servicos/ssh/`, alpine fixado por digest) com o modo em
  `WARD_MODO`:
  - `senha`: `deploy` por senha, com túnel só até `postgres:5432` (`PermitOpen`, para
    V4.29). `sftpuser` só sftp (`ForceCommand internal-sftp`, chroot, sem túnel, sem tty).
  - `chave`: `deploy` só por chave (`AuthenticationMethods publickey`).
  - `salto`: `salto` por senha; encaminha só até `ssh-interno:22`.
  - `interno`: `deploy` só por chave, só na rede `interno`.

  Em todos: `PermitRootLogin no`, `AllowUsers`, sem agent/X11/tunnel. As chaves
  autorizadas ficam em `/etc/ssh/autorizadas/%u`, fora da home.
- `phish`/`totp`/`redir`: um script Python só com a biblioteca padrão
  (`dev/servicos/web/servidor.py`). Roda como nobody, com raiz somente leitura.
  - `phish` imita a tela de login do WordPress (`log`/`pwd`, ids `user_login`/
    `user_pass`) e grava todo POST, em JSON por linha, em `/registro/posts.log` (0600,
    em tmpfs).
  - `totp` tem `/login` (usuário, senha e código `autocomplete=one-time-code`) e
    `/cadastro` (conta nova em memória, até 100 contas).
  - `redir` manda tudo (GET e POST) com 302 para `http://localhost:8082/wp-login.php`.
- `make dev-servicos` (`dev/servicos/gerar.py`) gera `.dev/servicos/` (0700):
  - senhas das contas tiradas das fixtures;
  - senhas internas da MariaDB aleatórias (`secrets`);
  - chave ed25519 do `deploy` com a passphrase de D15, digitada por um pty e não por
    argv;
  - chaves de host estáveis de cada SSH;
  - `known_hosts` próprio;
  - HOME falso dos clientes.

  É idempotente. `--trocar-chave-host <servico>` existe para V4.28.
- `dev/fixtures/entradas.json`: as 20 entradas da tabela D15 (senha mestra, PIN,
  `campos` não secretos, `segredos`, extras, anexos), mais
  `dev/fixtures/anexos/gama-generico/app.env`. É a fonte única: a Fase 19 deve montar
  o cofre a partir delas.
- `dev/servicos/conferir.sh` (`make dev-servicos-conferir`) confere todo o V1.4 (13
  checagens). `dev/servicos/comum.sh` tem os clientes de teste isolados:
  - `env -i` com HOME falso;
  - `ssh -F /dev/null` com `.dev/servicos/known_hosts`, `StrictHostKeyChecking=yes`,
    `IdentityAgent=none`, `IdentitiesOnly`;
  - senha e passphrase por `sshpass -f <arquivo>`;
  - `psql` da imagem do postgres com `-e PGPASSWORD` sem valor.
- Makefile: `dev-servicos`, `dev-servicos-subir`, `dev-servicos-parar` (`down -v`),
  `dev-servicos-conferir`, `dev-servicos-trocar-chave-host SERVICO=…` e
  `teste-ataques-servicos`. `CLAUDE.md` › "Comandos" atualizado (comandos e resumo dos
  serviços).

## Segurança
- Invariantes tocadas:
  - **S15**: as fixtures só têm valores de D15. O `make check` passa com a allowlist
    atual, sem valor novo. As senhas internas aleatórias e as chaves ficam só em
    `.dev/` (ignorado; teste abaixo).
  - **S3 (análogo, nos serviços)**: nenhuma senha em argv nem em `environment`. Os
    segredos entram como arquivos somente leitura em `/run/ward/`, por `*_FILE`, pelo
    stdin do `chpasswd`, ou pelo `--prompt` do wp-cli com stdin. A passphrase da chave
    vai por pty.
  - **S13 (análogo)**: o log de acesso dos sites só mostra método e caminho (sem query
    nem corpo). Nenhum segredo aparece nos `docker logs` (teste abaixo).
  - **R19**: nenhum arquivo de `dev/` cita pasta de chaves ou de credenciais do
    usuário, agente nem config do ssh. Os clientes rodam com HOME falso e `-F /dev/null`.
    `make teste-ataques-fundacao` (`v1_5`) segue passando com os arquivos novos.
  - Exposição na rede: todas as portas ficam presas em `127.0.0.1`.
  - Rede `interno` com `internal: true` + `com.docker.network.bridge.inhibit_ipv4`.
    Sem essa opção o host alcançava o `ssh-interno` pelo IP do contêiner (ataque
    abaixo, que pegou isso).
  - Estado em tmpfs; `no-new-privileges` em todos; imagens fixadas por digest; bind
    sem `create_host_path`, para que sem `make dev-servicos` o compose falhe em vez
    de subir com diretório vazio.
- Testes de ataque: `dev/testes/ataques-servicos.sh` (`make teste-ataques-servicos`,
  40 checagens, todas barradas, rodado 4 vezes). Funções do script:
  - `portas_so_loopback`: toda porta publicada é `127.0.0.1:*`, e nenhuma das 8 responde
    pelo IP de rede da máquina. Uma mutação temporária (`8084:8080` sem IP) foi pega e
    revertida.
  - `interno_inalcancavel`: conexão do host ao IP do contêiner `ssh-interno`:22 falha.
  - `autenticacao`:
    - 2223 só oferece `publickey` e 2222 só `password`;
    - a senha do deploy é recusada em 2223;
    - senha errada é recusada em 2222 e 2224;
    - `root` é recusado em 2222 e 2224;
    - a senha de outra conta é recusada.
  - `sftp_restrito`: o `sftpuser` não executa comando. `sshd -T` mostra
    `forcecommand internal-sftp`, chroot e `allowtcpforwarding no`. O túnel do
    `sftpuser` até o postgres é recusado.
  - `tuneis`: `deploy@2222` abre o canal até `postgres:5432` e não até `ssh-chave:22`.
    `salto` abre só `ssh-interno:22`, não `postgres:5432`. Prova positiva: resposta do
    destino pelo canal.
  - `chave_host`:
    - host desconhecido é recusado (sem TOFU);
    - depois de trocar a chave de host do `ssh-senha`, o known_hosts antigo é recusado
      com "REMOTE HOST IDENTIFICATION HAS CHANGED" (base de V4.28);
    - o known_hosts novo volta a entrar.
  - `wordpress`: senha errada e senha de outra conta não dão `wordpress_logged_in`.
  - `totp`: aceita senha e código certos. Recusa:
    - código errado, vazio ou de 5 min à frente;
    - senha errada com código certo;
    - senha de outra conta;
    - corpo de 20 KB (413).
  - `phish`: grava o POST, o `posts.log` é 0600, e o corpo acima do limite não é
    gravado. No fim o log é esvaziado para V6.3/V6.15. `redir` manda até POST para o
    8082.
  - `sem_segredo_exposto`: nenhum valor secreto das fixtures, nem as senhas internas
    aleatórias, aparece no `docker inspect`, no argv de algum processo dos 11
    contêineres, nos `docker logs` ou no `docker compose config`.
  - `permissoes`:
    - diretórios 0700;
    - chaves e senhas de SSH 0600;
    - nada gravável por grupo ou outros;
    - a chave do deploy não abre sem passphrase;
    - `.dev/servicos/` é ignorado pelo git.
  - `sem_arquivos_nao_sobe`: numa cópia sem `.dev/`, o `compose create` falha e não cria
    `.dev/` vazio.
- Dependências novas: nenhum crate. Imagens Docker de teste, fixadas por digest:
  - `alpine:3.22` (base do SSH, com os pacotes `openssh-server` e `openssh-sftp-server`
    do apk);
  - `python:3.13-alpine3.22` (sites falsos);
  - `wordpress:cli-2.12-php8.3` (instalação do WordPress).

  As imagens `postgres:16-alpine`, `mariadb:11.4` e `wordpress:6.8-php8.3-apache`
  já estavam na máquina. Motivo: são os serviços que V1.4 pede. Ferramentas do host
  usadas pelos scripts: `docker`, `sshpass`, `ssh`, `ssh-keygen`, `curl`, `python3`
  (sem `psql` nem `oathtool` no host: o psql vem da imagem e o TOTP do teste é
  calculado com a biblioteca padrão do Python).

## Ficou de fora
- `make dev-cofre`/`dev-variacoes`: são da Fase 19. As fixtures já estão no formato
  para ela ler.
- Nenhum `ward-*` foi usado: os serviços são só o alvo dos testes dos próximos
  milestones.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I -B dev/servicos/gerar.py` (`make dev-servicos`) | 0 |
| `docker compose -f dev/compose.yml up -d --build` | 0 |
| `make dev-servicos-parar` + `make dev-servicos-subir` (do zero) | 0 |
| `make dev-servicos-conferir` (V1.4, 13 checagens) | 0 |
| `make teste-ataques-servicos` (40 checagens; rodado 4×) | 0 |
| idem com a mutação `8084:8080` (tem de falhar) | 1 (2 achados, como esperado; revertido) |
| `make teste-ataques-fundacao` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **Nenhuma entrada de D15 aponta para `localhost:8083`**, mas V6.7 pede que a extensão
  preencha usuário, senha e TOTP ali. O serviço `totp` aceita as contas `wp_admin` e
  `wp_editor` com as senhas e a semente de `acme-wp-*`. Mesmo assim, a origem das
  entradas é 8081 e a extensão (D9) não vai sugerir nada em 8083. Saídas: acrescentar
  a D15 uma entrada `login` com URL `http://localhost:8083/login` (e valor novo na
  allowlist), ou o validador cadastrar uma no teste. Decisão do planner/orchestrator.
- O `ssh-interno` era alcançável do host pelo IP do contêiner, mesmo com
  `internal: true`. Foi corrigido com `inhibit_ipv4`, e o ataque cobre o caso.
  Registro porque o docker deste host (29.9.2) aceita a opção. Num docker antigo sem
  ela, o teste `interno_inalcancavel` falha (falha fechada).
- O teste `chave_host` troca de verdade a chave de host do `ssh-senha` (como V4.28
  pede). Quem guardou a chave antiga em outro known_hosts (o do Ward, no M4) vai ver o
  aviso, como esperado.
- Arquivos lidos por usuário não root dentro do contêiner (postgres, mariadb, www-data,
  nobody) precisam ser 0644. Ficam dentro de `.dev/servicos/segredos/` 0700, que é a
  barreira para os outros usuários da máquina. Os que só o root do contêiner lê (SSH)
  ficam 0600. O compose fora do Swarm não tem uid/modo por segredo.
- O TOTP do serviço `totp` e o do teste são RFC 6238 escritos sobre `hmac`/`hashlib` da
  biblioteca padrão. É código de serviço de teste, não do Ward. O TOTP do Ward continua
  na Fase 18 com biblioteca auditada.
- `docker compose -f dev/compose.yml up -d` sem `make dev-servicos` antes falha de
  propósito (bind sem `create_host_path`). O `CLAUDE.md` diz para rodar
  `make dev-servicos` antes ou usar `make dev-servicos-subir`.
- Os pacotes do apk na imagem SSH não têm versão fixa (só a base é fixada por digest).
  Para teste é aceitável. Se quiserem build reproduzível, fixar as versões no
  Dockerfile.
- O `make check` passou com 0 nesta rodada. Nada nos arquivos de evidência ou no
  `.gitleaks.toml` atrapalhou, e não mexi em nenhum dos dois.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/compose.yml` | criado |
| `dev/fixtures/entradas.json` | criado |
| `dev/fixtures/anexos/gama-generico/app.env` | criado |
| `dev/servicos/gerar.py` | criado |
| `dev/servicos/comum.sh` | criado |
| `dev/servicos/conferir.sh` | criado |
| `dev/servicos/ssh/Dockerfile` | criado |
| `dev/servicos/ssh/entrada.sh` | criado |
| `dev/servicos/web/servidor.py` | criado |
| `dev/servicos/wordpress/instalar.sh` | criado |
| `dev/testes/ataques-servicos.sh` | criado |
| `Makefile` | alterado (alvos de serviços; `dev-servicos` deixou de ser stub) |
| `CLAUDE.md` | alterado ("Comandos": serviços de teste) |
| `docs/mission/handoffs/phase-12.md` | criado |
| `.dev/servicos/**` | criado por `make dev-servicos` (fora do git) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (D15 + aleatórias em `.dev/`)
- [x] Teste de ataque para cada (SEG) tocado (V1.5 segue em `ataques-fundacao`; os
  ataques dos serviços estão em `ataques-servicos.sh`)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
