# Fase 4g — (DESIGN) Logo do Ward

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo no app. Antes,
  li a skill `figcraft` (com `reference/vetor.md`), o PRD, o plano, o contrato, o
  `design-nodes.md` e a referência `docs/design/referencias/logo-ward.png` (linha de cima).
  Fiz 3 `save_file` no mesmo caminho (depois do ícone, depois do Painel de marca e no fim).
  No fim, `dirty: false` e 184 variables.
- **Geometria do símbolo**: medi a referência pixel a pixel e redesenhei do zero, sem
  colar o PNG. O símbolo tem caixa 204×224 e espessura uniforme 34 no anel e na faixa do
  chevron. O topo sobe em telhado (inclinação 1/3), os lados são retos e a base fecha em
  curva até a ponta. O "W" é o chevron a 45° que sobe dentro do vazado. O vão (26) separa
  o losango, que é a ponta do escudo. Todos os cantos têm filete (raios de 4 a 14) e o
  vazado tem sentido invertido, então funciona com `nonzero` e com `evenodd`. O resultado
  é um único vetor com 3 contornos e 34 vértices, criado com `vector.create` a partir do
  path calculado por um script Python no scratchpad.
- **Variables novas** (code syntax WEB): `azul/600` (`VariableID:48`, #3550DC, primitiva
  oculta), `marca/gradiente-inicio` (→ `azul/400`), `marca/gradiente-fim` (→ `azul/600`),
  `marca/simbolo` (→ `primaria/padrao`) e `marca/sobre-cor` (→ `neutro/0`), ids
  `VariableID:49`–`52`.
- **`Logo / Símbolo`** (set `3289`): `Cor=Marca` `3287` e `Cor=Branco` `3290`. Quadrado
  64×64, o vetor ocupa a altura toda e tem restrição `scale`, então a instância escala
  limpa. O preenchimento vem de `marca/simbolo` / `marca/sobre-cor`.
- **`Logo / Ícone do app`** (`3292`): 512×512, squircle (raio 115, `cornerSmoothing` 0,6),
  gradiente linear em diagonal com as duas paradas ligadas a `marca/gradiente-*`, e a
  instância do símbolo Branco em 320×320, centrada.
- **`Logo / Horizontal`** (set `3300`): `Cor=Marca` `3295` e `Cor=Branco` `3301`. Símbolo
  de 40 (`controle/md`) + "ward" em minúsculas com `Título/Destaque` (Inter semibold 32), gap
  `espaco/8`. O nome fica num frame `palavra` com padding inferior `espaco/4`, que sobe o
  texto 2 px para a altura-x ficar no centro óptico do símbolo, como na referência.
- **Seção `Seção / Logo`** (`3286`, x 640, y 6674, 40 abaixo de `Seção / App`): título,
  descrição, os três componentes e a linha `tamanhos` com instâncias do símbolo em 16, 24
  e 32 (cor e branco). Tudo ligado a variables (fundo, raio, padding, gap, borda e texto).
- **Legibilidade**: conferi o símbolo em 16, 24 e 32 com export em 1x e ampliação por
  vizinho mais próximo. Em 16 continuam visíveis o escudo, o W e o losango, com o vão de
  cerca de 2 px. O ícone do app em 16 fica com o símbolo em cerca de 10 px, reconhecível
  mas mole (ver Problemas).
- **Painel de marca** (`717`): troquei o quadrado com cadeado (`719`) e o texto "Ward"
  (`723`), que apaguei, pela instância `Logo / Horizontal` Marca (`3322`). O frame `marca`
  (`718`) passou a `hug`. Conferi por screenshot da `Seção / Telas de acesso`.
- **Exports** em `docs/design/logo/` (29 arquivos): `ward-icone-app.svg` + PNG 16, 24,
  32, 48, 64, 128, 256 e 512; `ward-simbolo.svg` + PNG nos mesmos 8 tamanhos;
  `ward-simbolo-branco.svg` + PNG 16, 32, 128 e 512; `ward-horizontal.svg`,
  `ward-horizontal-branco.svg` e os dois em PNG `@4x`. Os SVGs não têm `<text>` (o texto
  sai em contorno). Para conferir, reabri o `.figcraft` salvo pela CLI (`cli info`, `cli
  export --node 3292`) num diretório do scratchpad, e o ícone saiu igual.
- Ids registrados em `docs/mission/design-nodes.md` (nova seção "Logo (Fase 4g)", tabelas
  de frames, variables e Painel de marca).

## Segurança
- Invariantes tocadas: nenhuma S1–S15 (fase só de design). Nenhum dado do cofre, segredo
  ou credencial em texto: o logo e o Painel de marca têm só texto fixo.
- Testes de ataque: não se aplicam (sem código).
- Dependências novas: nenhuma. O script de geometria e o decodificador de PNG ficaram no
  scratchpad e usam só a biblioteca padrão do Python.

## Ficou de fora
- A marca da `Navegação lateral` (`782`: variantes `783`, `3037`, `3077`) ainda usa o
  quadrado com cadeado e "Ward". O plano pede só o Painel de marca. Ver Problemas.
- Proposta de ícone de bandeja e da extensão: são da Fase 9.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11); `gitleaks` não está instalado |
| `inspect_document`, `get_node`, `screenshot` (seção, ícone, horizontal em 6x, Telas de acesso) | ok |
| `variable.create`/`set`/`update` (5 variables), `variable.bind` | ok |
| `vector.create`, `component.combineVariants`, `component.addVariant`, `instance.create`, `instance.setProperties`, `node.reparent`, `edit.delete`, `style.apply` | ok |
| `export` (5 SVG, 24 PNG) para `docs/design/logo/` | ok |
| `FigCraft.AppImage cli run`/`render` (prévia headless no scratchpad), `cli info`, `cli export --node 3292` | 0 |
| `save_file` (3×, mesmo caminho) | ok |

## Problemas descobertos
- **Navegação lateral com o logo antigo**: as 3 variantes da `Navegação lateral` ainda
  mostram o cadeado num quadrado e "Ward" com maiúscula. Sugiro trocar por `Logo /
  Horizontal` (ou pelo símbolo de 32 + nome) numa fase seguinte (4e/4f), para a marca ficar
  igual nas telas. Não mexi porque o plano cita só o Painel de marca.
- **Contrato x plano**: o plano cita V0.17 para a 4g, mas no contrato o V0.17 é sobre a
  `Bandeja`. Só o V0.23 (aprovação do usuário, "incluindo o ícone") trata do logo. Falta
  uma asserção para o logo (componentes, legibilidade em 16, exports). Não decidi qual
  lado está certo.
- **Raio do squircle sem variable**: o `Logo / Ícone do app` usa raio 115 (22,5% de 512,
  padrão de ícone de app), que não existe na coleção `Raio`. Não criei um token só para
  isso. Também por isso, uma instância do ícone reduzida (ex.: 40 px) mantém o raio 115 e
  vira círculo. Nas telas, use o `Logo / Símbolo` ou o `Logo / Horizontal`. O ícone serve
  para os exports, que saem do mestre de 512.
- **Ícone do app em 16 px**: legível, mas mole. Para favicon, bandeja e extensão em 16,
  prefira `ward-simbolo-16.png` / `ward-simbolo-branco-16.png`, que são mais nítidos. Se
  a Fase 9 precisar do ícone com fundo em 16, vale uma versão simplificada (sem o vão do
  losango).
- **Cor do gradiente**: a referência é um índigo mais violeta. Mantive a paleta do Ward
  (`azul/400` → `azul/600` novo), para combinar com `primaria/padrao`. Se o usuário quiser
  mais violeta, basta mudar o valor de `azul/600` e `azul/400`. O V0.23 (aprovação) decide.
- Por engano, liguei a largura das variantes do símbolo a `controle/padrao` num batch e
  desfiz logo em seguida (desligado e 64×64 de volta). O estado final foi conferido por
  `get_node`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (Seção / Logo, 3 componentes, 5 variables, Painel de marca) |
| `docs/design/logo/` (5 `.svg`, 24 `.png`) | criado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4g.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): sem `Makefile` ainda (Fase 11)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
