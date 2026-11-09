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
- Tenha uma cópia do arquivo `.kdbx` do KeePassXC (`~/cofre/`) num lugar seguro
  (pendrive ou backup cifrado). Ele já é cifrado; não exporte para CSV nem para texto.
- Crie a pasta de backups dos arquivos de configuração que o guia move (nenhum deles
  tem segredo) e a pasta temporária em memória para as chaves (tmpfs, some ao
  desligar):

  ```bash
  install -d -m 700 ~/ward-migracao
  install -d -m 700 "/run/user/$(id -u)/migracao"
  ```

- Se o `wl-copy` existir (`command -v wl-copy`), ele copia um arquivo para a área de
  transferência sem passar pela tela, e `wl-copy --clear` limpa. Sem ele, use o editor
  de texto só para selecionar e copiar, sem salvar. Se o painel do COSMIC guardar
  histórico da área de transferência, limpe o histórico ao fim de cada passo.
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
   campos. Para o valor secreto, copie no KeePassXC (ele limpa a área de transferência
   sozinho) e cole direto no campo do Ward. Se preferir, gere senha nova no gerador
   do Ward e troque no serviço.
3. Entradas `servidor` com chave ficam para o passo 2. Com senha, cadastre já.
4. Escolha o nível de cada entrada (padrão "com aprovação"; "bloqueado" para o que o
   Claude nunca deve usar).

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
  `~/.ssh` não fica nada dos servidores.

### 2a. Chaves pessoais

Para cada chave pessoal no KeePassXC (entrada com a aba **Agente SSH** ligada):

1. Na entrada, aba **Avançado › Anexos**, salve o anexo da chave privada direto em
   `~/.ssh/<nome>` (ex.: `~/.ssh/id_ed25519_github`). Mantenha a chave cifrada com a
   passphrase; não tire a passphrase.
2. Ajuste a permissão e gere a pública ao lado (pede a passphrase no terminal):

   ```bash
   chmod 600 ~/.ssh/id_ed25519_github
   ssh-keygen -y -f ~/.ssh/id_ed25519_github > ~/.ssh/id_ed25519_github.pub
   ```

3. No KeePassXC, desligue nessa entrada "Adicionar ao agente ao abrir/desbloquear".
4. Se o agente perguntar se deve lembrar a passphrase, **não marque** até o passo 5
   terminar: hoje "lembrar" grava no KeePassXC, que vai sair.

### 2b. Chaves de servidor

Para cada servidor cuja chave está no KeePassXC:

1. Salve o anexo da chave privada na pasta em memória:
   `/run/user/<uid>/migracao/<servidor>` (nunca no disco).
2. No Ward, crie (ou edite) a entrada `servidor`: host, porta, usuário,
   autenticação **chave**. Copie o conteúdo do arquivo (`wl-copy <
   "/run/user/$(id -u)/migracao/<servidor>"`) e cole no campo "Chave privada"; a
   passphrase, copie do KeePassXC e cole no campo "Passphrase". Servidor de salto, se
   houver.
3. Apague a cópia e limpe a área de transferência:

   ```bash
   rm -f "/run/user/$(id -u)/migracao/<servidor>"
   wl-copy --clear
   ```

4. Se alguma chave de servidor de trabalho estiver hoje no `~/.ssh` (arquivo ou
   bloco `Host` no `~/.ssh/config`), mova para `~/ward-migracao/ssh-servidores/` só
   depois do passo 3 conferir esse servidor.

Por fim, com todas as chaves movidas: no KeePassXC, **Ferramentas › Configurações ›
Agente SSH**, desligue a integração.

### Conferir

- `ssh-add -D` esvazia o agente; `ssh-add ~/.ssh/id_ed25519_github` carrega a chave do
  arquivo; `ssh-add -l` mostra a impressão digital dela; `ssh -T git@github.com`
  responde com o seu usuário. Faça o mesmo com cada chave pessoal.
- `ls -l ~/.ssh` mostra cada chave privada com `-rw-------`.
- `ls -A "/run/user/$(id -u)/migracao"` está vazio.
- No app do Ward, cada entrada `servidor` com chave mostra "chave" na autenticação
  (o valor fica escondido até "revelar").

### Voltar atrás

- Chave pessoal: `rm ~/.ssh/id_ed25519_github ~/.ssh/id_ed25519_github.pub`, religue
  "Adicionar ao agente" na entrada e a integração do Agente SSH no KeePassXC, trave e
  destrave o KeePassXC; `ssh-add -l` volta a mostrar a chave.
- Chave de servidor: no Ward, **Excluir entrada** (ou volte a autenticação para o
  que era); o arquivo movido do `~/.ssh`, traga de volta de
  `~/ward-migracao/ssh-servidores/`. A chave continua no KeePassXC.

## Passo 3 — Conferir cada servidor com `ward ssh`

Para cada entrada `servidor` (com senha ou chave):

1. Antes da primeira conexão, anote a impressão digital que você já conhece do host:
   `ssh-keygen -l -F <host> -f ~/.ssh/known_hosts` (porta diferente de 22:
   `-F '[<host>]:<porta>'`).
2. Rode `ward ssh <entrada> hostname`. A primeira conexão grava a chave do host no
   `known_hosts` do Ward (`~/.local/share/ward/known_hosts`), não no `~/.ssh`.
3. Servidor com salto e banco com túnel: `ward ssh <entrada-com-salto> hostname` e
   `ward run <entrada-banco-com-tunel> -- psql -c 'select 1'`.

### Conferir

- Cada `ward ssh <entrada> hostname` imprime o nome do servidor e sai 0, sem pedir
  senha nem passphrase.
- `ssh-keygen -l -f ~/.local/share/ward/known_hosts` mostra, para cada host, a mesma
  impressão digital do item 1. Diferente: **pare** e descubra o motivo antes de
  usar a entrada.
- O `~/.ssh/known_hosts` não mudou (`ls -l` antes e depois).

### Voltar atrás

- Nada a desfazer no servidor. Se uma entrada falhar, corrija no app do Ward e
  repita; enquanto isso, a entrada do KeePassXC e o `cofre-ssh` continuam
  funcionando. Uma linha errada do `known_hosts` do Ward se tira com
  `ssh-keygen -R <host> -f ~/.local/share/ward/known_hosts`.

## Passo 4 — Devolver o chaveiro ao gnome-keyring, inclusive "Chrome Safe Storage"

Hoje o KeePassXC é o Serviço Secreto da máquina, e o Chrome guarda nele a chave
"Chrome Safe Storage", que cifra as senhas salvas do Chrome. Se o gnome-keyring
voltar sem essa chave, o Chrome cria uma nova e deixa de abrir as senhas antigas.
Neste passo a chave e as outras entradas criadas por aplicativos são copiadas para o
gnome-keyring, **antes** de desfazer a montagem (passo 5).

1. Feche o Chrome e os aplicativos que usam o chaveiro (VS Code, Slack e outros
   Electron). `pgrep -a chrome` não mostra nada.
2. Copie os bancos de senhas do Chrome (já cifrados) para o backup:

   ```bash
   install -d -m 700 ~/ward-migracao/chrome
   cd ~/.config/google-chrome && find . -maxdepth 2 -name 'Login Data*' \
     -exec cp --parents -p {} ~/ward-migracao/chrome/ \; ; cd -
   ```

   Anote também quantas senhas o Chrome tem (você confere no passo 6): abra antes
   `chrome://password-manager/passwords`, anote o número e feche de novo.
3. No KeePassXC, no grupo exposto ao Serviço Secreto, liste as entradas criadas por
   aplicativos (não as suas credenciais): "Chrome Safe Storage" e, se houver, "Code
   Safe Storage", "Slack Safe Storage", senhas de rede etc. Para cada uma, anote só o
   **título** e os **atributos** (aba **Avançado**, atributos adicionais; no Chrome é
   em geral `application` = `chrome` e `xdg:schema` =
   `chrome_libsecret_os_crypt_password_v2`). Não anote o valor.
4. No KeePassXC, **Ferramentas › Configurações › Integração com o Serviço Secreto**,
   desligue a integração e aplique. Confira que ninguém é dono do nome:

   ```bash
   busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
     org.freedesktop.DBus NameHasOwner s org.freedesktop.secrets
   ```

   Esperado: `b false`.
5. Ligue os segredos do gnome-keyring nesta sessão e confira o dono:

   ```bash
   gnome-keyring-daemon --start --components=secrets
   busctl --user status org.freedesktop.secrets | grep -i comm
   ```

   Esperado: `gnome-keyring-d`. Se não for, pare e siga "Voltar atrás".
6. Para cada entrada do item 3, grave no gnome-keyring com os mesmos atributos. O
   `secret-tool store` pede o valor num prompt (nunca na linha de comando): copie o
   valor no KeePassXC e cole no prompt. Exemplo do Chrome:

   ```bash
   secret-tool store --label='Chrome Safe Storage' \
     application chrome xdg:schema chrome_libsecret_os_crypt_password_v2
   ```

   Se o gnome-keyring pedir para criar o chaveiro padrão, use a mesma senha do seu
   login, para o login destravá-lo sozinho. Depois, `wl-copy --clear`.
7. Mantenha o Chrome fechado e siga para o passo 5.

### Conferir

- `secret-tool search --all application chrome 2>&1 >/dev/null` mostra o rótulo
  "Chrome Safe Storage" e os atributos do item 3, num caminho
  `/org/freedesktop/secrets/collection/…` do gnome-keyring (a parte
  `2>&1 >/dev/null` descarta o valor e deixa só os atributos).
- Só um item do Chrome aparece. Faça o mesmo para cada entrada do item 3.
- `ls ~/ward-migracao/chrome` tem um `Login Data` por perfil do Chrome.

### Voltar atrás

1. Confira que o dono ainda é o gnome-keyring
   (`busctl --user status org.freedesktop.secrets | grep -i comm` dá `gnome-keyring-d`;
   se der `keepassxc`, **não** rode o `clear`, que apagaria a cópia do KeePassXC).
2. Apague só o que você criou: `secret-tool clear application chrome xdg:schema
   chrome_libsecret_os_crypt_password_v2` (e o mesmo para cada entrada do item 3).
3. `systemctl --user restart gnome-keyring-daemon` (ele volta só com `pkcs11`, como
   na montagem), religue a integração com o Serviço Secreto no KeePassXC e confira
   que `busctl --user status org.freedesktop.secrets | grep -i comm` dá `keepassxc`.
4. Abra o Chrome: as senhas continuam lá, porque a chave do KeePassXC não mudou.

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

- `busctl --user status org.freedesktop.secrets | grep -i comm` dá `gnome-keyring-d`,
  sem o KeePassXC aberto e sem rodar nada à mão.
- `secret-tool search --all application chrome 2>&1 >/dev/null` mostra o item do
  passo 4 sem pedir senha (o login destravou o chaveiro).
- `echo "$SSH_AUTH_SOCK"` continua `/run/user/<uid>/gcr/ssh` e `ssh -T
  git@github.com` funciona com a chave do `~/.ssh` (agora pode marcar "lembrar a
  passphrase", se quiser).

### Voltar atrás

- Desfaça na ordem inversa (5d, 5c, 5b, 5a), cada um pelo seu "Voltar atrás", saia e
  entre na sessão e religue a integração com o Serviço Secreto no KeePassXC. Confira
  que `busctl --user status org.freedesktop.secrets | grep -i comm` dá `keepassxc`.

## Passo 6 — Conferir as senhas do Chrome

1. Abra o Chrome e vá em `chrome://password-manager/passwords`.
2. Compare o número de senhas com o que você anotou no passo 4.
3. Abra uma senha salva e peça para mostrar; ela aparece (o Chrome abriu com a chave
   antiga). Entre num site que usa senha salva e confira o preenchimento.

### Conferir

- O número de senhas é o mesmo do passo 4 e o "mostrar" funciona.
- `secret-tool search --all application chrome 2>&1 >/dev/null` continua com um só
  item do Chrome (ele não criou uma chave nova).
- Repita a conferência com os outros aplicativos do passo 4 (VS Code, Slack…).

### Voltar atrás

1. Feche o Chrome.
2. Volte o passo 5 inteiro (seu "Voltar atrás") e o passo 4 (seu "Voltar atrás").
3. Se o Chrome tiver regravado o banco de senhas, devolva a cópia do passo 4 com ele
   fechado: `cp -p ~/ward-migracao/chrome/<perfil>/'Login Data'
   ~/.config/google-chrome/<perfil>/`.
4. Abra o Chrome e confira as senhas com o KeePassXC como Serviço Secreto.

Só avance se as senhas do Chrome estiverem todas lá. Avise na missão: "Chrome
manteve as senhas" (sem nenhum valor), para o registro do V10.4 no `plan.md`.

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

1. Feche o KeePassXC.
2. Desinstale pelo mesmo meio que instalou: `sudo apt remove keepassxc` (ou
   `flatpak uninstall org.keepassxc.KeePassXC`).
3. Guarde o `.kdbx` (cifrado) no backup do "Antes de começar" até ter certeza de que
   nada faltou; depois, apague a cópia do disco (`~/cofre/`). A configuração do
   KeePassXC fica em `~/.config/keepassxc/`; apague quando não for mais voltar.
4. Quando não for mais voltar, apague `~/ward-migracao/` e a pasta
   `/run/user/<uid>/migracao` (some sozinha ao desligar).

### Conferir

- `command -v keepassxc` não imprime nada.
- `busctl --user status org.freedesktop.secrets | grep -i comm` continua
  `gnome-keyring-d`; o Chrome abre com as senhas; `ssh -T git@github.com` funciona;
  `ward lista` e um `ward ssh <entrada> hostname` funcionam.

### Voltar atrás

- `sudo apt install keepassxc` (ou o flatpak), abra o `.kdbx` guardado e, se precisar
  do KeePassXC como Serviço Secreto de novo, desfaça os passos 7, 5 e 4 pelos seus
  "Voltar atrás" (refazendo a montagem a partir dos arquivos em `~/ward-migracao/`).
