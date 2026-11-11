# Fase 14 — `ward-core`: `body`, `body/test_vault`, `body/tests` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 88a9a48`.
- **Arquivos** (glossário seção 4): `src/corpo.rs` → `body.rs`, `corpo/cofre_teste.rs` →
  `body/test_vault.rs`, `corpo/testes.rs` → `body/tests.rs` (`git mv`); `lib.rs`:
  `pub mod body`. Os três traduzidos por inteiro (comentários, locais, parâmetros, nomes
  de teste).
- **Renomes públicos** (antigo → novo), com todos os usos no repositório:
  - `VERSAO_CORPO` → `BODY_VERSION` (**valor continua 4**; o 4 → 5 é da Fase 48).
  - `ErroCorpo` → `BodyError` (`Invalido` → `Invalid`, `GrandeDemais` → `TooLarge`,
    `Inconsistente` → `Inconsistent`).
  - `ErroAbrir` → `OpenError` (`Formato` → `Format`, `Corpo` → `Body`, `Retrocesso` →
    `Rollback`).
  - `abrir_entradas` → `open_entries`, `abrir_entradas_desde` → `open_entries_since`,
    `abrir_entradas_lida` → `open_entries_with_generation`, `serializar` → `serialize`,
    `desserializar` → `deserialize`, `CorpoLido` → `ReadBody`, `geracao_do_corpo`
    (`pub(crate)`) → `body_generation`.
- **Privados de `body.rs`**: `Milhar` → `Thousands`, `MAX_DADOS` → `MAX_DATA_FIELDS`,
  `Campo { Texto, Porta, Segredo, Nota }` → `Field { Text, Port, Secret, Note }`,
  `esquema` → `schema`, `Saida { total, buf, erro }` (`por`, `campo`) → `Output { total,
  buf, error }` (`put`, `field`), `Leitor { resto }` (`contagem`, `campo`) → `Reader { rest
  }` (`count`, `field`), `Lista` → `List`, `Bruta` → `RawEntry` (campos `kind`, `level`,
  `name`, `group`, `environment`, `project`, `tags`, `data`, `extras`, `attachments`,
  `dates`), `BrutaConfiguracao { geracao, hash_pin, minutos_trava, ligado, uuid }` →
  `RawConfig { generation, pin_hash, lock_minutes, enabled, uuid }`, `conferir_ordenadas`
  → `check_sorted`, `escrever_corpo`/`escrever_configuracao`/`escrever_entrada` →
  `write_body`/`write_config`/`write_entry`, `opcional`/`texto_opcional` →
  `optional`/`optional_text`, `autenticacao` → `auth_fields`, `campos_dados` →
  `data_fields`, `ler_cabecalho`/`ler_configuracao`/`ler_tag`/`ler_extra`/`ler_anexo`/
  `ler_anexos`/`ler_lista`/`ler_bruta`/`ler_datas` → `read_header`/`read_config`/
  `read_tag`/`read_extra`/`read_attachment`/`read_attachments`/`read_list`/`read_raw`/
  `read_dates`, `montar_configuracao`/`montar_dados`/`montar` → `build_config`/
  `build_data`/`build`, `conferir_estrutura` → `check_structure`, `texto`/`segredo`/
  `porta` → `text`/`secret`/`port`.
- **Cofre de teste (`test_vault.rs`)**: `SENHA_MESTRA` → `MASTER_PASSWORD`, `PIN_TESTE` →
  `TEST_PIN`, `SEGREDOS` → `SECRETS`, `configuracao_de_teste` → `test_config`,
  `configuracao_nova` → `new_config`, `cofre_de_teste` → `test_vault_entries`,
  `um_de_cada_cheio` → `one_of_each_full`, `chave_falsa` → `fake_key`, `validada` →
  `validated`, `bytes_0_a_255` → `bytes_0_to_255`; privados `rascunho` → `draft`, `banco`
  → `database`, `servidor_senha`/`servidor_chave` → `server_password`/`server_key` (como
  na Fase 13).
- **`tests.rs`**: ajudantes (`ler`/`ler_tudo`/`recusado` → `read`/`read_all`/`refused`,
  `mesmo_*` → `same_*`, `conferir_iguais`/`conferir_ida_e_volta`/`conferir_aceito` →
  `check_equal`/`check_round_trip`/`check_accepted`, `Crua`/`CruaCfg` (`padrao`, `de`) →
  `RawRecord`/`RawCfg` (`standard`, `of`; não `default`/`from`, que o clippy confunde com
  os traits), `cruas` →
  `raw_records`, `forjar*` → `forge*`, `indice` → `index`, `SplitMix::proximo`/`ate` →
  `next_u64`/`below` (não `next`, que o clippy pede como `Iterator`), `partes_do_hash` →
  `hash_parts`, `ParametrosKdfPin` → `PinKdfParams`) e os 33 nomes de teste em inglês
  (ex.: `v2_31_campos_forjados` → `v2_31_forged_fields`,
  `v2_44_geracao_sobe_e_retrocesso_e_recusado` →
  `v2_44_generation_rises_and_rollback_is_refused`). Nenhum script, teste nem contrato cita
  esses nomes (procurado).
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`lib.rs`, `atomic_write.rs` e
  testes, `backup.rs` (link de doc) e `backup/testes.rs`, `format.rs`, `limits/tests.rs`,
  `protocolo/erros.rs`, `protocolo/testes.rs`, `wire_v1.rs`, `tests/corpo_memoria.rs`,
  `tests/fontes.rs`, `examples/dev-vault/{main,tests}.rs`), `wardd` (`src/nucleo.rs`,
  `app-teste/main.rs`, testes `anexos_edicao.rs`, `app_valor.rs`, `backup.rs`,
  `comum/mod.rs`) e `app/core/src/daemon_falso.rs`. Todos os `#[path = ".../corpo/
  cofre_teste.rs"] mod cofre_teste;` viraram `#[path = ".../body/test_vault.rs"] mod
  test_vault;`. Feito por script (scratchpad, não versionado) com regex de palavra inteira
  para os nomes qualificados e nomes nus só nos arquivos que os importam; depois
  `cargo check` com 0 erro.
- **`daemon_falso::SEGREDOS`** (re-export, Fase 31): ficou
  `pub use test_vault::SECRETS as SEGREDOS;`, para não tocar `app/core/src/servico.rs`
  (Fase 30, em paralelo), `aprovacao.rs`, `bandeja.rs`, `cliente.rs`, `configuracoes.rs`,
  `credenciais.rs`, `restaurar.rs` e `crates/ward-test-bridge/tests/ataques.rs` (Fase 29,
  em paralelo). A Fase 31 troca o nome do re-export e os usos.
- **Glossário**: parágrafo "Fase 14" depois do da Fase 13 (seção 15).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`. Os comentários que citavam
  nomes gravados (`com-aprovacao`, `banco`, `servidor`), a entrada `delta-api-simbolos` e
  o arquivo `tests/corpo_memoria.rs` (Fase 18) foram reescritos sem o nome.

## Literais alterados (PRD regra 5)
- Caminhos de `#[path]`: `".../corpo/cofre_teste.rs"` → `".../body/test_vault.rs"` (7
  arquivos) e `"cofre_teste.rs"` → `"test_vault.rs"` em `body/tests.rs`.
- `tests/fontes.rs` (lê o código-fonte do `atomic_write.rs`):
  `"corpo::serializar(vault.config(), generation, entries)"` →
  `"body::serialize(vault.config(), generation, entries)"`, `"corpo::desserializar(&content)"`
  → `"body::deserialize(&content)"` e a exclusão `"cofre_teste.rs"` → `"test_vault.rs"`.
- `body/tests.rs`: pasta de teste `"ward-teste-corpo"` → `"ward-test-body"` (como o
  `ward-test-atomic-write` da Fase 11) e os nomes das subpastas `"ida-e-volta"`,
  `"troca"`, `"geracao"`, `"esgotada"` → `"round-trip"`, `"change"`, `"generation"`,
  `"exhausted"` (todas em `target/`, apagadas no fim do teste); capturas de `format!`
  renomeadas (`{caso}` → `{case}`, `{nome}` → `{name}`, `{erro:?}` → `{error:?}`,
  `{versao}`/`{sal}` → `{version}`/`{salt}`, `{tipo}` → `{kind}`, `{usuario}` →
  `{username}`, `{minutos}` → `{minutes}`, `{texto}` → `{text}`; V6.3).
- `body.rs`: capturas `{baixo}` → `{low}` (o texto da mensagem não muda);
  `test_vault.rs`: `{erros}` → `{errors}`.
- **Ficaram**: as mensagens pt-BR de `BodyError`/`OpenError` (`Display`), todos os textos
  de `panic!`/`assert!`, os nomes de caso das tabelas de ataque, os valores falsos de D15,
  os nomes de entrada/anexo/extra do cofre de teste, `"cofre.ward"` e os nomes gravados
  (`"generico"`, `"livre"`, `"banco"`…, Fase 48).
- Conferido por multiconjunto de literais por arquivo (HEAD × worktree): só as diferenças
  acima. Forma do código (`corpo.rs` × `body.rs`, identificadores trocados por um marcador,
  sem comentários): só campo abreviado (`private_key: chave_privada` → `private_key`) e
  vírgulas do `cargo fmt`.
- **Debug derivado**: `BodyError`/`OpenError` agora imprimem `Invalid`, `Rollback`,
  `Body(…)`; procurei `Retrocesso`, `Invalido`, `ErroAbrir`, `ErroCorpo` em scripts, testes
  e logs procurados: nenhum uso (só `strings-0.1.4.txt`, que o V6.3 deve aceitar).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S6/S7/S13 (corpo): leitura em duas passadas, conferência de todo comprimento contra o
    que resta e contra `limits` antes de alocar, soma dos anexos (D29), ordem estrita dos
    nomes, `Draft::validate` + `verify_in_vault`, erro único `BodyError::Invalid` sem dado
    do usuário; gravação com tamanho calculado antes e `Zeroizing<Vec<u8>>` de capacidade
    exata. Mesma lógica, mesma ordem (conferido pela forma do código acima).
  - D39/D53 (anti-rollback): `open_entries_since` confere a geração depois de decifrar e
    validar, devolve `OpenError::Rollback` sem valor; `max(lida, queimada)` igual.
  - S9 (PIN): o hash PHC do corpo continua conferido por `config::verify_text` antes de
    `PinHash::from_phc`; `valid_minutes`/`valid_uuid` iguais.
  - V2.2: `wire_v1.rs` mudou só nomes (`corpo::VERSAO_CORPO` → `body::BODY_VERSION`,
    `serializar`/`desserializar`); fixtures intocadas; `BODY_VERSION == 4`.
- **Testes de ataque**: nenhum novo (só rename); os existentes (corpo truncado, bit
  trocado, 100 000 mutações, campos forjados, hash do PIN rebaixado/inflado, datas
  forjadas, cofre v3 no disco, rollback) acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Locais e comentários pt-BR dos outros arquivos que usam os nomes novos ficam para as
  fases deles (ex.: `tests/corpo_memoria.rs` e `tests/fontes.rs`, Fase 18;
  `backup/testes.rs`, Fase 15; `protocolo/*`, Fases 17–18; `wardd`, Fases 19–27;
  `daemon_falso.rs`, Fase 31).
- O alias `pub use test_vault::SECRETS as SEGREDOS` no `daemon_falso.rs` (Fase 31).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 88a9a48` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (log `.dev/logs/fase14-check.log`; 1 rodada com erro: `crypto::hash_pin` pego pelo rename, revertido) | 0 |
| `cargo fmt --all` | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra; log `fase14-clippy.log`) | 0 |
| `python3 -I dev/english_check.py --paths crates/ward-core/src/body.rs crates/ward-core/src/body/tests.rs crates/ward-core/src/body/test_vault.rs` (log `fase14-english.log`) | 0 (0 achado) |
| `make compile` (log `fase14-make-compile.log`; inclui o `cargo check` do `app/src-tauri`, build do Vite e `syntax_check.py`) | 0 |
| depois do `make compile`, `CruaCfg::de` → `RawCfg::of`: `cargo fmt --all --check` e `cargo clippy -p ward-core --all-targets --all-features --locked -- -D warnings` (log `fase14-clippy2.log`) e o checador de novo | 0 / 0 / 0 achado |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `88a9a48`; reset com a árvore limpa.
- **Arquivos de fases paralelas**: nenhum arquivo da Fase 29 (`crates/ward-test-bridge`) nem
  da Fase 30 (`app/core`: `servico`, `segredo`, `erro`, `lib`) nem da 47a tocado; em
  `app/core` só o `daemon_falso.rs` (Fase 31), por causa do `#[path]`.
- **Arquivo da trilha S tocado**: `crates/ward-core/examples/dev-vault/{main,tests}.rs`
  (Fase 43), só os usos.
- `dev/fixtures/legacy/strings-0.1.4.txt` cita `"../../../crates/ward-core/src/corpo/
  cofre_teste.rs"` (literal de `#[path]` no binário 0.1.4); é linha de base, não muda, e o
  V6.3 deve aceitar a troca de caminho.
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 13).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/body.rs` (era `corpo.rs`) | renomeado e reescrito |
| `crates/ward-core/src/body/test_vault.rs` (era `corpo/cofre_teste.rs`) | renomeado e reescrito |
| `crates/ward-core/src/body/tests.rs` (era `corpo/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/{atomic_write,backup,format,wire_v1}.rs`, `atomic_write/tests.rs`, `backup/testes.rs`, `limits/tests.rs`, `protocolo/{erros,testes}.rs` | alterado (usos) |
| `crates/ward-core/tests/{corpo_memoria,fontes}.rs`, `crates/ward-core/examples/dev-vault/{main,tests}.rs` | alterado (usos) |
| `crates/wardd/src/nucleo.rs`, `crates/wardd/app-teste/main.rs`, `crates/wardd/tests/{anexos_edicao,app_valor,backup}.rs`, `crates/wardd/tests/comum/mod.rs` | alterado (usos) |
| `app/core/src/daemon_falso.rs` | alterado (usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-14.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
