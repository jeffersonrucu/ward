# Fase 35 — (UI) Base do `app/src`

## Implementado

Troca 1:1 pelo `docs/glossario.md` (seções 4 e 14, mais as entradas novas desta fase),
sem mudar comportamento nem texto visível em pt-BR.

- **Pastas** (`git mv`): `app/src/telas` → `screens`, `lib/componentes` → `lib/components`,
  `lib/estilo` → `lib/style`, `lib/ponte` → `lib/bridge`, `lib/teste` → `lib/test`.
- **Arquivos**: `ponte/comandos.js` → `bridge/commands.js`, `ponte/ponte-teste.js` →
  `bridge/test-bridge.js`, `teste/Vitrine.svelte` → `test/Showcase.svelte`,
  `icones.js` → `icons.js`, `tipos.js` → `types.js`, `niveis.js` → `levels.js`,
  `ward-simbolo.svg` → `ward-symbol.svg`, `app/scripts/gerar_design.py` →
  `generate_design.py`; os 36 componentes da tabela do glossário (`AcaoDeCampo` →
  `FieldAction` … `TituloDeSecao` → `SectionTitle`; `Logo`, `Modal`, `Select`, `Toast`
  ficam).
- **Ícones** (`app/design/icons/*.svg`, chaves de `ICONS`): `alerta` → `alert`, `busca` →
  `search`, `cadeado` → `lock`, `chave` → `key`, `chevron-baixo/direita` →
  `chevron-down/right`, `clipe` → `paperclip`, `copiar` → `copy`, `engrenagem` → `gear`,
  `erro` → `error`, `fechar` → `close`, `filtro` → `filter`, `gerador` → `generator`,
  `mais` → `plus`, `olho(-fechado)` → `eye(-closed)`, `relogio` → `clock`, `sucesso` →
  `success`, `tipo-*` → `type-*` (`tipo-banco` → `type-database`, `tipo-generico` →
  `type-generic`, `tipo-nota` → `type-note`, `tipo-servidor` → `type-server`); `check`
  fica. Todos os usos no app e em `extension/popup.js` (chaves montadas incluídas: os
  ícones de tipo saem agora de `types.js`, `TYPES[k].icon`).
- **Bandeja**: `app/src-tauri/icons/bandeja-{destravado,travado}-32.png` →
  `tray-{unlocked,locked}-32.png`, com o `include_image!` do `app/src-tauri/src/main.rs`.
- **Gerador de tokens** reescrito em inglês: `TOKEN_NAMES` (pt → en de cada variable),
  `COLLECTION_NAMES`/`MODE_NAMES` (comentários de seção e modos do tom do ícone), saída
  `src/lib/style/tokens.css` e `src/lib/components/icons.js` (`ICONS`). `tokens.css` e
  `icons.js` regenerados; V3.6 conferido por script: os tokens antigos com os nomes
  trocados pelo mapa são iguais aos novos, valor a valor (sem comentários).
- **Tokens CSS**: 244 propriedades custom e as 15 classes globais renomeadas (seção 14 do
  glossário) em `app/src/**`, `extension/popup.css`, `popup.js` e `popup.html`;
  `data-tom-icone` → `data-icon-tone`; locais do `Slider` `--progresso`/`--ponto` →
  `--progress`/`--point`.
- **Componentes** (`lib/components/*`): identificadores, props, classes locais, ids
  estáticos e comentários em inglês (mapa de props no glossário, seção 4). Módulos:
  `TYPES`/`entryType()` com `{ label, icon }`; `ACCESS_LEVELS`/`LEVEL_ORDER`/
  `accessLevel()` com `{ label, lower, icon, card, option }`; as chaves do fio (`'banco'`,
  `'com-aprovacao'`…) ficam, entre aspas.
- **`bidi.js`**: `partesBidi` → `bidiParts`, partes `{ text, rtl }`; a regex RTL
  ficou byte a byte igual (conferido).
- **`main.js`, `App.svelte`, `base.css`**: imports, identificadores, classes (`.casca` →
  `.shell`, `.janela` → `.window`, `.painel` → `.panel`, `.estado` → `.status`) e
  comentários.
- **Usos nas telas, no App e na Showcase** (por script com leitor de tags que respeita
  chaves, revisado): nomes das tags, imports, props, `bind:`, valores de `variant`/`kind`/
  `tone`, snippets `rodape`/`controle`/`descricao`/`acao` → `footer`/`control`/
  `description`/`action`, `:global(.botao|.campo|.destaque)` → `.button|.field|.highlight`.
  Objetos de opção do `Select` `{ valor, rotulo }` → `{ value, label }` (em
  `formulario.js`, `configuracoes.js`, `Credenciais.svelte`, `Formulario.svelte`,
  `Showcase.svelte`, com o `.find((x) => x.value === …)` de `formulario.js`). O
  `FiltersDialog` recebe/devolve `{ group, kind, environment }`; `Credenciais.svelte`
  converte para o objeto de filtros dela (`grupo`/`tipo`/`ambiente`), que fica para a
  Fase 36.
- **Exports da ponte** puxados para cá para a base ficar limpa (só o nome exportado e
  os usos): `chamar` → `call`, `ouvirAvisos` → `listenNotices`, `telaPedida` →
  `requestedScreen`. O resto de `lib/bridge/*` (identificadores e comentários) é da
  Fase 38.
- **Props das telas que o `App` usa**, por desestruturação com alias (o corpo das telas
  fica para as Fases 36–38): `Credenciais`/`Configuracoes` `ontravado` → `onlocked`,
  `Destravar` `pedido`/`ondestravado`/`onerro` → `request`/`onunlocked`/`onerror`,
  `PrimeiroUso` `oncriado` → `oncreated`. No `App`, os imports das telas usam o nome
  novo (`Settings`, `Credentials`, `Unlock`, `PasswordGenerator`, `ApprovalWindow`,
  `FirstUse`) apontando para o arquivo antigo, que a fase de cada tela renomeia.
- **Fora do `app/src`**: `dev/extension/build.py` (caminhos de `tokens.css`, `icons.js`,
  `ward-symbol.svg`; `dist/icons.js`), `extension/popup.js` (`ICONS` de `./icons.js`),
  `app/core/tests/configuracao_tauri.rs` (caminhos `src/screens/…`,
  `src/lib/bridge/commands.js`), `dev/tests/bidi.mjs` (`bidiParts`, `.text`),
  `dev/tests/revealed_selection.py` (`.revealed`, `.field`, `dataset.value`),
  `dev/tests/app-dist.py` e `app-release.sh` (marcas `test-bridge` e `Showcase` junto
  com as antigas), `CLAUDE.md` (caminho do `tokens.css`).
- **Glossário**: módulos de `lib/components`/`lib/bridge`, tabela dos ícones, props,
  eventos, snippets, valores e atributos `data-*` dos componentes, e a regra de citar o
  node do FigCraft pelo id nos comentários.

### Literais alterados (PRD regra 5; todos são nomes)

- Caminhos de import (`./lib/components/…`, `../bridge/index.js`, `./icons.js`…).
- Nomes de ícone (acima) e valores de prop: `variant` `primary/secondary/danger/ghost`,
  `kind` do Toast `success/error/warning`, `kind` do Highlight
  `neutral/warning/info/danger/prod`, `tone` `on-color/strong/disabled/brand/danger/
  warning/success`.
- Ids e prefixos de id gerados pelos componentes: `modal-titulo` → `modal-title`,
  `filtros-titulo` → `filters-title`, `nivel-titulo` → `level-title`,
  `popover-gerador-titulo` → `generator-popover-title`, `popover-senha/tamanho/
  minusculas/maiusculas/digitos/simbolos` → `popover-password/length/lowercase/
  uppercase/digits/symbols`, `filtro-grupo/tipo/ambiente` → `filter-group/kind/
  environment`, `config-pendrive` → `config-usb-drive`, `opcao-nivel-` →
  `level-option-`, sufixos `-ajuda`/`-rotulo`/`-opcoes`/`-opcao-`/`-explicacao` →
  `-help`/`-label`/`-options`/`-option-`/`-explanation`. Nenhum é lido fora do
  componente (conferido em `app/src`, `dev/`, `extension/`).
- Atributos `data-*` dos componentes: `data-teste` → `data-test`, `data-forca` →
  `data-strength`, `data-etapa` → `data-step`, `data-nivel` → `data-level`,
  `data-anexo` → `data-attachment`, `data-tipo` → `data-type`, `data-valor` →
  `data-value`. Os valores vindos do fio (`'fraca'`, `'pin-errado'`, `'simbolos'`…)
  ficam.
- Ficaram de propósito: `'nivel-pin'` (id do campo PIN no diálogo de nível),
  `name="nivel-de-acesso"`/`'versao'` (nome de grupo de rádio), os ids de tela
  (`'credenciais'`, `'gerador'`, `'configuracoes'`, `'aprovacao'`: `&tela=` da URL de
  teste e o rótulo da janela), comandos Tauri, códigos de erro e campos do fio.

### Entradas novas de `dev/english-check/frozen.txt` (todas Fase 54)

- `app/src/App.svelte`: `estado`, `pedido`, `restante`, `entrada`, `operacao`, `comando`,
  `mensagem`.
- `app/src/lib/components/GeneratorPopover.svelte`: `tamanho`, `minusculas`,
  `maiusculas`, `digitos`, `simbolos`, `forca`, `valor`, `campo`, `mensagem`.
- `app/src/lib/components/LevelDialog.svelte`: `entrada`, `nivel`, `nome`, `tipo`,
  `ambiente`, `codigo`, `mensagem`, `estado`.

## Segurança

- Invariantes: S2/S11/S12 (valor revelado só no `::before` por `data-value`, cópia e
  seleção canceladas, máscara sem tamanho real) e S9 (PIN só no estado local do
  `LevelDialog`, limpo antes da resposta) mantidas: só nomes e comentários mudaram; a
  lógica de `SecretField`, `PasswordField`, `PinField`, `LevelDialog` e
  `GeneratorPopover` é a mesma linha a linha. A ponte segue recusando comando fora de
  `COMANDOS`; nenhum segredo entra em estado global, armazenamento ou URL.
- S11 do pacote: `app-dist.py` e `app-release.sh` agora procuram `test-bridge` e
  `Showcase` além das marcas antigas (o pacote de teste tem de ter o `test-bridge`).
- Testes de ataque: nenhum novo (fase de nomes); `revealed_selection.py` (ataque à
  seleção do valor revelado) acompanha os nomes novos.
- Dependências novas: nenhuma.

## Ficou de fora

- Identificadores e comentários das telas (`screens/*`), da ponte (`lib/bridge/*`, exceto
  os 3 exports) e da `Showcase`: Fases 36–38 (o plano os põe lá). Os usos de componentes
  nelas já estão com os nomes novos.
- Nomes de comandos Tauri, campos de resposta e chaves de mensagem: congelados até a
  Fase 54 (`frozen.txt`).
- `extension/popup.js`/`popup.css` além de tokens e ícones: Fase 39.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `python3 app/scripts/generate_design.py` (tokens e ícones regenerados) | 0 |
| conferência V3.6 (tokens antigos com nomes mapeados = novos) | 0 |
| `python3 dev/english_check.py --literals` antigo × novo (só nomes mudaram) | 0 |
| `make english-check PATHS="app/src/lib/components app/src/lib/style app/src/lib/bidi.js app/src/main.js app/src/App.svelte app/scripts app/design"` | 0 (0 achados) |
| `make compile` (cargo check do workspace e do `app/src-tauri`, `npm ci`, `vite build`, `syntax_check.py`: 0 erro) | 0 |
| `npm run build:teste` (o `dist-teste` tem `test-bridge-*.js` e `Showcase-*.js`) | 0 |
| `python3 dev/tests/app-dist.py` (conferência rápida das marcas novas; não é a suíte) | 0 |
| `node dev/tests/bidi.mjs` (conferência rápida de `bidiParts`) | 0 |

O `vite build` mostra 10 avisos `state_referenced_locally` em
`screens/Formulario.svelte` (linhas 70–81), que já existiam antes desta fase.

## Problemas descobertos

- O glossário dizia que os comentários citam o nome do node do FigCraft como está lá; o
  checador acusa esses nomes (pt-BR). Os comentários citam o id e uma descrição em
  inglês; a regra foi registrada no glossário. O orchestrator decide se fica assim.
- `crates/ward-test-bridge/src/http.rs:392` cita `ponte-teste.js` › `ouvirAvisos` e
  `app/src-tauri/src/main.rs` (comentário perto da linha 862) cita
  `--primitiva-neutro-950`: são da trilha R; não mexi para não conflitar.
- Risco de conflito no merge com a trilha R em `app/src-tauri/src/main.rs` (linhas do
  `include_image!`), `app/core/tests/configuracao_tauri.rs` (caminhos) e
  `dev/english-check/frozen.txt` (entradas acrescentadas no fim).
- `revealed_selection.py` e as suítes de app/extensão não rodaram (regra da missão);
  ficam para o Milestone 6.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/src/telas/*` → `app/src/screens/*` (20 arquivos) | renomeado e alterado (usos) |
| `app/src/lib/componentes/*` → `app/src/lib/components/*` (36 `.svelte`, `icons.js`, `types.js`, `levels.js`, `ward-symbol.svg`) | renomeado e reescrito |
| `app/src/lib/estilo/{tokens,base}.css` → `app/src/lib/style/` | renomeado e alterado |
| `app/src/lib/ponte/{index,comandos,ponte-teste}.js` → `app/src/lib/bridge/{index,commands,test-bridge}.js` | renomeado e alterado |
| `app/src/lib/teste/Vitrine.svelte` → `app/src/lib/test/Showcase.svelte` | renomeado e alterado |
| `app/src/App.svelte`, `app/src/main.js`, `app/src/lib/bidi.js` | alterado |
| `app/scripts/gerar_design.py` → `app/scripts/generate_design.py` | renomeado e reescrito |
| `app/design/icons/*.svg` (27) | renomeado |
| `app/src-tauri/icons/bandeja-*-32.png` → `tray-*-32.png` | renomeado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/core/tests/configuracao_tauri.rs` | alterado |
| `extension/popup.css`, `extension/popup.js` | alterado |
| `dev/extension/build.py` | alterado |
| `dev/tests/app-dist.py`, `dev/tests/app-release.sh`, `dev/tests/bidi.mjs`, `dev/tests/revealed_selection.py` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md`, `CLAUDE.md` | alterado |
| `docs/mission/handoffs/phase-35.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma lógica de segurança mudou; o teste
  existente de V5 acompanha os nomes)
- [x] (UI) Conferido contra o FigCraft: medidas e cores vêm dos mesmos tokens (V3.6 por
  script); nenhuma medida mudou
- [x] Ficou dentro do escopo da fase (exceções: 3 exports da ponte e 4 props de telas,
  para a base ficar limpa; ver acima)
