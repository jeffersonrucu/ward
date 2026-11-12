# Fase 28g — Achados da 2ª auditoria do M4

## Implementado
- **Médio (write único > 64 KiB mostra o começo do segredo):** o corte não era só em 64 KiB. Com o buffer de leitura de 16 KiB, um `write` único com a senha cruzando 16 KiB também vazava (reproduzido: n de 16 370 a 16 383).
  - Ler de novo até EAGAIN, como o plano sugeria, não basta sozinho. Quando o `write` passa da capacidade do pipe (64 KiB), o kernel copia 64 KiB, acorda o leitor e só depois copia o resto. O leitor pega EAGAIN antes disso. Medido: 9 de 50 casos ainda vazavam só com a leitura repetida.
  - `ward-plataforma/src/sys.rs`: os pipes de saída e de erro do filho passam a 256 KiB (`F_SETPIPE_SZ`, `CAPACIDADE_PIPE_SAIDA`). Se o kernel recusar (limite de páginas de pipe do usuário), segue com o padrão, sem falhar.
    - Um `write` de até 256 KiB é copiado inteiro com a trava do pipe presa, e o leitor só o vê completo.
    - Por que 256 KiB e não 1 MiB: o limite `pipe-user-pages-soft` é de 64 MiB por usuário. 64 conexões × 2 pipes × 1 MiB passaria disso e encolheria os pipes de todos os programas do usuário. Com 256 KiB fica em 32 MiB.
  - `wardd/src/execucao.rs`: novo `ler_lote`, com buffer de 256 KiB (`LOTE`).
    - A primeira leitura vem depois do `poll`. As seguintes só acontecem enquanto `poll(0)` diz que há mais, sem nunca esperar dado novo: não segura nem atrasa nada, então D34 revisto continua valendo e não reabre o oráculo de tempo.
    - O fd fica bloqueante: o mestre do pty é o mesmo arquivo aberto da escrita da entrada, e `O_NONBLOCK` nele valeria para os dois.
    - O lote é zerado depois de mascarado.
  - `ler_saida` (`ward run`/`ward ssh`) e `ler_erro` da ponte (`ponte.rs`) usam o `ler_lote`.
- **Baixo (sshpass + salto):** a ponte agora aceita, além de quem está no grupo **e** na sessão do líder, o filho **direto** do líder que é líder de uma sessão própria (pid = pgrp = sid). A conferência olha o `ppid` e se o líder ainda é líder do grupo e da sessão. É o `ssh` que o `sshpass` cria com `setsid()`. Neto que sai da sessão, processo de fora e filho que não é líder da própria sessão continuam recusados.
  - O `sshpass` falso (`tests/ssh_falso.py`) agora faz como o de verdade: `fork`, `setsid` no filho, espera e sai com o código dele.
  - Com o falso novo e a ponte antiga, `fase28_salto_com_senha_pela_ponte` e `fase28_salto_com_chave_agente_de_cada_ssh` falham (conferido). Com a correção, passam.
- **Baixo (`<rt>ssh-*/` depois de SIGKILL):** `agente_ssh::limpar_pastas`, chamado na partida (`main.rs`) junto da limpeza de `tmp/`.
  - Confere `<rt>` antes (0700, sem link) e apaga só as entradas com o prefixo `ssh-`.
  - Pasta sai com `remove_dir_all`; link ou arquivo sai com `remove_file`. Nenhum dos dois segue link.
- **Texto:**
  - `docs/escopo.md` › S4 alinhado com D34 revisto, como pedido no plano. A nova redação diz que nada é segurado, que um `write` único de até 256 KiB é mascarado inteiro, e que valor cortado entre leituras separadas no tempo sai com o começo em claro, risco aceito pelo usuário.
  - `prd.md`: nota da Fase 28g em D11 (lote e pipe) e em D13 (regra da ponte).
  - `validation-contract.md`: nota em V4.17.
- **Item acrescentado (QA do M4, `teste-cli` com `WARD_DEV_APP`):** fiz as duas coisas pedidas.
  - `dev/daemon.sh` grava na marca `.dev/wardd.app` o socket na 2ª linha. A 1ª linha continua sendo o `WARD_DEV_APP`, e o `head -n 1` de antes não muda.
  - `dev/testes/cli.py` lê a marca no começo. Se o daemon de teste deste checkout está vivo nesse socket com `WARD_DEV_APP`, recusa com: "o daemon de teste está no ar com WARD_DEV_APP … Rode 'make dev-daemon-parar' e suba com 'make dev-daemon WARD_DEV_APP=' antes do 'make teste-cli'". Conferido com uma raiz falsa: sai 1 com a mensagem.
  - A exigência está documentada no `CLAUDE.md` (linha do `make teste-cli`), no comentário do `Makefile` e na docstring do `cli.py`.

## Segurança
- Invariantes tocadas:
  - **S4/D34 revisto:** mais segredo inteiro num pedaço, sem segurar nada.
  - **S8:** o lote de 256 KiB é `Zeroizing` e é zerado a cada lote.
  - **D13/D19:** a regra da ponte foi ampliada só para o filho direto do líder. Nenhum processo de fora vira filho do líder: o pai só muda para um subreaper ou o init, e o pid do líder não é reusado enquanto o grupo existe.
  - **S2/D12:** sobras do agente apagadas na partida.
  - `unsafe` só em `ward-plataforma/src/sys.rs` (`fcntl` `F_SETPIPE_SZ`/`F_GETPIPE_SZ`), com `// SAFETY:` e teste.
- Testes de ataque:
  - `crates/wardd/tests/run.rs` › `f28g_ataque_write_unico_grande_nao_mostra_o_comeco_da_senha`: n de 16 370 a 16 389 e de 65 520 a 65 539, mais 131 067 e 200 000, no stdout e no stderr. A saída tem de ser exatamente `b`×n + `••••\n`, e o log não pode ter a senha. **Falha contra o código antigo** ("fd=1 n=16370: o começo da senha saiu em claro"), conferido.
  - `crates/ward-plataforma/src/filho.rs`:
    - `ataque_write_unico_acima_de_64_kib_chega_inteiro`: uma leitura recebe o `write` inteiro. Se o pipe não foi ampliado, só aceita quando o kernel recusa ampliar um pipe novo; sem a ampliação, o teste falha (conferido).
    - `pipes_de_saida_com_capacidade_ampliada`.
  - `crates/wardd/src/ponte.rs` › `f28g_filho_direto_do_lider_com_sessao_propria`. Recusa processo de fora (pai init ou outro), neto, filho que não é líder da própria sessão, filho em outro grupo da sessão, líder que saiu da sessão e líder ausente. Aceita o `ssh` do `sshpass`.
  - `crates/wardd/src/agente_ssh.rs` › `f28g_partida_apaga_pastas_ssh_que_sobraram`. Um `ssh-*` que é link sai sem tocar no alvo. Com `<rt>` 0777, recusa sem apagar nada.
  - `crates/wardd/tests/ssh.rs` (`fase28_salto_*`) com o `sshpass` falso que faz `setsid`.
- Ao vivo, contra o daemon de teste com socket próprio (`/run/user/1000/ward-28g`, sem `WARD_DEV_APP`): `ward run acme-pg-dev -- python3 -c "os.write(fd, b'b'*n + PGPASSWORD + b'\n')"`, n de 65 520 a 65 539 e de 16 370 a 16 389, fd 1 e 2. Deu 0 casos ruins: nenhum padrão de `.dev/variacoes.txt` e nada além de `b`×n e `••••`.
- Dependências novas: nenhuma.

## Ficou de fora
- **pty (`ward ssh` interativo):** o `ler_lote` ajuda, mas o pty entrega a saída em pedaços de cerca de 4 KiB por um worker do kernel. Um `write` grande ainda pode ser cortado entre leituras. Pela D34 revista nada é esperado, então fica como risco aceito, no mesmo grupo do byte a byte.
- **`write` maior que 256 KiB, ou pipe sem ampliação** (usuário no limite de páginas): o corte volta a acontecer na fronteira da capacidade. Registrado no S4.
- Teste ao vivo de servidor com senha **e** salto pelo `ward ssh`: o cofre de teste não tem essa combinação (`acme-ssh-interno` é por chave, e o `ssh-interno` só aceita chave). Ficou coberto pelo teste Rust com o `sshpass` falso fiel ao de verdade.
- Teste ao vivo do SIGKILL no `wardd` com `<rt>ssh-*` sobrando: coberto pelo teste de unidade e pela chamada em `main.rs`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-plataforma --lib filho` | 0 |
| `cargo test -p wardd --all-features --test run f28g` (código antigo) | 101 (o ataque passava) |
| `cargo test -p wardd --all-features --test run f28g` | 0 |
| `cargo test -p wardd --all-features --test ssh` (falso novo + ponte antiga) | 101 (2 falhas, o achado) |
| `cargo test -p wardd --all-features --test ssh` | 0 |
| `make check` | 0 ("tudo limpo") |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-28g make dev-daemon WARD_DEV_APP=` | 0 |
| reprodução da auditoria (`aud28g.py`, scratchpad) | 0 casos ruins |
| `cli.py` com marca de `WARD_DEV_APP` (raiz falsa) | 1 (recusa no começo, esperado) |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-28g make teste-cli` | 0 ("0 falha(s)"; log do daemon em `trace` sem padrão de `variacoes.txt`) |
| `make dev-daemon-parar` | 0 |

## Problemas descobertos
- O achado médio era mais largo que o descrito: com o buffer de 16 KiB, o corte também acontecia em 16 KiB, e não só em 64 KiB.
- **Mudança de comportamento:** um filho direto do líder que faz `setsid` por conta própria (ex.: `setsid psql` direto do comando do `ward run`) volta a usar o túnel. Um neto, não. A nota da 28e ("descendente que sai da sessão não usa o túnel") fica valendo só para netos.
- `CLAUDE.md`: a linha do `make teste-cli` ainda cita "V4.31/V4.34 num pty" (pendência da 28e). Não mexi nessa parte, só na exigência de `WARD_DEV_APP=`.
- Memória: cada fluxo de saída agora aloca 256 KiB de lote, mais a saída. Com `mlockall` (D62), são cerca de 32 MiB travados no pior caso de 64 conexões com dois fluxos. Antes eram 16 KiB por fluxo.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/ponte.rs` | alterado |
| `crates/wardd/src/agente_ssh.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/wardd/tests/ssh_falso.py` | alterado |
| `dev/daemon.sh` | alterado |
| `dev/testes/cli.py` | alterado |
| `Makefile` | alterado (comentário) |
| `CLAUDE.md` | alterado (linha do `teste-cli`) |
| `docs/escopo.md` | alterado (S4) |
| `docs/mission/prd.md` | alterado (D11, D13) |
| `docs/mission/validation-contract.md` | alterado (nota em V4.17) |
| `docs/mission/handoffs/phase-28g.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (cópia do `.dev/` de teste no worktree, apagada no fim)
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
