# Fase 28 — Salto e túnel de `banco` por `servidor`, nível combinado, logs do M4 e V4.2 (2ª parte)

## Implementado
- **Ponte do salto e do túnel, sem `ProxyJump` e sem shell** (`crates/wardd/src/ponte.rs`, novo).
  - Por que não `ProxyJump`: o OpenSSH 9.6 monta `ssh -l … -p … -F <config> -W '[%h]:%p' <salto>` e o executa por `$SHELL -c "exec …"` (a string está no binário). Isso põe um `sh -c` na cadeia, que o V4.35 proíbe. Além disso, as opções de `-o` não chegam ao `ssh` do salto (só o `-F`), e ele ficaria com o `known_hosts`, as identidades e o agente padrão do `~/.ssh` (D13, R19).
  - O daemon escuta em `127.0.0.1:0`. A porta vem do kernel e fica presa ao socket do daemon, então ninguém a toma antes.
  - Cada conexão aceita só passa se o socket do par, achado em `/proc/net/tcp`, for do mesmo uid **e** estiver aberto por um processo do grupo de processos do filho da sessão (`/proc/<pid>/stat` + `/proc/<pid>/fd`).
    - No salto, o grupo é o do `ssh` principal (ou do `sshpass` dele). No túnel, é o do comando do `ward run`.
    - Qualquer outra conexão é fechada sem nenhum byte e sem lançar nada.
  - Para cada conexão aceita, o daemon executa por `execve` um `[sshpass -e] ssh -F /dev/null … -W <alvo>:<porta> -p <porta> -l <usuário> -- <host>` até a entrada intermediária. O endurecimento é o mesmo da Fase 27; o ambiente é montado do zero; a senha vai só em `SSHPASS`; com chave, cada `ssh` da ponte tem o seu agente de D12, preso ao pid dele.
  - Os bytes passam entre a conexão e o stdin/stdout desse `ssh`, com buffers `Zeroizing`. O stderr dele passa pela máscara das duas entradas (D19) e chega à CLI como `Erro`.
  - Limites: o salto aceita 1 conexão no total; o túnel aceita 8 simultâneas e 64 por sessão.
  - A ponte para de aceitar quando o cofre trava (contador `travas`) e morre com a sessão (o `ssh` de cada conexão leva `SIGKILL`).
- **`ward ssh` com salto** (`ssh.rs`):
  - A entrada de salto precisa ser `servidor`, visível e sem salto. Se for bloqueada ou tiver sumido depois da autorização, dá `negado`; se for de outro tipo, `entrega_falhou`.
  - O `ssh` principal conecta em `127.0.0.1:<porta da ponte>` com `HostKeyAlias=<nome do destino no known_hosts>` (`host` na 22, `[host]:porta` nas outras) e `CheckHostIP=no`. Assim a chave conferida continua sendo a do destino (D13).
  - A máscara cobre as duas entradas.
  - Funções novas: `montar_argv_com_alias`, `Programa::Encaminhar`, `alias_do_destino` e `intermediario`.
- **Túnel do `banco` no `ward run`** (`execucao.rs`): o `Recursos::tunel` deixou de ser stub.
  - Abre a ponte pelo `servidor` do túnel e devolve a porta local, que vai para `PGHOST=127.0.0.1`/`PGPORT` e o `DATABASE_URL`.
  - A máscara do run inclui o `servidor` do túnel (D19).
  - A ponte começa a aceitar quando o pid do comando é conhecido e fecha no fim. `Preparo` ganhou `ponte`, que o `sessao_com` inicia e derruba.
- **Nível combinado (D19, V4.30)**: o `uso.rs` já resolvia o nível. Agora os testes cobrem salto com aprovação (`aprovacao_indisponivel`/negado) e salto bloqueado (negado).
- **V4.33**:
  - O `make dev-daemon` sobe o daemon de teste com `RUST_LOG=trace` por padrão (`dev/daemon.sh` aceita só `error|warn|info|debug|trace`).
  - O `make teste-cli` confere que o log tem linhas `[debug]` e nenhum padrão de `.dev/variacoes.txt`.
  - A CLI roda com `RUST_LOG=trace` nos testes do salto e do túnel, com `sem_variacao`.
- **V4.2, 2ª parte (D65 e)**, no contêiner de `make teste-docker-plataforma` (`dev/conteiner-plataforma/dentro.sh`, `falso.c`). O Makefile compila `ward` e `wardd` sem `modo-teste` em `target/sem-modo-teste`, e o script recusa binário com a string `WARD_DEV_SOCKET_DIR`.
  - O `wardd` de release copiado para `/tmp` como uid 1000 não sobe: com `LISTEN_FDS` de um socket numa pasta do uid sai com "o socket do systemd não está no caminho esperado"; sem `LISTEN_FDS`, também não sobe.
  - Três falsos não-dumpable (`prctl(PR_SET_DUMPABLE,0)`, conferido `dumpable=0`) do uid 1000 escutam em `/run/user/1000/{ward,ward-teste,copia}/wardd.sock`. Um controle prova que o registro pega conexões de verdade.
  - `ward destravar`, `ward lista` e `ward travar` rodam num pty (`script`), com a senha falsa digitada e `XDG_RUNTIME_DIR`/`WARD_DEV_SOCKET_DIR` apontando para os falsos, em três cenários: com o socket velho do root, sem socket e sem a pasta `/run/ward/1000`. Em todos, a CLI sai ≠ 0 com "o daemon não está rodando", sem mostrar "Senha mestra", e nenhum falso recebe conexão nem byte.
  - `bind` do uid 1000 em `/run/ward/1000/` dá `EACCES`, tanto com nome novo quanto com `wardd.sock` depois de removido. Com o socket velho no lugar, o uid 1000 não o toma. O uid 1000 também não cria a pasta `/run/ward/1000`.
- **Testes integrados contra os serviços** (`dev/testes/cli.py`, seção 5): V4.25, V4.29, V4.30, V4.35 com salto, o ataque à ponte do túnel e V4.33. O cliente do Postgres é `dev/testes/pg_select.py`, novo: SCRAM-SHA-256 só com a biblioteca padrão, porque o host não tem `psql`. Ele entrou no `dev/testes/home-real.sh`.

## Segurança
- **Invariantes tocadas e como a fase as respeita**:
  - **S1**: nenhuma credencial do salto ou do túnel vai à CLI. Com chave, a ponte usa o agente de D12, que só lista e assina, e a chave privada não sai do daemon.
  - **S2**: só `id.pub` em `<rt>ssh-*/` (agente). Nada em `/tmp` nem no home.
  - **S3**: a senha vai só em `SSHPASS`. Nenhum argumento tem segredo, e não há `sh -c` em lugar nenhum: todo `ssh` é filho do `wardd` ou do `sshpass`, verificado no `/proc` pelo `teste-cli`.
  - **S4**: a máscara cobre as duas entradas, no stdout/stderr do comando e no stderr do `ssh` da ponte.
  - **S5**: V4.2 inteiro (as duas partes).
  - **S8**: o salto e o túnel valem o nível mais restritivo (D19), e a ponte para quando o cofre trava.
  - **S13**: o log usa só textos fixos e números. V4.33 conferido com `RUST_LOG=trace`.
  - **D43**: o usuário do salto vai só depois de `-l` (com `@` inteiro) e o host depois de `--`. Nunca há `usuário@host`, `User=` nem `ProxyJump`.
- **Testes de ataque escritos**:
  - `crates/wardd/tests/ssh.rs`:
    - `fase28_salto_com_senha_pela_ponte`: um estranho do mesmo uid conecta na ponte e é fechado sem byte, sem lançar o `ssh` do salto. A segunda conexão do salto é recusada, a senha do salto sai mascarada no stderr e a porta fecha no fim.
    - `fase28_tunel_do_banco_no_run`: estranho recusado no túnel, mascaramento de D19 e porta fechada no fim.
    - `fase28_travar_fecha_o_tunel_para_conexoes_novas`.
    - `fase28_salto_com_chave_agente_de_cada_ssh`: o agente do salto recusa adicionar, remover e travar, e o neto é recusado.
    - `recusas_sem_executar_nada`: salto com aprovação e salto bloqueado não executam nada (V4.30).
  - `crates/wardd/src/ssh_testes.rs`:
    - `fase28_ataque_alvo_e_alias_fora_do_formato`: `-oProxyCommand=x`, espaço, `%`, colchetes e porta 0 no alvo e no alias.
    - `fase28_argv_da_ponte_sem_proxyjump`.
  - `crates/wardd/src/ponte.rs`:
    - `ataque_socket_de_outro_uid_ou_ambiguo`.
    - `par_do_grupo_de_verdade`: outro grupo e outro uid são recusados.
    - `grupo_do_stat_com_parenteses_no_nome`.
  - `dev/testes/cli.py` (seção 5): ataque à ponte do túnel contra o daemon de teste real, sem nenhum `ssh` filho de `ssh`.
  - `dev/conteiner-plataforma/dentro.sh` (V4.2, 2ª parte): falso não-dumpable, `wardd` copiado e `bind` em `/run/ward/1000/`.
- **Dependências novas**: nenhuma. O `falso.c` é programa de teste compilado com o gcc do host, que o alvo já exigia. O `pg_select.py` usa só a biblioteca padrão.

## Reexecução depois do merge do master (disco cheio)
- **Causa da falha no master: o disco cheio.** O `/` estava 100% cheio quando os testes de ssh falharam (ENOSPC). Depois que o coordenador liberou 22 GB, rodei `git merge master` (o conflito no `CLAUDE.md` foi resolvido mantendo as duas linhas; merge em b905c8e) e repeti os testes:
  - `cargo test --all-features -p wardd --test ssh` passou com 12/12 numa execução sozinho e em 3 execuções em paralelo.
  - `make check` passou ("tudo limpo") duas vezes seguidas.
- **Endurecimento dos testes contra corrida** (sem mudar código de produção). Rodando em paralelo, apareceram duas corridas:
  - `fechar_entrada` (`crates/wardd/tests/ssh.rs`) dava pânico com broken pipe quando o daemon já tinha respondido e fechado o stream numa recusa cedo. Agora ignora o erro da escrita.
  - A checagem "porta fechada no fim" passou a usar o inode do socket de escuta (`inode_da_escuta`/`escuta_fechada`, por `/proc/net/tcp` e `/proc/self/fd`), porque, com testes em paralelo, outro teste pode reusar o número da porta.
  - O timeout de conexão subiu de 10 para 60 s no `ssh_falso.py` e no cliente Python do teste. Sob carga, o ssh falso do salto com chave roda o cliente do agente com um neto antes de responder.

## Ficou de fora
- **V4.27**: o `inotifywait` não está instalado e não há sudo. O `ssh` da ponte usa as mesmas opções do `ward ssh` da 27 (`-F /dev/null`, `IdentityFile=none`/`id.pub`, `CertificateFile=/dev/null`, `known_hosts` do Ward, ambiente do zero), mas a ausência total de acesso ao `~/.ssh` não foi provada. É o mesmo residual R19 do handoff 27.
- **`ward run` de um `servidor` com salto**: o `uso.rs` já cobra o nível combinado, mas a entrega continua com o `WARD_HOST`/`WARD_PORT` do destino, sem abrir a ponte. O escopo só cita o túnel de `banco` no `ward run`.
- **`make teste-home-real` e `make teste-ataques-servicos`**: não rodam em worktree (o `git` abre `.git/worktrees/…`, como no handoff 27). O `pg_select.py` foi incluído no `home-real.sh`, mas essa inclusão não foi executada.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-servicos` / `make dev-cofre` / `make dev-variacoes` | 0 |
| `cargo test -p wardd --test ssh` (12 testes) | 0 |
| `cargo test -p wardd --lib` | 0 |
| `make dev-daemon` (daemon desta worktree; nenhum outro estava no ar) | 0 |
| `make teste-cli` (seções 1–5, 0 falhas) | 0 |
| `make teste-docker-plataforma` | 0 |
| `make fmt` | 0 |
| `make check` (fmt, clippy, testes, audit, deny, gitleaks: "tudo limpo") | 0 |
| `make dev-daemon-parar` (só o daemon desta worktree) | 0 |
| `git merge master` (após liberar o disco) | 0 (conflito no `CLAUDE.md` resolvido) |
| `cargo test --all-features -p wardd --test ssh` (1 sozinho + 3 em paralelo, 12/12 cada) | 0 |
| `make check` duas vezes seguidas após o merge | 0, 0 |
| `make teste-home-real` / `make teste-ataques-servicos` | não rodados (worktree) |

## Problemas descobertos
- **Divergência com o escopo (D13: "Salto por `ProxyJump`")**: o salto não usa `ProxyJump`, pelos motivos da seção "Implementado" (`sh -c` na cadeia, contra o V4.35, e opções que não chegam ao `ssh` do salto). O planner precisa ajustar o texto de D13/D43 ("a Fase 28 usa um `ssh_config` gerado… ou recusa o salto com `@`"). Com a ponte, o usuário com `@` funciona e não há `ssh_config` gerado.
- **Túnel só para o grupo de processos do `ward run`**: um cliente que roda fora do grupo não conecta. Exemplos: `docker run --network host … psql`, um processo que chama `setsid` e um cliente de outro uid. Foi a escolha segura; se o usuário precisar de clientes em contêiner, é decisão dele.
- **Cada conexão do túnel faz um login SSH novo** no servidor do túnel (`ssh -W` por conexão). Clientes com pool de conexões (até 8 simultâneas e 64 por sessão) pagam essa latência.
- **Chaves dos serviços compartilhados**: os contêineres de pé usam a chave do checkout principal. Copiei `acme-deploy_ed25519{,.pub}` e `known_hosts` de `/home/…/cofre/.dev/servicos/` para o `.dev/` desta worktree (valores falsos) e rodei `make dev-cofre` de novo. Nenhum serviço foi derrubado.
- **O stderr do `ssh` da ponte** (por exemplo, "Permanently added '[127.0.0.1]:2224'…") aparece na CLI como `Erro`, mascarado, também no pty interativo.
- **`known_hosts` do Ward**: o salto grava `[127.0.0.1]:2224` e o destino grava `ssh-interno` (pelo alias), como numa conexão direta.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/ponte.rs` | criado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/src/ssh_testes.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `crates/wardd/tests/ssh_falso.py` | alterado |
| `dev/testes/cli.py` | alterado |
| `dev/testes/pg_select.py` | criado |
| `dev/testes/home-real.sh` | alterado |
| `dev/testes/docker-plataforma.sh` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado |
| `dev/conteiner-plataforma/falso.c` | criado |
| `dev/daemon.sh` | alterado |
| `Makefile` | alterado |
| `CLAUDE.md` | alterado (linhas de `dev-daemon`, `teste-cli` e `teste-docker-plataforma`) |
| `docs/mission/handoffs/phase-28.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V4.27 sem `inotifywait`; ver "Ficou de fora")
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
