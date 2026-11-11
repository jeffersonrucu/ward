# Roteiro do usuário — atualizar para a 0.2.0

Roteiro das Fases 66, 66a e 66b (missão 2; V6.12, V6.13). Quem roda é você, na sua máquina.
A missão não roda `sudo`, não instala nada, não mexe no `~/.claude` e não toca no cofre
real.

A 0.2.0 traduziu o código para o inglês. As telas e os textos continuam os mesmos. O
que muda para você (subcomandos, variáveis do `ward run`, `ward list --json`, caminhos
instalados e nomes reservados) está em `docs/instalacao.md` › "Atualizar da 0.1.x para
a 0.2.0". Leia essa seção antes. Este roteiro é o caminho curto, com os artefatos já
prontos.

A 0.2.0 inclui a correção da 0.1.5 (Fase 66a): abrir o Ward pelo menu de aplicativos
com o app já na bandeja mostra a janela. Onde este roteiro diz 0.1.x, vale para a 0.1.4
e a 0.1.5 (a máquina tem a 0.1.5; `dpkg-query -W ward`).

## Artefatos

Ficam em `target/release-0.2.0/` do checkout principal (fora do git), refeitos pela
última vez na Fase 66b (correções dos validadores do M6 e metainfo com a 0.1.5 e a
0.2.0). Os sha256 estão na tabela abaixo, no handoff da Fase 66b
(`docs/mission/handoffs/phase-66b.md`) e em `target/release-0.2.0/SHA256SUMS`. Os das
Fases 66 e 66a não valem mais.

| Artefato | sha256 |
| --- | --- |
| `ward_0.2.0_amd64.deb` | `1b7adb945c29a83baeee3a86f6267ae49be825eea713cfb9bd90903781161667` |
| `ward.crx` | `ad931ba0739de4b72635f37a20dc0aefe71be818f6e94e20f807327c17b8106b` |
| `bin/ward` | `994f8d29eaabe5e1eae5b6bdabd925eae9f2736621759c735cb6e4bc886bd70b` |
| `bin/wardd` | `0f85fe58941c7f89e007405f8e2164bc1e3104373ed4c30d4fa1172f27e419e5` |
| `bin/ward-nm` | `c073f81c88cae15bff34b3491772e9f5b1048543822b2b2c78ebef324cd76e97` |
| `bin/ward-app` | `ceaff36dc21f1b5bbcf80b21d737eeea3cbd51042deef085232131cd5c8445df` |
| `ward-install-0.2.0.tar` | `a64fbc686db4ec4f0379d053136c628be0b15e4ad7cf3cb617abdabf4a227209` |

| Arquivo | O que é |
| --- | --- |
| `ward_0.2.0_amd64.deb` | pacote para `sudo apt install`/`sudo dpkg -i` |
| `ward.crx` | extensão do Chrome 0.2.0, id `nhkehjnmlgkebhmelajfpmpblepaichf` (o de sempre) |
| `bin/ward`, `bin/wardd`, `bin/ward-nm`, `bin/ward-app` | binários de release, para o caminho `sudo make install` |
| `ward-install-0.2.0.tar` | o instalador do `make install` (`Makefile`, `install/` com o `tree.txt`, ícones), para rodar de uma pasta do root (3b) |

O `ward-install-0.2.0.tar` é reprodutível: `git archive --format=tar
a01b39e1d6435660000149418d1e664f17730996 Makefile install docs/design/logo` dá o mesmo
sha256.

Use **o mesmo meio** com que instalou a 0.1.x (`.deb` ou `make install`). Não misture.
Esta máquina tem a 0.1.5 pelo `.deb` (`dpkg-query -W ward`): o seu caminho é o 3a.

## 1. Copie a pasta do cofre (antes de tudo)

Na primeira vez que a 0.2.0 destrava, o cofre é regravado no formato novo, e a 0.1.x
não o abre mais. A cópia é o seu caminho de volta. Pare o daemon (o cofre trava) e
copie a pasta inteira, com o `estado` e os backups:

```sh
sudo systemctl stop "wardd@$(id -u).socket" "wardd@$(id -u).service"
install -d -m 0700 ~/ward-copia-0.1.x
cp -a ~/.local/share/ward/. ~/ward-copia-0.1.x/
ls -l ~/ward-copia-0.1.x/          # cofre.ward, estado e backups/
```

A cópia é cifrada pela senha mestra, como o cofre. Não a ponha em `/tmp` nem em nuvem.

Antes de seguir, procure nos campos extras e anexos das suas entradas os nomes
reservados da 0.2.0 (`ENTRY`, `GROUP`, `ENVIRONMENT`, `FILE`, `FILE_…`; `PROTOCOL` em
`ftp-sftp`; anexo `key` em `servidor`/`ftp-sftp`). Detalhe em `docs/instalacao.md`.

## 2. Confira os artefatos

```sh
cd ~/projetos/cofre/target/release-0.2.0
sha256sum -c SHA256SUMS                 # todos "OK", e os mesmos da tabela acima (Fase 66b)
dpkg-deb -f ward_0.2.0_amd64.deb Package Version Depends
# Package: ward / Version: 0.2.0 / Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0
./bin/ward --version                    # ward 0.2.0
```

## 3. Instale por cima da 0.1.x

Sem desinstalar antes. A instalação para todo `wardd@*` antes de trocar os binários e
religa os sockets no fim.

### 3a. Pelo `.deb` (se a 0.1.x veio pelo `.deb`)

Copie o pacote para uma pasta do root **antes** de conferir e instalar, para nada do
seu usuário trocá-lo no meio:

```sh
sudo install -d -m 0700 -o root -g root /root/ward
sudo install -m 0644 -o root -g root ~/projetos/cofre/target/release-0.2.0/ward_0.2.0_amd64.deb /root/ward/
sudo sha256sum /root/ward/ward_0.2.0_amd64.deb      # o mesmo do SHA256SUMS
sudo dpkg-deb -c /root/ward/ward_0.2.0_amd64.deb    # 61 linhas (o ./ e 60 caminhos), root/root, sem 's'
sudo dpkg-deb -I /root/ward/ward_0.2.0_amd64.deb preinst | less   # leia antes de rodar
sudo dpkg -i /root/ward/ward_0.2.0_amd64.deb
```

### 3b. Ou por `sudo make install` (só se a 0.1.x veio pelo `make install`)

**Nunca rode `sudo make install` de dentro do checkout** (`~/projetos/cofre`): ele é do
seu usuário, e qualquer programa seu pode trocar o `Makefile` ou o `install/install.sh`
entre a sua conferência e o `sudo`; o root roda o que estiver lá (auditoria de segurança
do M6, Fase 66b). Como no `.deb`, copie para uma pasta do root o instalador
(`ward-install-0.2.0.tar`), os binários e o pacote da extensão, confira lá e rode de lá:

```sh
R=~/projetos/cofre/target/release-0.2.0
sudo install -d -m 0700 -o root -g root /root/ward-src /root/ward-src/bin
sudo install -m 0644 -o root -g root $R/ward-install-0.2.0.tar $R/ward.crx /root/ward-src/
sudo install -m 0755 -o root -g root $R/bin/ward $R/bin/wardd $R/bin/ward-nm $R/bin/ward-app /root/ward-src/bin/
sudo sha256sum /root/ward-src/ward-install-0.2.0.tar /root/ward-src/ward.crx /root/ward-src/bin/*
#   os mesmos da tabela "Artefatos" acima
sudo tar -x --no-same-owner -f /root/ward-src/ward-install-0.2.0.tar -C /root/ward-src
sudo less /root/ward-src/install/install.sh        # leia antes de rodar
sudo make -C /root/ward-src install EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf \
    EXTENSION_CRX=/root/ward-src/ward.crx \
    BIN_WARD=/root/ward-src/bin/ward BIN_WARDD=/root/ward-src/bin/wardd \
    BIN_NM=/root/ward-src/bin/ward-nm BIN_APP=/root/ward-src/bin/ward-app
```

Guarde `/root/ward-src`: desinstalar é `sudo make -C /root/ward-src uninstall`, nunca do
checkout.

### 3c. Conferir a instalação

```sh
dpkg-query -W ward                                   # ward 0.2.0 (só no caminho do .deb)
/usr/local/bin/ward --version                        # ward 0.2.0
ls /usr/local/share/ward/                            # extension/, sem extensao/
ls /usr/local/lib/ward/                              # created-groups, sem grupos-criados
systemctl is-active "wardd@$(id -u).socket"          # active
```

## 4. Primeiro destrave = migração

Destrave pelo app (menu de aplicativos › Ward) ou, num terminal seu (fora do Claude
Code), **sempre pelo caminho completo**:

```sh
/usr/local/bin/ward unlock
```

É aqui que o cofre migra para o formato novo. A senha mestra é a mesma. Nada a fazer
além de destravar.

## 5. Confira as entradas

```sh
ward list                    # as mesmas entradas de antes, com os rótulos em português
ward list --json | head      # chaves em inglês (name, type, group, environment…)
ls -l ~/.local/share/ward/   # cofre.ward (mesmo nome), estado e backups/ com uma versão nova
```

No app, abra algumas entradas e confira campos, anexos, níveis e a configuração (tempo
de trava, backup externo). O PIN é o mesmo.

## 6. Atualize a skill e o deny no `~/.claude`

Você faz isto à mão, a partir de `skill/` do checkout principal (`skill/README.md` ›
"Atualizar da 0.1.x para a 0.2.0"):

1. Copie a pasta `skill/ward/` por cima de `~/.claude/skills/ward/`:

   ```sh
   cp -a ~/projetos/cofre/skill/ward/. ~/.claude/skills/ward/
   ```

2. Acrescente à lista `permissions.deny` do `~/.claude/settings.json` as regras de
   `skill/settings-deny.json` que ainda não estão lá. Não substitua o arquivo. As regras
   antigas (`*ward*destravar*` e afins) podem ficar.
3. No Claude Code, `/permissions` mostra as regras novas em "Deny" (por exemplo as de
   `unlock`).

## 7. Confira o app, a bandeja e a aprovação

1. **Janela nativa.** Abra o Ward pelo menu de aplicativos. A janela é a mesma da 0.1.x:
   barra de título escura com minimizar, maximizar e fechar, as mesmas telas e textos.
   Feche a janela (o Ward fica na bandeja) e abra o Ward **de novo pelo menu de
   aplicativos**: a janela principal volta (correção da 0.1.5, Fase 66a). Repita com a
   janela minimizada: ela volta e ganha o foco.
2. **Bandeja.** Feche a janela: o Ward fica na bandeja. O menu da bandeja diz "Cofre
   destravado" e tem **Travar agora** e **Sair**. Clique em **Travar agora**: o cofre
   trava e a bandeja passa a dizer "Cofre travado". Destrave de novo (passo 4).
3. **Aprovação.** Rode um `ward run` de uma entrada com nível de aprovação, por
   exemplo `ward run <entrada-com-aprovacao> -- true`. A janela de aprovação abre com a
   entrada, o ambiente em destaque, o comando e a cadeia de processos, e os 4 botões
   **Negar**, **Permitir uma vez**, **Permitir por 1 hora** e **Permitir sempre**.
   Permitir pede o PIN. Negue uma vez e permita outra para ver os dois lados.
4. **Código TOTP no detalhe (novo).** Abra uma entrada com TOTP. No detalhe, no lugar
   da antiga "Semente TOTP", aparece **Código TOTP**: o código atual em dois grupos de 3
   dígitos, a contagem até o próximo e o botão **Copiar**. A semente não aparece mais no
   detalhe; ela fica só no formulário de edição (Editar), com revelar e trocar.
5. **Extensão.** Feche e abra o Chrome. Em `chrome://extensions` a extensão Ward está na
   0.2.0 (mesmo id). Até o Chrome atualizar, a extensão 0.1.x não preenche nada; isso é
   esperado.

Se algo aqui não bater com a 0.1.x, não use a 0.2.0 no dia a dia: volte pelo passo 9 e
avise.

## 8. Depois de uns dias

Quando tiver certeza de que não vai voltar, apague a cópia:

```sh
rm -rf ~/ward-copia-0.1.x
```

## 9. Como voltar para a 0.1.x (pela cópia do passo 1)

Não há volta automática: a 0.1.x não abre o cofre migrado. O que você mudou no cofre
depois de atualizar (entradas novas, senhas trocadas) se perde e tem de ser refeito à
mão.

1. Reinstale a 0.1.x pelo mesmo meio de antes: o `.deb` da 0.1.x copiado para
   `/root/ward` e `sudo dpkg -i` de lá (como no 3a), ou o `make install` da tag dela
   rodado de uma cópia em `/root` (como no 3b), nunca de dentro de um clone. Pelo `.deb`, o mais seguro é
   `sudo dpkg -r ward` antes (o cofre fica intacto) e depois `sudo dpkg -i` da 0.1.x; o
   downgrade direto não foi conferido pela missão.
2. Pare o daemon, guarde o cofre migrado à parte (não apague) e reponha a cópia:

   ```sh
   sudo systemctl stop "wardd@$(id -u).socket" "wardd@$(id -u).service"
   mv ~/.local/share/ward ~/ward-migrado-0.2.0
   install -d -m 0700 ~/.local/share/ward
   cp -a ~/ward-copia-0.1.x/. ~/.local/share/ward/
   sudo systemctl start "wardd@$(id -u).socket"
   ```

3. Destrave a 0.1.x pelo caminho completo (na 0.1.x o subcomando ainda é
   `/usr/local/bin/ward destravar`) e confira as entradas.
4. Volte a skill e o deny do `~/.claude` para os da 0.1.x (a skill da 0.2.0 usa
   `ward list`, que a 0.1.x não conhece).

Sem a cópia do passo 1, o caminho de volta não está conferido (veja
`docs/instalacao.md` › "Voltar para a 0.1.x").
