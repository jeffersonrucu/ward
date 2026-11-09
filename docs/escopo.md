# Escopo — Ward

Decisões fechadas no grilling de 2026-11-08. Este arquivo é a fonte do planner: PRD,
plano e contrato saem daqui. Mudou uma decisão? Edite aqui primeiro.

**Nome: Ward.** CLI `ward`, daemon `wardd`, skill `ward`, variáveis `WARD_*`, repositório `git@github.com:jeffersonrucu/ward.git` (branch principal `master`). "Cofre" no texto é o substantivo, não o nome.

## Objetivo

Um cofre **offline**, só de **credenciais de trabalho**, com CLI, app desktop e
extensão do Chrome. Ele entrega credenciais ao Claude **sem que ele veja nenhum
segredo**. Substitui o KeePassXC, que é desinstalado quando o cofre cobrir 100%.

O problema que resolve: no KeePassXC não há tipos de credencial, cada entrada é
montada à mão e a integração com o agente não padroniza. Aqui cada tipo já sabe
quais campos tem e como entrega a credencial para a ferramenta.

**Segurança é o requisito número um.** Na dúvida entre conveniência e segurança,
escolha segurança e registre a decisão.

## Arquitetura

- **Daemon `wardd`**: sobe pelo **systemd do sistema** e roda como o usuário. Um par
  `wardd@.socket`/`wardd@.service` cria o socket em `/run/ward/<uid>/wardd.sock`
  (pasta do root, socket 0600 do usuário) e entrega o fd ao `wardd` (`User=<uid>`), que
  sobe na primeira conexão (decisão do usuário, 2026-11-08). Ele é **o único processo
  que toca em segredo**. CLI, app e extensão são clientes dele, por esse socket. O app
  (`ward-app`) e o host da extensão (`ward-nm`) são instalados setgid para grupos
  próprios (`ward-app`, `ward-nm`), e o daemon reconhece cada um pelo grupo que o
  kernel informa na conexão (decisão do usuário, 2026-11-08). O app só recebe o papel
  quando é aberto pelo próprio daemon, com um ambiente que o daemon monta (sessão
  Wayland e barramento do usuário nos caminhos padrão, sem X11); aberto por outro
  processo, ele pede ao daemon a instância verdadeira (PRD D57, planner, 2026-11-08).
- **Núcleo em Rust**: formato do arquivo, criptografia, tipos, máscara de saída e
  protocolo. O daemon e a CLI usam o núcleo.
- **App desktop em Tauri + Svelte**: cliente do daemon. Telas abaixo.
- **Extensão do Chrome**: fala com o daemon por native messaging.
- **Só Linux por enquanto** (Pop!_OS 24.04, COSMIC, Wayland). Tudo o que depende do
  sistema fica em módulos de plataforma isolados: iniciar com a sessão, detectar o
  bloqueio de tela e a suspensão, socket local, agente SSH temporário, proteção de memória e
  identificação do processo que chama. O núcleo, o formato, os tipos e a interface
  não dependem de Linux. Windows e macOS virão depois.
- **Instalação em `/usr/local`** (dono root), com `sudo make install` ou `.deb`.
  O `sudo` é pedido **uma vez**, na instalação: ela instala as units do sistema, cria
  os grupos `ward-app` e `ward-nm`, instala os dois binários setgid e roda
  `systemctl enable --now wardd@<uid>.socket`. Depois disso, o uso diário não pede
  `sudo`. A extensão é instalada por política do Chrome (`/etc/opt/chrome/policies`), sem
  modo desenvolvedor. Motivo: o Claude roda como o usuário e não pode trocar os
  binários nem a extensão por versões falsas.

## Credenciais

### Tipos prontos (v1)

| Tipo | Campos principais | Entrega (exemplos) |
|---|---|---|
| login | URL, usuário, senha, TOTP | `WARD_URL`, `WARD_USER`, `WARD_PASS`, `WARD_TOTP`; preenchimento pela extensão |
| banco | motor (postgres, mysql, mongo, redis, sqlserver), host, porta, base, usuário, senha, túnel SSH opcional (referência a um `servidor`) | `PGHOST`/`PGPASSWORD`…, `MYSQL_PWD`…, `REDISCLI_AUTH`, `MONGODB_URI`, `DATABASE_URL` |
| servidor | host, porta, usuário, senha **ou** chave (+ passphrase), servidor de salto opcional | `ward ssh` (agente temporário por conexão) |
| api | URL base, token, nome da variável (ex.: `GITHUB_TOKEN`) | `WARD_TOKEN` + a variável escolhida |
| aws | access key, secret, região, role, session token | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`… |
| smtp | host, porta, usuário, senha, segurança | `SMTP_*` |
| ftp/sftp | protocolo, host, porta, usuário, senha ou chave | `WARD_*` |
| nota segura | texto | — |
| genérico | só campos extras | `WARD_<CAMPO>` |

- Cada campo declara se é segredo. Segredo nunca aparece em `ward lista`.
- Toda entrada aceita **campos extras** (nome, valor, segredo sim/não).
- **Tipos customizados ficam para a v2.**
- Toda entrada tem `grupo` e `ambiente` **obrigatórios**: `dev`, `stage` ou
  `prod`. `projeto` e tags são opcionais. O app tem autocompletar de grupo.
- **Grupo** é só uma pasta genérica para organizar entradas (ex.: `acme`, `beta`,
  `gama`, `clientex`); não é escopo de permissão nem de isolamento. O nível de acesso
  continua por entrada (decisão do usuário, 2026-11-08).
- **TOTP**: o cofre gera o código; o agente recebe o código, nunca a semente.
- **Anexos** cifrados no cofre (chave SSH, `.pem`, `.env`, `.ovpn`). No `ward run`
  viram arquivo temporário em `/run/user/<uid>/ward/`, permissão 0600, apagado ao
  fim do comando. A ferramenta recebe só o caminho (`WARD_ARQUIVO_<NOME>`).
- **Gerador de senha** com tamanho e conjuntos de caracteres configuráveis (há
  painéis que não aceitam símbolo), no formulário e numa tela própria.

## Claude (agente)

**Princípio: o Claude recebe o acesso, nunca a credencial.** O cofre guarda a
senha e faz o login quando o Claude precisa. Depois disso, o Claude fica livre. O
cofre não restringe comandos dentro do servidor, nem no banco, nem no painel.

### Comandos

| Comando | Faz |
|---|---|
| `ward lista [--json]` | Entradas que o agente pode ver: nome, tipo, grupo, ambiente, projeto, URL/host. As bloqueadas não aparecem |
| `ward run <entrada> -- <cmd…>` | O **daemon** executa o comando com a credencial no ambiente do processo filho, nas variáveis do tipo. Devolve a saída mascarada e o código de saída |
| `ward ssh <entrada> [cmd]` | Conecta no servidor (senha ou chave, salto) |
| `ward login <entrada>` | Pela extensão: confere o domínio da aba, preenche, envia o formulário e limpa os campos |
| `ward playwright <entrada>` | Faz o login e entrega a sessão salva (storage state) para o teste. O Playwright é de primeira classe: o Claude usa bastante |

- **Não existe operação que devolva segredo para a CLI ou para o agente.** Revelar
  e copiar existem só no app.
- **Máscara de saída**: os valores secretos da entrada e as variações óbvias
  (URL-encoded, base64, dentro de `DATABASE_URL`) viram `••••` na saída devolvida.
- **Domínio**: `ward login` e o preenchimento recusam se o domínio da aba não
  bater com a URL da entrada.

### Nível de acesso por entrada

- **bloqueado**: o agente nem vê a entrada.
- **com aprovação** (padrão): cada uso abre a janela de aprovação.
- **livre**: o agente usa sem perguntar.

**O ambiente não muda as regras.** `prod` tem os mesmos níveis e botões que `dev` e
`stage`, inclusive "livre" e "Permitir sempre" (decisão do usuário). O ambiente serve
para organizar, filtrar e aparecer em destaque na janela de aprovação.

Mudar o nível exige o PIN. O nível vale para `run`, `ssh`, `login` e `playwright`.

### Janela de aprovação

- Abre em primeiro plano quando há pedido. Mostra a entrada, o ambiente, o comando
  exato e quem pediu (cadeia de processos, ex.: `claude → bash → ward run`).
- Botões: **Negar**, **Permitir uma vez**, **Permitir por 1 hora**, **Permitir
  sempre** (a entrada vira "livre").
- Todos os botões menos "Negar" pedem o **PIN**.
- Fechar a janela nega. Sem resposta em 2 minutos, o pedido expira (nega).
- Não há tela de aprovações, nem de "cortar permissão": voltar atrás é mudar o
  nível na tela da entrada.

### SSH (sem agente SSH global)

- **O Ward não é o agente SSH da máquina.** As chaves pessoais (GitHub etc.) ficam no
  `~/.ssh` com o agente do sistema (`gcr-ssh-agent`), fora do Ward. O Claude pode usá-las
  normalmente (`git push`), como hoje.
- Se algum servidor tiver chave no `~/.ssh`, o Claude usa direto (`ssh`), e o Ward não
  interfere em nada: ele não intercepta, não bloqueia nem audita o SSH fora do `ward ssh`.
- As credenciais dos **servidores de trabalho** (senha ou chave) ficam só nas entradas
  `servidor` do Ward, e são usadas só pelo `ward ssh` (e por `ward run` com túnel). Em
  `~/.ssh` não haverá nada dos servidores.
- `ward ssh` com chave: o daemon sobe um agente SSH **temporário**, só para aquela
  conexão, acessível só pelo processo `ssh` filho, e o derruba ao terminar. A chave
  privada nunca sai do daemon. Com senha: `sshpass` lendo do ambiente, nunca de argv.
- Para o usuário, `ward ssh` sempre funciona direto, sem PIN. Para o Claude, vale o
  nível de acesso da entrada. Quem chamou é conhecido pelo próprio `ward ssh`, sem
  heurística de árvore de processos.

### Skill `ward` (substitui a skill `cofre`)

Reescrita **do zero** para o cofre novo, com os comandos acima e o fluxo "listar,
cruzar grupo, ambiente e tipo com a tarefa, avisar qual entrada vai usar". As
regras de deny do `~/.claude/settings.json` passam a cobrir o arquivo do cofre e os
backups. Os scripts antigos (`cofre-lista`, `cofre-run`, `cofre-ssh`,
`cofre-abrir`) saem junto com o KeePassXC. Até lá a skill antiga continua.

## Destravar e travar

- Destrava **só com a senha mestra**, pela janela do app ou pela CLI no terminal do
  usuário. Desbloqueio por TPM está fora.
- Pedido do agente com o cofre travado: o app abre e pede a senha mestra ao
  usuário. O pedido espera até 2 minutos.
- Trava sozinho: bloqueio de tela, suspensão e **4 horas sem uso** (cada uso zera).
- "Travar agora" no app, na bandeja e em `ward travar`.
- **PIN** separado da senha mestra, de 4 a 6 dígitos. Só aprova usos e mudanças de
  nível; não destrava. Depois de 5 erros seguidos, o cofre trava.

## App desktop

Telas:
1. **Credenciais**: lista agrupada por grupo, filtro por tipo e ambiente, busca.
   A entrada abre o formulário do tipo dela, com revelar, copiar (a área de
   transferência é limpa em 30 s), editar, anexos, TOTP e nível de acesso.
2. **Gerador de senha** avulso.
3. **Configurações**: PIN, senha mestra, tempo de trava, backup externo.

Além das telas: destravar, primeiro uso (criar cofre, senha mestra, PIN), janela
de aprovação, ícone na bandeja, restaurar backup e kit de emergência.

Interface toda em português e só em tema escuro (sem modo claro nem alternância
de tema).

Sem leitor de tela (decisão do usuário, 2026-11-09): a acessibilidade (AT-SPI) fica
desligada no app, porque por ela qualquer processo do usuário leria o texto da tela,
inclusive o valor revelado (PRD D64).

## Extensão do Chrome

- Preenche para o usuário: sugere as entradas do domínio, preenche usuário, senha
  e TOTP.
- Preenche para o Claude: `ward login` (domínio conferido, envia, limpa os campos).
- Salvar login novo: oferece salvar e pede tipo, grupo e ambiente.
- Só o Chrome.

## Backup

- **Local automático**: uma cópia cifrada com data a cada mudança, 30 versões.
- **Externo**: só pendrive, opcional e **desligado por padrão**. Quando ligado,
  copia sempre que o pendrive escolhido for conectado.
- **Restaurar** pelo app, escolhendo a versão.
- **Kit de emergência**: página para imprimir, com onde está o backup e espaço para
  anotar a senha mestra à mão (o cofre nunca preenche).

## Migração e saída do KeePassXC

- Cadastro **manual** das credenciais. Sem importação de `.kdbx`.
- As contas pessoais vão para o 1Password ou para o gerenciador do Chrome.
- **Etapa final guiada**: mover as chaves SSH pessoais que hoje o KeePassXC carrega
  no agente para o `~/.ssh` (agente do sistema) e as dos servidores para entradas
  `servidor` do Ward; devolver o chaveiro da máquina ao gnome-keyring (inclusive
  a chave "Chrome Safe Storage", para o Chrome não perder as senhas), desfazer o que
  `~/.claude/skills/cofre/references/montagem.md` descreve (override do keyring,
  ativação D-Bus, autostart, PAM), conferir que o Chrome mantém as senhas e só então
  desinstalar o KeePassXC.
- A troca é em etapas: o KeePassXC continua funcionando até o último marco passar
  na validação.

## Segurança

### Modelo de ameaça (do que o cofre defende)

1. **O próprio agente**, por erro ou por prompt injection (página, issue ou arquivo
   que manda ele exfiltrar senhas). Defesa: ele nunca recebe segredo, a máscara de
   saída, os níveis de acesso, a aprovação com PIN e a checagem de domínio.
2. **Pacote ou script malicioso rodando como o usuário** (npm, pip, cargo). Defesa:
   não há operação que devolva segredo, binários e extensão em pasta do root, peer
   credentials no socket, daemon não-dumpable e sem `ptrace`.
3. **Disco, backup ou pendrive roubados.** Defesa: tudo cifrado em repouso, com
   Argon2id e AEAD.
4. **Página de phishing** recebendo a senha pelo preenchimento. Defesa: checagem de
   domínio na extensão e no `ward login`.

### Fora do modelo de ameaça (dito com honestidade)

- Root comprometido, kernel ou firmware.
- Leitura, por outro processo do mesmo usuário, do segredo entregue ao filho do
  `ward run` ou ao `sshpass` do `ward ssh` enquanto ele roda: variáveis em
  `/proc/<pid>/environ` e anexos 0600 em `/run/user/<uid>/ward/tmp/` (RA1; decisão do
  usuário, 2026-11-09). O kernel trata processos do mesmo uid como seus e não há
  mitigação genérica. O Claude continua sem ver o segredo pela conversa; o anexo é
  apagado no fim e a janela dura só a execução do comando aprovado.
- Alguém com acesso físico à máquina destravada.
- Um processo do usuário feito de propósito para driblar a máscara (ex.: inverter a
  senha antes de imprimir). Quem barra é a aprovação, que mostra o comando exato.
- Leitura do campo de senha da página no instante do preenchimento por um script
  feito para isso. Enviar o formulário na hora e limpar os campos reduz essa janela,
  mas não zera.
- Volta do cofre a uma versão antiga feita com o daemon parado, quando um processo do
  mesmo usuário troca juntos o arquivo do cofre e o arquivo de estado do daemon por
  cópias antigas e coerentes (decisão do usuário, 2026-11-08). Sem root ou TPM não há
  onde guardar o número da versão fora do alcance desse processo. O contador de geração
  pega a troca com o daemon rodando e a troca só do arquivo do cofre. Nenhuma operação
  passa a devolver segredo: voltam só níveis de acesso e o PIN antigo.
- Chrome depurado por um processo do usuário que seja ancestral dele (decisão do
  usuário, 2026-11-08). O app e o host da extensão são setgid e não-dumpable desde o
  `exec`, mas o Chrome e os processos auxiliares do WebKitGTK não são endurecidos e
  ficam protegidos só pelo Yama (`ptrace_scope=1`) contra quem não é ancestral. Um
  pacote malicioso que feche o Chrome e o abra de novo como filho pode se prender a
  ele por `ptrace` e fazer o navegador mentir a origem da aba; esse Chrome recebe o
  preenchimento de qualquer entrada `login`. É o mesmo limite da leitura do campo de
  senha por um script feito para isso. Também fica fora a negação de serviço de matar
  o `wardd` (o systemd o sobe de novo, sem que o processo receba a senha, e a espera
  das senhas erradas do agente continua de onde parou; PRD D62).

### Invariantes (o contrato de validação deve cobrir cada uma)

- **S1** Nenhuma operação do socket devolve segredo à CLI. Revelar e copiar só pelo
  app; preencher só pela extensão no domínio certo.
- **S2** Segredo nunca vai para disco em claro. Anexo temporário só em
  `/run/user/<uid>/ward/`, 0600, apagado ao fim (inclusive se o comando falhar ou
  for morto).
- **S3** Segredo nunca vai em argv (visível em `/proc/<pid>/cmdline`); só no
  ambiente do processo filho, em arquivo temporário ou em fd.
- **S4** A máscara cobre os valores secretos e as variações declaradas, inclusive
  quando o valor chega quebrado entre dois pedaços da saída.
- **S5** Socket 0600 do usuário em pasta do root (`/run/ward/<uid>/`, criada pelo
  systemd do sistema; o daemon de teste usa um diretório 0700 do usuário),
  conferência do uid do cliente por `SO_PEERCRED`. A CLI e o app conferem que o
  servidor é o `wardd` instalado (pasta do root e par com uid 0 no `SO_PEERCRED`)
  antes de enviar a senha mestra.
- **S6** Criptografia só com bibliotecas auditadas: Argon2id (parâmetros
  registrados no cabeçalho, mínimo ~256 MiB / 3 passes), XChaCha20-Poly1305 com o
  cabeçalho como dado autenticado, chave de dados aleatória embrulhada pela chave
  derivada. Um byte alterado no arquivo faz a abertura falhar. Nada de cripto feita
  à mão.
- **S7** Gravação atômica (temp + fsync + rename). Uma gravação interrompida nunca
  corrompe a versão anterior.
- **S8** Daemon com `PR_SET_DUMPABLE=0`, sem core dump, segredos zerados da memória
  ao travar e após o uso (`zeroize`). Daemon recusa iniciar com `LD_PRELOAD` e
  similares.
- **S9** Mudar nível de acesso exige PIN. 5 erros de PIN travam o cofre.
- **S10** Aprovação expira em 2 min e fechar
  nega.
- **S11** Tauri com capabilities mínimas (só os comandos que o app usa), CSP
  estrita, nenhum conteúdo remoto, nenhum segredo na webview fora do "revelar".
- **S12** Área de transferência limpa em 30 s.
- **S13** Logs de depuração nunca contêm segredo.
- **S14** Dependências mínimas; `cargo audit` e `cargo deny` limpos; lockfiles no
  git; frontend sem dependência de runtime além do necessário.
- **S15** O repositório nunca contém segredo real (varredura por gitleaks ou
  equivalente).

### Regras de desenvolvimento

- **Toda a missão usa um cofre de teste**: arquivo em `.dev/` (fora do git), senha
  mestra e PIN de teste, credenciais falsas. Postgres, SSH e WordPress de teste
  sobem em Docker (`docker compose`; o usuário está no grupo `docker`).
- **O cofre real do usuário nunca é tocado pela missão.** Ele nasce depois, na
  instalação feita pelo usuário.
- QA da interface: a interface Svelte roda também no Chrome em modo de teste,
  ligada ao daemon de teste. A janela nativa do Tauri o usuário confere por um
  roteiro curto no fim de cada milestone com UI.
- O Claude não consegue aprovar sozinho por design. O QA testa "negar" e "expirar",
  e o "aprovar com PIN" usa o PIN do cofre de teste.

## Missão 0 — Design no FigCraft

Antes de qualquer código, o design é montado **no FigCraft, pelo MCP do FigCraft**
(`mcp__figcraft__*`, backend headless). Até 2026-11-08 o design era feito no Figma; o
usuário trocou a ferramenta nessa data (ver "Arquivo de design"). Direção visual
(usuário, 2026-11-08): tudo arredondado e com cara de produto premium. Essa missão
produz:

- Design tokens como variables (cores, tipografia, espaçamento, raio, larguras). Só tema
  escuro: não existe modo claro nem alternância de tema (decisão do usuário,
  2026-11-08).
- Componentes base (botão, campo, campo secreto com revelar/copiar, select, badge
  de ambiente `dev`/`stage`/`prod`, badge de tipo, item de lista, modal, toast).
- Telas e estados:
  - primeiro uso;
  - destravar (e erro de senha);
  - Credenciais vazia e cheia, agrupada por grupo, com filtros;
  - formulário de cada um dos 9 tipos (novo e edição), com anexos e TOTP;
  - nível de acesso com o pedido de PIN;
  - gerador de senha;
  - Configurações;
  - restaurar backup;
  - kit de emergência para imprimir.
- Janela de aprovação: normal, `prod` (ambiente em destaque), PIN errado, PIN
  bloqueado e expirando.
- Menu da bandeja.
- Popup da extensão do Chrome: sugestões do domínio, salvar login novo e cofre
  travado.
- A página "Inspirações" já vem montada pelo usuário: o design parte dela.

### Arquivo de design

Fonte de verdade: o arquivo do FigCraft que o usuário criou,
`/home/jeffersonoliveira/documentos/Ward.figcraft`. Todo o design é feito e ajustado
nele (não se cria outro arquivo) e cada fase de design termina salvando o arquivo
(`save_file`). Páginas:

| Página | Uso |
|---|---|
| Inspirações | Referências do usuário. Só leitura: ler antes de desenhar, não editar |
| Design System | Variables (tokens), estilos e componentes |
| Layout | Telas e estados, usando instâncias do Design System |

Os ids dos nodes no FigCraft ficam em `docs/mission/design-nodes.md`.

O arquivo do Figma antigo (`Cofre`, fileKey `bzu9PxxFur2tU3vlNCHvsY`) vira **só
histórico e fonte de leitura** para a migração; nada mais é desenhado nele. A página
Inspirações dele (`1:3`,
https://www.figma.com/design/bzu9PxxFur2tU3vlNCHvsY/Cofre?node-id=1-3) continua só
leitura e é a referência visual quando o `Ward.figcraft` não tiver as imagens.

O milestone 0 fecha com a **aprovação do usuário** no FigCraft. As fases de UI dos
milestones seguintes implementam a partir desses nodes.

## Fora do escopo

Serviço Secreto e chaveiro da máquina (ficam com o gnome-keyring), contas pessoais,
sincronização e celular, Firefox e outros navegadores, desbloqueio por TPM,
importação de `.kdbx`, tela de atividade e registro de uso, tela de saúde do cofre,
tela de aprovações, tipos customizados (v2), restrição de comandos dentro dos
servidores, Windows e macOS (por enquanto).

## Pendências (não bloqueiam o plano)

- Ícone do produto: definir na missão 0.
- Conferência do servidor (PRD D46) e identificação do app e do host por grupo setgid
  (PRD D47): **resolvidas** pelo usuário em 2026-11-08 e registradas em "Arquitetura",
  S5 e "Fora do modelo de ameaça".
- Sessão gráfica ou shell adulterados por um processo do mesmo uid (proxy no lugar
  do socket Wayland ou do barramento em `/run/user/<uid>`, `ward` falso no `PATH`):
  **a decidir pelo usuário antes de fechar o M4** (PRD RA8). Proposta: registrar em
  "Fora do modelo de ameaça".
