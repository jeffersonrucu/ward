# Fase 13 — `ward-core`: `config` (+ testes) e `entry/tests` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 5255317`.
- **Arquivos** (glossário seção 4): `src/configuracao.rs` → `config.rs`,
  `configuracao/testes.rs` → `config/tests.rs` (`git mv`); `lib.rs`: `pub mod config`.
  `entry/tests.rs` (já movido na Fase 12) traduzido por inteiro.
- **`config.rs`**: comentários, locais e parâmetros em inglês. Renomes (antigo → novo),
  com todos os usos no repositório:
  - `Configuracao` → `VaultConfig` (`nova`, `de_partes`, `conferir_pin`, `hash_pin`,
    `trocar_hash_pin`, `minutos_trava`, `definir_minutos_trava`, `backup`,
    `definir_backup` → `new`, `from_parts`, `verify_pin`, `pin_hash`, `change_pin_hash`,
    `lock_minutes`, `set_lock_minutes`, `backup`, `set_backup`; campos `hash_pin`/
    `minutos_trava` → `pin_hash`/`lock_minutes`).
  - `HashPin` → `PinHash` (`criar`, `conferir`, `parametros`, `de_phc` → `create`,
    `verify`, `params`, `from_phc`; `bytes` fica).
  - `BackupExterno { ligado, uuid }` → `ExternalBackup { enabled, uuid }` (`desligado`,
    `novo`, `ligado` → `disabled`, `new`, `enabled`).
  - `ErroConfiguracao` → `ConfigError` (`PinInvalido` → `InvalidPin`, `TempoDeTrava` →
    `LockTimeout`, `UuidInvalido` → `InvalidUuid`, `BackupSemUuid` → `BackupWithoutUuid`;
    `Hash` fica).
  - `KDF_PIN_MINIMO`/`KDF_PIN_MAXIMO` → `KDF_PIN_MINIMUM`/`KDF_PIN_MAXIMUM` (`KDF_PIN`
    fica: já é inglês).
  - `pub(crate)`/privados: `conferir_texto` → `verify_text`, `minutos_validos` →
    `valid_minutes`, `uuid_valido` → `valid_uuid`, `pin_valido` → `valid_pin`, `ler` →
    `read`.
- **`config/tests.rs`** e **`entry/tests.rs`**: nomes dos testes, ajudantes, locais e
  comentários em inglês (ex.: `rascunho` → `draft`, `banco` → `database`,
  `servidor_senha`/`servidor_chave` → `server_password`/`server_key`, `nota` → `note`,
  `anexo` → `attachment`, `generico` → `generic`, `um_de_cada` → `one_of_each`,
  `segredos_de_teste` → `test_secrets`, `valida`/`erros` → `valid`/`errors`,
  `sem_segredo` → `no_secret`, `sem` → `without` com a macro `vazio` → `clear`,
  `FazDados` → `MakeData`, `NaoClone::conferir` → `NotClone::check`, `ImplementaClone`
  → `ImplementsClone`, `zera_ao_sair` → `zeroes_on_drop`, `anexos_de` →
  `attachments_of`, `sem_o_valor` → `without_the_value`, `FORA_DA_LISTA` →
  `OFF_THE_LIST`, `VARIAVEIS_PERIGOSAS`/`BOAS` → `DANGEROUS_VARIABLES`/`GOOD_VARIABLES`).
  Nenhum script nem teste fora destes arquivos cita os nomes de teste (procurado).
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`atomic_write.rs` e testes,
  `corpo.rs`, `corpo/cofre_teste.rs`, `corpo/testes.rs`, `crypto.rs` (comentário),
  `format.rs`, `limits/tests.rs`, `protocolo/pedido.rs`, `protocolo/resposta.rs`,
  `wire_v1.rs`, `tests/corpo_memoria.rs`, `examples/dev-vault/{main,tests}.rs`), `wardd`
  (`nucleo.rs`, `udisks.rs`, testes `backup.rs`, `d62.rs`), `app/core`
  (`backup_externo.rs`, `daemon_falso.rs`). Feito pelo compilador: um script (scratchpad,
  não versionado) lê os diagnósticos JSON do `cargo check` e troca o identificador só no
  span de cada erro causado pelo rename (método mapeado pelo tipo citado na mensagem);
  0 erro no fim. Caminho `configuracao::` → `config::` por `sed` (só com `::`, para não
  pegar os locais `configuracao` do `corpo.rs`, que são da Fase 14). Doctests de
  `format.rs` e `atomic_write.rs` (o `cargo check` não os compila) e os links de doc
  (`corpo.rs`, `daemon_falso.rs`) acertados à mão.
- **Homônimos que não mudaram**: `Operacao::Configuracao`, `Operacao::BackupExterno`,
  `Operacao::DefinirBackupExterno` e os de `TipoResposta` (Fase 17); `BackupExternoVisto`
  e `uuid_valido` do `app/core/backup_externo.rs`; `minutos_validos` do
  `app/core/configuracoes.rs` (Fase 33); `BrutaConfiguracao { ligado, minutos_trava }` e
  os locais `configuracao` do `corpo.rs` (Fase 14).
- **Glossário**: parágrafo "Fase 13" depois do da Fase 12.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`. Os comentários de
  `entry/tests.rs` que citavam nomes pt-BR (tabela "Cofre de teste", anexo "chave",
  `WARD_ARQUIVO_CHAVE`, "com aprovação", `nota segura`, `validacao.rs`, `servidor`,
  "usuário") foram reescritos sem o nome.

## Literais alterados (PRD regra 5)
- `config.rs`: `Debug` `"HashPin(••••)"` → `"PinHash(••••)"` (o valor continua `••••`).
  Nenhum script nem teste procura esse texto (só `strings-0.1.4.txt`, que o V6.3 deve
  aceitar).
- `config/tests.rs`: `"minutos_trava: 240"` → `"lock_minutes: 240"` (o teste procura o
  campo no `Debug` derivado do `VaultConfig`, que mudou de nome); capturas de `format!`
  `{cofre:?}` → `{vault:?}`, `{invalido}` → `{invalid}`.
- `entry/tests.rs`: as chaves do ajudante `without(data, field)` (antigo `sem`), que
  compara com `stringify!` do nome do campo desestruturado: `"usuario"`, `"senha"`,
  `"motor"`, `"chave_privada"`, `"regiao"`, `"seguranca"`, `"protocolo"`, `"texto"`,
  `"totp_semente"`, `"base"`, `"porta"`, `"tunel"`, `"variavel"` → `"username"`,
  `"password"`, `"engine"`, `"private_key"`, `"region"`, `"security"`, `"protocol"`,
  `"text"`, `"totp_seed"`, `"db_name"`, `"port"`, `"tunnel"`, `"variable"` (são nomes de
  campo do `DataDraft`, que a Fase 12 já renomeou). Os rótulos do erro na 3ª coluna
  (`"usuário"`, `"senha"`, `"motor"`…) ficaram. Mais capturas de `format!` renomeadas
  (`{rotulo}` → `{label}`, `{alvo}` → `{target}`, `{erros}` → `{errors}`, `{campo}` →
  `{field}`, `{nome}` → `{name}`, `{usuario:?}` → `{username:?}` etc.; V6.3).
- **Ficaram**: as mensagens pt-BR de `ConfigError` (`Display`), todas as mensagens e
  rótulos de validação, os textos dos `panic!`/`assert!`, os valores falsos de D15 e os
  nomes de extra/anexo dos ataques (`"chave"`, `"protocolo"`, `"entrada"`, `"GRUPO"`,
  `"arquivo"`…, que testam as variáveis entregues congeladas até a Fase 53).
- **Fora dos 3 arquivos**: nenhum literal mudou (multiconjunto de literais por arquivo,
  fora de comentários: zero diferença).
- **Debug derivado**: `VaultConfig` e `ExternalBackup` agora imprimem `lock_minutes`,
  `pin_hash`, `enabled`; procurei os nomes antigos em scripts e testes: só o teste acima.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S9/S13 (PIN): `PinHash::create` recusa PIN fora de 4–6 dígitos antes do Argon2;
    `verify` confere o formato e chama `crypto::verify_pin` (tempo constante pela
    biblioteca); `read` limita o custo por `KDF_PIN_MINIMUM`/`KDF_PIN_MAXIMUM` antes de
    rodar o Argon2. Mesma lógica, mesma ordem.
  - S8/S13 (memória): `PinHash` só com `Zeroizing<String>`, `ZeroizeOnDrop`, sem `Clone`
    nem `Display`, `Debug` só `••••`; `VaultConfig` sem `Clone`, `ZeroizeOnDrop`.
    `ConfigError` sem valor. O teste V2.28 (`NotClone`/`zeroes_on_drop`) segue cobrindo
    `PinHash` e `VaultConfig`.
  - D17: `ExternalBackup::new` recusa UUID inválido e ligado sem UUID; `valid_uuid` igual.
  - V2.2: `wire_v1.rs` mudou só um nome de método (`minutos_trava()` → `lock_minutes()`);
    fixtures intocadas.
- **Testes de ataque**: nenhum novo (só rename); os existentes (hash adulterado, hash com
  custo inflado, `Debug` sem segredo, injeção de opção no `ssh`, texto invisível etc.)
  acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Locais e comentários pt-BR dos outros arquivos que usam os nomes novos ficam para as
  fases deles (ex.: `corpo.rs` com `BrutaConfiguracao`, `ler_configuracao`,
  `escrever_configuracao`, Fase 14).
- Doctests: não rodaram (PRD regra 6); os 6 de `format.rs`/`atomic_write.rs` foram
  acertados à mão.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 5255317` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| script de rename por diagnóstico (`cargo check --workspace --all-targets --all-features --locked --message-format=json`, 15 rodadas até 0 erro; log `.dev/logs/fase13-fix.log`) | 0 |
| `cargo fmt --all` | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra; log `fase13-clippy.log`) | 0 |
| `python3 -I dev/english_check.py --paths crates/ward-core/src/config.rs crates/ward-core/src/config/tests.rs crates/ward-core/src/entry/tests.rs` (log `fase13-english.log`) | 0 (0 achado) |
| `make compile` (log `fase13-make-compile.log`; inclui `npm ci`, build do Vite, `cargo check` do `app/src-tauri` e `syntax_check.py`) | 0 |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `5255317`; reset com a árvore limpa.
- **Arquivo da trilha S tocado**: `crates/ward-core/examples/dev-vault/{main,tests}.rs`
  (Fase 43), só os usos dos nomes desta fase. Nenhum arquivo de `dev/` nem de `install/`
  tocado (as Fases 46 e 47 rodam em paralelo).
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 12).
- O módulo `ward_core::config` convive com o `wardd::config` (outro módulo, com
  `Config`); nenhum arquivo importa os dois pelo nome curto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/config.rs` (era `configuracao.rs`) | renomeado e reescrito |
| `crates/ward-core/src/config/tests.rs` (era `configuracao/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/entry/tests.rs` | reescrito |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/{atomic_write,corpo,crypto,format,wire_v1}.rs`, `atomic_write/tests.rs`, `corpo/{cofre_teste,testes}.rs`, `limits/tests.rs`, `protocolo/{pedido,resposta}.rs` | alterado (usos) |
| `crates/ward-core/tests/corpo_memoria.rs`, `crates/ward-core/examples/dev-vault/{main,tests}.rs` | alterado (usos) |
| `crates/wardd/src/{nucleo,udisks}.rs`, `crates/wardd/tests/{backup,d62}.rs` | alterado (usos) |
| `app/core/src/{backup_externo,daemon_falso}.rs` | alterado (usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-13.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
