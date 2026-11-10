# Fase 51 — `make install`/`uninstall` com `DESTDIR` e `PREFIX=/usr/local`

## Implementado
- **`instalacao/`**, pasta nova com o que vai para o sistema:
  - `systemd/wardd@.socket` e `wardd@.service`: as units da Fase 24, movidas de `dev/systemd/` (`git mv`). O conteúdo é o mesmo; saiu só o `Documentation=` que apontava para um README que não existe. As referências foram atualizadas em `crates/wardd/tests/units.rs`, `dev/testes/units.sh`, no `Makefile` e no `CLAUDE.md`.
  - `tmpfiles.d/ward.conf`: `d /run/ward 0755 root root -`.
  - `arvore.txt`: a lista declarativa da árvore, com tipo, modo, dono, grupo, caminho e origem. Ela é a fonte do instalador, do uninstall e do teste. A Fase 52 (`dpkg-deb`) pode tirar os donos daqui. São 11 arquivos:
    - `/usr/local/bin/{ward,wardd}` 0755 e `/usr/local/bin/ward-app` root:ward-app 2755;
    - `/usr/local/lib/ward/ward-nm` root:ward-nm 2755;
    - `/usr/local/lib/systemd/system/wardd@.{socket,service}` e `/usr/local/lib/tmpfiles.d/ward.conf`;
    - `/usr/local/share/ward/extensao/{ward.crx,update.xml}`;
    - `/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json` e `/etc/opt/chrome/policies/managed/ward.json`.
  - `instalar.sh instalar|desinstalar` (`#!/bin/bash -p`, PATH fixo, `LC_ALL=C`, `umask 077`):
    - **Nunca chama o cargo.** O build é do usuário, sem root, pelo `make release`.
    - **Ordem com falha fechada.** Primeiro confere tudo e copia para uma pasta `mktemp -d` dele. Depois vêm os grupos, os arquivos e o systemd. As conferências valem para a cópia (`cp -P`, depois confere que é arquivo regular), não para o caminho do usuário. Assim, um processo do mesmo uid que troque o artefato por um link para o `/etc/shadow` depois da conferência não ganha nada.
    - **Artefatos.** Cada um tem de ser arquivo regular sem link, ELF e sem marca de `modo-teste`. As marcas são `WARD_DEV_*` (menos `WARD_DEV_DBUS_SISTEMA`, ver Problemas), `ward-ponte-teste`, `wardd-app-teste` e `ward-teste`.
    - **Extensão.** `EXTENSAO_ID` precisa casar `^[a-p]{32}$`. O pacote `EXTENSAO_CRX` é lido pelo `instalacao/crx.py`, que só usa a biblioteca padrão. Ele lê o protobuf do CRX3, exige que o `crx_id` assinado seja o sha256 de uma chave RSA do cabeçalho, lê a versão do `manifest.json` e confere a regex de versão do Chrome. O id do pacote tem de ser igual ao `EXTENSAO_ID`.
    - **`PREFIX`.** Só `/usr/local` é aceito, porque os binários têm `/usr/local/bin/ward-app`, `/usr/local/bin/ward` e o `ExecStart` fixos.
    - **`DESTDIR`.** Tem de ser absoluto, sem `//`, `.` ou `..`, e sem link no caminho.
    - **Grupos (D60).** `groupadd --system` quando o grupo falta. Um grupo existente só é aceito se cumprir tudo isto:
      - uma linha só no `/etc/group`, gid de 1 a 999 e nenhum outro grupo com o mesmo gid;
      - sem membros, sem administradores e com `!` no `gshadow`;
      - não é grupo primário de nenhum usuário (`/etc/passwd` e NSS);
      - fora de toda faixa do `/etc/subgid`;
      - o NSS devolve o mesmo grupo pelo nome e pelo gid. Grupo que só existe no NSS é recusado.
      Recusado, nada é instalado. Um grupo recém-criado que é recusado é apagado.
    - **Pastas.** Pasta existente no caminho não pode ser link e, no sistema, tem de ser do root e sem escrita para grupo e outros. Pasta que falta é criada 0755 root:root.
    - **Arquivos.** Cada um vai para um `mktemp` na mesma pasta, recebe `chown` e depois `chmod` (o `chown` tira o setgid) e é trocado por `mv -T`. Um link no destino é substituído, nunca seguido.
    - **Systemd.** `systemd-tmpfiles --create`, `daemon-reload`, `systemctl enable --now wardd@$WARD_UID.socket` e `try-restart` do serviço (a reinstalação troca o binário e trava o cofre). `WARD_UID` vem por padrão do `SUDO_UID`; precisa ser ≥ 1000, existir e não ser o nobody.
    - **Avisos.** Avisa se `ptrace_scope=0` ou `suid_dumpable=1`, os casos em que o daemon recusa os papéis.
    - **Com `DESTDIR`.** Só copia a árvore: sem grupos, sem `chown` de arquivo e sem systemctl.
    - **Desinstalar.** Faz `disable --now`/`stop` de todo `wardd@<n>.socket|service` carregado ou habilitado (nome validado por regex). Apaga os arquivos da árvore e as pastas próprias que ficarem vazias, depois faz `daemon-reload` e `reset-failed`. Apaga `/run/ward/*` e `/run/ward` se estiverem vazias e os grupos `ward-nm`/`ward-app`. Nunca toca no home.
- **Makefile:**
  - `make release` compila `ward`, `wardd` e `ward-nm` com `-p`, uma chamada por pacote, sem `modo-teste`, mais o app (`app-front`, build do Tauri e `app-release.sh`).
  - `make install` e `make uninstall` chamam o `instalar.sh` e não dependem de `origens`, porque rodam como root.
  - As variáveis `PREFIX`, `DESTDIR`, `WARD_UID ?= $(SUDO_UID)`, `EXTENSAO_ID`, `EXTENSAO_CRX` e `BIN_{WARD,WARDD,NM,APP}` vão pelo ambiente (`export` por alvo), nunca interpoladas na linha do shell.
  - `make teste-install` é o alvo de teste, fora do `make check`.
- **`make teste-install`** (`dev/testes/install.sh`, `dev/testes/crx_falso.py`, `dev/conteiner-install/{Dockerfile,dentro.sh}`):
  1. **Host, sem root, com `DESTDIR` em `.dev/`.** Usa os artefatos sem `modo-teste` (debug), o `partida-conteiner` como `ward-app` e um CRX3 falso. Confere:
     - a árvore exata (V9.1), os modos e nada com escrita para grupo e outros;
     - binários, units, tmpfiles e pacote iguais aos de origem;
     - o JSON do native messaging (V9.6), a política e o `update.xml`;
     - o `systemd-analyze verify` das units instaladas;
     - a reinstalação por cima de um link plantado;
     - o uninstall (V9.10), que deixa intactos um "cofre" plantado e um arquivo de outro pacote;
     - 22 recusas, listadas em Segurança.
  2. **Contêiner `ubuntu:24.04`** (mesma digest), com systemd, make e python3, sem rede e com as mesmas permissões do `teste-units`. Roda `make install`/`make uninstall` de verdade como root, com 102 conferências (resultados em Segurança).
- **CLAUDE.md:** linhas novas para `make release`, `sudo make install`/`uninstall` e `make teste-install`. A linha do `teste-units` aponta para `instalacao/systemd/`.

## Segurança
- **Invariantes tocadas:**
  - **S5/D46.** Socket do sistema em pasta do root, conferido no contêiner. A CLI instalada recusa o servidor falso do uid antes da senha (V9.3).
  - **S8/D48/D57/D62.**
    - As units não têm `NoNewPrivileges=` nem implicações (`systemctl show` e `systemd-analyze security` dão "no"/✗).
    - Ficam `LimitCORE=0` e `LimitMEMLOCK=infinity`.
    - O `wardd` instalado é não-dumpable: o `/proc` é do root e o uid não lê `environ` nem `mem`.
    - O `LD_PRELOAD` que o atacante põe no gerente do usuário não chega ao `wardd` (V9.4).
  - **S1/D47/D60/D63.** Grupos conferidos (V9.11). O app aberto pelo daemon recebe o setgid e o papel app; o aberto pelo usuário, não.
  - **D7/D63.** O artefato `modo-teste` é recusado antes de qualquer mudança (V9.2), inclusive no contêiner, onde nenhum grupo é criado.
  - **S14.** Nenhuma dependência nova.
- **Testes de ataque** (todos falham do lado do atacante):
  - `dev/testes/install.sh` › seção "1b. Recusas":
    - `V9.2: wardd com modo-teste` (o binário de teste de verdade);
    - `V9.2: ward com WARD_DEV_SOCKET_DIR`, `ward-nm com WARD_DEV_DIR` e `ward-app com a ponte de teste`;
    - `wardd que não é ELF`, `ward por link simbólico`, `artefato ausente`;
    - `EXTENSAO_ID` vazio, em maiúsculas, com 31 letras e com aspas (injeção no JSON);
    - id de outra extensão, pacote com id que não é o da chave, versão com `'><`, pacote ausente;
    - `PREFIX=/opt/ward` e `PREFIX=/usr/local/../tmp`;
    - `DESTDIR` por link, `DESTDIR` com `..` e `DESTDIR` relativo;
    - `usr/local/bin` como link dentro do `DESTDIR` (nada escrito no alvo);
    - link plantado no lugar do `wardd`, que é trocado sem escrever no alvo;
    - `make install`/`uninstall` no sistema sem root, recusado antes de tocar em qualquer coisa.
  - `dev/conteiner-install/dentro.sh`:
    - **V9.11.** Grupo existente com gid ≥ 1000, com membro, com senha no `gshadow`, com administrador, com outro grupo no mesmo gid, como grupo primário de usuário e com gid numa faixa do `/etc/subgid`, mais o grupo novo que cairia no `subgid`. Em todos, a instalação recusa e não deixa nenhum arquivo, nem o setgid. Depois de instalado, `newgrp`/`sg ward-app` pelo uid 1000 falham.
    - **V9.2.** Recusado sem criar grupo.
    - **V9.4.** `systemctl set-environment` do sistema pelo uid é negado. `systemctl --user set-environment LD_PRELOAD=libmarca.so` funciona no `user@1000` real. Mesmo assim, com o `wardd` morto e reconectado (cliente também com `LD_PRELOAD`), o `wardd` novo não tem `LD_*` nem a lib no `maps`.
    - **V9.3.** O `wardd` copiado para `~/bin` não sobe. Com o falso não-dumpable em `/run/user/1000/ward/` e `WARD_SOCKET` apontando para ele, `ward destravar` sai ≠ 0 sem prompt e o falso recebe 0 conexões.
- **Dependências novas:** nenhuma. O `crx.py` só usa a biblioteca padrão. A imagem do teste ganha `make` e `python3` do apt do Ubuntu, só no contêiner.

## Ficou de fora
- **Extensão de verdade (Fase 41 ainda não está no master).** `EXTENSAO_ID` e `EXTENSAO_CRX` são variáveis obrigatórias do `make install`; sem elas a instalação recusa. Os testes usam um CRX3 falso. Quando a 41 sair, o usuário passa o id e o `.crx` empacotado com a chave dela.
- **App Tauri de release no teste.** O contêiner não tem WebKitGTK, então o `ward-app` foi o `partida-conteiner` (a mesma partida). O `make release` foi escrito, mas a parte do Tauri não rodou aqui (disco apertado). Rodaram `ward`, `wardd` e `ward-nm` em release (`cargo build --release -p …`), e o `make install DESTDIR` os aceitou.
- **Roteiro do usuário (V9.5, V9.7 na sessão real, V9.8):**
  - Chrome com a política: a extensão aparece como instalada pela organização e o `update_url` `file://` funciona;
  - `/run/user/<uid>/ward/` 0700 com logind;
  - `systemd-inhibit --list` (polkit do inibidor, handoff 24);
  - `~/.ssh` e `ssh_config` intactos.
  Ficam com a Fase 52 e o roteiro.
- **`systemctl --root=DESTDIR enable`.** O link `sockets.target.wants` depende do uid, então no staging não é criado. O `.deb` (Fase 52) faz isso no `postinst`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (gitleaks copiado do checkout principal) + `make gitleaks-binario` | 0 |
| `make dev-cofre` | 0 |
| `dev/testes/install.sh --so-destdir` (1ª: 7 falhas do próprio teste, contador em subshell; corrigido) | 1 → 0 |
| `make teste-install` (pelo script; 1ª: `systemd --user` não subia; corrigido com o `user@1000` e os drop-ins só do contêiner) | 1 → 0 (185 ok) |
| `make teste-units` (units movidas) | 0 |
| `cargo build --release -p ward/-p wardd/-p ward-nm` + `make install DESTDIR` com eles | 0 |
| `make check` (3 rodadas, com o host em load ~9 de outros workers) | 2 / 2 / 2: fmt e clippy limpos nas três; falhou só o `cargo test`, em testes sensíveis a tempo fora da fase (`wardd --test socket` › `v3_33_kill_9_entre_as_senhas_erradas` na 1ª e na 3ª; `wardd --test ssh` › `d34_pty_prefixo_parado_vira_marca_e_o_resto_nao_sai` na 2ª) |
| `cargo test -p wardd --test ssh d34_pty` sozinho, 3 vezes | 0 / 0 / 0 |
| `make test` (4ª rodada, sozinho) | 0 |
| `make audit` / `make deny` / `make gitleaks` (os passos que o `make check` não alcançou) | 0 / 0 / 0 |

## Problemas descobertos
- **O `make check` inteiro não saiu 0 numa rodada só.** Nas três rodadas, um teste sensível a tempo do `wardd` falhou com o host em load ~9 de outros workers:
  - `tests/socket.rs` › `v3_33_kill_9_entre_as_senhas_erradas` (esperas de 2,8 a 3,2 s; já anotado nas Pendências da 23a/22c);
  - `tests/ssh.rs` › `d34_pty_prefixo_parado_vira_marca_e_o_resto_nao_sai` (corrida entre o `Janela` aos 300 ms e o filho imprimir o tamanho).
  Nenhum dos dois toca em código desta fase, e cada um passa rodado sozinho. Fora isso, todos os passos passaram: fmt e clippy nas três rodadas, `make test` completo na 4ª, e audit, deny e gitleaks à parte. Os dois testes precisam de mais margem; fica para o orchestrator decidir a fase.
- **O `wardd` de release carrega o texto `WARD_DEV_DBUS_SISTEMA`.** Está em `trava.rs::caminho_de_teste` e na mensagem do `main.rs`, código sem `cfg`, mas só lê a variável no `modo-teste`. Por isso a recusa de V9.2 não pode ser um "qualquer `WARD_DEV_`": o instalador recusa toda `WARD_DEV_*` menos essa. Sugestão para uma fase do daemon: pôr essa mensagem e `caminho_de_teste` atrás de `cfg(feature = "modo-teste")` e tirar a exceção do `instalar.sh`.
- **`sudo make install` de uma árvore gravável pelo usuário.** Um pacote malicioso do mesmo uid pode trocar o `Makefile`, o `instalar.sh` ou o `arvore.txt` antes do `sudo`, e isso vira root. Os artefatos estão protegidos (conferência na cópia), mas os scripts não. Vale para todo `sudo make install`. O `.deb` construído pelo usuário tem o mesmo problema. Sugestão para o roteiro (Fase 52): instalar de um clone limpo e conferir `git status`/`git diff` antes do `sudo`. Decisão do planner/usuário.
- **PRD D18 desatualizado.** Ainda fala em `/usr/local/lib/systemd/user/wardd.service`. Pela D46 a instalação usa units do sistema em `/usr/local/lib/systemd/system/`, `tmpfiles.d` e `/usr/local/share/ward/extensao/`. Não mexi no PRD.
- **Política do Chrome.** `ExtensionSettings` num arquivo próprio (`ward.json`) não se funde com outro `ExtensionSettings` em outro arquivo de `/etc/opt/chrome/policies/managed/` (o Chrome só funde com `PolicyDictionaryMultipleSourceMergeList`). Também não confirmei que o Chrome aceita `update_url` `file://` para force-install no Linux (V9.5, roteiro da 52). Se não aceitar, a alternativa é o `external_crx` em `/opt/google/chrome/extensions/<id>.json`, mais a política só para impedir desativar.
- **O `crx.py` não confere assinatura** (exigiria RSA). Ele garante que o id instalado é o do pacote; a assinatura quem confere é o Chrome.
- **Sem `.desktop` do app.** O D18 não lista e não instalei. O app hoje abre pelo daemon (aprovação, `abrir_app`) ou por `ward-app` no terminal. Menu e autostart ficam para o planner.
- **O uninstall apaga `ward-app`/`ward-nm` mesmo que existissem antes da instalação**, porque não há registro disso. Também, se a instalação cria `ward-app` e depois recusa o `ward-nm`, o `ward-app` (conforme) fica.
- **Com `DESTDIR`, a pasta é criada antes das conferências.** Uma recusa deixa a pasta vazia. É cosmético.
- **Imagem do Docker.** O `make teste-install` deixa a imagem `ward-teste/install-systemd:1` no Docker do host, como o `teste-units`. Apaguei a minha no fim da fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `instalacao/systemd/wardd@.socket` | movido de `dev/systemd/` e alterado (comentário, sem `Documentation=`) |
| `instalacao/systemd/wardd@.service` | movido de `dev/systemd/` e alterado (comentário, sem `Documentation=`) |
| `instalacao/tmpfiles.d/ward.conf` | criado |
| `instalacao/arvore.txt` | criado |
| `instalacao/instalar.sh` | criado |
| `instalacao/crx.py` | criado |
| `dev/testes/install.sh` | criado |
| `dev/testes/crx_falso.py` | criado |
| `dev/conteiner-install/Dockerfile` | criado |
| `dev/conteiner-install/dentro.sh` | criado |
| `dev/testes/units.sh` | alterado (caminho das units) |
| `crates/wardd/tests/units.rs` | alterado (caminho e doc) |
| `Makefile` | alterado (`release`, `install`, `uninstall`, `teste-install`, caminho das units) |
| `CLAUDE.md` | alterado (3 linhas novas, caminho das units) |
| `docs/mission/handoffs/phase-51.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): todos os passos passaram, mas não numa rodada só (ver Problemas)
- [x] Só cofre de teste e credenciais falsas usados (nada instalado no host, nenhum sudo)
- [x] Teste de ataque para cada (SEG) tocado (V9.2, V9.3, V9.4, V9.6, V9.11)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
