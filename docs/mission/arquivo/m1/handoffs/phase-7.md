# Fase 7 — (DESIGN) Janela de aprovação

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo. Antes, li a
  skill `figcraft`, PRD (R15–R17, D2, D19, D21, D24), plano, contrato (V0.16, V0.19, V0.20),
  `docs/escopo.md` (Janela de aprovação, Destravar e travar, Segurança), `design-nodes.md`,
  os handoffs 4a–4g e 5 e a referência-alvo (só estilo). Fiz `save_file` no mesmo caminho
  no meio e no fim.
- Trabalho em paralelo com as Fases 6 e 8: todo comando teve `ids` explícitos (nada pela
  seleção), nenhum componente existente foi alterado, os frames ficaram em x ≥ 7200 e
  y 4000–7199 no Layout e os componentes novos numa seção minha na Design System em
  x = 8000. Para reaproveitar o cartão do `Destravar / Pedido do agente` (`3450`), usei
  `edit.duplicate` em `3506`, `3504`, `3503` e `3496` e movi as cópias para a minha área
  na mesma hora. Conferi no arquivo salvo que os frames de origem (`3501`, `3502`, `3476`)
  ficaram com os filhos de antes.
- **Componentes novos** em `Seção / Aprovação` (`16319`, página Design System, x = 8000,
  y = 0), todos ligados às variables:
  - `Contagem` (set `16348`): `Estado=normal` `16335` (fundo `aviso/sutil`, relógio e texto
    em `aviso/texto`) e `Estado=urgente` `16343` (fundo `perigo/sutil`, relógio e texto em
    `perigo/texto`). Propriedade de texto `Tempo` (ligada a `16339` e `16347`). Pílula de
    20 de altura, padding 2/6/8, `Texto/Etiqueta`, em minúsculas, a mesma medida do pedido
    do agente.
  - `Caixa de código` (`16340`): caixa escura (`fundo/padrao`, borda `borda/sutil`,
    `raio/controle`, padding 16) com `Rótulo` (`Texto/Legenda`) e `Conteúdo` (`Mono/Pequeno`).
    Serve para o comando exato e para a cadeia de processos.
- **Cinco frames** na página Layout (`2796`, modo `Escuro` herdado da página). Cada um é a
  própria janela nativa em tamanho de modal: 560 de largura (`largura/dialogo-largo`) e
  altura `hug`, com `fundo/padrao`, borda `borda/sutil`, `raio/janela`, padding 24, gap 20
  e `Sombra/Modal`. Estrutura comum:
  1. Cabeçalho: `Logo / Símbolo` (32), "Pedido de aprovação" (`Título/Modal`) e "Claude
     Code pediu uma credencial", `Contagem` e o botão fechar (`Ícone/fechar`).
  2. Cartão `pedido`, com a entrada (avatar do tipo, nome `grupo / ambiente / tipo`,
     `Badge tipo`, `Badge ambiente` e o nível atual à direita), `Caixa de código` "Comando"
     com o comando exato e `Caixa de código` "Quem pediu" com `claude → bash → ward run`.
  3. `Campo PIN` (6 dígitos), com a ajuda "Para permitir, digite o PIN. Negar não pede
     PIN."
  4. Quatro `Botão` em grade 2×2, nos cinco frames (R16, R17): **Negar** (secundário,
     estado `foco`, que é o foco inicial), **Permitir uma vez** (primário), **Permitir por
     1 hora** e **Permitir sempre** (secundários).
  5. Nota: fechar a janela nega, o pedido expira em 2 min e é negado, e "Permitir sempre"
     deixa a entrada livre (para voltar atrás, muda-se o nível na entrada).

| Frame | Id | Estado |
| --- | --- | --- |
| `Aprovação / Normal` | `16701` | `clientex / stage / banco`, `ward run clientex-stage-banco -- psql -c "select count(*) from pedidos"`, contagem "1:42 restantes", PIN vazio |
| `Aprovação / prod` | `16810` | `clientex / prod / banco`. O ambiente aparece em destaque: faixa `destaque` (`17110`, tint `ambiente/prod/tint`, borda `ambiente/prod/borda`) com o `Badge ambiente` prod grande (sólido) e "Credencial de produção. As opções são as mesmas dos outros ambientes; confira o comando com atenção.", e o cartão com borda `ambiente/prod/borda`. Os mesmos quatro botões (R16) |
| `Aprovação / PIN errado` | `16885` | `Campo PIN` erro "PIN incorreto. Nada foi permitido. Cinco erros seguidos travam o cofre." (sem dizer quantas tentativas restam, como no diálogo de nível da 4f); contagem "1:20 restantes" |
| `Aprovação / PIN bloqueado` | `16960` | faixa `destaque` (`17125`, `perigo/translucido`, borda `perigo/borda`, `Ícone/cadeado`) "Cinco erros seguidos de PIN. O cofre foi travado e o pedido, negado."; `Campo PIN` bloqueado; contagem oculta; os três "Permitir" em `desabilitado` e "Negar" em foco; nota explicando que o Claude já recebeu "negado" |
| `Aprovação / Expirando` | `17035` | `Contagem` urgente "0:12 restantes"; faixa `destaque` (`17129`, `perigo/translucido`, `Ícone/relogio`) "O pedido expira em 12 s. Sem resposta, ele é negado e o Claude recebe “negado”."; PIN digitando |

- **Conferência:**
  - screenshot de cada frame a 1–1,5x. O texto não encosta na borda: cartão e nota com
    padding 20, faixas com 16 × 20 e caixas de código com 16. Os badges têm 20 de altura,
    em minúsculas e com tint;
  - auditoria no `.figcraft` salvo, com o script da Fase 5
    (`/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/f5/audit.py`),
    nos 5 frames e na `Seção / Aprovação`: **0** preenchimentos, bordas, espessuras,
    raios, gaps ou paddings soltos e **0** textos sem text style fora das instâncias;
  - os ids dos frames, do set e dos componentes existem no arquivo salvo, com os nomes
    certos.

## Segurança
- A fase é só de design e não toca código. O desenho segue o escopo:
  - **S1 e R6:** a janela não mostra nenhum valor secreto. Ela mostra a entrada, o
    ambiente, o comando exato e quem pediu. O PIN aparece só como `•` (V0.20).
  - **R17 e S10:** os quatro botões aparecem em todos os estados. Só os de permitir pedem
    o PIN. O foco inicial fica em Negar. Fechar nega, e sem resposta em 2 min o pedido
    expira (contagem e estado `Expirando`).
  - **R16:** `prod` tem os mesmos botões, inclusive "Permitir sempre". Só o destaque muda.
  - **S9 e R24:** o PIN errado não diz quantas tentativas restam. Depois de 5 erros, o
    cofre trava e o pedido é negado (`PIN bloqueado`).
- Exemplos só falsos (`clientex`, `pedidos`). Não toquei em cofre, credencial, `~/.ssh`,
  `~/.aws` nem no Figma.
- Testes de ataque: não se aplicam (o M0 não tem asserção (SEG)).
- Dependências novas: nenhuma.

## Ficou de fora
- **Nível combinado (D19), com as duas entradas na janela** (por exemplo, `banco` por
  túnel): o V0.16 não pede. O cartão `pedido` aceita uma segunda linha `entrada`, e isso
  pode ser feito na Fase 36 ou num ajuste da 10c.
- **"Aprovação indisponível" (D21):** o pedido é negado sem abrir a janela, então não há
  tela para desenhar.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `get_node`, `component.properties`, `screenshot` (1x a 1,5x) | ok |
| `node.create`, `edit.duplicate`, `node.reparent`/`reorder`, `component.create`/`combineVariants`/`addProperty`/`bindProperty`, `instance.create`/`setProperties`/`swap`, `variable.bind`, `style.apply`, `node.set` | ok |
| `python3 -I f5/audit.py` (5 frames e a seção) no `.figcraft` salvo | 0 pendências |
| `save_file` (3×, mesmo caminho) | ok |

## Problemas descobertos
- **Largura do mestre sem token:** a `Caixa de código` (`16340`) tem 480 de largura de
  apresentação, sem variable. Nas telas ela usa `fill`. É o mesmo caso dos mestres de
  `Campo` e `Select` registrado na 4f.
- **Faixa de destaque montada como frame:** ela aparece 3 vezes (prod, bloqueado,
  expirando), junto com a nota e o aviso das telas de destravar. Pode virar componente
  (`Destaque`, com Tipo = prod | perigo | aviso | info) na 10c, junto com as bordas por
  estado.
- **Ícones coloridos por override:** o relógio da `Contagem` e os ícones das faixas
  receberam a cor por override nos vetores (`perigo/texto`, `aviso/texto`). É a mesma
  limitação da 10c ("a troca de ícone por propriedade não leva a cor").
- **Mono sem fonte na máquina:** o comando em `Mono/Pequeno` aparece com a fonte de
  fallback no screenshot, como já registrado na 4e.
- **"Negar" no PIN bloqueado:** depois de 5 erros, o pedido já está negado, e o botão só
  fecha a janela. Mantive o rótulo "Negar" porque o contrato pede os quatro botões em
  todos os estados. A Fase 36 pode trocar por "Fechar", se o usuário preferir.

## Ids para design-nodes
- Seção `Seção / Aprovação` `16319` na Design System (x = 8000, y = 0): título `16326`,
  descrição `16327`, linha `componentes` `16328`.
- `Contagem` set `16348`: `Estado=normal` `16335` (ícone `16336`, tempo `16339`) e
  `Estado=urgente` `16343` (ícone `16344`, tempo `16347`). Propriedade `Tempo`.
- `Caixa de código` `16340`: rótulo `16341`, conteúdo `16342`. Propriedades `Rótulo` e
  `Conteúdo`.
- Frames no Layout (`2796`), linha y = 4000:

| Frame | Id | x | Cabeçalho | `Contagem` | Cartão `pedido` | Comando | Quem pediu | `Campo PIN` | Botões (Negar, uma vez, 1 hora, sempre) | Nota | Destaque |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `Aprovação / Normal` | `16701` | 7200 | `16702` | `16709` | `16704` | `16751` | `16754` | `16715` | `16757` `16761` `16765` `16769` | `16742` | — |
| `Aprovação / prod` | `16810` | 7840 | `16811` | `16817` | `16826` | `16845` | `16848` | `16851` | `16863` `16867` `16872` `16876` | `16880` | `17110` |
| `Aprovação / PIN errado` | `16885` | 8480 | `16886` | `16892` | `16901` | `16920` | `16923` | `16926` | `16938` `16942` `16947` `16951` | `16955` | — |
| `Aprovação / PIN bloqueado` | `16960` | 9120 | `16961` | `16967` (oculta) | `16976` | `16995` | `16998` | `17001` | `17013` `17017` `17022` `17026` | `17030` | `17125` |
| `Aprovação / Expirando` | `17035` | 9760 | `17036` | `17042` | `17051` | `17070` | `17073` | `17076` | `17088` `17092` `17097` `17101` | `17105` | `17129` |

(Nos frames duplicados, os ids seguem o mesmo deslocamento a partir da raiz: cabeçalho
+1, `Contagem` +7, `pedido` +16, comando +35, quem pediu +38, PIN +41, botões +53/+57/+62/+66,
nota +70. Todos os ids da tabela foram conferidos por nome no `.figcraft` salvo.)

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (`Seção / Aprovação` com 2 componentes, 5 frames `Aprovação / *`) |
| `docs/mission/handoffs/phase-7.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) no M0)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase (só a seção nova na Design System; nenhum componente compartilhado alterado)
