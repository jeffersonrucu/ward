# Fase 57 — Caminhos instalados e atualização (m2:DT15, m2:DT16)

## Implementado
- Caminhos instalados novos (m2:DT15): a extensão vai para `/usr/local/share/ward/extension/`
  (`install/tree.txt`, `install/install.sh`, política do Chrome com
  `file:///usr/local/share/ward/extension/update.xml`) e a marca dos grupos criados vira
  `/usr/local/lib/ward/created-groups` (`install/groups.sh`, `GROUPS_MARK`). A marca não entra
  na árvore; `tree.txt` traz um comentário com o motivo (é gerada na instalação, só existe
  quando a instalação cria grupo).
- Marca antiga: `LEGACY_GROUPS_MARK=/usr/local/lib/ward/grupos-criados`. `marked_groups` lê a
  nova ou, sem ela, a antiga (link ou não-regular no primeiro nome existente: nada lido, não cai
  na outra). `migrate_legacy_mark` passa as linhas válidas da antiga para a nova (a nova vence se
  existirem as duas) e apaga a antiga; roda antes de `ensure_group` no `make install` e no
  preinst. `delete_marked_groups` (uninstall/postrm) apaga as duas.
- `install/upgrade.sh` (novo, também embutido nos scripts do `.deb` pela marca `#@UPGRADE@`):
  - `verify_legacy_paths <raiz>`: a pasta da extensão da 0.1.x (`extensao/` com `ward.crx` e
    `update.xml`) e as pastas acima dela sem link e no formato esperado; senão recusa antes de
    qualquer mudança ("caminho da 0.1.x fora do esperado: …").
  - `remove_legacy_paths <raiz>`: apaga os arquivos e a pasta da 0.1.x sem seguir link
    (avisa e deixa no lugar se achar um); `legacy_paths_left` diz o que sobrou.
  - `stop_running_daemons` (m2:DT16): para os `wardd@*.socket` ativos e depois os
    `wardd@*.service` ativos, confere que nenhum continua no ar e recusa ("nada foi trocado")
    se um stop falha ou um daemon fica. Os sockets param junto para a ativação por socket não
    subir o binário velho no meio da troca.
  - `start_enabled_sockets`: religa os sockets habilitados (links em
    `/etc/systemd/system/*.wants/wardd@<uid>.socket`) e faz `try-restart` de um serviço ainda
    ativo.
- `make install` por cima da 0.1.x: confere caminhos antigos junto com as outras recusas, migra a
  marca, para os `wardd@*` depois de todas as conferências e antes de copiar arquivo, apaga os
  caminhos antigos depois dos arquivos e religa os sockets no fim. `make uninstall` também apaga
  os caminhos da 0.1.x.
- `.deb`: preinst confere caminhos antigos, migra a marca e, por último (depois de toda
  recusa), para os `wardd@*`; o dpkg apaga a pasta antiga do pacote 0.1.4 (o pacote novo não a
  lista); postinst apaga a sobra de um `make install` 0.1.x (sem dono no dpkg), falha se sobrar
  caminho antigo e religa os sockets; postrm e postinst `abort-upgrade` religam os sockets se a
  atualização falhar depois do preinst. `install/deb.sh` recusa pacote com caminho da 0.1.x.
- Seis funções renomeadas (as que o `dev/tests/deb.sh` procura): `garantir_grupo` →
  `ensure_group`, `conferir_grupo` → `verify_group`, `conferir_politicas_chrome` →
  `verify_chrome_policies`, `apagar_grupos_marcados` → `delete_marked_groups`,
  `avisar_manifestos_de_usuario` → `warn_user_manifests`, `avisar_core_pattern` →
  `warn_core_pattern`, nos dois lados. Entradas da Fase 57 tiradas de
  `dev/english-check/frozen.txt`; linhas novas no `docs/glossario.md` (seção 9).
- Testes (escritos; Docker não roda nesta fase, regra 6):
  - `dev/tests/install.sh` (host, sem root): migração da marca, nova vence, leitura da antiga,
    ataques por link nas duas marcas, uninstall apaga as duas, ordem do stop (sockets antes dos
    serviços) com `systemctl` falso, daemon que não para e stop que falha recusam; m2:V5.6
    (DESTDIR com `extension/`, sem caminho antigo, comentário da marca no `tree.txt`, política
    no caminho novo); reinstalação e uninstall por cima de sobras da 0.1.x; recusas (pasta
    antiga ou `usr/local/share/ward` por link, pasta no lugar do `ward.crx` antigo).
  - `dev/container-install/inside.sh`: m2:V5.7 (instalador 0.1.4 da tag com os binários de
    release e o CRX falso da linha de base, cofre e estado 0.1.4, `wardd` 0.1.4 destravado pelo
    cliente v1, vigia em `/proc/<pid>/exe` com controle positivo, `make install` 0.2.0: parado
    antes dos arquivos, binário não trocado com o processo no ar, caminhos antigos fora,
    `created-groups` com os grupos e gids da 0.1.4, 2755, política nova, socket ativo, cofre
    intacto, primeira conexão sobe o 0.2.0, cliente v1 não atendido, destrave migra e grava
    `ward-state 2`) e m2:V5.8 (0.1.4 com `ward-app` do administrador, uninstall 0.2.0 apaga só
    o `ward-nm` da marca antiga, as duas marcas, as pastas, cofre intacto).
  - `dev/container-deb/inside.sh`: m2:V5.7/V5.9 (`.deb` 0.1.4 → `.deb` 0.2.0 com o `wardd`
    0.1.4 destravado: `dpkg -L` sem caminho antigo, vigia, statoverride 2755, socket, 0.2.0 na
    primeira conexão, migração no destrave, `dpkg -r`/`-P` com o cofre migrado intacto); sobras
    de um `make install` 0.1.x apagadas pelo postinst e marca antiga migrada; ataque com a pasta
    antiga por link (preinst recusa antes de parar o daemon, alvo intacto).
  - `dev/tests/deb.sh` (host): versão lida do `Cargo.toml` (não mais fixa em 0.1.4),
    `dpkg -c` sem caminho antigo e com `extension/update.xml`, greps das funções novas no
    preinst/postinst/postrm, marca `created-groups`, política nova; prepara para o contêiner o
    `.deb` 0.1.4 da linha de base, as fixtures e a senha de teste.
  - `dev/tests/legacy-baseline.sh` (novo, incluído pelos dois): confere cada artefato da linha
    de base pelo `.dev/baseline/SHA256SUMS`, lê os nomes da 0.1.4 de
    `dev/fixtures/legacy/install-0.1.4.txt` (novo; os testes não fixam nome antigo no código),
    copia as fixtures e extrai o instalador da tag `v0.1.4` por `git archive`.
  - `dev/tests/legacy_client.py` (novo): cliente do protocolo v1 que manda um quadro da fixture
    `protocol-v1.jsonl` e compara a resposta com outra linha (`same`/`different <chaves>`); não
    imprime a resposta.
- CLAUDE.md: linha do `make install` com os caminhos novos e a atualização; `test-install` e
  `test-deb` citam a Fase 57 e a necessidade de `.dev/baseline/`. Comentários do Makefile.

### Linha de base (`.dev/baseline/`)
No checkout principal ela já existe (`/home/jeffersonoliveira/projetos/cofre/.dev/baseline/`,
Fase 3); no worktree não (está fora do git). Os testes precisam de:
`bin-release/{ward,wardd,ward-nm,partida-conteiner}`, `deb-0.1.4/ward_0.1.4_amd64.deb`,
`deb-0.1.4/ward-fake.crx`, `deb-0.1.4/extension-id.txt` e `SHA256SUMS` (linhas `<sha> ./<caminho>`).
Montagem (como na `docs/mission/handoffs/phase-3.md`): `git worktree add .dev/baseline/src v0.1.4`;
na tag, `cargo build --release --locked -p ward -p wardd -p ward-nm` e o `partida-conteiner`;
`make deb` da tag com `BIN_APP=partida-conteiner` e o CRX falso (`crx_falso.py --versao 0.1.4`);
copiar para as pastas acima e gerar o `SHA256SUMS` com `sha256sum` a partir de `.dev/baseline/`.
Faltando algo, os testes param com "linha de base 0.1.4: …" e apontam para esta seção. A Fase 64
roda os testes de contêiner.

## Segurança
- Invariantes tocadas: D46/D47/D60 e m2:V5.6–V5.9 (V9.x).
  - Recusas de grupo (D60) intactas: só renomeadas; a migração da marca só aceita linhas válidas
    (nome do Ward, gid de sistema) e não segue link.
  - setgid 2755 com o grupo certo conferido depois da atualização (install e deb).
  - Política do Chrome continua com `NativeMessagingUserLevelHosts: false`; só muda o
    `update_url`.
  - m2:DT16: o `wardd@*` (e o socket) para antes de qualquer troca de binário, só depois de
    todas as recusas (instalação recusada não trava o cofre); stop que falha recusa.
  - Nada segue link nos caminhos da 0.1.x (recusa no preinst/`make install`; aviso no uninstall).
  - Nunca toca em `~/.local/share/ward/`: os testes conferem o hash do cofre antes e depois da
    instalação, da atualização e do uninstall; a migração só acontece no destrave pelo daemon.
  - Nenhum `sudo`/`make install` no host; nenhum contêiner rodado nesta fase.
- Testes de ataque escritos:
  - `dev/tests/install.sh` › "m2:DT15 ataque: marca nova por link …", "… marca antiga por link
    não é seguida", "… o alvo do link intacto", "m2:DT16 ataque: daemon que não para recusa",
    "m2:DT16: stop que falha recusa", recusas da pasta antiga por link, `usr/local/share/ward`
    por link e pasta no lugar do `ward.crx`, sobras por link não seguidas no uninstall.
  - `dev/container-install/inside.sh` › "m2:V5.7 (DT16): o binário não foi trocado com o wardd
    0.1.4 no ar" (com controle positivo do vigia), "m2:V5.8: o ward-app do administrador fica".
  - `dev/container-deb/inside.sh` › "m2:V5.7 (DT16): o dpkg não trocou o binário com o wardd
    0.1.4 no ar", "m2:DT15 ataque: pasta da 0.1.x por link: dpkg -i recusado / alvo intacto /
    recusado antes de parar o wardd".
  - Smoke local (scratchpad, sem root, com fakes) do bloco de marcas e stop do
    `dev/tests/install.sh`: 25 ok, 0 falha; e das funções de `upgrade.sh` com link no arquivo e
    na pasta-pai (link não seguido, `verify_legacy_paths` recusa).
- Dependências novas: nenhuma.

## Ficou de fora
- Rodar `make test-install`, `make test-deb` e os testes de host que precisam de root/Docker
  (regra 6; a Fase 64 roda). Nenhuma imagem Docker montada (disco).
- `make check` completo não foi rodado: a fase só mexe em shell/Python de instalação e testes; o
  pedido era `make compile` e `make english-check`.
- Docs de usuário (`docs/instalacao.md`, `docs/migracao.md`) ainda citam os caminhos antigos:
  Fase 59.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make compile` (log `fase57-make-compile.log` no scratchpad) | 0 |
| `make english-check` (log `fase57-english-check.log`) | 0 |
| `bash -n` dos scripts alterados e `ast.parse` do `legacy_client.py` | 0 |
| smoke do bloco de marcas/stop do `dev/tests/install.sh` (`fase57-smoke-marks.log`) | 0 |
| smoke de `install/upgrade.sh` com links (`fase57-smoke-legacy.log`) | 0 |

## Problemas descobertos
- `dev/container-install/inside.sh` (V9.3) e os cenários novos ainda chamam `ward destravar` /
  o cliente mínimo com `estado`/`destravar`; se a Fase 55 renomear os subcomandos/argumentos,
  esses checks precisam acompanhar (risco de virar check vazio).
- Interpretação de m2:V5.7: o `test-install` cobre `make install` 0.1.4 → `make install` 0.2.0 e
  o `test-deb` cobre `.deb` 0.1.4 → `.deb` 0.2.0, mais uma rodada `.deb` sobre sobras de um
  `make install` 0.1.x. Misturar (`make install` 0.1.4 → `.deb` 0.2.0) só está coberto pelas
  sobras plantadas, não pela instalação real da 0.1.4.
- Se a atualização do `.deb` falhar entre o preinst e o postinst, o postrm `abort-upgrade` da
  versão nova religa os sockets; numa falha de `make install` depois do stop (ex.: cópia), os
  sockets ficam parados até rodar a instalação de novo (a mensagem de erro sai, o cofre fica
  travado, nada de segredo exposto).
- `docs/instalacao.md`, `docs/migracao.md` e `docs/escopo.md` citam `/usr/local/share/ward/extensao/`
  e `grupos-criados` (fora do escopo da fase; Fase 59).
- O `.dev/baseline/` não está no worktree (fora do git); os testes de contêiner rodam no
  checkout principal ou com a linha de base copiada.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `install/upgrade.sh` | criado |
| `install/groups.sh` | alterado |
| `install/install.sh` | alterado |
| `install/chrome.sh` | alterado |
| `install/dump.sh` | alterado |
| `install/tree.txt` | alterado |
| `install/deb.sh` | alterado |
| `install/deb/preinst` | alterado |
| `install/deb/postinst` | alterado |
| `install/deb/prerm` | alterado |
| `install/deb/postrm` | alterado |
| `dev/tests/install.sh` | alterado |
| `dev/tests/deb.sh` | alterado |
| `dev/tests/legacy-baseline.sh` | criado |
| `dev/tests/legacy_client.py` | criado |
| `dev/fixtures/legacy/install-0.1.4.txt` | criado |
| `dev/container-install/inside.sh` | alterado |
| `dev/container-deb/inside.sh` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `Makefile` | alterado (comentários) |
| `CLAUDE.md` | alterado |
| `docs/mission/handoffs/phase-57.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não; `make compile` e `make english-check` 0 (regra 6)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (escritos; os de contêiner rodam na Fase 64)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
