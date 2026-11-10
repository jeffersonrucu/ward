# Instalação do Ward (roteiro do usuário)

Roteiro da Fase 52 (D18, D46, D47, D60; V9.5, V9.7, V9.8). Quem roda é você, uma vez,
na sua máquina. A missão nunca roda `sudo` nem instala nada no host.

Duas formas de instalar a mesma árvore (`instalacao/arvore.txt`). Escolha uma:

- **`.deb`** (recomendada): `make deb` e depois `sudo dpkg -i`. O `dpkg` registra os
  arquivos, a dependência de `sshpass` e a remoção.
- **`sudo make install`**: copia direto da árvore do repositório.

Não misture as duas. Se já usou uma, desinstale antes de trocar.

## 0. Antes de começar

1. Confira o kernel. O daemon recusa os papéis app e extensão se algum destes estiver no
   valor ruim (D47):

   ```sh
   cat /proc/sys/kernel/yama/ptrace_scope   # tem de ser 1, 2 ou 3 (não 0)
   cat /proc/sys/fs/suid_dumpable           # tem de ser 0 ou 2 (não 1)
   ```

2. Tire o retrato do SSH (V9.8). Você vai comparar com ele no fim:

   ```sh
   mkdir -p -m 0700 ~/ward-antes
   find ~/.ssh /etc/ssh -printf '%p %U %G %m %y\n' 2>/dev/null | sort > ~/ward-antes/arvore.txt
   sudo find ~/.ssh /etc/ssh -type f -exec sha256sum {} + | sort > ~/ward-antes/hashes.txt
   systemctl --user show-environment | grep -E '^SSH_AUTH_SOCK=' > ~/ward-antes/ambiente.txt
   ssh -G github.com | sort > ~/ward-antes/ssh-G.txt
   ```

3. **Código de confiança.** O `sudo` roda os scripts do repositório (`instalar.sh`,
   `Makefile`) ou os do pacote (`preinst`/`postinst`) como root. Um programa malicioso
   rodando como o seu usuário poderia trocá-los antes do `sudo`. Para diminuir esse
   risco:
   - faça um clone novo, só para a instalação, e confira a revisão:

     ```sh
     git clone ~/projetos/cofre ~/ward-instalar && cd ~/ward-instalar
     git log -1 --format='%H %s'      # a revisão que você espera
     git status --porcelain           # vazio
     ```

   - no caminho do `.deb`, copie o pacote para uma pasta do root **antes** de conferir
     e instalar. Assim nada do seu usuário consegue trocá-lo entre a conferência e o
     `dpkg` (passo 2).

   O risco não some: o build ainda roda como o seu usuário. Está anotado no handoff
   da Fase 52.

4. **Native messaging de outros apps.** A política do Ward desliga os hosts de native
   messaging do seu perfil do Chrome (`NativeMessagingUserLevelHosts: false`, passo 4).
   Anote quais você usa hoje, para copiá-los depois (passo 4a):

   ```sh
   ls -l ~/.config/google-chrome/NativeMessagingHosts/ 2>/dev/null
   ```

   Se aparecer um `br.ward.nm.json` aí, **pare**: o Ward nunca grava no perfil. Esse
   arquivo é o ataque que a política fecha. Apague-o e descubra quem o pôs antes de
   seguir.

5. **Outras políticas do Chrome.** A instalação recusa se outro arquivo de
   `/etc/opt/chrome/policies/managed/` citar `ExtensionSettings`,
   `NativeMessagingUserLevelHosts`, `PasswordManagerEnabled` ou
   `PasswordLeakDetectionEnabled` (ou tiver escape `\u`). Confira antes:

   ```sh
   ls -l /etc/opt/chrome/policies/managed/ 2>/dev/null
   grep -l -E 'ExtensionSettings|NativeMessagingUserLevelHosts|PasswordManagerEnabled|PasswordLeakDetectionEnabled|\\u' /etc/opt/chrome/policies/managed/* 2>/dev/null
   ```

   Se o `grep` listar algum arquivo, veja "Outras políticas do Chrome" no passo 4c.

## 1. Build (como você, sem sudo)

```sh
make release          # ward, wardd, ward-nm e o app (precisa de libwebkit2gtk-4.1-dev)
make extensao         # extensao/dist/
make extensao-crx     # target/extensao/ward.crx, assinado com .dev/extensao/chave.pem
```

O `make extensao-crx` usa o Chrome (`--pack-extension`) com perfil e HOME temporários
em `target/`, e não mexe no seu perfil. Ele recusa o pacote se o id não for o
`ID_EXTENSAO` do `ward-nm`. No fim imprime a linha `EXTENSAO_ID=… EXTENSAO_CRX=…` do
próximo passo.

A chave privada tem de estar em `.dev/extensao/chave.pem` (0600) no clone de
instalação. Copie do checkout principal com `install -m 0600`. Sem ela, o id muda e
nada casa.

## 2a. Instalar pelo `.deb`

```sh
make deb EXTENSAO_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSAO_CRX=$PWD/target/extensao/ward.crx
```

O comando gera `target/deb/ward_<versão>_<arq>.deb` e imprime o sha256 dele. O
`make deb` passa pelas mesmas conferências do `make install`: recusa artefato
`modo-teste` (V9.2), binário que não é ELF, link e pacote da extensão com outro id.
Depois confere o próprio `.deb`: a lista do `dpkg-deb -c` tem de ser igual à do
staging, tudo root/root, nenhum bit setgid no pacote e Depends com `sshpass` (V9.9).

```sh
sudo install -d -m 0700 -o root -g root /root/ward
sudo install -m 0644 -o root -g root target/deb/ward_*.deb /root/ward/
sudo sha256sum /root/ward/ward_*.deb                           # o mesmo que o make deb imprimiu
sudo dpkg-deb -c /root/ward/ward_*.deb                         # 60 caminhos, root/root, sem 's'
sudo dpkg-deb -f /root/ward/ward_*.deb Depends                 # sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0
sudo dpkg-deb -I /root/ward/ward_*.deb preinst | less          # leia os scripts antes de rodar
sudo apt install /root/ward/ward_*.deb                         # ou: sudo dpkg -i (sem resolver dependências)
```

Durante a instalação:

- o `preinst` cria `ward-app` e `ward-nm` como grupos de sistema (D60). Se um deles já
  existir fora das regras (com membro, senha, gid ≥ 1000, no `/etc/subgid`, dado no
  login pelo `/etc/security/group.conf`, citado num `sudoers`, ou com `subid:`/`sudoers:`
  do `/etc/nsswitch.conf` fora de `files`), a instalação para antes de qualquer arquivo
  e diz qual regra falhou. Os grupos que ele cria ficam anotados em
  `/usr/local/lib/ward/grupos-criados`;
- o `preinst` também para se outro arquivo de `/etc/opt/chrome/policies/managed/` citar
  um nome da política do Ward (`ExtensionSettings`, `NativeMessagingUserLevelHosts`,
  `PasswordManagerEnabled` ou `PasswordLeakDetectionEnabled`; passo 0.5);
- os binários setgid vêm no pacote como 0755. O 2755 com o grupo certo entra pelo
  `dpkg-statoverride` (confira com `dpkg-statoverride --list | grep ward`);
- o `postinst` liga `wardd@<seu uid>.socket`. O uid vem do `SUDO_UID` (`sudo apt
  install`/`sudo dpkg -i`). Instalado pela loja de apps (PackageKit), sem `SUDO_UID`, ele
  liga o socket de cada usuário humano do `/etc/passwd` (uid entre `UID_MIN` e `UID_MAX`
  do `/etc/login.defs`, com shell de login). Cada `wardd` roda com o uid do dono e só sobe
  na primeira conexão. `WARD_UID=<uid>` escolhe um só.

## 2b. Ou instalar por `sudo make install`

```sh
sudo make install EXTENSAO_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSAO_CRX=$PWD/target/extensao/ward.crx
```

## 3. Conferir o systemd (V9.7)

Depois de instalar e também depois de reiniciar a máquina e entrar de novo:

```sh
systemctl is-enabled wardd@$(id -u).socket     # enabled
systemctl is-active  wardd@$(id -u).socket     # active
stat -c '%U:%G %a' /run/ward /run/ward/$(id -u)   # root:root 755 nas duas
stat -c '%U %a %F' /run/ward/$(id -u)/wardd.sock  # <você> 600 socket
/usr/local/bin/ward destravar                   # primeira conexão sobe o wardd, sem sudo
ps -o user=,pid=,cmd= -C wardd                  # roda como você
stat -c '%U %a' /run/user/$(id -u)/ward         # <você> 700 (temporários)
systemd-inhibit --list | grep -i ward           # inibidor da suspensão (trava ao suspender)
```

O uso diário (`ward lista`, `ward run`, `ward ssh`, app) não pede `sudo`.

**Senha mestra só pelo caminho completo.** Destrave pelo app ou, no terminal, sempre com
`/usr/local/bin/ward destravar`, nunca o `ward` que o `PATH` acha. Um programa malicioso
rodando como você pode pôr um `ward` falso antes no `PATH` (em `~/.local/bin`, por
exemplo): ele mostra o mesmo pedido de senha e guarda o que você digitar. O caminho
completo pula o `PATH`. Ele não protege de um `~/.bashrc` ou `~/.zshrc` adulterado (uma
função do shell, ou um alias do zsh, pode ter o nome `/usr/local/bin/ward`): esse caso
fica fora do que o Ward cobre (escopo › "Fora do modelo de ameaça", RA8). Os outros
comandos (`ward lista`, `ward run`…) não recebem a senha mestra; o PIN é digitado só na
janela do app.

### V9.4 (opcional): o `LD_PRELOAD` do gerente do usuário não chega ao `wardd`

```sh
systemctl --user set-environment LD_PRELOAD=/tmp/nao-existe.so
pkill -KILL -x wardd; ward lista >/dev/null     # mata e sobe outro pela conexão
sudo cat /proc/$(pgrep -x wardd)/environ | tr '\0' '\n' | grep -c '^LD_'   # 0
grep -E '^(Max core)' /proc/$(pgrep -x wardd)/limits                       # 0 0
systemctl --user unset-environment LD_PRELOAD
```

### 3a. Core dump

O `ward`, o `wardd`, o `ward-app` e o `ward-nm` são não-dumpable: o kernel não gera core
deles. Os auxiliares do WebKitGTK que o app abre (`WebKitWebProcess`,
`WebKitNetworkProcess`, `WebKitGPUProcess`) são programas comuns, com a memória do que o
app mostra. Um `kill -SEGV` de outro programa seu neles faria o apport gravar essa
memória em `/var/crash`, legível por você, mesmo com o limite de core em 0. Por isso a
instalação:

- põe o Ward e os auxiliares do WebKitGTK na lista do apport,
  `/etc/apport/report-ignore/ward` (e no nome antigo, `/etc/apport/blacklist.d/ward`).
  O apport deixa de gravar relatório de crash desses programas, inclusive quando outro
  app que usa o WebKitGTK 4.1 cai;
- desliga o armazenamento do systemd-coredump, se ele for instalado algum dia:
  `/etc/systemd/coredump.conf.d/ward.conf` com `Storage=none` e `ProcessSizeMax=0`. Ele
  não tem lista por programa, então isso vale para o **sistema inteiro**: nenhum core é
  guardado em `/var/lib/systemd/coredump` (o journal ainda registra programa e sinal).
  Se você precisa de core para depurar outro programa, apague esse arquivo enquanto
  depura e reinstale o Ward depois.

Conferir:

```sh
cat /proc/sys/kernel/core_pattern
# |/usr/share/apport/apport ...      -> apport: a lista abaixo vale
# |/usr/lib/systemd/systemd-coredump -> systemd-coredump: o drop-in abaixo vale
# core (ou outro arquivo)            -> o limite 0 que o app passa aos auxiliares vale
# outro programa                     -> a instalação avisou: o Ward não cobre esse caso
grep -vx '#.*' /etc/apport/report-ignore/ward
# /usr/local/bin/ward ... e /usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/WebKit*Process
dpkg -L libwebkit2gtk-4.1-0 | grep -E 'WebKit[A-Za-z]*Process$' |
    grep -vxF -f /etc/apport/report-ignore/ward          # nada: todo auxiliar na lista
systemd-analyze cat-config systemd/coredump.conf | grep -E 'ward.conf|Storage|ProcessSizeMax'
ls /var/crash | grep -i -E 'webkit|ward'                # nada
```

Se um crash do app acontecer, `/var/log/apport.log` (com `sudo`) mostra "executable
version is in denylist" no lugar de "wrote report". A atualização do WebKitGTK que
trouxer um auxiliar novo faz a reinstalação do Ward recusar com o nome dele, até a lista
do Ward (`instalacao/despejo.sh`) ser atualizada.

## 4. Conferir o Chrome (V9.5)

Feche e abra o Chrome.

1. Em `chrome://policy`, `ExtensionSettings` aparece com o id
   `nhkehjnmlgkebhmelajfpmpblepaichf`, `force_installed`, sem erro, e
   `NativeMessagingUserLevelHosts`, `PasswordManagerEnabled` e
   `PasswordLeakDetectionEnabled` aparecem como `false`, também sem erro nem conflito.
2. Em `chrome://extensions`, a extensão Ward aparece como **instalada pela sua
   organização**. Não há botão de remover e a chave de ativar fica travada.
3. Ainda em `chrome://extensions`, ligue o modo desenvolvedor e tente "Carregar sem
   compactação" com `extensao/dist/`, que tem a mesma `key` e portanto o mesmo id. O
   Chrome tem de recusar.
4. O ícone fica fixado na barra. Num site de teste, o popup lista as sugestões.

Se a extensão **não** aparecer: a política aponta para `update.xml` e `ward.crx` por
`file://` em `/usr/local/share/ward/extensao/`, e não está confirmado que o Chrome do
Linux aceite `file://` em force-install (handoff da Fase 51). Não improvise. Anote o
que `chrome://policy` e `chrome://extensions` mostram e leve para a missão. A
alternativa planejada é o `external_crx` em `/opt/google/chrome/extensions/<id>.json`,
mais a política só para impedir desativar.

## 4a. Outros apps com native messaging (1Password, Claude in Chrome…)

**O que muda.** A política do Ward liga `NativeMessagingUserLevelHosts: false`. Com ela,
o Chrome só abre hosts de native messaging cujo manifesto esteja em
`/etc/opt/chrome/native-messaging-hosts/` (pasta do root). Os manifestos do seu perfil,
em `~/.config/google-chrome/NativeMessagingHosts/`, deixam de valer.

**Por que.** Sem isso, qualquer programa rodando como o seu usuário (um pacote npm, um
script) pode gravar um `br.ward.nm.json` no seu perfil. Esse manifesto vence o do
sistema, e o script dele recebe a conexão da extensão do Ward no lugar do `ward-nm`. A
auditoria do M9 fez isso e tirou senhas e códigos TOTP de 4 entradas `login`. Com a
política, esse arquivo é ignorado. O teste é o `make teste-politica-chrome`.

**O efeito colateral.** Apps que instalaram o manifesto só no seu perfil param de
falar com a extensão deles no Chrome. Exemplos: o 1Password (destravar pelo app do
desktop) e o Claude in Chrome. Alguns podem voltar a funcionar com o manifesto copiado
para a pasta do sistema; outros **não podem ser copiados**. **Você** executa os
comandos abaixo, um app por vez.

**Quando não copiar.** O manifesto copiado continua apontando (`path`) para o mesmo
programa de antes, e o Chrome roda esse programa como filho dele. Se o programa, ou
qualquer pasta acima dele, é gravável pelo seu usuário (está no seu home, por
exemplo), um processo do seu usuário troca o programa por outro e passa a rodar como
filho do Chrome. Isso reabre o ataque que a política fecha: um script nessa posição
fala com o `ward-nm` como se fosse o Chrome e tira senhas do Ward (2ª auditoria do M9).
Por isso, só copie quando o `path` e **cada pasta acima dele**, até a `/`, são do root
e sem escrita para grupo e outros, sem link no caminho, e o programa não é um script
(`#!`), porque o interpretador e o que ele carrega podem vir do seu home. O passo 1
confere isso e diz "NÃO COPIE" quando falha. O Claude in Chrome, com o host no seu
home, cai nesse caso: ele fica sem a ponte com o Chrome enquanto o Ward estiver
instalado. Mesmo copiando, o Ward não resolve tudo: um processo do seu usuário que
execute o Chrome instalado ainda consegue se passar por ele (RA10, risco aceito, em
`docs/escopo.md` › "Fora do modelo de ameaça").

1. Copie o manifesto para uma pasta do root e confira a cópia. Assim nada do seu
   usuário consegue trocá-lo entre a conferência e a instalação. Troque `<nome>` pelo
   nome do arquivo, sem o `.json` (por exemplo, `com.1password.1password`). Rode tudo
   no mesmo terminal, porque o passo 2 usa a função `conferir_host` do bloco seguinte:

   ```sh
   nome='<nome>'
   [ "$nome" != br.ward.nm ] || echo 'PARE: br.ward.nm no perfil é o ataque; apague-o'
   sudo install -d -m 0700 -o root -g root /root/ward-nm-hosts
   # O `cat` roda como você (um link para arquivo do root não é lido); só o `tee` é root.
   cat ~/.config/google-chrome/NativeMessagingHosts/"$nome".json | sudo tee /root/ward-nm-hosts/"$nome".json >/dev/null
   sudo chmod 0600 /root/ward-nm-hosts/"$nome".json
   sudo python3 -I -m json.tool /root/ward-nm-hosts/"$nome".json
   ```

Depois, no mesmo terminal, defina e rode a `conferir_host`. Este bloco fica fora da
lista e sem recuo de propósito: o `PY` que fecha o heredoc só fecha se estiver no
começo da linha, e o Python não aceita recuo nas linhas de fora. Copie o bloco inteiro
como está, da página ou do arquivo cru; se o seu editor ou terminal acrescentar
espaços no começo das linhas, a função não funciona (o shell fica esperando o `PY`).

```sh
# Confere o `name` e o `path` da cópia: o programa e cada pasta acima dele são do
# root, sem escrita para grupo e outros, sem link, e o programa não é script.
conferir_host() {
    sudo python3 -I - /root/ward-nm-hosts/"$nome".json "$nome" <<'PY'
import json, os, stat, sys
arq, nome = sys.argv[1], sys.argv[2]
def nao(motivo):
    sys.exit("NÃO COPIE: " + motivo)
m = json.load(open(arq))
if m.get("name") != nome or nome == "br.ward.nm":
    nao("o name não é %r (ou é br.ward.nm)" % nome)
p = m.get("path")
if not isinstance(p, str) or not p.startswith("/") or os.path.normpath(p) != p:
    nao("o path não é um caminho absoluto simples: %r" % (p,))
atual = p
while True:
    try:
        st = os.lstat(atual)
    except OSError as e:
        nao("%s: %s" % (atual, e.strerror))
    if stat.S_ISLNK(st.st_mode):
        nao("%s é um link" % atual)
    if st.st_uid != 0:
        nao("%s não é do root (dono uid %d)" % (atual, st.st_uid))
    if st.st_mode & 0o022:
        nao("%s tem escrita para grupo ou outros (%o)" % (atual, stat.S_IMODE(st.st_mode)))
    if atual == "/":
        break
    atual = os.path.dirname(atual)
st = os.lstat(p)
if not stat.S_ISREG(st.st_mode):
    nao("%s não é um arquivo comum" % p)
with open(p, "rb") as f:
    if f.read(2) == b"#!":
        nao("%s é um script (#!): o interpretador e o que ele carrega podem vir do seu home" % p)
print("ok: %s e cada pasta acima são do root e sem escrita para grupo e outros" % p)
PY
}
conferir_host
```

Confira na saída:
- a última linha começa com `ok:`. Se começar com `NÃO COPIE`, pare aqui, apague a
  cópia (`sudo rm /root/ward-nm-hosts/"$nome".json`) e deixe esse app sem a ponte
  com o Chrome;
- `"path"` é um caminho absoluto do app que você conhece (por exemplo,
  `/opt/1Password/…`);
- `"allowed_origins"` só tem a extensão desse app.

Um `path` que começa por `/bin/` (ou `/sbin/`) dá `NÃO COPIE: /bin é um link` (ou
`/sbin é um link`): no Ubuntu, `/bin` e `/sbin` são links para `/usr/bin` e
`/usr/sbin`, e a `conferir_host` recusa link em qualquer ponto do caminho. O mesmo
programa por `/usr/bin/...` (ou `/usr/sbin/...`) passa. Se for esse o único motivo,
troque o `path` da cópia do root para a forma `/usr/...`, confira a cópia de novo e
rode a `conferir_host` outra vez:

```sh
sudo python3 -I -c 'import json, sys; a = sys.argv[1]; m = json.load(open(a)); p = m["path"]; assert p.startswith(("/bin/", "/sbin/")), p; m["path"] = "/usr" + p; json.dump(m, open(a, "w"), indent=2)' /root/ward-nm-hosts/"$nome".json
sudo python3 -I -m json.tool /root/ward-nm-hosts/"$nome".json
conferir_host
```

Se algo não bater, pare e apague a cópia (`sudo rm /root/ward-nm-hosts/"$nome".json`).

2. Instale a cópia conferida na pasta do sistema. O comando roda a conferência do passo
   1 de novo e só instala se ela passar; também não sobrescreve um arquivo que já
   esteja lá:

   ```sh
   conferir_host &&
     [ ! -e /etc/opt/chrome/native-messaging-hosts/"$nome".json ] &&
     sudo install -m 0644 -o root -g root /root/ward-nm-hosts/"$nome".json /etc/opt/chrome/native-messaging-hosts/ &&
     echo instalado
   ```

3. **Conferir.**

   ```sh
   ls -l /etc/opt/chrome/native-messaging-hosts/   # root root 644 em cada um; br.ward.nm.json continua lá
   sudo cmp /root/ward-nm-hosts/"$nome".json /etc/opt/chrome/native-messaging-hosts/"$nome".json && echo igual
   ```

   Feche e abra o Chrome e use o app (por exemplo, destrave o 1Password pela extensão
   ou abra o Claude in Chrome). Em `chrome://policy`,
   `NativeMessagingUserLevelHosts` continua `false`.

4. **Voltar atrás** (só deste app):

   ```sh
   sudo rm /etc/opt/chrome/native-messaging-hosts/"$nome".json
   sudo rm /root/ward-nm-hosts/"$nome".json
   ```

   Nunca apague nem edite o `br.ward.nm.json` da mesma pasta, nem o `ward.json` de
   `/etc/opt/chrome/policies/managed/`. Editar a política reabre o ataque, e a próxima
   instalação a sobrescreve de qualquer forma. Para tirar a política, desinstale o Ward
   (passo 6).

Quando o app se atualizar e regravar o manifesto no seu perfil (com outro `path`, por
exemplo), refaça os passos 1 a 3, apagando antes a cópia antiga do sistema. A cópia é
uma foto de quando você a fez. Os manifestos que você copiou não são do Ward: o
`dpkg -r`/`make uninstall` não os apaga.

## 4b. O gerenciador de senhas do Chrome fica desligado

**O que muda.** A política do Ward liga `PasswordManagerEnabled: false` e
`PasswordLeakDetectionEnabled: false` para o Chrome inteiro, em todos os perfis. O
Chrome deixa de oferecer "Salvar senha?", não preenche mais senhas que ele mesmo tenha
guardado e não confere senhas contra vazamentos. Em `chrome://settings/passwords` as
opções aparecem travadas pela organização. As senhas que você já tinha guardado no
Chrome continuam lá (`chrome://password-manager`) e podem ser exportadas, mas o
Chrome não as usa enquanto a política estiver ligada.

**Por que.** Quando a extensão do Ward preenche um login, o Chrome vê o formulário
enviado e oferece guardar a senha no seu perfil (`~/.config/google-chrome/`), fora do
cofre, onde qualquer processo do seu usuário lê. A checagem de vazamento manda um hash
da senha para o Google. As duas coisas tiram a senha do lugar onde ela deveria ficar
(auditoria de segurança do M6, Fase 44c).

**Se você usa o gerenciador do Chrome para outras senhas:** passe-as para o Ward ou
para outro gerenciador antes de instalar. Não há como desligar só para os sites do
Ward. Voltar atrás é desinstalar o Ward (`sudo make uninstall` ou `sudo dpkg -r ward`),
que tira a política.

## 4c. Outras políticas do Chrome

O Chrome lê todo arquivo de `/etc/opt/chrome/policies/managed/` e não junta o mesmo
nome vindo de dois arquivos: um deles vence. Por isso a instalação (pelo `.deb` e pelo
`make install`) recusa se outro arquivo dessa pasta citar `ExtensionSettings`,
`NativeMessagingUserLevelHosts`, `PasswordManagerEnabled` ou
`PasswordLeakDetectionEnabled`, ou tiver um escape `\u`, que poderia esconder esses
nomes. A mensagem diz qual arquivo é, e nada é instalado.

O que fazer:

1. Veja o arquivo e de onde ele veio (`dpkg -S <arquivo>` diz se é de um pacote):

   ```sh
   sudo cat /etc/opt/chrome/policies/managed/<arquivo>
   dpkg -S /etc/opt/chrome/policies/managed/<arquivo>
   ```

2. Se você não precisa dele, tire-o da pasta, sem apagar:

   ```sh
   sudo install -d -m 0700 -o root -g root /root/ward-politicas
   sudo mv /etc/opt/chrome/policies/managed/<arquivo> /root/ward-politicas/
   ```

   Se ele também define outras políticas que você quer manter, crie um arquivo novo só
   com elas, sem os nomes acima, e rode a instalação de novo.

3. **Voltar atrás:** `sudo mv /root/ward-politicas/<arquivo> /etc/opt/chrome/policies/managed/`.
   Com o arquivo de volta, a próxima instalação do Ward recusa de novo.

Se você precisa de `ExtensionSettings` para outra extensão, o Ward ainda não consegue
conviver com isso: leve o caso para a missão.

## 5. Conferir o SSH (V9.8)

```sh
find ~/.ssh /etc/ssh -printf '%p %U %G %m %y\n' 2>/dev/null | sort | diff ~/ward-antes/arvore.txt -
sudo find ~/.ssh /etc/ssh -type f -exec sha256sum {} + | sort | diff ~/ward-antes/hashes.txt -
systemctl --user show-environment | grep -E '^SSH_AUTH_SOCK=' | diff ~/ward-antes/ambiente.txt -
ssh -G github.com | sort | diff ~/ward-antes/ssh-G.txt -
git push                                    # num repositório seu, com a chave pessoal
```

Os quatro `diff` têm de sair vazios, e o `git push` tem de funcionar como antes. O Ward
não usa `SSH_AUTH_SOCK`, `environment.d` nem `ssh_config` (D18).

## 6. Desinstalar

```sh
sudo dpkg -r ward          # ou sudo dpkg -P ward; ou sudo make uninstall
```

A remoção para e desliga todo `wardd@*`, apaga os arquivos, os overrides e `/run/ward`,
inclusive a lista do apport e o drop-in do systemd-coredump do passo 3a (as pastas
`/etc/apport/…` e `/etc/systemd/coredump.conf.d/` ficam).
Dos grupos `ward-app` e `ward-nm`, apaga só os que a instalação criou (anotados em
`/usr/local/lib/ward/grupos-criados`, com o gid). Um grupo que já existia antes fica, e
a remoção avisa. O cofre e os backups em `~/.local/share/ward/` ficam, e também os
manifestos de outros apps que você copiou no passo 4a.

## 7. Backup externo: como o Ward reconhece o pendrive

O pendrive do backup externo é reconhecido **só pelo UUID do sistema de arquivos**
(o que aparece em "Configurações › Backup externo" e no `lsblk -f`). O UUID não é
segredo: quem já viu o seu pendrive pode formatar outro com o mesmo UUID
(`mkfs.vfat -i`, `tune2fs -U`) e, se esse outro estiver conectado e montado no lugar
do seu, recebe as cópias. As cópias são o cofre cifrado com a senha mestra de cada
versão, então ninguém abre nada sem ela, mas é uma cópia a mais fora da sua mão.
Por isso:

- só conecte o pendrive escolhido na sua máquina, com o cofre destravado;
- se aparecerem dois pendrives com o mesmo UUID ao mesmo tempo, o Ward não copia
  para nenhum (o log do `wardd` diz "mais de um pendrive com o mesmo UUID");
- se perder o pendrive, troque a senha mestra (as cópias antigas continuam com a
  senha antiga) e escolha outro pendrive em "Trocar".
