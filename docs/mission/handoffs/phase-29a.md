# Fase 29a — App com D47 e D57 (lado setgid do app, lançador do daemon)

## Implementado
- **`ward-plataforma`, `processo.rs` (D57):** saiu o ramo do `systemd --user` de
  `ProcessoPar::classificar`, e com ele `e_gerente_systemd_do_usuario` e a `cmdline` do
  gerente. O papel app agora exige o pai igual ao próprio daemon (pidfd do par, `PPid`
  relido). `Recusa::PaiDoAppNaoPermitido` diz "o pai do app não é o daemon". A
  assinatura de `classificar` não mudou.
- **`ward-plataforma`, `lancador.rs` (novo; D57, D48):**
  - `ambiente_do_app()` monta o ambiente do zero:
    - `XDG_RUNTIME_DIR=/run/user/<uid>`: pasta do uid, 0700, sem link; as pastas de
      cima são do root ou do uid, sem escrita para outros;
    - `WAYLAND_DISPLAY`: o único `wayland-<n>` (só dígitos) que é socket do uid. Se
      não houver nenhum, houver mais de um, ou houver link, arquivo comum ou outro
      dono com esse nome, recusa;
    - `DBUS_SESSION_BUS_ADDRESS=unix:path=<rt>/bus`, socket do uid, sem link;
    - `XDG_SESSION_TYPE` e `GDK_BACKEND` `wayland`;
    - `HOME` do `getpwuid_r`;
    - `PATH=/usr/local/bin:/usr/bin:/bin`;
    - `LANG`/`LC_*` do próprio processo, só com nome de locale
      (`[A-Za-z0-9_.@-]`, até 64, sem `/` nem `..`);
    - as variáveis de D48 fixadas nesta fase (ver Segurança):
      `GST_PLUGIN_SYSTEM_PATH=/usr/lib/x86_64-linux-gnu/gstreamer-1.0`,
      `NO_AT_BRIDGE=1` e `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null`.
  - `conferir_ambiente_do_app(vars)`, do lado do app: recalcula o esperado e exige
    exatamente aquelas variáveis, com aqueles valores. Variável a mais, faltando,
    repetida ou com valor diferente recusa, e a mensagem leva só o nome higienizado.
  - `Lancador::novo(binario, &Instalacao)` / `abrir()`:
    - confere o binário (do root, caminho sem link nem escrita de grupo/outros,
      setgid com o gid de `ward-app`) e uma instância por vez (`JaAberto`);
    - faz `fork` + `execve` sem shell, com o ambiente montado, pasta `/`, 0–2 em
      `/dev/null` e todo fd ≥ 3 com `FD_CLOEXEC` antes do `exec`
      (`close_range(CLOSE_RANGE_CLOEXEC)`, o que mantém o canal de erro do `exec` do
      `std`).
  - `ErroSessao::sessao_indisponivel()` marca os casos em que a Fase 23 responde
    "aprovação indisponível" (D21).
  - Só no `modo-teste`, `lancar_para_teste` (com saída redirecionada, sem conferir o
    binário).
- **`ward-plataforma`, `app.rs` (novo):** operações do lado do app, todas seguras:
  - `largar_grupo()` (`setresgid` real/real/real, conferido) e `gids()`;
  - `dumpable()`/`valor_dumpable()`;
  - `tem_cloexec()`;
  - `esperar_fechamento()`: `poll` com `POLLRDHUP`, sem ler nada;
  - `Pai` (pidfd do pai do começo; `continua_pai()`; `conferir_parece_o_daemon()`:
    `Name: wardd`, os 4 uids iguais e `environ` dando `EACCES`, ou seja, não-dumpable);
  - `reexecutar()`: `exec` de `/proc/self/exe` com o ambiente conferido e fds ≥ 3
    fechados.
- **`ward-plataforma`, `sys.rs`:** `getresgid`, `largar_grupo_efetivo`, `getppid`,
  `tem_cloexec`, `esperar_fechamento`, `home_do_uid` (`getpwuid_r`),
  `fechar_fds_no_exec` (`pre_exec` com `close_range`), `valor_dumpable` e, só nos
  testes, `tirar_cloexec`. Cada `unsafe` tem `// SAFETY:` e teste.
- **`app/nucleo`, `partida.rs` (novo):** `partida::iniciar()`, chamado antes do GTK. A
  ordem:
  1. `PR_SET_DUMPABLE=0` e core 0;
  2. `TracerPid` 0;
  3. ambiente de D57 (fora dele, sai **sem conectar**);
  4. guarda o pai;
  5. conecta com o egid do `ward-app` (D46 inteiro) e confere que o socket tem
     `CLOEXEC`;
  6. larga o grupo;
  7. `PR_SET_DUMPABLE=0` de novo;
  8. reconfere gids, dumpable e depurador, e que o pai parece o daemon.

  `Partida::vigiar(ao_cair)` espera a conexão cair, limpa a área de transferência e
  chama `reexecutar_ou_sair()`: com o mesmo pai, se executa de novo; com outro, sai
  com mensagem. `ErroPartida` tem mensagens fixas.
- **`app/nucleo`, `daemon.rs`:** o `DaemonSocket` guarda a conexão.
  - `conectar_agora()` (a da partida) trava o objeto: ele nunca reconecta, e sem a
    conexão os pedidos respondem `ErroApp::ConexaoPerdida` (código
    `conexao-perdida`). Uma conexão nova teria o gid real e cairia no papel agente.
  - `padrao()`, usado pela ponte de teste e pelos testes, continua conectando no
    primeiro uso.
- **`app/src-tauri/src/main.rs`:**
  - a primeira coisa do `main` é `partida::iniciar()`; se falhar, imprime a mensagem e
    sai 1, sem GTK;
  - o serviço recebe o `DaemonSocket` da partida;
  - a vigia limpa a área de transferência antes de reexecutar;
  - `Estado` virou `Arc`, e o teste de ACL monta o estado com `DaemonSocket::padrao()`.
- **Contêiner (`make teste-docker-plataforma`):**
  - novo app de teste `app/nucleo/examples/partida-conteiner.rs`: a partida real sem
    GTK, com relatório em `/tmp` porque o lançador liga 0–2 a `/dev/null`;
  - subcomandos novos no `ward-plataforma-conteiner`: `--abrir-app`,
    `--abrir-app-com`, `lancar-app`, `escutar`, `ambiente` e `--user` (o gerente
    falso);
  - bloco 29a no `dentro.sh` com 33 conferências novas, 68 no total, todas `ok`;
  - `docker-plataforma.sh` e o alvo do `Makefile` montam os dois binários;
  - a linha do alvo no `CLAUDE.md` foi atualizada.

## Segurança
- **Invariantes tocadas:**
  - **S1/S8 (D47, D57):**
    - o daemon é o único pai aceito;
    - o app conecta antes do GTK, com o egid do setgid, larga o grupo e volta a
      dumpable 0;
    - o app não reconecta no mesmo processo depois de largar o grupo, e por isso se
      reexecuta;
    - o socket tem `CLOEXEC`, e os processos do WebKitGTK não o herdam;
    - o ambiente do app é montado do zero (nada do daemon, nem `LISTEN_*`, nem
      `DISPLAY`) e conferido pelo app antes de conectar;
    - fds ≥ 3 não passam ao app.
  - **D48:** o WebKitGTK real (2.52.6, deste host) foi conferido com um `HOME` falso
    cheio de iscas e `strace -f`. O teste usou o `ward-app` de release da Fase 29,
    com o ambiente de D57 passado à mão (ver Problemas).
    - **Sem `GST_PLUGIN_SYSTEM_PATH`**, o processo web carregou uma biblioteca de
      `~/.local/share/gstreamer-1.0/plugins` (a marca apareceu). Com a variável, não
      carrega, nem com o registro em cache que já listava o plugin.
    - **Sem `NO_AT_BRIDGE`/`WEBKIT_A11Y_BUS_ADDRESS`**, o processo de UI e o processo
      web conectam ao barramento AT-SPI da sessão (`/run/user/1000/at-spi/bus_1`), por
      onde qualquer processo do uid lê o texto da tela, inclusive o valor revelado.
      Com as duas, nenhum dos dois conecta.
    - `gtk-modules=` e `gtk-im-module=ibus` no `~/.config/gtk-3.0/settings.ini` não
      carregaram nada: o GTK no Wayland usou o `im-wayland.so` do sistema, e
      nenhuma `.so` fora de `/usr` abriu.
    - Com o ambiente fixo, o app de release rodou 8 s até o `timeout`, sem erro além
      do aviso de acessibilidade. O ambiente de D57 basta.
  - **S13:** mensagens só com nome de variável (higienizado), nunca o valor ou o
    caminho.
- **Testes de ataque escritos:**
  - `crates/ward-plataforma/src/lancador.rs`:
    - `ataque_dois_sockets_wayland_ou_link_nao_abre` (dois, link para proxy, arquivo
      comum, nenhum; `wayland-1.lock` e nomes parecidos não contam);
    - `ataque_barramento_ou_pasta_da_sessao_trocados` (bus ausente ou link, pasta 0755,
      pasta por link, `/tmp`);
    - `ataque_ambiente_forcado_no_app_e_recusado`:
      - `WAYLAND_DISPLAY` com `/tmp/proxy.sock`, `../x` ou outro `wayland-<n>`;
      - `DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus`, `XDG_RUNTIME_DIR=/tmp/x`, `HOME`
        e `PATH` trocados, `GST_PLUGIN_SYSTEM_PATH`, `NO_AT_BRIDGE=0` e `LANG` com
        caminho;
      - a mais: `GTK_MODULES`, `GTK3_MODULES`, `GTK_PATH`, `GST_PLUGIN_PATH`,
        `GIO_EXTRA_MODULES`, `GIO_MODULE_DIR`, `GDK_PIXBUF_MODULE_FILE`,
        `WEBKIT_INJECTED_BUNDLE_PATH`, `XDG_DATA_DIRS`, `XDG_CONFIG_HOME`, `DISPLAY`,
        `LISTEN_PID`, `LISTEN_FDS` e `LD_PRELOAD`;
      - variável faltando, `WAYLAND_DISPLAY` repetida (a primeira certa) e nome com
        controle;
    - `ataque_lancador_nao_passa_ambiente_nem_fd_do_daemon` (fd sem `CLOEXEC` e o
      ambiente do processo de teste não chegam ao filho; pasta `/`);
    - `ataque_binario_de_usuario_ou_sem_setgid_e_recusado`;
    - `lancar_poe_dev_null_e_fecha_fds`, `locale_so_nome_de_locale`,
      `home_vem_do_passwd`, `mensagens_sem_valor`,
      `ambiente_montado_tem_so_as_variaveis_de_d57`.
  - `crates/ward-plataforma/src/app.rs`: `ataque_pai_que_nao_e_o_daemon_e_recusado`,
    `sem_setgid_largar_grupo_nao_muda_nada`,
    `socket_do_std_tem_cloexec_e_o_fechamento_e_visto`.
  - `app/nucleo/src/partida.rs`:
    `ataque_ambiente_de_fora_nao_chega_a_conectar` (o servidor falso confere que não
    houve conexão) e `mensagem_sem_valor`.
  - `app/nucleo/src/daemon.rs`: `ataque_sem_a_conexao_da_partida_nao_reconecta`. Depois
    da partida, nenhum pedido abre conexão nova. No `modo-teste`, os pedidos usam a
    conexão da partida, e ela tem `CLOEXEC`.
  - `crates/ward-plataforma/tests/identificacao.rs`: o caso "grupo do app com outro
    pai" continua e agora cobre D57.
  - Contêiner (`dev/conteiner-plataforma/dentro.sh`, bloco 29a, V5.31). O daemon de
    teste `wardd` sobe com o ambiente "sujo": `WAYLAND_DISPLAY=/tmp/proxy.sock`,
    `GTK_MODULES`, `GST_PLUGIN_PATH`, `GIO_EXTRA_MODULES`, `DISPLAY`, `DBUS…=/tmp/bus`,
    `WEBKIT_INJECTED_BUNDLE_PATH` e `LISTEN_*`.
    - O app aberto pelo lançador:
      - desde o `exec`: gid `1000,G,G,G`, `dumpable=2`, `TracerPid 0`, fds `0,1,2` em
        `/dev/null`;
      - depois da partida: gid `1000,1000,1000,1000`, `dumpable=0`, `TracerPid 0`,
        sockets com `CLOEXEC`;
      - o ambiente é exatamente a lista de D57, e o daemon deu `papel=app`;
      - com a conexão caída, o app se reexecutou e reconectou com `papel=app`;
      - um segundo `abrir` dá `JaAberto`.
    - Aberto pelo daemon com `WAYLAND_DISPLAY=/tmp/proxy.sock`, `WAYLAND_DISPLAY=../x`,
      `DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/bus`, `XDG_RUNTIME_DIR=/tmp/x` ou
      `GTK_MODULES` forçados: o app sai com a mensagem e **nem conecta**.
    - Aberto por um processo qualquer com o ambiente dele: sai antes de conectar.
    - Aberto por um processo qualquer com o ambiente de D57 copiado: o daemon dá
      `papel=agente motivo=PaiDoAppNaoPermitido` e o app sai com mensagem.
    - Aberto pelo pai que imita o gerente (`Name: systemd`, `PPid: 1`, `cmdline`
      `/usr/lib/systemd/systemd --user`): `papel=agente motivo=PaiDoAppNaoPermitido`,
      e o app sai.
    - Com dois `wayland-<n>`, ou com o `wayland-1` trocado por link para um proxy, o
      lançador não abre (`indisponivel=true`). Com uma cópia do `ward-app` do
      usuário, também não abre (`BinarioNaoConfiavel`).
  - No host, o `ward-app` de release novo com:
    - o ambiente de D57: sai com "o daemon do Ward não está rodando", antes do GTK;
    - `GTK_MODULES` a mais, ou o ambiente da sessão: sai com o nome da variável;
    - `strace`: sai com "processo sob depurador".
- **Dependências novas:** nenhuma. Só `libc` e `std`, que já estavam no crate. O
  `Cargo.lock` da raiz e o do Tauri não mudaram.

## Ficou de fora
- **Operação `abrir_app` e o uso do `Lancador` pelo daemon:** Fase 23, como o plano
  diz. O lançador está pronto e devolve o erro que vira "aprovação indisponível".
  Também fica para ela o app de menu ou de autostart que pede ao daemon a instância
  verdadeira.
- **Teste positivo do app Tauri inteiro aberto pelo lançador com o setgid real:** exige
  instalar o `ward-app` root:`ward-app` 2755 no host (sudo, Fase 51). No contêiner não
  há WebKitGTK nem rede, então a partida foi testada pelo app de teste, que usa o
  mesmo código, sem o GTK.
- **Cgroup do gerente imitado:** o contêiner não delega cgroup. Como a conferência do
  gerente saiu inteira, o cgroup deixou de importar.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make app-check` antes das mudanças (com `PKG_CONFIG_PATH` para os .pc extraídos no scratchpad, R32): confirma o lock do Tauri corrigido na 21b (D60) | 0 |
| `cargo clippy -p ward-plataforma -p ward-app-nucleo --all-features --all-targets -- -D warnings` (e sem features) | 0 |
| `cargo test -p ward-plataforma --all-features` / `cargo test -p ward-app-nucleo` (com e sem `modo-teste`) | 0 |
| `make teste-docker-plataforma` (1ª: 3 falhas do próprio script: pai pid 1 e regex da linha `queda`; corrigido) | 2, 0, 0, 0 |
| `make app-check` depois das mudanças (mesmo `PKG_CONFIG_PATH`) | 0 |
| WebKitGTK real: `ward-app` da Fase 29 com o ambiente de D57, `HOME` falso com iscas e `strace -f` (antes e depois das variáveis de D48) | 124 (`timeout` de 8 s; esperado) |
| `ward-app` de release novo no host: ambiente de D57, `GTK_MODULES` a mais, ambiente da sessão, sob `strace` | 1 (esperado; mensagem certa em cada caso) |
| `make check` | 0 |

## Problemas descobertos
- **Acessibilidade desligada no app (decisão do usuário).** `NO_AT_BRIDGE=1` e
  `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null` tiram o app do barramento AT-SPI.
  Pelo AT-SPI, qualquer processo do mesmo uid leria o texto da tela, inclusive o valor
  revelado. O custo é que leitor de tela não funciona no app. Escolhi segurança.
  Precisa de aceite do usuário e de entrar na lista de D57 do PRD.
- **Lista de D57 no PRD:** o planner precisa acrescentar as três variáveis
  (`GST_PLUGIN_SYSTEM_PATH`, `NO_AT_BRIDGE` e `WEBKIT_A11Y_BUS_ADDRESS`). A pasta do
  GStreamer está fixa no caminho multiarch do Ubuntu/Pop!_OS (x86_64 e aarch64).
- **Conferência do pai pelo app é heurística.** O app confere `Name: wardd`, o mesmo
  uid e o pai não-dumpable. Um processo que se chame `wardd` e chame
  `PR_SET_DUMPABLE=0` passa nessa conferência. Mesmo assim, o daemon dá a ele o papel
  agente (a decisão é do pidfd do daemon), e o app abre só sem papel. Sugestão: quando
  o app usar o protocolo (Fases 23/30), conferir o papel na primeira resposta e sair se
  não for app.
- **App de teste (`modo-teste`) também só abre pelo lançador.** A conferência de
  ambiente não tem exceção de teste. Antes da Fase 23, o app Tauri de teste não abre
  à mão. Um app de teste sem setgid também receberia o papel agente do daemon de teste
  (o papel vem do egid). As Fases 22/23 precisam decidir como o app de teste ganha o
  papel app (por exemplo, a classificação do `modo-teste` aceitar o pai = daemon de
  teste com o gid do usuário), sem levar isso ao release.
- **Mensagem do app vai para `/dev/null`.** O lançador liga 0–2 a `/dev/null` (D57), e
  a mensagem de "não abriu" do app não aparece em lugar nenhum. A Fase 23 deve
  registrar no log do daemon o código de saída do filho.
- **V5.31 fala em "o `status` mostra `Dumpable`"**, mas o `/proc/<pid>/status` não tem
  esse campo. Medi por `prctl(PR_GET_DUMPABLE)` dentro do processo (2 desde o `exec`
  com `suid_dumpable=2`, 0 depois da partida). Sugiro ajustar o texto do contrato.
- **COSMIC deste host:** `/run/user/1000` tem **um** socket `wayland-1` (mais o arquivo
  comum `wayland-1.lock`), conferido só lendo. O "um só" de D57 vale. Os modos são
  `wayland-1` 0775 e `bus` 0666. O lançador confere só tipo e dono, sem exigir 0600.
- **NVIDIA:** com o ambiente de D57, o processo web conectou a `/tmp/nvidia-mps/control`
  (cliente MPS da biblioteca NVIDIA). Em `/tmp`, um processo do mesmo uid pode
  escutar ali. Não fixei `CUDA_MPS_PIPE_DIRECTORY` porque não conheço o efeito.
  Fica para o planner (família de RA8).
- **Dados do usuário que o GTK ainda lê:**
  - `~/.config/gtk-3.0/settings.ini` e o `gtk.css` (tema): podem mudar a aparência;
  - `~/.config/dconf/user`, `~/.cache/gtk-3.0/compose`, os caches do fontconfig e o
    registro do GStreamer.

  São dados, não código: os módulos de `settings.ini` não carregaram. Mudar a
  aparência por tema entra em RA8.
- **RA7:** o `cargo deny` do app avisa `advisory-not-detected` para RUSTSEC-2024-0429.
  O aviso já aparecia antes desta fase. Talvez a exceção nem seja mais necessária;
  vale o validador conferir.
- **Teste do WebKitGTK feito com o binário da Fase 29.** O binário novo exige `HOME`
  igual ao do passwd e sai antes do GTK sem daemon no host. Por isso a carga de
  módulos foi medida com o `ward-app` de release anterior (mesmo WebKitGTK e mesmo
  Tauri), com o ambiente de D57 e um `HOME` falso. O binário novo foi conferido no
  host até a partida.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/lancador.rs` | criado |
| `crates/ward-plataforma/src/app.rs` | criado |
| `crates/ward-plataforma/src/processo.rs` | alterado (sem o ramo do `systemd --user`) |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/endurecimento.rs` | alterado (`nome_para_mensagem` `pub(crate)`) |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/examples/conteiner.rs` | alterado |
| `crates/ward-plataforma/tests/identificacao.rs` | alterado (comentário) |
| `app/nucleo/src/partida.rs` | criado |
| `app/nucleo/examples/partida-conteiner.rs` | criado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado (`ConexaoPerdida`) |
| `app/nucleo/src/lib.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado (bloco 29a) |
| `dev/testes/docker-plataforma.sh` | alterado |
| `Makefile` | alterado (alvo `teste-docker-plataforma`) |
| `CLAUDE.md` | alterado (linha do `teste-docker-plataforma`) |
| `docs/mission/handoffs/phase-29a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks), mais o
  `make app-check` e o `make teste-docker-plataforma`
- [x] Só cofre de teste e credenciais falsas usados. Nenhum cofre foi aberto, nenhum
  `sudo` rodou e nada mudou em `/etc` ou `/run` do host. O `/run/user/1000` foi só
  lido; as pastas de teste ficam em `/run/user/<uid>/ward-*-teste-*` e são apagadas. O
  `HOME` falso ficou no scratchpad.
- [x] Teste de ataque para cada (SEG) tocado (V3.5 forma D47, V3.24 com D57, V5.31)
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase (não toquei em `crates/wardd` nem no protocolo)
