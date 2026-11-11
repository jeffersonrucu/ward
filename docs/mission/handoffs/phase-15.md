# Fase 15 — `ward-core`: `backup` (+ testes) e `delivery` (+ testes) (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 3e810a0`.
- **Arquivos** (glossário seção 4): `src/backup/testes.rs` → `backup/tests.rs`,
  `src/entrega.rs` → `delivery.rs`, `src/entrega/testes.rs` → `delivery/tests.rs`
  (`git mv`); `lib.rs`: `pub mod delivery`. Os quatro traduzidos por inteiro (comentários,
  locais, parâmetros, nomes de teste e de ajudantes).
- **Renomes públicos de `backup`** (antigo → novo), com todos os usos no repositório:
  - `VERSOES` → `VERSIONS`, `MAX_SUFIXO` → `MAX_SUFFIX`, `MAX_NOME` → `MAX_NAME`,
    `MARGEM_FUTURO` → `FUTURE_MARGIN`.
  - `Versao` → `Version` (campos `data`/`hora`/`sufixo` → `date`/`time`/`suffix`;
    `do_instante`/`de_nome`/`nome`/`fim_do_segundo` → `from_instant`/`from_name`/`name`/
    `end_of_second`; privados `com_sufixo`/`possivel` → `with_suffix`/`is_possible`).
  - `ErroBackup` → `BackupError` (`PastaInsegura` → `UnsafeDir`, `NaoAutentico` →
    `NotAuthentic`, `Relogio` → `Clock`, `NomesEsgotados` → `NamesExhausted`, `Gravacao` →
    `Write`; `Io` fica).
  - `LerVersao` → `ReadVersion`; `PastaBackup` → `BackupDir` (campo `ler` → `read`;
    `nova`/`com_leitor`/`caminho`/`listar`/`listar_ate` → `new`/`with_reader`/`path`/`list`/
    `list_until`; `pub(crate)` `guardar`/`guardar_com_pontos` → `keep`/`keep_with_points`;
    privados `conferir`/`rodar` → `check`/`rotate`).
  - `para_apagar_conferidas` → `to_delete_verified`, `fora_de_ordem` → `out_of_order`,
    `geracao_selada` → `sealed_generation`, `guardar_atual` → `keep_current`,
    `nome_valido` → `valid_name`.
  - Privados: `BASE_TEMPORARIO` → `TEMP_BASE`, `digitos` → `digits`, `civil_de_dias` →
    `civil_from_days`, `teto_de` → `ceiling_of`, `ler_versao_padrao` →
    `read_version_default`.
- **Renomes públicos de `delivery`**:
  - `ErroRecurso` → `ResourceError`; trait `Recursos` → `Resources` (`codigo_totp`/
    `arquivo`/`tunel` → `totp_code`/`file`/`tunnel`).
  - `ErroEntrega` → `DeliveryError` (`TipoSemEntrega` → `TypeWithoutDelivery`, `Recurso` →
    `Resource`, `RecursoInvalido` → `InvalidResource`, `Invalida` → `Invalid`).
  - `Variavel` → `Variable` (`nome`/`expor`/`e_segredo` → `name`/`expose`/`is_secret`);
    `Entrega` → `Delivery` (`variaveis`/`valor`/`nomes` → `variables`/`value`/`names`);
    `montar` → `build`.
  - Privados: `Montagem` → `Builder` (`texto`/`segredo`/`arquivo`/`conferir` → `text`/
    `secret`/`file`/`check`; `push` fica), `prefixado` → `prefixed`, `autenticacao` →
    `auth`, `banco` → `database`, `url_do_banco` → `database_url`, `juntar` →
    `join_parts`, `livre` → `unreserved`, `codificar_em` → `encode_into`,
    `texto_do_segredo` → `secret_text`, `nome_de_variavel_valido` → `valid_variable_name`.
- **Testes**: ajudantes e nomes de teste em inglês (ex.: `backup`: `Dir::novo`/`cofre`/
  `pasta`/`nomes`/`temporarios` → `new`/`vault`/`backup_dir`/`names`/`temp_files`,
  `cofre_novo` → `new_vault`, `plantar` → `plant`, `filho`/`esperar_linha`/`matar` →
  `child`/`wait_line`/`kill`, `tres_versoes`/`versoes_seladas`/`selar_proxima`/
  `conferir_pasta` → `three_versions`/`sealed_versions`/`seal_next`/`check_dir`,
  `PONTOS`/`ANTES_DO_LINK`/`VIDA_FILHO`/`INSTANTE`/`NOME_FILHO`/`ENV_MODO` →
  `POINTS`/`BEFORE_LINK`/`CHILD_LIFETIME`/`INSTANT`/`CHILD_NAME`/`ENV_MODE`;
  `filho_backup` → `child_backup`, `v7_1_31_gravacoes_deixam_30_versoes` →
  `v7_1_31_writes_leave_30_versions`, `f54c_ataque_lixo_com_data_mais_nova_nao_apaga_versoes_reais`
  → `f54c_attack_junk_with_newer_date_does_not_delete_real_versions` etc.; `delivery`:
  `Falsos` → `Fakes`, `rascunho` → `draft`, `valida` → `valid`, `confere` → `check`,
  `pares` → `pairs`, `sem_segredo` → `no_secret`, `SEGREDOS` → `SECRETS`, `chave_falsa` →
  `fake_key`, `todas_com_entrega` → `all_with_delivery`, `NaoClone`/`ImplementaClone`/
  `zera_ao_sair` → `NotClone`/`ImplementsClone`/`zeroes_on_drop`,
  `v2_14_entrega_esperada_do_contrato` → `v2_14_expected_delivery_of_the_contract` etc.).
  Nenhum script, teste nem contrato cita esses nomes de teste (procurado); os logs da
  missão 1 em `docs/mission/arquivo/` citam os antigos e ficam.
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`atomic_write.rs`,
  `mascara.rs`, `mascara/testes.rs`, `limits/tests.rs`, `totp/tests.rs`,
  `protocolo/pedido.rs`, `protocolo/resposta.rs`, `examples/dev-vault/main.rs`), `wardd`
  (`src/arquivos.rs`, `src/execucao.rs`, `src/externo.rs`, `src/nucleo.rs`,
  `tests/backup.rs`, `tests/externo.rs`) e `app/core` (`daemon_falso.rs`, `restaurar.rs`).
  Feito por script (scratchpad, não versionado): regex de palavra inteira para os tipos e
  caminhos qualificados (`backup::…`, `delivery::…`), os `impl Resources for` e os nomes
  dos métodos do trait nos implementadores (`RecursosRun`, `RecursosDoOrcamento`,
  `Falsos`, `Fakes`, `FixedClock`, `FakeResources`); depois os métodos pelos diagnósticos
  `E0599` do `cargo check` (troca só no span de cada erro cujo tipo é `Delivery`,
  `Variable`, `BackupDir` ou `Version`), até 0 erro.
- **Glossário**: parágrafo "Fase 15" depois do da Fase 14 (seção 15).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`. Os comentários que citavam
  nomes gravados ou entregues (`nota segura`, `servidor`, `banco`, `ftp/sftp`,
  `WARD_ARQUIVO_<NOME>`, `chave`) foram reescritos sem o nome.

## Literais alterados (PRD regra 5)
- `backup.rs` e `delivery.rs`: **nenhum** literal mudou (multiconjunto igual). As mensagens
  pt-BR de `BackupError`/`DeliveryError` (`Display`) e os textos fixos de recurso ficam.
- `backup/tests.rs`: `CHILD_NAME` `"backup::testes::filho_backup"` →
  `"backup::tests::child_backup"` (caminho do teste que o processo filho roda; acompanha o
  módulo e a função, como na Fase 11); pasta em `target/<perfil>/` `"ward-teste-backup"` →
  `"ward-test-backup"`; nomes das subpastas de teste (`Dir::new`) `"sufixo"`, `"troca"`,
  `"pasta-aberta"`, `"link-versao"`, `"atual"`, `"atual-outro"`, `"futuro"`, `"f54c-lixo"`,
  `"f54c-lixo-outro"`, `"f54c-troca"`, `"f54c-relogio"`, `"geracao"`, `"geracao-outro"`,
  `"v7-3-ponto"`, `"v7-3-aleatorio"` → `"suffix"`, `"change"`, `"open-dir"`,
  `"version-link"`, `"current"`, `"current-other"`, `"future"`, `"f54c-junk"`,
  `"f54c-junk-other"`, `"f54c-change"`, `"f54c-clock"`, `"generation"`,
  `"generation-other"`, `"v7-3-point"`, `"v7-3-random"` (todas apagadas no fim do teste,
  como na Fase 14); capturas de `format!` (`{nome}` → `{name}`, `{ruim:?}` → `{bad:?}`,
  `{nomes:?}` → `{names:?}`, `{fora:?}` → `{out:?}`, `{alvo:?}` → `{target:?}`,
  `{esperada}` → `{expected}`, `{antes}` → `{before}`, `{MORTES}`/`{com_temporario}`/
  `{uma:?}` → `{DEATHS}`/`{with_temp}`/`{one:?}`; texto igual, V6.3).
- `delivery/tests.rs`: só capturas de `format!` (`{rotulo}` → `{label}`, `{nome}` →
  `{name}`, `{erros}` → `{errors}`, `{erro}` → `{error}`, `{onde}` → `{context}`,
  `{porta}` → `{port}`, `{motor}` → `{engine}`, `{tipo}` → `{kind}`, `{depois}` →
  `{after}`, `{codigo:?}` → `{code:?}`, `{caminho:?}` → `{path:?}`, `{entrega:?}` →
  `{delivery:?}`).
- **Ficaram**: as variáveis de ambiente do filho `WARD_TESTE_BACKUP_{MODO,DIR,BASE}` (DT12,
  Fase 56; os identificadores Rust viraram `ENV_MODE`/`ENV_DIR`/`ENV_BASE`), os modos
  `"ponto:"`/`"laco"` e as marcas `WARD-PONTO`/`WARD-FIM`/`WARD-PRONTO`/`WARD-ACABOU`, a
  subpasta `"backups"`, `"cofre.ward"`, `"alvo"`, `"nao-existe"`, a magia `WARDCOFR`, os
  valores falsos de D15, todos os textos de `panic!`/`assert!` (inclusive os que citam
  `anexos_do_tipo`/`variaveis_do_tipo`), o campo `"base"` do erro de validação e **todos os
  nomes das variáveis entregues** (`WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`,
  `WARD_PROTOCOLO`, `WARD_ARQUIVO_*`, `WARD_ARQUIVO_CHAVE`…, Fase 53) e o nome do arquivo
  da chave `"chave"` passado a `Resources::file`.
- Nenhum literal mudou nos arquivos dependentes (conferido no diff).
- **Conferido por forma** (tokens com identificadores normalizados, sem comentários; script
  na scratchpad): os quatro arquivos iguais ao antigo, fora o campo abreviado
  (`private_key: chave_privada` → `private_key`, `tunnel: tunel`, `secret: segredo`,
  `data: dados`) e um braço do `Display` de `BackupError` sem chaves (rustfmt).
- **Debug derivado**: `BackupError` e `DeliveryError` agora imprimem `UnsafeDir`,
  `NotAuthentic`, `TypeWithoutDelivery`…; procurei os nomes antigos em scripts, testes e
  `dev/`: nenhum uso (só `strings-0.1.4.txt`, que o V6.3 deve aceitar). O `wardd`
  registra o backup pelo texto fixo de `codigo_backup` (`nucleo.rs`), que não mudou.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S2/S7 (backup): mesma versão selada, sem selar de novo; gravação por
    `atomic_write::write_new_with_points` (`O_EXCL` 0600, `link(2)` sem sobrescrever, fsync
    do diretório); temporário limpo antes; rotação só do que confere pela chave
    (`to_delete_verified`, Fase 54c), teto do segundo (Fase 49a), só arquivo comum, link
    nunca seguido nem apagado; pasta 0700 do uid conferida em toda operação; leitor sem
    seguir link e com `dev`/`ino` iguais ao `lstat`. Mesma lógica, mesma ordem.
  - D17/D39: `keep_current` só aceita o arquivo que `authenticates`; a geração não sai do
    daemon (só a marca de `out_of_order`).
  - S8/S13 (entrega): `Zeroizing<String>` no tamanho exato (URL em uma alocação),
    `ZeroizeOnDrop`, sem `Clone`, `Debug` só com nomes, erros de texto fixo; percent-encoding
    de usuário/senha/base; conferência final (nome `[A-Z_][A-Z0-9_]*`, sem `LD_*`, sem
    repetido, sem nulo); recusa da nota segura antes de qualquer recurso; código TOTP, caminho
    e porta do túnel conferidos.
  - Nomes das variáveis entregues congelados (Fase 53).
- **Testes de ataque**: nenhum novo (só rename); os existentes (nome fora do formato, pasta
  aberta, link com nome de versão, cofre atual não autêntico, datas futuras, lixo da Fase
  54c, geração forjada, SIGKILL em cada ponto e em 100 pontos; URL que muda de destino,
  extra que vira variável do sistema, recurso que devolve lixo, `Debug`/erros sem segredo,
  defesa da lista) acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Locais e comentários pt-BR dos arquivos que usam os nomes novos ficam para as fases deles
  (`mascara*`, Fase 16; `protocolo/*`, Fases 17–18; `wardd`, Fases 19–27; `app/core`,
  Fases 31–33; `examples/dev-vault`, já da trilha S).
- Os homônimos `ErroResposta::TipoSemEntrega`/`EntregaFalhou`, `protocolo::Variavel`,
  `VersaoBackup` e o campo de fio `fora_de_ordem` (Fases 17, 18 e 50), `ErroMascara::SemEntrega`
  e `RecursosDoOrcamento` (Fase 16), `RecursosRun` (Fase 21).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 3e810a0` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (log `.dev/logs/fase15-check.log`; 6 rodadas com os diagnósticos `E0599` dos métodos) | 0 |
| `cargo fmt --all` | 0 |
| `python3 -I dev/english_check.py --paths crates/ward-core/src/backup.rs crates/ward-core/src/backup/tests.rs crates/ward-core/src/delivery.rs crates/ward-core/src/delivery/tests.rs` (log `fase15-english.log`) | 0 (0 achado) |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra; log `fase15-clippy.log`) | 0 |
| `make compile` (log `fase15-make-compile.log`; inclui o `cargo check` do `app/src-tauri`, build do Vite e `syntax_check.py`) | 0 |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `3e810a0`; reset com a árvore limpa.
- **Arquivos de fases paralelas tocados (só usos)**: `crates/wardd/src/execucao.rs` (Fase
  21): import, `impl Resources for RecursosRun` com `totp_code`/`file`/`tunnel`,
  `ResourceError(…)`, `&Delivery`, `names()`/`variables()`/`name()`/`expose()`,
  `delivery::build` e `DeliveryError::TypeWithoutDelivery` (e o link do doc do topo).
  Nada da Fase 28 (`crates/ward`, `crates/ward-nm`) nem da Fase 30 (`app/core`: `servico`,
  `segredo`, `erro`, `lib`). Em `app/core` só `daemon_falso.rs` (Fase 31) e `restaurar.rs`
  (Fase 32).
- **Arquivo da trilha S tocado**: `crates/ward-core/examples/dev-vault/main.rs`, só os usos.
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 14).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/backup.rs` | reescrito |
| `crates/ward-core/src/backup/tests.rs` (era `backup/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/delivery.rs` (era `entrega.rs`) | renomeado e reescrito |
| `crates/ward-core/src/delivery/tests.rs` (era `entrega/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/{atomic_write,mascara}.rs`, `mascara/testes.rs`, `limits/tests.rs`, `totp/tests.rs`, `protocolo/{pedido,resposta}.rs` | alterado (usos) |
| `crates/ward-core/examples/dev-vault/main.rs` | alterado (usos) |
| `crates/wardd/src/{arquivos,execucao,externo,nucleo}.rs`, `crates/wardd/tests/{backup,externo}.rs` | alterado (usos) |
| `app/core/src/{daemon_falso,restaurar}.rs` | alterado (usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-15.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
