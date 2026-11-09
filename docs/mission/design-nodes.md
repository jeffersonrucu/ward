# Nodes do design — Ward.figcraft

Arquivo: `/home/jeffersonoliveira/documentos/Ward.figcraft` (FigCraft, D22). Ids de node
são inteiros do FigCraft; ids de variable e de estilo são strings (as que vieram do Figma
mantêm o formato `6:xx`; as criadas no FigCraft são `VariableID:n`). O Figma
`bzu9PxxFur2tU3vlNCHvsY` é só histórico: os ids `n:m` dos handoffs 1–4 **não** valem
aqui.

Atualizado na Fase 4a (2026-11-08), na Fase 4b (estilos, ícones, coleção `Elevação`), na Fase 4c (componentes base parte 1, `Ação de campo`, brilho e perigo translúcido) na Fase 4d (componentes base parte 2 e de apoio, altura padrão comum `controle/padrao`) na Fase 4g (logo), na Fase 4e (telas de acesso no Layout, logo na navegação) e na Fase 4f (telas de Credenciais e Nível de acesso, campos que esticam, Densidade compacta, Diálogo de filtros, "Cliente" virou "Grupo", selos em minúsculas). Fase 10a: limpeza de tokens e do `Chip de filtro` (seção no fim). Fase 10: conferência final do M0 (seção no fim). Fase 10d: ajustes do scrutiny e do QA do M0 (seção no fim). Fase 37a: correções de D65 (f) e design pendente da Fase 32 (seção no fim). As Fases 5–9a e 10b (feitas em paralelo) foram consolidadas aqui na Fase 10c, que também registra os refinamentos dela (seções no fim do arquivo).

## Páginas

| Página | Id | Uso | Modo `Cores` |
| --- | --- | --- | --- |
| Design System | `2` | variables, estilos, componentes | `Escuro` (`6:2`), fixado na página |
| Layout | `2796` | telas e estados (criada na Fase 4a; telas desde a 4e) | `Escuro` (`6:2`), fixado na página |
| Inspirações | — | não existe no `Ward.figcraft`; referência lida no Figma `1:3` (só leitura) | — |

## Frames de topo da página Design System (inventário da Fase 4a)

Vieram da importação do Figma. As Fases 4b–4d ajustam no lugar.

| Frame | Id |
| --- | --- |
| `Tokens / Cores` | `3` |
| `Componentes / Ícones` | `178` |
| `Seção / Botão` | `260` |
| `Seção / Campo` | `328` |
| `Seção / Campo secreto` | `372` |
| `Seção / Ação de campo` | `2965` (Fase 4c) |
| `Seção / Select` | `418` |
| `Seção / Badge ambiente` | `459` |
| `Seção / Badge tipo` | `475` |
| `Seção / Toast` | `522` |
| `Seção / Item de lista` | `550` |
| `Seção / Modal` | `608` |
| `Seção / Campo PIN` | `647` |
| `Seção / Telas de acesso` | `716` |
| `Seção / App` | `760` |
| `Seção / Logo` | `3286` (Fase 4g) |
| `Seção / Filtros` | `5219` (Fase 4f; x = 640, y = 7496) |

Posições (Fase 4d): coluna x = 640, 40 de respiro; y = `178` 0, `260` 144, `328` 636, `372` 966, `2965` 1296, `418` 1572, `459` 2028, `475` 2338, `522` 2594, `550` 2888, `608` 3382, `647` 3796, `716` 4138, `760` 5042, `3286` 6674 (Fase 4g).

## Componentes existentes (inventário da Fase 4a, ainda no estilo antigo)

Importados do Figma; raios já seguem os valores novos de `Raio` pelas ligações. Revisão
(D23) nas Fases 4b–4d, que atualizam esta tabela.

| Componente | Id | Variantes (id) |
| --- | --- | --- |
| `Botão` (set) — revisado 4c | `263` | primário padrão `264`, hover `268`, foco `272`, desabilitado `276`; secundário `280` `284` `288` `292`; perigo `296` `300` `304` `308`; fantasma `312` `316` `320` `324` |
| `Campo` (set) — revisado 4c | `331` | vazio `332`, padrão `340`, foco `348`, erro `356`, desabilitado `364` |
| `Campo secreto` (set) — revisado 4c | `375` | oculto `376`, revelado `390`, copiado `405` |
| `Ação de campo` (set) — novo 4c | `2882` | padrão `2877`, copiado `2883` |
| `Select` (set) — revisado 4c | `421` | padrão `422`, foco `428`, aberto `434`, desabilitado `453` |
| `Badge ambiente` (set) — revisado 4d | `462` | dev/stage/prod padrão `463` `465` `467`; grande `469` `471` `473` |
| `Badge tipo` (set) — revisado 4d | `478` | login `479`, banco `484`, servidor `489`, api `495`, aws `499`, smtp `503`, ftp/sftp `508`, nota segura `512`, genérico `517` |
| `Toast` (set) — revisado 4d | `525` | sucesso `526`, erro `534`, aviso `542` |
| `Item de lista` (set) — refeito 4d; Densidade na 4f | `553` | larga: padrão `554`, hover `2977`, selecionado `3001`; compacta (4f): padrão `4002`, hover `4028`, selecionado `4045` |
| `Modal` (set) — revisado 4d | `611` | padrão `612`, perigo `628` |
| `Campo PIN` (set) — revisado 4c | `650` | vazio `651`, digitando `661`, completo `674`, erro `690`, bloqueado `706` |
| `Painel de marca` — revisado 4d; logo na 4g | `717` | — |
| `Indicador de passos` (set) — revisado 4d | `741` | 1 `742`, 2 `748`, 3 `754` |
| `Item de navegação` (set) — revisado 4d | `763` | padrão `764`, hover `770`, ativo `776` |
| `Navegação lateral` (set) — refeito 4d | `782` | Credenciais `783`, Gerador de senha `3037`, Configurações `3077` (`822` e `861` apagados e recriados a partir do `783`) |
| `Cabeçalho de grupo` — revisado 4d | `901` | — |
| ~~`Chip de filtro`~~ (set) | ~~`5164`~~ | **apagado na 10a** (com `904`–`907`): zero instâncias no arquivo |
| `Opção de nível` (set) — revisado 4d | `909` | bloqueado não/sim `910` `920`; com aprovação `931` `940`; livre `950` `958` |
| `Diálogo de nível` (set) — revisado 4d | `967` | Escolher `968`, PIN `1027`, PIN errado `1078` |

Ícones (`Componentes / Ícones` `178`): olho `179`, olho-fechado `182`, copiar `186`,
check `189`, fechar `191`, chevron-baixo `193`, chevron-direita `195`, alerta `197`,
erro `200`, sucesso `203`, cadeado `206`, busca `209`, mais `212`, relogio `214`,
clipe `217`, tipo-login `219`, tipo-banco `222`, tipo-servidor `225`, tipo-api `229`,
tipo-aws `231`, tipo-smtp `233`, tipo-ftp `236`, tipo-nota `238`, tipo-generico `241`,
chave `244`, gerador `248`, engrenagem `257`, filtro `5197` (vetor `5200`, Fase 4f).

## Estilos (revisados na Fase 4b)

Todos os campos ligados a variables. Text styles: `fontFamily`, `fontStyle`, `fontSize`,
`lineHeight` e `letterSpacing`. Effect styles: cor, x, y, desfoque e spread de cada
camada.

| Estilo | Id | Tipo | Ligações (família · estilo · tamanho/altura · letra) |
| --- | --- | --- | --- |
| `Título/Destaque` | `8:8` | texto | Inter · semibold · `3xl` 32/40 · `titulo` |
| `Título/Página` | `8:9` | texto | Inter · semibold · `xl` 22/30 · `titulo` |
| `Título/Modal` | `8:10` | texto | Inter · semibold · `lg` 18/26 · `titulo` |
| `Título/Seção` | `8:11` | texto | Inter · semibold · `md` 16/24 · `normal` |
| `Texto/Corpo` | `8:12` | texto | Inter · regular · `base` 14/20 · `normal` |
| `Texto/Corpo forte` | `8:13` | texto | Inter · medio · `base` 14/20 · `normal` |
| `Texto/Pequeno` | `8:14` | texto | Inter · regular · `sm` 13/18 · `normal` |
| `Texto/Rótulo` | `8:15` | texto | Inter · medio · `sm` 13/18 · `normal` |
| `Texto/Legenda` | `8:16` | texto | Inter · regular · `xs` 12/16 · `normal` |
| `Texto/Selo` | `8:17` | texto | Inter · medio · `2xs` 11 / `xs` 16 · `normal`, minúsculas (sem text case) — Fase 4f (era maiúsculas com letra 0,4) |
| `Texto/Etiqueta` | `S:18` | texto | Inter · medio · `xs` 12/16 · `normal` — Fase 4e, `Badge tipo` |
| `Texto/Selo grande` | `12:19` | texto | Inter · semibold · `xs` 12/16 · `normal`, minúsculas (sem text case) — Fase 4f (era maiúsculas com letra 0,4) |
| `Mono/Corpo` | `8:18` | texto | JetBrains Mono · regular · `base` 14/20 · `normal` |
| `Mono/Pequeno` | `8:19` | texto | JetBrains Mono · regular · `sm` 13/18 · `normal` |
| `Mono/PIN` | `8:20` | texto | JetBrains Mono · negrito · `xl` 22/30 · `normal` |
| `Sombra/Modal` | `8:21` | efeito | 2 camadas, cor `sombra/suave`: contato `elevacao/modal/contato-*` (0, 4, 12) + ambiente `elevacao/modal/ambiente-*` (0, 24, 64); x e spread `elevacao/nenhum` |
| `Sombra/Brilho primário` | `S:17` | efeito | Fase 4c: 1 camada, cor `primaria/brilho`, x e spread `elevacao/nenhum`, y `elevacao/brilho/y` (4), desfoque `elevacao/brilho/desfoque` (16); usado no `Botão` primário padrão/hover/foco |
| `Sombra/Flutuante` | `8:22` | efeito | 2 camadas, cor `sombra/suave`: contato `elevacao/flutuante/contato-*` (0, 2, 6) + ambiente `elevacao/flutuante/ambiente-*` (0, 12, 32); x e spread `elevacao/nenhum` |

Nodes que usam os effect styles (seguem o estilo): `Sombra/Flutuante` em `menu` `440`
(Select aberto) e Toast `526` `534` `542`; `Sombra/Modal` em Modal `612` `628` e
Diálogo de nível `968` `1027` `1078`.

## Componentes base, parte 1 (revisados na Fase 4c)

Estilo D23 + referência `docs/design/referencias/alvo-layout-app.png`. Tudo ligado a
variables (auditoria no `.figcraft` salvo: 0 preenchimento, borda, raio, padding, gap ou
texto solto nas seções `260` `328` `372` `2965` `418` `647`). As propriedades de
componente da Fase 2 tinham perdido a ligação às camadas na importação; foram religadas
(92 ligações).

| Componente | Medidas e ligações | Propriedades (ligadas a) |
| --- | --- | --- |
| `Botão` `263` | variantes em `hug`, altura `controle/padrao` (44; era `controle/md` até a 4d), raio `raio/controle`, padding `espaco/20` × `espaco/0`, gap `espaco/8`; texto `Texto/Corpo forte`. Primário com effect style `Sombra/Brilho primário` (`S:17`); secundário fundo `superficie/elevada` + borda `borda/padrao`; perigo fundo `perigo/translucido` (hover `perigo/translucido-hover`), borda `perigo/borda`, texto `perigo/texto`. Ícone: `icone/sobre-cor` (primário), `perigo/texto`, `icone/forte`, `icone/desabilitado`. Set em grid 4×4, gap `espaco/24`, padding `espaco/32`, raio `raio/cartao` | `Rótulo#10:0` → `rótulo`; `Mostrar ícone` → visível do `ícone`; `Ícone#10:34` → troca do `ícone` |
| `Campo` `331` | largura `largura/campo`, gap `espaco/8`; entrada altura `controle/padrao` (44; era `controle/lg` até a 4d), raio `raio/controle`, padding `espaco/16` / `espaco/8` (dir.), borda `borda/sutil` (vazio, padrão, desabilitado); textos `Texto/Rótulo`, `Texto/Corpo`, `Texto/Legenda`; instância de `Ação de campo` na entrada (padrão, foco, erro, desabilitado) `2932` `2937` `2942` `2947` | `Rótulo#11:0`, `Placeholder` (vazio), `Valor#11:6`, `Ajuda#11:12`, `Mostrar ajuda`, `Mensagem de erro#11:18` (erro), `Ícone à esquerda`, `Ícone#11:36`, `Mostrar copiar` (novo, padrão falso) |
| `Campo secreto` `375` | largura `largura/campo`, gap `espaco/8`; entrada altura `controle/padrao` (44), raio `raio/controle`, padding `espaco/16` / `espaco/8`, borda `borda/sutil`; ações (gap `espaco/4`): revelar `381` `395` `410`, gerar `2902` `2906` `2911` (ícone `Ícone/gerador`), copiar = instância de `Ação de campo` `2888` `2893` `2898` (copiado usa a variante `copiado`); valor revelado `Mono/Corpo`; máscara `Texto/Corpo` | `Rótulo#11:42`, `Valor revelado` (só revelado), `Mostrar gerar` (novo, padrão falso; ligado à visibilidade de `ação gerar`) |
| `Ação de campo` `2882` | altura `controle/sm`, raio `raio/sm`, padding `espaco/8` / `espaco/12`, gap `espaco/6`, ícone 16 (`icone/sm`), texto `Texto/Rótulo`; padrão: fundo `superficie/elevada`, borda `borda/padrao`; copiado: fundo `sucesso/sutil`, borda e texto `sucesso/texto`, ícone `Ícone/check` | `Rótulo` (padrão), `Ícone` (troca, padrão) |
| `Select` `421` | largura `largura/select` (desligada na 4f; variable apagada na 10a), gap `espaco/8`; entrada altura `controle/padrao` (44), raio `raio/controle`, padding `espaco/16`, borda `borda/sutil` (padrão, desabilitado); menu raio `raio/controle`, padding `espaco/4`, hug; opções altura `controle/md`, raio `raio/sm`, padding `espaco/12` | `Rótulo#11:50`, `Valor#11:55` |
| `Campo PIN` `650` | variantes e `dígitos` em `hug`; dígito 48×48 (`controle/lg`), raio `raio/controle`, gap `espaco/12`; máscara `Mono/PIN`; textos `Texto/Rótulo`, `Texto/Legenda` | `Rótulo#13:22`, `Ajuda#13:28`, `Mensagem de erro#13:34` (erro; o texto do bloqueado é fixo), `Seis dígitos` → visível dos dígitos 5 e 6 |

Seções `260` `328` `372` `2965` `418` `647`: raio `raio/painel`, padding `espaco/32`,
`hug` na altura, título `Título/Seção`, descrição `Texto/Pequeno`. Sets: raio
`raio/cartao`, padding `espaco/32`, gap `espaco/24`. A coluna de seções foi restaurada
com 40 de respiro entre elas (só posição; as seções das Fases 4d+ não foram editadas).

## Componentes base, parte 2, e de apoio (revisados na Fase 4d)

Estilo D23 + referência `docs/design/referencias/alvo-layout-app.png` + regra de respiro.
Auditoria no `.figcraft` salvo (seções `459` `475` `522` `550` `608` `716` `760`): 0
preenchimento, borda, espessura, raio, padding, gap ou cor de texto soltos, 0 texto sem
text style, 0 propriedade sem camada ligada. Seções `459`–`608`: raio `raio/painel`,
padding `espaco/32`, `hug`; sets: auto layout, padding `espaco/32`, gap `espaco/24`, raio
`raio/cartao`. `716` e `760`: raio `raio/painel`, padding `espaco/32`, `hug`; sets de
`760` com fundo `fundo/padrao` e borda `borda/padrao`.

| Componente | Medidas e ligações | Propriedades (ligadas a) |
| --- | --- | --- |
| `Badge ambiente` `462` | compacto e discreto (pedido do usuário na Fase 4e): pílula `raio/pilula`, `hug`, sem borda; padrão 20 de altura, padding `espaco/2` × `espaco/8`, fundo em tint translúcido `ambiente/<amb>/tint`, texto `ambiente/<amb>/texto` em `Texto/Selo` (11/16 médio, minúsculas desde a 4f; texto do prod em `vermelho/300` desde a 10c); grande 24 de altura, padding `espaco/4` × `espaco/12`, `Texto/Selo grande` (12/16 semibold); dev/stage grandes em tint, prod grande sólido `ambiente/prod/destaque`; set em grid 3×2 | — (variantes Ambiente × Tamanho) |
| `Badge tipo` `478` | compacto (Fase 4e): pílula `raio/pilula`, `hug`, 20 de altura, sem borda, fundo `selo/neutro/fundo` (tint neutro translúcido), padding `espaco/2`, `espaco/6` (esq.) / `espaco/8` (dir.), gap `espaco/4`; ícone 12 (`icone/xs`); texto `Texto/Etiqueta` (12/16 médio) | — (variante Tipo, 9) |
| `Toast` `525` | largura `largura/toast`, `hug`, raio `raio/cartao`, padding `espaco/16` / `espaco/12` (dir.); ícone na cor do tipo (`sucesso/texto`, `perigo/texto`, `aviso/texto`); texto `Texto/Corpo`; `Sombra/Flutuante` | `Mensagem` → `mensagem` (`530` `538` `546`) |
| `Item de lista` `553` | largura 720, `hug`, raio `raio/item`, padding `espaco/16`, gap `espaco/16`; `avatar` 40×40 (`controle/md`, `raio/controle`, `superficie/elevada`, borda `borda/padrao`) com o ícone do tipo; colunas fixas `coluna tipo` (`largura/coluna-tipo`), `coluna ambiente` (`largura/coluna-ambiente`) e `nível` (`largura/coluna-nivel`), para os badges alinharem em colunas na lista (D23); hover `superficie/elevada`; selecionado `superficie/selecionada` + borda `borda/foco`; textos `Texto/Corpo forte`, `Texto/Pequeno`, `Texto/Legenda` | `Nome` → `nome`; `Detalhe` → `detalhe`; `Nível` → `nível texto`; `Mostrar nível` → visível de `nível`; `Ícone` (novo, troca) → ícone do avatar; `Mostrar colunas` (novo, padrão verdadeiro) → visível de `coluna tipo` e `coluna ambiente` (lista estreita da 4f) |
| `Modal` `611` | largura `largura/dialogo`, `hug`, raio `raio/janela`, padding `espaco/24`, gap `espaco/20`; rodapé gap `espaco/12`, botões em `hug`; ícone do perigo em `perigo/texto`; `Sombra/Modal` | `Título` → `título`; `Texto` → `texto` |
| `Painel de marca` `717` | raio `raio/painel`, borda `borda/sutil` nos 4 lados (Fase 4e; antes só à direita) com espessura `linha/fina`, padding `espaco/48`; logo `raio/controle`, cadeado em `icone/sobre-cor`; textos com text style | — |
| `Indicador de passos` `741` | variantes em `hug` (antes cortavam: 10 de altura para 30 de conteúdo), gap `espaco/12`; rótulo `Texto/Rótulo` | — (variante Passo) |
| `Item de navegação` `763` | altura `controle/md`, raio `raio/controle`, padding `espaco/8` × `espaco/12`; hover `superficie/padrao`; ativo `superficie/elevada` + borda `borda/padrao`, texto `Texto/Corpo forte` | `Rótulo#22:0` → `rótulo`; `Ícone#22:4` → troca do `ícone` |
| `Navegação lateral` `782` | painel arredondado: largura `largura/barra-lateral` (264), raio `raio/painel`, borda `borda/sutil` nos 4 lados, padding `espaco/24` × `espaco/16`, gap `espaco/32`; marca = instância `Logo / Horizontal` Marca (Fase 4e: `3327` em `784`, `3332` em `3038`, `3337` em `3078`; apagados o quadrado com cadeado e o texto "Ward" `785` `789` `3039` `3043` `3079` `3083`); rodapé em cartão (`superficie/padrao`, `raio/cartao`, padding `espaco/16`) com "Cofre destravado" e `Botão` secundário "Travar agora" com ícone cadeado | — (variante Ativo) |
| `Cabeçalho de grupo` `901` | padding `espaco/16` (topo) / `espaco/8` / `espaco/4`; `Título/Seção`, `Texto/Legenda` | `Cliente`, `Contagem` (já ligadas) |
| ~~`Chip de filtro` `904`~~ (apagado na 10a) | pílula `raio/pilula`, `hug`, padding `espaco/6` × `espaco/12`/`espaco/8`, borda `borda/foco`; x 16 (`icone/sm`); `Texto/Rótulo` | `Rótulo#22:10` (já ligada) |
| `Opção de nível` `909` | variantes em `hug` (antes 80 fixo, texto colado embaixo), padding `espaco/20`, gap `espaco/16`, raio `raio/cartao`; textos gap `espaco/6`, explicação com altura automática | — (variantes Nível × Selecionado) |
| `Diálogo de nível` `967` | largura `largura/dialogo-largo` (560), `hug`, raio `raio/janela`, padding `espaco/24`, gap `espaco/20`; cartão da entrada padding `espaco/16`, raio `raio/cartao`, borda `borda/sutil`; opções em `hug`; botões em `hug` | `Entrada` → `nome da entrada` (`979` `1038` `1089`) |

Pendências da 4c resolvidas aqui: `Botão` (16 variantes) e as entradas de `Campo`
(`334` `342` `350` `358` `366`), `Campo secreto` (`378` `392` `407`) e `Select` (`424`
`430` `436` `455`) ligados à altura comum `controle/padrao` (44); a ajuda do `Campo
secreto` oculto (`389`) ficou visível, e os três estados têm 86 de altura.

## Logo (Fase 4g)

Conceito escolhido pelo usuário em 2026-11-08 (`docs/design/referencias/logo-ward.png`,
linha de cima): escudo com "W" interno e losango embaixo. Redesenhado do zero em vetor
(nenhuma imagem colada): um único vetor de 3 contornos (escudo com o vazado e o chevron,
losango), espessura uniforme, cantos arredondados por filete, sentido do vazado invertido
(funciona com `nonzero` e `evenodd`). Seção `Seção / Logo` (`3286`): título `3306`,
descrição `3307`, linha `componentes` `3308`, linha `tamanhos` `3309` (instâncias do
símbolo em 16 `3310` `3316`, 24 `3312` `3318`, 32 `3314` `3320`, cor e branco).

| Componente | Id | Variantes (id) | Medidas e ligações |
| --- | --- | --- | --- |
| `Logo / Símbolo` (set) | `3289` | `Cor=Marca` `3287` (vetor `símbolo` `3288`), `Cor=Branco` `3290` (vetor `3291`) | quadrado 64×64; vetor ocupa a altura (58,3×63,6, centrado), restrições `scale`; preenchimento `marca/simbolo` (Marca) e `marca/sobre-cor` (Branco). Set: `fundo/padrao`, `borda/padrao`, `linha/fina`, `raio/cartao`, padding `espaco/32`, gap `espaco/24` |
| `Logo / Ícone do app` | `3292` | — | 512×512, raio 115 com `cornerSmoothing` 0,6 (squircle), gradiente linear do canto superior esquerdo ao inferior direito com as paradas ligadas a `marca/gradiente-inicio` e `marca/gradiente-fim`; instância `Logo / Símbolo` Branco `3293` em 320×320 (x = y = 96) |
| `Logo / Horizontal` (set) | `3300` | `Cor=Marca` `3295` (símbolo `3296`, `palavra` `3299`, nome `3298`), `Cor=Branco` `3301` (símbolo `3302`, `palavra` `3304`, nome `3305`) | auto layout, gap `espaco/8`; símbolo 40×40 (`controle/md`); "ward" em `Título/Destaque` (Inter semibold 32/40), cor `texto/primario` (Marca) ou `marca/sobre-cor` (Branco); `palavra` com altura `controle/md` e padding inferior `espaco/4` para a altura-x ficar no centro óptico do símbolo. Set com o mesmo estilo do `Logo / Símbolo` |

Uso: `Painel de marca` `717` › `marca` `718` agora tem a instância `Logo / Horizontal`
`3322` (Marca) no lugar do quadrado com cadeado e do texto "Ward" (apagados `719`, `723`).

Exportados em `docs/design/logo/`: `ward-icone-app.svg` e PNG 16, 24, 32, 48, 64, 128,
256, 512; `ward-simbolo.svg` e PNG 16–512 (mesmos tamanhos); `ward-simbolo-branco.svg` e
PNG 16, 32, 128, 512; `ward-horizontal.svg`, `ward-horizontal-branco.svg` e PNG `@4x`.
SVGs com texto em contorno (sem `<text>`).

## Ícones (revisados na Fase 4b)

27 componentes `Ícone/*` no frame `Componentes / Ícones` (`178`, raio `raio/painel`).
Cada um: 20 px (`icone/md`), traço `linha/icone` (1,5) com pontas e cantos
arredondados, cor `icone/padrao`. Ids na seção acima. `Ícone/tipo-generico` (`241`)
foi redesenhado como etiqueta (vetores `Etiqueta` `2815` e `Furo` `2817`), porque o
desenho antigo (chave) era igual ao `Ícone/chave`.

## Variables (Fase 4a; `Elevação` na Fase 4b)

8 coleções, 226 variables (229 até a 10c; −3 na 10a: `sombra/padrao`, `largura/select`, `espacamento-letra/caixa-alta`) (205 até a 4f; +1 na 5; +6 na 8; +5 na 9; +12 na 10c, com a coleção `Tom do ícone`) (166 até a 4b; +9 na 4c; +4 na 4d; +5 na 4g; +11 na 4e; +10 na 4f, depois de criar e apagar os 10 tints de grupo `VariableID:67`–`71` e `77`–`81`). Code syntax WEB em todas: `var(--cor-…)` em `Cores`,
`var(--primitiva-…)` em `Primitivas`, e `var(--<nome com hífens>)` nas demais.

### `Primitivas` — `6:2` (modos: `Valor` `6:0`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `neutro/0` | `6:3` | #FFFFFF | — (oculta) |
| `neutro/50` | `6:4` | #F7F8FA | — (oculta) |
| `neutro/400` | `6:8` | #9AA1AE | — (oculta) |
| `neutro/600` | `6:10` | #4B5262 | — (oculta) |
| `neutro/700` | `6:11` | #343A46 | — (oculta) |
| `neutro/800` | `6:12` | #24282F | — (oculta) |
| `neutro/850` | `6:13` | #1C1F25 | — (oculta) |
| `neutro/900` | `6:14` | #16181D | — (oculta) |
| `neutro/950` | `6:15` | #101114 | — (oculta) |
| `azul/300` | `6:18` | #93A8FB | — (oculta) |
| `azul/400` | `6:19` | #6E8BF7 | — (oculta) |
| `azul/500` | `6:20` | #4A6CF0 | — (oculta) |
| `azul/900` | `6:23` | #1B2559 | — (oculta) |
| `vermelho/400` | `6:26` | #F26B6B | — (oculta) |
| `vermelho/500` | `6:27` | #E5484D | — (oculta) |
| `vermelho/600` | `6:28` | #C93A3F | — (oculta) |
| `vermelho/900` | `6:30` | #4A1517 | — (oculta) |
| `ambar/400` | `6:33` | #F5B544 | — (oculta) |
| `ambar/500` | `6:34` | #E89B12 | — (oculta) |
| `ambar/900` | `6:37` | #3F2A06 | — (oculta) |
| `verde/400` | `6:40` | #4CC77E | — (oculta) |
| `verde/500` | `6:41` | #2FAE63 | — (oculta) |
| `verde/900` | `6:44` | #0F3320 | — (oculta) |
| `ciano/400` | `6:47` | #3CC2C2 | — (oculta) |
| `ciano/500` | `6:48` | #14A3A3 | — (oculta) |
| `ciano/900` | `6:51` | #073636 | — (oculta) |
| `sobreposicao/escuro` | `6:53` | #00000099 | — (oculta) |
| `sombra/preto-24` | `VariableID:23` | #0000003D | — (oculta) |
| `azul/500-40` | `VariableID:35` | #4A6CF066 | — (oculta) — Fase 4c |
| `vermelho/500-16` | `VariableID:36` | #E5484D29 | — (oculta) — Fase 4c |
| `vermelho/500-24` | `VariableID:37` | #E5484D3D | — (oculta) — Fase 4c |
| `azul/600` | `VariableID:48` | #3550DC | — (oculta) — Fase 4g: fim do gradiente do ícone |
| `ciano/400-16` | `VariableID:54` | #3CC2C229 | — (oculta) — Fase 4e |
| `ambar/400-16` | `VariableID:55` | #F5B54429 | — (oculta) — Fase 4e |
| `vermelho/400-16` | `VariableID:56` | #F26B6B29 | — (oculta) — Fase 4e |
| `neutro/400-12` | `VariableID:57` | #9AA1AE1F | — (oculta) — Fase 4e |
| `violeta/400` | `VariableID:64` | #A78BFA | — (oculta) — Fase 4f |
| `rosa/400` | `VariableID:65` | #F472B6 | — (oculta) — Fase 4f |
| `laranja/400` | `VariableID:66` | #FB923C | — (oculta) — Fase 4f |

### `Cores` — `6:54` (modos: `Escuro` `6:2`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `fundo/padrao` | `6:55` | → `neutro/950` | FRAME_FILL, SHAPE_FILL |
| `fundo/sutil` | `6:56` | → `neutro/900` | FRAME_FILL, SHAPE_FILL |
| `fundo/sobreposicao` | `6:57` | → `sobreposicao/escuro` | FRAME_FILL, SHAPE_FILL |
| `superficie/padrao` | `6:58` | → `neutro/850` | FRAME_FILL, SHAPE_FILL |
| `superficie/elevada` | `6:59` | → `neutro/800` | FRAME_FILL, SHAPE_FILL |
| `superficie/campo` | `6:60` | → `neutro/900` | FRAME_FILL, SHAPE_FILL |
| `superficie/hover` | `6:61` | → `neutro/700` | FRAME_FILL, SHAPE_FILL |
| `superficie/selecionada` | `6:62` | → `azul/900` | FRAME_FILL, SHAPE_FILL |
| `superficie/desabilitada` | `6:63` | → `neutro/800` | FRAME_FILL, SHAPE_FILL |
| `texto/primario` | `6:64` | → `neutro/50` | TEXT_FILL |
| `texto/secundario` | `6:65` | → `neutro/400` | TEXT_FILL |
| `texto/desabilitado` | `6:66` | → `neutro/600` | TEXT_FILL |
| `texto/inverso` | `6:67` | → `neutro/950` | TEXT_FILL |
| `texto/marca` | `6:68` | → `azul/300` | TEXT_FILL |
| `borda/padrao` | `6:69` | → `neutro/700` | STROKE_COLOR |
| `borda/forte` | `6:70` | → `neutro/600` | STROKE_COLOR |
| `borda/foco` | `6:71` | → `azul/400` | STROKE_COLOR |
| `primaria/padrao` | `6:72` | → `azul/500` | FRAME_FILL, SHAPE_FILL |
| `primaria/hover` | `6:73` | → `azul/400` | FRAME_FILL, SHAPE_FILL |
| `primaria/sutil` | `6:74` | → `azul/900` | FRAME_FILL, SHAPE_FILL |
| `primaria/sobre` | `6:75` | → `neutro/0` | TEXT_FILL |
| `perigo/padrao` | `6:76` | → `vermelho/600` | FRAME_FILL, SHAPE_FILL |
| `perigo/hover` | `6:77` | → `vermelho/500` | FRAME_FILL, SHAPE_FILL |
| `perigo/sutil` | `6:78` | → `vermelho/900` | FRAME_FILL, SHAPE_FILL |
| `perigo/texto` | `6:79` | → `vermelho/400` | FRAME_FILL, SHAPE_FILL, TEXT_FILL, STROKE_COLOR |
| `perigo/sobre` | `6:80` | → `neutro/0` | TEXT_FILL |
| `aviso/padrao` | `6:81` | → `ambar/400` | FRAME_FILL, SHAPE_FILL |
| `aviso/sutil` | `6:82` | → `ambar/900` | FRAME_FILL, SHAPE_FILL |
| `aviso/texto` | `6:83` | → `ambar/400` | FRAME_FILL, SHAPE_FILL, TEXT_FILL, STROKE_COLOR |
| `sucesso/padrao` | `6:84` | → `verde/500` | FRAME_FILL, SHAPE_FILL |
| `sucesso/sutil` | `6:85` | → `verde/900` | FRAME_FILL, SHAPE_FILL |
| `sucesso/texto` | `6:86` | → `verde/400` | FRAME_FILL, SHAPE_FILL, TEXT_FILL, STROKE_COLOR |
| `ambiente/dev/fundo` | `6:87` | → `ciano/900` | FRAME_FILL, SHAPE_FILL |
| `ambiente/dev/texto` | `6:88` | → `ciano/400` | TEXT_FILL |
| `ambiente/dev/borda` | `6:89` | → `ciano/500` | STROKE_COLOR |
| `ambiente/stage/fundo` | `6:90` | → `ambar/900` | FRAME_FILL, SHAPE_FILL |
| `ambiente/stage/texto` | `6:91` | → `ambar/400` | TEXT_FILL |
| `ambiente/stage/borda` | `6:92` | → `ambar/500` | STROKE_COLOR |
| `ambiente/prod/fundo` | `6:93` | → `vermelho/900` | FRAME_FILL, SHAPE_FILL |
| `ambiente/prod/texto` | `6:94` | → `vermelho/400` | TEXT_FILL |
| `ambiente/prod/borda` | `6:95` | → `vermelho/500` | STROKE_COLOR |
| `ambiente/prod/destaque` | `6:96` | → `vermelho/600` | FRAME_FILL, SHAPE_FILL |
| `ambiente/prod/sobre` | `6:97` | → `neutro/0` | TEXT_FILL |
| `icone/padrao` | `8:2` | → `neutro/400` | STROKE_COLOR |
| `icone/forte` | `8:3` | → `neutro/50` | STROKE_COLOR |
| `icone/desabilitado` | `8:4` | → `neutro/600` | STROKE_COLOR |
| `icone/marca` | `8:5` | → `azul/300` | STROKE_COLOR |
| `icone/sobre-cor` | `8:6` | → `neutro/0` | STROKE_COLOR |
| `borda/sutil` | `VariableID:24` | → `neutro/800` | STROKE_COLOR |
| `sombra/suave` | `VariableID:25` | → `sombra/preto-24` | EFFECT_COLOR |
| `primaria/brilho` | `VariableID:38` | → `azul/500-40` | EFFECT_COLOR — Fase 4c |
| `perigo/translucido` | `VariableID:39` | → `vermelho/500-16` | FRAME_FILL, SHAPE_FILL — Fase 4c |
| `perigo/translucido-hover` | `VariableID:40` | → `vermelho/500-24` | FRAME_FILL, SHAPE_FILL — Fase 4c |
| `perigo/borda` | `VariableID:41` | → `vermelho/500` | STROKE_COLOR — Fase 4c |
| `marca/gradiente-inicio` | `VariableID:49` | → `azul/400` | FRAME_FILL, SHAPE_FILL — Fase 4g |
| `marca/gradiente-fim` | `VariableID:50` | → `azul/600` | FRAME_FILL, SHAPE_FILL — Fase 4g |
| `marca/simbolo` | `VariableID:51` | → `primaria/padrao` | FRAME_FILL, SHAPE_FILL — Fase 4g |
| `marca/sobre-cor` | `VariableID:52` | → `neutro/0` | FRAME_FILL, SHAPE_FILL, TEXT_FILL — Fase 4g |
| `ambiente/dev/tint` | `VariableID:58` | → `ciano/400-16` | FRAME_FILL, SHAPE_FILL — Fase 4e |
| `ambiente/stage/tint` | `VariableID:59` | → `ambar/400-16` | FRAME_FILL, SHAPE_FILL — Fase 4e |
| `ambiente/prod/tint` | `VariableID:60` | → `vermelho/400-16` | FRAME_FILL, SHAPE_FILL — Fase 4e |
| `selo/neutro/fundo` | `VariableID:61` | → `neutro/400-12` | FRAME_FILL, SHAPE_FILL — Fase 4e |
| `grupo/1/cor` | `VariableID:72` | → `azul/400` | FRAME_FILL, SHAPE_FILL, TEXT_FILL, STROKE_COLOR — Fase 4f: marcador do grupo |
| `grupo/2/cor` | `VariableID:73` | → `violeta/400` | idem — Fase 4f |
| `grupo/3/cor` | `VariableID:74` | → `verde/400` | idem — Fase 4f |
| `grupo/4/cor` | `VariableID:75` | → `rosa/400` | idem — Fase 4f |
| `grupo/5/cor` | `VariableID:76` | → `laranja/400` | idem — Fase 4f |

### `Tipografia` — `6:98` (modos: `Valor` `6:3`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `familia/texto` | `6:99` | Inter | FONT_FAMILY |
| `familia/mono` | `6:100` | JetBrains Mono | FONT_FAMILY |
| `estilo/regular` | `6:101` | Regular | FONT_STYLE |
| `estilo/medio` | `6:102` | Medium | FONT_STYLE |
| `estilo/semibold` | `6:103` | Semi Bold | FONT_STYLE |
| `estilo/negrito` | `6:104` | Bold | FONT_STYLE |
| `peso/regular` | `6:105` | 400 | — (oculta) |
| `peso/medio` | `6:106` | 500 | — (oculta) |
| `peso/semibold` | `6:107` | 600 | — (oculta) |
| `peso/negrito` | `6:108` | 700 | — (oculta) |
| `tamanho/xs` | `6:109` | 12 | FONT_SIZE |
| `altura-linha/xs` | `6:110` | 16 | LINE_HEIGHT |
| `tamanho/sm` | `6:111` | 13 | FONT_SIZE |
| `altura-linha/sm` | `6:112` | 18 | LINE_HEIGHT |
| `tamanho/base` | `6:113` | 14 | FONT_SIZE |
| `altura-linha/base` | `6:114` | 20 | LINE_HEIGHT |
| `tamanho/md` | `6:115` | 16 | FONT_SIZE |
| `altura-linha/md` | `6:116` | 24 | LINE_HEIGHT |
| `tamanho/lg` | `6:117` | 18 | FONT_SIZE |
| `altura-linha/lg` | `6:118` | 26 | LINE_HEIGHT |
| `tamanho/xl` | `6:119` | 22 | FONT_SIZE |
| `altura-linha/xl` | `6:120` | 30 | LINE_HEIGHT |
| `tamanho/2xl` | `6:121` | 28 | FONT_SIZE |
| `altura-linha/2xl` | `6:122` | 36 | LINE_HEIGHT |
| `espacamento-letra/normal` | `6:123` | 0 | LETTER_SPACING |
| `espacamento-letra/titulo` | `6:124` | -0.2 | LETTER_SPACING |
| `tamanho/3xl` | `VariableID:21` | 32 | FONT_SIZE |
| `tamanho/2xs` | `VariableID:62` | 11 | FONT_SIZE — Fase 4e |
| `altura-linha/3xl` | `VariableID:22` | 40 | LINE_HEIGHT |

### `Espaçamento` — `6:126` (modos: `Valor` `6:4`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `espaco/0` | `6:127` | 0 | GAP |
| `espaco/2` | `6:128` | 2 | GAP |
| `espaco/4` | `6:129` | 4 | GAP |
| `espaco/6` | `6:130` | 6 | GAP |
| `espaco/8` | `6:131` | 8 | GAP |
| `espaco/12` | `6:132` | 12 | GAP |
| `espaco/16` | `6:133` | 16 | GAP |
| `espaco/20` | `6:134` | 20 | GAP |
| `espaco/24` | `6:135` | 24 | GAP |
| `espaco/32` | `6:136` | 32 | GAP |
| `espaco/40` | `6:137` | 40 | GAP |
| `espaco/48` | `6:138` | 48 | GAP |
| `espaco/64` | `6:139` | 64 | GAP |
| `controle/sm` | `6:140` | 32 | WIDTH_HEIGHT |
| `controle/md` | `6:141` | 40 | WIDTH_HEIGHT |
| `controle/lg` | `6:142` | 48 | WIDTH_HEIGHT |
| `icone/sm` | `6:143` | 16 | WIDTH_HEIGHT |
| `icone/md` | `6:144` | 20 | WIDTH_HEIGHT |
| `icone/lg` | `6:145` | 24 | WIDTH_HEIGHT |
| `linha/fina` | `6:146` | 1 | STROKE_FLOAT |
| `linha/grossa` | `6:147` | 2 | STROKE_FLOAT |
| `linha/icone` | `9:2` | 1.5 | STROKE_FLOAT |
| `espaco/80` | `VariableID:19` | 80 | GAP |
| `espaco/96` | `VariableID:20` | 96 | GAP |
| `icone/xs` | `VariableID:63` | 12 | WIDTH_HEIGHT — Fase 4e |
| `controle/padrao` | `VariableID:47` | 44 | WIDTH_HEIGHT — Fase 4d: altura comum de `Botão`, `Campo`, `Campo secreto` e `Select` |

### `Raio` — `6:148` (modos: `Valor` `6:5`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `raio/nenhum` | `6:149` | 0 | CORNER_RADIUS |
| `raio/sm` | `6:150` | 6 | CORNER_RADIUS |
| `raio/md` | `6:151` | 10 | CORNER_RADIUS |
| `raio/lg` | `6:152` | 12 | CORNER_RADIUS |
| `raio/xl` | `6:153` | 16 | CORNER_RADIUS |
| `raio/2xl` | `6:154` | 20 | CORNER_RADIUS |
| `raio/total` | `6:155` | 9999 | CORNER_RADIUS |
| `raio/3xl` | `VariableID:12` | 24 | CORNER_RADIUS |
| `raio/controle` | `VariableID:13` | → `raio/md` | CORNER_RADIUS |
| `raio/item` | `VariableID:14` | → `raio/lg` | CORNER_RADIUS |
| `raio/cartao` | `VariableID:15` | → `raio/xl` | CORNER_RADIUS |
| `raio/painel` | `VariableID:16` | → `raio/2xl` | CORNER_RADIUS |
| `raio/janela` | `VariableID:17` | → `raio/3xl` | CORNER_RADIUS |
| `raio/pilula` | `VariableID:18` | → `raio/total` | CORNER_RADIUS |

### `Largura` — `VariableCollectionId:1` (modos: `Valor` `VariableCollectionId:1/mode1`)

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `largura/janela` | `VariableID:1` | 1280 | WIDTH_HEIGHT |
| `largura/barra-lateral` | `VariableID:2` | 264 | WIDTH_HEIGHT |
| `largura/conteudo` | `VariableID:3` | 960 | WIDTH_HEIGHT |
| `largura/formulario` | `VariableID:4` | 640 | WIDTH_HEIGHT |
| `largura/campo` | `VariableID:5` | 240 (era 320; Fase 4f: largura mínima de `Campo` e `Campo secreto`) | WIDTH_HEIGHT |
| `largura/select-filtro` | `VariableID:7` | 160 (era 200; Fase 4f: largura mínima do `Select`) | WIDTH_HEIGHT |
| `largura/dialogo` | `VariableID:8` | 480 | WIDTH_HEIGHT |
| `largura/dialogo-largo` | `VariableID:9` | 560 | WIDTH_HEIGHT |
| `largura/toast` | `VariableID:10` | 360 | WIDTH_HEIGHT |
| `largura/popup` | `VariableID:11` | 360 | WIDTH_HEIGHT |
| `largura/coluna-tipo` | `VariableID:44` | 120 | WIDTH_HEIGHT — Fase 4d |
| `largura/coluna-ambiente` | `VariableID:45` | 72 | WIDTH_HEIGHT — Fase 4d |
| `largura/coluna-nivel` | `VariableID:46` | 128 | WIDTH_HEIGHT — Fase 4d |
| `largura/coluna-acesso` | `VariableID:53` | 440 | WIDTH_HEIGHT — Fase 4e: coluna do formulário nas telas de acesso |
| `largura/coluna-lista` | `VariableID:82` | 392 | WIDTH_HEIGHT — Fase 4f: painel da lista em Credenciais |
| `largura/busca` | `VariableID:83` | 360 | WIDTH_HEIGHT — Fase 4f: busca em pílula no topo de Credenciais |

### `Elevação` — `VariableCollectionId:2` (modos: `Valor`) — Fase 4b

Medidas das sombras (D23). Code syntax WEB `var(--elevacao-…)`.

| Variable | Id | Valor | Escopos |
| --- | --- | --- | --- |
| `elevacao/nenhum` | `VariableID:26` | 0 | EFFECT_FLOAT |
| `elevacao/flutuante/contato-y` | `VariableID:27` | 2 | EFFECT_FLOAT |
| `elevacao/flutuante/contato-desfoque` | `VariableID:28` | 6 | EFFECT_FLOAT |
| `elevacao/flutuante/ambiente-y` | `VariableID:29` | 12 | EFFECT_FLOAT |
| `elevacao/flutuante/ambiente-desfoque` | `VariableID:30` | 32 | EFFECT_FLOAT |
| `elevacao/modal/contato-y` | `VariableID:31` | 4 | EFFECT_FLOAT |
| `elevacao/modal/contato-desfoque` | `VariableID:32` | 12 | EFFECT_FLOAT |
| `elevacao/modal/ambiente-y` | `VariableID:33` | 24 | EFFECT_FLOAT |
| `elevacao/modal/ambiente-desfoque` | `VariableID:34` | 64 | EFFECT_FLOAT |
| `elevacao/brilho/y` | `VariableID:42` | 4 | EFFECT_FLOAT — Fase 4c |
| `elevacao/brilho/desfoque` | `VariableID:43` | 16 | EFFECT_FLOAT — Fase 4c |

## Telas de acesso no Layout (Fase 4e)

Página Layout (`2796`, modo `Escuro` fixado na página; os frames herdam). Cada tela tem
1280×800 e a mesma estrutura: janela (`fundo/padrao`, borda `borda/sutil`, raio
`raio/janela`, padding e gap `espaco/16`) com a instância do `Painel de marca` (altura
`fill`) e o painel `conteúdo` (`fundo/sutil`, borda `borda/sutil`, raio `raio/painel`,
padding `espaco/48`) com a coluna `formulário` centrada (largura `largura/coluna-acesso`,
gap `espaco/32`, ou `espaco/24` nas telas com mais conteúdo). Grade: linha y = 0
(primeiro uso) e y = 1000 (destravar), colunas x = 0, 1360, 2720.

| Frame | Id | Painel de marca | `conteúdo` | `formulário` | Instâncias principais |
| --- | --- | --- | --- | --- | --- |
| `Primeiro uso / Criar cofre` | `3537` | `3538` | `3562` | `3563` | `Indicador de passos` 1 `3588`; cartão `o que acontece` `3583` (itens `3594` `3602` `3610`, ícones cadeado/servidor/relógio); `Botão` primário "Criar cofre" `3579` |
| `Primeiro uso / Senha mestra` | `3619` | `3620` | `3644` | `3645` | `Indicador de passos` 2 `3706`; `Campo senha` foco `3679` (10b); `força` `3720`; `Campo senha` padrão (confirmação) `3712` (10b); aviso = `Destaque` aviso `18716` (Fase 10c; era o frame `3701`); `ações` `3692` (`Botão` secundário "Voltar" `3693`, primário "Continuar" `3697`) |
| `Primeiro uso / PIN` | `3733` | `3734` | `3758` | `3759` | `Indicador de passos` 3 `3760`; `Campo PIN` completo `3821` e digitando `3831`; destaque = `Destaque` info `18721` (Fase 10c; era `3806`); `ações` `3811` ("Voltar" `3812`, "Concluir" `3816`) |
| `Destravar / Padrão` | `3342` | `3343` | `3367` | `3368` | selo `3389`; `Campo senha` vazio `3372` (10b); `Botão` "Destravar" `3380`; nota = `Destaque` neutro `18701` (Fase 10c; era `3384`) |
| `Destravar / Senha errada` | `3393` | `3394` | `3418` | `3419` | selo `3421`; `Campo senha` erro `3427` (10b); `Botão` desabilitado "Tente de novo em 4 s" `3435`; nota = `Destaque` neutro `18706` (Fase 10c; era `3439`) |
| `Destravar / Pedido do agente` | `3450` | `3451` | `3475` | `3476` | cartão `pedido do agente` `3501` (contagem `3506`, `Badge tipo` banco `3520`, `Badge ambiente` stage `3525`, `comando` `3504`); `Campo senha` foco `3484` (10b); `ações` `3527` ("Agora não" `3528`, "Destravar" `3492`); nota = `Destaque` neutro `18711` (Fase 10c; era `3496`) |

## Fase 4f — campos que esticam, componentes novos e telas de Credenciais e Nível de acesso

### Antes das telas: `Campo`, `Campo secreto` e `Select` esticam

- Variantes `332` `340` `348` `356` `364` (Campo), `376` `390` `405` (Campo secreto) e
  `422` `428` `434` `453` (Select): largura **desligada** de `largura/campo`/`largura/select`,
  largura mínima ligada a `largura/campo` (240) e `largura/select-filtro` (160); altura
  `hug`. Filhos `entrada` (`334` `342` `350` `358` `366`, `378` `392` `407`, `424` `430`
  `436` `455`), `ajuda` (`339` `347` `355` `363` `371`, `389` `404` `417`) e menu `440` em
  `fill`. Na tela, a instância usa `sizingH: fill`. A largura 320 do mestre é só o tamanho
  de apresentação (sem token).
- Telas da 4e: as instâncias `3372` `3427` `3484` `3679` `3712` perderam a ligação a
  `largura/coluna-acesso` e agora preenchem a coluna `formulário` (que continua ligada a
  `largura/coluna-acesso`).

### Componentes

| Componente | Id | Notas |
| --- | --- | --- |
| `Item de lista` › Densidade=compacta | `4002` `4028` `4045` | Propriedade de variante nova `Densidade` (larga, compacta) no set `553`. Compacta: largura 360 (use `fill`), gap `espaco/12`, padding `espaco/16`; `textos` (`4007`, gap `espaco/4`) com `linha 1` (`4026`: nome + `Badge ambiente`) e `linha 2` (`4027`: `Badge tipo` + detalhe), gap `espaco/8`; 76 de altura; selecionado com borda `borda/foco` fora do layout (`strokesIncluded` falso) para não mudar a altura. Sem nível e sem chevron (o nível fica no painel de detalhe) |
| `Cabeçalho de grupo` | `901` | Propriedade `Cliente` renomeada para `Grupo`; camada `cliente` → `grupo` (`902`); `marcador` novo (`4062`, 8×8 `espaco/8`, `grupo/1/cor`; troque a ligação na instância para `grupo/2..5/cor`) |
| `Chip de filtro` (set) | `5164` | `904` virou `Estado=ativo`. Sem uso nas telas: os filtros foram para o `Diálogo de filtros`. **Apagado na 10a** |
| `Ícone/filtro` | `5197` | Três linhas decrescentes, vetor `5200` ligado a `icone/padrao` e `linha/icone` |
| `Diálogo de filtros` | `5222` | Em `Seção / Filtros` (`5219`, título `5220`, descrição `5221`). Largura `largura/dialogo`, raio `raio/janela`, padding `espaco/24`, gap `espaco/20`, fundo `superficie/elevada`, borda `borda/padrao`, `Sombra/Modal`. Cabeçalho `5223` (título `5243` `Título/Modal`, fechar `5244`); `Select` Grupo `5224`, Tipo `5230`, Ambiente `5236`; rodapé `5242` (`Botão` fantasma "Limpar filtros" `5246`, espaço `5250`, "Cancelar" `5251`, "Aplicar" `5255`) |

Removidos na mesma fase (decisão do usuário: grupo não fica na navegação, é só filtro):
`Seção / Navegação por grupo` (`3908`) com `Cabeçalho de seção` (`3916`) e `Item de grupo`
(`3925`), as instâncias que tinham entrado na `Navegação lateral` (`3942`–`3998`) e os
tints `grupo/*/tint` com as primitivas `*-16`. Os frames `espaço` da navegação (`813`
`3067` `3107`) voltaram ao que eram.

### Telas no Layout (página `2796`, modo `Escuro` herdado da página)

Grade: linha y = 2000 (Credenciais), linha y = 3000 (Nível de acesso); colunas x = 0, 1360,
2720, 4080. Estrutura de todas: janela 1280×800 (`fundo/padrao`, borda `borda/sutil`, raio
`raio/janela`, padding e gap `espaco/16`, largura `largura/janela`) com `Navegação lateral`
Credenciais (altura `fill`) e `principal` (gap `espaco/16`): `topo` (título `Título/Página`,
resumo, `busca` = `Campo` em pílula sem rótulo, largura `largura/busca`, e `Botão`
secundário `filtrar` com `Ícone/filtro`) e `colunas`: `lista` (largura
`largura/coluna-lista`, `fundo/sutil`, raio `raio/painel`, padding `espaco/16`) e `detalhe`
(`fill`; `cabeçalho` com borda de baixo, `corpo`, `rodapé` com borda de cima).

| Frame | Id | Principais |
| --- | --- | --- |
| `Credenciais / Vazia` | `4780` | busca e `Filtrar` desabilitados (`4846`, `5209`); lista vazia `5133`; estado vazio `5132` (avatar com `Ícone/chave`, título, texto, nota do nível com aprovação = `Destaque` neutro `18726` (Fase 10c; era `5137`)); `Nova credencial` `4999` |
| `Credenciais / Cheia` | `4063` | nav `4064`; topo `4125` (resumo `4137` "21 entradas · 4 grupos" desde a 37a, busca `4128`, filtrar `5201`); lista `4138` (itens `4141`: `Cabeçalho de grupo` `4161` + 7 `Item de lista` compactos `4165` `4182` (selecionado) `4199` `4216` `4233` `4250` `4267`; `Nova credencial` `4142`); detalhe `4139` de `acme-pg-dev` (cabeçalho `4146`: avatar `4290`, nome `4299`, selos `4300`, `Editar` `4292`; corpo `4147`: Host `4337` e Usuário `4376` com Copiar, Porta `4350`, Base `4363`, `Campo secreto` oculto `4311`, cartão do nível = instância `19026` do `Cartão de nível` livre desde a 37a (era o frame `4336`); rodapé `4148`: metadados `4400`, Duplicar `4401`, Excluir `4405`) |
| `Credenciais / Filtrada` | `4410` | busca foco "pg" `4476`; `Filtros · 2` `5205`; linha de resumo `5213` ("Tipo banco · ambiente stage · busca pg" + `Limpar` `5215`); grupo `4500` "2 de 13 entradas" (37a); `acme-pg-aprovacao` selecionado `4504`, `acme-pg-tunel` `4555`; detalhe com nível "Com aprovação" (instância `19038` desde a 37a) |
| `Credenciais / Filtros` | `5259` | Cheia sob `sobreposição` `5577` (`fundo/sobreposicao`) com `Diálogo de filtros` `5578` |
| `Nível de acesso / Escolher` | `5615` | lista com `acme-pg-prod` selecionado `5714`; `sobreposição` `5934`; `Diálogo de nível` Escolher `5935` |
| `Nível de acesso / PIN` | `5994` | `sobreposição` `6313`; `Diálogo de nível` PIN `6314` |
| `Nível de acesso / PIN errado` | `6373` | `sobreposição` `6692`; `Diálogo de nível` PIN errado `6693` |

## Fase 5 — formulários, parte 1 (consolidado na 10c)

Seção `Seção / Formulário` `6796` (título `6797`, descrição `6798`), Design System, x 640, y 8124.

| Componente | Id | Variantes / filhos |
| --- | --- | --- |
| `Interruptor` (set) | `6838` | `Ligado=sim` `6836` (botão `6837`), `Ligado=não` `6839` (botão `6840`); 36×20, `largura/interruptor` |
| `Título de seção` | `6841` | textos `6842` (título `6847`, descrição `6848`), `Botão` fantasma `6843`; props `Título`, `Descrição`, `Mostrar descrição`, `Mostrar ação` |
| `Anexo` | `6849` | avatar `6850`, textos `6851` (nome `6859`, detalhe `6860`), `Ação de campo` Remover `6852`; props `Nome`, `Detalhe` |
| `Campo extra` (set) | `6878` | `Segredo=não` `6861`, `Segredo=sim` `6879` (máscara fixa `6893`); props `Nome`, `Valor (só se não for segredo)` (renomeada na 10c; era `Valor`), `Variável` |
| `Código TOTP` (set) | `6909` | `Estado=ativo` `6894` (código `6898`, contagem `6899`, ajuda `6897`), `Estado=vazio` `6910` (texto `6925`, ajuda `6924`) |

Variable: `largura/interruptor` `VariableID:84` = 36 (`Largura`).

Frames no Layout (Novo em y 4000, Edição em y 5600; colunas x 0 login, 1360 banco, 2720 servidor, 4080 api, 5440 aws):

| Frame | Id | Detalhe | Corpo | Notas |
| --- | --- | --- | --- | --- |
| `Formulário / login / Novo` | `7450` | `7648` | `7669` | `Select` foco `7902` + `Select` combobox `7691` (absoluto; era `aberto` até a 10c); senha `7765` e semente `7791` em `Campo secreto` vazio (10c) |
| `Formulário / login / Edição` | `6926` | `7120` | `7141` | `Código TOTP` `7421`; `Campo extra` `7278`; nível `7221` |
| `Formulário / banco / Novo` | `8537` | `8731` | `8752` | senha `8882` vazio (10c) |
| `Formulário / banco / Edição` | `7908` | `8106` | `8127` | túnel `8475`; anexo `8507` |
| `Formulário / servidor / Novo` | `9900` | `10094` | `10119` | `Conectar` oculto `10113`; senha `10236` vazio (10c) |
| `Formulário / servidor / Edição` | `8981` | `9175` | `9196` | `Conectar` `9192`; anexo `9387`; extra `9365`; chave `9326` (Ajuda por propriedade) |
| `Formulário / api / Novo` | `11272` | `11456` | `11475` | token `11557` vazio (10c) |
| `Formulário / api / Edição` | `10389` | `10587` | `10608` | token `10857`; variável `10882` |
| `Formulário / aws / Novo` | `12170` | `12354` | `12373` | secret `12455` e session token `12487` vazios (10c) |
| `Formulário / aws / Edição` | `11705` | `11889` | `11908` | extra secreto `12036` |

10c: nos frames `7450` `8537` `7908` `9900` `8981` `11272` `10389` `12170` `11705`, a `Navegação lateral` e a `lista` passaram a `fill` na altura; os `Item de lista` ocultos das listas de api/aws foram apagados.

## Fase 6 — formulários, parte 2 (consolidado na 10c)

Seção `Seção / Nota segura` `12580` (Design System, x 4600, y 0; título `12581`, descrição `12582`).

| Nó | Id | Notas |
| --- | --- | --- |
| `Campo nota segura` (set) | `12613` | props `Rótulo`, `Ajuda`, `Resumo` |
| `Estado=oculto` | `12583` | rótulo `12584`, entrada `12585`, máscara `12587`, rodapé `12588` (resumo `12589`, ações `12594`: revelar `12595`, `Ação de campo` `12599`), ajuda `12586` |
| `Estado=vazio` | `12614` | placeholder `12617`, ações ocultas `12620` |

Frames (Novo em y 7200, Edição em y 8800; colunas x 0, 1360, 2720, 4080):

| Frame | Id | Principais |
| --- | --- | --- |
| `Formulário / smtp / Novo` | `13867` | corpo `14086`; Grupo foco `14103` + combobox `14115` ("be" → beta / Criar grupo, 10c); Host `14285`, Porta `14293`; Usuário `14168`, Segurança `14301`; Senha `14176` (vazio, 10c) |
| `Formulário / ftp/sftp / Novo` | `14743` | corpo `14962`; Protocolo `15236`; Host `15036`, Porta `15044`; Usuário `15053`, Autenticação `15061`; Chave privada `15067`, Passphrase `15211` (vazios, 10c) |
| `Formulário / nota segura / Novo` | `15399` | corpo `15618`; "Conteúdo" `15683`; `Campo nota segura` vazio `15828` |
| `Formulário / genérico / Novo` | `15845` | corpo `16064`; extras `16135`; `Campo extra` não `16290`, sim `16304`; anexos vazio `16153` |
| `Formulário / smtp / Edição` | `9454` | lista beta `9514` (selecionado `9614`); corpo `9673`; Host `9742`, Porta `10833`; Usuário `9756`, Segurança `10846`; Senha `9769`; extra `9843`; nível `9867` |
| `Formulário / ftp/sftp / Edição` | `10895` | lista beta `10955` (selecionado `10990`); corpo `11047`; Protocolo `11143`; Host `11116`, Porta `11658`; Usuário `11150`, Autenticação `11652`; Senha `11163`; extra `11196` |
| `Formulário / nota segura / Edição` | `12683` | lista gama `12743` (selecionado `12793`); corpo `12835`; `Campo nota segura` oculto `13122` |
| `Formulário / genérico / Edição` | `13459` | selecionado `13541`; corpo `13600`; extras `13739` (REGIAO), `13753` (CHAVE_LICENCA); `Anexo` `13767` |

## Fase 7 — janela de aprovação (consolidado na 10c)

Seção `Seção / Aprovação` `16319` (Design System, x 8000, y 0; título `16326`, descrição `16327`, linha `16328`).

| Componente | Id | Notas |
| --- | --- | --- |
| `Contagem` (set) | `16348` | `Estado=normal` `16335` (ícone `16336`, tempo `16339`; modo `Tom do ícone` = aviso desde a 10c), `Estado=urgente` `16343` (ícone `16344`, tempo `16347`; modo perigo); prop `Tempo` |
| `Caixa de código` | `16340` | rótulo `16341`, conteúdo `16342`; props `Rótulo`, `Conteúdo` |
| `Botão ícone` — novo na Fase 10 | `18832` | 32×32 (`controle/sm`), raio `raio/md` (`VariableID:13`), layout centrado; `ícone` `18833` (`Ícone/fechar`, `icone/md`); prop `Ícone` (troca, padrão `191`). Usado no fechar das 5 telas de aprovação |

Frames (linha y 4000). Fechar = instância do `Botão ícone` desde a Fase 10: `18835` (Normal), `18838` (prod), `18841` (PIN errado), `18844` (PIN bloqueado), `18847` (Expirando); antes eram frames soltos.

| Frame | Id | x | Cabeçalho | `Contagem` | `pedido` | Comando | Quem pediu | `Campo PIN` | Botões (Negar, uma vez, 1 hora, sempre) | Nota (`Destaque` neutro, 10c) | Destaque (10c) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `Aprovação / Normal` | `16701` | 7200 | `16702` | `16709` | `16704` | `16751` | `16754` | `16715` | `16757` `16761` `16765` `16769` | `18731` | — |
| `Aprovação / prod` | `16810` | 7840 | `16811` | `16817` | `16826` | `16845` | `16848` | `16851` | `16863` `16867` `16872` `16876` | `18736` | prod `18756` |
| `Aprovação / PIN errado` | `16885` | 8480 | `16886` | `16892` | `16901` | `16920` | `16923` | `16926` | `16938` `16942` `16947` `16951` | `18741` | — |
| `Aprovação / PIN bloqueado` | `16960` | 9120 | `16961` | `16967` (oculta) | `16976` | `16995` | `16998` | `17001` | `17013` `17017` `17022` `17026` | `18746` | perigo `18760` |
| `Aprovação / Expirando` | `17035` | 9760 | `17036` | `17042` | `17051` | `17070` | `17073` | `17076` | `17088` `17092` `17097` `17101` | `18751` | perigo `18765` |

## Fase 8 — gerador, configurações, restaurar e kit (consolidado na 10c)

Seção `Seção / Gerador e configurações` `9453` (Design System, x 5600, y 0; título `9890`, descrição `9891`).

| Item | Id | Variantes / filhos |
| --- | --- | --- |
| `Controle deslizante` (set) | `12604` | Posição 25% `9896` (preenchido `9898`, alça `9899`, restante `9897`), 50% `12605`, 75% `12609` |
| `Linha de opção` (set) | `10379` | interruptor `10368` (`10370`), botão `10380` (`10384`), select `10825` (`10829`) |
| `Item de versão` (set) | `11264` | padrão `11243`, selecionado `11265` |
| `Popover do gerador` | `11671` | cabeçalho `11672`, `Campo secreto` `11673`, tamanho `11698`, conjuntos `11699`, rodapé `11700` |
| `Diálogo de restauração` (set) | `13033` | versões `12631`; confirmar `13034` (conteúdo `13041`: `Item de versão` `13056`, **`Campo senha` foco `13088`** desde a 10c, aviso = `Destaque` aviso `18770` desde a 10c) |

Variables: `impressao/papel` `VariableID:85`, `impressao/texto` `86`, `impressao/texto-secundario` `87`, `impressao/linha` `88` (`Cores`); `pagina/a4-largura` `89` = 794, `pagina/a4-altura` `90` = 1123 (`Largura`).

| Frame | Id | Posição |
| --- | --- | --- |
| `Gerador de senha / Avulso` | `13141` | x 0, y 10400 |
| `Gerador de senha / No formulário` | `14307` | x 1360, y 10400 (1280×1392); Senha `14622`, `Popover do gerador` `15135`. Conferido na 10c: igual a `6926` fora o popover e o `Mostrar gerar` |
| `Configurações / Geral` | `15242` | x 2720, y 10400 |
| `Configurações / Backup externo ligado` | `16178` | x 4080, y 10400 |
| `Restaurar backup / Versões` | `16349` | x 0, y 11400 (diálogo `16458`) |
| `Restaurar backup / Confirmar` | `16512` | x 1360, y 11400 (diálogo `16621`; o `Campo senha` `16683` agora vem do componente, sem override) |
| `Kit de emergência / Impressão` | `16773` | x 2720, y 11400 (794×1123) |

## Fase 9 — bandeja e extensão (consolidado na 10c)

Seção `Seção / Bandeja e extensão` `17137` (Design System, x 11000, y 0; título `17242`, descrição `17243`, `tamanhos` `17244`).

| Nome | Id | Variantes / filhos |
| --- | --- | --- |
| `Ícone da bandeja` (set) | `17141` | `Estado=destravado, Cor=branco` `17138`; `Estado=travado, Cor=branco` `17142` (vetor `17144` em contorno desde a 10c); `Estado=destravado, Cor=grafite` `18826` (vetor `18828`, `marca/grafite`) e `Estado=travado, Cor=grafite` `18829` (vetor `18831`, contorno `marca/inativo`) — 10c |
| `Ícone da extensão` (set) | `17148` | destravado `17145`, travado `17149` (ligações de gradiente que sobravam removidas na 10c) |
| `Ícone/ward` | `17194` | símbolo `17195` (vetor `17196`, `tom/icone`) |
| `Item de menu` (set) | `17157` | padrão `17152`, hover `17158`, desabilitado `17163` (modo desabilitado) |
| `Menu da bandeja` (set) | `17201` | destravado `17168`, travado `17202` |
| `Cabeçalho do popup` (set) | `17234` | destravado `17227`, travado `17235` |

Variables: `marca/sobre-cor-inativo` `VariableID:91`, `marca/inativo` `92`, `largura/menu` `93`, `verde/400-16` `94`, `sucesso/tint` `95`.

| Frame | Id | Posição | Tamanho |
| --- | --- | --- | --- |
| `Bandeja / Destravado` | `17269` | 7200, 7200 | 480×300 |
| `Bandeja / Travado` | `17300` | 7760, 7200 | 480×300 (ícone `17304`) |
| `Extensão / Sugestões do domínio` | `17332` | 7200, 7600 | 360×494 |
| `Extensão / Salvar login novo` | `17414` | 7640, 7600 | **360×600** desde a 10c (corpo `17422` rola, `overflow` vertical; Senha `17450` = `Campo secreto` sem revelar nem copiar; Tipo `17463` e Ambiente `17475` lado a lado em `18669`) |
| `Extensão / Cofre travado` | `17392` | 8080, 7600 | 360×343 |
| `Extensão / Salvar login novo / Conteúdo inteiro` (Fase 10) | `18850` | 8520, 7600 | 360×806 (cópia de `17414` com o corpo `18858` sem rolagem, para mostrar o Grupo (`Select` `18926`) e o nível inicial (`18932`); documentação, não é um estado novo) |

Exports em `docs/design/icones/` (10c acrescentou `bandeja-*-grafite-{16,24,32}.png`/`.svg` e regravou `bandeja-travado-*`).

## Fase 9a — Nova credencial (consolidado na 10c)

Seção `Seção / Nova credencial` `17583` (Design System, x 14000, y 0; título `17584`, descrição `17585`).

| Nó | Id | Notas |
| --- | --- | --- |
| `Cartão de tipo` (set) | `17607` | props `Nome`, `Descrição`, `Ícone` |
| `Estado=padrão` | `17586` | avatar `17599` (ícone `17604`), textos `17600` |
| `Estado=hover` | `17608` | avatar `17609` |
| `Estado=selecionado` | `17616` | avatar `17617` (ícone `17618`); modo `Tom do ícone` = marca (10c) |

| Frame | Id | Principais |
| --- | --- | --- |
| `Nova credencial / Escolher tipo` | `17657` | x 7200, y 10400; lista `17716`; detalhe `17851`; grade `17975` (login `17976` … genérico `18040`); nota `18051` |
| `Nova credencial / Tipo escolhido` | `18055` | x 8560, y 10400; grade `18263` com login selecionado `18264` |

Flow de protótipo "Nova credencial" (início `4063`); 26 botões `Nova credencial` das outras telas navegam para `17657`.

## Fase 10b — `Campo senha` (consolidado na 10c)

Seção `Seção / Campo senha` `17499` (Design System, x 16000, y 0; título `17500`, descrição `17501`).

| Componente | Id | Variantes | Propriedades |
| --- | --- | --- | --- |
| `Campo senha` (set) | `17502` | vazio `17503`, padrão `17511`, foco `17524`, erro `17537`, desabilitado `17550`, revelado `17587` | `Rótulo`, `Placeholder`, `Ajuda`, `Mostrar ajuda`, `Mensagem de erro`, `Ícone à esquerda`, `Ícone`, `Mostrar revelar`, `Valor revelado`; sem copiar |

Instâncias nas telas: `3679` (foco) e `3712` (padrão) em `3619`; `3372` (vazio) em `3342`; `3427` (erro) em `3393`; `3484` (foco) em `3450`; `13088` (foco) no `Diálogo de restauração` › confirmar (10c).

## Fase 10c — refinamentos

### Coleção `Tom do ícone` (`VariableCollectionId:3`)

Uma variable, `tom/icone` (`VariableID:96`, code syntax `var(--tom-icone)`), com 8 modos que apontam para `Cores`: `padrão` → `icone/padrao`, `sobre-cor` → `icone/sobre-cor`, `forte` → `icone/forte`, `desabilitado` → `icone/desabilitado`, `marca` → `icone/marca`, `perigo` → `perigo/texto`, `aviso` → `aviso/texto`, `sucesso` → `sucesso/texto`. Todos os vetores dos 29 `Ícone/*` usam `tom/icone`. A cor do ícone vem do **modo fixado no componente ou na instância que o contém**, e não de override no vetor, então trocar o ícone pela propriedade mantém a cor.

| Onde | Modo |
| --- | --- |
| `Botão` primário padrão/hover/foco (`264` `268` `272`) | sobre-cor |
| `Botão` secundário e fantasma (`280` `284` `288` `312` `316` `320`) | forte |
| `Botão` perigo (`296` `300` `304`) | perigo |
| `Botão` desabilitado (`276` `292` `308` `324`), `Item de menu` desabilitado `17163` | desabilitado |
| `Item de navegação` hover `770` | forte |
| `Item de navegação` ativo `776`, `Cartão de tipo` selecionado `17616`, `Destaque` info `18679` | marca |
| `Ação de campo` padrão `2877` / copiado `2883` | forte / sucesso |
| `Contagem` normal `16335` / urgente `16343`, `Destaque` aviso `18684` / perigo `18689` | aviso / perigo |

Limitação do FigCraft: a instância copia o modo do componente só ao ser criada, e trocar a variante (`Tipo`, `Estado`) não atualiza o modo. Os 545 nós existentes (variantes e instâncias) foram acertados por script; numa instância nova ou com variante trocada, ajuste `variable.setMode` (`Tom do ícone`) no próprio nó. No código, isso é só `color: currentColor`.

### Componentes novos e alterados

| Componente | Id | Mudança |
| --- | --- | --- |
| `Destaque` (set) — novo | `18678` | Seção `Seção / Destaque` `18670` (x 18000, y 0; título `18671`, descrição `18672`). `Tipo=neutro` `18673` (`superficie/padrao`, borda `borda/sutil`, texto `texto/secundario`), `info` `18679` (`info/sutil`, `info/borda-suave`, ícone cadeado), `aviso` `18684` (`aviso/sutil`, `aviso/borda-suave`), `perigo` `18689` (`perigo/translucido`, `perigo/borda-suave`, ícone erro), `prod` `18694` (`ambiente/prod/tint`, `ambiente/prod/borda`, `Badge ambiente` prod grande `18699` no lugar do ícone). Padding `espaco/16` × `espaco/20`, gap `espaco/12`, `raio/cartao`, `linha/fina`, largura mínima `largura/campo`, texto `Texto/Pequeno`. Props `Texto`, `Ícone`, `Mostrar ícone` |
| `Campo secreto` | `375` | variante nova `Estado=vazio` `18609` (placeholder `18612` em `texto/secundario`, sem revelar nem copiar, gerar pela prop, ajuda fixa `18633`); props novas `Placeholder`, `Ajuda` (ligada à ajuda do oculto `389`), `Mostrar ajuda`, `Mostrar copiar` (`2888` `2893` `2898`), `Mostrar revelar` (`381` `395` `410`) |
| `Select` | `421` | variante nova `Estado=combobox` `18780`: `Ícone/busca` à esquerda, `texto digitado` `18800` (valor `18783` + cursor `18799`), menu com `opção encontrada` `18791` e `opção criar` `18795` (`Ícone/mais`) |
| `Toast` | `525` | borda `linha/fina` com `sucesso/borda-suave` (`526`), `perigo/borda-suave` (`534`), `aviso/borda-suave` (`542`) |
| `Campo extra` | `6878` | prop `Valor` → `Valor (só se não for segredo)` |
| `Diálogo de restauração` › confirmar | `13034` | `13088` virou `Campo senha` foco; aviso virou `Destaque` aviso `18770` |

### Variables novas (12)

| Variable | Id | Valor |
| --- | --- | --- |
| `tom/icone` | `VariableID:96` | coleção `Tom do ícone` (acima) |
| `azul/400-32`, `ambar/400-32`, `vermelho/400-32`, `verde/400-32` | `97` `98` `99` `100` | primitivas ocultas, 32% de opacidade |
| `info/borda-suave`, `aviso/borda-suave`, `perigo/borda-suave`, `sucesso/borda-suave` | `101` `102` `103` `104` | `Cores`, STROKE_COLOR, → as `*-32` |
| `info/sutil` | `105` | → `primaria/sutil` |
| `vermelho/300` | `106` | #F89090 (primitiva oculta); `ambiente/prod/texto` agora aponta para ela |
| `marca/grafite` | `107` | → `neutro/800` (ícone da bandeja para painéis claros) |

### Contraste do `Badge ambiente` (WCAG, texto 11 px sobre o tint composto)

| Fundo sob o selo | dev | stage | prod antes | prod depois (`vermelho/300`) |
| --- | --- | --- | --- | --- |
| `fundo/padrao` | 6,6 | 7,6 | 5,2 | 6,8 |
| `superficie/padrao` | 5,6 | 6,5 | 4,4 | 5,9 |
| `superficie/elevada` | 5,0 | 5,8 | 4,0 | 5,3 |
| `superficie/selecionada` | 4,9 | 5,8 | 4,0 | 5,3 |

Prod grande sólido: 5,1. Todos passam AA (4,5) depois da troca.

## Fase 10a — limpeza de tokens e componentes sem uso

Uso levantado por script no `document.json` do arquivo salvo: ligação em node (`bound`), em
estilo (inclusive efeitos), alias em qualquer coleção (inclusive os 8 modos de `Tom do ícone`)
e modo fixado (`var_modes`). Os 18 estilos e os 8 modos de `Tom do ícone` estão todos em uso.

| Removido | Id | Motivo |
| --- | --- | --- |
| `sombra/padrao` (`Cores`) | `8:7` | sem uso desde a 4b; `sobreposicao/escuro` (`6:53`) continua em uso por `fundo/sobreposicao` |
| `largura/select` (`Largura`) | `VariableID:6` | sem uso desde a 4f (`Select` usa `largura/campo` e `largura/select-filtro`) |
| `espacamento-letra/caixa-alta` (`Tipografia`) | `6:125` | sem uso: `Texto/Selo` e `Texto/Selo grande` usam `espacamento-letra/normal` e não estão mais em maiúsculas |
| `Chip de filtro` (set) | `5164` (variante `904`, filhos `905`–`907`) | zero instâncias; os filtros ficam no `Diálogo de filtros`. A coluna `900` da `Seção / App` ficou só com o `Cabeçalho de grupo` `901` |

`Primitivas`: as 45 estão em uso (alias em `Cores`), nenhuma removida. Ficaram, sem uso mas
fora da lista da 10a (escala ou conjunto que o código pode precisar): `peso/regular`,
`peso/medio`, `peso/semibold`, `peso/negrito` (`6:105`–`6:108`), `tamanho/2xl` (`6:121`),
`altura-linha/2xl` (`6:122`), `raio/nenhum` (`6:149`), `largura/conteudo` (`VariableID:3`),
`espaco/80` e `espaco/96` (`VariableID:19`, `20`), `grupo/4/cor` e `grupo/5/cor`
(`VariableID:75`, `76`). Componentes sem instância que ficaram (base do design system):
`Toast` `525`, `Modal` `611`, `Logo / Ícone do app` `3292`.


## Fase 10 — conferência final do M0

Conferido no arquivo salvo (cópia reaberta pela CLI headless) por `docs/mission/evidence/fase-10/v0-auditoria.json`.

| Mudança | Id | Motivo |
| --- | --- | --- |
| Componente `Botão ícone` (em `Seção / Aprovação`, linha `16328`) | `18832` (ícone `18833`) | o fechar das 5 telas de aprovação era um frame solto (V0.4); mesma medida, raio e ícone |
| Instâncias do fechar | `18835` `18838` `18841` `18844` `18847` | substituem os frames soltos apagados (16714, 16822, 16897, 16972, 17047) |
| Frame `Extensão / Salvar login novo / Conteúdo inteiro` | `18850` | o popup de 600 rola e esconde o Grupo; a cópia sem rolagem mostra tipo, grupo e ambiente juntos (V0.18) |
| Descrição da `Seção / Aprovação` | `16327` | cita o `Botão ícone` |

Ids citados aqui e ausentes no arquivo são só os já marcados como apagados ou substituídos
(`719` `723` `785` `789` `822` `861` `904`–`907` `3039` `3043` `3079` `3083` `3384` `3439`
`3496` `3701` `3806` `3908` `3916` `3925` `3942` `3998` `5137` `5164`).

## Fase 10d — ajustes do scrutiny e do QA do M0

Nenhum id existente mudou nem foi apagado. Novos:

| Novo | Id | Onde / uso |
| --- | --- | --- |
| Componente `Esmaecer rolagem` | `18948` | `Seção / Item de lista` (`550`); gradiente `fundo/sutil-transparente` → `fundo/sutil`, altura `espaco/48` (`6:138`) |
| Instâncias `esmaecer` (filho absoluto no fim de `itens`, constraints leftRight/bottom) | `18949` (`4141`, Credenciais / Cheia), `18950` (`17717`, Nova credencial / Escolher tipo), `18951` (`18115`, / Tipo escolhido), `18952` (`5675`, Nível de acesso / Escolher), `18953` (`6054`, / PIN), `18954` (`6433`, / PIN errado), `18955` (`5319`, Credenciais / Filtros) | listas cheias (12 entradas, 7 visíveis) |
| `neutro/900-0` (`Primitivas`) | `VariableID:108` | `neutro/900` com alfa 0, oculta dos pickers |
| `fundo/sutil-transparente` (`Cores`) | `VariableID:109` | → `neutro/900-0`; FRAME_FILL, SHAPE_FILL |
| `largura/select-compacto` (`Largura`) | `VariableID:110` | 120; `Select` (`10829`) da variante `Controle=select` (`10825`) da `Linha de opção` |

Alterados no lugar:

| O quê | Ids |
| --- | --- |
| Ajuda oculta do Role ARN (era a do `api`): "Vai em AWS_ROLE_ARN no ward run." | `12015` (aws / Edição), `12512` (aws / Novo) |
| `Mostrar copiar` ligado em Porta, Base e Nome da variável | detalhe: `4350` `4363` `5845` `5859` `6224` `6238` `6603` `6617` `5489` `5503` `4669` `4683`; formulários: `8481` `8494` `9286` `10833` `11658` `10882` |
| Altura do frame para o corpo caber acima do rodapé | `7450` 1404, `8537` 1454, `12170` 1478, `14743` 1470 |
| Máscara do gerador com 24 pontos (bate com "24 caracteres") | `13822` (Avulso), `15144` (No formulário) |
| Aprovação e pedido do agente usam a entrada da lista: `acme-pg-aprovacao` (prod: `acme-pg-prod`) | nome `16733` `16908` `16983` `17058` `3518` `16833`; comando `16753` `16922` `16997` `17072` `3510` `16847` |
| PIN errado com comprimento variável (máscaras ocultas) | Aprovação / PIN errado: 4 dígitos (`17118` `17119` ocultos); Nível de acesso / PIN errado: 5 dígitos (`6793` oculto) |
| Respiro entre "De … para …" e o PIN: `gap` do `corpo` → `espaco/20` | `1097` (Etapa=PIN errado), `1046` (Etapa=PIN) |
| Descrição da `Seção / Item de lista` cita o `Esmaecer rolagem` | `552` |

Posições na Design System depois da 10d (a `Seção / Item de lista` cresceu e já
sobrepunha a `Seção / Modal`): `608` y 3746, `647` y 4160, `716` y 4502, `760` y 5406,
`3286` y 7038, `5219` y 7860, `6796` y 8488 (x = 640 em todas).

## Fase 29 — tokens e componentes no código (só leitura do FigCraft)

Lidos de uma cópia do `Ward.figcraft` (o original não foi alterado), pelo MCP headless.

| O quê | Origem no FigCraft | No código |
| --- | --- | --- |
| Variables (todas as coleções) e estilos de texto/efeito | `variable.export css`, `variable.list`, `style.list` → `app/design/figcraft-export.jsonl` | `app/src/lib/estilo/tokens.css` (gerado por `python3 -I -B app/scripts/gerar_design.py`) |
| Ícones 20×20, traço 1.5 | frame `178` (filhos `179`–`257` e `5197`) → `app/design/icones/*.svg` | `app/src/lib/componentes/icones.js` (gerado) e `Icone.svelte` |
| Botão, Campo, Campo secreto, Select, Badge de ambiente/tipo, Toast, Item de lista (`553`), Modal, Campo de PIN | página Design System | `app/src/lib/componentes/*.svelte` (só tokens; conferência pixel a pixel por tela fica para as fases de UI) |

## Fase 32 — tela Credenciais no código (só leitura do FigCraft)

Lidos de uma cópia do `Ward.figcraft` (o original não foi alterado). Capturas do app
(ponte de teste, Chrome headless, 1280×800) e PNG dos frames em
`docs/mission/evidence/fase-32/`.

| Frame / componente | Id | No código |
| --- | --- | --- |
| `Credenciais / Vazia` | `4780` | `app/src/telas/Credenciais.svelte` (cofre sem entradas) |
| `Credenciais / Cheia` | `4063` | idem, com `acme-pg-dev` selecionada |
| `Credenciais / Filtrada` | `4410` | idem, tipo banco + ambiente stage + busca "pg" |
| `Credenciais / Filtros` | `5259` (diálogo `5578`) | `app/src/lib/componentes/DialogoFiltros.svelte` (componente `5222`) |
| `Navegação lateral` | set `782` (instância `4064`) | `app/src/lib/componentes/NavegacaoLateral.svelte` |
| `Cabeçalho de grupo` | `901` (instância `4161`) | `app/src/lib/componentes/CabecalhoDeGrupo.svelte` |
| `Item de lista` compacto | `4002`/`4028`/`4045` | `app/src/lib/componentes/ItemDeLista.svelte` (`compacta`) |
| `Destaque` | `18673` (instância `18726`) | `app/src/lib/componentes/Destaque.svelte` |
| `Esmaecer rolagem` | `18948` (instância `18949`) | `.esmaecer` em `Credenciais.svelte` |
| Busca em pílula | `4128` | `Campo.svelte` com `pilula`, `icone`, `rotuloOculto` |
## Fase 37 — gerador de senha avulso no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node`,
`screenshot` e `inspect.code` (HTML/CSS com as variables ligadas). Capturas em
`docs/mission/evidence/fase-37/` (a do FigCraft é `figcraft-13141-gerador-avulso.png`).

| Node no FigCraft | No código |
| --- | --- |
| `Gerador de senha / Avulso` `13141` (`principal` `13182`: `topo` `13183`, `colunas` `13199`) | `app/src/telas/GeradorSenha.svelte` |
| `Navegação lateral` `782`, variante `Ativo=Gerador de senha` `3037` (instância `13142`) | `app/src/lib/componentes/NavegacaoLateral.svelte` |
| `Linha de opção` `10379`, variante interruptor `10368`; `Interruptor` `6838` (ligado `6836`, desligado `6839`) | `app/src/lib/componentes/LinhaDeOpcao.svelte` |
| `Controle deslizante` `12604` (alça e preenchido de `9896`) | `app/src/lib/componentes/ControleDeslizante.svelte` |
| `Campo secreto` `375` (máscara de 24 pontos `13822`), `Ação de campo` `2882`, `Botão` `263` | componentes da Fase 29; o `Campo secreto` ganhou `mascara` (um ponto por caractere) |

## Fase 37a — correções do design (D65 f e design pendente da Fase 32)

Editado no `Ward.figcraft` original (cópia de segurança antes de editar, no scratchpad
da sessão), pelo MCP `figcraft` headless; salvo com `save_file` e conferido reaberto.
PNG em `docs/mission/evidence/fase-37a/`.

### Símbolos do gerador (D35)

O texto mostra o conjunto de `ward_core::gerador::SIMBOLOS`,
`!#$%&()*+,-./:;<=>?@[]^_{|}~` (28 caracteres, sem espaço, aspas, crase e `\`), com
`liga` e `calt` desligados no texto para a Inter não juntar `<=>` numa seta.

| Onde | Id | Texto |
| --- | --- | --- |
| `Gerador de senha / Avulso`, descrição da opção Símbolos | `13815` (rótulo `13814`) | `!#$%&()*+,-./:;<=>?@[]^_{\|}~` + quebra + "Desligue se o painel não aceitar." |
| `Popover do gerador` (componente), rótulo do interruptor | `12124` | "Símbolos" (era "Símbolos (!@#$%)") |
| `Popover do gerador`, texto novo `descrição símbolos` no fim de `conjuntos` `11699` | `18956` | "Símbolos: `!#$%&()*+,-./:;<=>?@[]^_{\|}~`" + quebra + "Desligue se o painel não aceitar." (estilo `8:14`, cor `6:65`) |
| `Gerador de senha / No formulário`, instância do popover `15135` | rótulo `15192`, descrição `18957` | herdados do componente |

### Componente novo: `Cartão de nível`

Seção `Seção / Cartão de nível` `18958` (Design System, x 19000, y 0; título `18959`,
descrição `18960`). Set `18998` (fundo `6:55`, gap `6:135`, padding `6:136`, raio
`VariableID:15`). Cada variante: frame horizontal (fundo `6:58`, borda `VariableID:24`,
raio `VariableID:15`, padding `6:134`, gap `6:133`), `ícone` 20 px (`6:144`, traço
`VariableID:96`), `textos` (rótulo "Nível de acesso" `8:16`, nível `8:13`, explicação
`8:14`) e `Botão` secundário "Mudar nível" com `Ícone/cadeado`. Nas telas, largura `fill`.

| Variante | Id | Ícone | Explicação |
| --- | --- | --- | --- |
| `Nível=livre` | `18986` | `Ícone/check` `189` | O Claude usa sem perguntar e só vê a saída mascarada. Mudar o nível pede o PIN. |
| `Nível=com aprovação` | `18999` | `Ícone/cadeado` `206` | Cada uso do Claude abre a janela de aprovação, que pede o seu PIN. Mudar o nível também pede o PIN. |
| `Nível=bloqueado` | `19011` | `Ícone/olho-fechado` `182` | O Claude nem vê esta entrada: ela some do ward lista. Só você usa, pelo app. Mudar o nível pede o PIN. |

Os cartões soltos do detalhe de Credenciais viraram instâncias (o frame antigo foi
apagado): `Credenciais / Cheia` `19026` (livre, era `4336`), `/ Filtrada` `19038` (com
aprovação, era `4734`), `/ Filtros` `19051` (livre, era `5554`), `Nível de acesso /
Escolher` `19063`, `/ PIN` `19076`, `/ PIN errado` `19089` (com aprovação; eram `5910`,
`6289`, `6668`). Os cartões dos formulários (textos próprios por tipo) continuam frames
soltos.

### Telas novas no Layout

| Frame | Id | Posição | Principais |
| --- | --- | --- | --- |
| `Credenciais / Sem resultado` | `19102` | x 5440, y 2000 | resumo `19147` "0 de 21 entradas · filtros ativos"; busca em foco "mysql" `19148` (valor `19154`); `Filtros · 2` `19161`; linha `filtros ativos` `19167` ("Tipo banco · ambiente prod · busca mysql" `19168` + `Limpar` `19169`); `itens` `19173` centralizado com "Nenhuma entrada com essa busca ou esses filtros." `19341`; `Nova credencial` `19212`; detalhe `19216` só com `corpo` `19342` › `vazio` `19343`: avatar `19344` com `Ícone/busca` `19356` (24, `6:145`), título "Nenhuma entrada encontrada" `19349`, texto "Limpe a busca ou os filtros para ver todas as entradas." `19350` |
| `Credenciais / Bloqueada` | `19359` | x 4080, y 3000 | resumo `19404` "3 de 21 entradas · filtros ativos"; busca "gama" `19405`; `Filtrar` `19418`; linha "Busca gama" `19425`; grupo `19598` "gama · 3 de 3 entradas"; `gama-bloqueada` selecionado `19602` (`4045`), `gama-generico` `19619`, `gama-nota` `19636`; detalhe `19473` de `gama-bloqueada` (avatar `Ícone/tipo-login` `19653`, nome `19480`, `Badge tipo` login `19482`, `Badge ambiente` prod `19487`, "grupo gama" `19489`; URL `19496` "https://bloqueada.invalid", Usuário `19523` "x", `Campo secreto` `19549`; **cartão `Nível=bloqueado` `19574`**; rodapé `19587`) |

### Contagens da fixture

`dev/fixtures/entradas.json` tem 21 entradas: acme 13, beta 4, gama 3, delta 1. Todos os
"20 entradas · 4 grupos" viraram "21 entradas · 4 grupos", "12 entradas" (acme) viraram
"13 entradas" (texto e propriedade `Contagem` do `Cabeçalho de grupo`), e a Filtrada
mostra "2 de 21 entradas · filtros ativos" (`4475`) e "2 de 13 entradas" (`4500`). Os
grupos beta (4) e gama (3) já batiam. São 27 frames: Credenciais Cheia, Filtrada e
Filtros, as três de Nível de acesso, os 18 formulários, `Gerador de senha / No
formulário` e as duas de `Nova credencial`. Quando a Fase 22a acrescentar a 22ª entrada, as contagens mudam de
novo.


## Fase 33 — formulário parte 1 no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas do app (ponte de teste com `--daemon-falso cheio`, Chrome headless,
1280×800) e `qa.json` em `docs/mission/evidence/fase-33/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Formulário / login / Novo` e `/ Edição` | `7450` (detalhe `7648`, corpo `7669`) e `6926` (`7120`, `7141`) | `app/src/telas/Formulario.svelte` + `formulario.js` (`TIPOS_FORM.login`) |
| `Formulário / api / Novo` e `/ Edição` | `11272` (corpo `11475`) e `10389` (`10608`) | idem (`TIPOS_FORM.api`) |
| `Formulário / aws / Novo` e `/ Edição` | `12170` (`12373`) e `11705` (`11908`) | idem (`TIPOS_FORM.aws`) |
| `Formulário / smtp / Novo` e `/ Edição` | `13867` (`14086`) e `9454` (`9673`) | idem (`TIPOS_FORM.smtp`) |
| `Formulário / nota segura / Novo` e `/ Edição` | `15399` (`15618`) e `12683` (`12835`) | idem; `Campo nota segura` (`15828` vazio, `13122` oculto) virou o `textarea` da tela e o `Campo secreto` editável |
| `Formulário / genérico / Novo` e `/ Edição` | `15845` (`16064`) e `13459` (`13600`) | idem (`TIPOS_FORM.generico`) |
| `Gerador de senha / No formulário` | `14307` (popover `15135`) | `app/src/lib/componentes/PopoverGerador.svelte` |
| `Nova credencial / Escolher tipo` e `/ Tipo escolhido` | `17657` (detalhe `17851`, grade `17975`) e `18055` | `app/src/telas/EscolherTipo.svelte` |
| `Título de seção` | `6841` | `app/src/lib/componentes/TituloDeSecao.svelte` |
| `Campo extra` (não `6861`, sim `6879`) | set `6878` | `app/src/lib/componentes/CampoExtra.svelte` (editável) |
| `Código TOTP` | `6909` | `app/src/lib/componentes/CodigoTotp.svelte` |
| `Popover do gerador` | `11671` | `PopoverGerador.svelte` |
| `Cartão de tipo` | `17607` | `app/src/lib/componentes/CartaoDeTipo.svelte` |
| Combobox de grupo | `7691` (smtp: `14115`) | `app/src/lib/componentes/ComboboxGrupo.svelte` |
| `Interruptor` compacto | `6838` | `app/src/lib/componentes/Interruptor.svelte` |
| `Campo secreto` vazio (10c) | `375` / `18609` | `CampoSecreto.svelte` ganhou `editavel`, `ontrocar`, `mostrarRevelar`, `mostrarCopiar` e `erro`; o revelar do detalhe de Credenciais usa `revelar_campo` (D65 b) |

## Fase 34 — formulário parte 2 no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado; sha256 igual antes e
depois), por `get_node` e `screenshot`. Capturas do app (ponte de teste com
`--daemon-falso cheio --anexos <pasta>`, Chrome headless, 1280×800) e `qa.json` em
`docs/mission/evidence/fase-34/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Formulário / banco / Novo` e `/ Edição` | `8537` (detalhe `8731`, corpo `8752`) e `7908` (`8106`, `8127`; túnel `8475`, anexo `8507`) | `app/src/telas/Formulario.svelte` + `formulario.js` (`TIPOS_FORM.banco`; texto de "Acesso" e porta padrão pelo motor) |
| `Formulário / servidor / Novo` e `/ Edição` | `9900` (`10094`, `10119`; `Conectar` oculto `10113`) e `8981` (`9175`, `9196`; `Conectar` `9192`, anexo `9387`, chave `9326`) | idem (`TIPOS_FORM.servidor`; "Autenticação" escolhe senha ou chave + passphrase; `Conectar` no cabeçalho só na edição) |
| `Formulário / ftp/sftp / Novo` e `/ Edição` | `14743` (corpo `14962`) e `10895` (corpo `11047`) | idem (`TIPOS_FORM['ftp-sftp']`; com `ftp`, só senha) |
| `Anexo` | `6849` (avatar `6850`, nome `6859`, detalhe `6860`, Remover `6852`) | `app/src/lib/componentes/Anexo.svelte` |
| `Botão` secundário com `Ícone/tipo-servidor` ("Conectar") | `9192` (main `280`, ícone `225`) | `Botao` com `icone="tipo-servidor"` |

## Fase 30 — primeiro uso no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado; sha256 igual antes e
depois), por `get_node` e `screenshot`. Capturas do app (ponte de teste com
`--daemon-falso sem-cofre`, Chrome headless, 1280×800, sistema em tema claro), as do
FigCraft (`figcraft-<id>-*.png`) e `qa.json` em `docs/mission/evidence/fase-30/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Primeiro uso / Criar cofre` | `3537` (`conteúdo` `3562`, `formulário` `3563`, cartão `3583`, itens `3594` `3602` `3610`) | `app/src/telas/PrimeiroUso.svelte` (passo 1) |
| `Primeiro uso / Senha mestra` | `3619` (`formulário` `3645`, `força` `3720`, aviso `18716`, `ações` `3692`) | idem (passo 2) + `primeiro-uso.js` (mínimo de 12, força com os limiares de D65 f) |
| `Primeiro uso / PIN` | `3733` (`formulário` `3759`, `Campo PIN` `3821`/`3831`, info `18721`, `ações` `3811`) | idem (passo 3) |
| `Painel de marca` | `717` (instância `3538`) | `app/src/lib/componentes/PainelDeMarca.svelte` |
| `Indicador de passos` | set `741` (`742`, `748`, `754`) | `app/src/lib/componentes/IndicadorDePassos.svelte` |
| `Campo senha` | foco `17524`, padrão `17511` | `app/src/lib/componentes/CampoSenha.svelte` |
| `Destaque` aviso e info | `18684` e `18679` | `Destaque.svelte` ganhou `tipo` (`neutro`, `aviso`, `info`) |
| `Campo PIN` | set `650` | `CampoPin.svelte` (já existia; o foco fica visível com os 6 dígitos cheios) |

## Fase 35 — nível de acesso com PIN no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas do app (ponte de teste com `--daemon-falso cheio`, Chrome
headless, 1280×800), PNG dos frames e `qa.json` em `docs/mission/evidence/fase-35/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Cartão de nível` (set; `Nível=livre`, `com aprovação`, `bloqueado`) | `18998` (`18986`, `18999`, `19011`) | `app/src/lib/componentes/CartaoDeNivel.svelte` (no detalhe de `Credenciais.svelte`, no lugar do cartão solto) |
| `Opção de nível` (set; Nível × Selecionado) | `909` (`910`/`920`, `931`/`940`, `950`/`958`) | `app/src/lib/componentes/OpcaoDeNivel.svelte` (`<input type="radio">`) |
| `Diálogo de nível` (set; Escolher, PIN, PIN errado) | `967` (`968`, `1027`, `1078`) | `app/src/lib/componentes/DialogoDeNivel.svelte` (`<dialog>` nativo, `data-etapa`) |
| `Campo PIN` (instâncias `1052` e `1103`) | `674` (completo), `690` (erro) | `CampoPin.svelte`: dígito cheio com `borda/padrao` e erro com `perigo/texto` |
| `Nível de acesso / Escolher` | `5615` (diálogo `5935`) | `nivel-escolher.png` |
| `Nível de acesso / PIN` | `5994` (diálogo `6314`) | `nivel-pin.png` |
| `Nível de acesso / PIN errado` | `6373` (diálogo `6693`) | `nivel-pin-errado.png` |

Textos dos níveis (cartão e opção) em `app/src/lib/componentes/niveis.js`.

## Fase 31 — destravar no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas do app (ponte de teste com `--daemon-falso travado-com-pedido`,
Chrome headless, 1280×800), PNG dos frames e `qa.json` em
`docs/mission/evidence/fase-31/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Painel de marca` (instância `3343`) | `717` | `app/src/lib/componentes/PainelDeMarca.svelte` (largura 560 = `largura/dialogo-largo`, sem variable própria) |
| `Campo senha` (set; vazio, foco, erro) | `17502` (`17503`, `17524`, `17537`) | `app/src/lib/componentes/CampoSenha.svelte` (erro com borda `perigo/texto`) |
| `Destravar / Padrão` | `3342` (`formulário` `3368`, selo `3389`, `Destaque` `18701`) | `app/src/telas/Destravar.svelte` › `destravar-padrao.png` |
| `Destravar / Senha errada` | `3393` (`Campo senha` erro `3427`, botão `3435`, `Destaque` `18706`) | `destravar-senha-errada.png` (campo esvaziado, foco nele) |
| `Destravar / Pedido do agente` | `3450` (`formulário` `3476`, cartão `3501`, contagem `3506`, `ações` `3527`, `Destaque` `18711`) | Fase 39a: `entrada` `3503` (avatar `3507` com `Ícone/chave`, nome `3518`) e `comando` `3504` = `CaixaDeCodigo.svelte`; no lugar dos `selos` `3519` (tipo e ambiente não existem com o cofre travado) vai o comando da operação (`ward run` etc.); `evidence/fase-39a/destravar-pedido-do-agente*.png` |

## Fase 38 — Configurações no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas do app (ponte de teste com `--daemon-falso cheio`, Chrome
headless, 1280×800, tema claro do sistema), PNG do frame e `qa.json` em
`docs/mission/evidence/fase-38/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Configurações / Geral` | `15242` (`acesso` `15289`, `backup` `15325`, `nota` `15374`) | `app/src/telas/Configuracoes.svelte` › `configuracoes-geral-1280x800.png` |
| `Linha de opção` `Controle=botão` / `Controle=select` | `10380` / `10825` | `app/src/lib/componentes/LinhaDeOpcao.svelte` (snippet `controle`) |
| `Select` compacto na linha | `10829` (`largura/select-compacto`) | `app/src/lib/componentes/Select.svelte` (`compacto`) |
| Diálogos "Trocar senha mestra" e "Trocar PIN" | sem frame | `app/src/telas/TrocarSenha.svelte`, `TrocarPin.svelte` (`Modal` 611 + `Campo senha` 17511 / `Campo PIN` 650) |

`Configurações / Backup externo ligado` (`16178`) não entra: ligar o backup externo é da
Fase 46.

## Fase 48 — Configurações › backup externo no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas do app (ponte de teste com `--daemon-falso cheio`,
`backup-externo-ligado` e `sem-udisks`, Chrome headless, 1280×800), PNG do frame e
`qa.json` em `docs/mission/evidence/fase-48/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Configurações / Backup externo ligado` | `16178` (`corpo` `16255`; `Linha de opção` interruptor `16265`) | `app/src/telas/Configuracoes.svelte` › `configuracoes-backup-externo-ligado-1280x800.png` |
| Cartão `pendrive` (frame solto, não é componente) | `16318` (avatar `16320` com `Ícone/sucesso` `16329`; textos `16321`: nome `16332`, detalhe `16333`, estado `16334`; `Botão` "Trocar" `16322`) | `app/src/lib/componentes/CartaoDePendrive.svelte` (borda `borda/foco` 6:71, estado `sucesso/texto` 6:86) |
| Diálogo "Escolher pendrive" / "Trocar pendrive" | sem frame | `app/src/telas/EscolherPendrive.svelte` (`Modal` 611, opções no desenho do `16318`, `Botão` 263) › `escolher-pendrive-1280x800.png` |

## Fase 36 — janela de aprovação no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas da janela (ponte de teste com `--daemon-falso aprovacao`,
`aprovacao-prod`, `aprovacao-expirando` e `aprovacao-ataque`, `&tela=aprovacao`, Chrome
headless), PNG dos frames e `qa.json` em `docs/mission/evidence/fase-36/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Contagem` (set; normal, urgente) | `16348` (`16335`, `16343`) | `app/src/lib/componentes/Contagem.svelte` (`urgente` a partir de 20 s, `SEGUNDOS_URGENTE`) |
| `Caixa de código` | `16340` | `app/src/lib/componentes/CaixaDeCodigo.svelte` (texto puro, `direction: ltr; unicode-bidi: isolate`) |
| `Botão ícone` | `18832` | `app/src/lib/componentes/BotaoIcone.svelte` (fechar = negar) |
| `Destaque` prod / perigo | `18756` / `18760`, `18765` | `Destaque.svelte` (`tipo="prod"`, `tipo="perigo"`) |
| `Aprovação / Normal` | `16701` | `app/src/telas/JanelaAprovacao.svelte` › `aprovacao-normal.png` |
| `Aprovação / prod` | `16810` | `aprovacao-prod.png` (borda do `pedido` e `Destaque` prod) |
| `Aprovação / PIN errado` | `16885` | `aprovacao-pin-errado.png` (campo esvaziado a cada envio) |
| `Aprovação / PIN bloqueado` | `16960` | `aprovacao-pin-bloqueado.png` (sem `Contagem`, Permitir desligados) |
| `Aprovação / Expirando` | `17035` | `aprovacao-expirando.png` |
| (sem frame: V5.29) | — | `aprovacao-ataque-d51.png`: comando e cadeia com escapes de D51 |

Textos da janela em `app/src/telas/aprovacao.js`.

## Fase 49 — kit de emergência no código (só leitura do FigCraft)

Lido de uma cópia do `Ward.figcraft` (o original não foi alterado), por `get_node` e
`screenshot`. Capturas (ponte com `--daemon-falso cheio` e `backup-externo-ligado`,
`&tela=configuracoes`, Chrome headless 1280×800), página do PDF, `qa-falso.json` e
`qa-backup-externo-ligado.json` em `docs/mission/evidence/fase-49/`.

| Frame / componente no FigCraft | Id | No código |
| --- | --- | --- |
| `Kit de emergência / Impressão` | `16773` | `app/src/telas/KitEmergencia.svelte` (folha `.folha`, 794×1123 por `pagina/a4-*`) |
| cabeçalho / cofre / backups / senha mestra / como restaurar | `16774` / `16775` / `16776` / `16777` / `16778` | seções da folha; textos em `app/src/telas/kit-emergencia.js` |
| linhas em branco da senha | `16796` (`16805`, `16806`) | `.linhas` / `.linha` (borda `impressao/linha`, sem texto) |
| `Logo / Horizontal` com o nome em `impressao/texto` | `16781` (`16785`) | `Logo.svelte` com `impressao` |
| `Linha de opção` botão (Kit de emergência › Imprimir) | `15242` › `10380` | `Configuracoes.svelte` › `config-kit`, agora habilitado |
| (sem frame) janela de pré-visualização com "Fechar"/"Imprimir" | — | `KitEmergencia.svelte` › `.barra` (`kit-de-emergencia-1280x800-falso.png`) |

## Fase 47 — Restaurar backup (UI; lido de uma cópia do `Ward.figcraft`, nada salvo)

| Nó | Id | No código |
| --- | --- | --- |
| `Restaurar backup / Versões` | `16349` (diálogo `16458`, itens `16466` … `16494`, legenda `16501`, rodapé `16502`) | `app/src/telas/RestaurarBackup.svelte`, etapa `versoes` |
| `Restaurar backup / Confirmar` | `16512` (diálogo `16621`, item `16676`, `Campo senha` `16683`, aviso `18775`, legenda `16664`, rodapé `16665`) | `RestaurarBackup.svelte`, etapa `confirmar` (+ `Campo PIN` "PIN atual", que o frame não tem) |
| `Item de versão` (set) | `11264` (padrão `11243`, selecionado `11265`) | `app/src/lib/componentes/ItemDeVersao.svelte` |
| `Diálogo de restauração` (set) | `13033` | o `<dialog>` de `RestaurarBackup.svelte` |
