# Fase 4e — (DESIGN) Telas de acesso (primeiro uso e destravar) no Layout do Ward.figcraft

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo. Antes, li a
  skill `figcraft`, o PRD, o plano, o contrato, o `design-nodes.md` e os handoffs 3, 4a–4d
  e 4g, além da referência-alvo `docs/design/referencias/alvo-layout-app.png` (só como
  estilo). Inspirações: o `.figcraft` não tem essa página, então li a `1:3` do Figma só
  por screenshot (nenhuma escrita). Fiz 9 `save_file` no mesmo caminho.
- **Antes das telas: marca da `Navegação lateral`.** Nas 3 variantes (`783`, `3037`,
  `3077`), o quadrado com cadeado e o texto "Ward" foram trocados pela instância
  `Logo / Horizontal` Marca (`3327`, `3332`, `3337`). O frame `marca` passou a `hug`.
  Conferi por screenshot.
- **`Painel de marca`** (`717`): a borda agora cobre os 4 lados (antes só a direita),
  com espessura ligada a `linha/fina`. Assim ele fica um painel arredondado igual aos da
  referência, separado do painel de conteúdo por respiro.
- **Variable nova** `largura/coluna-acesso` = 440 (`VariableID:53`, escopo WIDTH_HEIGHT,
  WEB `var(--largura-coluna-acesso)`). É a largura da coluna do formulário e das
  instâncias de `Campo` nessas telas.
- **Badges compactos** (pedido do usuário durante a fase: "exageradamente grandes").
  Ajustei os componentes, não só as instâncias:
  - `Badge ambiente` (`462`): padrão com 20 de altura (era 26), padding 2 × 8, sem
    borda, fundo em tint translúcido (`ambiente/<amb>/tint`, 16% da cor 400) e texto
    `Texto/Selo`, agora 11/16 médio, ainda em caixa alta e com letra 0,4. O grande tem
    24 (era 30), padding 4 × 12 e `Texto/Selo grande` 12/16 semibold (era negrito 13). Dev
    e stage grandes usam tint, e o prod grande continua sólido (destaque da aprovação),
    sem borda.
  - `Badge tipo` (`478`): 20 de altura (era 28), sem borda, fundo `selo/neutro/fundo`
    (tint neutro 12%), padding 2 / 6 / 8, gap 4, ícone 12 (`icone/xs`) e texto no estilo
    novo `Texto/Etiqueta` (12/16 médio, era `Texto/Pequeno` 13).
  - Tokens novos: primitivas ocultas `ciano/400-16`, `ambar/400-16`, `vermelho/400-16` e
    `neutro/400-12` (`VariableID:54`–`57`); em `Cores`, `ambiente/dev|stage|prod/tint` e
    `selo/neutro/fundo` (`58`–`61`, com alias para as primitivas); `tamanho/2xs` = 11
    (`62`) e `icone/xs` = 12 (`63`). Text style novo `Texto/Etiqueta` (`S:18`), com
    todos os campos ligados. Tirei as ligações de borda das 15 variantes. Agora são
    195 variables.
  - A pílula de contagem do pedido do agente segue a mesma medida (padding 2/6/8,
    ícone 12, `Texto/Etiqueta`).
  - Conferi com zoom de 2x os sets, o `Item de lista` (as 3 variantes, colunas
    alinhadas), o cartão da entrada do `Diálogo de nível` (prod) e o pedido do agente.
    As 14 instâncias existentes (Item de lista, Diálogo de nível, tela) seguiram o
    componente sem override.
- **Seis frames de 1280×800 na página Layout** (`2796`, modo `Escuro` fixado na
  página). Todos usam a mesma estrutura: janela escura com cantos `raio/janela` e borda
  sutil, dois painéis arredondados separados por 16 (instância do `Painel de marca` +
  painel `conteúdo`) e a coluna do formulário centrada. Textos em português e exemplos
  falsos.

| Frame | Id | Conteúdo |
| --- | --- | --- |
| `Primeiro uso / Criar cofre` | `3537` | Passo 1; "Criar seu cofre"; cartão com 3 itens (cifrado em repouso com Argon2id e XChaCha20-Poly1305; fica só nesta máquina, em `~/.local/share/ward/`; trava sozinho ao bloquear, suspender e após 4 h); `Botão` "Criar cofre" |
| `Primeiro uso / Senha mestra` | `3619` | Passo 2; `Campo` foco (senha mestra mascarada, "Mínimo de 12 caracteres."); barra de força (3 de 4, "Força: boa"); `Campo` de confirmação mascarado; aviso de que não há recuperação (kit de emergência); `Voltar` / `Continuar` |
| `Primeiro uso / PIN` | `3733` | Passo 3; "PIN de aprovação" (4 a 6 dígitos); `Campo PIN` completo + `Campo PIN` digitando ("Confirmar PIN"); destaque "O PIN não destrava o cofre… Cinco erros seguidos travam o cofre."; `Voltar` / `Concluir` |
| `Destravar / Padrão` | `3342` | selo com cadeado; "Cofre travado"; `Campo` vazio com cadeado à esquerda e ajuda "Só a senha mestra destrava. O PIN não serve aqui."; `Botão` "Destravar"; nota da trava automática |
| `Destravar / Senha errada` | `3393` | `Campo` erro "Senha mestra incorreta." (valor mascarado); `Botão` desabilitado "Tente de novo em 4 s"; nota "Cada tentativa errada aumenta a espera da próxima, até 30 s. O Ward não mostra quantas tentativas já houve." (D20) |
| `Destravar / Pedido do agente` | `3450` | cartão "Pedido do agente" com contagem em pílula "1:42 restantes", entrada `clientex / stage / banco` com avatar do tipo, `Badge tipo` banco e `Badge ambiente` stage, caixa escura com o comando exato `ward run clientex-stage-banco -- psql -c "select count(*) from pedidos"`; `Campo` foco; `Agora não` / `Destravar`; nota "Destravar não aprova o pedido… 'Agora não', fechar ou deixar expirar nega o pedido." |

- **Só instâncias** para todo botão, campo, PIN, badge, indicador, painel e ícone. Os
  frames que montei (cartões, notas, linhas) têm todo preenchimento, borda, espessura,
  raio, padding e gap ligados a variables, e todo texto tem text style e cor ligados.
- **Respiro**: cartões e notas com 2 ou mais linhas usam padding 20 (`espaco/20`). A
  caixa do comando usa 16. As pílulas seguem os badges (4/8/12). Os botões primários do
  rodapé mostram o brilho inteiro, porque tirei o `clip` dos frames `formulário` e
  `ações`. Usei espaço sem quebra em "4 h" e "30 s" para o número não se separar da
  unidade.
- **Conferência**: fiz screenshot de cada frame em 1x e dos formulários em 2x. Nenhum
  texto encosta na borda e nada se sobrepõe ou é cortado. Também comparei com a
  referência-alvo (painéis arredondados, superfícies em camadas, bordas sutis, caixas
  escuras) e com a Inspiração de login da `1:3` (duas colunas: marca e formulário).
  Antes de fechar, reauditei também as seções `Badge ambiente` (`459`), `Badge tipo`
  (`475`) e `Item de lista` (`550`), com 0 pendências.
- **Auditoria no `.figcraft` salvo** (descompactado no scratchpad, script
  `scripts/audit4e.py`): nos 6 frames há 0 preenchimento, borda, espessura, raio, gap ou
  padding soltos e 0 texto sem text style fora das instâncias. A navegação lateral salva
  tem a instância `Logo / Horizontal` (`3295`) nas 3 variantes.
- Ids registrados em `docs/mission/design-nodes.md` (seção nova "Telas de acesso no
  Layout (Fase 4e)", mais a `Navegação lateral`, o `Painel de marca`, a variable e a
  página Layout).

## Segurança
- Invariantes: a fase é só de design e não toca código de nenhuma S1–S15. O desenho
  segue o escopo:
  - A senha mestra e o PIN aparecem só mascarados (`•`). O campo da senha mestra é
    `Campo`, sem "copiar" (`Mostrar copiar` falso). Nenhum valor real aparece (V0.20).
  - R24: o PIN aparece explicitamente como algo que não destrava, e o destravar diz que
    o PIN não serve ali.
  - D20: espera crescente, sem dizer o número de tentativas.
  - R22/S10: o pedido do agente mostra a entrada e o comando exato, com contagem de
    2 min. Destravar não aprova o pedido, e negar, fechar ou expirar nega.
- Exemplos só falsos (`clientex / stage / banco`, `pedidos`). Não toquei em cofre,
  credencial, `~/.ssh` ou `~/.aws`. O Figma foi usado só para leitura (screenshot da
  `1:3`).
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. O script de auditoria usa só a biblioteca padrão do
  Python e ficou no scratchpad.

## Ficou de fora
- `Campo senha` (revelar sem copiar): as telas usam `Campo` com valor mascarado e
  cadeado à esquerda. A troca é da Fase 10b.
- Telas de credenciais e nível de acesso: são da Fase 4f.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11) |
| `inspect_document`, `get_node`, `component.properties`, `select.byName`, `screenshot` (1x e 2x) | ok |
| `instance.create`/`setProperties`/`swap`, `variable.create`/`update`/`bind`, `style.apply`, `node.set`/`reorder`/`reparent`, `edit.duplicate`/`delete` | ok |
| Figma `get_screenshot` `1:3` (só leitura) | ok |
| `python3 -I scripts/audit4e.py` no `.figcraft` salvo | 0 pendências nos 6 frames (exit 1 só na linha extra de inspeção, depois da auditoria) |
| `save_file` (9×, mesmo caminho) | ok |

## Problemas descobertos
- **`Campo` não preenche a largura**: a largura do componente está ligada a
  `largura/campo` (320), então `sizingH: fill` na instância não tem efeito. Nas telas
  liguei a largura da instância a `largura/coluna-acesso`. A 4f e as fases de formulário
  vão bater no mesmo problema. Talvez valha deixar o `Campo` com largura `fill` e
  mínimo `largura/campo`, mas essa decisão é do Design System (Fase 10).
- **Mono sem fonte**: o texto em `Mono/Pequeno` (comando do pedido) aparece no
  screenshot com fonte de fallback, sem cara de monoespaçada. Parece que a JetBrains
  Mono não está instalada na máquina. No app (Fase 31) a fonte vem empacotada.
- **Aviso sem borda própria**: a nota de aviso (`aviso/sutil`) e o destaque do PIN
  (`primaria/sutil`) ficaram sem borda, porque não existem `aviso/borda-sutil` nem
  `primaria/borda`. Isso cabe no refinamento de bordas por estado da Fase 10c.
- **Componente alterado fora das telas**: o `Painel de marca` passou a ter borda nos 4
  lados, e foi criada a variable `largura/coluna-acesso`. As duas mudanças são pequenas e
  necessárias para o layout em painéis da referência.
- O "Mínimo de 12 caracteres" e a barra de força continuam sem definição no PRD (já
  registrado na Fase 3).
- **Estilos de selo mudaram de tamanho**: `Texto/Selo` (12 → 11, semibold → médio) e
  `Texto/Selo grande` (13 → 12, negrito → semibold) valem para todo uso. Hoje só os
  badges usam esses estilos. As fases de UI devem pegar os valores novos.
- O `Badge ambiente` padrão em tint ficou com contraste menor que antes. O texto (cor
  400 sobre 16% da mesma cor, no fundo escuro) continua legível no screenshot, mas a
  Fase 10 deve medir o contraste AA.
- Nas telas de destravar, o selo com cadeado é um frame montado (com instância do
  ícone), não um componente. Se aparecer em mais telas (aprovação, 4f), vale virar
  componente na Fase 10.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (6 telas no Layout, logo na `Navegação lateral`, borda do `Painel de marca`, badges compactos, 11 variables, 1 text style novo e 2 revistos) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4e.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito; arquivo salvo auditado)
- [x] Ficou dentro do escopo da fase (exceções registradas: borda do `Painel de marca`, `largura/coluna-acesso` e badges compactos, estes a pedido do usuário)
