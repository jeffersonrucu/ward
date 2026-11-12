# Fase 63 — Bateria C (contêineres e instalação)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard
  59ff683` (master atual, "fase 62 concluída; fase 62a de correção").
- **Onde rodou**: no próprio worktree. As quatro suítes desta fase não usam o
  `real-home.sh`, então o problema do `git check-ignore` (handoff 62) não apareceu, e não
  precisei do clone em `.dev/fase63-repo`.
- **Preparação**: `.dev/tools` copiado do checkout principal (o `make gitleaks-binary`
  conferiu o sha256). Rodei `make dev-services`, `dev-vault` (22 entradas) e
  `dev-variations` (284 padrões). A linha de base 0.1.4 foi copiada do checkout principal
  para `.dev/baseline/`, e o `sha256sum -c SHA256SUMS` passou com todos os arquivos.
- **Bateria C**: rodei `make test-docker-platform`, `test-units`, `test-install` e
  `test-deb` exatamente como estão no commit. Nenhum código foi corrigido.
- **Rodadas de diagnóstico** (fora do commit): para saber o que as falhas de teste
  escondiam, rodei de novo `test-units`, `test-install` e `test-deb` com um patch
  temporário só nos scripts de teste. Ele está salvo em
  `evidence/M6/fase-63/fase63-diagnostic.patch` e foi desfeito com `git checkout` logo
  depois. Os ajustes:
  - cliente com `status`/`list`;
  - controle do vigia em `/var/tmp`, porque o `/tmp` do contêiner é `noexec`;
  - `--cap-add SYS_PTRACE` no contêiner do `test-install`, como o `test-deb` já faz;
  - a mensagem de parada que o `install.sh` imprime de fato;
  - 2 s de espera antes de digitar a senha no `ward unlock`.
- **À mão** (`scripts/manual-run.sh` + `manual-inside.sh`), as partes de V2.6, V2.7 e
  V4.4 que precisam do daemon de release ou de root. Rodaram num contêiner `ubuntu:24.04`
  descartável, com systemd e sem rede. A imagem `ward-teste/fase63-tools:tmp` (a
  `units-systemd:1` mais gdb, strace, inotify-tools e python3) foi apagada no fim. Os
  binários são `wardd` e `ward` de `cargo build --release -p …` (sem `test-mode`).
- **V2.3 (diff de lógica)**: `scripts/v23_logic_diff.py` tira os comentários, troca
  identificadores por `ID` e literais por `STR` e compara a estrutura de tokens entre a
  0.1.4 (`git archive v0.1.4 crates`) e o HEAD. O resultado está em
  `fase63-v23.log`.

### Resultado por asserção

| Asserção | Como | Resultado |
| --- | --- | --- |
| V2.3 (ataque) | `make test-docker-platform` (136 ok) | ok. Com setgid, `ptrace`/`LD_PRELOAD` dão EPERM ou são ignorados (há controle sem setgid). O servidor falso do mesmo uid é recusado antes de qualquer byte, e o `bind` em `/run/ward/1000/` dá EACCES. Também passaram o papel extensão, o pai que não é o Chrome e o Chrome com `LD_PRELOAD` |
| V2.3 (diff) | `v23-pairs.sh` | ok. `role.rs`, `connect.rs`, `hardening.rs` e o `main.rs` do `ward-nm` têm estrutura idêntica à 0.1.4. Em `server.rs`, `socket.rs` e `sys.rs` só mudou formatação (vírgula final e chaves do rustfmt). Em `process.rs` a única mudança está no módulo de teste (laço da 61a). No `lib.rs` do `ward-nm`, as mudanças são do protocolo v2 (DT da Fase 51: `v:2` e a recusa `unknown_version`) e o texto "erro interno do Ward" (já registrado no handoff 51). Literais: só `modo-teste`→`test-mode`, `ward-teste`→`ward-test` e nomes de formatação |
| V2.6 | à mão | ok. `LD_PRELOAD`, `LD_AUDIT` e `LD_LIBRARY_PATH` num drop-in da unit fazem o serviço ficar `failed` sem responder, e o journal diz "variável LD_… definida no ambiente; o processo não sobe…". Rodando: `/proc/<pid>/environ` é do root (não-dumpable; o controle de um processo comum do uid é dele), o uid não lê o `environ`, e `strace -p` e `gdb -p` do mesmo uid falham. Com `kill -SEGV`: limite de core 0 0, nenhum `core*`/`*.crash` no contêiner nem no cwd (`/`), e nenhum arquivo novo do wardd no `/var/crash` do host (o `core_pattern` do host é o apport) |
| V2.7 | à mão (release, root no contêiner) | ok. `ward run acme-pg-dev` em curso, com um neto em outra sessão (`setsid`), e depois `ward lock`: a CLI sai 137 com "o cofre travou; o comando foi encerrado", e o filho e o descendente do `setsid` morrem. O daemon fica no ar (mesmo pid), travado. Dump da memória do `wardd` pelo root (`/proc/<pid>/mem`, `manual_dump.py`): **0 padrões** depois da trava. Com o cofre destravado eram 38 (era esperado). O controle da varredura no filho do `ward run` achou 4. O `ward ssh` aberto não foi testado aqui (contêiner sem rede e sem SSH); a trava com `ward ssh` está no `test-cli` (V4.26), verde na Fase 62 |
| V4.4 | à mão (inotify + `strace -f -e openat,write,rename…` no `wardd` de release, destrave do `vault-0.1.4.ward`) | ok. A migração cria `.cofre.ward.ward-tmp-…`, `backups/.backup.ward-tmp-…` e `.estado.tmp-…`, todos com `O_CREAT\|O_EXCL`, 0600 e rename. Em cada arquivo capturado e em cada um no fim (cofre, versão do backup, estado): nenhum padrão, nenhum nome de tipo em claro (`database`, `banco`, `servidor`, `secure_note`…). Cofre e backup começam com `WARDCOFR`, o estado é `ward-state 2` e o log do strace não tem padrão. Os temporários foram renomeados antes da cópia; o conteúdo deles é o do arquivo final capturado |
| V5.4 | `test-install` (rodada de diagnóstico; na rodada normal estas linhas também passam, antes do ponto em que o script para) | ok. Cada marca nova e antiga (`WARD_DEV_EXTENSION`, `WARD_DEV_EXTENSAO`…; 36 linhas m2:V5.4), o **wardd de teste 0.1.4** de `.dev/baseline/bin-test-mode/`, o não-ELF e o link são recusados com saída 2, "nenhum arquivo do Ward" |
| V5.6 | `test-install` (DESTDIR) | ok. Tem `extension/` com `update.xml`, nenhum caminho da 0.1.x, o `tree.txt` explica a marca `created-groups` e a política aponta para `file:///usr/local/share/ward/extension/update.xml` |
| V5.7 (`make install`) | `test-install` | **falha na rodada normal** (Problemas 2a–2c: o script aborta logo depois do stop). Na rodada de diagnóstico, todos os itens passam: o vigia vê o binário trocado (controle), o wardd 0.1.4 fica no ar destravado e é parado antes da troca, a instalação para antes dos arquivos, os caminhos antigos somem, `created-groups` fica com os grupos e gids da 0.1.4, os binários novos ficam com 2755, o socket religa, nenhum wardd sobe antes da conexão, o cofre fica intacto até destravar, a primeira conexão sobe o 0.2.0, o cliente v1 não é atendido e `ward unlock` migra (`ward-state 2`, 0600) |
| V5.7 (`.deb`) e V5.9 | `test-deb` | **falha na rodada normal** (Problemas 2a, 2c e 3a, que são de teste, e 3b, que é **do produto**). Na rodada de diagnóstico só sobra o 3b. Passam o `dpkg -c` sem caminho antigo, o `dpkg -L` depois da atualização sem caminho antigo, o statoverride 2755, o wardd 0.1.4 parado antes da troca (o vigia com `SYS_PTRACE` funciona), a migração no destrave e `dpkg -r`/`-P` com o cofre migrado intacto |
| V5.8 | `test-install` | **não rodou na rodada normal** (o script abortou antes). Na de diagnóstico: o `ward-app` do administrador fica com o mesmo gid, o uninstall 0.2.0 sobre a 0.1.4 apaga só o `ward-nm` da marca antiga e as duas marcas, e o cofre fica intacto |
| V6.4 (parte C) | as quatro suítes | `test-docker-platform` passa. `test-units` (4 falhas), `test-install` (3 falhas, mais a parte que não rodou) e `test-deb` (7 falhas) **falham**. Dessas, todas as do `test-units` e `test-install` e 6 das 7 do `test-deb` são de teste ou do ambiente do teste. A 7ª do `test-deb` (Problema 3b) é defeito do produto |

## Segurança

- **Invariantes tocadas** (só por execução; nenhum código de produto mudou):
  - S5: papel pelo egid, servidor falso, `bind`, pai do `ward-nm`, setgid 2755 depois da
    atualização.
  - S7: migração atômica, cofre intacto até destravar.
  - S8: `LD_*`, não-dumpable, ptrace, core, trava que mata o descendente do `setsid`,
    memória sem segredo depois da trava, recusa de artefato `test-mode`.
  - S2: arquivos da migração cifrados.
  - S13: journal e strace sem padrão.
- **Testes de ataque escritos** (roteiros à mão, em
  `docs/mission/evidence/M6/fase-63/scripts/`):
  - `manual-inside.sh` › "V2.6 LD_PRELOAD/LD_AUDIT/LD_LIBRARY_PATH: o daemon não
    responde", "V2.6 strace -p do mesmo uid falha", "V2.6 gdb -p do mesmo uid falha",
    "V2.6 kill -SEGV: nenhum core no contêiner", "V2.7 o descendente com setsid morreu",
    "V2.7 dump da memória do wardd depois da trava sem padrão" (com controle positivo no
    filho do `ward run`), "V4.4: … sem padrão / sem nome de tipo em claro / começa com
    WARDCOFR", "V4.4: o strace do wardd … sem padrão";
  - `manual-run.sh` › "V2.6 host: nenhum arquivo novo do wardd em /var/crash";
  - `manual_dump.py` (lê `/proc/<pid>/mem` e imprime só contagens);
  - `v23_logic_diff.py` + `v23-pairs.sh` (V2.3).
- **Evidência sem segredo**: `grep -F -f .dev/variations.txt` em
  `docs/mission/evidence/M6/fase-63/` não acha nada. A senha mestra de teste, que o pty
  ecoou no `fase63-test-deb.log` (Problema 3a), foi trocada por um marcador. Os caminhos
  do home e do worktree viraram `<home>` e `<repo>`. Os scripts leem a senha de teste de
  `dev/fixtures/entries.json`, como o `dev/tests/install.sh`.
- **Root e cofre**: nada de `sudo` nem `make install` no host. Root só dentro dos
  contêineres. O `/usr/local/bin/wardd` instalado (daemon do sistema) ficou no ar e sem
  nenhum contato. Não toquei no cofre real, no `~/.ssh` nem no `~/.aws`.
- **Dependências novas**: nenhuma. A imagem temporária usou pacotes do Ubuntu (gdb,
  strace, inotify-tools e python3) e foi apagada.

## Ficou de fora

- Correções, por regra da fase. Os testes com expectativa antiga e o defeito do `.deb`
  vão para a fase de correção.
- V2.7 com `ward ssh` aberto no contêiner, porque lá não há serviço SSH nem rede. Está
  coberto pelo `test-cli` (V4.26) na Fase 62.
- V4.3, V4.5, V4.7 e o resto da Bateria D: Fase 64.

## Comandos executados

Logs em `docs/mission/evidence/M6/fase-63/`. Rajadas sob `systemd-run --user --scope -p
TasksMax=2000` (1000 nos roteiros à mão). `df -h` antes dos builds: 19 GB livres, 15 GB
no pico.

| Comando | Exit code |
| --- | --- |
| `make gitleaks-binary` (ferramentas copiadas) | 0 |
| `make dev-services && make dev-vault && make dev-variations` (`fase63-prep.log`) | 0 |
| `sha256sum -c SHA256SUMS` em `.dev/baseline/` | 0 |
| `make test-docker-platform` | 0 (136 ok) |
| `make test-units` | **2** (27 ok, 4 FALHOU; Problema 1) |
| `make test-install` | **2** (371 ok, 3 FALHOU e aborto; Problema 2) |
| `make test-deb` | **2** (238 ok, 7 FALHOU; Problemas 2 e 3) |
| `make test-units` com o patch de diagnóstico | 0 (31 ok) |
| `make test-install` com o patch de diagnóstico | 0 (408 ok) |
| `make test-deb` com o patch de diagnóstico | **2** (243 ok, só o Problema 3b) |
| `git checkout --` dos 4 scripts de teste (patch desfeito) | 0 |
| `make origins` + `cargo build --release --locked -p wardd --bin wardd` / `-p ward --bin ward` | 0 |
| `docker build` da imagem temporária `ward-teste/fase63-tools:tmp` | 0 |
| `manual-run.sh target/release/wardd target/release/ward target/no-test-mode/debug/examples/wardd-test-client` | 0 (56 ok) |
| `v23-pairs.sh` | 0 |
| `make english-check PATHS="docs/mission/evidence/M6/fase-63"` | 0 |
| `docker rmi ward-teste/fase63-tools:tmp`; `rm -rf target app/src-tauri/target` e cópias em `.dev/` | 0 |

`make check` não foi rodado: a fase não mudou código e a Bateria A (61a) já o deixou
verde.

## Problemas descobertos

1. **`make test-units`: V4.38 com os argumentos v1 do cliente** (teste com expectativa
   antiga).
   - Saída: `FALHOU: V4.38 ward run -- ward-app: papel agente (esperado 'resposta: status
     papel=agent', veio '')`, mais "lista do agente", "ward-nm: papel agente" e "o daemon
     recusou o papel app ao filho (D63)".
   - Causa: na Fase 60 o `wardd-test-client` passou a aceitar só `status|list|unlock`. Em
     `dev/container-units/inside.sh` (linhas 89, 91 e 93) ainda se passa `estado`/`lista`.
     O cliente sai 2 em silêncio, antes de conectar, e por isso a linha "papel app
     recusado" também não aparece.
   - Com `status`/`list`, as 4 linhas passam (31 ok): o filho do `ward run` recebe o
     papel agente e o journal tem "papel app recusado".
2. **`make test-install` e `make test-deb`: falhas do ambiente e da expectativa do teste
   da Fase 57** (nunca rodaram antes desta fase).
   - a. Controle do vigia: `/opt/inside.sh: line 427: /tmp/v57/sleep: Permission denied`.
     O `--tmpfs /tmp` do Docker é `noexec`. O controle sai `FALHOU` nas duas suítes.
     Correção: copiar o `sleep` para uma pasta com exec (`/var/tmp`) ou montar o tmpfs com
     `exec`.
   - b. **Só no `test-install`**: o contêiner não tem `CAP_SYS_PTRACE`. O wardd 0.1.4 é
     não-dumpable, e por isso:
     - `cmp /proc/<pid>/exe` falha ("m2:V5.7: wardd 0.1.4 no ar … veio ''");
     - o `readlink /proc/<pid>/exe` do `watch_swap` falha em silêncio;
     - o "m2:V5.7 (DT16): o binário não foi trocado com o wardd 0.1.4 no ar" passa **sem
       ver nada**. É um teste cego numa asserção de segurança.
     - Correção: `--cap-add SYS_PTRACE` como no `test-deb`, e um controle do vigia sobre
       um processo não-dumpable.
   - c. Mensagem de parada:
     - o teste procura "parando wardd@1000.service antes de trocar os binários";
     - o `stop_running_daemons` (`install/upgrade.sh`) para primeiro os sockets, e o
       `wardd@.service` tem `Requires=wardd@%i.socket`; o serviço para junto e o laço
       dos serviços não acha nenhum ativo, então essa linha nunca sai;
     - no `test-install`, o `grep` sem resultado com `set -euo pipefail` aborta o
       `inside.sh` ("contêiner: inside.sh saiu 1"), e V5.7 (resto) e V5.8 não rodam;
     - no `test-deb`, o preinst não imprime nenhuma linha de parada.
     - O comportamento está certo: o daemon para antes dos arquivos, e o vigia do
       `test-deb` e o de diagnóstico do `test-install` confirmam. Correção: mudar o teste
       para a mensagem que o `install.sh` imprime ("systemd: parando os wardd@* em
       execução antes de trocar os binários") ou fazer o `upgrade.sh` avisar ao parar o
       socket com o serviço ativo (no `.deb` hoje não sai aviso nenhum).
3. **`make test-deb`**:
   - a. **`ward unlock` com a senha digitada antes do prompt** (teste):
     - `unlock_with_cli` faz `printf senha | script -qec "ward unlock"`. A senha chega ao
       pty antes de a CLI desligar o eco. O pty a ecoa na saída e a CLI descarta o que
       foi digitado antes, então sai `ward: cancelado; nada foi enviado` (saiu=1).
     - Falham "`ward unlock` 0.2.0 destrava o cofre 0.1.4", "sai 0", "o destrave migra" e
       "estado no formato novo". O "dpkg -r/-P deixam o cofre migrado intacto" passou sem
       ter migração.
     - O mesmo padrão está na linha 485 do `dev/container-install/inside.sh`, mas não
       chegou a rodar por causa do 2c.
     - Com 2 s de espera antes de digitar, passa nas duas suítes.
     - Efeito colateral: a senha mestra de teste (falsa) apareceu no log por eco do pty.
       Na evidência, troquei por um marcador.
   - b. **Defeito do produto (DT16): uma instalação recusada reinicia o daemon e trava o
     cofre.**
     - Saída: `FALHOU: m2:DT15 ataque: recusado antes de parar o wardd (esperado '4270',
       veio '4377')`. Na rodada de diagnóstico: '4318' → '4425'.
     - O preinst recusa certo, antes de parar ("link simbólico", alvo intacto). Mesmo
       assim, o pid do `wardd@1000` muda.
     - Causa provável: com o preinst falhando, o dpkg roda o `postrm abort-upgrade` da
       versão nova e o `postinst abort-upgrade` da instalada. Os dois chamam
       `start_enabled_sockets`, que faz `systemctl try-restart` de todo `wardd@*.service`
       ativo. O daemon que ninguém parou é reiniciado e o cofre trava.
     - Isso contraria m2:DT16 / handoff 57 ("só depois de todas as recusas; instalação
       recusada não trava o cofre"). A falha é do lado seguro (trava, nada vaza), então é
       de disponibilidade.
     - Ideia de correção: o preinst deixa uma marca em `/run` quando de fato para; o
       `abort-upgrade` só religa ou reinicia se a marca existir, ou só dá `start` em
       socket inativo, sem `try-restart` em serviço que nunca parou.
4. **V2.6, ressalva de leitura**: o host tem `kernel.yama.ptrace_scope=1`. A falha de
   `strace -p`/`gdb -p` do mesmo uid vem também do Yama, não só do não-dumpable. A prova
   direta do não-dumpable é o `/proc/<pid>/environ` do root, que o uid não lê, com
   controle. Não existe campo "dumpable" no `/proc/<pid>/status`, como o texto do V2.6
   sugere. Também: `suid_dumpable=2` e o `core_pattern` é um pipe para o apport do host.
   O `kill -SEGV` não gerou crash no host.
5. **V2.7, ressalva**: com o cofre destravado, a memória do `wardd` tem 38 padrões
   (esperado). O contrato só exige zero depois da trava, e foi zero.
6. **`test-units`, journal**: depois do `kill -9`, o wardd registra "pasta de temporários
   do ward run indisponível (permission denied)". O contêiner não tem `/run/user/1000`
   (não há logind), então é do ambiente e não do produto. No roteiro à mão criei
   `/run/user/1000` e o `ward run` funcionou.
7. **Imagens**: as suítes recriaram `ward-teste/units-systemd:1`,
   `ward-teste/install-systemd:1` e `ward-teste/deb-systemd:1` (ficam, pela regra).

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `docs/mission/evidence/M6/fase-63/fase63-*.log` (prep, build-release, test-docker-platform, test-units, test-install, test-deb e as 3 de diagnóstico, manual, v23) | criados |
| `docs/mission/evidence/M6/fase-63/fase63-diagnostic.patch` | criado (patch temporário dos testes, não aplicado) |
| `docs/mission/evidence/M6/fase-63/scripts/{manual-run.sh,manual-inside.sh,manual_dump.py,v23_logic_diff.py,v23-pairs.sh}` | criados |
| `docs/mission/handoffs/phase-63.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, porque a fase não mudou código (a 61a deixou verde)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (suítes e roteiros à mão acima)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase:
  - nenhum código corrigido; o patch de diagnóstico foi desfeito;
  - nenhum contêiner meu de pé e a imagem temporária apagada;
  - `target/`, `app/src-tauri/target` e as cópias em `.dev/` apagados;
  - nenhum processo auxiliar; os serviços e contêineres da 62a e os `studiovisual-forja-*`
    não foram tocados.
