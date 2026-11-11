# Fase 11 — `ward-core`: `format` e `atomic_write` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard f03b2fa`.
- **Arquivos** (glossário seção 4): `src/formato.rs` → `format.rs`,
  `formato/testes.rs` → `format/tests.rs`, `gravacao.rs` → `atomic_write.rs`,
  `gravacao/testes.rs` → `atomic_write/tests.rs`. Módulos no `lib.rs`: `pub mod format`
  e `pub mod atomic_write` (este continua `#[cfg(unix)]`).
- **Comentários, locais, parâmetros, privados e nomes de teste** dos 4 arquivos em
  inglês (`ida_e_volta` → `round_trip`, `filho_gravacao` → `child_write`,
  `ataque_*` → `attack_*`…).
- **Renomes públicos e `pub(crate)`** (antigo → novo), com todos os usos ajustados:
  - `format`: `MAGIA` → `MAGIC`, `VERSAO` → `VERSION`, `TAM_SAL` → `SALT_SIZE`,
    `TAM_CABECALHO` → `HEADER_SIZE`, `MAX_CONTEUDO` → `MAX_CONTENT`;
    `ParametrosKdf { memoria_kib, passes, paralelismo }` → `KdfParams { memory_kib,
    passes, parallelism }` (`PADRAO`/`MINIMO`/`MAXIMO` → `DEFAULT`/`MINIMUM`/`MAXIMUM`,
    `conferir` → `verify`); `Cabecalho { versao, kdf, tamanho_corpo }` → `Header {
    version, kdf, body_size }`; `ErroFormato` → `FormatError` (`PasswordOrFile`,
    `UnsupportedVersion`, `ParamsBelowMinimum`, `ParamsAboveMaximum`,
    `ContentTooLarge`, `EmptyPassword`, `Random`, `Encrypt`); `SenhaMestra` →
    `MasterPassword` (`nova`/`de_bytes`/`digitada`/`serve_de_nova` → `new`/`from_bytes`/
    `typed`/`is_valid_new`); `CofreAberto` → `OpenVault` (`novo`, `novo_com`,
    `depois_da_geracao`, `de_partes`, `geracao`, `proxima_geracao`, `queimar`,
    `parametros`, `configuracao`, `configuracao_mut`, `selar`, `autentica`,
    `geracao_selada` → `new`, `new_with`, `after_generation`, `from_parts`, `generation`,
    `next_generation`, `burn`, `params`, `config`, `config_mut`, `seal`,
    `authenticates`, `sealed_generation`); `ChavesCofre` → `VaultKeys` (`sal`,
    `nonce_chave`, `embrulhada`, `chave_dados` → `salt`, `key_nonce`, `wrapped`,
    `data_key`; `novo_com`/`novo_interno`/`abrir_proprio` → `new_with`/`new_internal`/
    `open_own`); `ler_cabecalho` → `read_header`, `abrir` → `open`,
    `abrir_com_minimo` → `open_with_minimum`, `trocar_senha` → `change_password`,
    `SenhaTrocada` → `PasswordChanged`, `prefixo_cabecalho` → `header_prefix`, `u32_em`
    → `u32_at`; constantes privadas `P_*` e `TAM_EMBRULHADA` → `WRAPPED_SIZE`.
  - `atomic_write`: `IdentidadeArquivo { …, tamanho }` → `FileIdentity { …, size }`;
    `Etapa` → `Step` (`Read`, `CreateTemp`, `Write`, `FsyncTemp`, `Rename`,
    `FsyncDir`); `ErroGravacao` → `WriteError` (`GenerationExhausted`, `Rollback`,
    `NotVault`, `Format`, `Body`, `InvalidDestination`, `Io { step, kind, identity }`);
    `Ponto` → `Point` (`AfterCreate`, `Writing`, `AfterWrite`, `AfterFsync`,
    `AfterRename`, `AfterFsyncDir`); `Gravado { identidade, backup }` → `Written {
    identity, backup }`; `gravar_entradas`(`_com_backup`) → `write_entries`
    (`_with_backup`), `trocar_senha_lido` → `change_password_from_bytes`,
    `trocar_senha_com_backup_lido` → `change_password_with_backup_from_bytes`,
    `trocar_senha`(`_com_backup`) (só testes) → `change_password`(`_with_backup`),
    `gravar_cofre` → `write_vault`, `gravar` → `write`, `limpar_temporarios` →
    `clean_temp_files`, `gravar_novo_com_pontos` → `write_new_with_points`; privados
    `MARCA_TEMPORARIO`/`MODO`/`PEDACO` → `TEMP_MARK`/`MODE`/`CHUNK`, `Temporario {
    caminho, ativo }` → `TempFile { path, active }`, `criar_temporario` → `create_temp`,
    `escrever_temporario` → `write_temp`, `sincronizar_diretorio` → `sync_dir`,
    `separar` → `split`, `erro` → `error`, `ler_arquivo` → `read_file`,
    `*_com_pontos`/`*_selado` → `*_with_points`/`*_sealed`.
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`lib`, `crypto` — campos do
  `KdfParams` —, `backup` e testes, `configuracao` e testes, `corpo` e testes,
  `entrada/testes`, `protocolo/erros`, `protocolo/testes`, `wire_v1.rs`,
  `tests/corpo_memoria.rs`, `tests/fontes.rs`, `examples/dev-cofre/{main,testes}.rs`),
  `wardd` (`arquivos`, `externo`, `nucleo`; testes `anexos_edicao`, `app_valor`,
  `backup`, `comum/mod.rs`, `d62`, `externo`, `memoria`) e `app/core/src/primeiro_uso.rs`
  (a chamada a `ward_core::format::MasterPassword::typed(..).is_valid_new()`).
- **Colisões evitadas** (outros itens com o mesmo nome, não mudaram): o `SenhaMestra`
  do `app/core/src/destrave.rs` (e os usos no app, na ponte de teste e no Tauri); os
  `limpar_temporarios` de `wardd/src/execucao.rs` e `wardd/src/externo.rs`; o
  `ler_cabecalho` de `corpo.rs`, `protocolo/quadro.rs` e `protocolo/resposta.rs`;
  `ErroResposta::{Retrocesso, GeracaoEsgotada}` (fio, Fase 17/18) e
  `ErroAbrir::Retrocesso` (Fase 14); `backup::geracao_selada` (Fase 15, só o método do
  `OpenVault` mudou); `protocolo::VERSAO`; `configuracao::HashPin::parametros()`.
- **`tests/fontes.rs`** (auditor de V2.46/V2.50): o arquivo de produção que pode gravar
  passou a ser `atomic_write.rs`; a lista exata de `pub fn` é a nova
  (`change_password_from_bytes`, `change_password_with_backup_from_bytes`,
  `clean_temp_files`, `write_entries`, `write_entries_with_backup`); os trechos de
  código procurados acompanham os nomes novos (`fn seal(`/`fn write_vault(`/
  `fn write(`, `corpo::serializar(vault.config(), generation, entries)`,
  `corpo::desserializar(&content)`, `write_entries_sealed(destination, &mut vault,
  &entries`). Lógica do teste igual.
- **Testes de fio** (`wire_v1.rs`): diff só de nomes (`MasterPassword::new`,
  `format::open`); fixture e expectativas intactas.
- **Glossário**: parágrafo "Fase 11" na seção 15 (inclusive o termo novo "lido" →
  `from_bytes` e as colisões acima).
- **Exceções**: 4 entradas novas em `allow.txt` (`falso` e `mestra` em `format.rs` e
  `atomic_write.rs`: a senha falsa de D15 `"falso-mestra-ward-01"` nos doc-tests, que o
  checador lê como comentário). Nenhuma em `frozen.txt`.

## Literais alterados (PRD regra 5)
- `format.rs`: `Debug` manual: `"SenhaMestra(••••)"` → `"MasterPassword(••••)"`,
  `debug_struct("CofreAberto")` → `"OpenVault"` com os campos `"chaves"`,
  `"configuracao"`, `"geracao"` → `"keys"`, `"config"`, `"generation"`;
  `debug_struct("ChavesCofre")` → `"VaultKeys"` e `"chave_dados"` → `"data_key"` (o
  valor continua `"••••"`). Doc-test `compile_fail`: `b"corpo sem hash de PIN"` →
  `b"body without a PIN hash"` (não compila de propósito, nunca roda).
- `atomic_write.rs`: captura `({tipo})` → `({kind})` no `Display` de `WriteError::Io`
  (texto exibido igual); doc-tests (`compile_fail` e `no_run`, nunca gravam):
  `"cofre.ward"` → `"vault.ward"`.
- `atomic_write/tests.rs`: `CHILD_NAME` `"gravacao::testes::filho_gravacao"` →
  `"atomic_write::tests::child_write"` (caminho do teste que o processo filho roda; tem
  de acompanhar o módulo e a função); pasta de teste em `target/<perfil>/`
  `"ward-teste-gravacao"` → `"ward-test-atomic-write"`; capturas de `format!`/`assert!`
  (`{versao}`, `{SEGREDO_FALSO}`, `{marca}`, `{nome}`, `{alvo:?}`, `{outro:?}`,
  `{controle}`, `{esperada}`, `{MORTES}`, `{com_temporario}`, `{vistas:?}`, `{uma:?}`,
  `{caminho_tmp}`, `{destino}`, `{texto}`, `{devolvida:?}` → `{version}`,
  `{FAKE_SECRET}`, `{mark}`, `{name}`, `{target:?}`, `{other:?}`, `{control}`,
  `{expected}`, `{DEATHS}`, `{with_temp}`, `{seen:?}`, `{one:?}`, `{tmp_path}`,
  `{destination}`, `{text}`, `{returned:?}`; texto igual).
- `format/tests.rs`: capturas `{tam}` → `{size}`, `{nome}` → `{name}`, `{cofre:?}` →
  `{vault:?}`.
- `tests/fontes.rs`: os nomes de arquivo e os trechos de código procurados listados
  acima, mais a mensagem `"pub fn de atomic_write.rs"`.
- **Ficaram**: todos os `Display` pt-BR (`FormatError`, `WriteError`, `Step::name`), as
  marcas do filho (`WARD-PONTO`, `WARD-FIM`, `WARD-PRONTO`), os modos `"ponto:"`/`"laco"`/
  `"strace"`, as variáveis `WARD_TESTE_GRAVACAO_{MODO,DIR,MARCA,CONTROLE}` (DT12, Fase
  56; os identificadores Rust viraram `ENV_MODE`/`ENV_MARK`/`ENV_CONTROL`), a magia
  `WARDCOFR`, `"cofre.ward"` e o prefixo `.ward-tmp-` nos testes e os valores falsos.
- **Conferido** por comparação da forma (tokens com identificadores normalizados, sem
  comentários) entre cada arquivo antigo e o novo (script na scratchpad):
  `atomic_write.rs` igual token a token; `format.rs` igual, fora uma vírgula final que o
  rustfmt tirou; `format/tests.rs` igual, fora um bloco `{ … }` em volta de um
  closure (mesma expressão); `atomic_write/tests.rs` igual. Os literais que diferem são
  exatamente os listados acima.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S6 (`format`): cabeçalho de 160 bytes, posições e AAD iguais; recusa de parâmetros
    abaixo do mínimo e acima do máximo antes de derivar; erro único
    `PasswordOrFile`; chave de dados no heap (`Key`), `ZeroizeOnDrop` em
    `MasterPassword`, `OpenVault` e `VaultKeys`, `Debug` sem segredo; NFC da senha
    digitada igual (Fase 39c); `seal` e `from_parts` continuam `pub(crate)` (os
    doc-tests `compile_fail` E0609/E0599/E0616/E0624 acompanham os nomes novos).
  - S7/S2 (`atomic_write`): temporário `O_CREAT|O_EXCL|O_CLOEXEC` 0600 no mesmo
    diretório, escrita em pedaços, fsync, rename, fsync do diretório, `link(2)` sem
    sobrescrever para o backup; só bytes com a estrutura de cofre entram; geração
    queimada ao selar (D53); identidade do fd do temporário (D62). As únicas `pub fn`
    que gravam continuam 5 (V2.50, `tests/fontes.rs` com os nomes novos), e
    `write`/`write_vault` seguem `pub(crate)` (doc-tests `compile_fail` E0603).
  - S13: nenhum erro carrega conteúdo; os rótulos dos `Debug` manuais mudaram de nome,
    o valor continua `••••`.
- **Testes de ataque**: nenhum novo (só rename); os existentes renomeados junto
  (`v2_4_downgrade_to_8_mib_fails`, `splicing_between_vaults_and_writes_fails`,
  `attack_short_disguised_new_password_is_refused`,
  `refuses_to_write_bytes_that_are_not_a_vault`, `symlink_destination_is_not_followed`,
  `temp_file_does_not_follow_symlink_nor_reuse_file`,
  `v2_9_sigkill_at_200_random_points`, `v2_10_v2_11_strace_order_and_nothing_in_clear`,
  `v2_53_directory_fsync_fails_and_the_generation_does_not_repeat`…). Não rodaram (PRD
  regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes dos módulos das Fases 12–15 usados nestes arquivos (`Configuracao`,
  `HashPin::criar`, `corpo::serializar`/`desserializar`/`abrir_entradas`,
  `ErroAbrir::Retrocesso`, `Entrada`, `Rascunho*`, `Segredo`, `PastaBackup`, `Versao`,
  `ErroBackup`, `cofre_teste`…): são os 139 achados que sobram no `english-check` desta
  fase (comentários dos doc-tests que chamam `Configuracao::nova`/`HashPin::criar` e
  identificadores); nenhum nome desta fase.
- Os locais pt-BR de outros arquivos que usam os nomes novos (`cofre`, `senha`,
  `geracao`…, em `nucleo.rs`, `corpo/testes.rs` etc.) ficam para as fases deles; aqui só
  o identificador desta fase mudou.
- `tests/fontes.rs`: só os usos e os trechos procurados; a tradução do arquivo é da Fase
  18.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard f03b2fa` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (até ficar limpo; log `fase11-check.log`) | 0 |
| `cargo fmt --all` / `cargo fmt --all --check` | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra, log `fase11-clippy.log`) | 0 |
| `make compile` (log `fase11-make-compile.log`; inclui o `cargo check` do `app/src-tauri`) | 0 |
| `python3 -I dev/english_check.py --paths crates/ward-core/src/format.rs crates/ward-core/src/format crates/ward-core/src/atomic_write.rs crates/ward-core/src/atomic_write` (= `make english-check PATHS=…`; log `fase11-english.log`) | 1 (139 achados, todos nomes das Fases 12–15; 0 problema nas exceções) |
| comparação de forma e de literais antigo × novo (scripts na scratchpad) | — |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `f03b2fa`; com a árvore limpa, fiz
  `git reset --hard f03b2fa`.
- **Homônimos que não são desta fase**: `limpar_temporarios` existe também em
  `wardd/src/execucao.rs` (pública, chamada pelo `main.rs`) e em `wardd/src/externo.rs`
  (privada); `SenhaMestra` existe no `app/core/src/destrave.rs` com o mesmo `Debug`
  `"SenhaMestra(••••)"`. As fases deles (21, 23, 31) precisam escolher nomes; sugestão no
  glossário só para o do `atomic_write` (`clean_temp_files`).
- `strings-0.1.4.txt` tem os rótulos dos `Debug` manuais de `SenhaMestra`,
  `CofreAberto`, `ChavesCofre` (`"chaves"`, `"configuracao"`, `"geracao"`,
  `"chave_dados"`): o V6.3 deve aceitar a troca, como os `Debug` das Fases 8–10.
- **Arquivos compartilhados com as trilhas W e S**: `dev/english-check/allow.txt` (4
  linhas no fim) e `docs/glossario.md` (um parágrafo antes de "Funções e métodos…" na
  seção 15). Nenhum script de `dev/` nem de `install/` foi tocado.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/format.rs`, `crates/ward-core/src/format/tests.rs` (eram `formato.rs`, `formato/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/atomic_write.rs`, `crates/ward-core/src/atomic_write/tests.rs` (eram `gravacao.rs`, `gravacao/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/{backup,configuracao,corpo,crypto,wire_v1}.rs`, `backup/testes.rs`, `configuracao/testes.rs`, `corpo/testes.rs`, `entrada/testes.rs`, `protocolo/{erros,testes}.rs` | alterado (usos) |
| `crates/ward-core/tests/{corpo_memoria,fontes}.rs`, `crates/ward-core/examples/dev-cofre/{main,testes}.rs` | alterado (usos) |
| `crates/wardd/src/{arquivos,externo,nucleo}.rs`, `crates/wardd/tests/{anexos_edicao,app_valor,backup,d62,externo,memoria}.rs`, `crates/wardd/tests/comum/mod.rs` | alterado (usos) |
| `app/core/src/primeiro_uso.rs` | alterado (uso) |
| `dev/english-check/allow.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-11.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes renomeados; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
