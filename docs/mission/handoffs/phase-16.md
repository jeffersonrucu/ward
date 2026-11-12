# Fase 16 — `ward-core`: `mask` (+ `automaton`, `tests`) (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard efa6865`
  (master atual).
- **Arquivos** (glossário seção 4): `src/mascara.rs` → `src/mask.rs`,
  `src/mascara/automato.rs` → `src/mask/automaton.rs`, `src/mascara/testes.rs` →
  `src/mask/tests.rs` (`git mv`); `lib.rs`: `pub mod mask`. Os três traduzidos por inteiro
  (comentários, locais, parâmetros, campos privados, nomes de teste e de ajudantes).
- **Renomes públicos** (antigo → novo), com todos os usos no repositório:
  - módulo `mascara` → `mask`; `MARCA` → `MARK`.
  - `ErroMascara` → `MaskError` (`GrandeDemais` → `TooLarge`, `SemEntrega` → `NoDelivery`).
  - `ConstrutorMascara` → `MaskBuilder` (`entrada`/`entrega`/`construir` → `entry`/
    `delivery`/`build`).
  - `Mascara` → `Mask` (`da_entrada`/`tem_segredo_curto`/`fluxo`/`mascarar_tudo` →
    `from_entry`/`has_short_secret`/`stream`/`mask_all`).
  - `Fluxo` → `Stream` (`mascarar`/`terminar` → `mask`/`finish`).
  - `custo_da_entrada` → `entry_cost`; `pub(crate) somar` → `sum_patterns`.
- **Renomes privados** (lista completa no parágrafo "Fase 16" do glossário): `Gerador` →
  `Generator` e os métodos dele, `RecursosDoOrcamento` → `BudgetResources`, `visitar`/
  `Visitante` → `visit`/`Visitor`, `Modo { Cru, Percent, Formulario }` → `Mode { Raw,
  Percent, Form }`, `Escape { Nada, … }` → `Escape { Nothing, … }`, `Visao` → `View`
  (`andar`/`ler`/`fechar` → `walk`/`read`/`close`), `cobrir` → `cover`, campos do `Stream`
  (`pendente`/`emitido`/`lido`/`visoes`/`cobertos`/`em_marca` → `pending`/`emitted`/
  `consumed`/`views`/`covered`/`in_mark`, `emitir_ate` → `emit_until`), `JsonEscapado` →
  `EscapedJson`, as funções de forma de texto e de parser de linha; `Automato` →
  `Automaton` (campos e métodos), `NENHUM`/`RAIZ` → `NO_NODE`/`ROOT`, `reservar`/
  `empurrar`/`indice` → `reserve`/`push`/`index`.
- **Testes**: ajudantes e nomes em inglês (`rascunho`/`valida`/`anexo`/`generico`/
  `gama_generico`/`entrega`/`mascara` → `draft`/`valid`/`attachment`/`generic`/
  `gama_generic`/`delivery_of`/`mask_of`; `em_pedacos`/`nada_segurado`/`um_bloco`/`texto`/
  `sem_pedaco` → `in_chunks`/`nothing_held`/`one_block`/`text`/`no_piece`; `Sorteio`
  (`proximo`/`ate`) → `Draw` (`next`/`below`); `SENHA_PG`/`URL_PG`/`CODIGO_FALSO`/
  `SENHA_SSH`/`FIM_CURTO` → `PG_PASSWORD`/`PG_URL`/`FAKE_CODE`/`SSH_PASSWORD`/`SHORT_END`;
  `Falsos` → `Fakes`; `NaoClone`/`ImplementaClone`/`zera_ao_sair` → `NotClone`/
  `ImplementsClone`/`zeroes_on_drop`; `oraculo_cobre`/`oraculo_responde` →
  `oracle_covers`/`oracle_answers`; `v2_56_printf_q_igual_ao_bash` →
  `v2_56_printf_q_equal_to_bash` (citado no comentário de `printf_q_escapes`) etc.). 51
  testes antes e depois. Nenhum script, Makefile, contrato nem teste de fora cita os nomes
  de teste (procurado).
- **Usos ajustados** (só os nomes desta fase): `ward-core` (`lib.rs`, `entry/validation.rs`,
  `limits/tests.rs`, `examples/dev-vault/main.rs`) e `wardd` (`src/execution.rs`,
  `src/ssh.rs`, `src/ponte.rs`, `src/playwright.rs`). Feito por regex de palavra inteira
  nos tipos e caminhos qualificados, e os métodos pelos diagnósticos `E0599` do `cargo
  check` (troca só no span de cada erro cujo tipo é `MaskBuilder`, `Mask` ou `Stream`; um
  `Arc<Mask>::fluxo` à mão), até 0 erro. Os locais pt-BR desses arquivos (`mascara`,
  `mascarado`, `tem_segredo`) ficam para as fases deles.
- **Glossário**: parágrafo "Fase 16" depois do da Fase 15 (seção 15). Nota: aqui "emitir"
  (soltar saída) virou `emit` (`emit_until`, `emitted`), não o `issue` do token.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`. Os comentários que citavam
  nomes de dados ou de arquivos pt-BR (`"Segurança"` do escopo, `delta-api-simbolos`, os
  repros `desvio.rs`/`contorno19e.rs`/`oraculo_pty.py`/`tempo.py` do auditor, o valor
  `falso-PG%3A…` e `"-fim"`) foram reescritos sem o nome.

## Literais alterados (PRD regra 5)
Conferido por multiconjunto de literais (script na scratchpad, comentários fora):
- `mask.rs`: só o `Debug` dos três tipos, que acompanha o nome (como o `"Secret(••••)"` da
  Fase 12): `"ConstrutorMascara(••••)"` → `"MaskBuilder(••••)"`, `"Mascara(••••)"` →
  `"Mask(••••)"`, `"Fluxo(••••)"` → `"Stream(••••)"`. Nenhum script nem teste procura
  esses textos (só `strings-0.1.4.txt`, que o V6.3 deve aceitar). As mensagens pt-BR do
  `Display` de `MaskError` e o `"/orcamento"` ficam.
- `automaton.rs`: nenhum (os `expect` pt-BR ficam).
- `tests.rs`:
  - capturas de `format!` (texto igual, V6.3): `{SENHA_PG}`/`{URL_PG}`/`{CODIGO_FALSO}` →
    `{PG_PASSWORD}`/`{PG_URL}`/`{FAKE_CODE}`, `{forma}` → `{form}`, `{colado}` →
    `{glued}`, `{maior}`/`{menor}` → `{larger}`/`{smaller}`, `{caso}` → `{case}`,
    `{saida}` → `{output}`, `{fora}` → `{masked}`, `{alvo}` → `{target}`, `{valor}`/
    `{VALOR}` → `{value}`/`{VALUE}`, `{um}`/`{dois}` → `{one}`/`{two}`, `{linha}` →
    `{line}`, `{rotulo}` → `{label}`, `{nome}` → `{name}`, `{custo}` → `{cost}`,
    `{rodada}` → `{round}`, `{prefixo}`/`{sobra}` → `{prefix}`/`{leftover}`, `{erro}` →
    `{error}`, `{cand}` → `{candidate}`, `{maior}` (bytes) → `{longest}`.
  - **o script Python do teste cruzado com o oráculo** (dois iguais, em
    `v2_57_oracle_and_mask_with_d54_values` e `oracle_covers`): `v.variacoes(…, anexo=…)`
    → `v.variations(…, attachment=…)`, o nome de módulo `'variacoes'` → `'variations'` e o
    local `linha` → `line`. Motivo: é nome (a API do oráculo). A trilha S (`dev/variations.py`,
    Fase 42) renomeou `variacoes`/`anexo` para `variations`/`attachment` e estes dois
    scripts não acompanharam: no Milestone 6 os testes V2.57/V3.29/V3.31/V3.32 falhariam
    com `AttributeError`. O `dev/variations.py` não mudou (oráculo independente).
  - **Ficaram**: todos os dados de teste (valores falsos de D15 e dos casos, nomes de entrada
    e de extra, `teste-generico`, `REGIAO`, `CHAVE_LICENCA`…), as variáveis de ambiente
    locais dos scripts de teste (`NOTA`, `VALORES`, `V`, `A`), todos os textos de
    `panic!`/`assert!`/`eprintln!` e os nomes de caso em pt-BR.
- Nenhum literal mudou nos arquivos dependentes, fora as capturas `{MARCA}` → `{MARK}` em
  três mensagens do `examples/dev-vault/main.rs` (texto igual).
- **Incidente corrigido durante a fase**: a ferramenta de escrita decodificou os escapes
  `\uXXXX` dentro de strings cruas dos testes (e de um comentário do `mask.rs`). A
  conferência de literais acusou; as 17 linhas foram restauradas byte a byte por script
  (escapes montados sem `\u` no fonte), e a conferência final mostra só as diferenças
  acima.
- **Conferido por forma** (tokens com identificadores normalizados, sem comentários):
  `mask.rs` (8853 tokens) e `automaton.rs` (1295) iguais ao antigo; `tests.rs` igual fora
  quatro campos abreviados (`data: dados` → `data`, `secret: segredo` → `secret`) e uma
  vírgula final (rustfmt).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S4/D11/D34/D36/D37/D54/D58/D61 (máscara): mesmos padrões gerados pelo mesmo
    `Generator` (literal, base64 e miolos, Basic, formas de texto, passada por linha, valor
    de linha, JSON na linha, campos de tab), as três visões (cru, percent, formulário) no
    mesmo autômato, nada segurado entre pedaços (sem oráculo de prefixo), trechos
    sobrepostos numa marca só. Mesma lógica, mesma ordem (conferido por forma).
  - S8: `Zeroizing` em todas as tabelas e buffers, `reserve` que não deixa cópia, sem
    `Clone`, `ZeroizeOnDrop`, `Debug` sem conteúdo (só o nome do tipo mudou).
  - D29: `entry_cost` com os mesmos recursos falsos (porta do túnel máxima) e o mesmo
    teto `MASK_BUDGET`; `MaskError::TooLarge` falha fechado no `ward run`.
- **Testes de ataque**: nenhum novo (só rename); os existentes (pedaços quebrados, base64
  maior, segredos sobrepostos, JSON escapado, saída patológica, `Debug` sem segredo,
  anexo grande demais, oráculo de prefixo da auditoria, prefixo certo × errado, tempo,
  percent pela metade, oráculo cruzado) acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Locais e comentários pt-BR dos arquivos que usam os nomes novos ficam para as fases deles
  (`wardd`: `execution.rs`/`ssh.rs` já da Fase 21; `ponte.rs`/`playwright.rs` da Fase 22,
  em paralelo; `examples/dev-vault` da trilha S).
- Os homônimos `ErroCodificar::GrandeDemais`, `ErroQuadro::GrandeDemais` e
  `ErroResposta::TipoSemEntrega`/`EntregaFalhou` (Fase 17).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard efa6865` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` (logs `.dev/logs/fase16-check.{log,json}`; 6 rodadas com os diagnósticos `E0599`) | 0 |
| `cargo fmt --all` | 0 |
| `make english-check PATHS="crates/ward-core/src/mask.rs crates/ward-core/src/mask/automaton.rs crates/ward-core/src/mask/tests.rs"` (log `fase16-english.log`) | 0 (0 achado) |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extra; log `fase16-clippy.log`) | 0 |
| `make compile` (log `fase16-make-compile.log`; inclui o `cargo check` do `app/src-tauri`, build do Vite e `syntax_check.py`) | 0 |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `efa6865`; reset com a árvore limpa.
- **Teste cruzado com o oráculo quebrado desde a Fase 42**: os scripts Python embutidos em
  `mascara/testes.rs` chamavam `variacoes(…, anexo=…)`, que o `dev/variations.py` não tem
  mais (`variations(…, attachment=…)`). Corrigido aqui (nome da API, PRD regra 5). Vale
  o M6 conferir se outro chamador do oráculo ficou com o nome velho (procurei em
  `*.rs`, `*.py`, `*.sh`: nenhum).
- **Arquivos de fases paralelas tocados (só usos)**: `crates/wardd/src/ponte.rs` e
  `crates/wardd/src/playwright.rs` (Fase 22): imports, `Mask`, `Mask::from_entry`,
  `.stream()`/`.mask(…)`/`.finish(…)`, `mask_all`; nenhum local renomeado. `execution.rs` e
  `ssh.rs` (Fase 21, concluída) idem. Nada nas Fases 24 (testes `ssh.rs`/`run.rs`) e 31
  (`app/core`).
- **Arquivo da trilha S tocado**: `crates/ward-core/examples/dev-vault/main.rs`, só os usos.
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 15).
- **Ferramenta**: a escrita de arquivo decodifica `\uXXXX` no texto; quem traduzir arquivo
  com esses escapes (protocolo, extensão, `app/core`) deve conferir os literais por script.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/mask.rs` (era `mascara.rs`) | renomeado e reescrito |
| `crates/ward-core/src/mask/automaton.rs` (era `mascara/automato.rs`) | renomeado e reescrito |
| `crates/ward-core/src/mask/tests.rs` (era `mascara/testes.rs`) | renomeado e reescrito |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/entry/validation.rs`, `crates/ward-core/src/limits/tests.rs` | alterado (usos) |
| `crates/ward-core/examples/dev-vault/main.rs` | alterado (usos) |
| `crates/wardd/src/{execution,ssh,ponte,playwright}.rs` | alterado (usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-16.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase

## Merge com o master `5162340` (Fases 22, 24, 31)
- Conflito modify/delete em `crates/wardd/src/ponte.rs` (a Fase 22 o renomeou para
  `bridge.rs`): `ponte.rs` apagado e os usos desta fase aplicados no `bridge.rs`
  (`ward_core::mask::Mask`, `Arc<Mask>`, `mask.stream()`, `m.mask(…)`, `m.finish(…)`).
- Conflito de conteúdo em `crates/wardd/src/playwright.rs`: ficou a versão da Fase 22
  (`has_secret`, `mask`), com `Mask`, `mask_all` e `Mask::from_entry`.
- Glossário com as partes das Fases 15, 16, 21, 22, 24 e 31. Nenhum uso antigo restou
  (procurado em `crates/` e `app/`).
- Literais conferidos de novo por script nos arquivos tocados: iguais ao `5162340` fora
  as capturas `{MARCA}` → `{MARK}` do `dev-vault`; `mask*` iguais à conferência de antes.
- `cargo check` (log `.dev/logs/fase16-merge-check.log`) 0; `make compile` (log
  `.dev/logs/fase16-merge-make-compile.log`) 0.
