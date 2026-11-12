# Fase 4a — (DESIGN) Inventário e variables no `Ward.figcraft`

## Implementado
- Trabalho feito no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem `open_file` de outro arquivo.
  Cinco `save_file` no mesmo caminho, entre intermediários e o final.
- **Inventário** (`inspect_document`, `component.list`, `variable.export`): o arquivo tinha
  só a página Design System (`2`, 14 frames de topo), 156 variables em 5 coleções,
  14 text styles e 2 effect styles, e todos os componentes das Fases 2–4 já importados
  (base, ícones, `Painel de marca`, `Indicador de passos`, navegação, nível de acesso).
  Tudo registrado em `docs/mission/design-nodes.md`.
- **Inspirações**: o `Ward.figcraft` não tem essa página. Pela D22 ela não foi criada.
  Li a `1:3` do Figma só com leitura (`get_metadata`, `get_screenshot`). Os 4 filhos são
  idênticos ao snapshot da Fase 1. A conferência está em
  `docs/mission/evidence/fase-4a/inspiracoes-1-3-conferencia.json`. Como não há página
  no FigCraft, `figcraft-inspiracoes.json` não foi gerado.
- **Páginas**: criei a `Layout` (`2796`, vazia). A Design System foi mantida, sem
  duplicar. O modo `Cores = Escuro` ficou fixado nas duas páginas (V0.19).
- **Variables** (157 no fim, em 6 coleções). Ajustei as que existiam no lugar, mantendo
  os ids, para que tudo que já estava ligado acompanhasse a mudança:
  - `Raio` (D23): `raio/sm` 4→6, `md` 6→10, `lg` 10→12, `xl` 14→16, `2xl` 20, `total`
    9999 e o novo `raio/3xl` = 24. Também criei os papéis como alias da escala:
    `raio/controle` (→md, 10), `raio/item` (→lg, 12), `raio/cartao` (→xl, 16),
    `raio/painel` (→2xl, 20), `raio/janela` (→3xl, 24) e `raio/pilula` (→total). Os
    botões, campos e seções já ficaram arredondados no canvas, conferido por screenshot.
  - `Largura` (nova, modo `Valor`, escopo `WIDTH_HEIGHT`): `janela` 1280,
    `barra-lateral` 264, `conteudo` 960, `formulario` 640, `campo` 320, `select` 320,
    `select-filtro` 200, `dialogo` 480, `dialogo-largo` 560 (aprovação), `toast` 360,
    `popup` 360 (extensão).
  - `Espaçamento`: `espaco/80` e `espaco/96`, para dar mais respiro (D23).
  - `Tipografia`: `tamanho/3xl` = 32 e `altura-linha/3xl` = 40, para o título de destaque.
  - `Cores` (só `Escuro`): `borda/sutil` (→neutro/800, `STROKE_COLOR`) e `sombra/suave`
    (→ nova primitiva `sombra/preto-24` #0000003D, `EFFECT_COLOR`), para as bordas sutis
    e as sombras suaves de D23. As semânticas da Fase 1, as `icone/*` e a `sombra/padrao`
    da Fase 2 já estavam lá e ficaram.
  - `Primitivas`: removi os 24 tons sem uso. Antes, conferi o uso real no
    `document.json` salvo: nenhum alias em `Cores` e nenhuma ligação em node. Foram
    removidos `neutro/100,200,300,500`, `azul/50,100,600,700`, `vermelho/50,100,700`,
    `ambar/50,100,600,700`, `verde/50,100,600,700`, `ciano/50,100,600,700` e
    `sobreposicao/claro`. `neutro/0` e `neutro/50` ficam, porque estão em uso. Agora
    são 28 primitivas.
  - Code syntax WEB restaurado nas 157 variables, porque a importação o tinha perdido.
    O formato segue o da amostra `Tokens / Cores`: `var(--cor-…)` em `Cores`,
    `var(--primitiva-…)` e `var(--<nome>)` nas demais.
- Conferência final na versão salva em disco, lendo o `document.json` de uma cópia
  descompactada no scratchpad: páginas `2` e `2796` com `var_modes {Cores: Escuro}`,
  6 coleções e 157 variables. O `inspect_document` mostra `dirty: false`.

## Segurança
- Invariantes tocadas: nenhuma S1–S15 diretamente, porque a fase é só de design.
  Não toquei em nenhum cofre, credencial, `~/.ssh` ou `~/.aws`. Os arquivos novos não
  têm nenhum segredo.
- O Figma `bzu9PxxFur2tU3vlNCHvsY` foi usado só para leitura (`create_session`,
  `get_metadata`, `get_screenshot`), sem nenhuma escrita.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
- Revisão dos text styles e effect styles: os effect styles ainda não usam
  `sombra/suave`, e a sombra continua com offset e raio fixos. Isso é da Fase 4b.
- Religar os componentes aos papéis novos de raio (`raio/controle`, `raio/pilula`
  nos selos etc.) e às `largura/*`: é das Fases 4c–4d. Hoje os componentes usam a
  escala (`raio/md`, `raio/lg`…), que já tem os valores novos.
- A amostra `Tokens / Cores` (`3`) não mostra `borda/sutil`, `sombra/suave` e
  `icone/*`, e não há amostra de `Raio` nem de `Largura`. Não é exigido; pode entrar
  na Fase 4b ou na 10.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (ele é criado na Fase 11) |
| `inspect_document`, `component.list`, `variable.export` (inventário) | ok |
| Figma `get_metadata 1:3` / `get_screenshot 1:3` (só leitura) | ok, idêntico ao snapshot |
| `page.add`, `variable.*` (12 batches), `variable.setMode` | ok |
| `save_file` (5×, mesmo caminho) | ok |
| Conferência do `.figcraft` salvo (unzip numa cópia no scratchpad) | ok |

## Problemas descobertos
- **Não existe a página Inspirações no `Ward.figcraft`**. O arquivo guarda 4 imagens
  (`imageIndex`, 1024×768 e 1011×665) que nenhum node usa. Parecem as referências que
  não vieram numa página. Pela D22 a página é do usuário e as fases não a criam. Se o
  usuário quiser, ele pode montar a página com essas imagens. V0.21 fica só com a
  lista da `1:3`.
- **Divergência na nomenclatura de ids**: os handoffs 1–4 citam ids do Figma
  (`10:69` etc.). No FigCraft, os nodes têm ids inteiros (`263` etc.), e só variables
  e estilos mantêm o formato `n:m`. Quem for validar deve usar o `design-nodes.md`,
  não os handoffs antigos.
- O plano da Fase 10a manda podar primitivas sem uso. Já fiz isso aqui, porque a 4a
  pede "só as primitivas em uso". A 10a deve só reconferir. A primitiva nova
  `sombra/preto-24` está em uso por `sombra/suave`.
- O escopo (Missão 0) não cita `Campo PIN`, mas o contrato exige; a divergência já
  estava registrada na Fase 2. A coleção `Primitivas` é uma 6ª coleção além das cinco
  do V0.1, que diz "existem", não "só".
- O modo da `Largura` saiu com o id `VariableCollectionId:1/mode1`, gerado pelo
  FigCraft. É só cosmético.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (página Layout, variables) |
| `docs/mission/design-nodes.md` | criado |
| `docs/mission/evidence/fase-4a/inspiracoes-1-3-conferencia.json` | criado |
| `docs/mission/handoffs/phase-4a.md` | criado |

Ids do FigCraft desta fase: página Layout `2796`; coleção `Largura`
`VariableCollectionId:1` (`VariableID:1`–`11`); `raio/3xl` `VariableID:12`; papéis de
raio `VariableID:13`–`18`; `espaco/80` `19`, `espaco/96` `20`; `tamanho/3xl` `21`,
`altura-linha/3xl` `22`; `sombra/preto-24` `23`; `borda/sutil` `24`; `sombra/suave` `25`
(todos `VariableID:n`). A tabela completa está em `docs/mission/design-nodes.md`.

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito; versão em disco conferida)
- [x] Ficou dentro do escopo da fase
