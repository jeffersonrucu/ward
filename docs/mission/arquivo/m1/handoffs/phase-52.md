# Fase 52 — `.deb` por `dpkg-deb`, teste no contêiner `ubuntu:24.04` e roteiro de instalação

## Implementado
- **`make deb`** (`instalacao/deb.sh`). Roda como o usuário e recusa root. Nunca chama o cargo.
  1. **Staging.** É o próprio `instalar.sh instalar` com `DESTDIR=<DEB_DIR>/raiz`, então valem as mesmas recusas do `make install`: `modo-teste`, não-ELF, link, `EXTENSAO_ID` ≠ pacote, `PREFIX`, `DEB_DIR` relativo, com `..` ou por link.
  2. **Cópia sem setuid/setgid.** O `--root-owner-group` deixa tudo root:root. Um 2755 root:root daria o gid do root a quem rodasse o binário, por isso o bit sai. Os binários setgid vão 0755 no pacote e o 2755 com o grupo entra pelo `dpkg-statoverride`.
  3. **`DEBIAN/`.** O `control` declara `Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0`. A versão vem do `[workspace.package]` e a arquitetura do `dpkg --print-architecture`. Os scripts saem de `instalacao/deb/`.
  4. **Build e conferência.** `dpkg-deb --root-owner-group -Zxz --build`, depois uma conferência do resultado:
     - a lista do `dpkg-deb -c` é igual à do staging (V9.9);
     - tudo é root/root, sem link, sem bit especial e sem escrita para grupo e outros;
     - o Depends tem sshpass e os quatro scripts estão lá.
  5. Imprime o sha256. A saída vai para `target/deb/ward_0.1.0_amd64.deb`.
- **Scripts do mantenedor** (`instalacao/deb/`, todos `#!/bin/bash -p` com PATH fixo):
  - **`preinst`:**
    - recebe as funções de `instalacao/grupos.sh`, as mesmas do `make install`, e as pastas `D` da `arvore.txt`;
    - confere as pastas: sem link, do root e sem escrita para grupo e outros;
    - aplica `garantir_grupo ward-app`/`ward-nm` (D60);
    - registra os overrides `root ward-app 2755` e `root ward-nm 2755`, trocando qualquer override diferente;
    - avisa sobre `ptrace_scope`/`suid_dumpable`.
  - **`postinst`:**
    - exige o override exato e aplica `chown root:<grupo>` seguido de `chmod 2755`. Na 1ª instalação o dpkg lê a base de overrides antes do preinst, então o arquivo chega 0755 root:root, sem setgid, até esse ponto (verificado no contêiner);
    - roda `systemd-tmpfiles --create` e `daemon-reload`;
    - faz `enable --now wardd@<uid>.socket`, com o uid vindo de `WARD_UID` ou `SUDO_UID` (≥ 1000, existente, não nobody). Sem uid, imprime o comando;
    - faz `try-restart` dos `wardd@*.service` ativos.
  - **`prerm remove`:** `disable --now`/`stop` de todo `wardd@*`, como o `make uninstall`.
  - **`postrm remove|purge`:** tira os overrides, `/run/ward` vazia e os grupos. Nunca toca no home.
  - **`postrm abort-install`:** tira só os overrides. Um grupo que já existia e foi recusado é do administrador e não é apagado.
- **`instalacao/grupos.sh`.** `conferir_grupo`/`garantir_grupo` saíram do `instalar.sh` sem mudança de lógica. O `instalar.sh` agora faz `source` desse arquivo e o `deb.sh` o embute no preinst. O `dev/testes/install.sh` copia o arquivo para o contêiner.
- **`make extensao-crx`** (`instalacao/empacotar-extensao.sh`). Empacota `extensao/dist/` como CRX3 pelo próprio Chrome (`--headless=new --pack-extension`), sem assinatura feita à mão.
  - HOME e perfil ficam em `target/extensao-crx.*` e são apagados no fim; o perfil do usuário não é usado.
  - A chave é `.dev/extensao/chave.pem`, que tem de ser do usuário, 0600 e sem link, e vai só por caminho.
  - O `crx.py` confere o pacote, e o id tem de ser o `ID_EXTENSAO` do `ward-nm`.
  - A saída vai para `target/extensao/ward.crx`. O handoff 41 deixou o `.crx` assinado para a 51/52.
- **`make teste-deb`** (`dev/testes/deb.sh`, `dev/conteiner-deb/{Dockerfile,dentro.sh}`), fora do `make check`, 127 ok:
  1. **Host.**
     - V9.9: `dpkg -c` = staging = `arvore.txt` (28 caminhos);
     - tudo root/root, sem s/S/t, `ward-app`/`ward-nm` 0755 no pacote;
     - conteúdo do pacote = staging = artefatos;
     - control e Depends conferidos;
     - preinst com as funções de grupos e sem marcas de modelo, sem conffiles;
     - as recusas e os ataques ao `extensao-crx` (lista em Segurança).
  2. **Contêiner `ubuntu:24.04`** (mesma digest, systemd, sem rede, mesmas permissões do `teste-units`). A imagem `ward-teste/deb-systemd:1` traz openssh-client e libwebkit2gtk-4.1-0 instalados e o sshpass só baixado em `/opt/debs`. Fluxo:
     - `dpkg -i` sem sshpass falha citando sshpass, e o pacote desempacotado fica sem setgid;
     - `dpkg -r` desse estado limpa tudo;
     - 4 casos de grupo fora de D60, cada um recusado sem arquivo, sem override e sem pacote;
     - instalação com `SUDO_UID=1000`: `dpkg -L` com os 11 arquivos, donos e modos da `arvore.txt`, overrides exatos e grupos com gid < 1000, sem membros e com `!`;
     - `sg ward-app` falha, o socket fica habilitado e ativo, `/run/ward`, `/run/ward/1000` e o socket têm os modos certos;
     - a primeira conexão sobe o `wardd` como uid 1000, sem `no_new_privs` e não-dumpable;
     - V9.8: `~/.ssh` (chave falsa gerada no contêiner), `/etc/ssh`, `ssh -G` e `SSH_AUTH_SOCK` do `user@1000` iguais depois de instalar e de remover;
     - reinstalação: o override vale no desempacotar e o socket continua;
     - instalação sem `SUDO_UID` (dá a mensagem) e com `WARD_UID=0` (recusado);
     - `dpkg -r`/`-P`: tudo apagado e o "cofre" plantado intacto.
- **`docs/instalacao.md`**, o roteiro do usuário:
  - pré-checagem do kernel;
  - retrato do SSH antes e `diff` depois (V9.8, com `git push`);
  - clone limpo e cópia do `.deb` para `/root/ward` antes de conferir e instalar;
  - build (`release`, `extensao`, `extensao-crx`, `deb`);
  - `apt install`/`dpkg -i` ou `make install`;
  - systemd (V9.7) e V9.4 opcional (`LD_PRELOAD` no gerente do usuário);
  - Chrome (V9.5): `chrome://policy`, instalada pela organização, sem remover nem desativar, "Carregar sem compactação" com o mesmo id recusado. Se falhar, mostra o caminho de volta para a missão;
  - desinstalar.
- **`CLAUDE.md`:** linhas novas para `extensao-crx`, `deb` e `teste-deb`. A linha do `make install` não diz mais "falta a extensão".

## Segurança
- **Invariantes tocadas:**
  - **S1/D47/D60.** Grupos pelas mesmas regras do `make install`, num código só (`grupos.sh`). Nenhum binário setgid com grupo errado: no pacote vão 0755 e o setgid só entra com o override exato.
  - **S5/S8/D46/D57.** As units e o socket são os mesmos da 51. O `wardd` instalado pelo `.deb` não tem `no_new_privs` e é não-dumpable.
  - **D7/D63.** Artefato `modo-teste` recusado também no `make deb`.
  - **R19 (V9.8).** O SSH fica igual, conferido no contêiner.
  - **S14.** Nenhuma dependência nova de build.
- **Testes de ataque:**
  - `dev/testes/deb.sh` › "1b. Recusas":
    - `V9.2: wardd com modo-teste`;
    - `EXTENSAO_ID de outra extensão`, `EXTENSAO_ID vazio`;
    - `DEB_DIR por link` (nada escrito no alvo) e `DEB_DIR relativo`;
    - nenhum deixa `.deb`.
  - `dev/testes/deb.sh` › "1c": `extensao-crx` com outra chave (outro id), chave 0644, chave por link e link plantado no `dist/`, todos recusados sem pacote.
  - `dev/testes/deb.sh` › seção 1: "nenhum bit setuid/setgid/sticky no pacote", "todo caminho root/root" e "nada com escrita para grupo e outros". Cobrem o risco do setgid root:root.
  - `dev/conteiner-deb/dentro.sh`:
    - V9.11 pelo `.deb`: gid ≥ 1000, com membro, com senha no gshadow e grupo primário de usuário;
    - "desempacotado e não configurado: sem setgid";
    - `sg ward-app` pelo uid falha;
    - `WARD_UID=0` não liga `wardd@0.socket`;
    - V9.8, retrato igual.
- **Dependências novas:** nenhuma de build.
  - O `.deb` declara em runtime `sshpass` (pedido pelo contrato), `openssh-client` (o `ward ssh` chama o `ssh`), `systemd` e `libwebkit2gtk-4.1-0` (o `ward-app`).
  - A imagem de teste instala openssh-client e libwebkit2gtk-4.1-0 do apt do Ubuntu e baixa o sshpass, tudo só no contêiner.

## Ficou de fora
- **Roteiro rodado de verdade** (V9.5, V9.7 com logind, V9.8 com o `git push`): é do usuário (`docs/instalacao.md`). A missão não roda `sudo`.
- **`make extensao-crx` com a chave real.** A chave fica em `.dev/extensao/chave.pem` do checkout principal, e este worktree não tem. Não copiei a chave nem rodei o `make app-front`/`make extensao` aqui. O caminho positivo foi conferido à mão com uma chave descartável: o Chrome empacota e o `crx.py` lê. O caminho com o id certo fica para o roteiro.
- **`.desktop` do app:** continua sem, como na 51.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo build` dos artefatos sem `modo-teste` (`target/sem-modo-teste`) e do `wardd` de teste | 0 |
| `dev/testes/deb.sh --so-host` (1ª: regex das pastas do preinst sem `.`; corrigido) | 1 → 0 |
| `dev/testes/deb.sh` completo (1ª: override não aplicado na 1ª instalação; corrigido no postinst) | 1 → 0 |
| `make teste-deb` | 0 (127 ok) |
| `dev/testes/install.sh --so-destdir` (depois do `grupos.sh`) | 0 (83 ok) |
| `make dev-cofre` | 0 |
| `dev/testes/install.sh` completo, com o contêiner (regressão da 51 com o `grupos.sh`) | 0 (185 ok) |
| `make check` (gitleaks copiado do checkout principal, conferido pelo sha256 do Makefile) | 0 |

## Problemas descobertos
- **O dpkg não aplica na primeira instalação um override registrado no preinst.** Ele leu a base antes. Por isso o postinst aplica o modo, que é o mesmo do override. Entre o desempacotar e o postinst o binário fica 0755 root:root, sem setgid. Esse estado é seguro e o teste o confere.
- **Build de usuário e instalação de root.** O mesmo risco da 51: um processo do mesmo uid pode trocar os scripts ou o `.deb` antes do `sudo`.
  - O roteiro reduz o risco com o clone limpo e com a cópia para `/root/ward` antes de `sha256sum`/`dpkg-deb -I`/`-c`.
  - Ainda assim, o build roda como o usuário.
  - **Decisão do usuário/planner.** Uma opção seria o build num contêiner ou num usuário separado.
- **Depends com `libwebkit2gtk-4.1-0` e `openssh-client`, além do `sshpass` do contrato.** Faltando o primeiro, o `ward-app` não abre. Faltando o segundo, o `ward ssh` não funciona. Confirme se é o desejado.
- **Sem `conffiles`.** A política e o manifesto em `/etc` são gerados com o id e voltam ao original na atualização. Uma edição local não sobrevive. Escolhi assim de propósito: o arquivo do root é a fonte.
- **`/usr/local` num `.deb`** vai contra a Debian Policy (o lintian reclamaria). O dpkg aceita, e o escopo (R4, D18) pede `/usr/local`.
- **PRD D18 desatualizado** (já anotado na 51). Ainda fala em unit de usuário. Não mexi.
- **`file://` no `update_url` com force-install** continua sem confirmação (V9.5). O roteiro diz como reconhecer a falha e manda trazer de volta para a missão. A alternativa é o `external_crx`.
- **Os grupos saem no `dpkg -r`** mesmo se já existiam antes, como no `make uninstall`.
- **Imagens Docker.** Apaguei as do `teste-deb` e do `teste-install` no fim. As duas se reconstroem com rede no próximo `make`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `instalacao/deb.sh` | criado |
| `instalacao/deb/preinst` | criado |
| `instalacao/deb/postinst` | criado |
| `instalacao/deb/prerm` | criado |
| `instalacao/deb/postrm` | criado |
| `instalacao/grupos.sh` | criado (funções movidas do `instalar.sh`) |
| `instalacao/empacotar-extensao.sh` | criado |
| `instalacao/instalar.sh` | alterado (`source` de `grupos.sh`) |
| `dev/testes/deb.sh` | criado |
| `dev/conteiner-deb/Dockerfile` | criado |
| `dev/conteiner-deb/dentro.sh` | criado |
| `dev/testes/install.sh` | alterado (copia `grupos.sh` para o contêiner) |
| `docs/instalacao.md` | criado |
| `Makefile` | alterado (`deb`, `extensao-crx`, `teste-deb`, `.PHONY`) |
| `CLAUDE.md` | alterado (3 linhas novas, status do `make install`) |
| `docs/mission/handoffs/phase-52.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (nada instalado no host, nenhum sudo; chave SSH falsa gerada só no contêiner)
- [x] Teste de ataque para cada (SEG) tocado (V9.2 e V9.11 pelo `.deb`, setgid fora do pacote, V9.8)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
