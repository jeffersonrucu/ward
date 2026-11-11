# Fase 18 — `ward-core`: `protocol/response`, `protocol/tests`, `tests/sources.rs`, `tests/body_memory.rs` (Missão 2, Milestone 2, trilha R)

## Implementado

- `crates/ward-core/src/protocol/response.rs`: comentários em inglês e nomes pelo
  glossário. `Resposta` → `Response` com as variantes `Ok`, `Status { locked,
  vault_exists, role }`, `Error`, `List`, `Output { stream, data }`, `Exit { code }`,
  `PlaywrightSession { path }`, `Detail`, `Secret`, `Token`, `ApprovalRequest`,
  `UnlockRequest`, `TotpCode { code, remaining }`, `Suggestions`, `Fill { username,
  password, totp }`, `LoginRequest { id, entry }`, `Backups`, `Config { lock_minutes,
  external_backup, local_backup }`, `ExternalBackup { enabled, uuid }`, `UsbDrives`;
  `Fluxo { Saida, Erro }` → `Stream { Stdout, Stderr }`; structs `ListItem`,
  `ItemSummary`, `ExtraDetail`, `AttachmentDetail`, `EntryDetail`, `ApprovalRequest`,
  `UnlockRequest`, `Suggestion`, `UsbDrive`, `BackupVersion` com os campos da seção 10 do
  glossário; `de_entrada` → `from_entry`; `tipo()`/`conferir`/`escrever`/`ler` →
  `kind()`/`check`/`write`/`read`; constantes `MAX_USB_DRIVES`, `MAX_LABEL`,
  `MAX_UNLOCK_REMAINING`, `MAX_SUMMARY_NODES` (`MAX_BACKUPS` fica) e os privados
  (`TEXT_FIELDS`, `APPROVED_OPERATIONS`, `MAX_PROBLEMS`, `MAX_CHAIN`, `Header`,
  `write_header`/`read_header`, `write_fields`/`read_fields`/`write_texts`, `label_ok`,
  `fields_ok`, `totp_digits`, `fields_and_secrets`). Lista completa no glossário,
  seção 15, parágrafo "Fase 18".
- `crates/ward-core/src/protocol/tests.rs`: comentários, auxiliares (`req`,
  `mk_secret`, `valid_data`, `with_field`, `frame_of`, `role_of`, `Counter`,
  `THREADS`…) e os 46 nomes de teste em inglês (prefixo do ID mantido:
  `v3_15_operation_role_table`, `fuzz_decoder`, `attack_stdin_kind_out_of_format`…).
- `crates/ward-core/tests/fontes.rs` → `tests/sources.rs` (`production_sources`,
  `walk`, `code_lines`; testes `v2_46_key_on_heap_and_no_hash_password`,
  `v2_50_only_public_writes`) e `tests/corpo_memoria.rs` → `tests/body_memory.rs`
  (`Observer`, `ALLOCATOR`, `TempDir`, `through_socket`…; testes
  `v2_32_forged_body_refused_fast_without_allocating`,
  `v2_32_authentic_body_allocates_only_what_exists`,
  `v2_32_encoder_refuses_above_limit_before_allocating`,
  `v2_33_no_freed_block_has_secret`, `s8_protocol_no_freed_block_has_secret`).
- Usos dos nomes renomeados no workspace inteiro (`ward-core`, `ward`, `ward-nm`,
  `wardd`, testes do `wardd`, `app/core`) pelo corretor de diagnósticos do `cargo check`
  (só nos spans dos erros E0599/E0609/E0026/E0559/E0560, filtrados pelo tipo); nos
  arquivos das Fases 19 e 34 só mudaram usos. Links de doc `MAX_RESTANTE_DESTRAVE`/
  `MAX_ROTULO` em `wardd/src/aprovacao.rs` e `udisks.rs` atualizados.
- `sources.rs` › `v2_50_only_public_writes`: a lista esperada de `pub fn` de
  `atomic_write.rs` foi reordenada na ordem do `sort()` (os nomes em inglês ordenam
  diferente dos em pt-BR); o conjunto é o mesmo.
- Glossário: parágrafo "Fase 18" na seção 15.
- Merge com o master `93e49b3` (Fase 19): conflitos em `nucleo.rs`, `playwright.rs`,
  `server.rs`, `ssh.rs` e nos testes `approval.rs`, `backup.rs`, `core.rs`, `d62.rs`
  resolvidos tomando a versão do master e reaplicando o script de renomeação e o
  corretor; `trava.rs`/`uso.rs` (apagados no master) saíram, e `lock.rs`/`usage.rs`
  receberam os usos.

## Literais alterados (PRD regra 5)

Só capturas de `format!`/`assert!` (o nome da variável dentro das chaves), nenhuma
chave, valor ou mensagem de fio:

- `protocol/tests.rs`: `{senha}` → `{password}`, `{dados}` → `{data}`, `{outra:?}`/
  `{outro:?}` → `{other:?}`, `{ruim}` → `{bad}`, `{forjado}` → `{forged}`, `{par}` →
  `{pair}`, `{quadro}` → `{frame}`, `{nomes:?}` → `{names:?}`, `{aceitos}` →
  `{accepted}`, `{troca}` → `{swap}`, `{versao}` → `{version}`, `{campo}` → `{field}`,
  `{chave}`/`{valor}` → `{key}`/`{value}`, `{tipo}` → `{kind}`, `{decisao}` →
  `{decision}`, `{validos}` → `{valid}`, `{entrada}`/`{restante}` → `{entry}`/
  `{remaining}`, `{proibido:?}` → `{forbidden:?}`. O JSON gerado é o mesmo (as chaves
  `"senha"`, `"dados"`, `"versao"`… ficam).
- `tests/body_memory.rs`: `{caso}` → `{case}`, `{maior}` → `{largest}`, `{teto}` →
  `{ceiling}`, `{tempo:?}` → `{elapsed:?}`, `{nome}-{}` → `{name}-{}`.
- `tests/sources.rs`: `{proibida}` → `{forbidden}`.

Comparação por script (multiconjunto de literais, lexer da Fase 17) de cada `.rs`
tocado contra o master: só os três arquivos acima diferem, e só nessas capturas. Os
literais com `\u` de `tests.rs` estão byte a byte iguais. Forma dos tokens
(identificadores como `ID`, sem comentários): igual em todos, salvo chaves de bloco
que o rustfmt tirou ou pôs nos braços de `match` (nomes mais curtos/longos) e
`0..FIOS` → `0..THREADS`.

## Segurança

- Fio congelado: `cargo test -p ward-core --lib -- protocol wire_v1` (compara com
  `protocol-v1.jsonl`) passa antes e depois do merge (58 testes). As chaves do JSON são
  literais escritos à mão em `response.rs` e não mudaram.
- Nenhuma mudança de lógica: diff só de nomes, comentários e capturas; os testes de
  ataque (V2.32, V2.33, S8 em `body_memory.rs`; V2.46, V2.50 em `sources.rs`;
  `fuzz_decoder`, `attack_*` em `protocol/tests.rs`) continuam com as mesmas
  asserções e passam.
- `Secret`/`Fill`/`Token` mantêm os tipos que zeram (`SecretBytes`/`Zeroizing`); o
  `Debug` sem o valor não mudou.
- A variante `FalhaGravar::Resposta` do `wardd/nucleo.rs` é outro tipo; o script a
  tinha renomeado e foi devolvida (é da fase do `nucleo`).
- Dependências novas: nenhuma. Nenhum segredo real; só o cofre de teste não foi nem
  aberto.

## Ficou de fora

- `make check` completo (PRD regra 6: a fase roda `make compile` + `english-check`).
- Testes do `wardd`/`ward`/`app/core` (só compilados; o clippy cobre todos os alvos).
- `docs/mission/plan.md` ainda cita `tests/fontes.rs` e `tests/corpo_memoria.rs` nos
  arquivos da Fase 18: é do orchestrator.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 3356893` (partida no master) | 0 |
| `cargo check --workspace --all-targets --all-features --locked --message-format=json` (logs `.dev/logs/fase18-check{1..9}.json`, corretor de diagnósticos) | 0 na última |
| `cargo fmt --all` / `cargo fmt --all --check` | 0 / 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (logs `fase18-clippy{1..4}.json`; rodadas para os `x: x`) | 0 |
| `cargo test -p ward-core --lib --locked -- protocol wire_v1` (log `fase18-test-protocol.log`) | 0 (58 passaram) |
| `cargo test -p ward-core --locked --test sources --test body_memory` (log `fase18-test-sources-memory.log`) | 101 (ordem da lista esperada em `v2_50`), depois 0 (`fase18-test-sources2.log`) |
| `python3 -I dev/english_check.py --paths <os 4 arquivos>` (= `make english-check PATHS=…`; logs `fase18-english{1,2,3}.log`) | 1 (14 achados), depois 0 |
| `make compile` (log `fase18-make-compile.log`) | 0 |
| `git merge --no-commit master` (`93e49b3`) | 1 (conflitos, resolvidos) |
| pós-merge: `cargo check` (`fase18-merge-check{1..3}.json`), `cargo clippy -D warnings` (`fase18-merge-clippy{1..3}.json`), `cargo fmt --check`, `cargo test -p ward-core --lib -- protocol wire_v1`, `make compile` | 0 nas últimas |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos

- O teste `v2_50_only_public_writes` compara a lista ordenada de `pub fn` com uma
  lista literal; renomear as funções muda a ordem. Ajustado aqui; fases que renomearem
  `pub fn` de `atomic_write.rs` precisam reordenar a lista.
- Comentários de `response.rs`/`tests.rs` citavam os botões da janela ("Negar",
  "Confirmar", "Permitir") e a seção "Comandos" do escopo; foram descritos em inglês
  em vez de citados, e o nome da pasta `ward-teste-memoria` saiu do comentário de
  `body_memory.rs` (o literal fica, é a pasta de teste).
- `docs/mission/plan.md` cita os nomes antigos dos dois testes de integração.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocol/response.rs` | alterado |
| `crates/ward-core/src/protocol/tests.rs` | alterado |
| `crates/ward-core/tests/sources.rs` (era `fontes.rs`) | renomeado/alterado |
| `crates/ward-core/tests/body_memory.rs` (era `corpo_memoria.rs`) | renomeado/alterado |
| `crates/ward-core/src/protocol.rs`, `protocol/json.rs`, `wire_v1.rs` | alterado (usos) |
| `crates/ward/src/{client,commands,tests,text}.rs` | alterado (usos) |
| `crates/ward-nm/src/{lib,tests}.rs` | alterado (usos) |
| `crates/wardd/src/{aprovacao,execution,lock,login,nucleo,playwright,server,ssh,udisks,usage}.rs` | alterado (usos; `lock`/`usage` da Fase 19) |
| `crates/wardd/{app-teste,aprovador-teste}/main.rs`, `crates/wardd/examples/test-client.rs` | alterado (usos) |
| `crates/wardd/tests/{app_value_binary,app_value_core,approval,attachment_edit,backup,common/mod,core,d62,dates,extension,external,lock,login,playwright,run,settings,socket,ssh,test_app}.rs` | alterado (usos) |
| `app/core/src/{approval,client,credentials,daemon,destrave,external_backup,fake_daemon,restore,service,startup}.rs` | alterado (usos; `destrave` da Fase 34) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-18.md` | criado |

## Procedimentos

- [x] `make compile` + `english-check` rodados (PRD regra 6; `make check` não)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto)
- [x] Testes de ataque das invariantes tocadas (V2.32, V2.33, S8, V2.46, V2.50, fio v1) passam sem mudar asserção
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
