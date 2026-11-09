# Fase 24 — Trava automática (logind e inatividade) e units de teste do systemd

## Implementado
- **`crates/wardd/src/trava.rs`** (módulo novo):
  - **Inatividade.** `venceu(ultimo, agora, minutos)` (função pura) e `conta_como_uso(op, resposta)`.
    - Conta como uso toda operação que deu certo, menos `estado`, `travar` e `destravar`. O destravar que abre já marca o início; com o cofre aberto, ele responde `ok` sem olhar a senha.
    - Erro não conta como uso.
    - Relógio ilegível, relógio que volta ou conta que estoura: trava (falha fechada).
  - `vigiar_inatividade(daemon, 10 s)`: thread que chama `Daemon::travar_se_inativo`.
  - **logind** no barramento do **sistema** (o `wardd` é serviço do sistema, D46). `vigiar_logind(daemon, socket, pronto)` é uma thread com uma `MatchRule` só: sinais de `org.freedesktop.login1` em `path_namespace=/org/freedesktop/login1`. `evento_do_sinal` lê cada sinal:
    - `Session.Lock` (`loginctl lock-session`) trava;
    - `PropertiesChanged` de sessão com `LockedHint` verdadeiro, invalidado, de tipo estranho ou com corpo ilegível trava;
    - `Manager.PrepareForSleep(true)` trava. O daemon segura um inibidor `delay` de `sleep` (`Inhibit`) e só fecha o fd **depois** de travar. `PrepareForSleep(false)` pega outro inibidor.
  - O que vem do logind só trava. Qualquer sessão trava, e não só as do uid; isso fica registrado em "Problemas".
  - Conexão perdida depois de aberta: trava e reconecta com espera de 1 s a 60 s. Sem barramento desde a partida: um aviso só, e a inatividade continua valendo.
  - **Endereço.** Release: sempre `/run/dbus/system_bus_socket`, nunca tirado do ambiente. `modo-teste`: só `WARD_DEV_DBUS_SISTEMA`, conferida por `caminho_de_teste`. Ela precisa ser absoluta, sem `.`/`..`/`//`, com até 107 bytes e só `[A-Za-z0-9/._-]`, porque vai para o endereço D-Bus sem escape. Sem a variável, o daemon de teste não liga o logind, e assim nunca toca o logind real sem pedido.
- **`nucleo.rs`** (mudança mínima):
  - `Relogio::agora() -> Option<Duration>`, com padrão `ward_plataforma::tempo_desde_boot()`. Os relógios de teste que já existiam continuam compilando.
  - `Estado.ultimo_uso`: zera no `travar`. É marcado no destravar e no `criar_cofre`, e depois de cada uso em `atender`.
  - `Daemon::travar_se_inativo()` lê o `minutos_trava()` da configuração do cofre (D30). Os dois relógios são lidos com o estado travado no mutex: um uso de outra thread nunca fica "no futuro".
  - O contador do D20 (`falhas_senha`) não é tocado (D62).
- **`main.rs`**: liga as duas threads depois de criar o `Daemon`. Se não der para criar uma delas, o daemon sai com código ≠ 0. `WARD_DEV_DBUS_SISTEMA` inválida também faz sair ≠ 0. `servidor.rs` não mudou.
- **`ward-plataforma`**: `relogio::tempo_desde_boot()` sobre `sys::relogio_boot()` (`clock_gettime(CLOCK_BOOTTIME)`, com `// SAFETY:`). Esse relógio anda durante a suspensão, ao contrário do `Instant`. Uma suspensão que o `PrepareForSleep` não pegou continua contando como tempo sem uso.
- **Units de teste** `dev/systemd/wardd@.socket` e `dev/systemd/wardd@.service`:
  - `.socket`: `ListenStream=/run/ward/%i/wardd.sock`, `SocketUser=%i`, `SocketMode=0600`, `DirectoryMode=0755`, `Accept=no`.
  - `.service`: `User=%i`, `ExecStart=/usr/local/bin/wardd`, `UMask=0077`, `LimitCORE=0`, `LimitMEMLOCK=infinity` (D62) e `Restart=on-failure`.
  - Sem `Environment*`. Sem `NoNewPrivileges=` nem opção que o implique (D57), e sem `Protect*`/`PrivateTmp`. O motivo está nos comentários das units.
- **`make teste-units`** (`dev/testes/units.sh`, `dev/conteiner-units/`), fora do `make check`:
  1. `systemd-analyze verify` da instância `wardd@1000` numa cópia em `.dev/`, com um controle: uma cópia com diretiva inválida tem de ser acusada.
  2. Imagem `ubuntu:24.04` (mesma digest da 21b) mais o `systemd`. A rede só é usada no build. O contêiner roda com `--network none`, `--cgroupns=private` e `--ulimit memlock=-1:-1`. Ele também recebe `CAP_SYS_ADMIN` e roda sem AppArmor, só para o `iniciar.sh` remontar rw a própria subárvore de cgroup (ver "Problemas").
  3. O `dentro.sh` instala as units e o `wardd` **sem** `modo-teste` (`--target-dir target/sem-modo-teste`) e confere 17 pontos:
     - socket ativo, serviço parado, `/run/ward` e `/run/ward/1000` root:root 0755, socket do uid 1000 em 0600;
     - **a primeira conexão sobe o `wardd`**, que responde `estado`;
     - o daemon roda com os quatro `Uid` 1000, `NoNewPrivs: 0`, core 0/0, memlock unlimited e sem `LD_*` no ambiente;
     - morto com `kill -9`, ele sobe de novo na conexão seguinte;
     - o uid 1000 não apaga o socket nem cria arquivo em `/run/ward/1000`.
- `dev/daemon.sh`: repassa `WARD_DEV_DBUS_SISTEMA` só se ela vier definida. `CLAUDE.md`: a linha do `dev-daemon` ganhou a variável, e entrou a linha do `make teste-units`.
- Merge do master (D61–D64) feito antes de terminar. As notas de D62 foram aplicadas: `LimitMEMLOCK=infinity` e o D20 intocado.

## Segurança
- **Invariantes tocadas**
  - **S8:** a trava automática usa o `Estado::travar` de sempre: a chave e as entradas zeram (V3.8 continua passando). A unit tem `LimitCORE=0` (conferido no contêiner) e `LimitMEMLOCK=infinity` para o `mlockall` da 22b.
  - **S5/D46:** as pastas são criadas pelo systemd root:root 0755, e o socket fica do uid em 0600 (conferido no contêiner).
  - **D57:** nenhuma diretiva que ligue `no_new_privs`. O `NoNewPrivs: 0` foi lido do processo real.
  - **S9/D20:** a trava automática não mexe na espera do destravar.
  - **S13:** o log só tem textos fixos (motivo e minutos). O teste do binário procura senha, PIN e segredos no log com `RUST_LOG=trace`.
  - **Entrada não confiável:** o barramento só consegue travar. Nenhum sinal destrava, e o sinal forjado de outro par é ignorado ou trava (o agente já pode pedir `travar`). Corpo ilegível trava. O caminho do barramento do daemon de teste é validado.
- **Testes de ataque** (todos falham do lado do atacante):
  - `crates/wardd/tests/trava.rs`:
    - `ataque_agente_nao_segura_o_cofre_aberto_com_estado_ou_erro`: durante 4 h, `estado`, `executar` de entrada inexistente, `destravar` repetido com senha qualquer, `detalhe` pelo agente e PIN errado. O cofre trava às 4h00 do mesmo jeito.
    - `ataque_relogio_que_volta_ou_ilegivel_trava`.
    - `ataque_barramento_cai_com_o_cofre_aberto_trava`: o `dbus-daemon` falso é morto e o cofre trava em até 2 s.
    - `ataque_sinal_forjado_nao_destrava_nem_segura`: `Unlock`, `PrepareForSleep(false)` e `LockedHint=false` vindos de um par qualquer. O cofre segue travado.
    - `v3_19_prepare_for_sleep_trava_antes_de_soltar_o_inibidor`: quando o fd do inibidor fecha, o cofre já está travado. O inibidor é `sleep`/`delay` e é pego de novo ao acordar.
    - `d62_trava_automatica_nao_mexe_na_espera_do_d20`.
    - Caminho feliz do V3.19: `v3_19_inatividade_com_relogio_injetado` (3h59 não trava, 4h00 trava, um uso às 3h zera), `v3_19_tempo_lido_da_configuracao_do_cofre` (30 min, que continua valendo depois de reabrir), `v3_19_lock_e_locked_hint_travam_em_ate_2s` e `v3_19_binario_trava_com_lock_session_do_logind_falso` (binário real, `Lock` → `estado` travado em até 2 s, log sem segredo).
    - `daemon_de_teste_sem_barramento_falso_nao_liga_o_logind`: sem a variável, o logind fica desligado; com o caminho `/run/dbus/../x;unix:path=/tmp/x`, recusa subir.
  - `crates/wardd/tests/units.rs`: `d57_ataque_nenhuma_diretiva_que_liga_no_new_privs` (21 diretivas proibidas), `v3_21_servico_com_user_limitcore_e_sem_ld` (inclui `LimitMEMLOCK` e nenhum `Environment*`) e `v3_21_socket_do_sistema`.
  - `crates/wardd/src/trava.rs`: `ataque_caminho_do_barramento_estranho` (`,guid=`, `;unix:`, `%`, espaço, `..`, `//`, relativo, longo), `venceu_no_limite_e_falha_fechada` e `estado_travar_e_erro_nao_contam_como_uso`.
  - `crates/ward-plataforma/src/relogio.rs`: `nao_volta_e_bate_com_o_uptime`, que cobre o `unsafe` novo.
  - Barramento falso: um `dbus-daemon` privado em `$XDG_RUNTIME_DIR/wardd-bus-<pid>-<n>/` (0700), com config própria e `env_clear`. O logind falso é feito com o próprio `zbus` (dono de `org.freedesktop.login1`, `Inhibit` devolve a ponta de escrita de um pipe). O barramento do sistema e o logind real não foram usados.
- **Dependência nova: `zbus = "=6.2.1"`** (`default-features = false`, só `default-rt`; nos testes também `service`), mais `futures-core` direto (já vinha com o zbus) para o trait `Stream`.
  - **Motivo:** o logind só fala D-Bus. `zbus` é o cliente D-Bus em Rust puro mais usado, licença MIT, mantido pelo projeto z-galaxy (freedesktop/GNOME). As alternativas foram descartadas: `dbus` (bindings da libdbus, exige `libdbus-1-dev`, ausente aqui, e traz C) e `rustbus` (pouco uso, sem release desde 2023).
  - **Por que a 6 e não a 5.19:** comecei pela 5.19 (linha madura, 13 M de downloads). O `cargo audit --deny warnings` recusou: RUSTSEC-2026-0370/0371/0372 marcam `zvariant`, `zvariant_derive` e `zbus_names` como sem manutenção, porque foram fundidos no `zbus` 6. Seguir na 5 exigiria uma exceção no `deny.toml` e no audit para código que não recebe mais correção. Fui para a 6.2.1.
  - **Risco da 6:** ela é nova. A 6.0.0 saiu em 2026-10-16 e a 6.2.1 em 2026-11-01. O runtime `zruntime` 0.2.2 é do mesmo projeto e foi criado em 2026-10-13. A auditoria deve olhar isso. O ganho é uma árvore menor: 83 pacotes no `Cargo.lock`, contra cerca de 45 antes. Entram `zbus`, `zbus_macros`, `zbus_utils`, `zruntime`, `zcheapstr`, `endi`, `ordered-stream`, `async-trait`, `serde_repr`, `rustix`, `linux-raw-sys`, `socket2`, `uuid`, `hex`, `bitflags`, `winnow`, `futures-core`/`futures-io`, `pin-project-lite` e `syn` 3, além de crates só de Windows/WASI que não compilam aqui.
  - **Uso:**
    - só o endereço `unix:path=` (nada de `unixexec`/`launchd`, então o runtime não cria processo);
    - autenticação `EXTERNAL`, prazo de 5 s por chamada e fila de 256;
    - `zbus::block_on` só na thread do vigia.
  - **Teste novo no binário:** depois de o vigia conectar, o `SigCgt` do `wardd` não tem `SIGCHLD`. O runtime não toma o sinal dos filhos que o `ward run` e o lançador do app esperam.

## Ficou de fora
- **Conferir o dono de cada sessão** (`Session.User`): o daemon trava com o bloqueio de qualquer sessão. Isso é mais seguro e mais simples, mas trava demais numa máquina com vários usuários.
- **Ler o `LockedHint` atual na partida ou na reconexão:** o daemon sobe travado, e a reconexão só acontece depois de travar.
- **Fase 51:** instalar as units no host e o `tmpfiles.d`. Aqui só há as units de teste e o contêiner.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` | 0 |
| `cargo test -p wardd --all-features --test trava --test units` / `--lib` (com o zbus 5.19 e depois com o 6.2.1) | 0 / 0 |
| `make check` com o zbus 5.19 | 2 (`cargo audit`: RUSTSEC-2026-0370/0371/0372; trocado pelo 6.2.1) |
| `cargo clippy -p wardd -p ward-plataforma --all-targets --all-features -- -D warnings` | 0 |
| `git merge master` | 0 |
| `make teste-units` (17/17 ok, mais o verify e o controle) | 0 |
| `make dev-daemon` / `make dev-daemon-parar` (log: "logind desligado") | 0 / 0 |
| `make check` | 0 |

## Problemas descobertos
- **O contêiner do `make teste-units` recebe `CAP_SYS_ADMIN` e `apparmor=unconfined`.** O systemd não sobe com o `/sys/fs/cgroup` somente leitura que o Docker monta sem `--privileged`. Com `--cgroupns=private`, o remount só abre a subárvore do próprio contêiner. Mesmo assim, é um root com `CAP_SYS_ADMIN` sem namespace de usuário, rodando só os nossos scripts, sem rede, sem montar nada do host além de `/opt/teste` (somente leitura), e o contêiner é apagado no fim. O build da imagem usa rede (apt do Ubuntu, `systemd` 255.4-1ubuntu8.18). Fica a critério do orchestrator/auditor manter isso ou ficar só com o `systemd-analyze verify` e o `tests/units.rs`.
- **Agente em uso contínuo segura o cofre aberto.** `lista`/`run` do agente contam como uso ("cada uso zera", escopo). Um agente que pede `lista` a cada hora impede a trava por inatividade. O bloqueio de tela e a suspensão continuam travando. Se o planner quiser, pode contar só os usos do papel app e das aprovações.
- **O inibidor `delay` depende do polkit do logind real** (`org.freedesktop.login1.inhibit-delay-sleep`) para um serviço do sistema com `User=`. Isso não foi testado com o logind real (proibido na fase). Se o polkit negar, o daemon avisa uma vez e continua: a trava na suspensão segue pelo sinal, só que sem garantia de chegar antes do sono. O QA da 51 deve conferir com `systemd-inhibit --list`.
- **V3.19 com `loginctl lock-session` de verdade:** o `make dev-daemon` só vigia o logind com `WARD_DEV_DBUS_SISTEMA=/run/dbus/system_bus_socket`, e isso trava a tela do usuário. O validador decide se faz isso ou fica com o teste do barramento falso.
- **`LISTEN_FDS`/`LISTEN_PID`/`LISTEN_FDNAMES` ficam no ambiente do `wardd`** (lido do `/proc` no contêiner). Não é segredo, e o lançador do app (D57) monta o ambiente do zero. O `ward run` (Fase 26) não pode herdar essas variáveis; anoto para essa fase.
- **`MEMORY_PRESSURE_WATCH`/`MEMORY_PRESSURE_WRITE`** vêm do systemd 255 no ambiente do serviço. Não são `LD_*` nem da lista insegura da glibc. Registro só para a auditoria do ambiente (D48).
- **Resto no host:** a pasta `.dev/conteiner-units.*` é apagada pelo `trap`, mas a imagem `ward-teste/units-systemd:1` fica no Docker do host, como a `ward-teste/ssh:1`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/trava.rs` | criado |
| `crates/wardd/src/nucleo.rs` | alterado (relógio `agora`, `ultimo_uso`, `travar_se_inativo`, registro de uso) |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado (`zbus`, `futures-core`) |
| `crates/wardd/tests/trava.rs` | criado |
| `crates/wardd/tests/units.rs` | criado |
| `crates/ward-plataforma/src/relogio.rs` | criado |
| `crates/ward-plataforma/src/sys.rs` | alterado (`relogio_boot`) |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `Cargo.lock` | alterado (`zbus` 6 e dependências) |
| `dev/systemd/wardd@.socket` | criado |
| `dev/systemd/wardd@.service` | criado |
| `dev/testes/units.sh` | criado |
| `dev/conteiner-units/Dockerfile` | criado |
| `dev/conteiner-units/iniciar.sh` | criado |
| `dev/conteiner-units/dentro.sh` | criado |
| `dev/daemon.sh` | alterado |
| `Makefile` | alterado (`teste-units`) |
| `CLAUDE.md` | alterado (linhas do `dev-daemon` e do `teste-units`) |
| `docs/mission/handoffs/phase-24.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica: fase sem UI)
- [x] Ficou dentro do escopo da fase
