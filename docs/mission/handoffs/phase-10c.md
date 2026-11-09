# Fase 10c — (DESIGN) Refinamentos acumulados

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no `Ward.figcraft` já aberto, sem `open_file`. Usei ids explícitos em todos os comandos e fiz 14 `save_file` no mesmo caminho, nos pontos intermediários e no fim. Conferi cada mudança de componente por screenshot das telas afetadas e auditei o arquivo salvo lendo o `document.json`. Nenhum timeout de UI.
- Fiz a lista inteira, menos o `Chip de filtro` (ver "Ficou de fora"). Itens, na ordem de impacto:
  1. **Cor do ícone por estado, sem override** (resolve o ícone cinza no `Botão` primário, o item ativo da `Navegação lateral` e "ícones coloridos por override viram variantes"):
     - criei a coleção `Tom do ícone` (`VariableCollectionId:3`) com uma variable, `tom/icone` (`VariableID:96`). Ela tem 8 modos (padrão, sobre-cor, forte, desabilitado, marca, perigo, aviso, sucesso), que apontam para as variables de `Cores`;
     - todos os vetores dos 29 `Ícone/*` passaram a usar `tom/icone`. A cor vem do modo fixado no componente que contém o ícone, então trocar o ícone pela propriedade mantém a cor;
     - fixei o modo em `Botão` (16 variantes), `Item de navegação` (ativo = marca), `Ação de campo`, `Item de menu` desabilitado, `Cartão de tipo` selecionado, `Contagem` e `Destaque`;
     - acertei os 545 nós existentes por script e tirei os overrides de cor dos vetores do `Botão`, da `Ação de campo`, da `Contagem` e do `Gerador de senha / Avulso` (`13457`, `13791`).
  2. **`Destaque`** (set novo `18678`, em `Seção / Destaque` `18670`, x 18000): `Tipo` = neutro | info | aviso | perigo | prod, com tint e **borda suave do estado**. Ele substituiu 15 faixas e notas montadas como frame:
     - aprovação: faixas prod, PIN bloqueado e expirando, e as 5 notas;
     - destravar: as notas e os avisos da senha mestra e do PIN;
     - estado vazio de Credenciais;
     - aviso do `Diálogo de restauração`.
  3. **Segurança dos campos de segredo:**
     - o `Diálogo de restauração` › confirmar (`13034`) agora usa `Campo senha` (`13088`), e saiu o override da tela (`16683`);
     - em `Extensão / Salvar login novo`, a senha do site (`17450`) virou `Campo secreto` sem revelar nem copiar. Para isso criei as props `Mostrar revelar` e `Mostrar copiar` no componente;
     - o `Campo secreto` ganhou o estado `vazio` (placeholder "Nenhum valor salvo", sem revelar nem copiar) e as props `Placeholder`, `Ajuda` e `Mostrar ajuda`;
     - os 10 campos secretos dos formulários Novo passaram para `vazio`, no lugar da máscara apagada por override.
  4. **Popup `Salvar login novo`** com 600 de altura: o corpo rola (`overflow` vertical) entre o cabeçalho e o rodapé fixos. Tipo e Ambiente ficaram lado a lado (2 × 160 + 8 = 328).
  5. **`Select` › `Estado=combobox`** (`18780`): busca à esquerda, texto digitado com cursor, a opção encontrada e "Criar grupo …". Ele substituiu o `Select` aberto que fazia o papel de autocompletar em `login / Novo` e `smtp / Novo`.
  6. **Formulários:**
     - a `Navegação lateral` e a `lista` ficaram com altura `fill` em 9 frames (`7450` e os da Fase 5);
     - apaguei os 12 `Item de lista` ocultos das listas de api e aws;
     - a prop `Valor` do `Campo extra` virou `Valor (só se não for segredo)`.
  7. **Toast** com borda suave por estado. Criei 4 primitivas `*-32` e as variables `info/borda-suave`, `aviso/borda-suave`, `perigo/borda-suave`, `sucesso/borda-suave` e `info/sutil`.
  8. **Contraste do `Badge ambiente`**, medido pela WCAG sobre o tint composto:
     - o texto do prod ficava em 4,0 sobre `superficie/elevada` e `superficie/selecionada`. Troquei `ambiente/prod/texto` para a nova `vermelho/300` (#F89090), e o mínimo subiu para 5,3;
     - dev e stage já passavam (≥ 4,9). A tabela está em `design-nodes.md`.
  9. **Bandeja:**
     - `Ícone da bandeja` ganhou a propriedade `Cor` = branco | grafite (`marca/grafite` nova), para painéis claros;
     - o travado agora é **contorno** e não só cinza;
     - o `Ícone da extensão` travado ficou sem as ligações de gradiente que sobravam;
     - os exports foram regravados ou acrescentados em `docs/design/icones/`.
  10. **`Gerador de senha / No formulário` (`14307`):** comparei com `6926` por script. Só difere no popover e no `Mostrar gerar` do campo, então está em dia.
  11. **`design-nodes.md`:**
      - consolidei os ids das Fases 5, 6, 7, 8, 9, 9a e 10b e acrescentei a seção da 10c;
      - corrigi as referências aos frames substituídos;
      - conferi por script que os 425 ids citados nas seções novas existem no arquivo salvo.

## Segurança
- Invariantes: a fase é só de design e não toca em código.
  - **S1, R6 e V0.20:** campo de segredo vazio não tem revelar nem copiar. A senha do site na extensão aparece só como máscara, sem revelar nem copiar. A senha mestra do restaurar usa o `Campo senha`, que não tem copiar.
  - Varredura no arquivo salvo: 0 ocorrências dos segredos falsos do contrato.
  - Auditoria de valores soltos nos 54 frames do Layout: 0. Na seção nova, só sobra a borda tracejada padrão do set.
- Testes de ataque: não se aplicam, porque o M0 não tem (SEG).
- Dependências novas: nenhuma. Os scripts de leitura usam só a biblioteca padrão e ficam no scratchpad (`f10c/`).

## Ficou de fora
- **Bordas por estado no `Chip de filtro`:** ele não é usado em nenhuma tela desde a 4f e está na lista de remoção da 10a. Não vale estilizar antes dessa decisão.
- **Nível combinado (D19) na janela de aprovação:** não estava na lista da 10c.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não existe antes da Fase 11 (fase só de design) |
| `variable.*`, `component.*`, `instance.*`, `node.set`/`reparent`/`reorder`, `edit.delete`, `style.apply`, `file.export` | ok |
| `screenshot` das telas e dos sets afetados (1x a 3x) | ok |
| `python3 -I f5/audit.py` em todo o Layout e nas seções alteradas | 0 pendências (só a borda padrão do set) |
| Conferência dos ids do `design-nodes.md` contra o arquivo salvo; `FigCraft.AppImage cli info` | 0 faltando; 229 variables |
| `save_file` (14×) | ok |

## Problemas descobertos
- **Limitação do FigCraft com os modos de variable:**
  - a instância copia o modo do componente só quando é criada, e trocar a variante não atualiza o modo;
  - por isso, numa instância de `Botão` (ou de outro componente da tabela) cuja variante for trocada depois, é preciso acertar `variable.setMode` (`Tom do ícone`) no nó;
  - registrei isso em `design-nodes.md`. No código, o equivalente é `currentColor`;
  - é uma decisão de abordagem: "ícones coloridos por override viram **variantes**" virou "viram **modos**", porque variante por cor em 29 ícones multiplicaria os componentes.
- **V0.19:**
  - a coleção nova `Tom do ícone` tem 8 modos, mas eles não são temas. `Cores` continua só com `Escuro`;
  - a variante `Cor=grafite` do ícone da bandeja serve a painéis claros do sistema, não ao app;
  - um validador literal pode estranhar as duas coisas.
- **Bandeja travada em 16 px:** o contorno se distingue, mas fica pesado em 16, porque o traço de 2 vira 1 ao reduzir. Vale o usuário olhar na aprovação (V0.23). As folhas ampliadas estão em `scratchpad/f10c/folha-bandeja.png` e `folha-grafite.png`.
- **As variantes grafite somem no fundo escuro do set:** elas aparecem certas só sobre fundo claro, como mostram os exports.
- **O `Campo secreto` vazio tem ajuda fixa** ("Digite ou gere o valor…"). A prop `Ajuda` vale só para o estado oculto, porque revelado e copiado têm textos de estado próprios.
- **Medidas sem token:** o `cursor` do combobox tem largura `linha/fina` (1). A largura de apresentação do `Destaque` (480) não tem token, como os outros mestres. A largura 246 dos combobox absolutos continua sem token, como já era na Fase 5.

## Frames de tela mais alterados (para exportar)
- **Aprovação** (`Destaque` novo, ícones por modo): `16701`, `16810`, `16885`, `16960`, `17035`.
- **Acesso** (`Destaque` no lugar das notas e avisos): `3619`, `3733`, `3342`, `3393`, `3450`.
- **Credenciais:** `4780` (estado vazio).
- **Restaurar:** `16512` (Campo senha do componente, aviso).
- **Extensão:** `17414` (600 de altura, `Campo secreto`, Tipo e Ambiente lado a lado).
- **Bandeja:** `17300` (ícone travado em contorno).
- **Formulários Novo** (`Campo secreto` vazio, combobox, altura das colunas):
  - `7450` e `13867` (combobox);
  - `8537`, `9900`, `11272`, `12170`, `14743`.
- **Formulários Edição** (altura das colunas; api e aws sem itens ocultos): `10389`, `11705`, `7908`, `8981`.
- **Gerador:** `13141` (ícone do "Copiar" pelo modo).
- **Nova credencial:** `18055` (ícone do cartão selecionado em marca).
- **Botões com ícone e item ativo da navegação:** todas as telas de três colunas mudaram um pouco. O ícone do item ativo agora fica em `icone/marca`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `docs/mission/design-nodes.md` | alterado (consolidação das Fases 5–10b e seção 10c) |
| `docs/design/icones/bandeja-travado.svg`, `bandeja-travado-{16,24,32}.png` | alterado (contorno) |
| `docs/design/icones/bandeja-{destravado,travado}-grafite.svg` e `-{16,24,32}.png` | criado |
| `docs/mission/handoffs/phase-10c.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) no M0)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase
