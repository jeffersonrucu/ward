# Fase 48 — Corpo v5 no `ward-core` (DT4)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 1c4a963`
  (master da época). No fim, `git merge master` (Fases 34a e 47b), sem conflito.
- **`crates/ward-core/src/entry.rs`** — `name()`/`from_name()` com os nomes v5:
  - `EntryType`: `database`, `server`, `secure-note`, `generic` (os outros iguais);
  - `AccessLevel`: `blocked`, `approval`, `free`;
  - `SmtpSecurity`: `none` (`starttls`, `tls` iguais).
  - `EntryType::label()` deixou de cair em `name()` (`other => other.name()`) e lista os 9
    rótulos pt-BR por extenso (`login`, `banco`, `servidor`, `api`, `aws`, `smtp`, `ftp/sftp`,
    `nota segura`, `genérico`): sem isso o rótulo de banco/servidor viraria inglês.
- **`crates/ward-core/src/body.rs`**:
  - `BODY_VERSION` 4 → 5 (a única gravada);
  - `LEGACY_BODY_VERSION = 4` (só leitura);
  - enum privado `Version { V4, V5 }`: `read_header` devolve a versão (5 ou 4; o resto é
    `Invalid`) e `read_raw(l, version)` escolhe a tabela dos nomes;
  - no v4, a segurança SMTP vira o nome v5 já na 1ª passada (constante `SMTP_SECURITY` = 4,
    posição no `schema`); fora da tabela, `Invalid`;
  - documentação do módulo com a seção "Version 5".
  - A gravação não mudou: continua pelo `name()` de cada enum, e por isso só gera v5,
    inclusive o `change_password`, que lê e reescreve o corpo.
- **`crates/ward-core/src/body/legacy_v4.rs`** (novo) — tabela fechada v4 → v5:
  - `kind`, `level` e `smtp_security`, por `match` nos bytes exatos;
  - módulo privado do `body`, chamado só por `stored_kind`/`stored_level`/`read_raw` com
    `Version::V4`;
  - nenhum outro caminho (protocolo, CLI, app, validação de rascunho) conhece os nomes v4.
- **`dev/fixtures/entries.json`** — chaves em inglês pelo glossário:
  - raiz: `_about`, `version`, `master_password`, `pin`, `entries`;
  - entrada: `name`, `type`, `group`, `environment`, `level`, `fields`, `secrets`, `extras`
    (`name`/`value`/`secret`) e `attachments` (`name`/`file`);
  - campos: `username`, `port`, `engine`, `db_name`, `tunnel`, `jump`, `variable`, `region`,
    `security`, `protocol`;
  - segredos: `password`, `totp_seed`, `text`, `private_key_file`.
  - Tipos e níveis com os nomes v5. Os valores falsos são iguais, conferidos por script: só
    os 7 nomes v4 → v5 mudaram. O usuário `salto` do `acme-ssh-salto`/`-livre` é dado e
    ficou.
- **Leitores do `entries.json`, só nas chaves e nos nomes de tipo/nível que vêm dele**
  (troca 1:1):
  - Rust: o exemplo `dev-vault` (`main.rs`, que monta o cofre de teste, e `tests.rs`) e o
    `app/core` (`client.rs`, `service.rs`, `fake_daemon.rs`);
  - `crates/ward-test-bridge/tests/attacks.rs`;
  - `dev/variations.py` (oráculo) e `dev/services/generate.py`;
  - os testes `dev/tests/{dev-vault.sh, cli.py, extension.py, login.py, nm.py,
    socket_matrix.py, chrome_policy.py, playwright.py, ssh_home.py, variations_test.py,
    attacks-services.sh, docker-platform.sh, install.sh}`.
  - O parser da tabela "Cofre de teste" em `dev/tests/dev-vault.sh` passou para os nomes v5
    do contrato. A Fase 53 lista esse parser, mas sem a troca o `dev-vault-verify` já
    falharia. Conferido: a tabela do contrato e o `entries.json` dão as mesmas 22 linhas.
  - Ficaram de propósito: o `totp-contas.json` gerado pelo `generate.py` (`usuario`,
    `senha`, `totp_semente`, fio do serviço `totp`) e os nomes `WARD_*` do `cli.py`
    (Fase 53).
- **Testes escritos ou ajustados** (V4.1, V4.2, V4.8, V4.9). Rodei só os de unidade rápidos
  do corpo, para provar o byte a byte; os outros só rodam no Milestone 6.
  - `body/tests.rs`:
    - `check_accepted` aceita v4: confere que ele é regravado como v5 e relido igual;
    - `raw_records` exige v5;
    - `known_format_of_one_entry` vira v5 (`generic`/`free`) e ganha o gêmeo
      `known_format_of_one_entry_v4_is_written_as_v5`: o v4 escrito à mão relê a mesma
      entrada, regrava exatamente os bytes do v5, e os nomes cruzados e as versões 3/6
      são recusados;
    - `v2_31_forged_fields`: versão 4 sai da lista de desconhecidas e 6 entra;
      "v5 rotulado 4"; nomes v4 no corpo v5 (tipo, nível e segurança SMTP);
    - `d65_*` passam a usar o v5 como corpo corrente e o v4 com nomes v4 como controle.
  - Seção nova "m2:DT4" no `body/tests.rs`:
    - tabela v5 → v4 escrita à mão, independente do `legacy_v4`, mais `v4_records` e
      `v4_test_body`;
    - `dt4_tables_of_the_two_versions`;
    - `dt4_v4_body_reads_equal_and_is_written_as_v5` (tudo igual, bytes do v5 e diferença
      de tamanho exata);
    - `dt4_v4_smtp_security_none_becomes_v5_none`;
    - `dt4_v4_body_with_unknown_name_refused` (casos do V4.8);
    - `dt4_v4_bit_flipped_in_every_byte`;
    - `dt4_v4_vault_on_disk_opens_without_writing_and_any_write_is_v5` (senha errada,
      abertura que não grava, gravação v5 com geração + 1).
  - `wire_v1.rs`:
    - `vault_014_body_v4_is_read_and_written_as_v5` substitui o byte a byte do v4. Lê o
      `vault-0.1.4.ward`, confere nomes v5 e configuração, compara com o v4 em que cada
      campo de nome foi trocado e testa a canonicidade do v5;
    - `backup_014_*` compara as formas v5;
    - `vault_v5_header_same_as_014` (V4.9): magia, versão 1, KDF igual e ≥ 256 MiB/3,
      tamanho do cabeçalho;
    - `ENTRIES_V4` ficou como a 0.1.4 gravou.
  - `entry/tests.rs` (`combined_level_and_names`): nomes v5, rótulos pt-BR iguais e nomes v4
    desconhecidos no `from_name`.
  - `tests/body_memory.rs`: corpos forjados em v5 (`generic`/`free`).
  - `protocol/tests.rs`: os valores de `nivel` passaram a `blocked`/`free` (o `from_name`
    mudou).
- `atomic_write/tests.rs`: comentário dizendo que o corpo com datas é gravado como v5.
  Os nomes dos testes `d65_v4_*` ficaram.
- `dev/english-check/frozen.txt`:
  - saíram as 3 entradas da Fase 48 de `crates/ward-core/src/entry.rs` (`bloqueado`,
    `aprovacao`, `livre`), sem uso;
  - `app/core/src/service.rs livre` passou de 48 para 51: o comentário cita o valor de
    nível dos testes do app, que seguem o protocolo (ver Problemas).

## Segurança

- **Invariantes tocadas.**
  - S6/RM2: um nome fora da tabela da versão recusa o corpo inteiro com `Invalid`, sem
    valor no erro e sem gravar: nome v5 no v4, nome v4 no v5, maiúscula, espaço, `\0`,
    vazio.
  - A tabela v4 é fechada (`match` em bytes exatos, sem normalização) e só o caminho de
    leitura do v4 a chama.
  - S7: a gravação continua pelo caminho atômico de sempre e só gera v5. A migração no
    destrave é da Fase 49.
  - S8: a leitura v4 não aloca nada a mais. A segurança vira um `&'static [u8]` e as
    duas passadas são as mesmas.
  - Cripto e cabeçalho intactos: `format.rs` e `crypto.rs` não mudaram, nenhuma
    dependência nova, nada feito à mão.
- **Testes de ataque** (todos em `crates/ward-core/src/body/tests.rs`, salvo indicação).
  - V4.8/RM2:
    - `dt4_v4_body_with_unknown_name_refused`;
    - `dt4_tables_of_the_two_versions`;
    - `known_format_of_one_entry_v4_is_written_as_v5`;
    - `v2_31_forged_fields` (nomes v4 no v5, "v5 rotulado 4");
    - `dt4_v4_smtp_security_none_becomes_v5_none` (segurança cruzada).
  - V4.2:
    - `dt4_v4_bit_flipped_in_every_byte`, mais os 16 bits da versão do v4;
    - `wire_v1.rs › attack_vault_014_byte_flipped_does_not_open`, que já existia; o
      formato é o mesmo para v4 e v5 e o `format` V2.2 cobre cada byte.
  - V4.1: `dt4_v4_vault_on_disk_opens_without_writing_and_any_write_is_v5` (senha errada e
    abertura sem tocar o arquivo nem criar temporário).
  - V4.9: `wire_v1.rs › vault_v5_header_same_as_014`.
- **Dependências novas**: nenhuma.

## Ficou de fora

- **Migração no daemon** (DT5/DT6, Fase 49). O `ward-core` não expõe se o corpo lido era
  v4. A Fase 49 precisa de um jeito de saber, por exemplo
  `open_entries_with_generation` devolvendo a versão; `LEGACY_BODY_VERSION` já é `pub`.
- **Clientes do protocolo que levam valores v4 de tipo, nível ou segurança.** Desde esta
  fase o protocolo v1 leva os nomes v5 (`name()`/`from_name()`). Ficaram para as Fases
  51, 52 e 54 (lista em Problemas):
  - os testes Rust de JSON montado à mão em `crates/wardd/tests/*`, `app/core/src/*`,
    `app/src-tauri/src/main.rs`, `crates/ward-test-bridge/tests/attacks.rs`,
    `crates/ward-nm/src/tests.rs` e `crates/ward/src/{tests,text}.rs`;
  - o front-end (`app/src`) e a extensão.
- **Tabela da CLI com `label()`** (Fase 51).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 1c4a963` (árvore limpa) | 0 |
| `make english-check PATHS=…` (antes do merge; log `<scratchpad>/fase48/fase48-english-check.log`): achados só pré-existentes (Cargo.toml da 34a, `fake_daemon.rs`, `retrocesso`) e as 3 exceções sem uso, depois tiradas | 2 |
| `git merge --no-edit master` (Fases 34a e 47b) | 0 |
| `make english-check` (repositório inteiro, depois do merge) — 0 achado(s), 0 problema(s) nas exceções | 0 |
| `make compile` (log `<scratchpad>/fase48/fase48-make-compile.log`; 48 shell, 35 Python, 7 JS, 0 erro; 0 warning) | 0 |
| `cargo test -p ward-core --lib -- body::tests::dt4 body::tests::known_format body::tests::v2_31_forged_fields body::tests::d65_body_v3 body::tests::v2_30_empty entry::tests::combined_level wire_v1::vault wire_v1::backup` (17 testes) | 0 |
| `cargo fmt --all` e `cargo fmt --all -- --check` | 0 |
| Script de literais × HEAD (`<scratchpad>/fase48/literals.py`): nenhum `\u`/`\x`/`\0` perdido | 0 |

## Problemas descobertos

- **O fio v1 muda nesta fase para tipo, nível e segurança SMTP.** `name()`/`from_name()`
  servem ao corpo e ao protocolo. Até as Fases 50–54:
  - as linhas de `dev/fixtures/legacy/protocol-v1.jsonl` com `"tipo":"servidor"`,
    `"nivel":"livre"` e `"nivel":"bloqueado"` não fazem mais ida e volta byte a byte nos
    testes de `wire_v1.rs`; a Fase 50 transforma essas linhas em recusa;
  - o app manda `com-aprovacao`/`livre`/`banco`/`nenhuma`, que o `from_name` recusa:
    `app/src/screens/{form.js, Form.svelte, credentials.js, Credentials.svelte,
    ApprovalWindow.svelte}`, `app/src/lib/components/{types.js, levels.js,
    LevelDialog.svelte, LevelCard.svelte}` e `app/core/src/level.rs`;
  - os testes que montam JSON com esses valores (lista em "Ficou de fora");
  - `grep -rnE '"(banco|servidor|nota-segura|generico|bloqueado|com-aprovacao|livre|nenhuma)"'`
    acha todos.
  - O plano não diz em que fase os valores do `app/src` mudam (a 52 fala da ponte, a 54
    do `serde` da webview). O glossário, seção 10, diz que no protocolo v2 esses valores
    são os do corpo v5. Sugestão: a 51 cuida dos testes Rust e a 52/54 do `app/src`.
- **A coluna TIPO de `ward list` mostra o nome v5 até a Fase 51.** O `text.rs` imprime
  `kind.name()`. A Fase 51 já prevê trocar por `label()`; note que o `label()` também
  difere da 0.1.4 (`nota segura` × `nota-segura`, `ftp/sftp` × `ftp-sftp`, `genérico` ×
  `generico`).
- **`SMTP_SECURITY` entregue muda de `nenhuma` para `none`.** O valor vem de
  `SmtpSecurity::name()` (DT4). DT11 diz que os nomes `SMTP_*` não mudam, mas não fala
  do valor. No cofre de teste só há `starttls`, igual nas duas versões. A tabela "Entrega
  esperada" deveria registrar isso (orchestrator/planner).
- A mensagem de validação "deve ser nenhuma, starttls ou tls" ficou. É texto exibido
  (regra 2) e cita o rótulo da tela, mas agora não é mais o nome que o `from_name` aceita.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/body/legacy_v4.rs` | criado |
| `crates/ward-core/src/body.rs` | alterado |
| `crates/ward-core/src/body/tests.rs` | alterado |
| `crates/ward-core/src/entry.rs` | alterado |
| `crates/ward-core/src/entry/tests.rs` | alterado |
| `crates/ward-core/src/wire_v1.rs` | alterado |
| `crates/ward-core/src/protocol/tests.rs` | alterado |
| `crates/ward-core/src/atomic_write/tests.rs` | alterado (comentário) |
| `crates/ward-core/tests/body_memory.rs` | alterado |
| `crates/ward-core/examples/dev-vault/main.rs` | alterado |
| `crates/ward-core/examples/dev-vault/tests.rs` | alterado |
| `crates/ward-test-bridge/tests/attacks.rs` | alterado |
| `app/core/src/client.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/core/src/fake_daemon.rs` | alterado |
| `dev/fixtures/entries.json` | alterado |
| `dev/variations.py` | alterado |
| `dev/services/generate.py` | alterado |
| `dev/tests/dev-vault.sh` | alterado |
| `dev/tests/cli.py` | alterado |
| `dev/tests/extension.py` | alterado |
| `dev/tests/login.py` | alterado |
| `dev/tests/nm.py` | alterado |
| `dev/tests/socket_matrix.py` | alterado |
| `dev/tests/chrome_policy.py` | alterado |
| `dev/tests/playwright.py` | alterado |
| `dev/tests/ssh_home.py` | alterado |
| `dev/tests/variations_test.py` | alterado |
| `dev/tests/attacks-services.sh` | alterado |
| `dev/tests/docker-platform.sh` | alterado |
| `dev/tests/install.sh` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/mission/handoffs/phase-48.md` | criado |

**Literais alterados (regra 5), todos nomes:**

- nomes gravados de tipo, nível e segurança, v4 → v5 (`entry.rs`, testes, fixtures);
- chaves do `entries.json` e os acessos a elas;
- caminhos de chave nas mensagens de erro do `dev-vault` (`entries[{i}]`,
  `{name}.fields`, `.secrets`, `.extras[{j}].value`/`.secret`, `{name}.attachments`);
- `"[4, 0]"` → `"[5, 0]"` nas conferências de versão;
- o texto `_about` do `entries.json` (cita as chaves).

Nenhuma mensagem pt-BR exibida mudou. Os rótulos ficaram iguais, conferidos no teste.

## Procedimentos
- [x] `make compile` rodado (regra 6 do PRD: sem a suíte; mais os 17 testes de unidade do corpo)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V4.1, V4.2, V4.8; V4.9 sem SEG)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (os leitores do `entries.json` acompanham a troca das chaves; clientes do protocolo ficaram para 51/52/54)
