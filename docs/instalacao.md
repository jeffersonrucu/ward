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
sudo dpkg-deb -c /root/ward/ward_*.deb                         # 28 caminhos, root/root, sem 's'
sudo dpkg-deb -f /root/ward/ward_*.deb Depends                 # sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0
sudo dpkg-deb -I /root/ward/ward_*.deb preinst | less          # leia os scripts antes de rodar
sudo apt install /root/ward/ward_*.deb                         # ou: sudo dpkg -i (sem resolver dependências)
```

Durante a instalação:

- o `preinst` cria `ward-app` e `ward-nm` como grupos de sistema (D60). Se um deles já
  existir fora das regras (com membro, senha, gid ≥ 1000, no `/etc/subgid` etc.), a
  instalação para antes de qualquer arquivo e diz qual regra falhou;
- os binários setgid vêm no pacote como 0755. O 2755 com o grupo certo entra pelo
  `dpkg-statoverride` (confira com `dpkg-statoverride --list | grep ward`);
- o `postinst` liga `wardd@<seu uid>.socket`. O uid vem do `SUDO_UID`. Se ele faltar,
  o `postinst` imprime o comando, que é `sudo systemctl enable --now wardd@$(id -u).socket`.

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
ward destravar                                  # primeira conexão sobe o wardd, sem sudo
ps -o user=,pid=,cmd= -C wardd                  # roda como você
stat -c '%U %a' /run/user/$(id -u)/ward         # <você> 700 (temporários)
systemd-inhibit --list | grep -i ward           # inibidor da suspensão (trava ao suspender)
```

O uso diário (`ward lista`, `ward run`, `ward ssh`, app) não pede `sudo`.

### V9.4 (opcional): o `LD_PRELOAD` do gerente do usuário não chega ao `wardd`

```sh
systemctl --user set-environment LD_PRELOAD=/tmp/nao-existe.so
pkill -KILL -x wardd; ward lista >/dev/null     # mata e sobe outro pela conexão
sudo cat /proc/$(pgrep -x wardd)/environ | tr '\0' '\n' | grep -c '^LD_'   # 0
grep -E '^(Max core)' /proc/$(pgrep -x wardd)/limits                       # 0 0
systemctl --user unset-environment LD_PRELOAD
```

## 4. Conferir o Chrome (V9.5)

Feche e abra o Chrome.

1. Em `chrome://policy`, `ExtensionSettings` aparece com o id
   `nhkehjnmlgkebhmelajfpmpblepaichf`, `force_installed`, sem erro.
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

Também confira se não há outro arquivo em `/etc/opt/chrome/policies/managed/` com
`ExtensionSettings`. O Chrome não junta dois `ExtensionSettings` de arquivos
diferentes.

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

A remoção para e desliga todo `wardd@*`, apaga os arquivos, os overrides, `/run/ward`
e os grupos. O cofre e os backups em `~/.local/share/ward/` ficam. Os grupos
`ward-app` e `ward-nm` saem mesmo se já existiam antes da instalação.

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
