# Fase 4f — (DESIGN) Credenciais e Nível de acesso no Layout do Ward.figcraft

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo. Antes, li a
  skill `figcraft`, PRD, plano, contrato, `design-nodes.md`, os handoffs 4, 4a–4e e 4g e a
  referência-alvo `docs/design/referencias/alvo-layout-app.png` (só como estilo). Fiz 14
  `save_file` no mesmo caminho. No fim, `dirty: false` e 205 variables.
- **Antes das telas: campos que esticam.** Nas 12 variantes de `Campo`, `Campo secreto` e
  `Select`, desliguei a largura das variables (`largura/campo`, `largura/select`) e liguei a
  **largura mínima**: `largura/campo` (agora 240, era 320) e `largura/select-filtro` (agora
  160, era 200). Altura em `hug`. Os filhos `entrada`, `ajuda` e o menu do Select passaram a
  `fill`. Na tela, basta `sizingH: fill`, e a instância pode ficar mais larga ou mais
  estreita (até o mínimo). De quebra, o `Campo` e o `Campo secreto` tinham altura fixa
  (86/64) menor que o conteúdo (94), com a ajuda vazando. Agora estão em `hug`. Nas telas da
  4e, as 5 instâncias de `Campo` deixaram de usar `largura/coluna-acesso` e agora preenchem a
  coluna (que continua com 440).
- **Credenciais em três colunas** (navegação, lista, detalhe), como na referência-alvo:
  janela escura com painéis arredondados separados por 16. No topo ficam o título, o resumo,
  a busca em pílula e o botão `Filtrar`. A lista tem cabeçalho de grupo com marcador
  colorido, itens compactos com avatar do tipo e o botão primário `Nova credencial` no pé. O
  detalhe tem cabeçalho (avatar, nome, `Badge tipo`, `Badge ambiente`, grupo e motor,
  `Editar`), campos não secretos em caixas escuras com `Copiar` embutido (host, usuário),
  porta e base, a senha **só** no `Campo secreto` oculto (`••••`), o cartão do nível de
  acesso com `Mudar nível` e o rodapé com metadados, `Duplicar` e `Excluir`.
- **Componentes alterados ou novos** (ids em `design-nodes.md`):
  - `Item de lista`: propriedade de variante nova `Densidade` (larga | compacta). A compacta
    (`4002`, `4028`, `4045`, 76 de altura, padding 16) tem o nome e o `Badge ambiente` na
    linha 1 e o `Badge tipo` e o detalhe na linha 2. Assim cada item da lista estreita ainda
    mostra os dois badges (V0.7).
  - `Cabeçalho de grupo`: marcador de cor (`grupo/1..5/cor`), propriedade `Cliente` → `Grupo`.
  - `Diálogo de filtros` (novo, `5222`, em `Seção / Filtros` `5219`): Grupo, Tipo e
    Ambiente (`Select`), com `Limpar filtros`, `Cancelar` e `Aplicar`.
  - `Ícone/filtro` (novo, `5197`). `Chip de filtro` virou set (`5164`, só `Estado=ativo`),
    mas está sem uso nas telas.
  - Variables novas: `violeta/400`, `rosa/400`, `laranja/400` (primitivas ocultas),
    `grupo/1..5/cor` (`Cores`), `largura/coluna-lista` = 392 e `largura/busca` = 360.
- **Seções recolhíveis e grupos com cor na navegação (pendência da 4d), decisão final:**
  primeiro montei uma seção recolhível "Grupos" na `Navegação lateral`, com item de grupo
  em tint. O usuário mandou tirar: o grupo pertence às credenciais e deve ser só mais um
  filtro. Removi a seção, os componentes `Cabeçalho de seção` e `Item de grupo` e os tints.
  Com isso a navegação fica sem seções recolhíveis, porque o app só tem 3 telas (R25). A cor
  do grupo ficou só como marcador no cabeçalho do grupo na lista, com 5 cores atribuídas
  pela ordem e sem significado de ambiente ou estado.
- **Filtros (pedidos do usuário durante a fase):** chips em várias linhas → linha única
  com rolagem lateral → **diálogo**. Na versão final, um botão `Filtrar` ao lado da busca
  abre o `Diálogo de filtros`. Com filtro ativo, o botão vira `Filtros · 2` e a lista
  mostra uma linha de resumo ("Tipo banco · ambiente stage · busca pg") com `Limpar`.
- **"Cliente" virou "Grupo"** (decisão do usuário). Troquei:
  - `Campo`: rótulo padrão "Cliente" → "Grupo" nos 5 textos (`333` `341` `349` `357`
    `365`) e mensagem de erro "Informe o cliente." → "Informe o grupo." (`363`), mais as 8
    instâncias que guardavam esse texto (4e: `3372` `3484` `3679` `3712`; 4f: `4337`
    `4350` `4363` `4376`);
  - `Cabeçalho de grupo`: propriedade `Cliente` → `Grupo`, camada `cliente` → `grupo`;
  - descrições do `Item de lista` (`553`, "formato cliente / ambiente / nome") e do
    `Cabeçalho de grupo`;
  - textos das telas: "20 entradas · 4 grupos", "grupo acme · postgres", "separadas por
    grupo", "tipo, grupo e ambiente".
  
  Os nomes de exemplo (`acme`, `beta`, `gama`, `delta`, `clientex`) continuam, agora como
  nomes de grupo. Fiz a varredura no `.figcraft` salvo e não sobrou "Cliente/cliente" em
  rótulo, propriedade, descrição ou texto.
- **Selos em minúsculas** (pedido do usuário): `Texto/Selo` e `Texto/Selo grande` não têm
  mais text case maiúsculo, e a letra passou de 0,4 (`caixa-alta`) a 0
  (`espacamento-letra/normal`), com família, estilo, tamanho e altura religados.
  `Texto/Etiqueta` já estava em `original`. Varredura no arquivo: nenhum texto com case
  maiúsculo. Só o rótulo "PIN" é sigla. Conferi por screenshot o `Badge ambiente` (normal
  e grande), o `Badge tipo`, a lista e o diálogo de nível.
- **Sete frames de 1280×800** no Layout, só com instâncias para nav, botões, campos,
  selects, badges, itens, cabeçalhos, diálogos e ícones:

| Frame | Id | Conteúdo |
| --- | --- | --- |
| `Credenciais / Vazia` | `4780` | busca e `Filtrar` desabilitados; lista "As entradas aparecem aqui, separadas por grupo."; detalhe "Nenhuma credencial ainda" com a nota de que entradas novas começam com aprovação; `Nova credencial` |
| `Credenciais / Cheia` | `4063` | "20 entradas · 4 grupos"; grupo `acme` (12) com 7 itens visíveis (lista rola); `acme-pg-dev` selecionado e aberto no detalhe (host, porta, base, usuário, senha mascarada, nível livre) |
| `Credenciais / Filtrada` | `4410` | busca "pg", `Filtros · 2`, resumo dos filtros + `Limpar`; "2 de 12 entradas" (`acme-pg-aprovacao`, `acme-pg-tunel`); detalhe com nível com aprovação |
| `Credenciais / Filtros` | `5259` | a Cheia sob sobreposição com o `Diálogo de filtros` (banco, stage) |
| `Nível de acesso / Escolher` | `5615` | `acme-pg-prod` selecionado; `Diálogo de nível` Escolher (três níveis com explicação, aviso de que o ambiente não muda as regras) |
| `Nível de acesso / PIN` | `5994` | `Diálogo de nível` PIN |
| `Nível de acesso / PIN errado` | `6373` | `Diálogo de nível` PIN errado |

- **Conferência:** screenshot de cada frame em 1x, mais a lista e as seções em 1,5–2x. O
  texto não encosta na borda: itens com 16, cartões com 20, painéis com 16–24, badges com
  20 de altura e tint. Rodei também uma **auditoria no `.figcraft` salvo**
  (`scripts/audit4f.py` no scratchpad) nos 7 frames novos, nos 6 da 4e e em
  `Seção / Filtros`. Resultado: 0 preenchimentos, bordas, espessuras, raios, gaps ou
  paddings soltos e 0 texto sem text style fora das instâncias.

## Segurança
- Invariantes: a fase é só de design e não toca código. O desenho segue S1, S11 e R6:
  - o detalhe e a lista mostram só campos não secretos. A senha aparece só no
    `Campo secreto` oculto, como `••••`, e o valor revelado nunca entra numa tela da fase
    (V0.7, V0.20);
  - revelar e copiar ficam no app (S1). A ajuda diz que eles não pedem PIN. O `Copiar` de
    host e usuário vale só para campos não secretos;
  - mudar o nível passa pelo diálogo com PIN (S9, R15). O PIN errado diz "Nada foi
    alterado" e não mostra quantas tentativas restam. O mesmo diálogo vale para `prod`
    (R16);
  - os filtros não mudam o que o agente vê: são só da interface.
- Exemplos só falsos, tirados do cofre de teste do contrato (`acme-pg-dev`,
  `127.0.0.1:55432`, `app_teste`, `deploy@127.0.0.1:2223`). Não toquei em cofre,
  credencial, `~/.ssh` ou `~/.aws`, e não usei o Figma.
- Testes de ataque: não se aplicam (o M0 não tem asserção (SEG)).
- Dependências novas: nenhuma. Os scripts de leitura do `.figcraft` usam só a biblioteca
  padrão do Python e ficaram no scratchpad.

## Ficou de fora
- Estado de "busca sem resultado": o contrato não pede (já registrado na Fase 4).
- Ícone do nível colorido por estado no cartão do detalhe: fica para a 10c, junto com o
  ícone ativo da navegação.
- Detalhe de outros tipos (servidor com `Conectar`, login com TOTP etc.): é dos
  formulários das Fases 5 e 6. O detalhe aqui mostra um `banco`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `inspect_document`, `get_node`, `screenshot` (1x, 1,5x, 2x) | ok |
| `instance.create`/`setProperties`/`swap`, `component.addProperty`/`addVariant`/`combineVariants`/`editProperty`/`bindProperty`, `variable.create`/`update`/`set`/`bind`/`delete`, `style.update`/`bind`/`apply`, `node.set`/`reparent`, `edit.duplicate`/`delete` | ok |
| `python3 -I scripts/audit4f.py` (14 frames + seção) no `.figcraft` salvo | 0 pendências |
| `save_file` (14×, mesmo caminho) | ok |

## Problemas descobertos
- **Larguras do mestre sem token:** com a largura desligada, os mestres de `Campo`,
  `Campo secreto` e `Select` ficam com 320 de largura de apresentação, sem variable (o
  mínimo, sim, está ligado). O `Item de lista` compacto tem 360, como a larga já tinha 720.
  Nas telas, tudo usa `fill`. Se o validador do V0.2 olhar os componentes, isso aparece.
- **Altura da sobreposição** (800) e das telas não têm token, como já estava registrado na
  Fase 4. A largura está ligada a `largura/janela`.
- **Tokens sem uso** para a 10a: `largura/select` (320) e
  `espacamento-letra/caixa-alta` (0,4, sem uso desde os selos em minúsculas).
- **`Chip de filtro`** ficou sem uso nas telas, porque os filtros foram para o diálogo. A
  10 decide se mantém.
- **Contradição com os documentos:** "Cliente" virou "Grupo" e os selos passaram a
  minúsculas por decisão do usuário. O escopo, o PRD e o contrato (V0.7 "agrupada por
  cliente", R7 `cliente` obrigatório, campos `cliente` nos tipos) ainda dizem "cliente".
  Não editei, e o coordinator vai ajustar à parte. A tabela de entradas do contrato (coluna
  cliente) continua a ser a fonte dos exemplos.
- **Mudanças de design no meio da fase**, todas a pedido do usuário: grupo saiu da
  navegação, filtros foram para o diálogo, "Cliente" virou "Grupo" e os selos ficaram em
  minúsculas. O frame `Credenciais / Filtros` é extra (o contrato não pede), porque o
  diálogo precisava de uma tela.
- **Cabeçalho de grupo** tem padding esquerdo `espaco/4`. Na lista compacta, o marcador
  fica 12 px à esquerda do avatar dos itens. O efeito é só visual, e alinhar seria mudar o
  componente.
- **Item compacto:** o nome do nível saiu do item. Ele aparece no detalhe e no diálogo. Se o
  usuário quiser o nível na lista estreita, falta espaço para um terceiro selo.
- JetBrains Mono continua sem estar instalada (fallback), como já registrado na 4c e na 4e.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (7 telas, campos que esticam, `Item de lista` compacto, `Diálogo de filtros`, `Ícone/filtro`, `Chip de filtro` em set, marcador no `Cabeçalho de grupo`, Cliente → Grupo, selos em minúsculas, 10 variables líquidas) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4f.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots com zoom; arquivo salvo auditado; `save_file` feito, `dirty: false`)
- [x] Ficou dentro do escopo da fase (exceções a pedido do usuário: grupo, filtros em diálogo, selos em minúsculas; mudanças de componente necessárias às três colunas registradas acima)
