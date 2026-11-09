# Fase 25 — CLI `ward`: `lista [--json]`, `travar`, `destravar`

## Implementado
- `crates/ward` agora é lib + binário, com `#![forbid(unsafe_code)]`.
  - `args.rs`: `lista [--json]`, `travar`, `destravar`, `--help`/`-h`/`ajuda` e `--version`/`-V`. Uso errado sai com 2. A mensagem de erro nunca repete o argumento: `ward destravar <senha>` digitado por engano não devolve a senha.
  - `cliente.rs`: `Conexao` sempre fala no papel agente.
    - Conecta só por `ward_plataforma::conectar_servidor`. No release usa `ServidorEsperado::sistema()` (`/run/ward/<uid>/wardd.sock`: pastas do root, socket 0600, par com uid 0). Nenhuma variável nem argumento troca isso.
    - No `modo-teste` usa `teste_em($WARD_DEV_SOCKET_DIR/wardd.sock)` ou `teste()`. Como esse build pula o uid 0 (D46), ele exige também que o par seja **não-dumpable** como o `wardd` (`/proc/<pid>/exe` dá EACCES), pelo pidfd do `ProcessoPar`, e depois chama `reconferir_sem_depurador`. Com isso, o falso em Python do V4.2 é recusado também no build de teste.
    - Prazos: 130 s de leitura e 10 s de escrita.
    - A resposta é lida com `decodificar_resposta(Papel::Agente)`. Uma resposta com credencial, ou uma lista com nível, é recusada (S1, D50).
  - `comandos.rs`: o trait `Entorno` (conectar, endurecer, ler senha, stdout/stderr) deixa os testes trocarem o mundo de fora. O `destravar` segue esta ordem:
    1. conecta e confere;
    2. manda `estado`: se já está destravado, sai 0 sem ler a senha; se não há cofre, dá erro;
    3. chama `endurecer_processo` (não-dumpable, sem core). Se falhar, aborta antes de ler;
    4. lê a senha do `/dev/tty`. Vazia, cancelada, sem terminal ou longa demais: sai sem mandar nada;
    5. `reconferir()` no mesmo socket;
    6. manda `destravar`. A senha passa do `Zeroizing` para o `Segredo` com `mem::take`, sem cópia.
  - `texto.rs`: tabela alinhada e JSON.
    - `lista --json` é um array com **só** `nome`, `tipo`, `grupo`, `ambiente`, `projeto` e `endereco` (URL ou host), com `null` no campo vazio. Não leva tags nem nível.
    - Controles C0/C1/DEL e formatação Unicode (bidi, largura zero, tags, BOM) saem como escape visível no texto e como `\uXXXX` no JSON (regra de D51).
  - Mensagens em português, todas com o prefixo `ward:`:
    - "o daemon não está rodando";
    - "servidor não é o wardd (<motivo>); nada foi enviado";
    - "travado: destrave com `ward destravar` ou pelo app";
    - "cofre travado", "cofre destravado", "o cofre já está destravado";
    - a mensagem fixa do `ErroResposta` (ex.: "senha mestra incorreta ou arquivo do cofre adulterado").
- `ward-plataforma::terminal` (módulo novo) e `ler_senha(prompt, max)`:
  - abre `/dev/tty` com `O_NOCTTY|O_CLOEXEC` e nunca lê de stdin nem de argv;
  - desliga `ECHO`/`ECHONL`/`ICANON`/`ISIG`/`IEXTEN` com `TCSAFLUSH`, que descarta o que foi digitado antes do prompt, e confere de volta que o eco desligou;
  - restaura o terminal no `Drop`;
  - edição mínima: Backspace e Ctrl+U zeram o byte antes de tirá-lo; Ctrl+C e Ctrl+D com a linha vazia cancelam;
  - o buffer é `Zeroizing<Vec<u8>>` com a capacidade toda reservada antes (`MAX_SEGREDO`), então não realoca.
  - `sys.rs` ganhou `tcgetattr`/`tcsetattr` e, só em teste, `abrir_pty`. Todos têm `// SAFETY:`.
- `make teste-cli` (`dev/testes/cli.py`): a CLI `modo-teste` num pty de verdade contra um falso em Python e contra o `make dev-daemon`. A linha está no `CLAUDE.md`.
- Notas do planner tratadas:
  - **D62:** o prazo de leitura de 130 s cobre a espera máxima do D20 (30 s, também na primeira tentativa depois do reinício) mais o Argon2id.
  - **D63:** os `match` de `Resposta::Estado` usam `..`, então o campo `papel` da Fase 23 não quebra a CLI, e quem decodifica é o `ward-core`. Nos testes, só `resposta_estado()` em `testes.rs` monta `Resposta::Estado`; é o único ponto a ajustar no merge.

## Segurança
- Invariantes tocadas:
  - **S5 (D46):** toda conexão, e não só a do `destravar`, confere o servidor antes do primeiro byte. A conferência roda de novo logo antes de a senha sair. Um falso nunca recebe a senha, nem consegue alimentar `lista` falsa.
  - **S1:** a CLI só aceita respostas do papel agente. Não existe comando que peça segredo, e nenhuma mensagem repete argumento nem conteúdo do quadro.
  - **S3:** a senha vem só do `/dev/tty`.
  - **S8:** o processo fica não-dumpable antes de a senha existir. A senha fica em `Zeroizing` e depois em `Segredo`, sem realocar, e o byte apagado é zerado.
  - **S13:** os erros são textos fixos (tipo de `io::ErrorKind`, motivo da `RecusaServidor`).
- Testes de ataque:
  - `crates/ward/src/testes.rs`:
    - `v4_2_ataque_servidor_falso_recusado_antes_de_ler_a_senha`: regra do release com o falso numa pasta do usuário. `destravar`/`lista`/`travar` recusam, a senha não é lida e o falso tem 0 conexões e 0 bytes;
    - `v4_2_ataque_servidor_falso_no_socket_de_teste_recusado`: regra de teste com o falso dumpable. Ele vê a conexão e recebe 0 bytes, e o prompt não aparece;
    - `com_daemon_falso::ataque_socket_trocado_enquanto_digita`: o socket vira 0666 durante a digitação; a reconferência recusa e o falso só viu o `estado`;
    - `com_daemon_falso::ataque_resposta_com_credencial_ou_nivel_recusada`: `segredo`, `preenchimento`, lista com `nivel` e `v:2`, todos recusados sem saída;
    - também: `destravar_na_ordem`, `destravar_senha_errada_vazia_cancelada`, `v4_1_lista_em_texto_e_json`, `v4_22_travar_e_lista_responde_travado`, `uso_e_ajuda` e `daemon_ausente`.
  - `crates/ward/src/args.rs › ataque_argumento_nao_volta_na_mensagem`.
  - `crates/ward/src/texto.rs › ataque_texto_com_controle_bidi_e_aspas`.
  - `crates/ward-plataforma/src/terminal.rs`:
    - `ataque_senha_nao_ecoa_no_pty`: pty real; a senha não aparece no mestre, o que foi digitado antes do prompt não vira senha e o eco, o `ICANON` e o `ISIG` voltam;
    - `ctrl_c_cancela_e_restaura`, `edicao_da_linha`, `cancelar_e_limite`, `capacidade_reservada_sem_realocar` e `arquivo_comum_nao_e_terminal`.
  - `make teste-cli`, com 21/21 ok:
    - V4.2: falso em Python no socket de teste; recusa sem prompt, o falso recebe 0 bytes e a senha não está no registro;
    - V4.22;
    - senha errada e certa pelo pty, sem eco;
    - V4.1: 20 entradas (as 21 da fixture menos `gama-bloqueada`), só os seis campos e nenhum padrão de `.dev/variacoes.txt` na saída.
- Dependências: nenhuma crate nova no lockfile. O `Cargo.lock` ganhou só as arestas de `ward` para `ward-core`, `ward-plataforma` e `zeroize`.
  - `ward-core`: protocolo.
  - `ward-plataforma`: conferência do servidor e terminal.
  - `zeroize`: senha. O `ward-plataforma` passou a ligar `zeroize/alloc`, para o `Zeroizing<Vec<u8>>`.

## Ficou de fora
- `run`, `ssh`, `login` e `playwright` (Fases 26–28 e seguintes). Hoje respondem "comando desconhecido" e não aparecem no `--help`.
- Conferir o campo `papel` do `estado` (D63) na CLI. Ele ainda não existe; quando a Fase 23 chegar, a CLI pode exigir `agente`. Não é obrigatório.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-cofre`, `make dev-variacoes` | 0 |
| `cargo test -p ward --all-features` / `cargo test -p ward` (sem feature) | 0 / 0 |
| `cargo test -p ward-plataforma --lib terminal` | 0 |
| `cargo clippy -p ward -p ward-plataforma --all-targets [--all-features] -- -D warnings` | 0 |
| `make dev-daemon` | 0 |
| `make teste-cli` | 0 (21 ok, 0 falhas) |
| `ward lista` / `travar` / `lista` (travado → exit 1) / `destravar` sem tty (exit 1) / `make dev-daemon-parar` / `lista` sem daemon (exit 1) | conforme esperado |
| `make check` | 0 |

## Problemas descobertos
- **O V4.2 inteiro só vale no artefato de release.** No `modo-teste`, a CLI pula o uid 0 (D46), e a conferência extra (par não-dumpable) só barra um falso ingênuo. Um falso que chame `prctl(PR_SET_DUMPABLE, 0)`, ou o próprio `wardd` de teste copiado para `/tmp` escutando no socket de teste, passa. No release, o socket fica em `/run/ward/<uid>/`, que é do root, e o usuário nem consegue criar o falso lá; sem daemon, a CLI diz "o daemon não está rodando", e não "servidor não é o wardd". O validador deve rodar a segunda parte do V4.2 contra o build sem feature (por exemplo no contêiner da 21b), ou o texto do V4.2 precisa ser ajustado pelo planner.
- **Terminal com eco desligado se a CLI morrer por sinal** (SIGTERM/SIGHUP/SIGKILL) durante a digitação. O Ctrl+C não tem esse problema (o `ISIG` está desligado e o byte cancela). O `reset` resolve. Não instalei handler de sinal.
- **Teclas especiais** (setas, ESC) entram na senha como os bytes depois do ESC (ex.: `[A`). O ESC em si é ignorado.
- **A CLI não confere o próprio ambiente** (`LD_PRELOAD` etc.) antes de ler a senha. Quem controla o ambiente do shell do usuário já pode trocar o `ward` por outro programa, então isso fica fora do modelo. Registro.
- **Mudança fora de `crates/ward`:** o módulo `terminal` e duas funções em `sys.rs` do `ward-plataforma` (mais o `abrir_pty` de teste), porque o `termios` exige `unsafe`, que só pode viver ali. O protocolo do `ward-core` não mudou.
- No começo fiz um commit WIP e um merge do `master` no branch; no fim reescrevi tudo num commit único em cima do `master`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward/Cargo.toml` | alterado |
| `crates/ward/src/main.rs` | alterado |
| `crates/ward/src/lib.rs` | criado |
| `crates/ward/src/args.rs` | criado |
| `crates/ward/src/cliente.rs` | criado |
| `crates/ward/src/comandos.rs` | criado |
| `crates/ward/src/texto.rs` | criado |
| `crates/ward/src/testes.rs` | criado |
| `crates/ward-plataforma/Cargo.toml` | alterado (`zeroize/alloc`) |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/terminal.rs` | criado |
| `dev/testes/cli.py` | criado |
| `Makefile` | alterado (`teste-cli`) |
| `CLAUDE.md` | alterado (só a linha do `teste-cli`) |
| `Cargo.lock` | alterado (só as arestas do `ward`) |
| `docs/mission/handoffs/phase-25.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica: fase sem UI)
- [x] Ficou dentro do escopo da fase
