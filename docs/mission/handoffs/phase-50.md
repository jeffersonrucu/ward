# Fase 50 — Skill `ward` e trecho de deny

## Implementado
- `skill/ward/SKILL.md` (do zero):
  - frontmatter só com `name: ward` e uma `description` que dispara em login, senha,
    SSH, token, banco, AWS, SMTP, SFTP e Playwright, inclusive sem o nome da entrada;
  - o fluxo de V8.1 em 4 passos: listar (`ward lista --json`), cruzar grupo, ambiente
    e tipo com a tarefa (pergunta ao usuário se sobrar mais de uma candidata ou
    nenhuma; nunca troca de ambiente), avisar qual entrada vai usar, usar o comando
    certo;
  - tabela com os três pedidos de V8.3 → `acme-pg-dev`, `acme-ssh-senha-stage`,
    `acme-wp-prod` e o comando de cada um;
  - comandos: `ward lista [--json]`, `ward run` (variáveis por tipo, conforme a
    tabela "Entrega esperada"; anexos só por caminho; nota segura sem entrega; túnel
    pelo `banco` com túnel), `ward ssh` (sempre com comando remoto; salto automático;
    sftp pela entrada padrão; sem `-i`/`-J`/`-o`), `ward login`, `ward playwright`
    (só o caminho; não abrir o arquivo), `ward travar`;
  - **Fase 26a:** ambiente limpo e `PATH=/usr/local/bin:/usr/bin:/bin` no `ward run`;
    ferramenta do nvm, `~/.local/bin`, `~/.cargo/bin` ou venv vai por caminho
    absoluto (achado com `command -v` fora do `ward run`); `127` = comando fora do
    `PATH` fixo;
  - "Nunca faça" (V8.2, RA3): imprimir variável de credencial em qualquer forma
    (`echo`, `printf`, `env`, `printenv`, `set`, `export -p`, `declare -p`, `set -x`,
    `/proc/self/environ`, `base64`, `base32`, hex com `xxd`/`od`/`hexdump`, `rev`,
    `tr`, pedaços, `jq`, `repr`/`process.env`), gravar credencial em arquivo, imprimir
    ou copiar anexo, ler `~/.local/share/ward/**`, `~/.config/ward/**`,
    `/run/user/*/ward/**`, `/media/**/ward-backup/**` e `~/.ssh`, pedir senha mestra
    ou PIN, contornar o Ward;
  - "Quando der erro": `travado` (avisa e espera; nunca destrava), `travado: tempo
    esgotado`, aprovação aberta, `pedido negado` (não tenta de novo nem por outro
    caminho), `pedido expirado`, `aprovação indisponível`, `entrada inexistente`
    (inclui a bloqueada; não adivinha nomes), `tipo sem entrega`, domínio/origem
    diferente, daemon parado, códigos 125 e 127. Textos tirados de
    `crates/ward-core/src/protocolo/erros.rs` e `crates/ward/src/comandos.rs`;
  - V8.4: SSH com chave do `~/.ssh` (`git push`, `ssh git@github.com`) é direto, sem o
    Ward, e mesmo assim os arquivos do `~/.ssh` não são lidos.
  - Não cita `ward destravar` (V8.1 lista só lista/run/ssh/login/playwright/travar): a
    skill diz "destrave pelo app ou no seu terminal com a senha mestra".
- `skill/settings-deny.json`: só `permissions.deny`, para o usuário juntar ao
  `~/.claude/settings.json` no M10:
  - `Read` e `Edit` de `~/.local/share/ward/**`, `~/.config/ward/**`,
    `//run/user/*/ward/**`, `//media/**/ward-backup/**` (o `//` é o caminho absoluto
    na sintaxe do Claude Code) e `~/.ssh/**`;
  - `Bash` como cinto de segurança: comandos que citam esses caminhos, `*/environ*`,
    `*/.ssh/id_*`, `ward destravar*`, `ward ssh --token*` e, dentro de `ward run`,
    `env`, `printenv`, `export -p`, `declare -p`, `set -x`, `bash -x`, `sh -x`,
    `base64`, `base32`, `xxd`, `hexdump` e `cat` de `WARD_ARQUIVO_*`.
- `skill/README.md`: para o usuário; o que vai para onde, como juntar o deny sem
  substituir o `settings.json`, e que as regras `Bash` não são barreira.
- `dev/testes/skill.py` + `make teste-skill` (linha no `CLAUDE.md`):
  - frontmatter (só `name`/`description`, `name: ward`, tamanho, gatilhos login,
    senha, ssh, token, banco);
  - todo `ward <sub>` em código de `skill/ward/*.md` está em V8.1 **e** no
    `ward --help` do binário compilado (`login` aceito como pendente da Fase 43, com
    aviso, enquanto não estiver no `--help`);
  - sem KeePassXC, `cofre-*` e `--token-fd` na skill;
  - nenhum padrão de `.dev/variacoes.txt` em nenhum arquivo de `skill/` (0600
    conferido; nunca imprime o padrão, só arquivo e contagem); link simbólico em
    `skill/` recusado antes de qualquer leitura;
  - JSON do deny estrito (sem chave repetida, só `permissions.deny`, regras
    `Read|Edit|Bash(...)`, sem repetição), com `Read`/`Edit` de cada caminho de V8.2 e
    do `~/.ssh`, e regra `Bash` para cada caminho do Ward;
  - a skill cita os quatro caminhos de V8.2, `base64`, `hex`, `xxd`, `` `rev` ``,
    `printenv`, `set -x`, `/proc/self/environ`, `` `env` ``, anexo, RA3, `git push`,
    `~/.ssh`, o `PATH` fixo, "caminho absoluto" e as mensagens de erro.

## Segurança
- Invariantes tocadas:
  - S1/S4 (nenhum segredo para o agente; máscara): a skill não cita valor nenhum,
    só nomes de entrada e de variável; manda nunca imprimir variável nem anexo em
    nenhuma codificação, porque a máscara não cobre tudo (RA3).
  - S2 (anexo só em `/run/user/<uid>/ward/`, 0600): a skill manda usar só o caminho
    e nunca `cat`/`cp`/`base64` do anexo; deny de `Read`/`Edit` em
    `/run/user/*/ward/**`.
  - S3 (segredo fora de argv): a skill proíbe `ward ssh --token-fd` (só o app) e
    pedir/aceitar senha mestra ou PIN; deny de `Bash(ward destravar*)`.
  - D9 (domínio): "domínio não confere" → parar e avisar, sem tentar outra aba.
  - Nível/aprovação: negado não se repete nem se contorna; bloqueada = inexistente,
    sem adivinhar nomes.
- Testes de ataque (`dev/testes/skill.py` › `autoteste`, rodado antes da conferência
  real e depois de um controle que prova que a árvore real passa):
  segredo de teste plantado (3 casos: SKILL.md, arquivo novo, JSON), `ward revelar`,
  `ward destravar` em bloco, `ward exportar` em outro `.md`, `cofre-run`, KeePassXC,
  `--token-fd`, `name: cofre`, description sem "banco", JSON inválido, chave repetida,
  `allow` junto, cada `Read(...)` de V8.2 e do `~/.ssh` tirado, skill sem `base64`,
  skill sem o caminho do backup, link simbólico para `.dev/variacoes.txt`. Todos
  recusados.
- Dependências novas: nenhuma (Python da biblioteca padrão).

## Ficou de fora
- Instalar a skill e o deny no `~/.claude`: é do usuário, no M10 (pedido da fase).
- Conferir que o Claude Code aceita as regras `Bash` com `*` no início
  (`Bash(*.local/share/ward*)`) e o `//` absoluto: não dá sem tocar no `~/.claude`.
  Fica para o M10 (`/permissions` mostra as regras; passo 3 do `skill/README.md`).
- V8.3 (QA seguindo a skill com o cofre de teste) é do validador de uso.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make gitleaks-binario` (gitleaks copiado do repositório principal) | 0 |
| `make dev-cofre` | 0 |
| `make dev-variacoes` | 0 |
| `make teste-skill` | 0 (com o aviso de `login` pendente) |
| `make check` | 0 |

## Problemas descobertos
- **`ward login` ainda não existe** (Fase 43 em andamento). A skill o descreve pelo
  contrato (V6.3, V6.10, V6.11): aba ativa na página de login, extensão confere o
  domínio, preenche, envia e limpa. **Conferir depois do merge da 43**: o texto de
  erro de domínio (a skill cita "domínio não confere", de V6.3, e "a origem da
  página não bate com a da entrada", o `OrigemDiferente` atual), se precisa de aba já
  aberta, o que a CLI imprime, e tirar `login` de `PENDENTES` em
  `dev/testes/skill.py` (aí o `teste-skill` passa a exigir `login` no `--help`).
- **Fase 26a em paralelo**: a skill já descreve o ambiente por lista fixa e o `PATH`
  fixo de 26a. Conferir depois do merge da 26a que a lista de variáveis e o `PATH`
  batem com o implementado (o `teste-skill` exige o texto
  `PATH=/usr/local/bin:/usr/bin:/bin`).
- `ward ssh` de entrada `ftp/sftp`: cwd do `sftp` é o home do passwd (handoff 27); a
  skill manda usar caminho local absoluto em `get`/`put`.
- O deny de `Read(~/.ssh/**)` vai além dos quatro caminhos de V8.2: é pelo pedido da
  fase ("nunca ler `~/.ssh`") e não atrapalha o `git push`/`ssh` (o `ssh` lê as chaves
  sozinho). Se o usuário quiser que o Claude leia `~/.ssh/config` ou `known_hosts`,
  troca por `Read(~/.ssh/id_*)` no M10.
- As regras `Bash` comparam texto e são contornáveis; o README diz isso. A barreira é
  o daemon, a máscara e a aprovação.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `skill/ward/SKILL.md` | criado |
| `skill/settings-deny.json` | criado |
| `skill/README.md` | criado |
| `dev/testes/skill.py` | criado |
| `Makefile` | alterado (alvo `teste-skill`) |
| `CLAUDE.md` | alterado (linha do `make teste-skill`) |
| `docs/mission/handoffs/phase-50.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V8.2)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
