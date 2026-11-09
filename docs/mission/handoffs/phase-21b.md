# Fase 21b — `ward-plataforma` com D46, D47 e D48; `daemon.rs` do app com D46

## Implementado
- **D46, daemon** (`socket.rs`):
  - `escutar_de_listen_fds(caminho_esperado)` exige `LISTEN_PID` igual a este pid
    (decimal simples) e `LISTEN_FDS=1`. O fd 3 é usado uma vez só por processo.
    Primeiro ele é duplicado com `O_CLOEXEC`, sem tomar posse, e a cópia é conferida:
    precisa ser socket (`fstat`), `AF_UNIX`, `SOCK_STREAM` e `SO_ACCEPTCONN`=1, com
    nome igual ao caminho esperado. Depois disso o `fd 3` original é fechado.
  - O caminho é conferido no disco: toda pasta é do root, sem escrita para grupo e
    outros, sem link, e o socket é do euid e 0600.
  - A `Escuta` devolvida não apaga o socket ao sair (`id: Option`).
  - `caminho_socket_sistema()` dá `/run/ward/<euid>/wardd.sock`, e há a constante
    `DIRETORIO_SISTEMA`.
- **D46, cliente** (`servidor.rs`, novo):
  - `ServidorEsperado::sistema()` / `sistema_em(caminho)`: pastas do root, socket
    0600 do próprio uid, endereço do par igual ao caminho e `SO_PEERCRED` do par com
    uid 0.
  - `ServidorEsperado::teste()` / `teste_em()` só existem com `modo-teste`: pasta 0700
    do usuário, pastas de cima do root ou do usuário, socket 0600 e par do mesmo uid.
  - `conectar_servidor` confere antes e depois do `connect` e não escreve nada.
    `conferir_servidor(&UnixStream, &ServidorEsperado)` serve para um socket já
    conectado.
  - O antigo `conferir_servidor` pelo `/proc` saiu.
- **D47** (`papel.rs`, novo; `processo.rs`):
  - `Instalacao::nova(gid_app, gid_extensao, chrome)` recusa:
    - gid 0 ou 65534;
    - os dois gids iguais;
    - gid que este processo já tem (real, efetivo, suplementar ou salvo);
    - gid dentro de qualquer faixa do `/etc/subgid` (linha estragada também recusa).
  - `Instalacao::do_sistema(chrome)` lê `ward-app`/`ward-nm` do `/etc/group`: uma
    linha cada, sem membros. Só no `modo-teste` existe `Instalacao::de_teste`.
  - `PoliticaKernel::ler()` lê `ptrace_scope` e `suid_dumpable`. Passa com scope ≥ 1
    e suid_dumpable 0 ou 2; ilegível recusa. Nos testes há `simulada()`.
  - `ProcessoPar::classificar(&Instalacao, &PoliticaKernel)` dá o papel pelo egid do
    `SO_PEERCRED`:
    - grupo do app: mesmo uid, política, sem depurador e pai igual a este processo
      (o daemon) ou ao `systemd --user`. O gerente é conferido por `Name`, os quatro
      `Uid`, `PPid: 1`, `TracerPid: 0`, `cmdline` exata e o cgroup `init.scope` do
      `user@<uid>.service`, com o pidfd do pai e o `PPid` relido;
    - grupo do host: o mesmo, com o pai Chrome conferido pelo `/proc` (`conferir_pai`);
    - qualquer outro: `Comum { motivo: GrupoComum }`.
  - Novas recusas: `GrupoComum`, `PoliticaKernel` e `PaiDoAppNaoPermitido`.
  - `ClassePar::Comum` passou a `{ motivo }`.
  - `identificar` (exe/environ/maps) continua público só para o pai Chrome e os
    testes; nunca dá papel. Saiu `identificar_host_extensao`.
- **D48** (`endurecimento.rs`):
  - `conferir_ambiente_proprio` recusa qualquer `LD_*` e `MALLOC_*`, mais a lista
    `UNSECURE_ENVVARS` da glibc 2.39: `GCONV_PATH`, `GETCONF_DIR`, `GLIBC_TUNABLES`,
    `HOSTALIASES`, `LOCALDOMAIN`, `LOCPATH`, `MALLOC_TRACE`, `NIS_PATH`, `NLSPATH`,
    `RESOLV_HOST_CONF`, `RES_OPTIONS`, `TMPDIR` e `TZDIR`.
  - `ErroInicio::VariavelInsegura(String)` leva o nome higienizado (`[A-Za-z0-9_]`,
    até 64) e nunca o valor (V3.25).
- **App** (`app/nucleo/src/daemon.rs`):
  - `DaemonSocket` usa `ward_plataforma::conectar_servidor`: `/run/ward/<uid>/wardd.sock`
    pela regra do sistema; com `modo-teste`, `/run/user/<uid>/ward-teste/wardd.sock`
    pela regra de teste.
  - A API mudou: `em(caminho, uid)` virou `sistema_em(caminho)`, mais `teste_em` só no
    `modo-teste`. Ausente ou recusado na conexão dá `DaemonIndisponivel`; qualquer
    outra recusa dá `DaemonNaoConfiavel`.
  - O `modo-teste` do núcleo liga o de `ward-plataforma`.
- **Contêiner** (`make teste-docker-plataforma`):
  - Usa `ubuntu:24.04` fixado por digest, sem rede, com root só dentro dele.
  - Monta somente leitura uma pasta temporária em `.dev/` com o exemplo
    `ward-plataforma-conteiner` (feature `modo-teste`) e a `libmarca.so`, compilada no
    host pelo gcc a partir de `dev/conteiner-plataforma/marca.c`.
  - O `dentro.sh` faz 34 conferências: cria os grupos, instala `ward-app`/`ward-nm`
    root:grupo 2755, cria `/run/ward/1000` root:root 0755, e o `ativar` faz o papel
    do systemd com `setpriv` para o uid 1000 e `LISTEN_FDS`.
  - Fica fora do `make check`. A linha do alvo está no `CLAUDE.md`.

## Segurança
- Invariantes tocadas:
  - **S5 (D46)**:
    - o cliente só fala com socket em pasta do root e par uid 0. Um servidor falso do
      mesmo uid é recusado antes de qualquer byte;
    - o daemon só aceita o fd do systemd no caminho e na pasta certos;
    - o socket que escuta fica com `O_CLOEXEC`, então os filhos do `ward run` não o
      herdam.
  - **S1/S8 (D47)**:
    - o papel sai do egid que o kernel fixa;
    - setgid deixa o binário não-dumpable e com `AT_SECURE` desde o `exec`;
    - a política fraca do kernel desliga os papéis;
    - um gid que o usuário consegue assumir (suplementar, subgid) é recusado na
      configuração.
  - **S8 (D48)**: lista insegura da glibc. **S13**: mensagens só com nome, nunca valor
    nem caminho do cliente.
- Testes de ataque escritos:
  - `tests/identificacao.rs`:
    - `ataque_servidor_falso_do_mesmo_uid_e_recusado_antes_de_qualquer_byte` (V3.23,
      servidor dumpable e não-dumpable);
    - `ataque_servidor_de_teste_com_socket_aberto_ou_endereco_trocado`;
    - `ataque_ptrace_scope_0_ou_suid_dumpable_1_desliga_os_papeis` (V3.24);
    - `ataque_namespace_de_usuario_nao_forja_o_grupo` (V3.24, `unshare --user
      --map-root-user`);
    - `host_da_extensao_exige_o_pai_permitido`: inclui o pai do app que não é o daemon;
    - `ataque_cliente_sob_strace`: inclui o grupo do app sob depurador;
    - `binario_instalado_e_identificado`: inclui papel app com o teste como "daemon" e
      outro gid como agente;
    - os ataques da Fase 21 pelo `/proc` continuam.
  - `tests/ativacao.rs`, só com `modo-teste`:
    - `ataque_systemd_falso_do_usuario_com_socket_na_pasta_dele`;
    - `ataque_socket_em_outro_caminho`;
    - `ataque_listen_pid_e_listen_fds_errados`;
    - `ataque_mais_de_um_fd`.
  - `tests/endurecimento.rs` › `ataque_subir_com_lista_insegura_da_glibc_recusa`
    (V3.25: `GLIBC_TUNABLES`, `GCONV_PATH`, `LD_` inventado, `MALLOC_ARENA_MAX` e
    `LOCPATH`; o nome aparece na mensagem e o valor não).
  - `src/papel.rs`:
    - `ataque_grupo_com_membro_repetido_ou_estragado`;
    - `ataque_gid_dentro_de_faixa_do_subgid`;
    - `ataque_gid_que_este_processo_ja_tem`;
    - `politica_do_kernel`.
  - `src/socket.rs`: `ataque_pasta_do_usuario_nao_passa_por_pasta_do_sistema` (com
    socket conectado e arquivo comum no lugar do fd em escuta) e
    `pastas_do_root_sao_aceitas_e_link_nao`. `src/servidor.rs`:
    `sistema_so_aceita_uid_0` e `caminho_invalido_recusa`.
  - `src/endurecimento.rs`: `ataque_lista_insegura_da_glibc` e
    `ataque_nome_com_controle_nao_vai_cru_para_a_mensagem`.
  - `app/nucleo/src/daemon.rs`:
    - `ataque_servidor_falso_do_mesmo_uid_e_recusado_sem_receber_nada` (V3.23: o
      servidor falso confere que nenhum byte chegou);
    - os de antes (pasta aberta, link, arquivo no lugar do socket) passam a valer nos
      dois builds, mais `ataque_socket_aberto_para_outros_e_recusado`.
  - Contêiner (`dev/conteiner-plataforma/dentro.sh`):
    - V3.23: o `wardd` de teste aceita o socket do root por `LISTEN_FDS`, com o fd 3
      fechado e `CLOEXEC`, e o cliente aceita o par uid 0. Um servidor falso do uid
      1000 em `/run/user/1000` é recusado (`PastaNaoConfiavel`). O uid 1000 mata o
      wardd e o `bind` em `/run/ward/1000` dá EACCES; ele também não apaga o socket.
    - V3.24:
      - o `ward-app` aberto pelo daemon fica app e o lançado por outro processo fica
        agente;
      - o `ward-nm` com o "Chrome" de pai fica extensão, e sem ele fica agente;
      - com `LD_PRELOAD` não aparece marca no setgid; o controle sem setgid grava a
        marca;
      - `PTRACE_ATTACH`/`PTRACE_SEIZE` dão EPERM e `/proc/<pid>/mem` dá EACCES; no
        controle sem setgid, o attach funciona;
      - com `PTRACE_TRACEME` antes do `exec`, o kernel não aplica o setgid e o
        processo fica agente;
      - o uid 1000 com outro gid fica agente.
    - V3.5 (forma de D47): a cópia do `ward-app` perde o setgid e fica agente.
- Dependências novas: nenhuma crate de fora.
  - `app/nucleo` passa a depender de `ward-plataforma`, que é do workspace e usa
    `libc`/`zeroize`, já presentes. Motivo: uma única implementação da conferência do
    servidor, a mesma da CLI.
  - O `Cargo.lock` da raiz e o de `app/src-tauri` ganham só a ligação. O lock do
    Tauri, atualizado por `cargo metadata --offline`, também ganhou `base64ct` sob
    `ward-core`, uma deriva de uma fase anterior que ele não tinha.
  - O teste de contêiner usa o gcc do host só para a biblioteca de ataque.

## Ficou de fora
- **Lado setgid do app** (conectar antes do GTK, largar o grupo, lista branca de
  ambiente, reexecutar): Fase 29a, por decisão do orchestrator. Também não há
  `largar_grupo()` na plataforma.
- **Ligação no `wardd`** (`main` com `escutar_de_listen_fds(&caminho_socket_sistema())`,
  `Instalacao::do_sistema`, `PoliticaKernel::ler`, `classificar`): Fase 22.
- **Units do systemd e instalação de grupos e binários**: Fases 24 e 51. O `dentro.sh`
  reproduz à mão o que elas vão instalar.
- **Teste positivo de `e_gerente_systemd_do_usuario`** contra o `systemd --user` real:
  depende da sessão gráfica e não é automatizado. Só o negativo está coberto (bash de
  pai).
- **`make app-check`**: não rodou, porque falta `libwebkit2gtk-4.1-dev` (R32). O
  `main.rs` do Tauri só usa `DaemonSocket::padrao()`, que não mudou de forma.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `git merge master` (plano novo e `app/`) | 0 |
| `cargo test -p ward-plataforma --all-features` / sem features | 0 / 0 |
| `cargo clippy -p ward-plataforma -p ward-app-nucleo --all-targets -- -D warnings` (sem features) | 0 |
| `cargo test -p ward-app-nucleo` / `--features modo-teste` | 0 / 0 |
| `make teste-docker-plataforma` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **O pai `systemd --user` não protege o ambiente do app (D47). Decide o planner.**
  - O gerente do usuário roda as units de `~/.config/systemd/user/`, que o próprio
    usuário (e um pacote malicioso) pode escrever, e aceita `systemctl --user
    set-environment`.
  - Um pacote cria `evil.service` com `ExecStart=/usr/local/bin/ward-app` e
    `Environment=WAYLAND_DISPLAY=<proxy>` (ou `DBUS_SESSION_BUS_ADDRESS`). O pai do app
    passa a ser o `systemd --user` de verdade e o app recebe o papel.
  - As duas variáveis estão na lista branca de D47. Um compositor-proxy veria o PIN
    digitado e o valor revelado.
  - A conferência do gerente também é forjável: `cmdline` e cgroup delegado ao usuário.
    O gerente não é dumpable, então o `exe` dele não se lê.
  - Opções: aceitar só o pai = daemon (o daemon abre o app); ou o app fixar
    `WAYLAND_DISPLAY`/barramento no caminho padrão de `/run/user/<uid>` e conferir o
    dono do socket; ou aceitar no escopo.
- **`suid_dumpable=2` (o padrão do Pop!_OS) deixa o setgid com dumpable 2, não 0.**
  - O ptrace e o `/proc/<pid>/mem` continuam negados (EPERM/EACCES, conferido no
    contêiner), porque 2 = `SUID_DUMP_ROOT`.
  - A V5.31 diz "`Dumpable` segue 0", o que só vale depois que o app chama
    `PR_SET_DUMPABLE=0` (Fase 29a). O contêiner confere "≠ 1".
- **V3.25 × handoff da Fase 21.** A 21 escondia o nome de `LD_*` inventado ("`LD_*`").
  A V3.25 pede o nome na mensagem, e segui o contrato: o nome vai higienizado e sem o
  valor. A `Recusa::VariavelLd` do pai Chrome continua com "`LD_*`".
- **`TMPDIR` está na lista insegura da glibc**, e o daemon recusa subir com ele.
  - A unit não define `TMPDIR`. Já o `make dev-daemon` (Fase 22) precisa subir o daemon
    de teste com o ambiente limpo (`env -i` ou lista), senão quem tiver `TMPDIR` na
    sessão vê o daemon de teste recusar.
- **`LISTEN_PID`/`LISTEN_FDS` continuam no ambiente do daemon.** O Rust 2024 exige
  `unsafe` para `remove_var`. A Fase 22/26 deve montar o ambiente de cada filho sem
  `LISTEN_*`. O fd já tem `CLOEXEC`.
- **Instalação (Fase 51).** Os grupos precisam de senha travada no `gshadow` (`!`, o
  padrão do `groupadd`), porque um grupo com senha deixa qualquer um entrar por
  `newgrp`.
  - O daemon não lê o `gshadow` (é do root). Ele só confere membros no `/etc/group` e
    a faixa do `/etc/subgid`.
  - A instalação também tem de escolher gids de sistema (< 1000), fora das faixas do
    subgid.
- **`ptrace` "antes da primeira linha do `main`, com `SIGSTOP`" (V3.24).** Implementei
  as duas metades do caso:
  - o pai se prende logo depois do `exec`, com o filho parado antes de conectar, e
    recebe EPERM;
  - o pai está preso desde antes do `exec` (`PTRACE_TRACEME`), e o kernel não aplica o
    setgid, então o processo fica agente.
  - Entre `exec` e `main` não há como parar o setgid sem ptrace. Se o validador quiser
    a forma literal, o contrato precisa dizer como.
- **O teste de namespace de usuário (host) exige userns sem privilégio.** Num Ubuntu
  com `kernel.apparmor_restrict_unprivileged_userns=1` ele falharia. No Pop!_OS deste
  host a opção não existe.
- **O lock do Tauri estava atrás** (`base64ct` em `ward-core`, de uma fase anterior);
  foi corrigido junto, mas o `make app-check` não confirmou (R32).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/Cargo.toml` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/socket.rs` | alterado |
| `crates/ward-plataforma/src/processo.rs` | alterado |
| `crates/ward-plataforma/src/endurecimento.rs` | alterado |
| `crates/ward-plataforma/src/papel.rs` | criado |
| `crates/ward-plataforma/src/servidor.rs` | criado |
| `crates/ward-plataforma/src/teste.rs` | criado |
| `crates/ward-plataforma/examples/conteiner.rs` | criado |
| `crates/ward-plataforma/tests/ativacao.rs` | criado |
| `crates/ward-plataforma/tests/identificacao.rs` | alterado |
| `crates/ward-plataforma/tests/endurecimento.rs` | alterado |
| `app/nucleo/Cargo.toml` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/src-tauri/Cargo.lock` | alterado |
| `Cargo.lock` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | criado |
| `dev/conteiner-plataforma/marca.c` | criado |
| `dev/testes/docker-plataforma.sh` | criado |
| `Makefile` | alterado (alvo `teste-docker-plataforma`) |
| `CLAUDE.md` | alterado (só a linha do alvo novo) |
| `docs/mission/handoffs/phase-21b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados. Nenhum cofre foi aberto, nenhum
  `sudo` rodou e nada foi feito em `/etc` ou `/run` do host. As pastas de teste ficam em
  `/run/user/<uid>/ward-*-teste-*` e são apagadas.
- [x] Teste de ataque para cada (SEG) tocado (V3.5 forma D47, V3.23, V3.24, V3.25)
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase (não toquei em `ward-core`, `dev/variacoes.py` nem
  `dev/testes/dev-cofre.sh`)
