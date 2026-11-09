# Fase 4c — (DESIGN) Componentes base, parte 1, arredondados

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem `open_file` de outro arquivo.
  Antes de começar, li a skill `~/.claude/skills/figcraft/SKILL.md`. Ajustei os componentes
  que já existiam, sem recriar nenhum, e fiz 5 `save_file` no mesmo caminho. No fim,
  `dirty: false`.
- **Propriedades religadas.** A importação do Figma tinha perdido a ligação entre as
  propriedades da Fase 2 e as camadas: os sets declaravam `Rótulo`, `Valor` etc., mas
  nenhuma camada usava essas propriedades, e a instância não trocava o texto. Religuei
  todas pelo `component.bindProperty` (92 ligações nas 6 seções). Conferi com instâncias
  de teste (`Rótulo`, `Mostrar ícone`, troca de `Ícone`, `Seis dígitos` = falso,
  `Valor revelado`, `Mostrar copiar`) e apaguei as instâncias depois.
- **Text styles aplicados.** Os textos dos componentes tinham fonte solta, sem text style.
  Agora usam: `Texto/Corpo forte` no rótulo do botão; `Texto/Rótulo`, `Texto/Corpo` e
  `Texto/Legenda` no rótulo, valor e ajuda dos campos; `Mono/Corpo` no valor revelado;
  `Mono/PIN` na máscara do PIN; `Título/Seção` e `Texto/Pequeno` nas seções.
- **Raios e larguras.**
  - Raios ligados aos papéis: controles em `raio/controle`, menu do Select em
    `raio/controle`, seções em `raio/painel`, sets em `raio/cartao`.
  - Botões internos pequenos (ação revelar/gerar, opção do menu, `Ação de campo`) ficaram
    em `raio/sm`, para ficarem concêntricos com o controle (10 − 4 = 6).
  - `Campo` e `Campo secreto` usam `largura/campo`, e o `Select` usa `largura/select`.
- **Respiro.**
  - Seções com padding `espaco/32` e altura em `hug`. Antes, o `Seção / Botão` tinha altura
    fixa e o set encostava na borda de baixo.
  - Sets com auto layout, padding 32 e gap 24.
  - Gap rótulo → campo → ajuda passou de 6 para `espaco/8`.
  - Botão com padding horizontal `espaco/20` e variantes em `hug` (antes eram fixas em 74).
  - Entradas de campo com altura `controle/lg` (48) e padding 16 à esquerda.
  - Opções do menu do Select com altura `controle/md` (texto 10/10). O menu está em `hug`
    e antes cortava a opção "prod".
  - Dígitos do PIN em 48×48 com gap `espaco/12`.
  - Conferi cada set por screenshot com zoom de 1,5–4×. Nenhum texto encosta na borda.
- **Referência-alvo `docs/design/referencias/alvo-layout-app.png`** (pedido do
  coordinator), usada só como estilo, sem marca, textos ou ícones dela:
  - Campos em caixa escura arredondada (`superficie/campo` sobre a seção), com borda
    sutil `borda/sutil` nos estados de repouso e rótulo acima.
  - Ações embutidas à direita: componente novo **`Ação de campo`** (`2882`), um botão
    pequeno com ícone + "Copiar", fundo `superficie/elevada` e borda `borda/padrao`, e
    a variante `copiado` em verde.
    - `Campo secreto`: revelar (ícone), gerar (ícone `Ícone/gerador`, opcional pela
      propriedade nova `Mostrar gerar`, padrão falso) e Copiar (`Ação de campo`). O
      estado copiado usa a variante `copiado`.
    - `Campo`: `Ação de campo` embutida e opcional pela propriedade nova `Mostrar copiar`
      (padrão falso), nos estados padrão, foco, erro e desabilitado.
  - Botões:
    - Primário em azul com brilho suave: effect style novo `Sombra/Brilho primário`
      (`S:17`), com cor e medidas ligadas a variables. As instâncias de botão primário no
      Modal e no Diálogo de nível já herdaram o brilho.
    - Secundário com fundo sutil e borda `borda/padrao`.
    - Perigo com fundo vermelho translúcido (`perigo/translucido`, hover
      `perigo/translucido-hover`), borda `perigo/borda` e texto `perigo/texto`.
    - Cor do ícone por tipo (`icone/sobre-cor`, `perigo/texto`, `icone/forte`,
      `icone/desabilitado`). Antes, o ícone ficava cinza sobre o azul.
- **Variables novas (9)**, todas com code syntax WEB e escopo:
  - `Primitivas`: `azul/500-40`, `vermelho/500-16`, `vermelho/500-24` (ocultas).
  - `Cores`: `primaria/brilho` (EFFECT_COLOR), `perigo/translucido`,
    `perigo/translucido-hover` (FILL), `perigo/borda` (STROKE).
  - `Elevação`: `elevacao/brilho/y` (4) e `elevacao/brilho/desfoque` (16).
  - No total, 175 variables em 7 coleções.
- Seção nova `Seção / Ação de campo` (`2965`). Reorganizei a coluna de seções com 40 de
  respiro, mexendo só na posição das seções das fases seguintes.
- **Auditoria no `.figcraft` salvo** (cópia descompactada no scratchpad), nas seções
  `260` `328` `372` `2965` `418` `647`: 0 preenchimento ou borda sólida sem variable,
  0 espessura de borda solta, 0 raio, padding ou gap soltos, 0 texto sem text style,
  e cor dos vetores sobrescritos nas instâncias ligada.
- Atualizei `docs/mission/design-nodes.md`: tabela da Fase 4c, ids, variables novas e
  `S:17`.

Ids do FigCraft:
- sets: `Botão` `263`, `Campo` `331`, `Campo secreto` `375`, `Select` `421`,
  `Campo PIN` `650`, `Ação de campo` `2882` (padrão `2877`, copiado `2883`);
- `ação gerar`: `2902` `2906` `2911`;
- instâncias de `Ação de campo` no `Campo secreto`: `2888` `2893` `2898`;
- instâncias de `Ação de campo` no `Campo`: `2932` `2937` `2942` `2947`;
- seção `2965`;
- variables `VariableID:35`–`43`, estilo `S:17`.

## Segurança
- Invariantes: nenhuma S1–S15 tocada em código, porque a fase é só de design. O desenho
  respeita S11/S12/D4:
  - O `Campo secreto` só mostra o valor no estado revelado, com o aviso "some em 30 s ou
    ao sair da tela".
  - O copiado avisa da limpeza da área de transferência em 30 s.
  - O PIN aparece só como `•`.
  - A ação gerar fica desligada por padrão, para não aparecer em telas de leitura onde
    trocaria o segredo sem querer.
- Exemplos só falsos (`falso-exemplo-9f3k`, `clientex`, `stage`). Os testes temporários
  usaram `falso-token-a1b2`, `deploy_stage` e `db.clientex.test`, e as instâncias foram
  apagadas. Não toquei em cofre, credencial, `~/.ssh` ou `~/.aws`. Não usei o Figma.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
- As seções da Fase 4d (Badge, Toast, Item de lista, Modal, navegação, nível) não foram
  editadas, só movidas. Pelo inventário, elas também têm propriedades sem ligação às
  camadas e textos sem text style (ver Problemas).
- Revisão dos `Botão` aninhados no Modal e no Diálogo de nível: eles herdaram o novo
  estilo, mas a conferência visual é da 4d.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11) |
| `component.properties`, `get_node`, `inspect_document`, `screenshot` (zoom por set) | ok |
| batches `variable.bind`, `style.apply`, `component.bindProperty`, `node.set` (Botão, Campo, Campo secreto, Select, Campo PIN) | ok |
| `variable.create`/`update`/`set` (9 variables), `style.create` + `style.bind` (`S:17`) | ok |
| `component.combineVariants`, `component.addVariant`, `component.addProperty` (`Ação de campo`, `Mostrar gerar`, `Mostrar copiar`) | ok |
| `save_file` (5×, mesmo caminho) | ok |
| Auditoria de ligações no `.figcraft` salvo (script Python no scratchpad) | ok, 0 pendências |

## Problemas descobertos
- **Propriedades de componente sem ligação às camadas em todo o arquivo importado.** Só
  3 camadas tinham `prop_refs` (`902`, `903`, `905`). Corrigi as 5 seções da 4c. A
  **Fase 4d** precisa religar as dela (`Item de lista`, `Modal`, `Toast` etc.), senão
  as telas das 4e/4f não conseguem trocar texto por propriedade.
- **Text styles não aplicados no arquivo importado.** Só 19 de 323 textos tinham text
  style antes desta fase, e o handoff da Fase 2 dizia que todos tinham. Corrigi os da
  4c. A 4d precisa aplicar os dela.
- **Respiro em controles.** A regra pede no mínimo 16 em cima e embaixo em contêineres.
  Nos controles de altura fixa centralizados, apliquei: entrada 48 (texto 20 → 14/14),
  botão 40 (10/10), opção de menu 40 (10/10) e `Ação de campo` 32 (texto 18 → 7/7).
  Isso segue a referência-alvo. Se o usuário quiser 16 também em controle, sobe
  `controle/*`.
- O `Botão` continua em `controle/md` (40) e os campos passaram a `controle/lg` (48),
  então numa linha com os dois as alturas diferem. A referência tem botões com mais ou
  menos a altura do campo. Fica para o usuário decidir se o botão sobe para 48 (ou um
  `controle/44` novo).
- A fonte JetBrains Mono não está instalada no sistema (`fc-list`). O FigCraft grava
  `Mono/*` certo, mas desenha com a fonte de fallback. Na UI (Fase 29), a fonte precisa
  ir embutida no app.
- `Campo secreto` no estado oculto continua com a ajuda escondida (64 de altura, contra
  86 dos outros). Veio da Fase 2. Ao trocar de estado, a instância muda de altura.
- A cor da borda do `Ação de campo` copiado (`sucesso/texto`) é forte. Não existe
  `sucesso/borda` sutil. Fica como pendência de refinamento para a 10.
- O escopo segue sem citar `Campo PIN`. A divergência já está registrada nas Fases 2 e 4a.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (componentes base parte 1, `Ação de campo`, 9 variables, `S:17`, posição das seções) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4c.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (`save_file` feito; versão em disco auditada; screenshot com zoom em cada set)
- [x] Ficou dentro do escopo da fase (exceção: novo `Ação de campo` e variables de brilho/perigo, pedidos pela referência-alvo; seções da 4d só reposicionadas)
