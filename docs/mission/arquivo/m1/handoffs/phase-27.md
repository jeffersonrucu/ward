# Fase 27 — `ward ssh` com senha e com chave

## Implementado
- **CLI `ward ssh [--token <t>] <entrada> [--] [comando remoto…]`** (`crates/ward/src/args.rs`, `comandos.rs`, `cliente.rs`).
  - O token tem formato fixo `[A-Za-z0-9_-]{16,64}` e não pode começar com `-`.
  - Os limites do comando são os mesmos do `ward run` (D52).
  - Um erro do Ward sai 125. Com o cofre travado, a CLI avisa e espera, como no `run`.
  - Sem comando e com stdin/stdout em terminal, a sessão é interativa:
    - o terminal fica em modo cru (`ModoCru`, que volta no `Drop`);
    - a janela inicial vai no pedido;
    - uma thread olha o tamanho a cada 250 ms e manda `janela` quando ele muda.
- **Daemon** (`crates/wardd/src/ssh.rs`): o `Pedido::Ssh` do papel agente é atendido no `servidor.rs` pelo mesmo executor do `ward run`. O `execucao::sessao_com` foi refatorado para aceitar pty e agente.
  - **Argv** (`montar_argv`): `execve` sem shell.
    - Senha: `sshpass -e`, com a senha só em `SSHPASS`.
    - Sempre: `-F /dev/null`, `IdentitiesOnly=yes`, `CertificateFile=/dev/null`, `UserKnownHostsFile=<pasta do cofre>/known_hosts`, `GlobalKnownHostsFile=/dev/null` e `StrictHostKeyChecking=accept-new`.
    - Também sempre: `UpdateHostKeys=no`, `ControlMaster=no`, `ControlPath=none`, `ForwardAgent=no`, `ForwardX11=no`, `PermitLocalCommand=no` e `ProxyCommand=none`.
    - Por senha: `PubkeyAuthentication=no`, `IdentityAgent=none` e `IdentityFile=none`.
    - Por chave: `IdentityAgent=<socket>`, `IdentityFile=<id.pub>`, `PasswordAuthentication=no`, `KbdInteractiveAuthentication=no` e `BatchMode=yes`.
    - `ssh`: `-p <porta> -l <usuário> -- <host> [comando…]` (D43).
    - `sftp`: `-o User=<usuário> -P <porta> -- <host>` (D56).
    - Usuário e host são conferidos de novo: o conjunto de D43, sem `-` no começo e sem `@` no host. Valor fora do formato não monta argv.
  - **Ambiente do filho do zero**: `PATH=/usr/bin:/bin`, `LANG=C.UTF-8`, `TERM=xterm-256color` (só no pty) e `SSHPASS` (só por senha). O cwd é o home do passwd.
  - **Tipos**:
    - `servidor` é aceito.
    - `ftp/sftp` com protocolo `sftp` é aceito sem comando; os comandos vêm pela entrada padrão. Com comando, dá `pedido_invalido`.
    - O resto dá `tipo_sem_entrega`.
    - `servidor` com salto responde `entrega_falhou` até a Fase 28.
    - O tipo é conferido depois do nível, sob a trava do estado. Assim o V4.32 numa entrada `banco` responde pelo nível.
- **Agente SSH temporário de D12** (`crates/wardd/src/agente_ssh.rs`).
  - Cria a pasta `<rt>ssh-<16 aleatórios>/` (0700) com `agent.sock` (0600) e `id.pub` (0600).
  - A chave é decifrada com a passphrase pelo `ssh-key` e só Ed25519 é aceita.
  - Só responde lista (11→12) e assina (13→14). Qualquer outra mensagem recebe FAILURE (5).
  - Só atende o pid do filho. No `sftp`, atende também o `ssh` que é filho do filho; quem confere é o SO_PEERCRED + ppid.
  - Qualquer outro processo é fechado sem nenhum byte. O limite é 4 conexões e 256 KiB por mensagem.
  - O agente some no fim da sessão, quando a CLI morre (o grupo é morto) e quando o cofre trava (contador `travas` no `Estado`).
- **Pty** (`ward-plataforma`):
  - `Comando::lancar_em_pty`: `setsid` + `TIOCSCTTY` no filho e janela por `TIOCSWINSZ`.
  - Também: `Pty::definir_janela`, `terminal::{e_terminal, tamanho, ModoCru}`, `pasta_pessoal()` e `ProcessoPar::pid_do_pai()`.
  - Todo `unsafe` está em `sys.rs`, com `// SAFETY:` e testes.
- **D34**: `Fluxo::segurando()` e `Fluxo::soltar()` em `ward-core::mascara`.
  - No pty, um fim segurado que fica 300 ms (`PAUSA_INTERATIVA`) sem saída nova sai como `••••`.
  - O trecho vira intervalo coberto, então o resto do mesmo segredo, quando chega, continua escondido. Uma saída que diverge sai normal.
- **Token do "Conectar" (D3, V5.24)**: `Daemon::emitir_token`.
  - Só o papel app pede, só para `servidor` e só com o cofre destravado.
  - São 32 letras e dígitos (`gerador::token_conectar`), válidos por 30 s no relógio do motor.
  - Cabem no máximo 16 tokens; o mais antigo sai. Ficam em `Aberto`, então somem ao travar.
  - `usar_token` compara em tempo constante (`gerador::tokens_iguais` → `cripto::iguais_em_tempo_constante`).
  - Todo token apresentado que existe é queimado, mesmo usado na entrada errada.
  - Com token válido para a entrada, o nível e a aprovação são pulados. Inválido, vencido, repetido ou de outra entrada: vale o nível.
- **Testes de integração**:
  - `make teste-cli` (`dev/testes/cli.py`, seção 4) roda contra os SSH de teste reais.
  - `crates/wardd/tests/ssh.rs` usa `ssh`/`sshpass`/`sftp` falsos (`tests/ssh_falso.py`, que registram argv e ambiente e nunca conectam).

## Segurança
- **Invariantes tocadas e como a fase as respeita**:
  - **S1**: a senha e a chave nunca vão à CLI. O agente nunca exporta a chave e só assina.
  - **S2**: o único arquivo é `id.pub` (público), em `<rt>ssh-*/`. Nada vai para `/tmp`.
  - **S3**: nenhum segredo em argv.
  - **S4**: a saída é mascarada, com a pausa de D34 no pty.
  - **S8**: o token é comparado em tempo constante.
  - **S13**: os logs não têm valores. O token é logado só como evento.
  - Decisões tocadas: D3, D12, D13, D34, D43 e D56.
- **Testes de ataque, `crates/wardd/tests/ssh.rs`**:
  - `v4_35_d43_usuario_com_arroba_e_senha_so_no_ambiente`: `deploy@cliente.com.br` vai inteiro depois de `-l`. O comando remoto `-- -l x@y` fica depois do host, sem `User=` e sem `@host`. A senha não aparece em argv (só sha256 em `SSHPASS`). O ambiente tem exatamente LANG, PATH e SSHPASS. O filho que imprime a senha aparece como `••••`.
  - `v4_37_d56_sftp_com_user_e_host_depois_de_traco_traco`: `sftp` com comando é recusado sem executar nada.
  - `recusas_sem_executar_nada`: tipo errado, `ftp`, bloqueada, inexistente, com aprovação sem app e salto.
  - `v4_24_d12_agente_so_do_filho`: adicionar, remover e travar recebem 5. O neto do `ssh` é recusado. O processo de teste (mesmo uid) conecta e recebe 0 bytes. Pasta 0700, socket e pública 0600, sem `SSH_AUTH_SOCK`, e tudo some no fim.
  - `d12_travar_derruba_o_agente`.
  - `d12_sftp_com_chave_atende_o_ssh_do_sftp`: o neto do `ssh` é recusado.
  - `d34_pty_prefixo_parado_vira_marca_e_o_resto_nao_sai`: nenhuma janela de 6+ bytes da senha na saída, e a marca chega entre 250 e 950 ms.
  - `v5_24_token_do_conectar`: só o app emite; testa token reusado, inventado, de outra entrada (queimado), vencido com 31 s (com 29 s vale), apagado ao travar, bloqueada com token válido (continua inexistente) e o 17º token derrubando o 1º.
- **Testes de ataque, unidade**:
  - `crates/wardd/src/ssh_testes.rs` › `d43_usuario_com_arroba_so_depois_de_l` (o `-oProxyCommand=x` do pedido nunca vira opção), `ataque_valores_fora_do_formato_recusados`, `d56_sftp` e `chave_usa_o_agente_da_sessao`.
  - `crates/wardd/src/agente_ssh.rs` › `ataque_adicionar_remover_travar_e_lixo_recusados`.
  - `crates/ward-core/src/mascara/testes.rs` › `d34_ataque_prefixo_em_base64_e_percent_tambem_vira_marca` e os outros três `d34_*`.
  - `crates/ward/src/args.rs` › `ssh`: tokens fora do formato, e a mensagem de uso não repete argumento.
  - Pty em `ward-plataforma`: `filho.rs` › `pty_terminal_de_controle_janela_e_entrada` e `pty_matar_o_grupo_leva_a_sessao_e_o_ambiente_e_exato`; `terminal.rs` › `modo_cru_liga_e_volta_e_tamanho_do_pty`.
- **Ponta a ponta (`make teste-cli`, seção 4, tudo `ok`)**:
  - V4.23: `whoami` → `deploy`, com `sshpass -e`, `PubkeyAuthentication=no` e `IdentityAgent=none` vistos em `/proc/<pid>/cmdline`.
  - V4.24 e V4.37: login real do `sftpuser`.
  - V4.26: `ssh-add -L`, `ssh-add <chave>` e `ssh-add -D` de outro processo recusados; o agente some no fim, com SIGKILL na CLI e ao travar.
  - V4.28: uma linha falsa no `.dev/known_hosts` faz o `ssh` recusar; o arquivo é restaurado no fim.
  - V4.31 e V4.34: num pty, `P:••••` em cerca de 0,3 s; o resto não aparece e o `Q:` que diverge sai normal.
  - V4.32: o token inventado é ignorado.
  - V4.14: varredura de `/proc/*/cmdline` a cada 50 ms, 97 mil leituras e 0 achados.
  - V4.35 e V4.37 pelo argv em `/proc/<pid>/cmdline`, com o pai do `sshpass` sendo o `wardd` (sem `sh -c`).
  - Nenhum padrão de `.dev/variacoes.txt` em nenhuma saída.
- **Dependência nova**: `ssh-key =0.7.0-rc.12` (RustCrypto), com features `alloc`, `ed25519` e `encryption`, no `wardd`.
  - **Por quê**: ler a chave OpenSSH da entrada, decifrar a passphrase (bcrypt-pbkdf + aes-ctr) e assinar Ed25519 no agente de D12. Sem ela, isso seria cripto feita à mão, o que o projeto proíbe.
  - Sem `rsa`, por causa do RUSTSEC-2023-0071 (Marvin).
  - Crates transitivas novas: aes, aes-gcm, bcrypt-pbkdf, blowfish, ctr, des, ed25519, ed25519-dalek, curve25519-dalek, ghash, polyval, pbkdf2, pem-rfc7468, sha2, signature, ssh-cipher, ssh-encoding, subtle e algumas de build.
  - Divide `cipher 0.5` e `digest 0.11` com as atuais. `cargo audit` e `cargo deny` estão limpos.
  - É uma versão `rc`, fixada com `=`.

## Ficou de fora
- **V4.27** (HOME de sentinela com `~/.ssh/config` + `inotifywait`):
  - O `inotifywait` não está instalado.
  - O `ssh` usa o home do passwd, não `$HOME`, então um HOME falso não testa nada. O teste de verdade exige o daemon num bwrap com o home em tmpfs.
  - O que está coberto: `-F /dev/null` (o `ssh` não lê config nem cria `~/.ssh` com `-F`), ambiente sem `SSH_AUTH_SOCK` e identidades só as do Ward.
  - Fica para o auditor ou para uma fase de ferramentas.
- **V4.35/V4.37 "por strace"**: o `wardd` recusa subir sob depurador ("processo sob depurador (TracerPid ≠ 0); não sobe"), e `ptrace_scope=1` impede anexar.
  - A conferência foi feita pelo argv de `/proc/<pid>/cmdline` de cada `sshpass`/`ssh`/`sftp` vivo, que é o argv do `execve`.
  - O caso `deploy@cliente.com.br` foi conferido só com o `ssh` falso, porque o cofre de teste não tem essa entrada.
- **V4.35 com salto** (`acme-ssh-interno`): fica para a Fase 28. Por enquanto responde `entrega_falhou`.
- **V5.24 pela interface**: o app (Fase 34) já chama `emitir_token` + `ward ssh --token`. O teste pela UI é do M5. O núcleo está coberto por `v5_24_token_do_conectar`.
- **`scp`**: não é usado nesta fase.
- **RA2**: a confirmação com o usuário é do orchestrator.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-servicos` / `make dev-cofre` / `make dev-variacoes` | 0 |
| `make dev-daemon` (daemon desta worktree; só eu o subi, parei e subi de novo) | 0 |
| `cargo test -p wardd --test ssh` (8 testes) | 0 |
| `cargo test -p ward --features modo-teste` | 0 |
| `make fmt` | 0 |
| `make check` (fmt, clippy, testes, audit, deny, gitleaks) | 0 |
| `make teste-cli` (seções 1–4) | 0 |
| `make teste-ataques-servicos` | 2 (aborta no `teste-home-real`: o `git check-ignore` abre `.git/worktrees/<agente>/HEAD`; ambiente de worktree, ver Problemas) |

## Problemas descobertos
- **`make teste-ataques-servicos` não roda numa worktree.** O `teste-home-real` acusa o `git` ("o cliente devia ter entrado e não entrou" e "check-ignore abriu `/home/…/cofre/.git/worktrees/agent-…/HEAD`"). Numa worktree, o `.git` é um arquivo que aponta para o repositório principal, fora do permitido pelo detector. O defeito não é desta fase. Os outros clientes passaram, e a suíte aborta ali.
- **O protocolo `Pedido::Ssh` não traz cwd nem ambiente, e o D13 fala do ambiente da CLI.**
  - Escolhi a opção segura: o filho do `ssh` recebe um ambiente fixo, montado do zero, com `TERM=xterm-256color` fixo no pty, e cwd = home do passwd. O home é para o `get`/`put` do `sftp` interativo; `/` seria a alternativa neutra.
  - É uma divergência pequena do escopo. O orchestrator decide se o protocolo deve levar `TERM`/cwd.
- **D13 diz `IdentityFile=none`.** No modo chave, o argv usa `IdentityFile=<id.pub do Ward>` com `IdentitiesOnly=yes`, para o `ssh` oferecer só a chave do agente. No modo senha continua `none`.
- **Só Ed25519 no agente.** RSA ficou de fora por causa do RUSTSEC-2023-0071, e uma entrada com chave RSA dá `entrega_falhou`. Precisa de decisão se o escopo exigir RSA.
- **`emitir_token` responde também para `servidor` bloqueado.** O app vê tudo, então não há vazamento para o agente. O uso do token continua dando `entrada_inexistente`.
- **O `known_hosts` fica ao lado do cofre.** No release é `~/.local/share/ward/known_hosts`; no teste, `.dev/known_hosts`. O escopo não fixa o caminho.
- **R19 residual:** o `ssh` ainda resolve o home pelo passwd (por exemplo, para expandir `~`). Com `-F /dev/null` ele não lê `~/.ssh/config` nem cria `~/.ssh`. Sem `inotifywait`, não deu para provar a ausência total de acesso (ver V4.27).
- **O V3.15 (`tests/aprovacao.rs`) lê só os arquivos do primeiro nível de `wardd/src/`.** Um módulo em subpasta quebra o teste (EISDIR) e também fica fora da varredura. Por isso os testes do `ssh` estão em `src/ssh_testes.rs` com `#[path]`. Vale o V3.15 passar a percorrer subpastas, mas não mexi porque está fora do escopo.
- **`ssh-key` é `rc`** (`0.7.0-rc.12`). Fica a acompanhar a versão estável.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Cargo.lock` | alterado |
| `CLAUDE.md` | alterado (linha do `make teste-cli`) |
| `crates/ward-core/src/cripto.rs` | alterado |
| `crates/ward-core/src/gerador.rs` | alterado |
| `crates/ward-core/src/mascara.rs` | alterado |
| `crates/ward-core/src/mascara/testes.rs` | alterado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/processo.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/terminal.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/cliente.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/src/agente_ssh.rs` | criado |
| `crates/wardd/src/ssh.rs` | criado |
| `crates/wardd/src/ssh_testes.rs` | criado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | criado |
| `crates/wardd/tests/ssh_falso.py` | criado |
| `dev/testes/cli.py` | alterado |
| `docs/mission/handoffs/phase-27.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (a chave dos testes de cargo é gerada na hora, com passphrase aleatória)
- [x] Teste de ataque para cada (SEG) tocado (V4.27 parcial, ver "Ficou de fora")
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
