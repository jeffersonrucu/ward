# Fase 49 — Migração no daemon e estado v2 (Missão 2, Milestone 4; DT5, DT6)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, `git reset --hard 5663000`
  (master atual, Fase 48 concluída).
- **`crates/ward-core/src/body.rs`** (achado da Fase 48: a API não dizia se o corpo lido era
  v4):
  - `BodyRead { generation, version }` e `BodyRead::is_legacy()`, sem dado do cofre.
  - `open_entries_with_version(file, password, min, burned)` devolve `(OpenVault,
    Vec<Entry>, BodyRead)`. As conferências são as mesmas: corpo validado inteiro, depois
    o piso de retrocesso. A versão não decide nada no núcleo.
  - `open_entries_with_generation` passa a delegar a ela (mesma assinatura, mesmo
    resultado). `deserialize` delega à nova `deserialize_with_version` (privada), que
    devolve a versão lida de `read_header`.
  - Cripto, formato, gravação e tabela v4 → v5 não mudaram.
- **`crates/wardd/src/files.rs`** (DT6):
  - `STATE_HEADER` = `ward-state 2`, `LEGACY_STATE_HEADER` = `ward-estado 1`.
  - Tabela fechada `StateKeys` por formato:
    - `STATE_KEYS`: `seen`, `burned`, `agent_failures`, as três obrigatórias;
    - `LEGACY_STATE_KEYS`: `vista`, `queimada`, `falhas_agente`, esta opcional (formato da
      Fase 22).
  - `parse_state` escolhe as chaves pelo cabeçalho. Chave do outro formato ou qualquer
    outro texto recusa o arquivo inteiro.
  - `state_text` só gera `ward-state 2`.
  - Leitura estrita, `burned ≥ seen`, `O_NOFOLLOW|O_NONBLOCK`, 0600 e gravação atômica:
    tudo como antes.
- **`crates/wardd/src/core.rs`** (DT5):
  - `unlock` abre por `open_entries_with_version`. Com corpo v4 (`is_legacy`), chama
    `Daemon::migrate` antes de pôr o cofre aberto no estado e antes de responder `ok`.
  - `migrate` grava pelo `write_vault` de sempre:
    - confere a identidade do arquivo;
    - grava `burned` = geração + 1 no estado;
    - `write_with_backup` (temporário `O_EXCL`, fsync, rename, fsync da pasta, versão no
      backup local);
    - `confirm_write` (`seen` = geração nova).
  - Erro de gravação: o destrave responde o erro (`write_failed`) e fica travado, com nada
    apagado. `known_identity` fica com a identidade do arquivo no lugar: a v4, ou a v5 se
    só o fsync da pasta falhou. Arquivo trocado no meio: `vault_changed`.
  - Um v5 nunca passa por `migrate`.
  - Os contadores de senha errada zeram mesmo se a migração falhar: a senha estava certa,
    e o disco já tinha zerado.
  - `write_vault` ganhou o parâmetro `points: Option<&MigrationHook>`. As outras gravações
    passam `None`.
  - `MigrationPoint { BeforeBurn, BeforeWrite, BeforeConfirm, AfterConfirm }` e
    `MigrationHook`, mais `Daemon::with_migration_hook`, só com a feature `test-hooks`
    (V3.15). O gancho só observa ou para o processo; não muda a gravação.
  - Log da migração só com texto fixo:
    - "cofre no formato da 0.1.x (corpo v4): gravando no formato novo (v5)";
    - "cofre migrado para o corpo v5";
    - "migração para o corpo v5 falhou: cofre travado, nada apagado";
    - "migração recusada: o arquivo do cofre mudou fora do Ward (D39)".
  - Documentação: bullet "Migration" no topo do módulo; o restaurar diz que uma versão v4
    é gravada como v5. Três comentários `vista` → `seen`.
- **Restaurar v4 grava v5** e **rotação local/pendrive lendo v4 e v5**: o código já fazia.
  O restaurar grava por `write_entries_with_backup`, que só gera v5. A rotação usa
  `sealed_generation` → `body_generation` → `check_structure`, que aceita os dois. Sem
  mudança de código; só testes (abaixo).
- **Scripts que criam estado de teste** passam a escrever
  `ward-state 2\nseen 0\nburned 0\nagent_failures 0\n`, a pedido do achado da Fase 46:
  - `dev/daemon.sh`;
  - `dev/container-{install,platform,units}/inside.sh`;
  - `dev/tests/{extension,login,nm,playwright,socket-attacks}.sh`;
  - `dev/tests/{chrome_policy,ssh_home}.py`.
- **`tests/approval.rs` (V3.15)**: `TEST_HOOKS` ganha `with_migration_hook` (5 ganchos).
- **`tests/d62.rs`**: a regravação de um estado no formato antigo agora espera
  `ward-state 2`.
- **`docs/glossario.md`** (seção 11): regra das linhas obrigatórias e os nomes novos do
  código.
- **`frozen.txt`**: não havia entrada da Fase 49. Nada a tirar, e o `english-check` do
  repositório dá 0 achado.

## Segurança

- **Invariantes tocadas.**
  - S7/RM2: a migração usa a mesma gravação atômica e a mesma ordem de gerações (D53) de
    toda gravação; nenhum caminho novo de escrita.
  - O v4 só é substituído pelo rename de um v5 completo e cifrado. Em erro antes do rename,
    o v4 fica byte a byte.
  - A geração queimada nunca volta: uma falha depois de queimar a 4 faz a próxima migração
    selar a 5.
  - S6: cripto intacta, nenhuma dependência nova. O núcleo só expõe a versão lida (um
    `u16`).
  - S2/S13: nada em claro no disco (o v5 é selado; o estado só tem números). O log da
    migração só tem texto fixo.
  - D39/D53: a cópia v4 posta no lugar depois da migração dá `rollback` (o estado guarda a
    4). Trocar com o cofre aberto dá `vault_changed` na próxima gravação.
  - DT6: formatos de estado fechados e sem mistura. Link, FIFO e modo frouxo continuam
    recusados.
- **Testes de ataque e de caminho escritos.** Não rodados (regra 6), salvo os de unidade
  listados em "Comandos".
  - `crates/wardd/src/files.rs`:
    - `dt6_v1_and_v2_read_the_same_and_the_write_is_v2`;
    - `dt6_attack_mixed_or_forged_state_refused` (21 formas forjadas ou misturadas);
    - `dt6_v1_state_on_disk_becomes_v2_and_strange_files_are_refused` (V4.11: 0600,
      atômico, sem temporário, modo frouxo, link e FIFO nos dois formatos);
    - `strict_state` com um caso v2.
  - `crates/ward-core/src/atomic_write/tests.rs`:
    - `dt5_sigkill_at_each_point_v4_to_v5` (V4.3 no núcleo): SIGKILL depois de criar o
      temporário, depois do write, depois do fsync, depois do rename e depois do fsync da
      pasta, com o `vault-0.1.4.ward` no lugar e a forma v5 dele. O resultado é a v4 byte a
      byte ou a v5 inteira. O temporário é cifrado, prefixo do v5 e 0600. A v5 relê as
      mesmas entradas.
    - `dt5_attack_migration_bytes_have_no_clear_name` (V4.4): nenhum nome de tipo ou nível
      v4/v5 nem nome de entrada em claro.
  - `crates/wardd/tests/migration.rs` (novo):
    - `v4_3_first_unlock_writes_the_v5_with_the_same_data` (V4.3/V4.5/V4.11): mesmos
      configuração e entradas pelo corpo canônico, PIN, 240 min, backup externo desligado,
      geração 4, `ward-state 2` 4/4, v5 no backup, 0600, sem temporário, sem nome ou senha
      em claro.
    - `v4_6_migrated_vault_is_not_written_again`: travar e destravar 3× e um daemon novo;
      identidade, bytes, estado e backups iguais.
    - `v4_7_attack_v4_copy_after_the_migration`: `vault_changed` com o cofre aberto e
      `rollback` no mesmo daemon e num novo; a v4 não é regravada.
    - `v4_1_wrong_password_on_the_v4_writes_no_vault`.
    - `v4_3_write_failure_keeps_the_v4_and_answers_the_error`: pasta do cofre sem escrita
      dá `write_failed`, v4 intacta e estado 3/4. Corrigida, a migração sela a 5, uma vez.
    - `v4_3_state_failure_keeps_the_v4_and_the_old_state`.
    - `v4_11_old_state_with_failures_and_mixed_state`.
    - `v4_10_restoring_a_v4_version_writes_the_v5`.
    - `v4_10_local_rotation_counts_v4_and_v5`: lixo e cópia renomeada não ocupam lugar; as
      v4 saem quando as 30 maiores gerações são v5; o lixo fica.
    - `v4_10_usb_drive_rotation_counts_v4_and_v5`.
    - `death::v4_3_sigkill_at_each_migration_point` (feature `test-hooks`): processo filho
      com `with_migration_hook`, SIGKILL em `BeforeBurn`, `BeforeWrite`, `BeforeConfirm` e
      `AfterConfirm`. O arquivo é a v4 ou a v5 inteira. O estado sempre lê, sem `seen`
      acima da geração do disco. O próximo destrave funciona com a geração esperada (4 ou
      5), sem temporário, e não regrava. O log do daemon morto não tem senha, PIN, nome de
      entrada nem `falso-`.
  - `crates/wardd/tests/approval.rs`: o V3.15 cobre o gancho novo (só com a feature e só
    antes do `pub fn`).
- **Dependências novas**: nenhuma.

## Ficou de fora

- **A parte de contêiner do V4.4** (`inotifywait`/`strace -f` no contêiner do
  `test-units`) não foi escrita: precisa do contêiner e é do Milestone 6. Os testes de
  bytes acima cobrem o conteúdo dos arquivos criados.
- **V4.12** (o `wardd` 0.1.4 da linha de base recusa o v5): é da Fase 64, por DT7.
- **Testes do protocolo**: os novos testes do `wardd` usam o protocolo v1 de hoje
  (`destravar`, `tempo_trava`, `restaurar_backup`, `definir_backup_externo`, `lista`,
  `backups`), como os outros testes do `wardd`. Passam aos nomes v2 nas Fases 50/51.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 5663000` (árvore limpa) | 0 |
| `cargo check -p wardd -p ward-core --all-targets --all-features --locked` (log `.dev/logs/fase49-check.log`) | 0 |
| `cargo fmt --all` e `cargo fmt --all -- --check` | 0 |
| `cargo clippy -p wardd -p ward-core --all-targets --all-features --locked -- -D warnings` (log `fase49-clippy.log`) | 0 |
| `cargo clippy -p wardd --lib --bins --locked -- -D warnings` (sem features, release; log `fase49-clippy-release.log`) | 0 |
| `cargo test -p wardd --lib files::tests` (7 testes, de unidade; log `fase49-test-files.log`) | 0 |
| `cargo test -p ward-core --lib atomic_write::tests::dt5` (2 testes; log `fase49-test-core-dt5.log`) | 0 |
| `cargo test -p wardd --test migration v4_3_first_unlock` (1 teste, para provar o fluxo; log `fase49-test-migration-first.log`) | 0 |
| `make compile` (log `.dev/logs/fase49-make-compile.log`; 48 shell, 35 Python, 7 JS, 0 erro, 0 warning) | 0 |
| `make english-check` (repositório inteiro): 0 achado, 0 problema nas exceções | 0 |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos

- **V4.1 pelo agente grava o estado.** Uma senha errada no papel agente grava a falha no
  estado (D62), já como `ward-state 2`. O "não grava nada" do V4.1 vale para o cofre e para
  o papel app; a pasta do cofre muda pelo estado no caso do agente (no teste, cofre e estado
  ficam na mesma pasta `.dev/`). O validador deve testar o V4.1 pelo app ou olhar só o
  arquivo do cofre.
- **V4.7 com o daemon rodando.** `vault_changed` só aparece com o cofre destravado e uma
  gravação depois da troca. Travado, o destrave dá `rollback`: a conferência da geração vem
  antes da identidade no `unlock`, ordem que já existia. O teste cobre as duas formas.
- **A migração não guarda a v4 atual no backup antes de gravar.** DT5 só diz que "as
  versões v4 do backup ficam", e elas ficam. A volta para a 0.1.x (DT7) depende de uma v4
  no backup. Se o backup local da 0.1.x não funcionava, não há nenhuma; e com 30 versões, a
  rotação tira as v4 depois de 30 gravações. Sugestão para o orchestrator/planner: decidir
  se a migração chama `backup::keep_current` antes (como o restaurar) e se o roteiro de
  `docs/instalacao.md` manda copiar o cofre antes de atualizar.
- **Nomes v5 mais longos.** `database` +3, `server` −2, `approval` −5… Um v4 no limite exato
  de `MAX_BODY` poderia não caber como v5. Nesse caso a migração recusa ("o cofre passaria
  do limite de tamanho"), o v4 fica e o destrave falha sempre na 0.2.0. Não acontece na
  prática (+3 bytes por entrada de banco, no máximo 10 000 entradas), mas fica registrado.
- O `make dev-daemon`, `test-cli` e os outros alvos de teste criam agora `ward-state 2`. Um
  daemon 0.1.4 (Fase 64) recusaria esse estado como inválido. Se a Fase 64 subir o 0.1.4
  com um estado criado por script, precisa escrever `ward-estado 1` (o fixture
  `state-0.1.4` serve).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/body.rs` | alterado |
| `crates/ward-core/src/atomic_write/tests.rs` | alterado (testes) |
| `crates/wardd/src/core.rs` | alterado |
| `crates/wardd/src/files.rs` | alterado |
| `crates/wardd/tests/migration.rs` | criado |
| `crates/wardd/tests/approval.rs` | alterado (V3.15) |
| `crates/wardd/tests/d62.rs` | alterado |
| `dev/daemon.sh` | alterado |
| `dev/container-install/inside.sh` | alterado |
| `dev/container-platform/inside.sh` | alterado |
| `dev/container-units/inside.sh` | alterado |
| `dev/tests/extension.sh` | alterado |
| `dev/tests/login.sh` | alterado |
| `dev/tests/nm.sh` | alterado |
| `dev/tests/playwright.sh` | alterado |
| `dev/tests/socket-attacks.sh` | alterado |
| `dev/tests/chrome_policy.py` | alterado |
| `dev/tests/ssh_home.py` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-49.md` | criado |

**Literais alterados (regra 5):**

- O cabeçalho e as chaves gravadas do estado passam a `ward-state 2`, `seen`, `burned` e
  `agent_failures`. `ward-estado 1`, `vista`, `queimada` e `falhas_agente` continuam como
  literais da leitura.
- As expectativas de texto de estado nos testes (`files.rs`, `d62.rs`).
- O estado inicial dos scripts de teste.
- Quatro mensagens de log novas, em pt-BR, só com texto fixo.
- Conferido por diff: nenhum escape `\u`/`\x`/`\0`/`\t` perdido ou mudado.

## Procedimentos
- [ ] `make check` rodado — não, por regra 6 do PRD; rodaram `make compile` (0), `cargo fmt --check`, `cargo clippy -D warnings` com e sem features (0) e 10 testes de unidade/um de integração
- [x] Só cofre de teste e credenciais falsas usados (fixtures 0.1.4 copiados para `target/tmp`; nenhum cofre real nem o de `.dev/`)
- [x] Teste de ataque para cada (SEG) tocado (V4.3, V4.4 em bytes, V4.7; V4.1 no daemon)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
