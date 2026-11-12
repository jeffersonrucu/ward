# Fase 51a — Achados da auditoria do M9

## Implementado
- **Crítico: política do Chrome com `"NativeMessagingUserLevelHosts": false`.**
  - O `instalacao/instalar.sh` gera o `ward.json` com essa chave, ao lado de
    `ExtensionSettings`. O `.deb` usa o mesmo staging, então o pacote sai igual.
  - Com a chave, o Chrome só abre host de native messaging de
    `/etc/opt/chrome/native-messaging-hosts/`. Um `br.ward.nm.json` plantado no perfil
    é ignorado.
  - Conferido em `teste-install` (host e contêiner) e em `teste-deb` (pacote extraído e
    instalado).
- **Teste com o Chrome real: `make teste-politica-chrome`** (`dev/testes/politica-chrome.sh`
  e `dev/testes/politica_chrome.py`, feitos a partir de `e2e/rodar.py`,
  `e2e/hospedeiro.py` e `nmhijack/forca*.py` da auditoria).
  - Gera a política pelo `instalar.sh` (DESTDIR em `.dev/politica-chrome.*`, artefatos
    sem `modo-teste`).
  - Roda o Chrome num `bwrap --dev-bind / / --tmpfs /etc/opt --bind <trab>/etc-chrome /etc/opt/chrome`.
    O script confere por uma marca que `/etc/opt/chrome` é mesmo a pasta do teste.
  - São quatro rodadas:
    1. **Precedência sem política (controle).** O host do perfil é chamado:
       `{"sistema": false, "usuario": true}`.
    2. **Precedência com o `ward.json` gerado.** `{"sistema": true, "usuario": false}`.
    3. **Ataque inteiro da auditoria sem política (controle).** Roda fora do bwrap,
       explicação em Problemas. O host do perfil faz `exec` do `ward-nm` de teste e
       tira 4 `preenchimento` com senha/TOTP de um `wardd` de teste só dele
       (`WARD_DEV_EXTENSAO=chrome`). Os logs do daemon e do Chrome ficam sem padrão de
       `.dev/variacoes.txt`.
    4. **O mesmo ataque com a política.** `{"atacante_rodou": false, "sistema_rodou": true, "com_segredo": 0}`.
  - O que o controle tira (falso, D15) nunca é impresso: só pedido e tipo. O arquivo é
    apagado logo depois do resumo.
- **Baixo: outras políticas em `policies/managed/`.** O `instalacao/chrome.sh` traz
  `conferir_politicas_chrome`, usada pelo `instalar.sh` e pelo `preinst`.
  - Recusa, antes de qualquer mudança, se outro arquivo da pasta tiver
    `ExtensionSettings`, `NativeMessagingUserLevelHosts` ou um escape `\u`, incluindo
    arquivo oculto.
  - Também recusa se a entrada não for arquivo legível.
  - Com DESTDIR, a conferência vale para a pasta do DESTDIR. Assim dá para testar no
    host, sem root.
- **Aviso dos manifestos de usuário.** `avisar_manifestos_de_usuario`, também em
  `chrome.sh`, é chamada no fim do `make install` e do `postinst`.
  - Lista só os nomes dos `*.json` em `~<uid>/.config/google-chrome/NativeMessagingHosts/`
    (não lê nem segue nada) e aponta para o guia.
  - Um `br.ward.nm.json` ali é marcado como suspeito.
- **Baixo: D60 confere mais coisas** (`instalacao/grupos.sh`, `conferir_grupo`):
  - `/etc/security/group.conf` e `group.conf.d/*`: o pam_group dá o grupo no login;
  - `/etc/sudoers`, `/etc/sudoers.d/*` e os `@include`/`@includedir` deles, inclusive
    as formas com `#`, até 512 arquivos (passou disso, recusa). Recusa se algum citar o
    nome do grupo ou `#<gid>`;
  - `subid:` e `sudoers:` no `/etc/nsswitch.conf` com fonte que não seja `files`.
- **Baixo: o uninstall só apaga os grupos que a instalação criou.**
  - A marca fica em `/usr/local/lib/ward/grupos-criados` (0644 root), com uma linha
    `nome gid` por grupo.
  - A pasta da marca é garantida **antes** do `groupadd`, para que um grupo criado
    fique sempre marcado.
  - Um grupo que já existia e confere com D60 não entra na marca, e uma linha com outro
    gid sai.
  - `make uninstall`, `postrm remove|purge` e `postrm abort-install` só apagam grupo
    marcado com o mesmo gid. Os outros ficam, com o aviso "não foi criado pelo Ward".
  - O `postrm` recebe `grupos.sh` e tira a marca e a pasta. O `preinst` cria
    `/usr`, `/usr/local` e `/usr/local/lib` se faltarem; no `ubuntu:24.04` falta
    `/usr/local/lib`.
- **`instalacao/deb.sh`.** As marcas `#@GRUPOS@`, `#@CHROME@` e `#@PASTAS@` agora
  valem nos quatro scripts do mantenedor. Uma marca repetida ou desconhecida recusa.
- **Baixo: `WARD_DEV_DBUS_SISTEMA` só com `modo-teste`.**
  - `trava::caminho_de_teste` virou `caminho_do_barramento`, com erro genérico, porque
    o release também a usa para conferir o caminho fixo.
  - Só o ramo `cfg(feature = "modo-teste")` de `barramento_do_daemon` cita a variável.
  - A mensagem do `main.rs` não cita mais a variável.
  - O `instalar.sh` deixou de ter exceção: qualquer `WARD_DEV_*` recusa. O `wardd` sem
    `modo-teste` tem 0 ocorrências de `WARD_DEV_` (conferido no `teste-install`).
- **Baixo: `Operacao` sem uso sem `modo-teste`** (`app/nucleo/src/daemon.rs`). O import
  saiu, e as duas funções `cfg(any(test, feature = "modo-teste"))` usam
  `protocolo::Operacao`. O build do `partida-conteiner` sem `modo-teste` sai sem aviso.
- **`docs/instalacao.md`:**
  - passos 0.4 e 0.5: listar os manifestos do perfil e as outras políticas;
  - passo 4: `chrome://policy` com `NativeMessagingUserLevelHosts=false`;
  - **4a, "Outros apps com native messaging"**: o efeito da política, o porquê (o
    ataque da auditoria), o risco do `path` do app e a cópia com sudo de um manifesto
    por vez, em quatro partes:
    - cópia para `/root/ward-nm-hosts/`, com o `cat` como usuário e o `tee` como root,
      para que um link para arquivo do root não seja lido;
    - instalação em `/etc/opt/chrome/native-messaging-hosts/` só se for JSON com `name`
      igual ao nome, nunca `br.ward.nm`, e sem sobrescrever;
    - conferência;
    - volta atrás.
  - **4b, "Outras políticas do Chrome"**: como achar, mover e voltar o arquivo;
  - passo 6: a regra nova dos grupos.
- **`CLAUDE.md`:** linha do `make teste-politica-chrome`; a linha do `make install`
  traz a política, o D60 novo e a marca.

## Segurança
- **Invariantes tocadas:**
  - **S1/D9 (V9.5, V9.6).** Só o host do root fala com a extensão; o do perfil é
    ignorado (rodadas 2 e 4).
  - **S1/D47/D60 (V9.11).** O gid do grupo não chega por pam_group, sudo nem subid
    pelo NSS.
  - **D7/D63 (V9.2).** Nenhum `WARD_DEV_*` é aceito no artefato.
  - **V9.10.** O uninstall não apaga grupo do administrador.
- **Testes de ataque** (todos falham do lado do atacante):
  - **`dev/testes/politica-chrome.sh`**: rodadas 2 e 4 (manifesto plantado no perfil e
    o `exec` do `ward-nm` da auditoria). As rodadas 1 e 3 são os controles que mostram
    o ataque funcionando sem a política.
  - **`dev/testes/install.sh`, seção "1b":**
    - "V9.2: wardd com WARD_DEV_DBUS_SISTEMA (sem exceção)";
    - "o wardd de release não tem nenhum WARD_DEV_";
    - "política: outro arquivo com NativeMessagingUserLevelHosts", "… com
      ExtensionSettings", "… nome escondido com escape \u" e "… arquivo oculto também
      conta", nenhum deixando arquivo do Ward;
    - o caso positivo "outro arquivo sem os nomes do Ward não impede" (e fica intacto).
  - **`dev/conteiner-install/dentro.sh`:**
    - V9.11: "pam_group (group.conf) dá o grupo no login", "sudoers.d com sudo -g
      ward-nm", "sudoers por @include com #<gid>", "nsswitch com subid: sss" e
      "nsswitch com sudoers: files ldap", todos sem arquivo nem setgid e com o grupo
      recém-criado apagado;
    - "política conflitante (NativeMessagingUserLevelHosts em outro arquivo)", sem
      arquivo e sem grupo;
    - marca com nome e gid, root 644;
    - "grupo do administrador": aceito, fora da marca e mantido no uninstall com o
      mesmo gid.
  - **`dev/conteiner-deb/dentro.sh`:**
    - "pam_group (group.conf, Fase 51a)";
    - "política conflitante" pelo `preinst` (o outro arquivo fica intacto);
    - política instalada com a chave;
    - marca do `preinst`;
    - "grupo do administrador" mantido no `dpkg -r`.
  - **`dev/testes/deb.sh`:** `preinst` com `conferir_politicas_chrome` chamada,
    `postrm` com `apagar_grupos_marcados`, `postinst` com o aviso, nenhum `#@` nos
    quatro scripts e política do pacote com a chave.
- **Dependências novas:** nenhuma. Os testes usam `bwrap` e `openssl`, que já estão no
  host, e o Chrome em `/opt/google/chrome`.

## Ficou de fora
- **`make check` inteiro.** O disco ficou entre 4,2 e 6,5 GB livres, com outros
  agentes compilando, e o `target/debug` do workspace passa de 17 GB. Rodei só os
  passos que cabem (tabela abaixo). Faltaram o `cargo test` do workspace, o
  `cargo audit` e o `cargo deny`; nenhuma dependência mudou. Faltou também o
  `tests/trava.rs` com `modo-teste`, mas a mensagem que ele confere, "WARD_DEV_DBUS_SISTEMA
  recusado…", continua igual no ramo `modo-teste`, e "logind desligado" continua na
  mensagem do `main.rs`.
- **Binários `modo-teste` usados nos testes.** `wardd`, `ward-nm` e o exemplo
  `ward-plataforma-conteiner` foram copiados do `target/debug` do checkout principal
  (build de 17:01), não compilados neste worktree, pela mesma falta de disco. Eles
  diferem deste ramo só no texto do aviso do `main.rs`. Os binários **sem**
  `modo-teste` (`ward`, `wardd`, `ward-nm`, `wardd-cliente-teste` e
  `partida-conteiner`) foram compilados aqui. O `wardd` do checkout principal seria
  recusado (ainda tem `WARD_DEV_DBUS_SISTEMA`).
- **`make teste-politica-chrome` não foi rodado pelo `make`**: rodei o
  `dev/testes/politica-chrome.sh` direto, com os caminhos do item anterior. O alvo do
  `Makefile` compila tudo e chama o mesmo script.
- **`file://` no force-install (V9.5)** continua sem confirmação; é roteiro do usuário.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo build -p wardd --bin wardd --example wardd-cliente-teste -p ward -p ward-nm --target-dir target/sem-modo-teste` (sem `modo-teste`, 0 `WARD_DEV_` nos binários) | 0 |
| `cargo build -p ward-app-nucleo --example partida-conteiner --target-dir target/sem-modo-teste` (sem aviso) | 0 |
| `dev/testes/politica-chrome.sh …` (1ª: o `wardd` no bwrap recusa `/run/user` do nobody; rodada 3 movida para fora do bwrap) | 1 → 0 |
| `dev/testes/install.sh --so-destdir` | 0 (101 ok) |
| `dev/testes/deb.sh --so-host` | 0 (53 ok) |
| `dev/testes/install.sh` completo, com o contêiner | 0 (232 ok; rodado de novo depois da `pasta_da_marca`: 0, 232 ok) |
| `dev/testes/deb.sh` completo (1ª: `/usr/local/lib` falta no contêiner e a marca não se criava; corrigido no `preinst` e com `pasta_da_marca` antes do `groupadd`) | 1 → 0 (153 ok) |
| `cargo fmt --all -- --check` (1ª: uma linha do `main.rs`; `cargo fmt --all`) | 1 → 0 |
| `cargo clippy -p wardd -p ward-app-nucleo --all-targets -- -D warnings` | 0 |
| `cargo clippy -p wardd --features modo-teste --all-targets -- -D warnings` | 0 |
| `cargo test -p wardd --lib trava::` | 0 (3 ok) |
| `make origens` | 0 |
| `make gitleaks` (binário copiado do checkout principal, conferido pelo sha256 do Makefile) | 0 |
| `make check` inteiro | não rodado (disco; ver Ficou de fora) |

## Problemas descobertos
- **O controle do ataque inteiro (rodada 3) não roda no bwrap.** O `wardd` e o `ward-nm`
  de teste conferem todas as pastas acima do socket (root ou o próprio uid). No
  namespace sem privilégio do bwrap, as pastas do root aparecem como do nobody, e a
  conferência recusa. Por isso a rodada 3 roda fora do bwrap, o que exige um host sem
  `/etc/opt/chrome`. Depois que você instalar o Ward neste host, ela imprime "pulado"
  (sem falhar). As rodadas 1, 2 e 4 continuam valendo.
- **O D60 do sudoers é textual.** Não cobre uma regra que dê `(:ALL)` ou um
  `Runas_Alias` de grupos amplo a um usuário que não é admin, e recusa até menção em
  comentário. Também não cobre outro módulo PAM que dê grupo além do `pam_group`.
- **A conferência de política é textual.** Basta citar `ExtensionSettings` ou
  `NativeMessagingUserLevelHosts` (mesmo num comentário) ou ter um `\u` em qualquer
  lugar para recusar. É de propósito, na dúvida recusa. A desvantagem: quem precisa de
  `ExtensionSettings` para outra extensão não consegue conviver com o Ward. O guia manda
  levar o caso à missão. Decisão do planner/usuário.
- **Marca velha.** Se o administrador apaga um grupo que o Ward criou e cria outro com
  o mesmo nome **e o mesmo gid**, o uninstall o apaga. Com outro gid, fica. Risco baixo.
- **Misturar `make install` e `.deb`.** O `postrm abort-install` apaga os grupos da
  marca, que pode ter vindo de um `make install` anterior. O guia já manda não misturar.
- **`dpkg -r`.** O dpkg deve avisar que `/usr/local/lib/ward` não está vazia (a marca
  não é do pacote). O `postrm` apaga a marca e a pasta, e o teste confere que a pasta
  sai. Não conferi o texto do aviso.
- **Manifestos copiados (guia 4a).** O `path` do app continua onde estava (às vezes no
  home). Esse risco é do app, não do Ward, e o guia o explica. A cópia também é uma
  foto: o app que se atualizar no perfil precisa ser copiado de novo.
- **PRD D18 desatualizado** (já anotado nas 51 e 52).
- **Imagens Docker.** Apaguei as do `teste-install` e do `teste-deb` no fim; elas se
  reconstroem com rede.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `instalacao/chrome.sh` | criado |
| `instalacao/instalar.sh` | alterado (política, sem exceção `WARD_DEV_`, `chrome.sh`, pastas antes dos grupos, aviso, uninstall pela marca) |
| `instalacao/grupos.sh` | alterado (D60: pam_group, sudoers, nsswitch; marca dos grupos) |
| `instalacao/deb.sh` | alterado (marcas nos quatro scripts) |
| `instalacao/deb/preinst` | alterado (`conferir_politicas_chrome`, pastas da marca) |
| `instalacao/deb/postinst` | alterado (aviso dos manifestos) |
| `instalacao/deb/postrm` | alterado (grupos pela marca, também no `abort-install`) |
| `crates/wardd/src/trava.rs` | alterado (`caminho_do_barramento`, nome da variável só no `modo-teste`) |
| `crates/wardd/src/main.rs` | alterado (aviso sem o nome da variável) |
| `app/nucleo/src/daemon.rs` | alterado (`Operacao` sem import solto) |
| `dev/testes/politica-chrome.sh` | criado |
| `dev/testes/politica_chrome.py` | criado |
| `dev/testes/install.sh` | alterado (política, recusas novas, copia `chrome.sh`) |
| `dev/testes/deb.sh` | alterado (scripts e política do pacote) |
| `dev/conteiner-install/dentro.sh` | alterado (D60 novo, política, marca, grupo do administrador) |
| `dev/conteiner-deb/dentro.sh` | alterado (pam_group, política, marca, grupo do administrador) |
| `docs/instalacao.md` | alterado (0.4, 0.5, 4, 4a, 4b, 6) |
| `Makefile` | alterado (`teste-politica-chrome`) |
| `CLAUDE.md` | alterado (linha nova, linha do install) |
| `docs/mission/handoffs/phase-51a.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): só fmt, clippy (pacotes tocados, com e sem `modo-teste`), testes da `trava`, origens e gitleaks; o resto não coube no disco
- [x] Só cofre de teste e credenciais falsas usados (cópia de `.dev/cofre-teste.ward`; nenhum sudo; perfil do Chrome só em `.dev/`)
- [x] Teste de ataque para cada (SEG) tocado (V9.2, V9.5/V9.6 com o Chrome real, V9.10, V9.11)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
