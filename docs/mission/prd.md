# PRD — Ward

Fonte: `docs/escopo.md` (grilling de 2026-11-08 e respostas do mesmo dia sobre nome,
`prod` e SSH). As referências abaixo usam os títulos das seções do escopo. Se este PRD
e o escopo divergirem, vale o escopo; corrija o PRD.

Repositório `git@github.com:jeffersonrucu/ward.git`, branch principal `master`.
Nomes: CLI `ward`, daemon `wardd`, skill `ward`, variáveis `WARD_*`, dados em
`~/.local/share/ward/`, temporários em `/run/user/<uid>/ward/`. "Cofre" no texto é o
substantivo.

## Objetivo

Um cofre **offline**, só de **credenciais de trabalho**, com CLI, app desktop
(Tauri + Svelte) e extensão do Chrome, que entrega credenciais ao Claude **sem que ele
veja nenhum segredo**: o Claude recebe o acesso, nunca a credencial ("Objetivo";
"Claude (agente)"). Cada tipo de credencial já sabe seus campos e como entregá-los à
ferramenta ("Credenciais" › "Tipos prontos (v1)"). Substitui o KeePassXC, que só é
desinstalado depois que o último milestone passar na validação ("Migração e saída do
KeePassXC").

**Segurança é o requisito número um.** Na dúvida entre conveniência e segurança,
escolhe-se segurança e a decisão fica registrada aqui ("Objetivo").

## Regras (com referência ao escopo)

### Arquitetura
- R1. `wardd` é **o único processo que toca em segredo**; CLI, app e extensão são
  clientes dele por socket local. Inicia com a sessão (systemd do usuário).
  ("Arquitetura")
- R2. Núcleo em Rust (`ward-core`): formato, criptografia, tipos, máscara, protocolo.
  Daemon e CLI usam o núcleo. ("Arquitetura")
- R3. Só Linux (Pop!_OS 24.04, COSMIC, Wayland). O que depende do sistema (iniciar com
  a sessão, bloqueio de tela e suspensão, socket, agente SSH temporário, proteção de
  memória, identificação do processo que chama) fica em `ward-plataforma`; núcleo,
  formato, tipos e interface não dependem de Linux. ("Arquitetura")
- R4. Instalação em `/usr/local`, dono root, por `sudo make install` ou `.deb`;
  extensão instalada por política do Chrome em `/etc/opt/chrome/policies`, sem modo
  desenvolvedor. ("Arquitetura")

### Credenciais
- R5. Nove tipos v1: `login`, `banco`, `servidor`, `api`, `aws`, `smtp`, `ftp/sftp`,
  `nota segura`, `genérico`, com os campos e a entrega de "Credenciais" › "Tipos
  prontos (v1)". Variáveis exatas em D10.
- R6. Cada campo declara se é segredo; segredo nunca aparece em `ward lista`. Toda
  entrada aceita campos extras (nome, valor, segredo sim/não). Tipos customizados
  ficam para a v2. ("Credenciais")
- R7. `grupo` e `ambiente` (`dev`, `stage`, `prod`) obrigatórios; `projeto` e tags
  opcionais; autocompletar de grupo no app. Grupo é só uma pasta genérica de
  entradas, sem efeito em permissão (D24). ("Credenciais")
- R8. TOTP: o cofre gera o código; o agente recebe o código, nunca a semente.
  ("Credenciais")
- R9. Anexos cifrados no cofre; no `ward run` viram arquivo em `/run/user/<uid>/ward/`,
  0600, apagado ao fim; a ferramenta recebe só o caminho em `WARD_ARQUIVO_<NOME>`.
  ("Credenciais")
- R10. Gerador de senha com tamanho e conjuntos configuráveis, no formulário e em tela
  própria. ("Credenciais")

### Claude (agente)
- R11. Comandos: `ward lista [--json]`, `ward run <entrada> -- <cmd…>`,
  `ward ssh <entrada> [cmd]`, `ward login <entrada>`, `ward playwright <entrada>`,
  além de `ward travar` e destravar pela CLI. ("Claude (agente)" › "Comandos";
  "Destravar e travar")
- R12. **Não existe operação que devolva segredo para a CLI ou para o agente.**
  Revelar e copiar só no app. ("Claude (agente)" › "Comandos")
- R13. Máscara de saída: valores secretos da entrada e variações óbvias (URL-encoded,
  base64, dentro de `DATABASE_URL`) viram `••••`. ("Claude (agente)" › "Comandos";
  "Segurança" › S4)
- R14. `ward login` e o preenchimento recusam se o domínio da aba não bater com a URL
  da entrada. ("Claude (agente)" › "Comandos")
- R15. Níveis por entrada: **bloqueado** (o agente nem vê), **com aprovação** (padrão;
  cada uso abre a janela), **livre** (usa sem perguntar). Valem para `run`, `ssh`,
  `login` e `playwright`. Mudar o nível exige PIN. ("Claude (agente)" › "Nível de
  acesso por entrada")
- R16. **O ambiente não muda as regras.** `prod` tem os mesmos níveis e botões que
  `dev` e `stage`, inclusive "livre" e "Permitir sempre"; o ambiente organiza, filtra
  e aparece em destaque na janela de aprovação. ("Claude (agente)" › "Nível de acesso
  por entrada")
- R17. Janela de aprovação: abre em primeiro plano; mostra entrada, ambiente, comando
  exato e cadeia de processos (`claude → bash → ward run`). Botões **Negar**,
  **Permitir uma vez**, **Permitir por 1 hora**, **Permitir sempre** (a entrada vira
  "livre"). Todos menos "Negar" pedem o PIN. Fechar nega; sem resposta em 2 min,
  expira e nega. Não há tela de aprovações; voltar atrás é mudar o nível na entrada.
  ("Claude (agente)" › "Janela de aprovação")
- R18. **SSH sem agente global.** O Ward **não é o agente SSH da máquina**: chaves
  pessoais (GitHub etc.) ficam no `~/.ssh` com o agente do sistema (`gcr-ssh-agent`),
  fora do Ward, e o Claude as usa normalmente (`git push`). Se um servidor tiver chave
  no `~/.ssh`, o Claude usa `ssh` direto e o Ward não intercepta, não bloqueia nem
  audita nada fora do `ward ssh`. Credenciais de servidores de trabalho ficam só em
  entradas `servidor`, usadas só por `ward ssh` (e por `ward run` com túnel). Com
  chave: o daemon sobe um agente SSH **temporário**, só para aquela conexão,
  acessível só pelo `ssh` filho, derrubado ao fim; a chave privada nunca sai do
  daemon. Com senha: `sshpass` lendo do ambiente, nunca de argv. Para o usuário,
  `ward ssh` sempre funciona sem PIN; para o Claude vale o nível da entrada; quem
  chamou é conhecido pelo próprio `ward ssh`, sem heurística de árvore de processos
  (ver D3). ("Claude (agente)" › "SSH (sem agente SSH global)")
- R19. **Nenhuma fase mexe em `~/.ssh`, `ssh_config` ou `SSH_AUTH_SOCK` do usuário**
  (nem lê). ("Claude (agente)" › "SSH (sem agente SSH global)"; `CLAUDE.md`)
- R20. Skill `ward` reescrita do zero, com o fluxo "listar, cruzar grupo, ambiente e
  tipo com a tarefa, avisar qual entrada vai usar"; regras de deny do
  `~/.claude/settings.json` cobrem o arquivo do cofre e os backups; a skill antiga
  `cofre` e seus scripts saem junto com o KeePassXC. ("Claude (agente)" › "Skill
  `ward`")

### Destravar e travar
- R21. Destrava só com a senha mestra, pelo app ou pela CLI no terminal do usuário.
  Sem TPM. ("Destravar e travar")
- R22. Pedido do agente com o cofre travado: o app abre e pede a senha mestra; o
  pedido espera até 2 min. ("Destravar e travar")
- R23. Trava sozinho no bloqueio de tela, na suspensão e após 4 h sem uso (cada uso
  zera). "Travar agora" no app, na bandeja e em `ward travar`. ("Destravar e travar")
- R24. PIN de 4 a 6 dígitos, separado da senha mestra; só aprova usos e mudanças de
  nível, não destrava; 5 erros seguidos travam o cofre. ("Destravar e travar";
  "Segurança" › S9)

### App, extensão, backup, migração
- R25. App: Credenciais (agrupada por grupo, filtros por tipo e ambiente, busca; a
  entrada abre o formulário do tipo com revelar, copiar, editar, anexos, TOTP e
  nível), Gerador de senha, Configurações (PIN, senha mestra, tempo de trava, backup
  externo); além de destravar, primeiro uso, janela de aprovação, bandeja, restaurar
  backup e kit de emergência. Interface toda em português. Só tema escuro, no app e na
  extensão: sem modo claro, sem alternância de tema e sem opção de tema em
  Configurações. ("App desktop"; "Missão 0 — Design no FigCraft")
- R26. Extensão do Chrome (só Chrome): sugere entradas do domínio e preenche usuário,
  senha e TOTP para o usuário; executa `ward login` para o Claude (domínio conferido,
  envia, limpa os campos); oferece salvar login novo pedindo tipo, grupo e
  ambiente. ("Extensão do Chrome")
- R27. Backup local automático cifrado a cada mudança, 30 versões; externo só em
  pendrive, opcional, **desligado por padrão**, copia quando o pendrive escolhido é
  conectado; restaurar pelo app escolhendo a versão; kit de emergência para imprimir
  com espaço para anotar a senha à mão (o cofre nunca preenche). ("Backup")
- R28. Migração manual (sem `.kdbx`). Etapa final guiada, feita pelo usuário: mover as
  chaves SSH pessoais que o KeePassXC carrega no agente para o `~/.ssh` (agente do
  sistema) e as dos servidores para entradas `servidor`; devolver o chaveiro ao
  gnome-keyring (inclusive "Chrome Safe Storage"); desfazer o que
  `~/.claude/skills/cofre/references/montagem.md` descreve; conferir as senhas do
  Chrome; só então desinstalar o KeePassXC. O KeePassXC funciona até o último marco
  passar na validação. ("Migração e saída do KeePassXC")

### Desenvolvimento
- R29. Toda a missão usa o cofre de teste em `.dev/` (fora do git), com senha mestra e
  PIN de teste e credenciais falsas; Postgres, SSH e WordPress de teste em
  `docker compose`. O cofre real nunca é tocado. ("Segurança" › "Regras de
  desenvolvimento")
- R30. QA da interface: o Svelte roda também no Chrome em modo de teste ligado ao
  daemon de teste; a janela nativa do Tauri o usuário confere por roteiro curto no fim
  de cada milestone com UI. O Claude não aprova sozinho: o QA testa "negar" e
  "expirar"; "aprovar com PIN" usa o PIN de teste. ("Segurança" › "Regras de
  desenvolvimento")
- R31. Antes de qualquer código, o design é montado no FigCraft pelo MCP
  (`mcp__figcraft__*`), no arquivo `/home/jeffersonoliveira/documentos/Ward.figcraft`
  (páginas Inspirações só leitura, Design System, Layout), salvo (`save_file`) ao fim
  de cada fase de design. O Figma `bzu9PxxFur2tU3vlNCHvsY` é só histórico e leitura
  (Inspirações `1:3` só leitura). O milestone 0 fecha com a aprovação do usuário no
  FigCraft; as fases de UI implementam a partir dos nodes de
  `docs/mission/design-nodes.md`. ("Missão 0 — Design no FigCraft" › "Arquivo de
  design")

## Modelo de ameaça e invariantes (S1–S15, copiadas do escopo, com o milestone que cobre cada uma)

Do que o cofre defende ("Segurança" › "Modelo de ameaça"):
1. O próprio agente, por erro ou prompt injection.
2. Pacote ou script malicioso rodando como o usuário (npm, pip, cargo).
3. Disco, backup ou pendrive roubados.
4. Página de phishing recebendo a senha pelo preenchimento.

Fora do modelo ("Segurança" › "Fora do modelo de ameaça"): root, kernel ou firmware
comprometidos; acesso físico à máquina destravada; processo feito de propósito para
driblar a máscara (quem barra é a aprovação, que mostra o comando exato); leitura do
campo de senha da página no instante do preenchimento; troca conjunta do cofre e do
arquivo de estado por cópias antigas com o daemon parado (RA4, decisão do usuário em
2026-11-08). Também fora, por decisão do escopo: SSH com chaves do `~/.ssh` (o Ward
não interfere).

| # | Invariante (texto do escopo) | Milestones |
|---|---|---|
| S1 | Nenhuma operação do socket devolve segredo à CLI. Revelar e copiar só pelo app; preencher só pela extensão no domínio certo. | M3, M4, M5, M6 |
| S2 | Segredo nunca vai para disco em claro. Anexo temporário só em `/run/user/<uid>/ward/`, 0600, apagado ao fim (inclusive se o comando falhar ou for morto). | M2, M4, M6, M7 |
| S3 | Segredo nunca vai em argv (visível em `/proc/<pid>/cmdline`); só no ambiente do processo filho, em arquivo temporário ou em fd. | M4, M6 |
| S4 | A máscara cobre os valores secretos e as variações declaradas, inclusive quando o valor chega quebrado entre dois pedaços da saída. | M2, M4 |
| S5 | Socket em diretório 0700, conferência do uid do cliente por `SO_PEERCRED`. A CLI confere que o servidor é o `wardd` instalado antes de enviar a senha mestra. | M3, M4, M9 |
| S6 | Criptografia só com bibliotecas auditadas: Argon2id (parâmetros registrados no cabeçalho, mínimo ~256 MiB / 3 passes), XChaCha20-Poly1305 com o cabeçalho como dado autenticado, chave de dados aleatória embrulhada pela chave derivada. Um byte alterado no arquivo faz a abertura falhar. Nada de cripto feita à mão. | M2, M7 |
| S7 | Gravação atômica (temp + fsync + rename). Uma gravação interrompida nunca corrompe a versão anterior. | M2, M7 |
| S8 | Daemon com `PR_SET_DUMPABLE=0`, sem core dump, segredos zerados da memória ao travar e após o uso (`zeroize`). Daemon recusa iniciar com `LD_PRELOAD` e similares. | M3, M9 |
| S9 | Mudar nível de acesso exige PIN. 5 erros de PIN travam o cofre. | M3, M5 |
| S10 | Aprovação expira em 2 min e fechar nega. | M3, M5 |
| S11 | Tauri com capabilities mínimas (só os comandos que o app usa), CSP estrita, nenhum conteúdo remoto, nenhum segredo na webview fora do "revelar". | M5 |
| S12 | Área de transferência limpa em 30 s. | M5 |
| S13 | Logs de depuração nunca contêm segredo. | M3, M4, M5, M6 |
| S14 | Dependências mínimas; `cargo audit` e `cargo deny` limpos; lockfiles no git; frontend sem dependência de runtime além do necessário. | M1 (e todo milestone, por `make check`) |
| S15 | O repositório nunca contém segredo real (varredura por gitleaks ou equivalente). | M1 (e todo milestone, por `make check`) |

## Fora do escopo

Copiado de "Fora do escopo": Serviço Secreto e chaveiro da máquina (ficam com o
gnome-keyring), contas pessoais, sincronização e celular, Firefox e outros
navegadores, desbloqueio por TPM, importação de `.kdbx`, tela de atividade e registro
de uso, tela de saúde do cofre, tela de aprovações, tipos customizados (v2), restrição
de comandos dentro dos servidores, Windows e macOS (por enquanto).

Também fora (decorre do escopo): agente SSH da máquina, interceptar ou auditar SSH fora
do `ward ssh`, qualquer mudança em `~/.ssh`/`ssh_config`/`SSH_AUTH_SOCK` ("SSH (sem
agente SSH global)"); operações de escrita pela CLI (cadastrar ou editar entrada é só
pelo app e pelo "salvar login novo" da extensão; D3).

## Fluxos

1. **Usuário no app.** Primeiro uso: cria o cofre, define senha mestra e PIN. Depois:
   destrava com a senha mestra → Credenciais agrupada por grupo, filtra por tipo e
   ambiente, busca → abre a entrada no formulário do tipo → revela (o valor some em
   30 s ou ao sair da tela), copia (a área de transferência é limpa em 30 s), edita,
   anexa arquivo, vê o TOTP, muda o nível (pede PIN), e numa entrada `servidor` clica
   **"Conectar"** (abre o terminal com `ward ssh`, sem PIN, D3) → usa o gerador avulso
   → em Configurações troca PIN, senha mestra, tempo de trava e liga o backup externo →
   restaura um backup escolhendo a versão → imprime o kit de emergência. Trava pela
   bandeja, pelo app ou sozinho (bloqueio de tela, suspensão, 4 h sem uso).
2. **Usuário no terminal.** `ward destravar` pede a senha mestra no TTY (a CLI confere
   antes que o servidor é o `wardd` instalado). `ward lista`, `ward travar`. O `ssh` e o
   `git` com chaves do `~/.ssh` seguem como hoje, sem o Ward no meio (R18). Os comandos
   `ward run/ssh/login/playwright` digitados no terminal seguem o nível da entrada
   (D3); o atalho do usuário sem PIN para servidor é o "Conectar" do app.
3. **Claude via skill.** A skill `ward` roda `ward lista --json` → cruza grupo,
   ambiente e tipo com a tarefa → avisa qual entrada vai usar → chama
   `ward run <entrada> -- <cmd>` / `ward ssh <entrada> [cmd]` / `ward login <entrada>` /
   `ward playwright <entrada>`. O daemon confere o nível: bloqueado → "entrada não
   encontrada" (a mesma resposta de uma entrada que não existe); livre → executa; com
   aprovação → janela em primeiro plano com entrada, ambiente (em destaque se `prod`),
   comando exato e cadeia de processos → o usuário nega, ou permite uma vez / por 1 h /
   sempre com PIN → o daemon executa e devolve só a saída mascarada e o código de saída
   (ou o caminho do storage state, no `playwright`). Cofre travado: o app abre no
   destravar e o pedido espera até 2 min. Expirou ou fechou: nega.
4. **Extensão.** Usuário: abre o popup numa página de login → vê as entradas cuja
   origem bate com a aba → clica → a extensão preenche usuário, senha e TOTP. Envio de
   formulário com credencial não cadastrada → oferece salvar e pede tipo, grupo e
   ambiente (nível inicial "com aprovação"). Cofre travado → popup "cofre travado" com
   botão que abre o app no destravar. Claude: `ward login <entrada>` → daemon (nível,
   aprovação) → host de native messaging → extensão confere a origem da aba ativa,
   preenche, envia e limpa os campos.
5. **Backup.** Cada gravação gera uma cópia cifrada datada (30 versões). Com o externo
   ligado, ao conectar o pendrive escolhido o daemon copia as versões. Restaurar:
   app → Configurações → Restaurar → escolhe a versão → senha mestra daquela versão →
   o atual vira backup e a versão escolhida entra no lugar.

## Decisões provisórias

Aceitas pelo usuário em 2026-11-08 ou derivadas do escopo; mudam só editando o escopo
ou com nova resposta do usuário.

- D1. **Tema**: só escuro (decidido pelo usuário em 2026-11-08; "Missão 0 — Design no
  FigCraft"). A coleção `Cores` tem um único modo, `Escuro`; não existe modo claro,
  alternância de tema nem leitura do tema do sistema, no app e na extensão.
  **Ícone**: proposto no M0 ("Pendências").
- D2. **"Permitir por 1 hora"** vale para a entrada inteira (qualquer comando do
  agente com aquela entrada) e acaba ao travar o cofre. "Permitir uma vez" vale só para
  o pedido exibido.
- D3. **CLI = agente; o caminho do usuário no `ward ssh` vem do app** (a confirmar
  com o usuário antes do M4; ver RA2). O daemon trata toda conexão de cliente comum
  (qualquer processo que não seja o app ou o host da extensão instalados, D5) como
  agente: aplica o nível em `run/ssh/login/playwright` e esconde as bloqueadas no
  `lista`. O escopo diz que o próprio `ward ssh` sabe quem chamou, sem heurística; o
  único sinal que o Claude não forja é o papel "app" de D5 (TTY, variável ou argumento
  ele forja com `script` ou `env`). Então o caminho do usuário sem PIN é o botão
  **"Conectar"** da entrada `servidor` no app: o daemon emite ao app um token de uso
  único (30 s, uma conexão, preso àquela entrada) e o app abre o terminal padrão
  rodando `ward ssh --token <t> <entrada>`. O token não é segredo de credencial e
  morre no primeiro uso. `ward ssh` sem token segue o nível. A CLI não tem operação de
  escrita; destravar e travar são as únicas mudanças de estado.
- D4. **Revelar e copiar** exigem só o app destravado, sem PIN. "Revelar" mostra o
  valor por 30 s ou até sair da tela e depois o tira do DOM. "Copiar" é resolvido no
  lado Rust do app: o valor vai do daemon direto para a área de transferência e nunca
  passa pela webview (S11). A limpeza em 30 s só apaga se a área de transferência ainda
  contém o valor copiado (S12).
- D5. **Papéis no socket.** O daemon atende operações de app (revelar, copiar, editar,
  aprovar, mudar nível, emitir token do "Conectar") e de extensão (sugerir,
  preencher) **só** quando o processo do outro lado (pid por `SO_PEERCRED`) é o binário
  instalado (`/proc/<pid>/exe` em `/usr/local/…`, dono root, sem escrita para grupo e
  outros), sem depurador (`TracerPid: 0`) e sem `LD_PRELOAD`/`LD_AUDIT`/
  `LD_LIBRARY_PATH` no ambiente. Para o host da extensão, o pai também tem de ser o
  Chrome instalado (`/opt/google/chrome/chrome`). Qualquer outro cliente fica com o
  papel "agente". O mesmo teste vale para a CLI e o app conferirem o `wardd` antes de
  mandar a senha mestra (S5). **Revisto pela Fase 21:** contra o `wardd` endurecido
  (S8) a conferência pelo `/proc` falha fechada, e o app e o host precisam ficar
  dumpable até serem identificados. Ver D46 (servidor), D47 (app e host) e D48 (`LD_*`),
  provisórios até a resposta do usuário (RA5, RA6).
- D6. **`ward playwright`**: o daemon abre o Chrome instalado em headless com
  `--remote-debugging-pipe` (sem porta TCP) e perfil temporário em
  `/run/user/<uid>/ward/pw-<aleatório>/`, confere a origem, preenche, envia, confere
  que a origem final continua a da entrada, grava o storage state em
  `/run/user/<uid>/ward/playwright/<entrada>.json` (0600) e devolve só o caminho. O
  perfil é apagado ao fim; os storage states são apagados ao travar e na partida do
  daemon. O storage state é acesso (sessão), não credencial; nunca contém a senha.
- D7. **Ponte de teste.** O Svelte roda no Chrome em modo de teste por uma ponte HTTP
  (`ward-ponte-teste`, 127.0.0.1, porta aleatória, token aleatório) que só existe com a
  feature `modo-teste`. O build de release não contém a ponte nem lê `WARD_DEV_DIR`;
  `make install` recusa artefatos com `modo-teste`. O daemon de teste usa o cofre
  `.dev/cofre-teste.ward`, backups em `.dev/backups/` e, para socket e temporários,
  `/run/user/<uid>/ward-teste/` (tmpfs, como pede S2); nunca `/run/user/<uid>/ward/`
  nem `~/.local/share/ward/`.
- D8. **Aprovação antes do app.** Até o M5 a aprovação é testada nos testes Rust do
  daemon, com um aprovador em processo; o socket não tem porta dos fundos para aprovar
  em nenhum build.
- D9. **Origem no preenchimento** (`ward login`, extensão, `playwright`): compara a
  origem inteira (esquema, host em minúsculas e punycode, porta) da aba com a URL da
  entrada. Subdomínio não conta como igual. `http` só é aceito para `localhost` e
  `127.0.0.1`. A mesma regra já vale ao salvar, pelo parser estrito de URL de D41.
- D10. **Variáveis de entrega** (`ward run`). Em todo tipo: `WARD_ENTRADA`,
  `WARD_GRUPO`, `WARD_AMBIENTE`. Campos extras viram `WARD_<CAMPO>` e anexos
  `WARD_ARQUIVO_<NOME>` (nome em maiúsculas; o que não for `[A-Z0-9]` vira `_`; nome
  repetido é erro de validação ao salvar).
  - `login`: `WARD_URL`, `WARD_USER`, `WARD_PASS`, `WARD_TOTP` (código atual, se houver
    semente).
  - `banco` postgres: `PGHOST`, `PGPORT`, `PGDATABASE`, `PGUSER`, `PGPASSWORD`,
    `DATABASE_URL=postgresql://…`. mysql: `MYSQL_HOST`, `MYSQL_TCP_PORT`, `MYSQL_USER`,
    `MYSQL_PWD`, `MYSQL_DATABASE`, `DATABASE_URL=mysql://…`. redis: `REDIS_HOST`,
    `REDIS_PORT`, `REDISCLI_AUTH`, `REDIS_URL`, `DATABASE_URL=redis://…`. mongo:
    `MONGODB_URI`, `DATABASE_URL=mongodb://…`. sqlserver: `SQLCMDSERVER`, `SQLCMDUSER`,
    `SQLCMDPASSWORD`, `SQLCMDDBNAME`, `DATABASE_URL=sqlserver://…`. Usuário e senha
    dentro das URLs vão com percent-encoding. Com túnel SSH, o daemon abre o túnel pelo
    `servidor` referenciado (mesma mecânica de D12/D13) e host/porta apontam para
    `127.0.0.1:<porta local>`.
  - `servidor`: `WARD_HOST`, `WARD_PORT`, `WARD_USER`, e `WARD_PASS` ou
    `WARD_ARQUIVO_CHAVE` (+ `WARD_PASSPHRASE` se houver). O caminho usual de servidor é
    `ward ssh`; `ward run` existe para ferramentas como `rsync`/`scp` com a chave.
  - `api`: `WARD_URL`, `WARD_TOKEN` e a variável escolhida (ex.: `GITHUB_TOKEN`) com o
    mesmo token.
  - `aws`: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` (se houver),
    `AWS_REGION`, `AWS_DEFAULT_REGION`, `AWS_ROLE_ARN` (se houver).
  - `smtp`: `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, `SMTP_SECURITY`
    (`nenhuma`, `starttls`, `tls`).
  - `ftp/sftp`: `WARD_PROTOCOLO`, `WARD_HOST`, `WARD_PORT`, `WARD_USER`, e `WARD_PASS` ou
    `WARD_ARQUIVO_CHAVE` (+ `WARD_PASSPHRASE`).
  - `nota segura`: sem entrega; `ward run` recusa com "tipo sem entrega".
  - `genérico`: só os `WARD_<CAMPO>` dos campos extras.
- D11. **Máscara.** Para cada segredo da entrada (inclusive campos extras secretos,
  passphrase e semente TOTP) mascara: o valor literal; percent-encoding em maiúsculas e
  minúsculas (`%2F`/`%2f`) e com `+` para espaço; base64 padrão e URL-safe, com e sem
  `=`; base64 de `usuário:senha` (Basic auth); e o `DATABASE_URL` inteiro. Anexos
  entram na máscara pelo conteúdo inteiro e por cada linha de texto com 6 caracteres ou
  mais (e, em linhas `CHAVE=valor`, também pelo valor). Todo segredo de várias linhas
  (chave privada de `servidor` e `ftp/sftp`, extra secreto com `\n`) também passa por
  linha, como os anexos (D36). Também entram as formas escapadas de D37: `\u00XX` de
  JSON (Go e `ensure_ascii`) e `\$` entre aspas duplas de shell (`declare -p`). stdout e stderr são mascarados
  em separado, com buffer que segura o fim de cada pedaço até saber se é começo de
  segredo. O código TOTP não é mascarado (o agente o recebe, R8).
  Segredo com menos de 6 caracteres é mascarado do mesmo jeito, e o app avisa que a
  máscara fica pouco confiável.
- D12. **Agente SSH temporário** do `ward ssh` com chave: socket em
  `/run/user/<uid>/ward/ssh-<aleatório>/agent.sock` (diretório 0700), passado ao filho
  por `IdentityAgent`. O daemon só atende conexões cujo pid (`SO_PEERCRED`) é o `ssh`
  filho daquela sessão; carrega só a chave da entrada (e a do salto, se houver); só
  lista e assina, e recusa adicionar, remover e travar chaves; some (socket e
  diretório) quando o `ssh` termina, falha ou é morto, e quando o cofre trava.
- D13. **`ward run` e `ward ssh`**: a CLI manda ao daemon o argv, o cwd e o ambiente
  dela sem `LD_*`. O daemon cria o filho, repassa stdin (e pty no `ward ssh`
  interativo), devolve stdout/stderr mascarados e o código de saída, e mata o grupo de
  processos do filho se a CLI cair. O `ssh` filho roda com `-F /dev/null`,
  `IdentitiesOnly=yes`, `IdentityFile=none` e
  `UserKnownHostsFile=~/.local/share/ward/known_hosts` (a primeira conexão grava a
  chave do host; chave diferente depois recusa), para não ler nem gravar nada em
  `~/.ssh`. Com senha: `sshpass -e` (senha em `SSHPASS` no ambiente do filho, nunca em
  argv), `PubkeyAuthentication=no`, `IdentityAgent=none`. Com chave: agente de D12.
  Salto por `ProxyJump` com as credenciais da entrada de salto. `sshpass` vira
  dependência de sistema declarada na instalação.
- D14. **Memória.** O daemon trava (`mlock`) os buffers de segredo e a chave de dados;
  a memória de trabalho do Argon2 não é travada (limite de `RLIMIT_MEMLOCK` do usuário).
  Risco residual registrado: swap sem cifra; o Pop!_OS com disco cifrado cifra a swap.
- D15. **Cofre de teste.** Senha mestra de teste `falso-mestra-ward-01`, PIN de teste
  `135790`, entradas da tabela "Cofre de teste" do contrato (21, com `acme-totp-dev`
  de D28). Tudo falso, gerado em `.dev/` por `make dev-cofre`; os valores fixos ficam na
  allowlist do gitleaks.
- D16. **Inatividade e espera.** "Uso" é qualquer operação autenticada (lista, run,
  ssh, login, playwright, ação no app). A espera de 2 min pelo destrave (R22) e a de
  2 min pela aprovação (R17) contam em separado.
- D17. **Backup** em `~/.local/share/ward/backups/ward-AAAAMMDD-HHMMSS.ward`, no mesmo
  formato cifrado. Pendrive escolhido pelo UUID do sistema de arquivos; cópias em
  `<montagem>/ward-backup/`, também com 30 versões. Restaurar pede a senha mestra da
  versão escolhida e grava o atual como backup antes de trocar. As regras de deny da
  skill passam a cobrir `/media/**/ward-backup/**`.
- D18. **Instalação.** Binários em `/usr/local/bin` (`ward`, `wardd`, `ward-app`) e
  `/usr/local/lib/ward/` (`ward-nm`); unit em
  `/usr/local/lib/systemd/user/wardd.service`; manifesto de native messaging em
  `/etc/opt/chrome/native-messaging-hosts/`; política da extensão em
  `/etc/opt/chrome/policies/managed/` (force-install com update local e só a extensão
  do Ward com aquele ID). Nada de `SSH_AUTH_SOCK`, `environment.d` ou `ssh_config`.
  `.deb` gerado por `dpkg-deb` a partir da mesma árvore, sem dependência nova de build.

- D19. **Nível combinado.** Quando um uso envolve duas entradas (`servidor` com salto,
  `banco` com túnel), vale o nível mais restritivo das duas, e a janela de aprovação
  mostra as duas.
- D20. **Senha mestra pelo socket.** Cada tentativa errada de destravar aumenta a
  espera da seguinte (dobra a partir de 1 s, até 30 s); a resposta não diz quantas
  tentativas houve. Protege contra o agente tentar adivinhar pelo socket.
- D21. **Aprovação sem app.** Se o app não pode ser aberto (ainda não existe, ou falhou
  ao subir), o pedido com aprovação é negado na hora com "aprovação indisponível".
- D22. **Ferramenta de design: FigCraft** (decisão do usuário, 2026-11-08; "Missão 0 —
  Design no FigCraft" › "Arquivo de design"). Fonte de verdade:
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, ajustado no lugar, nunca
  substituído por outro arquivo. As Fases 1–4 foram feitas no Figma
  (`bzu9PxxFur2tU3vlNCHvsY`, handoffs `phase-1..4.md` com os node ids do Figma) e são
  migradas pelas Fases 4a–4f; o Figma fica só para leitura (histórico e Inspirações
  `1:3`). Toda fase (DESIGN) termina com `save_file` e registra os ids do FigCraft em
  `docs/mission/design-nodes.md`. Provisório: se o `Ward.figcraft` não tiver a página
  Inspirações, ela não é criada pelas fases (é do usuário); a referência é lida no
  Figma `1:3`.
- D23. **Direção visual** (pedido do usuário, 2026-11-08: "layout quadrado, não passa
  cara de premium/profissional"): tudo arredondado e com cara de produto premium.
  Coleção `Raio` com controles ~10–12, cartões/painéis ~16–20, modais/janelas ~24,
  selos e toggles em pílula, e todo raio ligado a ela; profundidade com camadas de
  superfície, bordas sutis e sombras suaves; respiro maior no espaçamento; hierarquia
  tipográfica refinada; tokens de largura (barra lateral, diálogo, formulário,
  selects); badges do `Item de lista` alinhados em colunas. Vale para toda fase
  (DESIGN) e (UI).
- D24. **Grupo em vez de cliente** (decisão do usuário, 2026-11-08: "clientes acaba
  fechando um escopo e na verdade é só uma pasta"). O campo obrigatório de
  organização é `grupo` (pasta genérica; plural "grupos"), com os exemplos `acme`,
  `beta`, `gama`, `clientex`. Vale para modelo de dados, `ward lista` (coluna e
  `--json`), `WARD_GRUPO` no `ward run`, app (lista agrupada por grupo, autocompletar),
  extensão (salvar login pede grupo) e skill `ward` (cruza grupo, ambiente e tipo).
  Grupo não é fronteira de segurança: nenhuma regra ou invariante S1–S15 dependia de
  "cliente"; nível de acesso, aprovação, "Permitir por 1 hora" (D2) e nível combinado
  (D19) continuam por entrada. ("Credenciais")

- D25. **Regras do modelo de entrada** (tomadas pela Fase 15 pelo lado mais seguro e
  confirmadas pelo planner em 2026-11-08; nenhuma contradiz "Credenciais" › "Tipos
  prontos (v1)", que lista os campos sem dizer quais são obrigatórios). O comportamento
  fica como está em `ward-core::entrada`.
  - **Obrigatórios por tipo**, além de nome, `grupo` e `ambiente` (R7): `login` URL,
    usuário e senha (TOTP opcional); `banco` motor, host, usuário e senha (base, porta e
    túnel opcionais); `servidor` e `ftp/sftp` host, usuário e senha **ou** chave
    (passphrase só com chave; `ftp` não aceita chave); `api` token (URL e variável
    opcionais); `aws` access key ID, secret e região (role e session token opcionais);
    `smtp` host, usuário, senha e segurança; `nota segura` texto; `genérico` pelo menos um
    extra. Porta vazia usa a padrão do motor, do protocolo, da segurança ou 22.
  - **Nome da entrada em slug**: `[a-z0-9][a-z0-9._-]*`, até 64 caracteres, único no
    cofre. Os nomes de D15 já seguem a regra.
  - **Salto e túnel com um nível só** ("Credenciais" diz "servidor de salto opcional" e
    "túnel SSH opcional (referência a um `servidor`)", no singular): a referência aponta
    para um `servidor` existente, sem autorreferência; o servidor de salto e o do túnel
    não têm salto, e um servidor usado como salto ou túnel não pode ganhar salto. Assim
    todo uso envolve no máximo as duas entradas de D19. Cadeia de saltos fica para a v2,
    e exigiria dizer em D19 como fica o nível de três entradas.
  - **Variáveis proibidas no `api`**: a variável escolhida não pode ser `LD_*`,
    `WARD_*`, `BASH_FUNC_*`, `PATH`, `BASH_ENV`, `SSHPASS`, `GIT_SSH_COMMAND`,
    `NODE_OPTIONS` e as demais da lista do código (S3, D13). Desde a Fase 19c vale
    antes uma lista branca por sufixo (D42), e esta lista fica como segunda camada; extra e anexo também não podem
    gerar uma variável que o tipo já entrega (D10), e o anexo `chave` é recusado em
    `servidor` e `ftp/sftp`.
  - **Limites de tamanho**: 64 KiB por segredo, 1 MiB por nota, 256 KiB de anexos
    somados por entrada (D29; era 16 MiB por anexo), anexo vazio recusado (D29), 64
    extras, 32 anexos e 16 tags por entrada, semente TOTP de 16 a 128 caracteres base32,
    e limites de texto em todo campo; tudo numa fonte só, `ward_core::limites` (D32).
    A Fase 20 dimensiona o protocolo para isso: o
    limite de quadro de V3.4 vale para o cliente comum, e o anexo do app chega em
    pedaços (ou num quadro maior só para o papel app já identificado, D5).
  - **Texto não confiável**: recusa caractere de controle, separador de linha e
    formatação Unicode (bidi, largura zero, tags, BOM) em todo texto, nulo em segredo, e
    `-` no começo de host, usuário, salto e túnel (o `-oProxyCommand` não vira opção do
    `ssh`). O `usuário` do `login` aceita espaço; nos outros tipos, não. A Fase 19c
    troca essa lista de recusa por uma lista branca por campo (D38), e o usuário de
    `servidor` e `ftp/sftp` ganha formato próprio (D43).
  - A semente TOTP chega normalizada (maiúsculas, sem espaço): quem normaliza é o app
    (Fase 33) e a extensão, antes de mandar.
  - A entrada nova não tem campo de nível e nasce "com aprovação"; editar mantém o nível
    de antes; só `definir_nivel` muda, chamado pelo daemon depois do PIN (S9, Fase 22).
  - Sem referência de chave entre entradas: `acme-ssh-interno` e `-aprov` guardam uma
    cópia da chave de `acme-ssh-chave-prod` (Fase 19).
- D26. **Corpo do cofre** (lacuna achada pela Fase 15; resolvido pela Fase 15a). O
  `formato` cifra bytes opacos, e o conteúdo é uma codificação binária própria do
  `ward-core`, versionada e sem dependência nova (S14): versão do corpo, número de
  entradas e, por entrada, os campos com comprimento prefixado (u32 LE), tipo e nível
  pelos identificadores estáveis da Fase 15. Ao gravar, o buffer é um
  `Zeroizing<Vec<u8>>` com a capacidade final calculada antes (nenhuma realocação deixa
  cópia em claro, S8) e a codificação é canônica (mesmas entradas, mesmos bytes). Ao ler,
  todo comprimento é conferido contra o que resta do buffer e contra os limites de D25
  **antes** de alocar; o corpo inteiro tem limite de 64 MiB (provisório) e até 10 000
  entradas; cada entrada passa de novo por `Rascunho::validar` (o disco é tratado como
  entrada não confiável), recebe o nível guardado por `definir_nivel` e o cofre todo
  passa por `conferir_no_cofre`. Qualquer falha recusa o corpo inteiro com um erro
  genérico em português, sem valor do usuário (S13); nunca se abre um cofre pela metade.
  A versão 1 do corpo é substituída pela 2, que ganha a seção de configuração (D30), e
  a 2 pela 3, que ganha o contador de geração (D39).
- D27. **`gravacao::gravar` sai da API pública** (achado da Fase 14; feito na Fase 15a).
  Ele aceita qualquer buffer com cabeçalho válido, porque não tem a chave para conferir
  a autenticidade. Vira `pub(crate)`. Fora do `ward-core`, toda gravação parte de um
  `CofreAberto`: `gravar_cofre` (selar o conteúdo e gravar; `pub(crate)` desde a Fase 19c, D44, ficando `gravar_entradas` como a pública) e uma função de troca de
  senha que sela e grava pelo mesmo caminho. O backup (Fase 45) também fica dentro do
  `ward-core` e usa o `gravar` interno.
- D28. **TOTP no `localhost:8083`** (pendência da Fase 12). D9 compara a origem com a
  porta, então nenhuma entrada de 8081 vale para o serviço `totp`. O cofre de teste
  ganha `acme-totp-dev` (login, `acme`, `dev`, livre, URL `http://localhost:8083/login`,
  usuário `wp_admin`, senha `falso-WP-senha-01`, semente `JBSWY3DPEHPK3PXP`), com os
  mesmos valores falsos que o serviço já aceita (a conta de `acme-wp-dev` em
  `totp-contas.json`), então nem o `gerar.py` nem a allowlist do gitleaks mudam. A
  Fase 19 a acrescenta em `dev/fixtures/entradas.json`. O V6.7 a usa, e o validador não
  precisa cadastrar nada.

Decisões do planner sobre os handoffs 15a, 16, 17 e 18 (2026-11-08). O código muda nas
Fases 19a e 19b (M2) e nas fases pendentes citadas.

- D29. **Anexos: limite real e anexo vazio** (handoffs 15a e 17). A máscara recusa mais
  de 8 MiB de padrões, e um anexo gera cerca de 7 vezes o próprio tamanho em padrões
  (inteiro, 4 formas de base64 e por linha, D11). Com 16 MiB, a entrada existiria no
  cofre e ficaria sem `ward run`. Valem então três regras, iguais no cofre, na máscara e
  no app:
  - **anexos somados até 256 KiB por entrada** (cada anexo também até 256 KiB). Chave
    SSH, `.pem`, `.env`, `.ovpn` e kubeconfig ficam muito abaixo disso ("Credenciais" ›
    "Anexos");
  - **orçamento da máscara por entrada de 4 MiB de padrões**, metade do limite da
    máscara, para que as duas entradas de um uso combinado (D19) caibam juntas. A
    validação (`Rascunho::validar`, logo também a leitura do corpo) calcula o custo com
    a mesma função que o `Gerador` usa e recusa a entrada que passa disso, com a
    mensagem "entrada grande demais para a máscara do `ward run`". A `nota segura` fica
    fora do orçamento porque não tem entrega (D10);
  - **anexo vazio é recusado** ao salvar e na leitura do corpo, como a Fase 15 já faz.
    O anexo vazio não entrega nada e geraria um padrão vazio na máscara. O V2.30 deixa
    de pedir ida e volta de anexo vazio.
  O app (Fase 34) avisa ao escolher o arquivo, antes de salvar, quando ele ou a soma
  passa de 256 KiB, e mostra no campo a recusa do orçamento. O protocolo (Fase 20) só
  precisa levar anexos até esse limite. O corpo continua com 64 MiB e 10 000 entradas.
- D30. **Corpo v2 com seção de configuração** (handoff 15a). O V3.13 exige o hash do
  PIN dentro da parte cifrada, e o corpo v1 só tem entradas. Ainda não existe cofre
  real, então não há migração: a Fase 19b define a versão 2 do corpo (D26) e a leitura
  passa a recusar a 1 como versão desconhecida. A seção de configuração vem antes das
  entradas, é obrigatória e guarda: o hash Argon2id do PIN em formato PHC, com
  parâmetros e sal, feito por `cripto::argon2id` (nota da Fase 13); o tempo de trava
  por inatividade em minutos (padrão 240, R23/D16, alterável em Configurações, V5.22);
  e o backup externo, desligado por padrão, com o UUID opcional do pendrive (D17). Um
  cofre sem hash de PIN é recusado. O PIN nunca é gravado em claro, nem mesmo dentro
  do corpo. A comparação em tempo constante fica na Fase 22. A Fase 19b também faz o
  `make dev-cofre` gravar o hash do PIN de teste de D15. Ficou de fora fazer isso só na
  Fase 22: o formato ficaria aberto até o M3, e o M2 fecharia sem o formato final.
- D31. **`unsafe` em teste de integração** (handoff 15a). A regra do worker é "`unsafe`
  só em módulo de plataforma". Ela vale para código de produção. Fica aceita uma
  exceção documentada: o `#[global_allocator]` de teste em
  `crates/ward-core/tests/corpo_memoria.rs` (V2.33). Nenhum outro arquivo pode usar
  essa exceção. O `#![allow(unsafe_code)]` vale só naquele binário de teste, cada bloco
  tem `// SAFETY:` e nada dele entra em binário de produção. Os crates de produção
  continuam sem `unsafe` fora do `ward-plataforma`. A Fase 19a troca o `alloc` desse
  alocador por `alloc_zeroed`, para a procura não ler bytes nunca escritos (memória não
  inicializada no modelo do Rust). O security-auditor confere que a exceção não passou
  para outro arquivo.
- D32. **Limites numa fonte só** (handoff 15a). A Fase 19a cria `ward_core::limites`
  com constantes `pub`, já que limite não é segredo. Validação, corpo, orçamento da
  máscara (D29) e, depois, protocolo (Fase 20) e app (Fase 34) usam essas constantes.
  O `corpo.rs` deixa de repetir os números de D25.
- D33. **Banco: formato da base e escolhas da Fase 16** (handoff 16). A validação passa
  a restringir a `base` (Fase 19a), em vez de depender do percent-encoding da entrega:
  `[A-Za-z0-9_-]`, de 1 a 63 caracteres (63 é o limite do Postgres), em todo motor
  menos `redis`, onde a base é um número de 0 a 99999. `?`, `#`, `/`, `.`, espaço e
  `%` são recusados com mensagem em português. Com isso, `PGDATABASE` e as demais
  recebem sempre um nome limpo. O percent-encoding na URL fica como defesa extra.
  Ficam confirmadas as escolhas da Fase 16: `SQLCMDSERVER=host,porta` (sintaxe do
  `sqlcmd`); a base do sqlserver no URL como `?database=`; no redis, o usuário só no
  URL (D10 não tem `REDIS_USER`) e a base, que agora é sempre número, sempre no URL;
  `WARD_TOTP` e `WARD_ARQUIVO_*` não entram na máscara (D11: o código TOTP não é
  mascarado; o caminho não é segredo, o conteúdo do anexo é).
- D34. **Máscara: `CHAVE=valor` e saída interativa** (handoff 17).
  - Em linha `CHAVE=valor` de anexo, o **valor só vira padrão próprio com 6 caracteres
    ou mais**, o mesmo mínimo das linhas (D11). Sem mínimo, `DEBUG=1` ou `PORT=8080`
    fariam todo `1` ou todo `8080` da saída virar `••••`. A linha inteira continua
    mascarada. Um valor curto que aparece sozinho na saída não é mascarado. Esse risco
    está em RA3 e é aceito. O segredo de campo (senha, token etc.) segue mascarado em
    qualquer tamanho, como manda D11.
  - **Saída interativa do `ward ssh` (Fase 27)**: no pty, um fim segurado que pode ser
    começo de segredo é solto depois de 300 ms sem saída nova, sempre como `••••` e
    nunca em claro. O autômato guarda o estado do casamento: se a continuação completa
    o segredo, nada mais sai; se diverge, o resto sai normal. O preço é mascarar a mais
    um prefixo inocente, como um prompt que termina em `f`. Sem pty (`ward run`,
    `ward ssh` com comando), segue segurando até chegar mais saída ou até o fim.
- D35. **Gerador e TOTP** (handoff 18; "Credenciais" › "Gerador de senha", "TOTP").
  - **Símbolos do gerador**: `!#$%&()*+,-./:;<=>?@[]^_{|}~`. Ficam fora espaço, `"`,
    `'`, crase e `\`, que quebram aspas de shell, `.env` e arquivos de configuração. O
    app (Fases 33 e 37) mostra o mesmo conjunto, e "sem símbolos" segue para os painéis
    que não aceitam nenhum.
  - **TOTP só SHA1, 6 dígitos e 30 s** na v1. Para não gerar código errado sem aviso, o
    app (Fase 33) e a extensão recusam ao salvar um `otpauth://` com `algorithm`,
    `digits` ou `period` diferentes, com a mensagem "parâmetros de TOTP não suportados".
    Algoritmo, dígitos e período no modelo ficam para a v2.
  - **Semente mínima de 80 bits** (16 caracteres base32), como a Fase 15 aceita. Quem
    escolhe a semente é o provedor, não o Ward, e muitos usam 80 bits. Recusar sementes
    menores que os 128 bits do RFC 4226 deixaria contas reais sem TOTP.

Decisões do planner sobre a auditoria de segurança do M2 (2026-11-08). O M2 foi
aprovado com 4 achados médios e 5 baixos. As repros estão no scratchpad do auditor
(`ataque/src/bin/`). O código muda na Fase 19c, no fim do M2 e antes da Fase 20. O que
cabe ao daemon fica anotado nas Fases 22, 27 e 45.

- D36. **Máscara por linha em segredo de várias linhas** (achado médio 1; "Segurança" ›
  S4). A chave privada de `servidor` e de `ftp/sftp` e os extras secretos com `\n` eram
  mascarados só pelo texto inteiro. Com isso, `grep -v -- -----`, `head -n 4` e a saída
  com CRLF do pty deixavam passar quase toda a chave. Todo segredo de campo que contém
  `\n` passa a usar também a passada por linha de `Gerador::anexo`, pela mesma função e
  com as mesmas regras:
  - cada linha com 6 caracteres ou mais, com e sem `\r` no fim e aparada;
  - o valor de `CHAVE=valor` com o mínimo de D34;
  - no mesmo passo, e também nos anexos, o valor de linhas `"chave": "valor"` (JSON) e
    `chave: valor` (YAML), com e sem aspas e com o mesmo mínimo. Assim
    `jq -r .private_key` de um JSON de conta de serviço e o `token:` de um kubeconfig
    ficam cobertos.
  O segredo continua mascarado também inteiro e nas variações de D11. **Orçamento de
  D29 recontado:** o custo sobe porque `custo_da_entrada` usa o mesmo `Gerador`. Os 4
  MiB por entrada e os 8 MiB da máscara não mudam. A Fase 19c refaz o V2.36 (maior
  entrada de cada tipo, par de D19) e confere que as 21 entradas de D15 continuam
  aceitas. Se uma entrada do cofre de teste passar a ser recusada, a fase para e
  devolve o caso ao planner, sem afrouxar o orçamento.
- D37. **Variações escapadas** (achado baixo 5; D11). O gerador emite `$&<>` (D35), e
  três ferramentas comuns escrevem o segredo de forma diferente da literal. Para cada
  segredo, a máscara passa a gerar mais três formas, cada uma só quando é diferente do
  valor literal:
  - **JSON do Go**: `&`, `<`, `>`, U+2028 e U+2029 como `\u0026`, `\u003c`,
    `\u003e`, `\u2028` e `\u2029`, com hex em minúsculas, junto com o escape JSON que já existe
    (`\"`, `\\`);
  - **JSON com `ensure_ascii`** (Python): todo caractere fora do ASCII como `\uXXXX` em
    minúsculas, com par substituto acima de U+FFFF (`😀` vira `\ud83d\ude00`);
  - **aspas duplas de shell** (`declare -p`, `export -p`): `\` antes de `$`, `` ` ``,
    `"` e `\`.
  O hex em maiúsculas (`\u00E7`) fica em RA3. O custo entra no orçamento de D29 pela
  mesma função.
- D38. **Texto: lista branca por campo** (achado médio 3; D25 "Texto não confiável";
  "Claude (agente)" › "Janela de aprovação"). A lista de recusa de D25 prometia cobrir
  a categoria Unicode Cf e não cobria (U+0600, U+1D173, U+13430, U+1BCA0). Ela também
  deixava passar preenchedores (U+3164, U+115F, U+FFA0, U+2800), seletores de variante
  (U+FE0F, U+180B, U+E0100) e U+034F. Recusar por categoria geral exigiria tabela
  Unicode ou dependência nova (S14) e continuaria perseguindo casos novos. Fica então
  uma **lista branca por campo**, sem dependência nova:
  - **texto de exibição** (`grupo`, `projeto`, tags, nome de extra, nome de anexo,
    `usuário` do `login` e valor de extra não secreto): só ASCII visível (U+0021 a
    U+007E), espaço U+0020 no meio (nunca nas pontas) e letras latinas pré-compostas
    de U+00C0 a U+017F, menos `×` (U+00D7) e `÷` (U+00F7). Cobre o português e as
    línguas latinas. Marca combinante (`a` + U+0301), espaço fora do ASCII (U+00A0,
    U+3000), emoji, Braille e qualquer outro caractere são recusados. Quem normaliza
    para NFC é o app (Fase 33), como a semente TOTP de D25;
  - **campos técnicos** (host, usuário fora do `login`, URL, variável, base, região,
    access key, ARN, UUID, nome da entrada): só ASCII, cada um com o formato que já tem
    (D25, D33), mais D41, D42 e D43;
  - **segredos** (senha, token, chave, passphrase, semente, nota, valor de extra
    secreto e anexos): a regra de hoje (UTF-8, sem nulo; anexo em bytes). Eles não são
    exibidos fora do "revelar".
  A mensagem é a de hoje ("caractere não permitido"), sem o valor. A leitura do corpo
  aplica a mesma regra, porque passa por `Rascunho::validar` (D26).
- D39. **Contador de geração contra rollback** (achado médio 2; "Backup";
  "Segurança" › "Modelo de ameaça" 2). Hoje um processo do mesmo uid pode trocar o
  arquivo por uma versão autêntica mais antiga (um backup de D17) e assim voltar um
  nível de acesso ou o hash do PIN. A cifra não acusa a troca, porque a versão antiga
  é autêntica.
  - **Na Fase 19c (núcleo):** a seção de configuração ganha a `geracao` (u64, a partir
    de 1). Por isso o corpo vai para a **versão 3**, e a 2 passa a ser desconhecida
    (não há cofre real, como em D30). Toda gravação que parte de um `CofreAberto`
    (`gravar_entradas`, `trocar_senha`) grava `geracao + 1`. O `CofreAberto` só passa
    a ter o número novo depois que a gravação dá certo. Em `u64::MAX`, a gravação é
    recusada. O número pode ser lido (`CofreAberto::geracao()`), mas não existe setter
    público: `configuracao_mut` não mexe nele. A abertura ganha uma variante com
    `geracao_minima`. Abaixo dela, a abertura falha com um erro próprio, `Retrocesso`
    ("o cofre no disco é mais antigo que o último aberto"). Ele não leva valor e só
    aparece depois de decifrar. Geração 0 no corpo é recusada com o erro genérico. O
    `make dev-cofre` grava a geração 1.
  - **Na Fase 22 (M3, daemon):** o daemon guarda a maior geração vista num arquivo de
    estado ao lado do cofre (`~/.local/share/ward/estado`, `.dev/` nos testes; 0600,
    sem segredo, gravação atômica). Ele atualiza o arquivo a cada destravar e a cada
    gravação e abre sempre com `geracao_minima`. Com `Retrocesso`, o cofre fica
    travado e a mensagem aparece no estado de erro que a tela Destravar já tem, sem
    tela nova. Se o arquivo de estado faltar ou estiver ilegível com o cofre já
    existente, o daemon também recusa destravar com o mesmo aviso; só o primeiro uso
    cria o estado. O daemon guarda também `dev`, inode, `mtime` e tamanho do cofre
    depois da própria leitura ou gravação, lidos por `fstat` do fd aberto. Antes de
    gravar ou reabrir, ele confere esses valores. Se mudaram, ele não grava por cima,
    trava e avisa: "o arquivo do cofre mudou fora do Ward".
  - **Na Fase 45 (M7):** restaurar continua só pelo papel app (D17, V7.8). A versão
    restaurada é selada de novo com `geracao` = maior vista + 1. Restaurar, portanto,
    nunca baixa a geração. Copiar um backup por cima do arquivo, à mão, cai no
    `Retrocesso`.
  - O que sobra está em RA4, fora do modelo de ameaça por decisão do usuário.
- D40. **Chave de dados e KEK no heap** (achado médio 4; S8, D14). A chave de dados
  (`Zeroizing<[u8;32]>`) era movida por valor várias vezes (`chave_aleatoria`,
  `ChavesCofre`, `CofreAberto::de_partes`), e cada movimento pode deixar uma cópia na
  pilha que ninguém zera. `cripto::Chave` passa a ser um newtype sobre `Box<Zeroizing<[u8; 32]>>` (o `zeroize`
  não dá `ZeroizeOnDrop` a `Box<T>`; handoff 19c), para
  a chave de dados e para a KEK derivada pelo Argon2id. Assim, mover passa só o
  ponteiro, e os bytes vivem num bloco do heap que zera ao liberar e que o alocador do
  V2.33 enxerga. Uma asserção de compilação prende `size_of::<Chave>()` ao tamanho de
  um ponteiro. Em `ler_phc_pin`, o `buf` vira `Zeroizing`. A saída recalculada em
  `conferir_pin` fica num `Zeroizing<[u8; 32]>` e é comparada contra a guardada pelo
  `CtEq` do `ctutils` (a comparação continua da biblioteca, V2.7). Os tipos `Copy` do
  `phc` (`Output`, `Salt`) ficam só para ler e escrever o texto PHC guardado, que já é
  o conteúdo do `HashPin`. O resíduo que sobra na pilha (cópias internas do Argon2 e do
  `phc`) é o mesmo de D14.
- D41. **URL por parser estrito** (achado baixo 6; D9). `url_valida` aceitava userinfo,
  `\`, `%2f` antes do `@`, porta 99999 e `http` em qualquer host. Ela passa a ser um
  parser estrito, sem dependência nova, para a URL do `login` e a do `api`:
  - esquema `https://`, ou `http://` só com host `localhost` ou `127.0.0.1` (a regra de
    D9, valendo já ao salvar e também no `api`, porque o token iria em claro pela
    rede);
  - URL só em ASCII visível. Domínio em minúsculas, em rótulos `[a-z0-9-]` de 1 a 63
    caracteres que não começam nem terminam em `-`, com até 253 no total; nome
    internacional só em punycode `xn--`, e quem converte é o app (Fase 33). Também
    vale IPv4 de 4 octetos sem zero à esquerda ou IPv6 entre colchetes, lido por
    `std::net`. Host só de dígitos e pontos é lido como IPv4 e nunca como domínio, e
    rótulo vazio (`x..example`) é recusado;
  - nada de userinfo: `@`, `\` ou `%` antes do fim da autoridade são recusados;
  - porta opcional de 1 a 65535, sem zero à esquerda;
  - caminho, query e fragmento em ASCII visível sem `\`, e todo `%` seguido de dois
    dígitos hex.
  Esquema ou host em maiúsculas são recusados, porque a URL chega normalizada pelo
  app. Um `http://` de API interna fica para quando o usuário pedir.
- D42. **Variável do `api` por lista branca de sufixo** (achado baixo 7; D25; S3).
  Passavam `GIT_PROXY_COMMAND`, `GIT_EXTERNAL_DIFF`, `PAGER`, `LESSOPEN`, `PS0`,
  `JAVA_TOOL_OPTIONS`, `OPENSSL_CONF`, `SSLKEYLOGFILE` e `HTTPS_PROXY`. Ampliar a lista
  de recusa perseguiria o caso seguinte para sempre. Por isso a variável escolhida
  passa a valer só no formato `[A-Z][A-Z0-9_]*` e terminando em `_TOKEN`, `_KEY`,
  `_SECRET`, `_PASSWORD`, `_PAT` ou `_AUTH`. Isso cobre `GITHUB_TOKEN`, `GH_TOKEN`,
  `NPM_TOKEN`, `OPENAI_API_KEY`, `STRIPE_SECRET_KEY`, `DOCKER_PASSWORD` e
  `NPM_CONFIG__AUTH`. A lista de recusa de D25 continua como segunda camada (`WARD_*`,
  `LD_*`, `BASH_FUNC_*`, `*ASKPASS*` etc.), e também a regra de não repetir variável do
  tipo (D10). Mensagem: "a variável precisa terminar em _TOKEN, _KEY, _SECRET,
  _PASSWORD, _PAT ou _AUTH". Um nome fora disso (ex.: `TF_TOKEN_app_terraform_io`)
  fica para quando o usuário pedir.
- D43. **Usuário de `servidor` e `ftp/sftp`** (achado baixo 8; D13). O usuário aceitava
  `@`, `;`, `'$(id)'` e `%h`. As duas defesas valem juntas:
  - **validação (Fase 19c):** `[A-Za-z0-9._@-]`, sem `-` no começo, até o limite de
    usuário de `ward_core::limites`. O `@` vale já na v1 para conta de domínio
    (`deploy@cliente.com.br`; decisão do usuário, 2026-11-08). Vale para o usuário da
    entrada e para o do salto, e a leitura do corpo aplica a mesma regra;
  - **entrega (Fase 27, M4):** o daemon passa o usuário só como argumento próprio de
    `-l <usuário>`, com o host depois de `--`, por `execve` sem shell. Nunca como
    `usuário@host` nem como `-o User=`, que no `ssh_config` expande `%`. Com `@` no
    usuário isso é obrigatório: `deploy@cliente.com.br@host` seria lido pelo `ssh` de
    outro jeito. A Fase 27 tem teste com usuário que contém `@`. O `ProxyJump` é
    montado só com usuário e host já validados; com `@` no usuário do salto, ele não
    pode ir como `usuário@host` na linha do `ProxyJump` (a Fase 28 usa um `ssh_config`
    gerado com `User` literal sem `%`, ou recusa o salto com `@`).
  - **`banco` e `smtp` (confirmado pelo planner em 2026-11-08, depois da 19c):** o
    usuário é campo técnico de D38, só ASCII visível (U+0021 a U+007E, sem espaço) e
    sem `-` no começo, para não virar opção de `psql -U` ou de cliente de e-mail.
    Não segue a lista de `servidor` porque o usuário de smtp costuma ser um e-mail e
    o de banco pode ter outros sinais; no `DATABASE_URL` ele já sai percent-encoded
    (D10). A leitura do corpo aplica a mesma regra.
  - O limite é `MAX_USUARIO` em `ward_core::limites` (hoje igual a `MAX_LINHA`, 2
    KiB). Decisão provisória: fica assim; baixar para 256 é uma linha, se o usuário
    pedir.
- D44. **Gravação pública só a partir de entradas** (achado baixo 9; D27). `gravar_cofre`
  era público e selava qualquer corpo, inclusive um sem hash de PIN. Ele passa a ser
  `pub(crate)`, e `CofreAberto::selar` também. Fora do `ward-core`, as únicas
  gravações públicas ficam sendo `gravar_entradas` e `gravacao::trocar_senha`, que
  serializam ou validam o corpo antes de selar. O teste `tests/corpo_memoria.rs`, que
  usava `selar`, passa a gravar por `gravar_entradas` num diretório temporário.
  Doctests `compile_fail` cobrem os dois.
- D45. **Escolhas das Fases 19a e 19b, confirmadas pelo planner** ("Destravar e
  travar"; "Backup"; D30, D33):
  - trava por inatividade de 1 a 1440 minutos, sem "nunca", porque o escopo manda
    travar sozinho;
  - hash do PIN com 64 MiB, 3 passes e 1 faixa. A leitura aceita de `KDF_PIN` até 256
    MiB, 16 passes e 4 faixas: o piso barra o rebaixamento e o teto barra o hash
    inflado que travaria o daemon. Sal de 16 bytes e saída de 32;
  - UUID do pendrive com 1 a 36 caracteres `[0-9A-Fa-f-]`, sem `-` nas pontas;
  - texto PHC canônico (escrito de novo, dá os mesmos bytes);
  - base do `redis` sem zero à esquerda (`0` vale, `00` e `03` não), para haver uma
    forma só de cada número;
  - a mensagem de `ErroCorpo::GrandeDemais` cita "64 MiB e 10 000". A Fase 19c a põe no
    teste `mensagens_citam_os_limites_atuais`, ou a monta a partir de
    `ward_core::limites`, para que ela não fique desatualizada se a constante mudar.

- D46. **Conferência do servidor pelo socket do sistema** (**provisória**, Fase 21;
  depende do usuário porque muda "Arquitetura" › "systemd do usuário"; S5, S8, D5). Com
  o `wardd` não-dumpable (S8), `exe`, `environ` e `maps` dele ficam ilegíveis para o
  mesmo uid (EACCES), e a conferência de D5 pelo `/proc` falha fechada contra o daemon
  real. Também não há segredo que o daemon guarde e um processo do mesmo uid não leia.
  Proposta: o socket deixa de ser criado pelo próprio `wardd`. Um par
  `wardd@.socket`/`wardd@.service` **do systemd do sistema** cria o socket em
  `/run/ward/<uid>/wardd.sock`, com o diretório root:root 0755 e o socket do usuário
  0600 (`SocketUser=`, `SocketMode=0600`), e entrega o fd ao `wardd` (`LISTEN_FDS`),
  que roda com `User=<uid>`. A CLI e o app conferem, antes de mandar a senha mestra:
  - toda pasta do caminho é do root e sem escrita para grupo e outros, sem link;
  - o socket é do próprio uid e 0600;
  - o `SO_PEERCRED` do servidor tem **uid 0**. Para quem conecta, o kernel devolve a
    credencial de quem fez o `listen`, que foi o systemd do sistema. Nenhum processo do
    usuário cria um socket que escuta com uid 0, nem pega o fd do `wardd`
    (`pidfd_getfd` e `/proc/<pid>/fd` exigem ptrace, negado pelo S8).
  Custo: a instalação (já com `sudo`, D18) passa a instalar as duas units do sistema e a
  rodar `systemctl enable --now wardd@<uid>.socket`; o daemon sobe na primeira conexão
  e não herda o ambiente da sessão, então abre o app pelo barramento do usuário
  (`systemd-run --user` ou uma unit `ward-app.service`); `systemctl --user
  set-environment` deixa de alcançar o daemon (V9.4 muda de forma, S8 fica mais forte).
  O daemon de teste (D7) continua no diretório do usuário e a CLI só pula a conferência
  do uid 0 no artefato `modo-teste`, que a instalação recusa (V9.2). Resíduo: um
  processo do mesmo uid ainda mata o `wardd` (negação de serviço; o systemd o sobe de
  novo), sem passar a receber a senha. Opções descartadas: rótulo do AppArmor por
  `SO_PEERSEC` (um processo não confinado pode trocar para qualquer perfil carregado,
  então o rótulo é forjável, e confinar o `wardd` atrapalha o `ward run`, que executa
  qualquer comando); nome do processo (`Name: wardd` no `status`), que qualquer um
  forja; aceitar o risco, que entrega a senha mestra, e com ela o arquivo do cofre, a um
  pacote malicioso (ameaça 2). Se o usuário recusar D46, S5 fica sem a conferência do
  servidor e isso vai para "Fora do modelo de ameaça" no escopo.
- D47. **App e host da extensão identificados por grupo setgid** (**provisória**, Fase
  21; depende do usuário porque cria grupos e binários setgid na instalação e registra
  um resíduo novo; S1, S8, D5). A identificação de D5 lê o `/proc` do cliente, o que
  obriga o app e o `ward-nm` a ficarem dumpable até o daemon responder. Nessa janela,
  qualquer ancestral (inclusive um pacote malicioso que lance ele mesmo o
  `/usr/local/bin/ward-app`, o que o Yama `ptrace_scope=1` permite) se prende, injeta
  código, se solta e herda o papel app. O `TracerPid` só pega o depurador preso, e
  código escrito por `/proc/<pid>/mem` numa biblioteca do root não aparece no `maps`.
  Proposta:
  - `ward-app` instalado root:`ward-app` 2755 e `ward-nm` root:`ward-nm` 2755, com dois
    grupos de sistema sem membros e sem permissão em arquivo nenhum;
  - o `exec` com setgid deixa o processo não-dumpable desde a primeira instrução (o
    kernel aplica `fs.suid_dumpable`, que não pode ser 1) e põe o `ld.so` em modo seguro
    (`AT_SECURE`), que ignora `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`,
    `GLIBC_TUNABLES`, `GCONV_PATH` e o resto da lista insegura da glibc;
  - o daemon dá o papel pelo **egid do `SO_PEERCRED`** (`ward-app` → app, `ward-nm` →
    extensão, qualquer outro → agente), que o kernel fixa no `connect` e que o usuário
    não forja sem root (num namespace de usuário o gid vira 65534);
  - o app conecta antes de iniciar o GTK, depois larga o grupo (`setresgid` para o gid
    real, porque o GTK 3 se recusa a iniciar com setgid) e chama `PR_SET_DUMPABLE=0` de
    novo; se a conexão cair, ele se executa de novo para reconectar;
  - o app só recebe o papel se o pai for o `systemd --user` do usuário ou o próprio
    daemon (lido do `PPid` no `status`, que continua legível). Lançado por outro
    processo, ele pede ao daemon que abra a instância verdadeira e sai. Antes do GTK, o
    app limpa o próprio ambiente por lista branca (`WAYLAND_DISPLAY`, `DISPLAY`,
    `XDG_RUNTIME_DIR` igual a `/run/user/<uid>`, `DBUS_SESSION_BUS_ADDRESS`,
    `XDG_SESSION_TYPE`, `LANG`/`LC_*`, `HOME`, `PATH` fixo), porque os processos do
    WebKitGTK herdam o ambiente sem `AT_SECURE`; o fd do socket é `CLOEXEC`;
  - o daemon recusa o papel app e extensão com `kernel.yama.ptrace_scope` = 0 ou
    `fs.suid_dumpable` = 1 (falha fechada, com aviso);
  - no `ward-nm`, o daemon continua conferindo o pai Chrome pelo `/proc` (exe, dono
    root), porque o Chrome não é setgid. Se o processo do navegador for não-dumpable,
    a conferência passa a usar só o `PPid` e o `Name` do `status` do pai mais o dono do
    exe instalado; confirmar na Fase 40 com o Chrome real (handoff 21).
  Custo: dois grupos e dois binários setgid na instalação (Fase 51, `.deb` na 52); o
  daemon deixa de ler `exe`, `environ` e `maps` do app e do host, e a parte
  correspondente da Fase 21 fica só para o pai Chrome e para a cadeia exibida. Resíduo
  (RA6): os processos auxiliares do WebKitGTK e o próprio Chrome não são endurecidos;
  eles ficam protegidos de quem não é ancestral pelo Yama, e o ancestral é o systemd ou
  o daemon. Um pacote que mate o Chrome e o lance de novo como filho vira ancestral do
  Chrome e pode se prender a ele; um Chrome assim pode mentir a origem da aba e
  receber o preenchimento de qualquer entrada do tipo login. É o mesmo caso de "Fora do
  modelo de ameaça" › leitura do campo de senha por um script feito para isso, agora
  dito com o ptrace, e só sai do modelo se o usuário aceitar. Se o
  usuário recusar D47, vale D5 com a janela de endurecimento registrada em "Fora do
  modelo de ameaça".
- D48. **`LD_*` e bibliotecas** (Fase 21; S8, D5, D47).
  - `wardd`: recusa **qualquer** `LD_*` e também `GLIBC_TUNABLES`, `GCONV_PATH`,
    `MALLOC_TRACE` e `LOCPATH` (a lista insegura da glibc é o "e similares" de S8). O
    ambiente do daemon vem da unit, então a regra estrita não custa nada.
  - App e host com D47: o daemon não confere o ambiente nem o `maps` deles; quem
    neutraliza `LD_*` é o `AT_SECURE`, e a lista branca de D47 cuida do que o GTK, o GIO
    e o GStreamer leriam do ambiente. Quem tem `LD_LIBRARY_PATH` na sessão continua
    usando o app normalmente.
  - Sem D47 (usuário recusa): fica a regra da Fase 21 (qualquer `LD_*` e toda
    biblioteca executável do root), com teste na Fase 29 contra o WebKitGTK real
    (módulos do GTK, plugins do GStreamer em `~/.local/share/gstreamer-1.0`, IBus) e na
    Fase 40 contra o Chrome real. Se algo legítimo de fora do root for mapeado, o app
    pré-define o ambiente que desliga essa fonte (ex.: `GST_PLUGIN_SYSTEM_PATH` e
    `GST_PLUGIN_PATH` vazios) em vez de afrouxar a regra.
- D49. **V3.2 em Docker** (Fase 21; S5). Sem root não há processo de outro uid no
  host, e com o diretório 0700 o uid 4242 nem chega ao socket. O teste roda num
  contêiner `ubuntu:24.04` (mesma glibc do Pop!_OS 24.04 e mesmo kernel do host), que
  não monta nada do `/run/user` do host. O root do contêiner cria `/run/user/1000`
  (0700, uid 1000) e sobe o `wardd` de teste com `setpriv --reuid 1000 --regid 1000
  --clear-groups`. Três clientes em Python:
  - uid 4242 sem capacidade → `connect` falha com EACCES (primeira camada);
  - uid 4242 com `CAP_DAC_OVERRIDE` ambiente (`setpriv --reuid 4242 --inh-caps
    +dac_override --ambient-caps +dac_override`) → passa pelo 0700 e pelo 0600, e o
    daemon fecha sem mandar nenhum byte e registra `UidRecusado { uid: 4242 }` (segunda
    camada, `SO_PEERCRED`);
  - uid 0 → mesma recusa.
  Ao fim, um cliente do uid 1000 recebe resposta normal. Alvo `make teste-docker-uid`,
  fora do `make check` (exige Docker), rodado pela Fase 22 e pelo validador do M3.
- D50. **Operações do app sem PIN e `lista` do agente** (Fase 20, confirmadas pelo
  planner; "Destravar e travar" › PIN "só aprova usos e mudanças de nível"; "Claude
  (agente)" › "Comandos").
  - `remover_entrada` e `tempo_trava` não pedem PIN. São só do papel app, que é o
    usuário, e o escopo limita o PIN a aprovar usos e mudar nível. O tempo de trava
    segue de 1 a 1440 minutos (D45).
  - Entrada criada ou recriada nasce sempre com o nível padrão ("com aprovação"). Criar
    já com outro nível conta como mudar nível e pede PIN (S9); remover e recriar não
    serve para fugir do PIN.
  - A `lista` do papel agente **não leva o nível**. O escopo dá só nome, tipo, grupo,
    ambiente, projeto e URL/host. Saber quais entradas são "livres" deixaria um agente
    sob prompt injection escolher as que não abrem janela, e o pedido de uma entrada
    com aprovação é justamente o que avisa o usuário. A Fase 22 tira o campo da resposta
    do agente (o app continua vendo o nível). Mudar isso é decisão do usuário no escopo.
- D51. **Texto do agente na janela de aprovação** (Fase 20; "Claude (agente)" ›
  "Janela de aprovação"; D38). `comando` e `cadeia` do `PedidoDeAprovacao` chegam
  exatos, sem filtro, porque a janela mostra o comando exato. Quem exibe (Fase 23 no
  texto que o daemon monta, Fase 36 na janela) neutraliza sem esconder: caractere de
  controle (C0, C1, `DEL`) e de formatação Unicode (bidi U+202A–U+202E e
  U+2066–U+2069, largura zero, tags, BOM) aparecem como escape visível (`‮`,
  `\x1b`), cada argumento entre aspas e separado, quebra de linha como `\n`, e texto
  longo cortado com aviso, nunca em silêncio. O que o usuário aprova é o comando que o
  daemon executa, byte a byte.
- D52. **Notas de implementação da Fase 20.**
  - Na troca de senha (Fase 22, D45), `ErroGravacao::Formato` vindo de `trocar_senha`
    responde `senha_incorreta`; em `gravar_entradas` continua `falha_ao_gravar`.
  - `MAX_QUADRO_COMUM` (1 MiB) limita argv + ambiente do `ward run` a cerca de 750 KiB
    depois do base64. A CLI (Fase 26) mede antes de mandar e, se passar, sai ≠ 0 com
    "o comando e o ambiente passam de 750 KiB; reduza o ambiente (ex.: `env -i`)",
    sem citar valor; o daemon segue recusando com `quadro_invalido`.

### Riscos abertos

- **RA1. `/proc/<pid>/environ` do filho** (decidir antes de fechar o M4). Enquanto o
  filho do `ward run` (ou o `sshpass` do `ward ssh`) roda, outro processo do mesmo
  usuário pode ler as variáveis dele em `/proc/<pid>/environ` e os anexos 0600 em
  `/run/user/<uid>/ward/tmp/`, porque o kernel trata processos do mesmo uid como
  seus. Isso afeta as ameaças 1 e 2. A Fase 26 pesquisa
  uma mitigação (filho sem dump, namespaces, entrega por fd quando a ferramenta
  aceita) e registra o resultado. Sem mitigação que funcione, a asserção V4.13 reprova
  e o usuário decide entre aceitar o risco (vai para "Fora do modelo" no escopo) ou
  mudar a entrega.
- **RA2. Caminho do usuário no `ward ssh`** (confirmar antes do M4). D3 é a leitura
  segura de "quem chamou é conhecido pelo próprio `ward ssh`". Se o usuário quiser
  `ward ssh` sem PIN digitado direto no terminal, não há sinal que o Claude não forje,
  e o nível vira opcional para o Claude também; o escopo teria de registrar esse
  limite.
- **RA3. Limites conhecidos da máscara** (handoff 17; aceito pelo planner em
  2026-11-08). A máscara cobre o que D11 declara (S4) e algumas variações a mais. Ela
  não cobre: base64 quebrado em linhas (`base64 -w76` de um anexo), hex (`xxd`),
  percent-encoding duplo (`%253A`), entidades HTML, metade do segredo no stdout e
  metade no stderr, e o valor curto de `CHAVE=valor` (D34). Fica como risco aceito, sem
  trabalho novo, por dois motivos. Primeiro, um processo que recodifica o segredo de
  propósito está "Fora do modelo de ameaça", e quem barra é a aprovação, que mostra o
  comando. Segundo, para o erro sem má intenção, a skill (Fase 50, V8.2) manda nunca
  imprimir anexo nem variável de credencial, em nenhuma codificação. Se um caso
  desses aparecer em uso normal, ele vira variação nova de D11. Depois da auditoria do
  M2 (D36, D37), a lista ganha dois casos que continuam sem cobertura: `\u` com hex
  em maiúsculas e o valor JSON com escapes que o `jq -r` decodifica (a `private_key`
  com `\n` vira várias linhas, e só as linhas que aparecem iguais no segredo são
  mascaradas).
- **RA4. Rollback com o daemon parado** (D39; **fechado em 2026-11-08: fora do modelo
  de ameaça por decisão do usuário**, registrado em "Segurança" › "Fora do modelo de
  ameaça"; não há trabalho nem asserção nova). O contador de geração e o arquivo de estado pegam a troca do cofre
  enquanto o daemon roda e entre um destravar e outro. Um processo do mesmo uid que
  troque, com o daemon parado, **o cofre e o arquivo de estado** por cópias antigas e
  coerentes passa pela conferência. Não há onde guardar o número fora do alcance do
  mesmo uid sem root ou TPM, e o TPM está fora ("Destravar e travar"). O impacto é
  limitado: nenhuma operação passa a devolver segredo (S1), e o que volta são níveis
  e o hash antigo do PIN. O PIN antigo só serve a quem também o conhece. A V3.22
  continua cobrindo o que o daemon pega (troca só do cofre, estado apagado, troca com
  o daemon rodando).
- **RA5. Conferência do servidor (S5)** (Fase 21; **aberto, depende do usuário**). Ver
  D46. Até a resposta, a Fase 22 aceita o socket por `LISTEN_FDS` e também o criado pelo
  próprio daemon (só no daemon de teste), e a Fase 25 não fecha V4 sem a conferência.
- **RA6. Janela de endurecimento do app e do host** (Fase 21; **aberto, depende do
  usuário**). Ver D47. Com D47, sobra o resíduo dos auxiliares do WebKitGTK e do Chrome
  (protegidos só pelo Yama contra quem não é ancestral). Sem D47, sobra a janela entre
  a identificação e o `PR_SET_DUMPABLE=0`, em que um ancestral malicioso injeta código.
  Qualquer dos dois resíduos vai para "Fora do modelo de ameaça" se o usuário aceitar.

## Critérios de aceite

- [ ] M0: `Ward.figcraft` (FigCraft) com tokens como variables, os componentes base e
  todas as telas e estados de "Missão 0 — Design no FigCraft", no estilo arredondado
  (D23), com instâncias do Design System; arquivo salvo; mapa de nodes em
  `docs/mission/design-nodes.md`; aprovado pelo usuário no FigCraft.
- [ ] M1: `make check` (fmt, clippy `-D warnings`, testes, `cargo audit`, `cargo deny`,
  gitleaks) passa limpo; `docker compose` de teste sobe Postgres, SSH (senha, chave e
  salto) e WordPress; comandos escritos no `CLAUDE.md`.
- [ ] M2: formato cifrado com Argon2id + XChaCha20-Poly1305 que recusa senha errada e
  qualquer byte alterado; gravação atômica; os 9 tipos com a entrega de D10; máscara
  de D11; TOTP e gerador com vetores conhecidos; corpo do cofre serializado (D26) com ida
  e volta, recusa de adulteração e limites; `gravar` fora da API pública (D27);
  `make dev-cofre` cria o cofre de teste; limites numa fonte só, com anexos e
  orçamento da máscara de D29 e base do banco de D33; corpo com o hash do PIN na
  seção de configuração (D30); correções da auditoria do M2 (D36–D45): máscara por
  linha e formas escapadas, texto por lista branca, geração contra rollback no corpo
  v3, chave no heap, URL estrita, variável do `api` por sufixo, usuário SSH restrito e
  `gravar_cofre`/`selar` fora da API pública.
- [ ] M3: `wardd` endurecido (S5, S8), papéis de D5, destravar/travar, trava
  automática, PIN, níveis e o motor de aprovação (S9, S10) testados em Rust; nenhuma
  operação do socket devolve segredo a um cliente comum; V3.2 em Docker (D49);
  conferência do servidor e papel do app/host por D46/D47 ou risco aceito no escopo
  (RA5, RA6).
- [ ] M4: `ward lista/run/ssh/travar/destravar` contra Postgres e SSH de teste (senha,
  chave com agente temporário, salto, túnel de banco), saída mascarada, anexos
  temporários limpos, nada lido ou gravado em `~/.ssh`; RA1 e RA2 resolvidos.
- [ ] M5: app Tauri com todas as telas do M0, janela de aprovação com os 4 botões e PIN,
  bandeja, "Conectar" do `servidor` (D3), S11 e S12 conferidas; roteiro da janela
  nativa conferido pelo usuário.
- [ ] M6: extensão instalada por política, preenchimento por origem, salvar login novo,
  `ward login` e `ward playwright` no WordPress de teste, phishing recusado.
- [ ] M7: backup local de 30 versões, externo em pendrive desligado por padrão,
  restaurar e kit de emergência.
- [ ] M8: skill `ward` nova com fluxo e deny rules, pronta para instalar.
- [ ] M9: `sudo make install` e `.deb` instalam tudo em pastas de root; o `wardd`
  instalado sobe com a sessão; nada em `~/.ssh` nem no `SSH_AUTH_SOCK` muda.
- [ ] M10: etapa final guiada documentada e verificável (mover chaves pessoais do
  KeePassXC para o `~/.ssh` e as de servidor para entradas `servidor`, gnome-keyring,
  `montagem.md`, senhas do Chrome), feita pelo usuário; o KeePassXC só sai depois de
  todos os milestones aprovados.
- [ ] Auditoria final do security-auditor sobre S1–S15 sem achado crítico aberto.
