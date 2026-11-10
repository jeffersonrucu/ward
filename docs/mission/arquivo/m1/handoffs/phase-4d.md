# Fase 4d — (DESIGN) Componentes base, parte 2, e de apoio, arredondados

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem `open_file` de outro arquivo.
  Antes, li a skill `~/.claude/skills/figcraft/SKILL.md`, os handoffs 2, 3, 4, 4a, 4b e 4c,
  o `design-nodes.md` e a referência-alvo `docs/design/referencias/alvo-layout-app.png`
  (usada só como estilo). Fiz 6 `save_file` no mesmo caminho. No fim, `dirty: false`.
- **Text styles aplicados** em todos os textos das seções da 4d (91 textos sem estilo):
  `Título/Seção`, `Texto/Corpo`, `Texto/Corpo forte`, `Texto/Pequeno`, `Texto/Rótulo`,
  `Texto/Legenda`; a marca da navegação passou a `Título/Modal`.
- **Propriedades religadas às camadas**: `Toast.Mensagem`; `Item de lista.Nome`,
  `Detalhe`, `Nível`, `Mostrar nível`; `Modal.Título`, `Texto`; `Item de navegação.Rótulo`,
  `Ícone`; `Diálogo de nível.Entrada`. `Cabeçalho de grupo` e `Chip de filtro` já
  estavam ligados. Testei com instâncias temporárias (troca de texto, ícone, variante,
  `Mostrar colunas`, `Mostrar nível`) e apaguei o frame de teste.
- **Selos em pílula** (`raio/pilula`, `hug`): `Badge ambiente` (padrão 4/12, grande 6/16)
  e `Badge tipo` (ícone 16, padding 4/8/12, gap 6). As 8 instâncias de badge que estavam
  com largura fixa passaram a `hug`.
- **`Item de lista` refeito**: avatar 40×40 com o ícone do tipo (como a referência),
  nome e detalhe, e colunas fixas para tipo, ambiente e nível (`largura/coluna-*`), para
  os badges alinharem em colunas (D23). Padding 16, raio `raio/item`. Propriedades novas
  `Ícone` (troca do ícone do avatar) e `Mostrar colunas` (esconde tipo e ambiente numa
  lista estreita, pensando nas três colunas da 4f). Hover e selecionado foram apagados e
  recriados a partir do padrão (`component.addVariant`); ids novos `2977` e `3001`. O texto
  "com aprovação", que antes era cortado, agora cabe.
- **`Toast`**: raio `raio/cartao`, padding 16, largura `largura/toast`, ícone na cor do tipo.
- **`Modal`**: raio `raio/janela`, largura `largura/dialogo`, `hug`, gap 20, botões em
  `hug` (antes tinham largura fixa e o rótulo encostava na borda), ícone do perigo vermelho.
- **`Opção de nível`** (queixa do usuário): as variantes tinham 80 fixo e o texto colava
  na borda de baixo. Agora estão em `hug` com padding 20 nos quatro lados, raio
  `raio/cartao` e explicação com altura automática. As instâncias do diálogo também.
- **`Diálogo de nível`**: largura `largura/dialogo-largo`, `hug`, raio `raio/janela`;
  cartão da entrada com padding 16 e raio `raio/cartao`; botões em `hug`.
- **`Navegação lateral`** no estilo da referência: painel arredondado (`raio/painel`,
  borda sutil nos 4 lados, largura `largura/barra-lateral` = 264), marca, itens e rodapé
  em cartão com "Cofre destravado" e "Travar agora" (com ícone de cadeado). As variantes
  Gerador de senha e Configurações foram recriadas a partir da Credenciais (ids novos
  `3037` e `3077`).
- **`Item de navegação`**: raio `raio/controle`, ativo com `superficie/elevada` + borda.
  **`Chip de filtro`**: pílula com borda `borda/foco`. **`Painel de marca`**: raio
  `raio/painel`, logo com cadeado branco. **`Indicador de passos`**: as variantes estavam
  com 10 de altura para 30 de conteúdo e cortavam; agora estão em `hug`.
- **Pendências da 4c**:
  - Altura padrão comum: variable nova `controle/padrao` = 44, ligada às 16 variantes do
    `Botão` e às entradas de `Campo`, `Campo secreto` e `Select`. Antes eram 40 e 48;
    a referência fica entre 40 e 44.
  - `Campo secreto`: a ajuda do estado oculto ficou visível, e os três estados têm 86.
  - As descrições das seções foram atualizadas.
- **Seções**: as da 4d seguem o padrão da 4c (raio `raio/painel`, padding 32, `hug`,
  sets com padding 32 e gap 24). A coluna foi reorganizada com 40 de respiro.
- **Variables novas (4)**, com escopo e code syntax WEB: `largura/coluna-tipo` 120
  (`VariableID:44`), `largura/coluna-ambiente` 72 (`45`), `largura/coluna-nivel` 128
  (`46`), `controle/padrao` 44 (`47`). Agora são 179 variables.
- **Auditoria no `.figcraft` salvo** (cópia descompactada no scratchpad), nas seções
  `459` `475` `522` `550` `608` `716` `760`: 0 pendências. Não há preenchimento, borda,
  espessura, raio, padding, gap ou cor de texto soltos, nenhum texto sem text style e
  nenhuma propriedade sem camada. Nas seções da 4c continuam 0 valores soltos. As 22
  "propriedades sem camada" que aparecem são props de estado, presentes só na variante
  que as usa (ex.: `Mensagem de erro` fora do erro), como já era na 4c.
- Conferi cada set por screenshot com zoom de 1,5 a 2×. Nenhum texto encosta na borda.

Ids do FigCraft:
- sets: `Badge ambiente` `462`, `Badge tipo` `478`, `Toast` `525`, `Item de lista` `553`
  (padrão `554`, hover `2977`, selecionado `3001`), `Modal` `611`, `Painel de marca`
  `717`, `Indicador de passos` `741`, `Item de navegação` `763`, `Navegação lateral` `782`
  (Credenciais `783`, Gerador de senha `3037`, Configurações `3077`), `Cabeçalho de grupo`
  `901`, `Chip de filtro` `904`, `Opção de nível` `909`, `Diálogo de nível` `967`;
- nós novos no `Item de lista` padrão: `avatar` `2971`, `coluna tipo` `2972`,
  `coluna ambiente` `2973`, ícone do tipo `2974`;
- apagados: `572`, `590`, `822`, `861`;
- variables `VariableID:44`–`47`.

A tabela completa está em `docs/mission/design-nodes.md`.

## Segurança
- Invariantes: nenhuma S1–S15 tocada em código, porque a fase é só de design. O desenho
  respeita R6/V0.7/V0.20: o `Item de lista` mostra só campos não secretos (nome, detalhe
  com host e usuário, tipo, ambiente, nível). O `Toast` não leva segredo. O PIN aparece
  só como `•`. O `Diálogo de nível` mantém "PIN não destrava o cofre" e "5 erros travam".
- Exemplos só falsos (`clientex / stage / banco`, `acme-pg-prod`, `db.clientex.test`).
  Os testes temporários usaram `FALSO_API_TOKEN`, `api.acme.test` e `beta-ssh-stage`, e
  as instâncias foram apagadas. Não toquei em cofre, credencial, `~/.ssh` ou `~/.aws`, e
  não usei o Figma.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
- **Seções recolhíveis e grupos com cor própria na navegação** (pastas com tint na
  referência): o Ward só tem 3 telas no app (R25), e os grupos por cliente vivem na
  lista. Cor por cliente pediria tokens novos e dados que são da tela. Fica para a 4f
  decidir, junto com as três colunas.
- **Textos de exemplo por variante**: as propriedades de texto valem para o set todo,
  então o `Toast` erro/aviso e o `Modal` perigo mostram o texto padrão ("Entrada salva.",
  "Salvar alterações?"). Nas telas, a instância troca o texto (testado).
- **Ícone do nível no `Item de lista`**: é sempre o cadeado no componente. Nas telas, a
  troca é feita na instância aninhada.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11) |
| `inspect_document`, `get_node`, `component.properties`, `screenshot` (zoom por set) | ok |
| batches `style.apply`, `variable.bind`, `component.bindProperty`, `node.set`, `node.reparent` | ok |
| `variable.create`/`update` (4 variables) | ok |
| `edit.delete` + `component.addVariant` (Item de lista, Navegação lateral), `component.addProperty` (`Ícone`, `Mostrar colunas`) | ok |
| instâncias de teste + `instance.setProperties` (depois apagadas) | ok |
| `save_file` (6×, mesmo caminho) | ok |
| Auditoria de ligações no `.figcraft` salvo (`scripts/audit.py` no scratchpad) | ok, 0 pendências |

## Problemas descobertos
- **Ids trocados**: hover/selecionado do `Item de lista` (`572`/`590` → `2977`/`3001`) e
  duas variantes da `Navegação lateral` (`822`/`861` → `3037`/`3077`). Nenhuma instância
  usava esses ids, porque o Layout ainda está vazio. O `design-nodes.md` já foi
  atualizado. Handoffs antigos que citam os ids velhos não valem mais.
- **Override de cor em ícone trocado não segue a troca**: pintar o ícone dentro do
  componente principal só vale para o ícone padrão. Quando a propriedade `Ícone` troca
  o ícone, a cor volta a `icone/padrao`. Por isso o item ativo da navegação ficou com o
  ícone na cor padrão. Para destacar o ícone ativo, o caminho seria um ícone com variante
  de cor, que é decisão de design system para a Fase 10.
- **`controle/padrao` = 44 fora da escala** `controle/sm|md|lg` (32/40/48). Escolhi 44
  porque a referência fica entre 40 e 44 e assim o texto de 20 tem 12 em cima e embaixo.
  Os dígitos do `Campo PIN` continuam com 48. `controle/md` e `controle/lg` continuam
  em uso no `Item de navegação`, nas opções do Select e no PIN.
- **Bordas coloridas fortes** no `Toast` (`sucesso/texto` etc.) e no `Chip de filtro`
  (`borda/foco`). Não existe variante "borda sutil" dessas cores. Isso é refinamento
  para a Fase 10, que já tem a pendência de `sucesso/borda` aberta pela 4c.
- A `Seção / App` (`760`) tem bastante área vazia, porque o `Cabeçalho de grupo` tem 720
  de largura numa coluna própria. É só apresentação.
- O escopo segue sem citar `Campo PIN`, o que já foi registrado nas Fases 2, 4a e 4c.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (componentes da 4d, altura comum, 4 variables, posição das seções) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4d.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (`save_file` feito; versão em disco auditada; screenshot com zoom em cada set)
- [x] Ficou dentro do escopo da fase (exceções: `avatar` e `Mostrar colunas` no `Item de lista`, e rodapé em cartão na navegação, pedidos pela referência-alvo; 4 variables novas)
