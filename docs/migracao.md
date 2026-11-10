# Migração do KeePassXC para o Ward

Guia da etapa final (Milestone 10). **Quem executa é você**, no seu terminal e nas
suas janelas, um passo por vez. A missão não executa nenhum passo deste guia e o
Claude não roda nenhum destes comandos: tudo aqui toca no `~/.ssh`, no chaveiro ou
na sessão, e isso é seu (PRD R19, V10.2).

O KeePassXC continua instalado e funcionando até o passo 8. Até lá, todo passo tem
volta.

## Antes de começar

- **Nunca cole senha, chave, passphrase, token, código TOTP nem a saída de um
  comando deste guia na conversa com o Claude.** Não rode estes comandos pelo Claude
  Code (nem com `!`, que joga a saída na conversa). Use um terminal seu, fora dele.
- O Ward já está instalado (`sudo make install` ou o `.deb`), o cofre real foi
  criado no app (primeiro uso: senha mestra e PIN) e o kit de emergência está
  impresso, com a senha mestra anotada à mão.
- Destrave o Ward pelo app. No terminal, use sempre o caminho completo,
  `/usr/local/bin/ward destravar`, nunca o `ward` que o `PATH` acha: um programa malicioso
  rodando como você pode pôr um `ward` falso antes no `PATH` (em `~/.local/bin`, por
  exemplo), que mostra o mesmo pedido e guarda a senha mestra. O caminho completo pula
  o `PATH`, mas não um `~/.bashrc` ou `~/.zshrc` adulterado (escopo › "Fora do modelo
  de ameaça", RA8). Os outros comandos `ward` deste guia não pedem a senha mestra.
- O app do Ward tem o botão **Carregar de arquivo** no campo "Chave privada" das
  entradas `servidor` (Fase 53a). Sem ele, não faça o passo 2b: nenhuma chave
  privada passa pela área de transferência.
- Tenha uma cópia do arquivo `.kdbx` do KeePassXC (`~/cofre/`) num lugar seguro
  (pendrive ou backup cifrado). Ele já é cifrado; nunca gere uma cópia em texto
  dele.
- Crie a pasta de backups e a pasta temporária em memória para as chaves (tmpfs,
  some ao desligar):

  ```bash
  install -d -m 700 ~/ward-migracao
  install -d -m 700 "/run/user/$(id -u)/migracao"
  ```

  A `~/ward-migracao` recebe arquivos de configuração, a skill antiga e a cópia dos
  bancos de senhas do Chrome. Esses bancos são cifrados pela chave "Chrome Safe
  Storage", mas quem tiver o banco e o chaveiro abre as senhas: trate a pasta como
  sensível (0700, fora de backup em nuvem). Chave privada e senha em claro nunca vão
  para ela.
- Área de transferência: só valores curtos (senha, passphrase, o valor do "Chrome
  Safe Storage"), copiados no KeePassXC e colados direto no destino. O KeePassXC
  limpa a área sozinho depois de alguns segundos; mesmo assim, no fim de cada passo
  que copiou algo, rode `wl-copy --clear`. Se o painel do COSMIC guardar histórico da
  área de transferência, limpe o histórico também.
- Os passos 4, 5 e 6 são feitos de uma vez, na mesma sessão: o Chrome fica fechado
  do começo do passo 4 até o passo 6, e a skill `cofre` para de funcionar no passo 4.

Em cada passo: faça, confira em "Conferir" e só avance se deu certo. Se não deu,
siga "Voltar atrás" e pare.

## Passo 1 — Cadastrar as credenciais à mão no Ward

Sem importação de `.kdbx` (escopo › "Migração e saída do KeePassXC"). Só as
credenciais de **trabalho** vão para o Ward; as contas pessoais vão para o 1Password
ou para o gerenciador do Chrome.

1. Abra o app do Ward e destrave com a senha mestra.
2. Para cada entrada de trabalho do grupo exposto do KeePassXC: no Ward, **Nova
   credencial**, escolha o tipo (login, banco, servidor, api, aws, smtp, ftp/sftp,
   nota segura, genérico), preencha grupo, ambiente (`dev`, `stage` ou `prod`) e os
   campos. Para o valor secreto, copie no KeePassXC e cole direto no campo do Ward.
   Se preferir, gere senha nova no gerador do Ward e troque no serviço.
3. Entradas `servidor` com chave ficam para o passo 2. Com senha, cadastre já.
4. Escolha o nível de cada entrada (padrão "com aprovação"; "bloqueado" para o que o
   Claude nunca deve usar).
5. Ao terminar, limpe a área de transferência:

   ```bash
   wl-copy --clear
   ```

### Conferir

- `ward lista` mostra cada entrada não bloqueada com nome, tipo, grupo e ambiente,
  sem nenhum segredo. Conte e compare com o número de entradas de trabalho do
  KeePassXC.
- Use uma entrada de cada tipo, por exemplo `ward run <entrada-banco> -- psql -c
  'select 1'` (aprova com o PIN) e, numa entrada `login`, a sugestão da extensão no
  site certo.

### Voltar atrás

- No app, abra a entrada e use **Excluir entrada**. O KeePassXC não muda neste passo;
  a skill `cofre` continua funcionando.

## Passo 2 — Mover as chaves SSH: pessoais para o `~/.ssh`, de servidor para entradas `servidor`

Hoje o KeePassXC carrega chaves no agente do sistema (`gcr-ssh-agent`, em
`/run/user/<uid>/gcr/ssh`). Depois da migração:

- **chaves pessoais** (GitHub, GitLab etc.) ficam no `~/.ssh`, com o agente do
  sistema, fora do Ward;
- **chaves de servidor de trabalho** ficam só em entradas `servidor` do Ward. No
  `~/.ssh` não fica nada dos servidores, nem as chaves que hoje só existem lá.

### 2a. Chaves pessoais

Para cada chave pessoal no KeePassXC (entrada com a aba **Agente SSH** ligada):

1. Na entrada, aba **Avançado › Anexos**, salve o anexo da chave privada na pasta em
   memória: `/run/user/<uid>/migracao/<nome>` (ex.: `id_ed25519_github`), nunca
   direto no `~/.ssh`. Uma chave sem passphrase gravada no `~/.ssh` (disco) ficaria
   lá em claro, mesmo que ganhe passphrase depois. Mantenha a chave cifrada com a
   passphrase; não tire a passphrase.
2. Ajuste a permissão da cópia e gere a pública no `~/.ssh` (pede a passphrase no
   terminal):

   ```bash
   chmod 600 "/run/user/$(id -u)/migracao/id_ed25519_github"
   ssh-keygen -y -f "/run/user/$(id -u)/migracao/id_ed25519_github" > ~/.ssh/id_ed25519_github.pub
   ```

3. Se o `ssh-keygen -y` não pediu passphrase, a chave está sem nenhuma: ponha uma
   agora, ainda na pasta em memória. O comando pede a passphrase nova duas vezes, no
   terminal:

   ```bash
   ssh-keygen -p -f "/run/user/$(id -u)/migracao/id_ed25519_github"
   ```

4. Com a passphrase posta, leve a chave para o `~/.ssh` (o `install` já a cria
   0600) e apague a cópia em memória:

   ```bash
   install -m 600 "/run/user/$(id -u)/migracao/id_ed25519_github" ~/.ssh/id_ed25519_github
   rm -f "/run/user/$(id -u)/migracao/id_ed25519_github"
   ```

5. No KeePassXC, desligue nessa entrada "Adicionar ao agente ao abrir/desbloquear".
6. Se o agente SSH perguntar se deve lembrar a passphrase, **não marque** até o
   passo 5 terminar: hoje "lembrar" grava no KeePassXC, que vai sair.

### 2b. Chaves de servidor

Para cada servidor cuja chave está no KeePassXC:

1. Salve o anexo da chave privada na pasta em memória:
   `/run/user/<uid>/migracao/<servidor>` (nunca no disco).
2. No Ward, crie (ou edite) a entrada `servidor`: host, porta, usuário,
   autenticação **chave**. No campo "Chave privada", use **Carregar de arquivo** e
   escolha o arquivo da pasta em memória: o app manda o conteúdo direto para o
   daemon, sem área de transferência. A passphrase, copie do KeePassXC e cole no
   campo "Passphrase". Servidor de salto, se houver.
3. Apague a cópia e limpe a área de transferência (a passphrase passou por ela):

   ```bash
   rm -f "/run/user/$(id -u)/migracao/<servidor>"
   wl-copy --clear
   ```

Para cada chave de servidor de trabalho que hoje só existe no `~/.ssh` (arquivo
usado por um bloco `Host` no `~/.ssh/config`), ela também vai para o Ward:

1. Copie a chave para a pasta em memória e guarde o `~/.ssh/config` como está (ele
   não tem segredo, só nomes e caminhos):

   ```bash
   install -m 600 ~/.ssh/<chave> "/run/user/$(id -u)/migracao/<servidor>"
   cp -p ~/.ssh/config ~/ward-migracao/config-do-ssh.antes
   ```

2. Crie a entrada `servidor` como no item 2 acima, com **Carregar de arquivo**. Se
   a chave não tiver passphrase, deixe o campo "Passphrase" vazio.
3. Apague a cópia em memória:

   ```bash
   rm -f "/run/user/$(id -u)/migracao/<servidor>"
   ```

4. Não apague nada do `~/.ssh` ainda: a chave e o bloco `Host` saem no passo 3,
   depois de o `ward ssh` conferir esse servidor.

Por fim, com todas as chaves movidas: no KeePassXC, **Ferramentas › Configurações ›
Agente SSH**, desligue a integração.

### Conferir

- `ssh-add -D` esvazia o agente SSH; `ssh-add ~/.ssh/id_ed25519_github` carrega a
  chave do arquivo e pede a passphrase; `ssh-add -l` mostra a impressão digital dela;
  `ssh -T git@github.com` responde com o seu usuário. Faça o mesmo com cada chave
  pessoal.
- `ls -l ~/.ssh` mostra cada chave privada com `-rw-------`.
- `ls -A "/run/user/$(id -u)/migracao"` está vazio.
- No app do Ward, cada entrada `servidor` com chave mostra "chave" na autenticação
  (o valor fica escondido até "revelar").

### Voltar atrás

- Chave pessoal: `rm ~/.ssh/id_ed25519_github ~/.ssh/id_ed25519_github.pub`, religue
  "Adicionar ao agente" na entrada e a integração do Agente SSH no KeePassXC, trave e
  destrave o KeePassXC; `ssh-add -l` volta a mostrar a chave.
- Chave de servidor: no Ward, **Excluir entrada** (ou volte a autenticação para o
  que era). A chave continua no KeePassXC, e a que só existia no `~/.ssh` continua
  lá, porque ela só sai no passo 3.

## Passo 3 — Conferir cada servidor com `ward ssh`

O `ward ssh` aceita a chave do host na primeira conexão e grava no `known_hosts` do
Ward (`~/.local/share/ward/known_hosts`), não no `~/.ssh`. Para a primeira conexão
não confiar no que vier da rede, passe antes a linha que você já tem.

Para cada entrada `servidor` (com senha ou chave):

1. Antes do primeiro `ward ssh` da entrada, grave no `known_hosts` do Ward a linha
   do host que está no seu `~/.ssh/known_hosts` (porta diferente de 22:
   `'[<host>]:<porta>'` no lugar de `'<host>'`):

   ```bash
   ssh-keygen -F '<host>' -f ~/.ssh/known_hosts | grep -v '^#' >> ~/.local/share/ward/known_hosts
   chmod 600 ~/.local/share/ward/known_hosts
   ssh-keygen -l -F '<host>' -f ~/.local/share/ward/known_hosts
   ```

   O `>>` cria o arquivo com a sua umask (em geral 0644), e o `ward ssh` recusa um
   `known_hosts` que grupo ou outros leem; o `chmod 600` o acerta. A última linha
   mostra a impressão digital gravada. Se ela não mostrar nada, o
   host não estava no seu `~/.ssh/known_hosts`: pegue a impressão digital por outro
   canal (painel do provedor, quem administra o servidor) antes de conectar.
2. Servidor com salto: faça o mesmo para o host do salto e para o host de destino.
3. Rode `ward ssh <entrada> hostname`. Com salto e banco com túnel:
   `ward ssh <entrada-com-salto> hostname` e
   `ward run <entrada-banco-com-tunel> -- psql -c 'select 1'`.

### Conferir

- Cada `ward ssh <entrada> hostname` imprime o nome do servidor e sai 0, sem pedir
  senha nem passphrase.
- `ssh-keygen -l -F '<host>' -f ~/.local/share/ward/known_hosts` mostra a mesma
  impressão digital de antes da primeira conexão (a do seu `~/.ssh/known_hosts` ou a
  do outro canal), uma linha por tipo de chave. Diferente: **pare** e descubra o
  motivo antes de usar a entrada.
- O `~/.ssh/known_hosts` não mudou (`ls -l` antes e depois).

### Depois de conferir: tirar do `~/.ssh` as chaves de servidor

Só para os servidores conferidos acima cuja chave existia no `~/.ssh`: apague a chave
e tire o bloco `Host` do `~/.ssh/config` no seu editor (a cópia do arquivo inteiro já
está em `~/ward-migracao/config-do-ssh.antes`).

```bash
rm -f ~/.ssh/<chave> ~/.ssh/<chave>.pub
```

A chave fica só no Ward (e nos backups cifrados do cofre). Não guarde cópia dela em
`~/ward-migracao` nem em outro lugar do disco.

### Voltar atrás

- Nada a desfazer no servidor. Se uma entrada falhar, corrija no app do Ward e
  repita; enquanto isso, a entrada do KeePassXC e o `cofre-ssh` continuam
  funcionando. Não apague a chave do `~/.ssh` de servidor que não conferiu.
- Uma linha errada do `known_hosts` do Ward se tira com
  `ssh-keygen -R '<host>' -f ~/.local/share/ward/known_hosts`.
- O bloco `Host` volta de `~/ward-migracao/config-do-ssh.antes`. A chave apagada do
  `~/.ssh` não volta para o disco: ela continua na entrada do Ward, que é o lugar
  dela.

## Passo 4 — Devolver o chaveiro ao gnome-keyring, inclusive "Chrome Safe Storage"

Hoje o KeePassXC é o Serviço Secreto da máquina, e o Chrome guarda nele a chave
"Chrome Safe Storage", que cifra as senhas salvas do Chrome. Se o gnome-keyring
voltar sem essa chave, o Chrome cria uma nova e deixa de abrir as senhas antigas.
Neste passo a chave e as outras entradas criadas por aplicativos são copiadas para o
gnome-keyring, **antes** de desfazer a montagem (passo 5).

1. Com o KeePassXC ainda como Serviço Secreto, conte os itens do Chrome que ele
   guarda (a resposta tem só caminhos, nenhum valor):

   ```bash
   busctl --user call org.freedesktop.secrets /org/freedesktop/secrets \
     org.freedesktop.Secret.Service SearchItems 'a{ss}' 1 application chrome
   ```

   A resposta é `ao ao` e duas listas de caminhos (itens destravados e travados),
   cada uma começando pelo número de itens dela. Some os dois números e anote: é o
   total de itens do Chrome. Em geral é 1; pode ser mais (por exemplo, uma versão
   antiga do esquema ao lado da nova).
2. Feche o Chrome e os aplicativos que usam o chaveiro (VS Code, Slack e outros
   Electron). `pgrep -a chrome` não mostra nada.
3. Copie os bancos de senhas do Chrome (já cifrados) para o backup, todos os
   `Login Data*` de todos os perfis:

   ```bash
   install -d -m 700 ~/ward-migracao/chrome
   cd ~/.config/google-chrome && find . -maxdepth 2 -name 'Login Data*' \
     -exec cp --parents -p {} ~/ward-migracao/chrome/ \; ; cd -
   ```

   Anote também quantas senhas o Chrome tem (você confere no passo 6): abra antes
   `chrome://password-manager/passwords`, anote o número e feche de novo.
4. No KeePassXC, no grupo exposto ao Serviço Secreto, liste as entradas criadas por
   aplicativos (não as suas credenciais): todos os itens do Chrome contados no item 1
   e, se houver, "Code Safe Storage", "Slack Safe Storage", senhas de rede etc. Para
   cada uma, anote só o **título** e os **atributos** (aba **Avançado**, atributos
   adicionais; no Chrome é em geral `application` = `chrome` e `xdg:schema` =
   `chrome_libsecret_os_crypt_password_v2`). Não anote o valor. Se dois itens tiverem
   exatamente os mesmos atributos, **pare**: o gnome-keyring guarda um só item por
   conjunto de atributos, e copiar os dois perderia um.
5. No KeePassXC, **Ferramentas › Configurações › Integração com o Serviço Secreto**,
   desligue a integração e aplique. Confira que ninguém é dono do nome:

   ```bash
   busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
     org.freedesktop.DBus NameHasOwner s org.freedesktop.secrets
   ```

   Esperado: `b false`.
6. Ligue os segredos do gnome-keyring nesta sessão e confira o dono:

   ```bash
   gnome-keyring-daemon --start --components=secrets
   busctl --user status org.freedesktop.secrets | grep -x 'Comm=gnome-keyring-d'
   ```

   Esperado: a linha `Comm=gnome-keyring-d`. Se não aparecer, pare e siga "Voltar
   atrás".
7. Para cada entrada do item 4, grave no gnome-keyring com o mesmo título e os mesmos
   atributos. O `secret-tool store` pede o valor num prompt (nunca na linha de
   comando): copie o valor no KeePassXC e cole no prompt. Exemplo do Chrome:

   ```bash
   secret-tool store --label='Chrome Safe Storage' \
     application chrome xdg:schema chrome_libsecret_os_crypt_password_v2
   ```

   Se o gnome-keyring pedir para criar o chaveiro padrão, use a mesma senha do seu
   login, para o login destravá-lo sozinho. Depois da última entrada:

   ```bash
   wl-copy --clear
   ```

8. Mantenha o Chrome fechado e siga para o passo 5.

### Conferir

- O `SearchItems` do item 1, rodado de novo, dá o mesmo total de itens do Chrome que
  você anotou, agora servido pelo gnome-keyring. Não conte com rótulo nem com o
  formato do caminho: confira o número.
- Faça o mesmo para cada entrada do item 4, trocando `application chrome` pelos
  atributos dela (o número depois de `a{ss}` é a quantidade de pares).
- `ls -R ~/ward-migracao/chrome` tem os `Login Data*` de cada perfil do Chrome.

### Voltar atrás

1. Apague só o que você criou, e só se o dono do Serviço Secreto ainda for o
   gnome-keyring. O `grep` guarda o `clear`: se o dono for o KeePassXC, o `clear`
   não roda (ele apagaria a cópia do KeePassXC). Para cada entrada do item 4:

   ```bash
   busctl --user status org.freedesktop.secrets | grep -qx 'Comm=gnome-keyring-d' && secret-tool clear application chrome xdg:schema chrome_libsecret_os_crypt_password_v2
   ```

2. `systemctl --user restart gnome-keyring-daemon` (ele volta só com `pkcs11`, como
   na montagem), religue a integração com o Serviço Secreto no KeePassXC e confira
   que `busctl --user status org.freedesktop.secrets` mostra `Comm=keepassxc`.
3. Abra o Chrome: as senhas continuam lá, porque a chave do KeePassXC não mudou.

## Passo 5 — Desfazer a montagem do KeePassXC, um item por vez

Os itens são os de `~/.claude/skills/cofre/references/montagem.md`. Cada arquivo vai
para `~/ward-migracao/` (não é apagado), para poder voltar.

### 5a. Override do keyring

```bash
mv ~/.config/systemd/user/gnome-keyring-daemon.service.d/override.conf ~/ward-migracao/
systemctl --user daemon-reload
```

- **Conferir:** `systemctl --user cat gnome-keyring-daemon.service` não mostra mais o
  `override.conf`.
- **Voltar atrás:** `mv ~/ward-migracao/override.conf
  ~/.config/systemd/user/gnome-keyring-daemon.service.d/` e
  `systemctl --user daemon-reload`.

### 5b. Ativação D-Bus do KeePassXC

```bash
mv ~/.local/share/dbus-1/services/org.freedesktop.secrets.service ~/ward-migracao/
```

- **Conferir:** `ls ~/.local/share/dbus-1/services/` não tem mais
  `org.freedesktop.secrets.service`.
- **Voltar atrás:** `mv ~/ward-migracao/org.freedesktop.secrets.service
  ~/.local/share/dbus-1/services/`.

### 5c. Autostart escondido

```bash
mv ~/.config/autostart/gnome-keyring-secrets.desktop ~/ward-migracao/
```

- **Conferir:** `ls ~/.config/autostart/` não tem mais `gnome-keyring-secrets.desktop`
  (volta a valer o do sistema, em `/etc/xdg/autostart/`).
- **Voltar atrás:** `mv ~/ward-migracao/gnome-keyring-secrets.desktop
  ~/.config/autostart/`.

### 5d. PAM do `cosmic-greeter`

Um PAM errado pode impedir o login. Antes de mexer, abra um console de texto
(Ctrl+Alt+F3), entre e deixe um `sudo -i` aberto lá até o fim do passo 6.

```bash
diff /var/backups/cosmic-greeter.antes-cofre /etc/pam.d/cosmic-greeter
```

A diferença tem de ser só o `auto_start` da linha do `pam_gnome_keyring.so`. Se
houver mais (atualização do sistema mexeu no arquivo), **pare**: devolva só o
`auto_start` à mão, com `sudoedit /etc/pam.d/cosmic-greeter`, em vez de copiar.

```bash
sudo cp -p /etc/pam.d/cosmic-greeter /var/backups/cosmic-greeter.com-cofre
sudo cp /var/backups/cosmic-greeter.antes-cofre /etc/pam.d/cosmic-greeter
```

- **Conferir:** `grep pam_gnome_keyring /etc/pam.d/cosmic-greeter` mostra
  `auto_start` na linha de sessão.
- **Voltar atrás:** `sudo cp /var/backups/cosmic-greeter.com-cofre
  /etc/pam.d/cosmic-greeter` (pelo console aberto, se o login falhar).

### 5e. Sair e entrar na sessão

Saia da sessão e entre de novo (o console do 5d continua aberto).

### Conferir

- `busctl --user status org.freedesktop.secrets` mostra `Comm=gnome-keyring-d`, sem
  o KeePassXC aberto e sem rodar nada à mão.
- O `SearchItems` do passo 4 dá o mesmo total de itens do Chrome, todos na lista de
  destravados (o login destravou o chaveiro, sem pedir senha).
- `echo "$SSH_AUTH_SOCK"` continua `/run/user/<uid>/gcr/ssh` e `ssh -T
  git@github.com` funciona com a chave do `~/.ssh` (agora pode marcar "lembrar a
  passphrase", se quiser).

### Voltar atrás

- Desfaça na ordem inversa (5d, 5c, 5b, 5a), cada um pelo seu "Voltar atrás", saia e
  entre na sessão e religue a integração com o Serviço Secreto no KeePassXC. Confira
  que `busctl --user status org.freedesktop.secrets` mostra `Comm=keepassxc`.

## Passo 6 — Conferir as senhas do Chrome

1. Abra o Chrome e vá em `chrome://password-manager/passwords`.
2. Compare o número de senhas com o que você anotou no passo 4.
3. Abra uma senha salva e peça para mostrar; ela aparece (o Chrome abriu com a chave
   antiga). Entre num site que usa senha salva e confira o preenchimento.

### Conferir

- O número de senhas é o mesmo do passo 4 e o "mostrar" funciona.
- O `SearchItems` do passo 4 continua com o mesmo total de itens do Chrome (ele não
  criou uma chave nova).
- Repita a conferência com os outros aplicativos do passo 4 (VS Code, Slack…).

### Voltar atrás

1. Feche o Chrome (`pgrep -a chrome` não mostra nada).
2. Volte o passo 5 inteiro (seu "Voltar atrás") e o passo 4 (seu "Voltar atrás").
3. Se o Chrome tiver regravado os bancos de senhas, devolva todos os `Login Data*`
   do backup do passo 4, com ele fechado. Os atuais vão antes para
   `~/ward-migracao/chrome-depois` (nada é perdido), e depois voltam os do backup:

   ```bash
   install -d -m 700 ~/ward-migracao/chrome-depois
   cd ~/.config/google-chrome && find . -maxdepth 2 -name 'Login Data*' \
     -exec cp --parents -p {} ~/ward-migracao/chrome-depois/ \; -exec rm {} \; ; cd -
   cd ~/ward-migracao/chrome && find . -maxdepth 2 -name 'Login Data*' \
     -exec cp --parents -p {} ~/.config/google-chrome/ \; ; cd -
   ```

4. Abra o Chrome e confira as senhas com o KeePassXC como Serviço Secreto.

Só avance se as senhas do Chrome estiverem todas lá. Para o registro do V10.4 no
`plan.md`, escreva na missão só a frase "Chrome manteve as senhas", sem nenhum
valor.

## Passo 7 — Instalar a skill `ward` e remover a `cofre` e os scripts antigos

1. Instale a skill e as regras de deny conforme `skill/README.md` (copiar
   `skill/ward/` para `~/.claude/skills/ward/` e juntar as regras de
   `skill/settings-deny.json` ao `permissions.deny` do `~/.claude/settings.json`).
2. Mova a skill antiga e os scripts para o backup:

   ```bash
   mv ~/.claude/skills/cofre ~/ward-migracao/skill-cofre
   install -d -m 700 ~/ward-migracao/bin
   mv ~/.local/bin/cofre-abrir ~/.local/bin/cofre-lista \
      ~/.local/bin/cofre-ssh ~/.local/bin/cofre-run ~/ward-migracao/bin/
   ```

3. Antes de deixar alguma entrada como "livre" (ou de usar "Permitir sempre"),
   saiba que "livre" quer dizer que o Claude **consegue ler o segredo** dela: um
   comando que recodifique a variável de um jeito que a máscara não cobre mostra o
   valor (RA3), e a skill e as regras de deny só pegam os casos comuns. Deixe
   "livre" só a credencial que você aceitaria que o Claude visse; as outras ficam
   "com aprovação" (`skill/README.md` › "Nível livre").

### Conferir

- `ls ~/.claude/skills` tem `ward` e não tem `cofre`.
- `command -v cofre-lista cofre-run cofre-ssh cofre-abrir` não imprime nada.
- As regras novas aparecem em "Deny" no `/permissions` do Claude Code.
- Num pedido como "liste as credenciais do Ward", o Claude usa `ward lista`.

### Voltar atrás

- `mv ~/ward-migracao/skill-cofre ~/.claude/skills/cofre` e
  `mv ~/ward-migracao/bin/cofre-* ~/.local/bin/`. A skill `cofre` só volta a
  funcionar com o passo 5 e o passo 4 desfeitos (KeePassXC de novo como Serviço
  Secreto). Para tirar a skill `ward`, apague `~/.claude/skills/ward/` e as regras
  que você juntou.

## Passo 8 — Só então desinstalar o KeePassXC

Só faça este passo depois do passo 6 conferido e registrado. É o único passo que não
volta sozinho: a volta é reinstalar e abrir o `.kdbx` guardado.

1. Abra a cópia guardada do `.kdbx` (a do "Antes de começar", não a de `~/cofre/`)
   no KeePassXC, por **Banco de dados › Abrir banco de dados**, com a senha dele, e
   confira que as entradas estão lá. Se o banco usa arquivo-chave, guarde uma cópia
   do arquivo-chave também, num lugar seguro separado da cópia do `.kdbx`: sem ele a
   cópia não abre. Feche o KeePassXC.
2. Desinstale pelo mesmo meio que instalou: `sudo apt remove keepassxc` (ou
   `flatpak uninstall org.keepassxc.KeePassXC`).
3. Guarde a cópia do `.kdbx` (cifrada) até ter certeza de que nada faltou; depois,
   apague o original do disco (`~/cofre/`). A configuração do KeePassXC fica em
   `~/.config/keepassxc/`; apague quando não for mais voltar.
4. Quando não for mais voltar, apague `~/ward-migracao/` e a pasta
   `/run/user/<uid>/migracao` (some sozinha ao desligar).

### Conferir

- `command -v keepassxc` não imprime nada.
- `busctl --user status org.freedesktop.secrets` continua com `Comm=gnome-keyring-d`;
  o Chrome abre com as senhas; `ssh -T git@github.com` funciona; `ward lista` e um
  `ward ssh <entrada> hostname` funcionam.

### Voltar atrás

- `sudo apt install keepassxc` (ou o flatpak), abra a cópia guardada do `.kdbx` (com
  o arquivo-chave, se houver) e, se precisar do KeePassXC como Serviço Secreto de
  novo, desfaça os passos 7, 5 e 4 pelos seus "Voltar atrás" (refazendo a montagem
  a partir dos arquivos em `~/ward-migracao/`).
