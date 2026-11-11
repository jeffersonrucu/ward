# Fase 54a — Correção dos achados da Fase 51 (socket do sistema nos testes, gitleaks)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 83b921e`
  (master atual).
- **(1) Nenhum teste alcança o socket do sistema.**
  - Guarda em todo alvo de teste que roda um binário do Ward ou fala com um daemon:
    `#[cfg(not(feature = "test-mode"))] compile_error!("este teste exige a feature
    test-mode …")`. Sem a feature, a compilação para com essa mensagem, em vez de o
    binário de release do teste falar com `/run/ward/<uid>/wardd.sock`:
    - `crates/ward-nm/tests/binary.rs` (o caso da Fase 51: `no_daemon_answers_internal`
      com o `ward-nm` sem `test-mode` pela unificação de features);
    - `crates/ward/src/tests.rs` (testes da CLI contra servidores falsos);
    - `crates/ward-test-bridge/tests/attacks.rs`, `crates/wardd/tests/test_app.rs` e
      `app_value_binary.rs`: o `#![cfg(feature = "test-mode")]` (pulava calado) virou a
      guarda;
    - `crates/wardd/tests/socket.rs`, `lock.rs`, `run.rs`: esses já tinham
      `required-features = ["test-mode"]` no `crates/wardd/Cargo.toml` (o cargo recusa
      `--test <nome>` e um `cargo test -p wardd` simples os pula); a guarda é a 2ª camada
      se alguém tirar o `required-features`.
  - `cargo test -p ward -p ward-nm -p ward-test-bridge` (a combinação do achado) agora
    não compila, com a mensagem da guarda.
  - `crates/ward/src/tests.rs`: os 7 `ExpectedServer::system()` e os 2
    `system_at("/run/ward/4294967294/wardd.sock")` viraram `unreachable_server()`
    (regras de release num caminho que nunca existe, `/run/ward-nao-existe-falso/1/…`,
    o mesmo dos testes do `ward-platform` e do `app/core`): um teste que conecte por
    engano recebe "o daemon não está rodando", nunca o daemon instalado.
  - Varredura do workspace: os demais testes que citam o caminho do sistema
    (`ward-platform` `server.rs`/`socket.rs`, `app/core` `daemon.rs`) só comparam a string
    do caminho; os do `wardd` sem `test-mode` não têm `LISTEN_FDS` nem conectam (o
    `wardd` é servidor). Confirmado pelo teste de ataque abaixo, que roda todos.
- **Teste de ataque `make test-system-socket`** (`dev/tests/system-socket.sh` +
  `dev/container-system-socket/inside.sh`, reaproveita `dev/container-platform/fake.c`):
  1. estático: todo `.rs` de `crates/` e `app/core` com `CARGO_BIN_EXE_` tem a guarda;
     autoteste com uma cópia de `binary.rs` sem a guarda (tem de ser acusada);
  2. sem `test-mode`, 9 combinações (a do achado, `-p ward-nm`, `-p ward`,
     `-p ward-test-bridge` e os 5 testes do `wardd` que sobem o daemon) não compilam, com
     a mensagem da guarda (ou a recusa do `required-features` do cargo);
  3. monta os binários de teste de `--workspace --all-features` (o conjunto do
     `make check`) e os que compilam sem a feature (`ward-core`, `ward-platform`,
     `ward-app-core`, libs do `wardd` e do `ward-nm`), e o `ward-nm` de release (controle);
  4. num contêiner `ubuntu:24.04` (mesmo digest dos outros) sem rede, com
     `--pids-limit 2048 --memory 5g --cpus 4`, o root **planta um socket do sistema
     falso**: `/run/ward/1000/` do root 0755, `wardd.sock` 0600 do uid 1000, ouvido pelo
     `fake` como uid 0 e não-dumpable (passa todas as conferências de D46 do cliente de
     release). Cada binário de teste roda como uid 1000 com ambiente limpo; o registro do
     falso tem de ficar vazio. No fim, o controle: o `ward-nm` de release com um `fill`
     conecta no falso e o registro pega (prova que o plantio funciona). O `/run` do host
     não é montado: nada do teste alcança o daemon real.
  Evidência em `.dev/logs/system-socket/` (sem segredo). Fora do `make check` (Docker).

- **(2) gitleaks**: os 9 `generic-api-key` da árvore saíram. Cada valor falso novo foi
  trocado por um valor de D15 (já na allowlist, casado inteiro); nada entrou no
  `.gitleaks.toml`. O handoff da Fase 50 ficou sem o valor.

  Literais alterados (regra 5):
  | Arquivo | Antes (forma) | Depois | Motivo |
  | --- | --- | --- | --- |
  | `app/core/src/first_use.rs` (confirmação diferente) | senha mestra falsa fora de D15 | `falso-WP-senha-01` | gitleaks |
  | `crates/ward-core/src/backup/tests.rs` (nova senha) | idem | `falso-SSH-senha-01` | gitleaks |
  | `crates/wardd/tests/d62.rs` (nova senha) | idem | `falso-SALTO-01` | gitleaks |
  | `crates/wardd/tests/backup.rs` (nova senha) | idem | `falso-passphrase-01` | gitleaks |
  | `crates/wardd/tests/settings.rs:111` (troca recusada ao agente/extensão) | idem | `falso-aws-secret-01` | gitleaks |
  | `crates/wardd/tests/settings.rs:461` (troca com o arquivo real) | idem | `falso-licenca-01` | gitleaks |
  | `crates/ward-test-bridge/tests/attacks.rs` (corpo da chave carregada) | base64 de um valor fora de D15 | base64 de `falso-WP-senha-01` (já na allowlist) | gitleaks |
  | `crates/ward-test-bridge/tests/attacks.rs` (confirmação diferente) | senha mestra falsa fora de D15 | `falso-WP-senha-01` | gitleaks |
  | `docs/mission/handoffs/phase-50.md:181` | o valor citado | descrição sem o valor | gitleaks |
  | `crates/ward/src/tests.rs` | `/run/ward/4294967294/wardd.sock` | `/run/ward-nao-existe-falso/1/wardd.sock` | fora do `/run/ward` do sistema |
  | 8 arquivos de teste | — | mensagem do `compile_error!` da guarda | guarda do `test-mode` |

  Todos os valores novos têm ≥ 12 caracteres (mínimo da senha mestra) e diferem da
  senha mestra de teste; os testes só comparam por variável.

## Segurança

- **S1/S5 (D46)**: nenhum teste do workspace chega ao socket do `wardd` instalado; o
  cliente de release só existe nos testes dentro do contêiner, contra o socket falso.
- **S15 / V2.1**: árvore sem achado do gitleaks; allowlist intacta (só D15 inteiro).
- Testes de ataque escritos:
  - `dev/tests/system-socket.sh` (+ `dev/container-system-socket/inside.sh`): socket do
    sistema falso plantado; nenhum dos 59 binários de teste conecta; controle conecta;
    autoteste da varredura estática; recusa de compilação sem `test-mode`.
  - Guarda por `compile_error!` nos 8 arquivos citados (o ataque — compilar o teste sem
    a feature — falha na compilação).
- Dependências novas: nenhuma.

## Ficou de fora

- `app/src-tauri` (workspace próprio, precisa de WebKitGTK): fora do teste de ataque. Os
  testes dele usam `DaemonSocket::system_at` com caminhos de teste (comentário em
  `main.rs:1081`); não vi caminho para o socket do sistema.
- Histórico do git: não se reescreve (pedido). Achados que só existem no histórico
  (`gitleaks git --config .gitleaks.toml`, 12 achados, todos `generic-api-key`, valores
  falsos de teste fora de D15; nenhum segredo real). O usuário decide (reescrever,
  `.gitleaksignore` por fingerprint, ou aceitar): enquanto isso, o passo `gitleaks git` do
  `make check` continua falhando.
  | Commit | Arquivo:linha |
  | --- | --- |
  | `1ae5610` | `crates/ward-test-bridge/tests/attacks.rs:652`, `:786` |
  | `0c09209` | `crates/ward-core/src/backup/tests.rs:369` |
  | `c0d849e` | `crates/wardd/tests/configuracoes.rs:109`, `:459` |
  | `81960fb` | `crates/wardd/tests/backup.rs:437`, `d62.rs:449`, `settings.rs:111`, `settings.rs:465` |
  | `541720e` | `app/core/src/first_use.rs:199`, `:227` |
  | `1b2820b` | `docs/mission/handoffs/phase-50.md:181` |
- Linha da tabela de comandos do `CLAUDE.md` para `make test-system-socket`: não editei o
  `CLAUDE.md` (arquivo de instruções); fica para o orchestrator/usuário.
- Suítes inteiras (`make check`) não rodadas fora do contêiner (regra 6 e a regra do
  pedido: só `test-mode`, nunca no host com risco de socket do sistema).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| gitleaks 8.30.1 (binário do checkout principal, sha256 conferido) `dir --config dev/gitleaks-tree.toml` antes (`.dev/logs/fase54a-leaks-tree-before.log`) | 1 (9 achados) |
| idem depois (`.dev/logs/fase54a-leaks-tree-after.log`) | 0 |
| gitleaks `git --config .gitleaks.toml` (`.dev/logs/fase54a-leaks-history.log`) | 1 (12 achados só no histórico, tabela acima) |
| `make compile` (`.dev/logs/fase54a-make-compile.log`) | 0 |
| `make english-check` (`.dev/logs/fase54a-english-check.log`) | 0 |
| `make fmt-check` | 0 |
| `make test-system-socket` 1ª rodada (`.dev/logs/fase54a-test-system-socket.log`) | 2: o ataque passou (0 conexões, controle ok); o passo 2 não aceitava a recusa do `required-features` do cargo para os testes do `wardd` (corrigido no script) |
| `make test-system-socket` 2ª rodada (`.dev/logs/fase54a-test-system-socket-2.log`) | 0 (0 conexões no socket falso, controle conecta, 9 recusas de compilação, autoteste ok) |

Na rodada do contêiner, 39 dos 59 binários passaram; os 20 que não passaram falham por
falta do que o contêiner não tem (strace/ptrace, dbus-daemon, ssh, Chrome, tempo); só o
registro do socket falso decide o teste.

## Problemas descobertos

- **gitleaks no histórico** (tabela acima): o `make check` (passo `gitleaks git`) segue
  falhando até o usuário decidir.
- `docs/mission/plan.md:392` (Fase 61) ainda cita o valor falso que saiu do handoff da
  Fase 50; o gitleaks não acusa (sem contexto de atribuição), e o plano é do orchestrator.
- Os outros testes do `wardd` sob `required-features` (`ssh`, `login`, `playwright`, com
  `test-hooks`) e `ward-platform/tests/{activation,identification}.rs`
  (`#![cfg(feature = "test-mode")]`) pulam calados sem a feature; não falam com o socket
  do sistema, então ficaram como estão.
- Conflito possível com a Fase 53 (paralela): só a guarda no topo de
  `crates/wardd/tests/run.rs`, `socket.rs`, `lock.rs` e duas linhas de `settings.rs`,
  `d62.rs`, `backup.rs`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Makefile` (alvo `test-system-socket`) | alterado |
| `dev/tests/system-socket.sh` | criado |
| `dev/container-system-socket/inside.sh` | criado |
| `crates/ward-nm/tests/binary.rs` | alterado |
| `crates/ward/src/tests.rs` | alterado |
| `crates/ward-test-bridge/tests/attacks.rs` | alterado |
| `crates/wardd/tests/{socket,lock,run,test_app,app_value_binary,settings,d62,backup}.rs` | alterado |
| `crates/ward-core/src/backup/tests.rs` | alterado |
| `app/core/src/first_use.rs` | alterado |
| `docs/mission/handoffs/phase-50.md` | alterado |
| `docs/mission/handoffs/phase-54a.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6); `make compile`, `fmt-check`, `english-check`,
  gitleaks e `make test-system-socket` rodados
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (S1/S5: `make test-system-socket`; S15: gitleaks)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
