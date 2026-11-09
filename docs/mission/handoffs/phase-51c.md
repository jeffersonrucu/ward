# Fase 51c — Core dump dos auxiliares do WebKitGTK (S2/S8)

## Implementado
- `instalacao/despejo.sh` (novo, compartilhado entre o `instalar.sh` e o `postinst` do `.deb`):
  - `pasta_webkit`: acha a pasta dos auxiliares (`/usr/lib/<multiarch>/webkit2gtk-4.1`) pela arquitetura do dpkg. Arquitetura fora da tabela faz a instalação recusar.
  - `lista_apport`: monta a lista do apport com `ward`, `wardd`, `ward-app`, `ward-nm`, `WebKitWebProcess`, `WebKitNetworkProcess` e `WebKitGPUProcess`. Se o `libwebkit2gtk-4.1-0` estiver instalado, a lista é conferida contra o `dpkg-query -L` dele. Um auxiliar `WebKit*Process` que o Ward ainda não conhece faz a instalação recusar (falha fechada). Sem o pacote, sai um aviso e valem os caminhos padrão.
  - `avisar_core_pattern`: avisa quando o `kernel.core_pattern` manda o core para outro programa (pipe ou socket) que não seja o apport nem o systemd-coredump.
- `instalacao/arvore.txt`: entram as pastas comuns (D) `etc/apport`, `etc/apport/report-ignore`, `etc/apport/blacklist.d`, `etc/systemd` e `etc/systemd/coredump.conf.d`. Entram também os arquivos `etc/apport/report-ignore/ward`, `etc/apport/blacklist.d/ward` (ambos `ger:apport`) e `etc/systemd/coredump.conf.d/ward.conf` (`src:`).
  - O apport 2.28 lê as duas pastas. Os anteriores leem só o nome antigo, `blacklist.d`, que é o que o plano cita.
  - As pastas são criadas mesmo sem o apport ou o systemd-coredump, para valerem se um deles for instalado depois. O uninstall e o `dpkg -r` tiram só os arquivos.
- `instalacao/coredump.conf.d/ward.conf`: `Storage=none` e `ProcessSizeMax=0`.
- `instalar.sh`: inclui o `despejo.sh` e gera `ger-apport` antes de qualquer mudança. No sistema, chama `avisar_core_pattern`.
- `.deb`: o `deb.sh` ganhou a marca `#@DESPEJO@`. O `postinst` define `aviso` e chama `avisar_core_pattern`. A lista e o drop-in vêm do staging.
- `docs/escopo.md`: corrigido o parágrafo que dizia que os auxiliares do WebKitGTK ficam protegidos só pelo Yama. Agora explica o core dump pelo apport e pelo systemd-coredump e o que a instalação faz.
- `docs/instalacao.md`: seção nova "3a. Core dump" (o que muda, o efeito no sistema inteiro e como conferir) e uma nota no "Desinstalar".

### Avaliação do "só os processos do app" (pedido do plano)
Provado no host com um processo falso meu (`/usr/bin/sleep`); os `.crash` gerados foram apagados:
1. Com `prlimit --core=0:0` e `kill -SEGV`, o apport grava `/var/crash/_usr_bin_sleep.1000.crash`, dono o usuário. É a falha da auditoria: o apport ignora o RLIMIT_CORE para o relatório.
2. Com `--core=1:1`, o kernel 7.2 aborta e não chama o pipe (caso especial do limite 1). Nenhum relatório.
3. O mesmo `1:1`, mas o "atacante" do mesmo uid roda `prlimit --pid <p> --core=0:1`: o relatório volta. O auxiliar do WebKit tem as mesmas credenciais do usuário, então o `prlimit(2)` é permitido. O `coredump_filter` em `/proc/<pid>` também é gravável pelo dono.

Conclusão: não há forma só por processo que resista a um processo do mesmo uid. A lista do apport é necessária. Para o systemd-coredump, o RLIMIT_CORE 0/0 do app também não basta: o `/usr/lib/sysctl.d/50-coredump.conf` do Ubuntu 24.04 (systemd 255.4-1ubuntu8.18) passa `9223372036854775808` no lugar de `%c`. Conferi na imagem do pacote. Ele não tem lista por programa, então o drop-in desliga o armazenamento para o sistema inteiro. Por isso o RLIMIT_CORE do app ficou como está (0/0); mudar para 1/1 não dá ganho contra um atacante ativo.

## Segurança
- **Invariantes tocadas:**
  - S2: a memória do app não vai para disco por `/var/crash` nem por `/var/lib/systemd/coredump`.
  - S8: um processo do mesmo uid não lê a memória do WebKit pelo core.
  - V9.1, V9.9 e V9.10: a árvore, o `.deb` e o uninstall incluem os arquivos novos.
- **Testes de ataque:**
  - `dev/testes/install.sh` (DESTDIR, host):
    - `51c: o apport (report-ignore|blacklist.d) cala os 7 caminhos e não os controles`: usa o `Report.check_ignored()` real do apport do host, por `dev/testes/apport_lista.py`.
    - `51c ataque: auxiliar novo no pacote (WebKitPrintProcess) recusa a lista`.
    - `51c: arquitetura fora da tabela recusa`.
    - `51c: etc/apport/report-ignore por link` e `etc/systemd/coredump.conf.d por link`: recusa sem escrever no alvo.
    - `core_pattern` de outro programa ou socket avisa.
  - `dev/conteiner-install/dentro.sh`:
    - instalação sem apport e sem libwebkit: avisa e cria a lista;
    - `systemd-analyze cat-config` lê o drop-in (`Storage=none`, `ProcessSizeMax=0`);
    - depois do uninstall, o drop-in sumiu.
  - `dev/conteiner-deb/dentro.sh`:
    - o apport e o systemd-coredump reais são chamados à mão com um core falso de um `sleep` do uid 1000;
    - **controle sem o Ward:** a falha da auditoria reproduzida (relatório do `WebKitWebProcess`, dono o usuário, mesmo com `-c0`; core guardado pelo systemd-coredump);
    - **com o `.deb`:** nenhum relatório para cada auxiliar do `dpkg -L` nem para o `ward-app`, nenhum core guardado, e o `/usr/bin/sleep` ainda gera relatório (a lista não cala tudo);
    - **depois do `dpkg -r`:** os arquivos saem, o README do apport fica e os dois voltam a gravar.
  - Antes de tudo, o teste confere que o `/proc/sys` do contêiner é só leitura. O `core_pattern` é do host e nunca é escrito.
- **Dependências novas:** nenhuma no código. A imagem `ward-teste/deb-systemd:1` passa a ter:
  - o `apport` instalado, com as units mascaradas;
  - o `systemd-coredump` só extraído em `/opt/coredump` (sem postinst, para não mexer no `core_pattern`), com o usuário dele criado pelo `systemd-sysusers`.
  - O `teste-deb` ganhou `--cap-add SYS_PTRACE`, só para o apport, que roda como root no contêiner, ler `/proc/<pid>/exe`.

## Ficou de fora
- RLIMIT_CORE 1/1 no app (ver a avaliação acima): o atacante do mesmo uid derruba. Não mexi no `partida.rs`, que também é da 39f.
- Teste com o `WebKitWebProcess` de verdade crashando. O host não pode (regra da fase), e no contêiner o `core_pattern` é do host. O teste usa o `ExecutablePath` real do auxiliar com um processo falso, o mesmo caminho de código do apport.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-cofre` | 0 |
| `make teste-install` | 0 |
| `make teste-deb` | 0 |
| `make check` (0/6–5/6: origens, fmt, clippy, test, audit, deny) | 0 até o 6/6; o 6/6 parou por falta do gitleaks no `.dev/` do worktree |
| `make gitleaks` (6/6, com o binário copiado do checkout principal e o sha256 conferido pelo Makefile) | 0 |

## Problemas descobertos
- **Efeito no sistema inteiro (decisão do usuário):** `Storage=none` + `ProcessSizeMax=0` desliga o armazenamento de core de qualquer programa enquanto o Ward estiver instalado, se o systemd-coredump estiver presente (hoje não está no host). A lista do apport também cala os crashes do WebKitGTK 4.1 de outros apps. Está documentado em `docs/instalacao.md` › 3a.
- **Chrome:** o crash do Chrome vai para o apport (há `/var/crash/_opt_google_chrome_chrome.1000.crash` no host, do usuário). O renderer guarda a senha preenchida pela extensão. É o mesmo tipo de vazamento, fora da fase. Deixei registrado no escopo como "o crash do Chrome continua indo para o apport". Pôr `/opt/google/chrome/chrome` na lista calaria os relatórios do navegador do usuário: decisão para o usuário ou o planner.
- `CLAUDE.md`: as linhas do `teste-install` e do `teste-deb` deviam citar a Fase 51c (apport e systemd-coredump na imagem, CAP_SYS_PTRACE). Não editei; é do orchestrator.
- O contrato não tem asserção V9.x própria para o core dump. Sugiro uma ("crash de auxiliar do WebKitGTK não gera `/var/crash` nem core guardado").

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `instalacao/despejo.sh` | criado |
| `instalacao/coredump.conf.d/ward.conf` | criado |
| `dev/testes/apport_lista.py` | criado |
| `instalacao/arvore.txt` | alterado |
| `instalacao/instalar.sh` | alterado |
| `instalacao/deb.sh` | alterado |
| `instalacao/deb/postinst` | alterado |
| `dev/testes/install.sh` | alterado |
| `dev/testes/deb.sh` | alterado |
| `dev/conteiner-install/dentro.sh` | alterado |
| `dev/conteiner-deb/dentro.sh` | alterado |
| `dev/conteiner-deb/Dockerfile` | alterado |
| `docs/escopo.md` | alterado |
| `docs/instalacao.md` | alterado |
| `docs/mission/handoffs/phase-51c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
