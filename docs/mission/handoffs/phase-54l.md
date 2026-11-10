# Fase 54l — Achados da 54c: FIFO na leitura do cofre; fim do `ward ssh` sem `ssh` órfão

## Implementado
- **`arquivos::abrir_leitura`** (`crates/wardd/src/arquivos.rs`), usada por `ler_cofre`,
  `identidade` e `ler_estado_completo`, abre agora com
  `O_NOFOLLOW | O_NONBLOCK | O_NOCTTY | O_CLOEXEC`. Um FIFO no lugar do cofre, do estado
  ou de uma versão do backup abre na hora. O `fstat` (`conferir_arquivo`, só arquivo
  comum do uid) recusa com `PermissionDenied`, e a thread não fica presa. Isso fecha a
  lista do restaurar (`nucleo::backups`, que segura o estado do daemon), o
  `restaurar_backup` e a leitura das versões locais na cópia externa (`externo::copiar`).
  O `O_NOCTTY` impede que um terminal posto no lugar vire o terminal do daemon. Num
  arquivo comum, nenhuma das duas flags muda a leitura.
- **Fim da sessão do `ward run`/`ward ssh`** (`crates/wardd/src/execucao.rs`). Todo
  `grupo.matar()` passou a `grupo.matar_com_sessoes_filhas()`, a mesma função da trava
  (54c). Isso cobre a CLI que cai (ler o quadro falha, ou `par_fechou` com a fila
  cheia), o dado na entrada declarada nula, o quadro fora da sessão, o pty sem clone,
  o leitor da CLI que não sobe e o fim do filho. O `ssh` que o `sshpass` cria com
  `setsid()` não depende mais do `SIGHUP` do pty do `sshpass`.
- **`Drop for Grupo`** (`crates/ward-plataforma/src/filho.rs`) também usa
  `matar_com_sessoes_filhas`. Assim nenhum caminho de erro que solta o grupo sem colher
  deixa esse `ssh` para trás.
- Docs: módulo `ssh.rs` (item "Fim da sessão"), comentário de `ler_da_cli`, contrato
  V4.26 (a CLI que cai por senha), cabeçalho do `dev/testes/cli.py`.

## Segurança
- Invariantes tocadas:
  - **S8 / D13**: o que recebeu segredo (a sessão SSH autenticada por senha) não
    sobrevive à CLI que caiu. Só levam o sinal os filhos diretos do líder com pid =
    grupo = sessão, então nenhum processo de fora é atingido (teste da 54c segue valendo).
  - **S1**: o agente de D12 continua sumindo no fim (`rt_sem_agente`).
  - **S6/S7** (backup/restaurar): a leitura não segue link, não fica presa em FIFO e só
    aceita arquivo comum do uid. Um DoS do mesmo uid no daemon inteiro (estado preso)
    deixa de existir.
  - **S13**: nenhum log novo.
- Testes de ataque escritos (os três falham com o código antigo; controles rodados):
  - `crates/wardd/src/arquivos.rs` › `f54l_ataque_fifo_no_lugar_nao_prende_a_leitura`:
    FIFO no lugar, `ler_cofre`/`identidade`/`ler_estado_completo` voltam em < 3 s com
    recusa; controle com arquivo comum lido inteiro. Sem `O_NONBLOCK`: preso (falha).
  - `crates/wardd/tests/backup.rs` ›
    `f54l_ataque_fifo_trocado_no_lugar_da_versao_nao_prende_o_daemon`: uma thread troca
    a versão por FIFO e de volta sem parar (rename atômico) por 3 s enquanto o app pede
    a lista dos backups. Cada lista volta em < 5 s, e no fim a versão real confere. Sem
    `O_NONBLOCK`: "a lista dos backups ficou presa num FIFO (o daemon parou)".
  - `crates/wardd/tests/ssh.rs` › `f54l_ataque_cli_cai_no_ward_ssh_nenhum_ssh_sobra`:
    senha com pipes, senha com pty e chave. A CLI fecha o socket no meio da sessão
    (`ssh` falso parado 30 s, que não morre por `SIGHUP`), e nenhum `ssh`/`sshpass`
    sobra em 5 s. Com `matar()` no lugar: "s-livre: ssh … sobreviveu à CLI que caiu".
  - `dev/testes/cli.py` (V4.26 no `teste-cli`): SIGKILL na CLI de `ward ssh
    acme-ssh-chave-prod 'sleep 20'` e `acme-ssh-senha-stage 'sleep 23'` contra os SSH de
    teste. Nenhum `/usr/bin/ssh`/`sshpass` da sessão sobra, e o agente some na chave.
    Com o OpenSSH real o `SIGHUP` também mataria; o ataque de verdade é o teste em Rust.
- Dependências novas: nenhuma.

## Ficou de fora
- `ponte.rs` e `playwright.rs` seguem com `matar()` nos próprios caminhos (o `Drop` do
  `Grupo` já cobre o fim). Nenhum dos dois cria filho com `setsid` conhecido. Não
  entram no achado.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test --workspace --all-features --locked --test ssh --test backup -- f54l` | 0 |
| idem, e `cargo test -p wardd --lib -- f54l`, com o código antigo (controle) | 101 (esperado) |
| `make check` (957 testes) | 0 |
| `make dev-servicos-subir` / `make dev-servicos-parar` | 0 / 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54l make dev-daemon WARD_DEV_APP=` / `make dev-daemon-parar` | 0 / 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54l make teste-cli` | 0 (0 falhas; as duas checagens 54l ok) |
| `git checkout -- docs/mission/evidence/fase-4[123]` | 0 |

## Problemas descobertos
- Nenhum novo. O `.dev/known_hosts` já estava igual ao `.dev/servicos/known_hosts` e
  não foi recopiado. Disco: 2,7 GB livres no fim, nada apagado. Serviços de teste e
  daemon parados; `docs/mission/evidence/fase-4[123]` restaurado.
- `docs/escopo.md` e o PRD não foram tocados (a 54g mexe em "Fora do modelo de ameaça"
  e no RA11 em paralelo).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado (só doc) |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/wardd/tests/backup.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `dev/testes/cli.py` | alterado |
| `docs/mission/validation-contract.md` | alterado (V4.26) |
| `docs/mission/handoffs/phase-54l.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
