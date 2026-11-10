# Fase 9 — (DESIGN) Bandeja e extensão

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo e sempre com
  `ids` explícitos. Antes, li a skill `figcraft`, o PRD, o plano, o contrato (V0.17–V0.19),
  o `design-nodes.md`, os handoffs 4d e 4g, a referência-alvo e a Inspirações `1:3` do
  Figma (só leitura, por screenshot).
- **Primeira tentativa interrompida**: a UI do app parou de responder ("timed out waiting
  for the UI") e parei sem insistir. Na retomada, o `inspect_document` respondeu. Conferi
  `17137` e o `Item de menu`: o id real era `17152` e estava vazio, porque as chamadas
  que deram timeout não tinham sido aplicadas. Salvei logo em seguida e continuei. Fiz 7
  `save_file` no mesmo caminho.
- **Proposta de ícone** a partir do logo da 4g (`Logo / Símbolo`, set `3289`):
  - `Ícone da bandeja` (set `17141`, base 32): símbolo monocromático. `destravado` `17138`
    é branco (`marca/sobre-cor`). `travado` `17142` é cinza (`marca/sobre-cor-inativo`
    → `neutro/400`). O PRD não pede um estado "pedido pendente", então não desenhei.
  - `Ícone da extensão` (set `17148`, base 32): squircle (raio `raio/sm`, suavização 0,6)
    com o símbolo branco em 24, que ocupa 75% do ícone (o ícone do app usa 62,5%) para
    ficar mais nítido em 16. `destravado` `17145` usa o gradiente `marca/gradiente-*`.
    `travado` `17149` tem fundo cinza (`marca/inativo` → `neutro/600`), para o
    `chrome.action.setIcon` mostrar o estado.
  - **Legibilidade**: exportei em 16, 24 e 32 (extensão também em 48 e 128) e montei uma
    folha ampliada por vizinho mais próximo sobre fundo escuro (#101114 e #202124) e claro
    (#F1F3F4). Em 16 continuam legíveis o escudo, o W e o losango, nos quatro ícones,
    sobre fundo escuro. O ícone da extensão também é legível sobre fundo claro. A linha
    `tamanhos` (`17244`) da seção mostra as instâncias em 16, 24 e 32.
- **Componentes novos** em `Seção / Bandeja e extensão` (`17137`, x 11000, y 0; mesmo
  padrão das outras seções: `fundo/sutil`, `raio/painel`, `espaco/32`, título com
  `Título/Seção` e descrição com `Texto/Pequeno`):
  - `Ícone/ward` `17194`: símbolo em 20 (`icone/md`), cor `icone/padrao`, para "Abrir o
    Ward" em menus e botões. Fiz este componente porque não é possível redimensionar uma
    instância trocada dentro de outra.
  - `Item de menu` (set `17157`): `padrão` `17152`, `hover` `17158`
    (`superficie/hover`), `desabilitado` `17163`. Altura `controle/md`, raio
    `raio/controle`, padding `espaco/8` × `espaco/12`. Propriedades `Rótulo` (texto) e
    `Ícone` (troca).
  - `Menu da bandeja` (set `17201`): `destravado` `17168` tem a linha de estado (ponto
    `sucesso/texto`, "Cofre destravado", "Trava sozinho após 4 h sem uso"), separador,
    "Abrir o Ward", "Travar agora", separador e "Sair". `travado` `17202` tem o ponto
    cinza, "Cofre travado", "Destrave com a senha mestra", "Abrir o Ward", "Destravar…" e
    "Sair". Fundo `superficie/elevada`, borda `borda/padrao`, `raio/cartao`,
    `Sombra/Flutuante` e largura `largura/menu` (272).
  - `Cabeçalho do popup` (set `17234`): `destravado` `17227` e `travado` `17235`. Leva o
    símbolo de 24, "ward" em `Título/Seção` e um selo de estado em minúsculas: pílula de
    20, `Texto/Selo` 11, tint translúcido (`sucesso/tint` ou `selo/neutro/fundo`). Tem
    borda de baixo `borda/sutil` e largura `largura/popup`.
- **Telas no Layout** (página `2796`, modo `Escuro` herdado da página; área x ≥ 7200, y
  7200–10399), só com instâncias dos componentes:
  - `Bandeja / Destravado` `17269` (480×300, x 7200, y 7200): barra do sistema `17270`
    (relógio e `Ícone da bandeja` 16 `17273` numa pílula de hover `17271`) e `Menu da
    bandeja` destravado `17276` ancorado embaixo do ícone.
  - `Bandeja / Travado` `17300` (x 7760, y 7200): igual, com o ícone `17304` e o menu
    `17307` em travado.
  - `Extensão / Sugestões do domínio` `17332` (360, x 7200, y 7600): cabeçalho `17333`.
    No corpo `17340`: o cartão `origem` `17341` ("Origem da aba" e
    `https://painel.stage.clientex.test` em mono), "2 entradas para esta origem" e a lista
    `17343` com dois `Item de lista` compactos: `clientex / stage / painel` em hover
    (`17348`) e `clientex / stage / admin` (`17365`), ambos tipo login e ambiente stage.
    Abaixo vem a nota "nada fica salvo no navegador". No rodapé `17385`, o `Botão`
    secundário "Abrir o Ward" `17386`.
  - `Extensão / Salvar login novo` `17414` (x 7640, y 7600): cabeçalho `17415`. Corpo
    `17422`: "Salvar este login no Ward?". Os `Campo` são Nome `17424`, Usuário `17437` e
    Senha `17450`; a senha aparece só como `••••`, sem revelar nem copiar, com a ajuda
    "Vai direto para o cofre; a extensão não mostra nem guarda". Os `Select` são Tipo
    `17463` (login), Grupo `17469` (clientex) e Ambiente `17475` (stage). O cartão
    "Nível inicial: com aprovação" é o `17483`. No rodapé `17484`: "Agora não" `17488` e
    "Salvar no Ward" `17492`.
  - `Extensão / Cofre travado` `17392` (x 8080, y 7600): cabeçalho travado `17393`, avatar
    com cadeado, "O cofre está travado", `Botão` primário "Abrir o Ward para destravar"
    `17404` e a nota "A extensão nunca pede a senha mestra".
- **Variables novas (5)**, com escopo e code syntax WEB:
  - `marca/sobre-cor-inativo` `VariableID:91` (→ `neutro/400`).
  - `marca/inativo` `VariableID:92` (→ `neutro/600`).
  - `largura/menu` `VariableID:93` (272).
  - `verde/400-16` `VariableID:94` (#4CC77E29, primitiva oculta).
  - `sucesso/tint` `VariableID:95` (→ `verde/400-16`).
- **Exports** em `docs/design/icones/` (16 arquivos):
  - `bandeja-{destravado,travado}-{16,24,32}.png` e `.svg`;
  - `extensao-{destravado,travado}-{16,32,48,128}.png` e `.svg`.
- Conferência do arquivo salvo: copiei o `.figcraft` para o scratchpad e rodei `cli info`.
  Os 5 frames de tela e a seção estão lá, e os nomes dos componentes e das variables novas
  aparecem no `document.json`.

## Segurança
- Nenhuma S1–S15 tocada em código (fase só de design). O desenho segue as regras:
  - **S1 e R26**: a extensão preenche só na origem da aba, e o popup mostra a origem
    conferida. A senha do "Salvar login novo" aparece só como máscara, sem revelar nem
    copiar.
  - **R21**: o popup "Cofre travado" não pede a senha mestra. Ele abre o app e diz isso
    na tela.
  - **R23**: há "Travar agora" na bandeja.
  - **R6 e V0.20**: as listas mostram só campos não secretos (nome, usuário, tipo,
    ambiente).
- Só dados falsos: `clientex`, `painel.stage.clientex.test`, `ana.souza@clientex.test`,
  `admin@clientex.test`. Não toquei em nenhum cofre, credencial, `~/.ssh` ou `~/.aws`. No
  Figma, só li a `1:3`.
- Testes de ataque: não se aplicam (o M0 não tem asserção SEG).
- Dependências novas: nenhuma. O script da folha de conferência
  (`scratchpad/scripts/folha.py`) usa só a biblioteca padrão do Python.

## Ficou de fora
- **Estado "pedido pendente" no ícone da bandeja**: o PRD e o contrato não pedem. A janela
  de aprovação já abre em primeiro plano.
- **Variante clara do ícone da bandeja** para painéis claros: o branco some sobre a barra
  clara (ver Problemas).
- **Instâncias do ícone da extensão em 16 na linha de tamanhos**: o raio da instância não
  escala. A conferência em 16 e 48 foi feita pelos exports.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| FigCraft `inspect_document`, `get_node`, `screenshot` | ok (3 timeouts na primeira tentativa) |
| FigCraft `create_node`, `instance.create`/`setProperties`, `component.combineVariants`/`addVariant`/`addProperty`, `variable.create`/`set`/`update`/`bind`, `style.apply`, `node.set`/`reparent`, `edit.duplicate` | ok |
| FigCraft `export` (16 arquivos para `docs/design/icones/`) | ok |
| `save_file` (7×, mesmo caminho) | ok |
| `FigCraft.AppImage cli info` na cópia salva + `unzip` e `grep` no `document.json` | 0 |
| `python3 -I folha.py` (folhas ampliadas em 16, 24 e 32) | 0 |

## Problemas descobertos
- **O menu da bandeja no Linux é nativo** (AppIndicator/libayatana, via Tauri): a
  aparência (raio, sombra, cores, ponto colorido) vem do tema do sistema. O que vale para
  a Fase 39 é a ordem, os rótulos, os separadores e o estado habilitado. A linha de estado
  vira um item desabilitado ("Cofre destravado"). Os ícones nos itens podem não aparecer.
- **O ícone branco da bandeja some em painéis claros**: o GNOME padrão tem barra escura,
  mas temas claros existem. Uma variante escura do ícone (ou um ícone simbólico
  `-symbolic.svg` que o tema recolore) fica para a Fase 39 ou para a aprovação (V0.23).
- **O travado da bandeja só usa cor** (branco → cinza). Num painel monocromático isso é
  sutil. Um cadeado pequeno sobreposto fica ilegível em 16. Fica para o usuário decidir na
  aprovação.
- **O popup "Salvar login novo" tem 892 de altura** e o Chrome limita popups a 600: a
  implementação (Fase 42) precisa rolar o corpo com cabeçalho e rodapé fixos. Tipo e
  Ambiente não cabem lado a lado, porque o `Select` tem largura mínima 160 e o corpo útil
  tem 328.
- **Os popups não têm cantos arredondados próprios**: no Linux, o Chrome desenha a moldura
  do popup, e um raio interno deixaria cantos brancos. Os frames da extensão têm raio 0;
  os da bandeja, `raio/cartao` (é só o recorte de apresentação).
- **A senha do "Salvar login novo" precisa ir direto da página ao daemon** (content script
  → `ward-nm` → `wardd`), sem passar pelo estado do popup nem pelo `chrome.storage`. No
  design ela aparece só como máscara. Isso é para as Fases 40–42.
- **O ícone trocado não herda a cor**: no `Botão` primário, o ícone trocado fica com a cor
  padrão (cinza), o mesmo problema já registrado na 4d. Por isso o "Abrir o Ward para
  destravar" ficou sem ícone.
- **A variante `travado` do `Ícone da extensão` (`17149`)** guarda ligações antigas
  (`fills/0/stops/*`) da cópia do gradiente. O preenchimento real é sólido (`marca/inativo`)
  e a cor está certa. Não achei um comando para desligar essas ligações; vale limpar na
  10a.
- **O `Menu da bandeja` travado** (`17202`) marca `characters` como override no texto da
  legenda, porque a edição foi feita na variante. A legenda está certa; isso aparece só no
  `get_node`.
- **"Cliente" → "Grupo"**: usei "Grupo" (regra do plano). O PRD R26 já fala em grupo.

## Ids para design-nodes

Seção e componentes (página Design System `2`):

| Nome | Id | Variantes / filhos |
| --- | --- | --- |
| `Seção / Bandeja e extensão` | `17137` | título `17242`, descrição `17243`, `tamanhos` `17244` (bandeja 16/24/32 `17245` `17248` `17251` destravado, `17254` `17257` `17260` travado; extensão 32 `17263` `17266`) |
| `Ícone da bandeja` (set) | `17141` | `Estado=destravado` `17138` (símbolo `17139`), `Estado=travado` `17142` (vetor `17144`) |
| `Ícone da extensão` (set) | `17148` | `Estado=destravado` `17145` (símbolo `17146`), `Estado=travado` `17149` |
| `Ícone/ward` | `17194` | símbolo `17195` (vetor `17196`) |
| `Item de menu` (set) | `17157` | `padrão` `17152` (ícone `17153`, rótulo `17156`), `hover` `17158`, `desabilitado` `17163`; props `Rótulo`, `Ícone` |
| `Menu da bandeja` (set) | `17201` | `Estado=destravado` `17168`, `Estado=travado` `17202` |
| `Cabeçalho do popup` (set) | `17234` | `Estado=destravado` `17227`, `Estado=travado` `17235` |

Variables: `marca/sobre-cor-inativo` `VariableID:91`, `marca/inativo` `VariableID:92`,
`largura/menu` `VariableID:93`, `verde/400-16` `VariableID:94`, `sucesso/tint`
`VariableID:95`.

Frames de tela (página Layout `2796`) — **lista para exportar**:

| Frame | Id | Posição | Tamanho |
| --- | --- | --- | --- |
| `Bandeja / Destravado` | `17269` | 7200, 7200 | 480×300 |
| `Bandeja / Travado` | `17300` | 7760, 7200 | 480×300 |
| `Extensão / Sugestões do domínio` | `17332` | 7200, 7600 | 360×494 |
| `Extensão / Salvar login novo` | `17414` | 7640, 7600 | 360×892 |
| `Extensão / Cofre travado` | `17392` | 8080, 7600 | 360×343 |

Exports de ícone: `docs/design/icones/` (lista em "Implementado").

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (seção, 6 componentes, 5 variables, 5 telas) |
| `docs/design/icones/` (12 `.png`, 4 `.svg`) | criado |
| `docs/mission/handoffs/phase-9.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): sem `Makefile` (Fase 11)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito; arquivo salvo conferido pela CLI)
- [x] Ficou dentro do escopo da fase (extras: `Ícone/ward` e 5 variables, com motivo acima)
