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
  clientes dele por socket local. Sobe pelo socket do systemd do sistema
  (`wardd@<uid>.socket` em `/run/ward/<uid>/`, pasta do root) e roda como o usuário
  (D46). ("Arquitetura")
- R2. Núcleo em Rust (`ward-core`): formato, criptografia, tipos, máscara, protocolo.
  Daemon e CLI usam o núcleo. ("Arquitetura")
- R3. Só Linux (Pop!_OS 24.04, COSMIC, Wayland). O que depende do sistema (iniciar com
  a sessão, bloqueio de tela e suspensão, socket, agente SSH temporário, proteção de
  memória, identificação do processo que chama) fica em `ward-plataforma`; núcleo,
  formato, tipos e interface não dependem de Linux. ("Arquitetura")
- R4. Instalação em `/usr/local`, dono root, por `sudo make install` ou `.deb`;
  extensão instalada por política do Chrome em `/etc/opt/chrome/policies`, sem modo
  desenvolvedor. O `sudo` é pedido uma vez, na instalação, que põe as units do sistema,
  os grupos `ward-app`/`ward-nm` e os binários setgid (D46, D47). ("Arquitetura")

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
- R32. **Pré-requisito do app** (handoff 29): `make app-check` precisa dos headers do
  WebKitGTK, instalados uma vez pelo usuário com `sudo apt install
  libwebkit2gtk-4.1-dev` (traz gtk3, soup3 e javascriptcore). Sem eles, o crate do
  Tauri não compila, e por isso ele fica fora do `make check`. O `app/nucleo` compila e
  testa sem eles. Nenhuma fase roda `sudo`: se o pacote faltar, a fase avisa e para no
  que exige o Tauri. ("Segurança" › "Regras de desenvolvimento")

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
2026-11-08); Chrome (e auxiliares do WebKitGTK) depurado por um ancestral malicioso,
que pode mentir a origem da aba, e matar o `wardd` como negação de serviço (RA6, RA5,
decisão do usuário em 2026-11-08). Também fora, por decisão do escopo: SSH com chaves do `~/.ssh` (o Ward
não interfere).

| # | Invariante (texto do escopo) | Milestones |
|---|---|---|
| S1 | Nenhuma operação do socket devolve segredo à CLI. Revelar e copiar só pelo app; preencher só pela extensão no domínio certo. | M3, M4, M5, M6 |
| S2 | Segredo nunca vai para disco em claro. Anexo temporário só em `/run/user/<uid>/ward/`, 0600, apagado ao fim (inclusive se o comando falhar ou for morto). | M2, M4, M6, M7 |
| S3 | Segredo nunca vai em argv (visível em `/proc/<pid>/cmdline`); só no ambiente do processo filho, em arquivo temporário ou em fd. | M4, M6 |
| S4 | A máscara cobre os valores secretos e as variações declaradas, inclusive quando o valor chega quebrado entre dois pedaços da saída. **D34 revisto:** a garantia é o segredo inteiro num mesmo pedaço; quebrado, só o pedaço que o completa sai marcado (nada é segurado). | M2, M4 |
| S5 | Socket 0600 do usuário em pasta do root (`/run/ward/<uid>/`, criada pelo systemd do sistema; o daemon de teste usa um diretório 0700 do usuário), conferência do uid do cliente por `SO_PEERCRED`. A CLI e o app conferem que o servidor é o `wardd` instalado (pasta do root e par com uid 0 no `SO_PEERCRED`) antes de enviar a senha mestra. | M3, M4, M9 |
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
  **Divergência registrada (Fases 27a e 28b):** o token não vai em argv. `ward ssh
  --token <t>` deixaria o token em `/proc/<pid>/cmdline` do terminal e da CLI, legível
  por qualquer processo do mesmo uid (achado alto da auditoria do M3). O app entrega o
  token por um socket herdado e a CLI o lê com `ward ssh --token-fd <n> <entrada>`;
  `--token` em argv é recusado sem conectar. O V4.32 vale com `--token-fd` (token
  inventado é ignorado e vale o nível).
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
  confirmados pelo usuário em 2026-11-08 (RA5, RA6 fechados).
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
  JSON (Go e `ensure_ascii`) e `\$` entre aspas duplas de shell (`declare -p`), e a
  tabela única de D54 (formas de texto por linha, aspas simples, `printf %q`, PHP,
  campo de tab). stdout e stderr são mascarados
  em separado. **D34 revisto (2026-11-09):** nada é segurado nem atrasado; cada pedaço
  lido sai na hora, mascarado (o segredo inteiro dentro do pedaço; o estado do casamento
  segue, então o pedaço que completa um segredo quebrado sai marcado, e o começo que já
  saiu fica como saiu). **Fase 28g:** cada leitura pega tudo o que já está no pipe ou no
  pty (sem esperar dado novo), num buffer de 256 KiB, e os pipes de saída do filho têm
  256 KiB (`F_SETPIPE_SZ`, sem falhar se o kernel recusar): um `write` único de até
  256 KiB é mascarado inteiro. O código TOTP não é mascarado (o agente o recebe, R8).
  Segredo com menos de 6 caracteres é mascarado do mesmo jeito, e o app avisa que a
  máscara fica pouco confiável.
- D12. **Agente SSH temporário** do `ward ssh` com chave: socket em
  `/run/user/<uid>/ward/ssh-<aleatório>/agent.sock` (diretório 0700), passado ao filho
  por `IdentityAgent`. O daemon só atende conexões cujo pid (`SO_PEERCRED`) é o `ssh`
  filho daquela sessão; carrega só a chave da entrada (e a do salto, se houver); só
  lista e assina, e recusa adicionar, remover e travar chaves; some (socket e
  diretório) quando o `ssh` termina, falha ou é morto, e quando o cofre trava.
  **Divergência registrada (Fases 27 e 28b):** o agente só aceita chave **Ed25519**
  (assinatura por `ed25519-dalek`, pelo `ssh-key` do RustCrypto). RSA ficaria com
  RUSTSEC-2023-0071 (crate `rsa`), que o `cargo audit` recusa; ECDSA não foi
  implementado. Uma entrada `servidor` com chave de outro tipo falha no `ward ssh` com
  "só chaves Ed25519 são aceitas pelo agente", sem conectar. Aceitar RSA/ECDSA é decisão
  do usuário (dependência nova e advisory).
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
  **Divergência registrada (Fases 28 e 28b):** o salto **não** usa `ProxyJump`. No
  OpenSSH, o `ProxyJump` vira um `ProxyCommand` executado por `$SHELL -c` (um `sh -c` na
  cadeia, que o V4.35 proíbe), e as opções de `-o` da linha de comando não chegam ao
  `ssh` do salto (só o `-F`), que ficaria com o `known_hosts`, as identidades e o agente
  padrão do `~/.ssh` (contra este D13 e R19). Também não daria para passar o usuário do
  salto só por `-l` (D43). No lugar, o daemon abre uma ponte em `127.0.0.1:<porta>`
  (`crates/wardd/src/ponte.rs`): para cada conexão do grupo de processos do filho (no
  grupo e na sessão do filho, auditoria do M4; ou, Fase 28g, filho direto dele com
  sessão própria, como o `ssh` que o `sshpass` cria com `setsid()`), lança
  por `execve` um `ssh -W <alvo>:<porta>` até a entrada de salto com o mesmo
  endurecimento (`-F /dev/null`, `known_hosts` do Ward, ambiente do zero, usuário por
  `-l`, host depois de `--`, senha por `sshpass -e` ou chave no agente de D12), e o
  `ssh` principal conecta na ponte com `HostKeyAlias` igual ao nome do destino no
  `known_hosts`. O túnel de `banco` por `servidor` usa a mesma ponte. O V4.27 confere,
  com o home do passwd coberto por tmpfs e iscas, que nem o `ssh` principal nem o do
  salto abrem nada do home.
- D14. **Memória.** O daemon trava (`mlock`) os buffers de segredo e a chave de dados;
  a memória de trabalho do Argon2 não é travada (limite de `RLIMIT_MEMLOCK` do usuário).
  Risco residual registrado: swap sem cifra; o Pop!_OS com disco cifrado cifra a swap.
  **Revista pela D62:** `mlockall` do processo inteiro, com `LimitMEMLOCK=infinity` na
  unit (inclui a memória do Argon2).
- D15. **Cofre de teste.** Senha mestra de teste `falso-mestra-ward-01`, PIN de teste
  `135790`, entradas da tabela "Cofre de teste" do contrato (21, com `acme-totp-dev`
  de D28; 22 a partir da Fase 22a, com `delta-api-simbolos` de D59). Tudo falso, gerado em `.dev/` por `make dev-cofre`; os valores fixos ficam na
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
  **Revista pela D62:** contador por papel (o agente não atrasa o app) e o do agente
  guardado no arquivo de estado (matar o `wardd` não zera a espera).
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
  **Revisto pelo usuário (2026-11-09, auditoria do M4):** segurar o prefixo virou oráculo
  (o agente recuperou a senha caractere a caractere); a máscara passa a cobrir só o
  segredo inteiro num mesmo bloco de saída, sem segurar nem atrasar prefixo. Segredo
  impresso em pedaços não é cenário do uso (sempre a senha inteira).
  - Em linha `CHAVE=valor` de anexo, o **valor só vira padrão próprio com 6 caracteres
    ou mais**, o mesmo mínimo das linhas (D11). Sem mínimo, `DEBUG=1` ou `PORT=8080`
    fariam todo `1` ou todo `8080` da saída virar `••••`. A linha inteira continua
    mascarada. Um valor curto que aparece sozinho na saída não é mascarado. Esse risco
    está em RA3 e é aceito. O segredo de campo (senha, token etc.) segue mascarado em
    qualquer tamanho, como manda D11.
  - ~~**Saída interativa do `ward ssh` (Fase 27)**~~ (substituído pela revisão acima,
    Fase 28e: sem pausa nem fim segurado, com ou sem pty): no pty, um fim segurado que pode ser
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
    (`gravar_entradas`, `trocar_senha`) grava `geracao + 1`. A geração é queimada ao
    selar, e o `CofreAberto` sobe mesmo se a gravação falhar (D53, Fase 19e; antes, ele
    só subia depois da gravação que dava certo). Em `u64::MAX`, a gravação é
    recusada. O número pode ser lido (`CofreAberto::geracao()`), mas não existe setter
    público: `configuracao_mut` não mexe nele. A abertura ganha uma variante com
    `geracao_minima`. Abaixo dela, a abertura falha com um erro próprio, `Retrocesso`
    ("o cofre no disco é mais antigo que o último aberto"). Ele não leva valor e só
    aparece depois de decifrar. Geração 0 no corpo é recusada com o erro genérico. O
    `make dev-cofre` grava a geração 1.
  - **Na Fase 22 (M3, daemon):** o daemon guarda a maior geração vista num arquivo de
    estado ao lado do cofre (`~/.local/share/ward/estado`, `.dev/` nos testes; 0600,
    sem segredo, gravação atômica), com os dois números de D53 (`vista` e `queimada`).
    Ele atualiza o arquivo a cada destravar e a cada
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
    outro jeito. A Fase 27 tem teste com usuário que contém `@`. No `sftp` e no `scp`,
    onde `-l` é limite de banda, vale D56 (`-o User=`). O `ProxyJump` é
    montado só com usuário e host já validados; com `@` no usuário do salto, ele não
    pode ir como `usuário@host` na linha do `ProxyJump` (a Fase 28 usa um `ssh_config`
    gerado com `User` literal sem `%`, ou recusa o salto com `@`).
    (Fase 28b: a Fase 28 não usa `ProxyJump`; o usuário do salto vai por `-l` no
    `ssh -W` da ponte, ver a divergência registrada em D13.)
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

- D46. **Conferência do servidor pelo socket do sistema** (**confirmada pelo usuário
  em 2026-11-08**, Fase 21; mudou "Arquitetura" para o systemd do sistema, a instalação
  para o `sudo` uma vez e o texto de S5; S5, S8, D5). Com
  o `wardd` não-dumpable (S8), `exe`, `environ` e `maps` dele ficam ilegíveis para o
  mesmo uid (EACCES), e a conferência de D5 pelo `/proc` falha fechada contra o daemon
  real. Também não há segredo que o daemon guarde e um processo do mesmo uid não leia.
  Decisão: o socket deixa de ser criado pelo próprio `wardd`. Um par
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
  e não herda o ambiente da sessão, então abre o app ele mesmo, com o ambiente que ele
  monta (D57; antes era pelo barramento do usuário, que a Fase 21b mostrou forjável);
  `systemctl --user
  set-environment` deixa de alcançar o daemon (V9.4 muda de forma, S8 fica mais forte).
  O daemon de teste (D7) continua no diretório do usuário e a CLI só pula a conferência
  do uid 0 no artefato `modo-teste`, que a instalação recusa (V9.2). Resíduo: um
  processo do mesmo uid ainda mata o `wardd` (negação de serviço; o systemd o sobe de
  novo), sem passar a receber a senha. Opções descartadas: rótulo do AppArmor por
  `SO_PEERSEC` (um processo não confinado pode trocar para qualquer perfil carregado,
  então o rótulo é forjável, e confinar o `wardd` atrapalha o `ward run`, que executa
  qualquer comando); nome do processo (`Name: wardd` no `status`), que qualquer um
  forja; aceitar o risco, que entrega a senha mestra, e com ela o arquivo do cofre, a um
  pacote malicioso (ameaça 2). A negação de serviço (matar o `wardd`) está em "Fora do
  modelo de ameaça" no escopo.
- D47. **App e host da extensão identificados por grupo setgid** (**confirmada pelo
  usuário em 2026-11-08**, Fase 21; cria grupos e binários setgid na instalação e
  registra o resíduo RA6 em "Fora do modelo de ameaça"; S1, S8, D5). A identificação de D5 lê o `/proc` do cliente, o que
  obriga o app e o `ward-nm` a ficarem dumpable até o daemon responder. Nessa janela,
  qualquer ancestral (inclusive um pacote malicioso que lance ele mesmo o
  `/usr/local/bin/ward-app`, o que o Yama `ptrace_scope=1` permite) se prende, injeta
  código, se solta e herda o papel app. O `TracerPid` só pega o depurador preso, e
  código escrito por `/proc/<pid>/mem` numa biblioteca do root não aparece no `maps`.
  Decisão:
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
  - o app só recebe o papel se o pai for **o próprio daemon** (lido do `PPid` no
    `status`, que continua legível). **Revisto pela D57 (Fase 21b):** o `systemd --user`
    saiu como pai aceito. Lançado por outro processo, ele pede ao daemon que abra a
    instância verdadeira e sai. Antes do GTK, o app confere e limpa o próprio ambiente
    pela lista branca de D57 (valores fixos montados pelo daemon; `DISPLAY` saiu),
    porque os processos do WebKitGTK herdam o ambiente sem `AT_SECURE`; o fd do socket
    é `CLOEXEC`;
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
  eles ficam protegidos de quem não é ancestral pelo Yama, e o ancestral do app é o daemon (D57).
  Um pacote que mate o Chrome e o lance de novo como filho vira ancestral do
  Chrome e pode se prender a ele; um Chrome assim pode mentir a origem da aba e
  receber o preenchimento de qualquer entrada do tipo login. É o mesmo caso de "Fora do
  modelo de ameaça" › leitura do campo de senha por um script feito para isso, agora
  dito com o ptrace, e o usuário o aceitou fora do modelo (escopo, "Segurança" › "Fora
  do modelo de ameaça", 2026-11-08).
- D48. **`LD_*` e bibliotecas** (Fase 21; S8, D5, D47).
  - `wardd`: recusa **qualquer** `LD_*` e também `GLIBC_TUNABLES`, `GCONV_PATH`,
    `MALLOC_TRACE` e `LOCPATH` (a lista insegura da glibc é o "e similares" de S8). O
    ambiente do daemon vem da unit, então a regra estrita não custa nada.
  - App e host com D47: o daemon não confere o ambiente nem o `maps` deles; quem
    neutraliza `LD_*` é o `AT_SECURE`, e a lista branca de D47 cuida do que o GTK, o GIO
    e o GStreamer leriam do ambiente. Quem tem `LD_LIBRARY_PATH` na sessão continua
    usando o app normalmente.
  - Como D47 foi aceita (2026-11-08), vale o ramo acima: a regra da Fase 21 (qualquer
    `LD_*` e toda biblioteca executável do root, lidas do `/proc` do app e do host) sai
    para o app e o host e fica só para o pai Chrome e a cadeia exibida. A Fase 29
    confere com o WebKitGTK real que a lista branca de ambiente de D47 basta (módulos do
    GTK, plugins do GStreamer em `~/.local/share/gstreamer-1.0`, IBus); se algo de fora
    do root for carregado pelo ambiente, o app pré-define o ambiente que desliga essa
    fonte (ex.: `GST_PLUGIN_SYSTEM_PATH` e `GST_PLUGIN_PATH` vazios).
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

Reauditoria do M2 (2026-11-08): aprovou, com um achado médio e quatro baixos. Repros
em `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/ataque/src/bin/`
(`geracao.rs`, `desvio.rs`, `url.rs`). O código muda na Fase 19e, antes da Fase 22.

- D53. **Geração queimada ao selar** (achado médio; D39; "Backup"; S7, S9). Se a
  gravação falhava depois do rename (ex.: no fsync do diretório), o `CofreAberto`
  ficava em n e a gravação seguinte selava n+1 de novo. Dois arquivos diferentes com a
  mesma geração: a cópia do primeiro abria com `geracao_minima` = n+1.
  - **Núcleo (Fase 19e):** `gravar_entradas` e `gravacao::trocar_senha` queimam a
    geração assim que o corpo é selado. O contador do `CofreAberto` sobe para n+1 antes
    de escrever o temporário e fica lá se qualquer passo seguinte falhar (escrita,
    fsync, rename, fsync do diretório). A recusa em `u64::MAX` continua antes de selar,
    sem tocar no disco e sem queimar. A abertura com piso recebe também a maior geração
    já queimada que o chamador conhece: `abrir_entradas_desde(arquivo, senha,
    geracao_minima, geracao_queimada)`. Disco abaixo de `geracao_minima` continua
    `Retrocesso`. O `CofreAberto` aberto fica com o contador em max(geração do disco,
    `geracao_queimada`), e a próxima gravação usa um número que nunca foi selado.
    `trocar_senha(destino, atual, nova, geracao_minima, geracao_queimada)` sela com
    max(disco, queimada) + 1. `CofreAberto::geracao()` passa a ser "a última geração
    lida ou selada", e continua sem setter público.
  - **Daemon (Fase 22):** o arquivo de estado guarda dois números: `vista` (a maior
    geração confirmada no disco, passada como `geracao_minima`) e `queimada` (a maior
    selada ou tentada). Antes de cada gravação, o daemon grava no estado `queimada` =
    `cofre.geracao()` + 1, com gravação atômica e fsync. Se essa escrita falhar, a do
    cofre não acontece. Depois da gravação que deu certo, `vista` = `cofre.geracao()`.
    Em erro, o estado fica como está, e o `CofreAberto` já subiu. Um erro de
    `trocar_senha` depois de selar trava o cofre, porque o `CofreAberto` que o daemon
    guarda não passou pela queima; o destravar seguinte abre com a `queimada` do
    estado. Ao destravar, o daemon abre com (`vista`, `queimada`) e sobe `vista` para a
    geração lida, se ela for maior.
  - Resíduo, sem trabalho: o temporário selado de uma gravação que falhou antes do
    rename pode ser copiado por um processo do mesmo uid enquanto existe. Posto no
    lugar do cofre antes de outra gravação, ele abre (n+1 ≥ `vista`) e traz a mudança
    que o daemon disse ter falhado. Não é retrocesso, porque é a versão mais nova
    tentada, e a gravação seguinte usa n+2.
- D54. **Máscara: entregas, linhas e formas novas** (achados baixos; D11, D36, D37;
  RA3; S4).
  - **Entrega com `\n`:** todo valor secreto de entrega que contém `\n` passa também
    por `linhas`, como o segredo de campo (D36). A chamada fica no `visitar`, e
    `ConstrutorMascara::new().entrega(&g).construir()` sozinho já cobre o token de `api`
    de várias linhas (`head -n1`). Escolhido em vez de fazer o `construir` exigir a
    entrada de cada entrega, que mudaria a API das Fases 26–28 sem cobrir mais nada.
  - **D37 por linha:** cada linha de 6+ caracteres de segredo de várias linhas e de
    anexo (com e sem `\r`, e aparada) gera as **formas de texto** da tabela abaixo,
    cada uma só quando difere da linha literal. As **codificações base64** (com e sem
    `=`, URL-safe, miolos) não valem por linha: ficam para o segredo inteiro e para o
    valor de linha, como hoje. O base64 de uma linha solta vai para RA3, porque com ele
    um anexo de 256 KiB passaria dos 4 MiB de D29. O percent-encoding vale em todo
    padrão, porque o autômato o lê em qualquer forma (D11).
  - **Só linha de texto (confirmado pelo planner, handoff 19e):** as formas de texto e
    os campos de tab por linha valem só para linha que é UTF-8 válido. Linha que não é
    texto (anexo binário) continua mascarada pelo literal, com e sem `\r` e aparada, e
    pelo percent. Motivo: com as formas em toda linha, um anexo binário de ~200 KiB
    passava dos 4 MiB de D29 (custo de ~33× o tamanho), e `json.dumps`, `printf %q` e
    `cut -f` não produzem essas formas a partir de bytes que não são texto. O oráculo
    (`dev/variacoes.py`) segue a mesma regra. A forma de texto de uma linha binária vai
    para RA3.
  - **Formas fora de D37, decisão por caso:**
    - `${X@Q}`, `set` e `set -x` (aspas simples do shell, `'` vira `'\''`):
      **variação**, só quando o valor tem `'`;
    - `printf %q` do bash (barra antes de cada caractere especial): **variação**, só
      para valor em ASCII visível e espaço, com o conjunto do bash 5.2 do Pop!_OS
      24.04 conferido contra o bash real no teste. Com controle ou fora do ASCII o bash
      troca para `$'…'`, e esse caso vai para **RA3**, como o `${X@Q}` de valor com
      controle;
    - `json_encode` do PHP (`\/` e `\uXXXX` minúsculo juntos): **variação** combinada,
      só quando o valor tem `/` e caractere fora do ASCII;
    - YAML `chave : valor` (espaço antes do `:`), `chave:\tvalor`, JSON colado
      `"k":"v"` e chave JSON entre aspas com qualquer caractere menos `"` (`"private
      key"`, `"x/y"`): **variação**, no parser da linha (`valor_de_dois_pontos`);
    - campo separado por tab (`cut -f`, `awk -F'\t'`, TSV): **variação** na passada
      por linha. Cada campo de 6+ caracteres de uma linha com tab vira padrão e é
      tratado como valor de linha. O `expand` (tab trocado por espaços) vai para
      **RA3**.
  - **Tabela única (oráculo × máscara).** A máscara e o `dev/variacoes.py` implementam
    a mesma tabela, sem um chamar o outro:
    1. **segredo inteiro** sem quebra de linha: literal, base64 (padrão e URL-safe,
       com e sem `=`), miolos de base64 (3 alinhamentos), percent e as formas de texto;
    2. **Basic**: base64 de `usuário:segredo`;
    3. **por linha** (segredo de várias linhas e anexo): a linha de 6+ (com e sem
       `\r`, aparada), percent e as formas de texto dela; os campos de tab de 6+;
    4. **valor de linha** (`CHAVE=valor`, `"chave": "valor"`, `chave: valor` nas formas
       acima, campo de tab), com e sem aspas, 6+: tratado como segredo inteiro (1).
    **Formas de texto:** escape JSON padrão (com e sem `\/`), JSON do Go, `ensure_ascii`,
    PHP (`\/` + `ensure_ascii`), aspas duplas do shell, aspas simples do shell e
    `printf %q`, cada uma só quando difere do literal. O `variacoes.py` deixa de
    aplicar base64 e miolos à linha (hoje aplica `todas(linha)`), passa a aplicar as
    formas de texto e ganha as formas novas. Um teste cruzado (rodado por
    `dev/testes/dev-cofre.sh`) confere que cada padrão do oráculo, sozinho na saída,
    sai `••••` pela máscara da entrada dele.
  - **Orçamento:** tudo pela mesma função de custo (D29). A Fase 19e refaz o V2.36 e o
    V2.43; se uma entrada de D15 passar a ser recusada, a fase para e devolve o caso ao
    planner, sem afrouxar o orçamento.
- D55. **Último rótulo do domínio** (achado baixo; D41; S13). O host aceitava
  `0x7f000001`, `0x7f.0.0.1`, `1.2.3.0x4` e `x.123`, que o Chrome e o `curl` leem como
  IPv4 (formas hex e numéricas do WHATWG) ou que não são domínio. Regra nova: domínio
  cujo último rótulo começa por dígito é recusado, com a mensagem de URL inválida. Isso
  inclui todo rótulo que começa por `0x`. Host só de dígitos e pontos continua lido
  como IPv4 por D41. Vale ao salvar, na leitura do corpo, no `login` e no `api`.
- D56. **Usuário no `sftp`** (achado baixo; D43; S3). No `sftp` (e no `scp`), `-l` é
  limite de banda, não usuário. Onde o daemon executa `sftp` ou `scp` (Fase 27), o
  usuário vai como dois argumentos, `-o` e `User=<usuário>`, com `-F /dev/null` e o
  host depois de `--`, por `execve` sem shell. O `-o User=` que D43 proíbe no `ssh` é
  aceito só aqui, porque o conjunto de D43 (`[A-Za-z0-9._@-]`, sem `-` no começo) não
  tem `%`, espaço, aspas, `=` nem `,`, e nada é expandido. No `ssh` continua `-l`. Se o
  conjunto de D43 crescer, D56 é revista antes.
- D57. **App só como filho do daemon; ambiente montado pelo daemon** (handoff 21b; D46,
  D47; S1, S8; decisão do planner em 2026-11-08, endurece D47 sem mudar o que o usuário
  aceitou). A Fase 21b mostrou que o pai `systemd --user` não protege o app: o gerente
  roda as units de `~/.config/systemd/user/` e aceita `systemctl --user
  set-environment`, que um pacote malicioso escreve. Uma unit com
  `ExecStart=/usr/local/bin/ward-app` e `Environment=WAYLAND_DISPLAY=<proxy>` (ou
  `DBUS_SESSION_BUS_ADDRESS`) dá ao app o papel com o pai "certo", e o proxy vê o PIN,
  a senha mestra digitada e o valor revelado. A conferência do gerente também é
  forjável (`cmdline` e cgroup delegado ao usuário; o `exe` dele não se lê), então um
  processo qualquer com `PPid: 1` no `init.scope` passaria por ele. Decisão:
  - o daemon é **o único pai aceito** do app. O ramo do `systemd --user` sai do
    `ProcessoPar::classificar`. O app de menu, do autostart ou da bandeja de outra
    instância não recebe papel e pede ao daemon a instância verdadeira (operação
    `abrir_app`, sem segredo, de qualquer papel, no máximo uma instância por vez);
  - o daemon lança o `ward-app` instalado por `fork` + `execve` (caminho absoluto
    fixo, sem shell, fds fechados menos 0–2 ligados a `/dev/null`) com um ambiente
    **montado do zero**, que não herda nada do daemon (sem `LISTEN_*`) nem do pedido:
    - `XDG_RUNTIME_DIR=/run/user/<uid>`, pasta do uid, 0700, sem link;
    - `WAYLAND_DISPLAY=wayland-<n>`: nome sem `/`, do único socket `wayland-<n>` do
      uid nessa pasta (sem link). Nenhum ou mais de um: o daemon não abre o app e
      responde "aprovação indisponível" (D21), com aviso;
    - `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/<uid>/bus`, socket do uid, sem link;
    - `XDG_SESSION_TYPE=wayland`, `GDK_BACKEND=wayland`; **sem `DISPLAY`** (num cliente
      X11, qualquer outro cliente lê o teclado e a tela);
    - `HOME` do `getpwuid`, `PATH=/usr/local/bin:/usr/bin:/bin`, `LANG`/`LC_*` do
      próprio daemon (que vêm do locale do sistema pela unit);
    - as variáveis que desligam fontes de módulos e canais fora do root (D48, fixadas
      pela Fase 29a, por D64 e por D67): `GST_PLUGIN_SYSTEM_PATH=/usr/lib/<multiarch>/gstreamer-1.0`,
      `NO_AT_BRIDGE=1`, `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null`,
      `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps`,
      `GST_PLUGIN_LOADING_WHITELIST` (só `coreelements` e `app` da pasta do sistema) e
      `__GL_APPLICATION_PROFILE=0`.
  - o app confere, antes do GTK, que o ambiente que recebeu tem exatamente essas
    variáveis nessas formas (pasta, sockets, donos, sem link); fora disso, não usa o
    papel e sai com mensagem. É defesa em profundidade: o daemon já montou assim;
  - o app vive no cgroup do `wardd@<uid>.service`. Se o daemon cai, o app cai junto
    (`KillMode` padrão) e é reaberto por quem pedir. A reexecução de D47 só vale com o
    `PPid` ainda igual ao daemon;
  - as units do sistema (Fases 24 e 51) **não** usam `NoNewPrivileges=`,
    `RestrictSUIDSGID=` nem opção que os implique para serviço com `User=`
    (`SystemCallFilter=`, `RestrictNamespaces=`, `LockPersonality=`,
    `MemoryDenyWriteExecute=` etc.): com `no_new_privs` o `execve` não aplica o setgid e
    o app nunca recebe o papel. `MemoryDenyWriteExecute=` também quebra o JIT do
    WebKitGTK.
  Custo: o app deixa de ser aberto pelo `systemd-run --user`; o daemon passa a ter um
  lançador (`ward-plataforma`, Fase 29a) e a operação `abrir_app` (Fase 23). Opções
  descartadas: aceitar o risco (entrega o PIN e a senha mestra ao proxy); só fixar e
  conferir os sockets com o `systemd --user` ainda como pai (fecha a injeção pelo
  ambiente, mas mantém a conferência forjável do gerente e não fecha mais nada que o
  daemon-pai não feche). Resíduo: RA8.
- D58. **Go com `ensure_ascii`** (handoff 19e; D37, D54; S4). O JSON "html-safe" com
  `ensure_ascii` (`&` e `ç` no mesmo valor) passava. Vira **variação
  combinada** da tabela de formas de texto de D54, só quando o valor tem `&`, `<`, `>`,
  U+2028 ou U+2029 **e** caractere fora do ASCII, como a do PHP. Vale no segredo
  inteiro, no valor de linha e por linha de texto. Escolhido em vez de RA3 porque o
  custo é uma forma condicional e o caso aparece com o gerador de D35 (`&<>`) em valor
  com acento. Entra na máscara e no `variacoes.py`, com o teste cruzado e o orçamento
  de D29 (se uma entrada de D15 passar a ser recusada, a fase para). Fase 22a.
- D59. **Entrada de teste com caracteres especiais** (handoff 19e; D15, D54, D58). As
  fixtures não têm caractere especial, e o teste cruzado na lista real exercita pouco
  de D54. Entra em D15 a 22.ª entrada `delta-api-simbolos` (api, grupo `delta`, `dev`,
  livre): URL `https://api.teste.invalid`, token `falso-Ab3$k&<9>Zq!'/ç-01`, variável
  `SIMBOLOS_TOKEN`, e o extra secreto `SEGREDO_ASCII=falso-Ab3$k&<9>Zq!'` (só ASCII,
  para o `printf %q`). Cobre aspas simples (`${X@Q}`), aspas duplas do shell, JSON do
  Go, PHP (`/` + fora do ASCII), D58 e percent. Não é usada por nenhum serviço do
  `docker compose`. Os valores entram na allowlist do gitleaks. Fase 22a, depois da 22
  (as duas mexem no `Makefile` e nas contagens do cofre de teste); o V2.29 e o V2.30
  passam a valer com 22 entradas (V3.29).
- D60. **Notas da Fase 21b** (S5, S8, D46–D48).
  - **`Dumpable` depois do `exec`:** com `fs.suid_dumpable=2` (padrão do Pop!_OS), o
    setgid sobe com dumpable 2 (`SUID_DUMP_ROOT`), não 0. O ptrace e o
    `/proc/<pid>/mem` continuam negados ao mesmo uid. O 0 só vale depois do
    `PR_SET_DUMPABLE=0` da partida do app (Fase 29a). O contrato confere "≠ 1" antes e
    "0" depois (V5.31).
  - **Ptrace no começo do processo (V3.24):** não há como parar um setgid entre o
    `exec` e o `main` sem ptrace. Vale a forma em duas metades da 21b: preso depois do
    `exec`, com o filho parado antes de conectar → EPERM; preso antes do `exec`
    (`PTRACE_TRACEME`) → o kernel não aplica o setgid e o processo fica agente.
  - **Nome da variável insegura (V3.25):** a mensagem do `wardd` leva o nome
    higienizado (`[A-Za-z0-9_]`, até 64 caracteres), nunca o valor. A `Recusa::VariavelLd`
    do pai Chrome continua só com "`LD_*`".
  - **`TMPDIR` e daemon de teste:** `TMPDIR` está na lista insegura da glibc e o
    `wardd` recusa subir com ela. O `make dev-daemon` (Fase 22) sobe o daemon de teste
    com `env -i` e uma lista fixa (`HOME`, `PATH`, `XDG_RUNTIME_DIR`, `LANG`, mais as
    variáveis do `modo-teste`), para não falhar em quem tem `TMPDIR` na sessão.
  - **`LISTEN_PID`/`LISTEN_FDS`** ficam no ambiente do `wardd` (o Rust 2024 exige
    `unsafe` para `remove_var`). Nenhum filho do daemon herda o ambiente dele: o `ward
    run` monta o do filho a partir do da CLI (D13, Fase 26) e o lançador do app monta o
    dele do zero (D57). O fd do socket já tem `CLOEXEC`.
  - **Grupos na instalação (Fase 51):** `ward-app` e `ward-nm` criados com `groupadd
    --system` (gid < 1000), fora de toda faixa do `/etc/subgid`, sem membros e com a
    senha travada no `gshadow` (`!`, padrão do `groupadd`). Grupo com senha deixa
    qualquer um entrar por `newgrp`. O daemon não lê o `gshadow` (é do root), então
    quem confere é a instalação, que recusa grupo já existente com membros, com senha
    ou com gid fora dessas regras.
  - **Namespace de usuário no teste:** onde o userns sem privilégio está bloqueado
    (`kernel.apparmor_restrict_unprivileged_userns=1`), o caso do V3.24 diz isso na
    saída e passa, porque o ataque não é possível; no Pop!_OS deste host ele roda.
  - **Lock do Tauri:** corrigido na 21b sem `make app-check` (R32). A Fase 29a roda o
    `make app-check` e confirma.

- D61. **Máscara: reauditoria da 19e** (1 achado médio e 3 baixos; D11, D29, D54; RA3;
  S4; decisão do planner em 2026-11-09). Critério: vira variação o que aparece em
  arquivo de credencial de uso normal ("Credenciais" › "Anexos": `.env`, `.pem`,
  kubeconfig, JSON de conta de serviço) lido sem má intenção e custa pouco no
  orçamento de D29; vai para RA3 o que é impressão de depuração do valor ou
  recodificação, que a skill proíbe (V8.2).
  - **JSON numa linha (médio): variação.** Em toda linha de texto (UTF-8 válido) de
    segredo de várias linhas e de anexo, e no segredo de uma linha só que, aparado,
    começa por `{` ou `[`, cada par `"chave"` + espaços opcionais + `:` + espaços
    opcionais + string JSON, **em qualquer posição da linha**, dá um valor: a string
    decodificada pelos escapes do JSON (`\"`, `\\`, `\/`, `\b\f\n\r\t`, `\uXXXX` com par
    substituto; escape inválido descarta o par). Com 6+ caracteres depois de decodificado,
    o valor é **valor de linha** (item 4 da tabela de D54, tratado como segredo inteiro).
    Cobre o anexo `client_secret_*.json` minificado, o extra `{"token":…}`, o objeto
    aninhado na linha (`"db": {"senha": "…"}`) e duas chaves na mesma linha. Elemento
    de array sem chave (`["a","b"]`) e o flow do YAML (`{k: v}`) vão para RA3.
  - **YAML e dotenv (baixos): variação**, no parser de `chave: valor` e de
    `CHAVE=valor`, cada candidato a mais só quando difere do que já sai:
    - comentário: valor sem aspas termina antes do primeiro espaço ou tab seguido de
      `#`, aparado; valor entre aspas (`"…"` ou `'…'`) seguido de espaços e `# …` dá o
      miolo das aspas. Vale no YAML e no dotenv (`SENHA="v" # nota`, que o `source`
      lê como `v`);
    - chave YAML sem aspas com espaço ou `/` (`private key:`, `aws/secret:`): depois do
      recuo, a chave vai até o primeiro `:` seguido de espaço, tab ou fim, sem `#` nem
      aspas, e não começa por `-`, `{` nem `[`;
    - valor YAML entre aspas simples: `''` vira `'` no valor decodificado (o
      `'it''s'` que o `yq` imprime como `it's`).
    Motivo de não ir para RA3, como o handoff sugeria: `.env` com comentário no fim e
    YAML com chave composta são de uso normal, e cada regra gera no máximo um candidato
    por linha.
  - **Campos de tab em linha que não é UTF-8 (baixo): variação só pelo literal**
    (sugestão do handoff). Cada campo de 6+ bytes da linha com tab vira padrão pelo
    literal (com e sem `\r`, aparado) e pelo percent, sem formas de texto nem base64,
    como a própria linha binária em D54. O `iconv` da linha e o valor depois do `=`
    dentro de um campo desses vão para RA3.
  - **`repr()` do Python (baixo): RA3**, junto com o `util.inspect` do Node (crase
    quando o valor tem `'` e `"`). É impressão de depuração do valor, cada linguagem
    tem a sua forma, e a skill manda nunca imprimir variável de credencial.
  - **Orçamento:** tudo pela função de custo de D29. O JSON dá no máximo um valor por
    par, e a soma dos valores de uma linha não passa do tamanho dela; YAML, dotenv e
    tab dão no máximo um candidato a mais por linha. Um anexo JSON minificado grande
    pode passar dos 4 MiB e é recusado com a mensagem de D29: é a regra, sem afrouxar.
    A Fase 22a refaz o V2.36 e o V2.43; se uma entrada de D15 (22 com D59) passar a ser
    recusada, a fase para e devolve ao planner. O `dev/variacoes.py` implementa as
    mesmas regras, sem chamar a máscara, e o teste cruzado cobre os padrões novos.
- D62. **Notas do handoff 22** (S3, S7, S8, S9; D14, D20, D39, D50, D53; decisão do
  planner em 2026-11-09). Fase 22b, depois das 23 e 24 (as três mexem em `crates/wardd`).
  - **Espera do D20 por papel e guardada no estado.** São dois contadores: o do
    papel agente (inclui o `ward destravar` da CLI) e o do papel app; o papel extensão
    não destrava. Erro do agente não atrasa o app. O contador do agente vai para o
    arquivo de estado (linha nova; o leitor estrito aceita o formato antigo como 0 e o
    daemon regrava no novo), com fsync **antes** de responder o erro. Ao subir, o
    daemon aplica a espera desse número à primeira tentativa do agente, contada da
    partida. Os dois zeram no destravar certo de qualquer papel. O do app fica em
    memória. Motivo: o mesmo uid mata o `wardd` (negação de serviço fora do modelo), e
    o systemd o sobe de novo sem espera; com o contador só em memória, matar e
    reconectar dá umas 6 tentativas a cada 30 s em vez de 2 por minuto, e D20 deixa de
    proteger. Resíduo, sem trabalho: quem reescreve o arquivo de estado zera o contador
    (mesma família de RA4); nenhuma operação passa a devolver segredo.
  - **Erros de PIN continuam em memória.** Reiniciar trava o cofre, e o PIN só vale
    no papel app depois da senha mestra: o reinício não dá tentativa de PIN a mais a
    quem não destravou.
  - **`mlock` (D14 revista).** O `CofreAberto` não expõe a chave, e expor seria API
    pior. Em vez de travar buffer a buffer, `iniciar_endurecido` chama
    `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)`. As units (teste da Fase 24,
    instalação da Fase 51) levam `LimitMEMLOCK=infinity`, porque com limite o
    `MCL_FUTURE` faz a alocação do Argon2 falhar. Release: sem `RLIMIT_MEMLOCK`
    infinito ou com erro do `mlockall`, o `wardd` sai ≠ 0 com mensagem e sem socket
    (falha fechada). `modo-teste`: com o limite comum do usuário, aviso no log e segue,
    para o `make dev-daemon` subir. `fork` e `execve` não herdam o travamento, então os
    filhos (app, `ward run`) não ficam travados. Cobre também a memória do Argon2.
  - **`criar_cofre` com estado velho não baixa o estado.** Sem cofre e com estado de
    `queimada` q, o cofre novo nasce na geração q+1 e o estado vai a `vista` =
    `queimada` = q+1, com aviso no log. Se o núcleo não cria com geração inicial, ganha
    o parâmetro (mudança pequena no `ward-core`, com teste). Motivo: apagar o cofre,
    esperar o usuário recriar e pôr no lugar uma cópia antiga abriria a cópia.
  - **TOCTOU do `stat`.** `gravar_entradas` passa a devolver a identidade (dev, ino,
    mtime com ns, tamanho) lida por `fstat` do fd do temporário depois do fsync e antes
    do rename, que não muda esses campos. O daemon usa essa identidade e nunca um
    `stat` do caminho depois do rename. Mudança pequena no `ward-core`.
  - **Mudanças da Fase 22 no `ward-core`** (`ItemLista.nivel` como `Option`, `Some` só
    no papel app; `corpo::abrir_entradas_lida`): aceitas pelo planner (D50, D53). O
    scrutiny do M3 as revê como código do núcleo.
  - **Pilha.** O detector da V3.8 mostrou que resto de pilha volta ao heap pelo
    enchimento de `enum`. Regra para as fases do daemon: segredo não vai para array na
    pilha; fica em `Segredo`/`Zeroizing` no heap. O handler de SIGSEGV do runtime do
    Rust fica registrado para a auditoria (só imprime "stack overflow").
- D63. **Papel app só ao processo que o lançador abriu; app de teste** (handoffs 22 e
  29a; D7, D8, D47, D57; S1, S10; decisão do planner em 2026-11-09).
  - **Release (achado do planner).** Hoje o papel app vale para qualquer par com egid
    `ward-app` e `PPid` igual ao daemon. A partir da Fase 26 o daemon cria filhos do
    `ward run`, e `ward run <entrada livre> -- /usr/local/bin/ward-app` daria um
    `ward-app` setgid filho do daemon, com argv, cwd e fds escolhidos pelo agente e um
    ambiente que ele consegue montar igual ao de D57. Regra: o lançador guarda o pidfd
    do processo que abriu (tirado no `fork`, antes do `execve`); o papel app exige
    egid `ward-app` **e** pid igual ao desse pidfd, ainda vivo, **e** `PPid` igual ao
    daemon. A reexecução de D47 mantém o pid. Todo outro filho do daemon (`ward run`,
    `ward ssh`, Playwright) fica agente. Fase 23; ataque do `ward run` na Fase 26
    (V4.38).
  - **`modo-teste`.** A única diferença é não exigir o egid; o pid do lançador e o pai
    continuam exigidos. O daemon de teste lê `WARD_DEV_APP` (caminho absoluto do
    binário de teste, o `ward-app` de teste ou a `ward-ponte-teste`: arquivo regular do
    uid, sem link, dentro do repositório) e o `abrir_app` lança esse binário por
    `lancar_para_teste`. O app Tauri de teste recebe o ambiente de D57; a ponte, que não
    abre janela, um ambiente fixo mínimo (`HOME`, `PATH`, `LANG`, `XDG_RUNTIME_DIR` e as
    variáveis do `modo-teste`), sem exigir Wayland. Nenhum dos dois herda o ambiente do
    daemon (D60) nem o do pedido. A saída vai para `.dev/logs/<binário>.log` (0600).
    `make app-ponte` passa a subir o daemon de teste com `WARD_DEV_APP` da ponte (ou
    exigir que esteja no ar com ele), pedir `abrir_app` e imprimir a URL com o token
    lida desse log. Nenhum cliente escolhe o binário nem ganha o papel pedindo. Quem
    aprova é o humano na ponte, como no app: D8 e V3.15 continuam valendo.
  - **Por que não outra forma.** Um token em `.dev/` ou uma variável que o cliente
    apresenta seria uma porta que o protocolo conhece (V3.15). Uma exceção pelo `exe`
    do par seria a conferência pelo `/proc` que D47 descartou. Com o pid do lançador, o
    caminho de teste é o do release menos o egid.
  - **Contra vazar para o release.** A exceção e `WARD_DEV_APP` ficam atrás de
    `cfg(feature = "modo-teste")`. Nenhum membro do workspace liga `wardd/modo-teste`
    nem `ward-plataforma/modo-teste` em dependência normal (só em dev-dependency ou por
    `--features` explícito). Um teste sem a feature confere que o par sem egid aberto
    pelo lançador fica agente. O V3.37 procura `WARD_DEV_APP` nos binários de release,
    como o V5.6 faz com `WARD_DEV_DIR`. O build de release (Fase 51) é por pacote
    (`-p`), nunca `--workspace --all-features`, e o `make install` recusa artefato
    `modo-teste` (D7).
  - **Papel na primeira resposta.** A resposta de `estado` traz o papel (`agente`,
    `app` ou `extensao`, sem nenhum outro dado). O app e a ponte conferem na primeira
    resposta. Sem `app`, não usam a conexão: o app sai com mensagem (e pede `abrir_app`
    se o pai não é o daemon, como em D57); a ponte sai com "o daemon não deu o papel
    app". Protocolo na Fase 23; lado do app e da ponte na 23a.
  - **Código de saída do filho.** O daemon registra no log o código de saída (ou o
    sinal) do app que lançou, só o número (handoff 29a; Fase 23).
  - Resíduo, só no `modo-teste`: qualquer processo do uid que leia o token da ponte em
    `.dev/logs/` dirige a ponte. Só existem o cofre de teste e credenciais falsas (D7,
    D15).
- D64. **Pendências da 29a** (S8, S11, S14; D48, D57; RA7, RA8; decisão do planner em
  2026-11-09).
  - **Lista de D57** com as variáveis da 29a e a do MPS (texto de D57 atualizado). A
    acessibilidade desligada (`NO_AT_BRIDGE=1`, `WEBKIT_A11Y_BUS_ADDRESS`) é decisão do
    usuário de 2026-11-09, registrada em "App desktop": pelo AT-SPI qualquer processo
    do uid leria o texto da tela, inclusive o valor revelado, e leitor de tela não
    funciona no Ward.
  - **`/tmp/nvidia-mps`: decidido aqui, não em RA8.** O cliente MPS da biblioteca da
    NVIDIA, no processo web, conecta a `/tmp/nvidia-mps/control`, e em `/tmp` um
    processo do mesmo uid cria a pasta e escuta. O lançador fixa
    `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps`, caminho que ninguém cria, nem o
    root. O efeito é o de uma máquina sem servidor MPS (o cliente não acha o canal e
    segue sem MPS). O app confere a variável como as outras de D57.
  - **Varredura de canais graváveis.** Com o WebKitGTK real sob `strace -f`, a Fase 29b
    lista todo `open`/`connect` em pasta onde o uid ou outros escrevem fora de um
    caminho do root (`/tmp`, `/var/tmp`, `/dev/shm`, `/run/user/<uid>` fora do
    `wayland-<n>` e do `bus`). Canal novo volta ao planner. Dado do usuário que o GTK lê
    (tema em `settings.ini`/`gtk.css`, dconf, caches de fonte) fica em RA8.
  - **V5.31:** o `Dumpable` é lido por `prctl(PR_GET_DUMPABLE)` dentro do processo (o
    app de teste o imprime na saída), porque o `status` não tem esse campo; o
    `TracerPid` continua pelo `status`. Contrato ajustado.
  - **RA7:** se o `cargo deny` do app der `advisory-not-detected` para
    RUSTSEC-2024-0429 e o `cargo audit` em `app/src-tauri/` não o achar, a exceção sai
    do `deny.toml` do app; uma exceção sem advisory só esconderia a volta dele. Fase
    29b; V5.30 ajustada. A 0370 continua com RA7, que vai ao usuário.
  - **Papel na primeira resposta:** D63.
- D65. **Pendências das Fases 32, 25 e 37** (handoffs 32, 25 e 37; D4, D5, D35, D46,
  D47, D50, D51, D57, D62, D63; S1, S5, S10, S11, S13; decisão do planner em
  2026-11-09).
  - **(a) Transporte do app.** `DaemonSocket::trocar` passa a usar `escrever_quadro` e
    `ler_quadro` na conexão única de D47 (sem reconectar; a queda segue D57). Uma thread
    leitora é a única que lê o socket e separa o que chega:
    - a resposta vai para quem pediu. Um pedido por vez, sob mutex: o protocolo não tem
      id de pedido e o daemon responde na ordem;
    - `pedido_de_aprovacao` e `pedido_de_destrave` são avisos e vão para uma fila
      limitada (16), nunca no lugar de uma resposta. Com a fila cheia, o aviso novo é
      descartado com log sem conteúdo, e o pedido do agente expira e nega (S10, falha
      fechada);
    - resposta sem pedido, ou quadro que não decodifica em `decodificar_resposta(Papel::App)`,
      fecha a conexão.
    Prazo da resposta: 130 s (destravar com a espera de D20 mais o Argon2, D62).
    Buffers em `Zeroizing` e limite de quadro de `ward_core::limites`. O daemon (Fase
    23) manda os avisos só na conexão do papel app, cada um num quadro inteiro, nunca
    no meio de uma resposta. Transporte e fila na Fase 23a, junto com a conferência do
    papel na primeira resposta (D63), que precisa dele; a Fase 31 liga
    `pedido_de_destrave` a um evento do Tauri e à tela; a 36 liga `pedido_de_aprovacao`.
    O evento leva só o texto do aviso, que não tem segredo, e quem exibe usa D51.
  - **(b) Revelar: nenhuma operação nova.** A operação `revelar` já está na tabela da
    Fase 20 (D5: só papel app, resposta `segredo`), ao lado de `copiar` e `totp`. Falta
    o daemon atender (hoje responde `interno`) e o app chamar. Regras:
    - só o papel app (D63), com o cofre destravado;
    - sem PIN, sem janela de aprovação e sem olhar o nível (D4). Nível e aprovação
      regulam o uso pelo agente ("Claude (agente)" › "Nível de acesso por entrada": "O
      nível vale para `run`, `ssh`, `login` e `playwright`"), e o escopo limita o PIN a
      aprovar usos e mudar nível ("Destravar e travar"). O papel app só existe para o
      processo que o lançador abriu (D63), então quem pede é o usuário, e pedir o PIN
      a ele mesmo não acrescenta barreira contra as ameaças do modelo. Entrada
      bloqueada também é revelável no app;
    - o daemon registra `revelar <entrada> <campo>` sem valor (S13) e devolve o
      `Segredo` em `Zeroizing`. `copiar` e o `totp` do app seguem as mesmas regras;
    - no app, o comando `revelar_campo` leva o valor à webview só no clique e o tira em
      30 s, ao esconder ou ao sair (D4), no padrão `Revelada` da Fase 37 e sem
      ligaduras na fonte.
    Daemon na Fase 22c; app na Fase 33 (formulário e detalhe de Credenciais, que usam o
    mesmo `Campo secreto`). V5.3 e V5.11 fecham no M5 contra o daemon real, depois das
    Fases 22c e 23a.
  - **(c) Lista do app sem N+1.** A `ItemLista` ganha `resumo: Option<ResumoItem>`,
    `Some` só na lista do papel app, no mesmo padrão do nível (D50): `codificar_resposta`
    recusa mandar ao agente um item com resumo, e a leitura do agente recusa um item
    que o traga. `ResumoItem` tem os campos de texto do tipo (os mesmos do `detalhe`,
    sem segredo), os ids dos segredos preenchidos e o número de extras e de anexos. A
    tela monta a 2ª linha e a busca por usuário só com a `lista`, numa troca. O agente
    continua com os seis campos do escopo. Fase 22c.
  - **(d) Datas de criação e alteração** (provisória: o escopo não lista datas; elas vêm
    do frame aprovado no M0, cujo rodapé mostra "Criada em … · Alterada em …"). A
    `Entrada` ganha `criada_em` e `alterada_em` (segundos UTC) dentro do corpo cifrado,
    que passa a v4. A v3 é recusada como versão não suportada: não há cofre instalado,
    e o `make dev-cofre` regrava o de teste. O daemon carimba pelo próprio relógio
    (injetado nos testes). Entrada nova recebe as duas datas; edição e `mudar_nivel`
    trocam só `alterada_em`; `criada_em` nunca muda, e data vinda do cliente é recusada
    pelo decodificador. Restaurar um backup mantém as datas. Só o `detalhe` do papel app
    leva as datas; o agente nunca. Nenhuma decisão de segurança usa essas datas (o
    rollback é D39, pela geração). Fase 22d. Se o usuário dispensar as datas, a Fase
    37a tira o texto do rodapé e a 22d sai do plano.
  - **(e) V4.2 em duas partes.** No `modo-teste` a CLI pula o uid 0 (D46), e a
    conferência extra (par não-dumpable) só barra um falso ingênuo: um falso que chame
    `prctl(PR_SET_DUMPABLE, 0)` passa. A garantia de S5 é a do release, pasta do root e
    par de uid 0. O V4.2 passa a ter duas partes: a 1ª no build de teste, com o falso
    em Python (já coberta pela Fase 25); a 2ª no build sem `modo-teste`, no contêiner da
    21b, com o `wardd` verdadeiro copiado e um falso não-dumpable, ambos do uid 1000.
    Teste e alvo na Fase 28.
  - **(f) Gerador.** Faixa de tamanho na interface: 8 a 64, no avulso e no formulário
    (o núcleo segue com 4 a 128, handoff 18; nenhum comando do app aceita fora de 8 a
    64). Limiares de força aceitos pelo usuário em 2026-11-09: fraca < 60 bits,
    razoável < 80, forte < 128, muito forte ≥ 128. O texto de "Símbolos" no FigCraft
    passa a ser o conjunto de D35 (Fase 37a); o app mostra o que vem de
    `ward_core::gerador::SIMBOLOS`.
  - **Design pendente** (handoff 32), na Fase 37a: frame `Credenciais / Sem resultado`
    (busca ou filtros sem resultado), cartão de nível bloqueado no detalhe e contagens
    do cofre de teste tiradas da fixture (21 entradas hoje, 22 depois da Fase 22a; 13
    no acme).
- D66. **Extensão com `host_permissions` e content script fixo** (decisão do usuário,
  2026-11-09; Fases 42 e 43). Para oferecer "Salvar login" no envio e para o `ward
  login` preencher a aba ativa sem clique, a extensão declara `host_permissions`
  exatamente `https://*/*`, `http://localhost/*` e `http://127.0.0.1/*` (as origens que
  D9 aceita), um content script `envio.js` em todo http/https (só quadro principal, só
  envio com gesto real) e um service worker `servico.js` (remetente conferido pelo
  `sender` do Chrome; senha capturada só na memória dele, no máximo 3 min). O
  `dev/extensao/montar.py` recusa qualquer desvio. Preço aceito: a extensão pode
  injetar código em qualquer página https sem clique. A origem continua conferida no
  daemon (D9) e cada preenchimento vale uma vez.

- D67. **Canais da 29b fechados por variável** (handoff 29b; D57, D64; S8; decisão da
  Fase 39c, 2026-11-09, pelo lado mais seguro). A varredura da 29b achou, com o ambiente
  de D57 completo, o `gst-plugin-scanner` (filho do processo web) carregando código do
  HOME e abrindo canais graváveis. Medido de novo na 39c sem o WebKit
  (`make teste-canais-gst`: o mesmo scanner do sistema e o EGL da GPU, HOME falso):
  - **frei0r, LADSPA e libvisual** (`dlopen` de `~/.frei0r-1/lib`, `~/.ladspa`,
    `~/.libvisual/actor`), **`/dev/shm/cuda_injection_path_shm`**, o socket abstrato
    `@cuda-uvmfd-…` e `~/.nv/ComputeCache` (libcuda, pelo `nvcodec`), **`~/.mfx_trace`** e
    `/dev/shm/GFX_MEDIA_TRACE` (plugins de vídeo da Intel): todos vêm do `plugin_init`
    de plugins que o app não usa. Fecha
    `GST_PLUGIN_LOADING_WHITELIST=gstreamer@<pasta>/libgstcoreelements.so:gst-plugins-base@<pasta>/libgstapp.so`
    (`<pasta>` = a de `GST_PLUGIN_SYSTEM_PATH`): o scanner ainda faz `dlopen` dos
    arquivos do root, mas só esses dois rodam o `plugin_init`. `LADSPA_PATH` não fecha o
    `~/.ladspa`, a libvisual não tem variável e `FREI0R_PATH` fecha só o frei0r; por
    isso a lista, e não uma variável por plugin. Descartadas: registro do root com
    `GST_REGISTRY_UPDATE=no` (precisa da instalação e de regravar a cada atualização do
    GStreamer) e pasta só com os plugins (idem). Sem mídia no app, perder os outros
    plugins não tira nada; se o WebKitGTK avisar elemento ausente, o
    `make teste-webkit-real` mostra.
  - **`~/.nv/nvidia-application-profiles-rc*`** (driver da NVIDIA no app e no processo
    web, que muda opções do driver): fecha `__GL_APPLICATION_PROFILE=0`, mesmo com um
    `nvidia-application-profile-globals-rc` plantado com `enabled: true`. O driver
    ainda lê esse arquivo de globais (só a chave `enabled`, que a variável vence): fica
    em RA8.
  - O app confere as duas como as outras de D57 (`conferir_ambiente_do_app`), e o
    `make teste-webkit-real` passa a reprovar V5.32 se uma isca carregar ou se um
    desses caminhos for aberto (antes saía 2, "pendente"). A conferência com o
    WebKitGTK real fica com o usuário enquanto o `libwebkit2gtk-4.1-dev` faltar (R32).
  - Fora da lista de canais, sem mudança: a pasta de dados do WebKit
    (`~/.local/share/local.ward.app/`, cache HTTP legível pelo uid; handoff 29b) segue
    para a auditoria do M5.

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
  **Decisão do usuário (2026-11-09): risco aceito**, registrado em "Fora do modelo de
  ameaça" no escopo. Consequência (Fase 28b): a entrega por ambiente e anexo 0600 de
  D10/D13 fica como está; o V4.13 não bloqueia o M4 e o teste `ra1_…` de
  `crates/wardd/tests/run.rs` só registra o estado (o environ do filho segue legível
  pelo mesmo uid). Mudar a entrega volta a ser decisão do usuário.
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
  mascaradas). Depois da reauditoria do M2 (D54), entram também: base64 de uma linha
  solta de segredo de várias linhas; `printf %q` e `${X@Q}` de valor com controle ou
  fora do ASCII (`$'…'`), o que inclui o `declare -p` de um valor inteiro de várias
  linhas (`$'…\n…'`, com acento em octal; handoff 19e); o `expand` de linha com tab; e
  as formas de texto de uma linha de anexo que não é UTF-8 válido (D54). Depois da
  reauditoria da 19e (D61): `repr()` do Python e `util.inspect` do Node de valor com
  `'` e `"`; `iconv` de linha que não é UTF-8 e o valor depois do `=` num campo de tab
  dessa linha; elemento de array JSON sem chave e flow do YAML (`{k: v}`).
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
- **RA5. Conferência do servidor (S5)** (Fase 21; **fechado em 2026-11-08: D46
  confirmada**). A Fase 22 aceita o socket por `LISTEN_FDS` e, só no daemon de teste,
  o criado pelo próprio daemon. Matar o `wardd` (negação de serviço) ficou em "Fora do
  modelo de ameaça".
- **RA6. Janela de endurecimento do app e do host** (Fase 21; **fechado em
  2026-11-08: D47 confirmada**). Sobra o resíduo dos auxiliares do WebKitGTK e do
  Chrome (protegidos só pelo Yama contra quem não é ancestral; um Chrome depurado por
  um ancestral malicioso pode mentir a origem da aba), registrado em "Segurança" ›
  "Fora do modelo de ameaça".
- **RA7. Advisories do gtk-rs 0.18 no app** (Fase 29; **aceito provisoriamente pelo
  planner, vai ao usuário**; S14). O Tauri 2 exige o gtk-rs 0.18, que traz
  RUSTSEC-2024-0370 (`proc-macro-error` sem manutenção, só em tempo de build) e
  RUSTSEC-2024-0429 (`glib::VariantStrIter` unsound; o app não usa `Variant`). Os dois
  ficam ignorados, com motivo, **só** no `app/src-tauri/deny.toml` e no `make
  app-check`; o `cargo audit`/`cargo deny` da raiz continuam sem exceção. Saem quando o
  Tauri passar para gtk-rs ≥ 0.20. Condições: nenhum código do app usa `glib::Variant`
  (o validador confere por `grep`), e uma exceção nova no `deny.toml` do app volta ao
  planner. Se o usuário recusar, o M5 não fecha até o Tauri subir o gtk-rs. A Fase 29a
  viu `advisory-not-detected` para a 0429: se o `cargo audit` também não a acha, a
  exceção sai (D64, Fase 29b) e fica só a 0370.
- **RA8. Canais da sessão controlados pelo mesmo uid** (handoff 21b; D57; **aberto,
  vai ao usuário antes de fechar o M4**, porque também toca a Fase 25; não bloqueia a
  29a, cujo código é o mesmo com qualquer resposta). D57 fecha a injeção pelo ambiente,
  mas `/run/user/<uid>` é do usuário: um pacote malicioso pode apagar o
  `wayland-<n>` ou o `bus` e pôr no lugar um proxy que repassa ao compositor, e o app
  aberto depois disso digita o PIN e a senha mestra e mostra o valor revelado por ele.
  O compositor roda como o usuário, então não há âncora de root para conferir o par
  (pelo `/proc` é a conferência por nome que D46 descartou). O mesmo vale para o
  terminal: um `ward` falso antes no `PATH`, ou um alias no `~/.bashrc`, lê a senha
  mestra do `ward destravar` (Fase 25). Proposta do planner: registrar em "Fora do
  modelo de ameaça" como "sessão gráfica ou shell do usuário adulterados por um
  processo do mesmo uid", com o que já reduz o caso (daemon como único pai, ambiente
  fixo, um só socket Wayland, sem X11, senha mestra nunca no Claude). Alternativa:
  conferir o compositor pela cadeia de processos até o gerente de login do root, que é
  específica do COSMIC, frágil e forjável por um compositor aninhado. O
  `/tmp/nvidia-mps` saiu daqui por D64; tema do GTK (`settings.ini`, `gtk.css`), dconf e
  caches de fonte, que o app lê do home, entram aqui (handoff 29a), e também o
  `~/.nv/nvidia-application-profile-globals-rc` que o driver da NVIDIA ainda lê com
  `__GL_APPLICATION_PROFILE=0` (D67).
- **RA9. Menu da bandeja no barramento da sessão** (handoff 39; registrado na Fase
  39c; **vai ao usuário com o M5**; S10, S13). A bandeja (V5.23) usa o AppIndicator: o
  menu é exportado pelo `dbusmenu` no barramento da sessão, e qualquer processo do
  mesmo uid lê os rótulos e "clica" nos itens. O que um pacote malicioso ganha com isso:
  - **ler:** só textos fixos ("Cofre destravado"/"Cofre travado", "Trava sozinho após
    4 h sem uso"), que o papel agente já obtém pelo `estado`; nenhum nome de entrada,
    grupo ou valor entra no menu;
  - **clicar:** "Travar agora" (trava o cofre: negação de serviço que ele já tinha com
    `ward travar`), "Abrir o Ward"/"Destravar…" (só mostram a janela; a senha é
    digitada nela) e "Sair" (fecha o app, que ele já podia matar). Nenhum item revela,
    copia, aprova, muda nível ou destrava; id desconhecido ou ação fora do estado
    mostrado não fazem nada (`app/nucleo/src/bandeja.rs`,
    `ataque_id_forjado_no_menu_nao_faz_nada`);
  - **canal novo do app no barramento:** o app passa a ter um nome e objetos no
    barramento da sessão (o AppIndicator registra o ícone no `StatusNotifierWatcher`).
    Quem controla o barramento (RA8) já controla mais que isso;
  - **o ícone em disco:** o PNG fixo vai para `$XDG_RUNTIME_DIR/ward-bandeja` (0700,
    conferida sem seguir link).
  Proposta: aceitar como está (nada sai do cofre e as ações são as que o mesmo uid já
  tem), com a regra de que item novo no menu que revele, copie, aprove ou mude estado
  além de travar volta ao planner. Alternativa: tirar a bandeja (V5.23 sai do escopo).

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
  `gravar_cofre`/`selar` fora da API pública; reauditoria (D53–D55): geração
  queimada ao selar, máscara com a tabela única de D54 (entregas de várias linhas,
  formas de texto por linha, formas novas, oráculo alinhado) e último rótulo do domínio
  sem dígito.
- [ ] M3: `wardd` endurecido (S5, S8), papéis de D5, destravar/travar, trava
  automática, PIN, níveis e o motor de aprovação (S9, S10) testados em Rust; nenhuma
  operação do socket devolve segredo a um cliente comum; V3.2 em Docker (D49);
  conferência do servidor por D46 e papel do app/host por D47 (confirmadas); arquivo
  de estado com `vista` e `queimada` (D53); entrada `delta-api-simbolos` (D59) e
  variação de D58 no teste cruzado; variações de D61 (JSON numa linha, YAML/dotenv,
  tab em linha binária); espera de D20 por papel e guardada no estado, `mlockall`,
  geração do `criar_cofre` e identidade pelo fd (D62); papel app só ao pid do lançador
  e app de teste pelo lançador do daemon de teste (D63).
- [ ] M4: `ward lista/run/ssh/travar/destravar` contra Postgres e SSH de teste (senha,
  chave com agente temporário, salto, túnel de banco), saída mascarada, anexos
  temporários limpos, nada lido ou gravado em `~/.ssh`; usuário do `sftp`/`scp` por
  `-o User=` (D56); RA1, RA2 e RA8 resolvidos.
- [ ] M5: app Tauri com todas as telas do M0, janela de aprovação com os 4 botões e PIN,
  bandeja, "Conectar" do `servidor` (D3), S11 e S12 conferidas; app com D46, D47 e
  D57 (Fases 21b, 29a e 23); RA7 aceito pelo usuário ou resolvido; roteiro da janela nativa
  conferido pelo usuário.
- [ ] M6: extensão instalada por política, preenchimento por origem, salvar login novo,
  `ward login` e `ward playwright` no WordPress de teste, phishing recusado.
- [ ] M7: backup local de 30 versões, externo em pendrive desligado por padrão,
  restaurar e kit de emergência.
- [ ] M8: skill `ward` nova com fluxo e deny rules, pronta para instalar.
- [ ] M9: `sudo make install` e `.deb` instalam tudo em pastas de root; o `wardd`
  instalado sobe pelo `wardd@<uid>.socket` do sistema, com o `sudo` só na instalação; nada em `~/.ssh` nem no `SSH_AUTH_SOCK` muda.
- [ ] M10: etapa final guiada documentada e verificável (mover chaves pessoais do
  KeePassXC para o `~/.ssh` e as de servidor para entradas `servidor`, gnome-keyring,
  `montagem.md`, senhas do Chrome), feita pelo usuário; o KeePassXC só sai depois de
  todos os milestones aprovados.
- [ ] Auditoria final do security-auditor sobre S1–S15 sem achado crítico aberto.
