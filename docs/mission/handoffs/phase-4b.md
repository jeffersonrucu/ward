# Fase 4b — (DESIGN) Estilos e ícones no `Ward.figcraft`

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem `open_file` de outro arquivo.
  Ajustei o que existia, sem recriar nada. Fiz três `save_file` no mesmo caminho: depois
  das sombras, depois dos ícones e no fim.
- **Effect styles (D23, sombras suaves)**: `Sombra/Modal` (`8:21`) e `Sombra/Flutuante`
  (`8:22`) saíram de uma camada a 60% de alfa (`sombra/padrao`) e passaram a ter duas
  camadas suaves, contato e ambiente, com a cor das duas ligada a `sombra/suave`
  (`VariableID:25`, preto a 24%).
  - Flutuante: (0, 2, desfoque 6) + (0, 12, desfoque 32).
  - Modal: (0, 4, desfoque 12) + (0, 24, desfoque 64).
  - Offset x, offset y, desfoque e spread também ficaram ligados a variables, na
    coleção nova `Elevação` (`VariableCollectionId:2`, modo `Valor`, 9 variables
    `VariableID:26`–`34`, escopo `EFFECT_FLOAT`, code syntax `var(--elevacao-…)`).
    Assim a sombra não tem nenhuma medida solta, que era a pendência da Fase 2.
  - Os 9 nodes com sombra (menu do Select `440`, Toast `526` `534` `542`, Modal `612`
    `628`, Diálogo de nível `968` `1027` `1078`) usam o effect style e acompanharam a
    mudança. Conferi no `document.json` salvo.
- **Text styles (14)**: todos já tinham família, tamanho, altura de linha e espaçamento
  de letra ligados. Faltava o `fontStyle`, e agora cada um está ligado a `estilo/*`
  (`6:101`–`6:104`).
  - Hierarquia (D23): `Título/Destaque` passou para `tamanho/3xl` e `altura-linha/3xl`
    (32/40, as variables que a 4a criou para isso) e de Bold para Semi Bold.
  - Os demais mantêm os valores da Fase 2.
- **Ícones (27)**: os 24 da Fase 2 e `chave`, `gerador`, `engrenagem` da Fase 4 já
  estavam como componentes `Ícone/*` em `Componentes / Ícones` (`178`), com 20 px
  ligados a `icone/md`, traço `linha/icone` e cor `icone/padrao`. Mudanças:
  - Pontas (`strokeCap`) e cantos (`strokeJoin`) arredondados nos 54 vetores, para
    combinar com a direção arredondada.
  - `Ícone/tipo-generico` (`241`) foi redesenhado como etiqueta (vetores `2815`, `2817`,
    ligados a `icone/padrao` e `linha/icone`). O desenho antigo era uma chave igual à
    `Ícone/chave`. O `Badge tipo` genérico (`517`) acompanhou.
  - As descrições dos 27 agora seguem o mesmo padrão.
  - O frame `178` passou a usar o raio `raio/painel` (20).
- **Consequência corrigida**: com o Destaque em 32/40, o `Painel de marca` (`717`)
  passou a cortar a 3ª garantia. Ela já vinha cortada antes, porque o frame
  `garantias` (`727`) tinha altura fixa de 92 para 100 de conteúdo. Pus `mensagem`
  (`724`) e `garantias` (`727`) em `hug`, e a conferência por screenshot está ok.
- Atualizei `docs/mission/design-nodes.md`: tabela de estilos com as ligações, seção
  Ícones e coleção `Elevação`.
- Conferência em disco: descompactei uma cópia do `.figcraft` salvo no scratchpad. Os
  16 estilos estão com todas as ligações, a coleção `Elevação` existe, `724`/`727`
  estão em `hug` e `241` tem os filhos `Etiqueta` e `Furo`. O `inspect_document`
  mostrou `dirty: false` antes da última edição, e salvei de novo depois dela.

## Segurança
- Invariantes tocadas: nenhuma S1–S15, porque a fase é só de design. Não toquei em
  cofre, credencial, `~/.ssh` ou `~/.aws`, e nenhum arquivo tem segredo.
- O Figma `bzu9PxxFur2tU3vlNCHvsY` não foi usado.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
- Religar os componentes aos papéis de raio e às `largura/*` fica para as Fases 4c–4d.
- A amostra de tokens: o `Tokens / Cores` (`3`) não mostra `Raio`, `Largura` nem
  `Elevação`. Não é exigido e pode entrar na Fase 10.
- O `Painel de marca` em si (raio, revisão D23) fica para a fase dele (4e). Só corrigi
  o corte de texto que a mudança de estilo causou.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11) |
| `style.list`, `get_node 178/241/717`, `screenshot` | ok |
| `variable.createCollection` + batches `variable.create`/`variable.update` (Elevação) | ok |
| `style.update` + `style.bind` (effect e text styles) | ok |
| `edit.delete` + `vector.create` + `variable.bind` + `node.set` (ícones) | ok |
| `node.set` `724`/`727` `sizingV: hug` | ok |
| `save_file` (3×, mesmo caminho) | ok |
| Conferência do `.figcraft` salvo (unzip de uma cópia no scratchpad) | ok |

## Problemas descobertos
- **Coleção nova `Elevação`**: é a 7ª coleção. O plano diz "ligados às variables", mas
  não havia token para offset e desfoque de sombra. Usar as `espaco/*` (escopo `GAP`)
  misturaria papéis. O V0.1 diz "existem", não "só", então não reprova. O orchestrator
  decide se mantém.
- **`sombra/padrao` (`8:7`) ficou sem uso**: nenhum estilo ou node a usa mais. Mantive
  porque é da Fase 2. A Fase 10a pode podar.
- Os 2 effect styles saíram com `show_behind: false`. Antes estava `true`, que veio
  da importação. `false` é o padrão de sombra comum e não muda nada visível em fundo
  opaco.
- O `textCase` maiúsculo de `Texto/Selo` e `Texto/Selo grande` não tem variable,
  porque não existe tipo de variable para isso. É uma propriedade, não uma medida.
- O `Painel de marca` usava `mensagem`/`garantias` com altura fixa e clip, e o texto
  já estava cortado antes desta fase. As fases seguintes devem preferir `hug` em
  blocos de texto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (coleção Elevação, estilos, ícones, `724`/`727`) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-4b.md` | criado |

Ids do FigCraft desta fase:
- coleção `Elevação`: `VariableCollectionId:2`, variables `VariableID:26`–`34`;
- estilos: `8:8`–`8:22` e `12:19` (ligações em `design-nodes.md`);
- ícones: `178`–`257`, com vetores novos `2815` e `2817` em `241`.

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito; versão em disco conferida)
- [x] Ficou dentro do escopo da fase (exceção: o ajuste de `hug` no `Painel de marca`, para corrigir o corte causado pela mudança de estilo)
