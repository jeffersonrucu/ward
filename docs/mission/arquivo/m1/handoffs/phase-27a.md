# Fase 27a — O token do "Conectar" nunca vai em argv (correção do achado alto do M3)

## Implementado
- **Desenho escolhido: socket Unix herdado (`socketpair`), não pipe nem memfd.**
  - O app cria o par e escreve o token na ponta dele. Depois fecha essa ponta, e o token fica só na fila de leitura da outra.
  - A outra ponta é o único fd ≥ 3 que passa pelo `exec` do terminal. Antes do `exec`, o filho roda `close_range(3, ~0, CLOSE_RANGE_CLOEXEC)` e tira o `FD_CLOEXEC` só do canal.
  - O terminal recebe `-e /usr/local/bin/ward ssh --token-fd <n> <entrada>`. Só o número do fd vai em argv, e ele não é segredo.
  - O `ward ssh` endurece o processo (não-dumpable, sem core), lê o token do fd e fecha o fd antes de conectar. Só então conecta.
  - **Por que socket:** o mesmo uid reabre um pipe, um memfd ou um arquivo de outro processo por `/proc/<pid>/fd/<n>`, porque basta o `PTRACE_MODE_READ`, que o Yama não restringe. Com isso, o atacante leria o token antes do `ward`. Um socket não reabre por ali (`ENXIO`). Tirar o fd de outro processo exige `pidfd_getfd`, que pede `PTRACE_MODE_ATTACH`, e o Yama com `ptrace_scope` ≥ 1 (exigido por D47) só deixa um ancestral fazer isso. Por esse motivo, o `ward ssh` recusa canal que não seja socket Unix de fluxo do mesmo uid.
  - **Terminais conferidos no host (COSMIC):** o `x-terminal-emulator` aponta para o `/usr/bin/cosmic-term`, e o `xterm` também está instalado. Os dois repassam o fd herdado ao comando do `-e`. Rodei os dois de verdade com o `ward ssh` de teste real contra o daemon de teste: o token chegou ao daemon, e um leitor de `/proc` em laço não o achou (`docs/mission/evidence/fase-27a/terminais.log`). Outros terminais (gnome-terminal, konsole) não estão instalados e não foram conferidos. Um terminal que entrega o comando a um servidor já aberto (caso do gnome-terminal) não repassa o fd. Nesse caso o `ward ssh` sai com "o canal do token do Conectar não está aberto" e não conecta (falha fechada, sem cair no argv).
- **`crates/ward-plataforma/src/conectar.rs` (novo):**
  - `CanalConectar::novo(token)`: par com `CLOEXEC` nas duas pontas, token de 1 a 64 bytes.
  - `numero()` e `preparar(&mut Command)`.
  - `ler_token_herdado(n)`: aceita fd de 3 a 2^20, socket `AF_UNIX`/`SOCK_STREAM` fora de escuta, com `SO_PEERCRED` do mesmo uid. Lê até o fim com prazo de 2 s, no máximo 64 bytes, num buffer `Zeroizing`, e fecha o fd sempre.
  - `ErroCanal` só com mensagens fixas.
- **`crates/ward-plataforma/src/sys.rs`:**
  - `herdar_so_no_exec`: `pre_exec` com `close_range` CLOEXEC e `fcntl(F_SETFD, 0)` no canal.
  - `tomar_socket_herdado`: confere `F_GETFD` e `SO_DOMAIN` antes de tomar posse e põe `FD_CLOEXEC`.
  - Os dois com `// SAFETY:`. O `unsafe` continua só no `sys`.
- **CLI `ward` (`args.rs`, `comandos.rs`):**
  - `ward ssh --token-fd <n> <entrada>`, com `n` só em dígitos decimais, de 3 a 2^20.
  - **`--token <t>` e `--token=<t>` são recusados** (`ErroUso::TokenEmArgv`, saída 2, sem conectar, mensagem sem o valor).
  - O token lido vai direto para o `Segredo` do pedido, sem cópia (`mem::take`). Um token fora do formato `[A-Za-z0-9_-]{16,64}` sai 125 com mensagem fixa.
  - A ajuda não ensina mais o token em argv.
  - `Entorno::ler_token` (padrão: o `ward-plataforma`).
- **App (`app/nucleo/src/terminal.rs`):**
  - `TerminalDoSistema::comando` devolve `Abertura { comando, canal }`. `Abertura::abrir()` faz o `spawn` e solta o canal do app.
  - `com_caminhos` serve aos testes. O `Terminal::abrir` continua com a mesma assinatura, então o Tauri, a ponte e o `servico.rs` não mudam (só o comentário do `servico.rs`).
  - De quebra, o terminal não herda mais nenhum outro fd ≥ 3 do app.
- **Infra de teste:**
  - `dev/daemon.sh` aceita `WARD_DEV_SOCKET_DIR` (pasta do socket), para subir o daemon desta worktree sem derrubar o do checkout principal, que estava no ar.
  - `dev/testes/cli.py` usa a mesma variável. O V4.32 passou a usar o canal, e foi criada a seção 6 (Fase 27a).
  - `CLAUDE.md`: linhas do `dev-daemon` e do `teste-cli`.
- **Daemon:** sem mudança. O `wardd` já recebia o token no `Pedido::Ssh` e o comparava em tempo constante (`ssh.rs:355`). O problema estava só no transporte entre o app e a CLI.

## Segurança
- **Invariantes tocadas e como a fase as respeita:**
  - **S9 e S10 (via D3):** o token pula PIN, nível e aprovação. Com ele fora de argv e de ambiente, um processo do mesmo uid (agente, pacote malicioso) não o lê em `/proc/<pid>/cmdline` ou `environ`, não reabre o canal por `/proc/<pid>/fd` e não o tira com `pidfd_getfd` (Yama ≥ 1). Por isso não consegue usar o token antes do `ward ssh` e pular nível e aprovação.
  - **S3:** nenhum segredo em argv. O token (que dá acesso) também não.
  - **S1 (entrada não confiável):** número do fd, tipo do fd, uid do par, tamanho e formato do conteúdo são conferidos. Erros sem eco.
  - **S8:** o `ward ssh` fica não-dumpable antes de o token existir na memória dele. O token fica em `Zeroizing` até virar o `Segredo` do pedido.
  - **S13:** nenhuma mensagem nem log com o token. O daemon segue logando só o evento.
- **Testes de ataque escritos:**
  - `crates/ward-plataforma/src/conectar.rs`:
    - `ataque_canal_forjado_e_recusado_sem_eco`: lixo grande, 65 bytes, vazio, socket que nunca fecha (prazo), pipe, arquivo, socket em escuta, datagrama, fds 0–2, número fora da faixa e fd fechado.
    - `ataque_reabrir_o_canal_por_proc_falha`: `ENXIO` ao reabrir o socket por `/proc/self/fd`. A contraprova mostra que um pipe no mesmo lugar seria reaberto e lido.
    - `exec_leva_so_o_canal`: um fd sem CLOEXEC não passa ao `exec`, e o canal passa.
    - `token_vai_e_volta_pelo_canal_e_o_fd_fecha`, `as_pontas_nascem_com_cloexec`, `token_vazio_ou_grande_nem_vira_canal`.
  - `crates/ward/src/args.rs`:
    - `ataque_token_em_argv_recusado_sem_eco`: `--token`, `--token=`, sem valor, e a ajuda sem `--token <`.
    - `ssh`: números inválidos de `--token-fd`, como `-3`, `+3`, ` 3`, `0x3`, vazio e acima do limite.
    - `formato_do_token`.
  - `crates/ward/src/testes.rs`:
    - `ataque_canal_do_token_forjado_nao_conecta`: lixo, `-` no início, vazio, grande, pipe e fd fechado. Todos saem 125 com mensagem fixa, sem conectar, e com o servidor sem conexão. `--token` em argv sai 2 sem conectar.
    - `ssh_pedido_token_saida_e_codigo`: o token chega ao pedido pelo canal, com a ordem `endurecer` → `conectar`.
  - `app/nucleo/src/terminal.rs`:
    - `ataque_token_do_conectar_nao_aparece_em_proc`: "Conectar" real com terminal falso e `ward` falso. Um leitor em laço varre `/proc/*/cmdline` e `/proc/*/environ` de todo processo e reabre `/proc/<pid>/fd/*` (≥ 3) do terminal e do `ward`, sem achar o token, que chega inteiro ao `ward` falso. Contraprova: com o token posto no argv de propósito, o teste falha ("token visto por outro processo").
    - `comando_do_terminal_com_ambiente_do_zero`: o token não aparece em nenhum argumento, valor de ambiente nem no programa.
  - `dev/testes/cli.py`, seção 6 (`make teste-cli`), com o `ward` de teste real contra o daemon de teste e um terminal falso:
    - um leitor de `/proc/*/cmdline`, `environ` e `fd` em laço não acha o token;
    - o token chega ao daemon (uma linha "token inválido" a mais no log, porque o token é inventado);
    - `--token` e `--token=` em argv saem 2, sem eco e sem chegar ao daemon;
    - canal com lixo, vazio, 4 KiB, pipe ou fd fechado sai 125, sem eco e sem chegar ao daemon.
  - Ataques do V5.24 que dependem de um token emitido (reuso, 30 s, outra entrada): seguem cobertos por `crates/wardd` › `v5_24_token_do_conectar` (Fase 27), sem mudança.
- **Dependências novas:** nenhuma. Usei `libc` (já no `ward-plataforma`) e a biblioteca padrão (`UnixStream::pair`).

## Ficou de fora
- **Terminais além de cosmic-term e xterm:** não instalados no host. O comportamento nos que não repassam o fd é falha fechada com mensagem. Se o usuário quiser suportar um desses, a alternativa é o próprio app abrir o pty e entregar a conexão, o que fica fora desta fase.
- **V5.24 pela interface com o `ward-app` real** (botão → janela do COSMIC): é do QA do M5. O caminho foi conferido aqui com o canal de verdade nos dois terminais, sem o app GTK.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` (com `.dev/servicos/` copiado do checkout principal, valores falsos, porque os contêineres no ar usam essas chaves) | 0 |
| `make dev-variacoes` | 0 |
| `cargo test -p ward-plataforma --lib conectar` (a 1ª rodada falhou em `exec_leva_so_o_canal`: o teste comparava números de fd, e o `ls` reaproveitou o número; passou a comparar o alvo do link) | 101, depois 0 |
| `cargo test -p ward --features modo-teste` | 0 |
| `cargo test -p ward-app-nucleo terminal` (mais a contraprova com o token no argv, que falhou como esperado) | 0 (contraprova: 101) |
| `cargo clippy --all-targets --all-features -p ward-plataforma -p ward -p ward-app-nucleo -p ward-ponte-teste -- -D warnings` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-27a make dev-daemon` (daemon só desta worktree; o do checkout principal ficou no ar) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-27a make teste-cli` (seções 1–6, 0 falhas) | 0 |
| `scratchpad/terminal_real.py` com `x-terminal-emulator -e` (cosmic-term) e `xterm -e` | 0 |
| `make check` | 0 |
| `make app-front` | 0 |
| `make app-check` (com `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do R32) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-27a make dev-daemon-parar` (só o desta worktree; o script só mata o pid cujo `cmdline` é o `wardd` desta worktree. O daemon do checkout principal já tinha sido parado pelo dono dele antes disso, porque o `.dev/wardd.pid` dele não existia mais) | 0 |
| `cargo clean` (target da worktree) | 0 |

## Problemas descobertos
- **PRD D3 diverge do código:** o texto ainda diz que o app abre `ward ssh --token <t> <entrada>` e que "o token não é segredo de credencial". O código agora usa `--token-fd <n>` e trata o token como valor que não pode vazar para o mesmo uid. O planner atualiza o D3 (e o RA2, se citar o argv). O V5.24 fala em "reusar o token visto em `/proc/<pid>/cmdline`", que agora nem aparece lá. A asserção pode virar "o token não aparece em `/proc/*/cmdline`/`environ`/`fd`, e reusar um token usado → nega". Não escolhi lado.
- **O cosmic-term repassa ao filho fds próprios (`/dmabuf:`)**, e todo filho do terminal herda a cópia do canal (já vazio). Não vaza o token, que é lido uma vez, mas registro.
- **Dependência do Yama:** a proteção contra `pidfd_getfd` depende de `ptrace_scope` ≥ 1, que o app e o daemon já exigem (D47). Com `ptrace_scope` 0, qualquer processo do mesmo uid tiraria o fd, mas aí o Ward já recusa rodar.
- **`npm audit` (dentro do `make app-front`):** 2 avisos *low* em `deepmerge` via `@sveltejs/vite-plugin-svelte` (dependência de build, sem correção). O alvo passa (`--audit-level=moderate`). Já existia antes e não tem relação com esta fase.
- **`dev/daemon.sh` e o `cli.py` não aceitavam `WARD_DEV_SOCKET_DIR`.** Com outro daemon de teste no ar (validação do M3 no checkout principal), não dava para rodar o `teste-cli` numa worktree sem derrubá-lo. Acrescentei a variável (infra de teste, sem efeito no release).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/conectar.rs` | criado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado |
| `app/nucleo/src/terminal.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado (comentário) |
| `dev/daemon.sh` | alterado |
| `dev/testes/cli.py` | alterado |
| `CLAUDE.md` | alterado (linhas de `dev-daemon` e `teste-cli`) |
| `docs/mission/evidence/fase-27a/terminais.log` | criado |
| `docs/mission/handoffs/phase-27a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Não se aplica (sem tela); "Conectar" conferido com terminal de verdade
- [x] Ficou dentro do escopo da fase
