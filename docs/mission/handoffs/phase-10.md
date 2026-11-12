# Fase 10 — `ward-core`: lib, limits, crypto, totp, generator, origin, display (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 6d8cda8`.
- **Arquivos** (`git mv`, glossário seção 4): `src/limites.rs` → `limits.rs`,
  `limites/testes.rs` → `limits/tests.rs`, `cripto.rs` → `crypto.rs`, `totp/testes.rs`
  → `totp/tests.rs`, `gerador.rs` → `generator.rs`, `gerador/testes.rs` →
  `generator/tests.rs`, `origem.rs` → `origin.rs`, `exibicao.rs` → `display.rs`. Módulos
  no `lib.rs`: `limits`, `crypto` (privado), `totp`, `generator`, `origin`, `display`; os
  das fases seguintes (`backup`, `configuracao`, `corpo`, `entrada`, `entrega`,
  `formato`, `mascara`, `protocolo`, `gravacao`) ficam com o nome antigo.
- **Comentários, locais, parâmetros, privados e nomes de teste** dos 10 arquivos em
  inglês (`ataque_*` → `attack_*`, `testes` → `tests`).
- **Renomes públicos** (antigo → novo), com todos os usos ajustados no repositório:
  - `limits`: `MAX_NOME` → `MAX_NAME`, `MAX_LINHA` → `MAX_LINE`, `MAX_USUARIO` →
    `MAX_USERNAME`, `MAX_NOME_EXTRA` → `MAX_EXTRA_NAME`, `MAX_NOME_ANEXO` →
    `MAX_ATTACHMENT_NAME`, `MAX_REGIAO` → `MAX_REGION`, `MAX_BASE` → `MAX_DB_NAME`,
    `MAX_BASE_REDIS` → `MAX_REDIS_DB_NAME`, `MAX_TEXTO` → `MAX_TEXT`, `MAX_SEGREDO` →
    `MAX_SECRET`, `MAX_VALOR_EXTRA` → `MAX_EXTRA_VALUE`, `MAX_NOTA` → `MAX_NOTE`,
    `MAX_ANEXOS` → `MAX_ATTACHMENTS`, `MAX_ANEXOS_SOMADOS` → `MAX_ATTACHMENTS_TOTAL`,
    `LIMITE_PADROES` → `PATTERN_LIMIT`, `ORCAMENTO_MASCARA` → `MASK_BUDGET`,
    `MINIMO_CONFIAVEL` → `TRUSTED_MINIMUM`, `MAX_CORPO` → `MAX_BODY`, `MAX_ENTRADAS` →
    `MAX_ENTRIES`, `MIN_CARACTERES_SENHA_MESTRA` → `MIN_MASTER_PASSWORD_CHARS`,
    `MIN_DIGITOS_PIN`/`MAX_DIGITOS_PIN` → `MIN_PIN_DIGITS`/`MAX_PIN_DIGITS`,
    `MAX_HASH_PIN` → `MAX_PIN_HASH`, `MINUTOS_TRAVA_PADRAO` → `DEFAULT_LOCK_MINUTES`,
    `MIN_MINUTOS_TRAVA`/`MAX_MINUTOS_TRAVA` → `MIN_LOCK_MINUTES`/`MAX_LOCK_MINUTES`,
    `MAX_UUID_BACKUP` → `MAX_BACKUP_UUID`, `MAX_DATA` → `MAX_DATE`, `MAX_ENTRADA` →
    `MAX_ENTRY`, `MAX_TEXTOS_ENTRADA` → `MAX_ENTRY_TEXTS`, `MAX_QUADRO_COMUM`/`_APP`/
    `_RESPOSTA` → `MAX_COMMON_FRAME`/`MAX_APP_FRAME`/`MAX_RESPONSE_FRAME`,
    `MAX_ARGUMENTO`/`MAX_ARGUMENTOS` → `MAX_ARGUMENT`/`MAX_ARGUMENTS`,
    `MAX_VARIAVEIS_AMBIENTE` → `MAX_ENV_VARIABLES`, `MAX_TEXTO_EXIBIDO` →
    `MAX_DISPLAYED_TEXT` (`KIB`, `MIB`, `MAX_URL`, `MAX_HOST`, `MAX_TAG`, `MAX_TAGS`,
    `MAX_EXTRAS`, `MIN/MAX_ACCESS_KEY`, `MIN/MAX_TOTP` ficam). Os reexports em
    `mascara.rs` e `corpo.rs` seguem os nomes novos.
  - `crypto` (`pub(crate)`): `Chave` → `Key` (`zerada` → `zeroed`, `de_bytes` →
    `from_bytes`; o `Key` do `chacha20poly1305` entra como `AeadKey`), `FalhaCripto` →
    `CryptoFailure`, `TAM_CHAVE`/`TAM_NONCE`/`TAM_TAG` → `KEY_SIZE`/`NONCE_SIZE`/
    `TAG_SIZE`, `derivar_kek` → `derive_kek`, `selar` → `seal`, `abrir` → `open`,
    `aleatorio` → `random`, `chave_aleatoria` → `random_key`, `TAM_SAL_PIN`/
    `TAM_HASH_PIN` → `PIN_SALT_SIZE`/`PIN_HASH_SIZE`, `PhcPin { custo }` →
    `PinPhc { cost }`, `ler_phc_pin` → `read_pin_phc`, `conferir_pin` → `verify_pin`,
    `iguais_em_tempo_constante` → `constant_time_eq`; privados `cifra` → `cipher`,
    `BufferFixo` → `FixedBuffer`, `escrever_phc` → `write_phc`.
  - `totp`: `PERIODO` → `PERIOD`, `DIGITOS_PADRAO` → `DEFAULT_DIGITS`,
    `MIN_SEMENTE`/`MAX_SEMENTE` → `MIN_SEED`/`MAX_SEED`, `ErroTotp` → `TotpError`
    (`InvalidSeed`, `InvalidDigits`, `InvalidClock`), `CodigoTotp` → `TotpCode`
    (`code()`, `remaining()`), `Totp::de_semente`/`com_digitos`/`codigo_em`/
    `codigo_agora` → `from_seed`/`with_digits`/`code_at`/`code_now`; privados
    `valor_base32` → `base32_value`, `decodificar_base32` → `decode_base32`.
  - `generator`: `TAMANHO_MIN`/`TAMANHO_MAX`/`TAMANHO_PADRAO` → `MIN_SIZE`/`MAX_SIZE`/
    `DEFAULT_SIZE`, `MINUSCULAS`/`MAIUSCULAS`/`DIGITOS`/`SIMBOLOS` → `LOWERCASE`/
    `UPPERCASE`/`DIGITS`/`SYMBOLS`, `OpcoesGerador { tamanho, minusculas, maiusculas,
    digitos, simbolos }` → `GeneratorOptions { size, lowercase, uppercase, digits,
    symbols }`, `ErroGerador` → `GeneratorError` (`InvalidSize`, `NoSet`,
    `RandomUnavailable`), `gerar` → `generate`, `gerar_com` → `generate_with`,
    `TAMANHO_TOKEN` → `TOKEN_SIZE`, `token_conectar` → `connect_token`,
    `tokens_iguais` → `tokens_equal`; privados `MAX_TENTATIVAS` → `MAX_ATTEMPTS`,
    `Reserva` → `Reserve`.
  - `origin`: `Origem { porta }` → `Origin { port }`, `da_url` → `from_url`,
    `mesma_origem` → `same_origin`.
  - `display`: `AVISO_CORTE` → `CUT_NOTICE`, `invisivel` → `invisible`,
    `exibir_argumentos` → `display_args`, `exibir_texto` → `display_text`,
    `neutralizar` → `neutralize`; privados `Montagem` → `Assembly` (`por` → `push`,
    `fim` → `finish`), `pedacos` → `chunks`, `escrever_char` → `write_char`.
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`backup`, `configuracao`,
  `corpo`, `entrada`, `formato`, `gravacao`, `mascara`, `protocolo` e testes,
  `tests/corpo_memoria.rs`, `tests/fontes.rs`, `examples/dev-cofre/main.rs`), `wardd`
  (`agente_ssh`, `execucao`, `login`, `nucleo`, `playwright`, `ssh`, `uso`,
  `app-teste/main.rs`; testes `aprovacao`, `configuracoes`, `extensao`, `ssh`,
  `valor_app`), `ward` (`cliente`, `comandos`, `testes`), `ward-nm` (`lib`, `main`),
  `app/core` (`anexos`, `aprovacao`, `area_transferencia`, `backup_externo`, `chave`,
  `comandos`, `configuracoes`, `daemon`, `daemon_falso`, `destrave`, `formulario`,
  `gerador`, `lib`, `nivel`, `primeiro_uso`, `servico`, `terminal`),
  `app/src-tauri/src/main.rs` (comentário) e `dev/tests/bidi_icu.py`.
- **Colisões evitadas**: `backup::MAX_NOME` (Fase 15), o `MAX_CORPO` da ponte de teste
  (Fase 29), os `TAMANHO_*`/`Opcoes` do `app/core/src/gerador.rs` (Fase 34), o
  `Resposta::CodigoTotp` do protocolo (Fase 17/18), os `neutralizar`/`invisivel`
  próprios de `crates/ward/src/texto.rs` e o `enum Origem` do `dev-cofre` não mudaram.
- **Testes de fio** (`wire_v1.rs`): não usam nenhum nome desta fase; arquivo e fixtures
  intactos.
- **Glossário**: parágrafo "Fase 10" na seção 15 (variantes, métodos e as constantes
  que não saem direto da seção 2).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt` (nenhum nome de fio nesta fase).

## Literais alterados (PRD regra 5)
- `totp.rs`: `Debug` manual do `Totp`: rótulos `"semente"`/`"digitos"` →
  `"seed"`/`"digits"` (o valor continua `"••••"`); captura `{valor:0width$}` →
  `{value:0width$}` (texto igual).
- `display.rs`: `"{AVISO_CORTE}{fora} bytes não mostrados]"` →
  `"{CUT_NOTICE}{left_out} bytes não mostrados]"` (texto exibido igual).
- `generator/tests.rs`: `include_str!("../gerador.rs")` → `"../generator.rs"` e o trecho
  procurado `"gerar_com(opcoes, &mut cripto::aleatorio)"` →
  `"generate_with(options, &mut crypto::random)"` (é o código-fonte).
- `tests/fontes.rs` (auditor do V2.46): caminho `"src/cripto.rs"` → `"src/crypto.rs"`,
  `.expect("crypto.rs")`, trechos procurados com `Key`, `KEY_SIZE`, `MAX_PIN_HASH` e
  `"output[..] =="`; e a lista de exclusão passou a pular também `tests`/`tests.rs`
  (senão os testes renomeados entravam como "produção"; ver Problemas).
- `dev/tests/bidi_icu.py`: caminho `"display.rs"`, regex `pub fn invisible\(` e a
  mensagem de erro com os nomes novos.
- Capturas de `format!`/`assert!` com o nome novo (V6.3 aceita): `{tipo}` → `{kind}`,
  `{chave}` → `{key}`, `{erro}` → `{error}`, `{custo}` → `{cost}`, `{erros}` →
  `{errors}`, `{rotulo}` → `{label}`, `{nome}` → `{name}`, `{tempo}` → `{time}`,
  `{texto}` → `{text}`, `{mascara:04b}` → `{mask:04b}`, `{tamanho}` → `{size}`,
  `{media}` → `{mean}`, `{aba:?}` → `{tab:?}`, `{entrada:?}` → `{input:?}`,
  `{mostrado}` → `{shown}`.
- **Ficaram**: todos os `Display` pt-BR (`TotpError`, `GeneratorError`), o aviso
  `" … [cortado: mais "`, o `WARD_EXIGIR_OATHTOOL` (Fase 56) e os valores falsos.
- **Conferido** por comparação da forma (tokens com identificadores e strings
  normalizados, sem comentários) entre cada arquivo antigo e o novo: iguais, fora o
  `use chacha20poly1305::{Key as AeadKey, …}` e dois campos escritos por extenso
  (`dados: data`, `memoria_kib: memory_kib`).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S6 (`crypto`): Argon2id, XChaCha20-Poly1305 com AAD, KEK e chave de dados no heap
    (`Key` = `Box<Zeroizing<[u8; 32]>>`, asserção de tamanho de ponteiro), PHC canônico
    com limites, comparação pelo `CtEq` do `ctutils`. Lógica igual; a auditoria por
    leitura do fonte (`tests/fontes.rs`, V2.46) acompanha o arquivo e os nomes novos.
  - R8/S13 (`totp`): semente em `Zeroizing`, `Debug` com `••••`, erros sem a semente.
  - R10/D3 (`generator`): RNG do sistema só, rejeição sem viés, token do "Conectar",
    comparação de token em tempo constante.
  - D9 (`origin`): mesma origem por esquema, host e porta, parser estrito.
  - D51 (`display`): escape visível, faixas de `invisible` iguais (o
    `dev/tests/bidi_icu.py` lê a função nova).
  - D32 (`limits`): números e asserções de compilação iguais.
- **Testes de ataque**: nenhum novo (só rename); os existentes renomeados junto
  (`attack_malformed_seed_is_refused`, `attack_modulo_bias`,
  `attack_failing_rng_does_not_generate_weak_password`, `open_refuses_any_flipped_bit`,
  `attack_lookalike_origins_do_not_match`,
  `attack_control_and_bidi_become_visible_escape`…). Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes dos módulos das Fases 11–18 usados nestes arquivos (`Segredo`, `entrada`,
  `TipoEntrada`, `Rascunho*`, `ParametrosKdf { memoria_kib, paralelismo }`, `corpo`,
  `mascara`, `entrega`, `configuracao`…): são os 405 achados que sobram no
  `english-check` desta fase, todos identificadores de outros módulos; nenhum
  comentário nem nome desta fase.
- Comentários pt-BR de outros arquivos que citam os nomes novos (ex.: `app/core`) ficam
  para as fases deles; aqui só o identificador citado mudou.
- `tests/fontes.rs` e `tests/corpo_memoria.rs` não estão em nenhuma fase do plano: só os
  usos foram ajustados (ver Problemas).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 6d8cda8` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (até ficar limpo) | 0 |
| `cargo fmt --all` / `cargo fmt --all --check` | 0 |
| `make compile` | 0 |
| `python3 -I dev/english_check.py --paths <os 10 arquivos da fase>` (= `make english-check PATHS=…`) | 1 (405 achados, todos nomes de módulos das Fases 11–18; 0 em comentário; 0 problema nas exceções) |
| comparação de forma e de literais antigo × novo (scripts na scratchpad) | — |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `6d8cda8`; com a árvore limpa, fiz
  `git reset --hard 6d8cda8`.
- **`crates/ward-core/tests/fontes.rs` e `tests/corpo_memoria.rs` sem fase no plano**: o
  `fontes.rs` pula os módulos de teste pelo nome (`testes`, `testes.rs`); com o rename
  para `tests.rs` os testes passariam a contar como produção. Acrescentei `tests`/
  `tests.rs` à exclusão (mudança de lógica de teste, pequena e necessária para o V2.46
  e o V2.50 continuarem certos). A tradução desses dois arquivos precisa entrar em
  alguma fase (sugestão: Fase 14 ou 18).
- **Trilha S**: esta fase mexeu em `dev/tests/bidi_icu.py` (5 linhas: caminho, regex,
  docstrings). Se a Fase 46 renomear ou reescrever o arquivo, o merge tem de levar o
  `display.rs`/`invisible`.
- `strings-0.1.4.txt` pode ter os rótulos do `Debug` do `Totp` (`"semente"`,
  `"digitos"`): o V6.3 deve aceitar a troca, como os `Debug` manuais das Fases 8 e 9.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/{limits,crypto,generator,origin,display}.rs` (eram `limites`, `cripto`, `gerador`, `origem`, `exibicao`) | renomeado e alterado |
| `crates/ward-core/src/{limits,totp,generator}/tests.rs` (eram `testes.rs`) | renomeado e alterado |
| `crates/ward-core/src/{lib,totp}.rs` | alterado |
| `crates/ward-core/src/{backup,configuracao,corpo,entrada,formato,gravacao,mascara,protocolo}.rs` e `backup/testes.rs`, `configuracao/testes.rs`, `corpo/{cofre_teste,testes}.rs`, `entrada/{testes,validacao}.rs`, `formato/testes.rs`, `gravacao/testes.rs`, `mascara/testes.rs`, `protocolo/{json,pedido,quadro,resposta,testes}.rs` | alterado (usos) |
| `crates/ward-core/tests/{corpo_memoria,fontes}.rs`, `crates/ward-core/examples/dev-cofre/main.rs` | alterado (usos) |
| `crates/wardd/src/{agente_ssh,execucao,login,nucleo,playwright,ssh,uso}.rs`, `crates/wardd/app-teste/main.rs`, `crates/wardd/tests/{aprovacao,configuracoes,extensao,ssh,valor_app}.rs` | alterado (usos) |
| `crates/ward/src/{cliente,comandos,testes}.rs`, `crates/ward-nm/src/{lib,main}.rs` | alterado (usos) |
| `app/core/src/{anexos,aprovacao,area_transferencia,backup_externo,chave,comandos,configuracoes,daemon,daemon_falso,destrave,formulario,gerador,lib,nivel,primeiro_uso,servico,terminal}.rs`, `app/src-tauri/src/main.rs` | alterado (usos) |
| `dev/tests/bidi_icu.py` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-10.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0) e `cargo fmt`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes renomeados; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
