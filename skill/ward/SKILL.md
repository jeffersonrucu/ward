---
name: ward
description: Usa credenciais de trabalho do cofre Ward (CLI `ward`) sem ver o segredo, para login em site, senha, chave SSH, token de API, AWS, SMTP, SFTP ou banco de dados (Postgres e outros) de clientes e servidores. Use sempre que uma tarefa precisar de login, senha, usuário e senha, SSH, servidor, token, credencial, banco ou sessão logada para teste Playwright, mesmo que o usuário não diga qual entrada usar ("conecta no banco de teste do acme", "entra no servidor de stage", "faz login no WordPress de produção", "roda isso no servidor do cliente"). Use também quando um comando `ward` falhar (travado, negado, expirado, entrada inexistente).
---

# Ward: credenciais de trabalho sem ver o segredo

O Ward guarda as credenciais de trabalho (dos clientes e servidores) e faz o login por
você. **Você recebe o acesso, nunca a credencial.** O daemon `wardd` entrega a senha,
o token ou a chave direto à ferramenta, e a saída que volta tem os segredos trocados
por `••••`. Nenhum comando devolve segredo; revelar e copiar só existem no app, para o
usuário.

Contas pessoais do usuário (GitHub pessoal, chaves do `~/.ssh`) **não** passam pelo
Ward. Veja "SSH com chave do `~/.ssh`" no fim.

## Fluxo (sempre nesta ordem)

1. **Listar.** Rode `ward lista --json` (ou `ward lista` para a tabela). Cada entrada
   traz `nome`, `tipo`, `grupo`, `ambiente` (`dev`, `stage` ou `prod`), `projeto` e
   `endereco` (URL ou host). Nunca traz segredo. Entradas bloqueadas não aparecem.
2. **Cruzar com a tarefa.** Escolha pela combinação de:
   - **grupo**: o cliente ou a pasta citada (`acme`, `beta`, `clientex`…);
   - **ambiente**: "teste"/"local"/"desenvolvimento" → `dev`; "homologação"/"stage" →
     `stage`; "produção" → `prod`;
   - **tipo**: banco → `banco`; servidor/SSH → `servidor`; site/painel → `login`;
     token/API → `api`; AWS → `aws`; e-mail → `smtp`; SFTP/FTP → `ftp-sftp` (na tela, "ftp/sftp").

   Se sobrar mais de uma candidata, ou nenhuma, **pergunte ao usuário** em vez de
   chutar. Nunca troque de ambiente por conta própria (não use `prod` quando o pedido
   é `dev`, nem o contrário).
3. **Avisar antes de usar.** Diga numa linha qual entrada vai usar e para quê, por
   exemplo: "Vou usar `acme-pg-dev` (banco, acme, dev) para rodar a consulta."
4. **Usar o comando certo** (tabela abaixo) e trabalhar normalmente com o resultado.

### Exemplos (cofre de teste, tudo falso)

| Pedido | Entrada | Comando |
| --- | --- | --- |
| "conecta no banco de teste do acme" | `acme-pg-dev` (banco, acme, dev) | `ward run acme-pg-dev -- /usr/bin/psql -tAc 'select current_user'` |
| "entra no servidor de stage do acme" | `acme-ssh-senha-stage` (servidor, acme, stage) | `ward ssh acme-ssh-senha-stage -- uname -a` |
| "faz login no WordPress de produção" | `acme-wp-prod` (login, acme, prod) | `ward login acme-wp-prod` (aba já aberta no site) ou `ward playwright acme-wp-prod` (teste) |

## Comandos

| Comando | Para quê |
| --- | --- |
| `ward lista [--json]` | ver as entradas que você pode usar |
| `ward run <entrada> -- <comando> [args…]` | rodar um comando local com a credencial no ambiente |
| `ward ssh <entrada> [--] <comando remoto…>` | rodar um comando no servidor da entrada (senha, chave e salto resolvidos pelo Ward) |
| `ward login <entrada>` | preencher e enviar o login na aba do Chrome, pela extensão |
| `ward playwright <entrada>` | fazer o login e receber o caminho de um storage state para o Playwright |
| `ward travar` | travar o cofre agora (quando o usuário pedir) |

### `ward run`

O comando roda com a credencial em variáveis de ambiente do processo filho. Use a
variável pelo nome **dentro do comando que precisa dela**, nunca para mostrar o valor.

```sh
ward run acme-pg-dev -- /usr/bin/psql -tAc 'select count(*) from pedidos'
ward run beta-github-api -- /usr/bin/curl -sf --variable %GITHUB_TOKEN --expand-header 'Authorization: Bearer {{GITHUB_TOKEN}}' https://api.github.com/user
ward run beta-aws-dev -- /usr/local/bin/aws sts get-caller-identity
ward run acme-pg-tunel -- /usr/bin/psql -tAc 'select 1'   # túnel SSH aberto pelo Ward
```

- Variáveis por tipo:
  - `banco` (postgres): `PGHOST`, `PGPORT`, `PGDATABASE`, `PGUSER`, `PGPASSWORD`,
    `DATABASE_URL`;
  - `login`: `WARD_URL`, `WARD_USER`, `WARD_PASS`, `WARD_TOTP` (código, nunca a
    semente);
  - `servidor`: `WARD_HOST`, `WARD_PORT`, `WARD_USER` e `WARD_PASS` ou
    `WARD_ARQUIVO_CHAVE` + `WARD_PASSPHRASE`;
  - `api`: `WARD_URL`, `WARD_TOKEN` e a variável escolhida na entrada (ex.:
    `GITHUB_TOKEN`);
  - `aws`: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`,
    `AWS_REGION`, `AWS_DEFAULT_REGION`, `AWS_ROLE_ARN`;
  - `smtp`: `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, `SMTP_SECURITY`;
  - `ftp-sftp`: `WARD_PROTOCOLO`, `WARD_HOST`, `WARD_PORT`, `WARD_USER`, `WARD_PASS`;
  - `genérico`: `WARD_<CAMPO>` de cada campo extra;
  - anexos: `WARD_ARQUIVO_<NOME>` com o **caminho** de um arquivo temporário 0600,
    apagado quando o comando termina;
  - todas: `WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`.
- `nota segura` não tem entrega: `ward run` sai com "tipo sem entrega". A nota é só
  para o usuário ler no app.
- **Ambiente limpo e `PATH` fixo.** O comando não herda o seu ambiente: roda com
  `PATH=/usr/local/bin:/usr/bin:/bin` e só `HOME`, `USER`, `LOGNAME`, `LANG`, `LC_*`,
  `TERM`, `TZ`, `COLORTERM`, `NO_COLOR` e as variáveis da credencial. Ferramenta
  instalada fora desse `PATH` (node/npm/npx do nvm, `~/.local/bin`, `~/.cargo/bin`,
  venv do projeto) vai pelo **caminho absoluto**:
  `ward run beta-github-api -- /home/<usuário>/.nvm/versions/node/<versão>/bin/node scripts/chamar-api.js`.
  Descubra o caminho com `command -v node` **fora** do `ward run`. Variável que o
  comando precisa (fora as da credencial) vai como argumento ou arquivo de
  configuração do projeto, não pelo ambiente.
- Para pôr o token num cabeçalho do `curl`, use `--variable %VAR` com
  `--expand-header` (curl 8.3 ou mais novo), como no exemplo acima: o `curl` lê a
  variável do próprio ambiente, sem shell e sem o valor no argv.
- Nada de redirecionamento (`>`, `>>`, `2>&1`, `tee`) nem de pós-processar a saída
  com `cut`, `tr`, `od`, `rev`, `fold` dentro do `ward run`: as regras de deny barram
  esses comandos. A saída e o stderr do comando já voltam para você, com a máscara.
- O diretório de trabalho é o seu. O código de saída é o do comando; `125` é erro do
  próprio Ward (veja "Quando der erro").
- `sh -c '…'` é permitido para encadear comandos, desde que nenhum deles imprima
  variável de credencial (veja "Nunca faça").

### `ward ssh`

```sh
ward ssh acme-ssh-senha-stage -- uname -a
ward ssh acme-ssh-chave-prod -- 'systemctl status nginx --no-pager'
ward ssh acme-ssh-interno -- hostname         # salto pelo servidor intermediário, automático
printf 'ls\n' | ward ssh beta-sftp-dev         # sftp: comandos pela entrada padrão
```

- **Sempre passe o comando remoto.** Sem comando, o `ward ssh` abre um shell
  interativo, que só funciona no terminal do usuário.
- O Ward resolve senha, chave e salto. Não passe `-i`, `-J`, `-o` nem porta: o host,
  o usuário e a porta vêm da entrada.
- A chave do servidor nunca vira arquivo seu e nunca vai para o `~/.ssh`.
- Numa entrada `ftp-sftp`, os comandos do sftp vão pela entrada padrão; o diretório
  local é o home do usuário, então use caminho local absoluto em `get`/`put`.
- Para um banco atrás de SSH, use a entrada `banco` com túnel (ex.: `acme-pg-tunel`)
  no `ward run`; o Ward abre o túnel e ajusta `PGHOST`/`PGPORT`/`DATABASE_URL`.

### `ward login`

```sh
ward login acme-wp-prod
```

- Precisa da aba ativa do Chrome **já na página de login** do site da entrada. A
  extensão confere o domínio da aba, preenche usuário, senha e código TOTP, envia o
  formulário e limpa os campos.
- "domínio não confere": a aba não está no site da entrada (pode ser phishing ou
  redirecionamento). **Pare e avise o usuário**; não tente outra aba nem outra
  entrada por conta própria.

### `ward playwright`

```sh
WARD_ESTADO="$(ward playwright acme-wp-prod)" /usr/bin/python3 tests/test_painel.py
```

- O Ward faz o login num Chrome headless e imprime **só o caminho** do storage state
  (um arquivo na pasta de execução do Ward). A senha nunca vai para o arquivo.
- Passe o caminho ao teste **por variável**, como no exemplo, e leia a variável no
  teste (`storage_state=os.environ["WARD_ESTADO"]` no Python,
  `storageState: process.env.WARD_ESTADO` no `browser.newContext` em JS). **Não
  escreva o caminho em comando nenhum**: as regras de deny barram todo comando que
  cita a pasta de execução do Ward.
- Não abra, não imprima e não copie o arquivo: ele tem os cookies da sessão. Travar o
  cofre apaga as sessões; peça de novo depois.

## Nunca faça

A máscara de saída cobre o valor puro e as variações óbvias (URL, base64 de uma
linha, JSON), mas **não cobre tudo** (RA3): base64 quebrado em linhas, hex, valor
recodificado ou cortado entre stdout e stderr passam. Por isso, mesmo sem má
intenção:

- **Nunca imprima variável de credencial, em nenhuma forma ou codificação.** Nada de
  `echo $PGPASSWORD`, `printf`, `env`, `printenv`, `set`, `export -p`, `declare -p`,
  `set -x`/`bash -x`/`sh -x` (que ecoam os comandos já expandidos), `cat
  /proc/self/environ` (ou de qualquer `/proc/<pid>/environ`), `base64`, `base32`,
  hex (`xxd`, `od`, `hexdump`), `rev`, `tr`, `cut`/`head -c` de pedaços, `jq` sobre o
  valor, `repr()`/`print(os.environ)` em Python, `console.log(process.env)` em Node,
  log de depuração da ferramenta que mostre a URL de conexão.
- **Nunca grave credencial em arquivo**: nada de `> arquivo`, `tee`, `.env` gerado,
  `~/.pgpass`, `~/.netrc`, config de cliente com a senha, `git config`, histórico.
- **Nunca imprima nem copie anexo.** O `WARD_ARQUIVO_<NOME>` é para a ferramenta ler
  (`ssh -i`, `--key-file`, `source` de um `.env` pelo programa). Nunca `cat`, `head`,
  `less`, `base64`, `xxd`, `cp`, `mv` nem `ln` do anexo.
- **Nunca leia os arquivos do Ward**: `~/.local/share/ward/**` (cofre e estado),
  `~/.config/ward/**`, `/run/user/*/ward/**` (socket, anexos temporários, sessões do
  Playwright), `/media/**/ward-backup/**` e `/run/media/**/ward-backup/**` (backup
  no pendrive). Também não liste, copie nem calcule hash deles.
- **Nunca leia `~/.ssh`** (chaves privadas, `config`). O `ssh` do sistema usa as
  chaves sozinho.
- Nunca peça, aceite ou repita a senha mestra ou o PIN. Se o usuário colar uma senha
  na conversa, avise que ela deve ser trocada e cadastrada no app do Ward.
- **Comando negado pela permissão do Claude Code** (regras de deny): não reescreva
  o comando de outro jeito para passar (outro nome de ferramenta, script, caminho
  com curinga). Diga ao usuário o que queria fazer e por quê, e espere.
- Nunca tente contornar o Ward: nada de procurar a credencial em outro lugar (código,
  `.env` do projeto, histórico, navegador) quando o Ward recusar.

## Quando der erro

| Mensagem | O que fazer |
| --- | --- |
| `travado` / `cofre travado: o comando espera o destrave por até 2 min` | O cofre está travado. Avise o usuário: "o Ward está travado; destrave pelo app ou no seu terminal com a senha mestra". O `ward run`/`ward ssh` espera até 2 min pelo destrave; se expirar, rode de novo depois que o usuário confirmar. **Você nunca destrava** (a senha mestra não passa por você). |
| `travado: tempo esgotado` | Passaram os 2 min sem destrave. Peça ao usuário para destravar e rode de novo. |
| Janela de aprovação aberta (o comando fica esperando) | A entrada é "com aprovação": o usuário vê a entrada, o ambiente e o comando exato e decide com o PIN. Avise "abri um pedido de aprovação para `<entrada>`" e espere (até 2 min). Não repita o pedido em paralelo. |
| `pedido negado` | O usuário negou (ou fechou a janela). **Não tente de novo nem por outro caminho**; pergunte o que ele quer fazer. |
| `pedido expirado` | Ninguém respondeu em 2 min. Avise e pergunte se deve pedir de novo. |
| `aprovação indisponível` | O app não está aberto para aprovar. Peça ao usuário para abrir o app do Ward. |
| `entrada inexistente` | O nome não existe **ou** a entrada está bloqueada para você (o Ward não diferencia). Rode `ward lista` de novo e confira o nome; se não houver outra adequada, pergunte ao usuário. Não tente adivinhar nomes. |
| `tipo sem entrega` | A entrada (ex.: nota segura) não tem credencial para entregar. Escolha outra ou pergunte. |
| `domínio não confere` / `a origem da página não bate com a da entrada` | A aba ou a página não é a do site da entrada. Pare e avise o usuário. |
| `a extensão do Ward no Chrome não respondeu (o Chrome está aberto?)` (`ward login`, código 1) | O Chrome não está aberto ou está sem a extensão do Ward. Peça ao usuário para abrir o Chrome com a extensão (e a aba na página de login) e rode `ward login` de novo depois que ele confirmar. Não tente outro caminho para o login. |
| `o daemon não está rodando` / `ainda não existe cofre` | O Ward não está no ar ou não foi criado. Avise o usuário; não procure a credencial em outro lugar. |
| código `125` no `ward run` | Erro do Ward, não do comando: leia a mensagem do `ward:` no stderr. |
| `127` no `ward run` | Comando não achado no `PATH` fixo: use o caminho absoluto. |

## SSH com chave do `~/.ssh` (fora do Ward)

O Ward **não** é o agente SSH da máquina. As chaves pessoais do usuário (GitHub etc.)
ficam no `~/.ssh` com o agente do sistema. Para elas, use direto, sem o Ward:

```sh
git push
git pull
ssh git@github.com
```

O Ward não intercepta nem bloqueia esse SSH. Use `ward ssh` só para os servidores de
trabalho que estão no `ward lista` (entradas `servidor`). Mesmo usando o `~/.ssh`
direto, nunca leia nem imprima os arquivos dele.
