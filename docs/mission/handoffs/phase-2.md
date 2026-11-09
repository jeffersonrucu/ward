# Fase 2 — (FIGMA) Componentes base com variantes

## Implementado
- Sessão do Figma no arquivo `bzu9PxxFur2tU3vlNCHvsY`. Tudo foi criado na página
  **Design System** (`1:2`). A página Layout (`0:1`) não foi tocada. A página
  Inspirações (`1:3`) foi só lida.
- Fundações que faltavam para os componentes (sem recriar nada da Fase 1):
  - Variables novas em `Cores` (modo `Escuro`, alias das Primitivas, code syntax WEB):
    `icone/padrao` (`VariableID:8:2`, neutro/400), `icone/forte` (`8:3`, neutro/50),
    `icone/desabilitado` (`8:4`, neutro/600), `icone/marca` (`8:5`, azul/300),
    `icone/sobre-cor` (`8:6`, neutro/0), todas com escopo `STROKE_COLOR`;
    `sombra/padrao` (`8:7`, sobreposicao/escuro, escopo `EFFECT_COLOR`).
  - Variable nova em `Espaçamento`: `linha/icone` = 1,5 (`VariableID:9:2`, `STROKE_FLOAT`),
    usada no traço dos ícones.
  - 14 text styles, com família, estilo, tamanho, altura de linha e espaçamento de letra
    ligados às variables de `Tipografia`: `Título/{Destaque,Página,Modal,Seção}`,
    `Texto/{Corpo,Corpo forte,Pequeno,Rótulo,Legenda,Selo,Selo grande}` (os dois Selo em
    caixa alta), `Mono/{Corpo,Pequeno,PIN}` (JetBrains Mono).
  - 2 effect styles com a cor ligada a `sombra/padrao`: `Sombra/Modal`, `Sombra/Flutuante`.
- 24 componentes de ícone (`Ícone/*`, 20 px ligados a `icone/md`, traço `linha/icone`, cor
  `icone/padrao`), desenhados no próprio arquivo para não depender de biblioteca remota:
  olho, olho-fechado, copiar, check, fechar, chevron-baixo, chevron-direita, alerta, erro,
  sucesso, cadeado, busca, mais, relogio, clipe e um por tipo (`tipo-login`, `tipo-banco`,
  `tipo-servidor`, `tipo-api`, `tipo-aws`, `tipo-smtp`, `tipo-ftp`, `tipo-nota`,
  `tipo-generico`). Ficam no frame `Componentes / Ícones` (`9:3`).
- 10 component sets, cada um numa seção `Seção / <nome>` (coluna x = 640, abaixo dos ícones),
  com descrição em português no componente:

| Componente | Id do set | Variantes | Propriedades |
| --- | --- | --- | --- |
| `Botão` | `10:69` | Tipo (primário, secundário, perigo, fantasma) × Estado (padrão, hover, foco, desabilitado) = 16 | Rótulo, Mostrar ícone, Ícone (swap) |
| `Campo` | `11:45` | Estado (vazio, padrão, foco, erro, desabilitado) | Rótulo, Valor, Placeholder, Ajuda, Mensagem de erro, Mostrar ajuda, Ícone à esquerda, Ícone (swap) |
| `Campo secreto` | `11:91` | Estado (oculto, revelado, copiado); ações `revelar` e `copiar` em todos | Rótulo, Valor revelado |
| `Select` | `11:132` | Estado (padrão, foco, aberto, desabilitado) | Rótulo, Valor |
| `Badge ambiente` | `12:35` | Ambiente (dev, stage, prod) × Tamanho (padrão, grande) = 6 | — |
| `Badge tipo` | `12:82` | Tipo (login, banco, servidor, api, aws, smtp, ftp/sftp, nota segura, genérico) = 9 | — |
| `Item de lista` | `13:101` | Estado (padrão, hover, selecionado); contém instâncias de `Badge tipo` e `Badge ambiente` | Nome, Detalhe, Nível, Mostrar nível |
| `Modal` | `13:143` | Tipo (padrão, perigo); rodapé com instâncias de `Botão` | Título, Texto |
| `Toast` | `12:110` | Tipo (sucesso, erro, aviso) | Mensagem |
| `Campo PIN` | `13:212` | Estado (vazio, digitando, completo, erro, bloqueado) | Rótulo, Ajuda, Mensagem de erro, Seis dígitos |

- Ids das variantes:
  - Botão: primário `10:5` `10:9` `10:13` `10:17`; secundário `10:21` `10:25` `10:29`
    `10:33`; perigo `10:37` `10:41` `10:45` `10:49`; fantasma `10:53` `10:57` `10:61`
    `10:65` (na ordem padrão, hover, foco, desabilitado).
  - Campo: vazio `11:5`, padrão `11:13`, foco `11:21`, erro `11:29`, desabilitado `11:37`.
  - Campo secreto: oculto `11:49`, revelado `11:63`, copiado `11:78`.
  - Select: padrão `11:95`, foco `11:101`, aberto `11:107`, desabilitado `11:126`.
  - Badge ambiente: dev/stage/prod padrão `12:23` `12:25` `12:27`; grande `12:29` `12:31`
    `12:33`.
  - Badge tipo: login `12:39`, banco `12:44`, servidor `12:49`, api `12:55`, aws `12:59`,
    smtp `12:63`, ftp/sftp `12:68`, nota segura `12:72`, genérico `12:77`.
  - Toast: sucesso `12:86`, erro `12:94`, aviso `12:102`.
  - Item de lista: padrão `13:47`, hover `13:65`, selecionado `13:83`.
  - Modal: padrão `13:105`, perigo `13:123`.
  - Campo PIN: vazio `13:147`, digitando `13:157`, completo `13:170`, erro `13:186`,
    bloqueado `13:202`.
- Seções (frames de apresentação na Design System): Botão `10:2`, Campo `11:2`,
  Campo secreto `11:46`, Select `11:92`, Badge ambiente `12:20`, Badge tipo `12:36`,
  Toast `12:83`, Item de lista `13:44`, Modal `13:102`, Campo PIN `13:144`.
- Auditoria por script em todos os 34 componentes e sets da página: todo preenchimento e
  borda sólidos ligados a variable de cor, toda espessura de borda ligada a `linha/*`, todo
  padding, gap e raio maior que zero ligados a variables, todo texto com text style (que
  por sua vez é ligado às variables de tipografia), toda sombra por effect style, e as 71
  instâncias de ícone com cor ligada. A auditoria achou 2 bordas soltas nos Botões
  fantasma (padrão e desabilitado), já corrigidas. Altura de controle ligada a
  `controle/{sm,md,lg}` e ícones a `icone/{sm,md}`.
- Conferência visual por screenshot de cada seção.

## Segurança
- Invariantes tocadas: nenhuma S1–S15 diretamente, porque a fase é só de design. Mesmo
  assim, o desenho segue S11/S12/D4: o `Campo secreto` usa máscara fixa `••••` em
  oculto e copiado (a máscara não é propriedade, então nenhuma instância oculta carrega
  valor), o valor só aparece no estado revelado, com o aviso "some em 30 s ou ao sair da
  tela", e o estado copiado avisa que a área de transferência é limpa em 30 s. O
  `Campo PIN` mostra só `•`, nunca dígitos.
- S15/V0.20: todos os exemplos são falsos (`falso-exemplo-9f3k`, `clientex / stage /
  banco`, `db.clientex.test`). Nenhuma credencial real em lugar nenhum.
- Testes de ataque: não se aplica, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. Os ícones foram desenhados no arquivo, sem biblioteca
  remota (o arquivo tem Material 3/Simple DS/Apple como bibliotecas, mas não usei
  nenhuma).

## Ficou de fora
- Telas e frames do Layout (Fases 3–9).
- Janela de aprovação com os quatro botões: o `Modal` é o diálogo genérico de
  confirmação; a aprovação é desenhada na Fase 7 a partir de `Botão`, `Badge ambiente`
  (Tamanho=grande) e `Campo PIN`.
- Ícone do produto (Fase 9).
- Code Connect: não há código ainda (fica para a Fase 29).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (ele é criado na Fase 11) |
| `use_figma` com a conferência de `1:3` (script do snapshot da Fase 1) | ok, idêntico ao snapshot |
| `use_figma` com a auditoria de ligações | ok, 0 pendências depois da correção |

## Problemas descobertos
- **Plano da Fase 10a está errado num exemplo**: ela manda remover `neutro/50` como tom
  claro sem uso, mas `texto/primario` e o novo `icone/forte` são alias de `neutro/50`.
  Remover quebra as cores. A Fase 10a precisa conferir o uso real de cada primitiva antes
  de apagar (`neutro/0` também é usado por `primaria/sobre`, `perigo/sobre`,
  `ambiente/prod/sobre` e `icone/sobre-cor`).
- O escopo (Missão 0) lista os componentes base sem `Campo PIN`, enquanto o plano e o
  V0.3 o exigem. Ele foi criado, porque é o que o contrato pede. Divergência só
  registrada.
- Variables adicionadas fora das quatro coleções de tokens previstas: `icone/*` e
  `sombra/padrao` em `Cores`, e `linha/icone` em `Espaçamento`. Motivo: as cores de texto
  só têm escopo `TEXT_FILL`, e ícone de traço precisa de `STROKE_COLOR`. A sombra
  precisava de cor ligada.
- Propriedade de texto é do set inteiro, então o exemplo de `Toast` (erro e aviso) e de
  `Modal` (perigo) mostra o texto padrão da propriedade ("Entrada salva.", "Salvar
  alterações?"). Nas telas, a instância troca o texto. Se o QA estranhar a apresentação,
  isso é esperado do Figma.
- A sombra (offset e desfoque) dos effect styles tem números fixos, porque não há token de
  elevação. A cor está ligada. O V0.2 não cita efeitos, mas fica registrado.
- Larguras de referência dos componentes são fixas (Campo/Select 320, Toast 360,
  Modal 480, Item de lista 720). Nas telas, o tamanho vem do layout (FILL). Não há token
  de largura no escopo.
- A página `1:3` continua com o nome "Inspiration" no arquivo (já registrado na Fase 1).
- As inconsistências de D1/V0.1 já registradas na Fase 1 seguem com o orchestrator.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/mission/handoffs/phase-2.md` | criado |

Nodes/ids no Figma (página Design System `1:2`): ver as tabelas acima, e também
`Componentes / Ícones` `9:3` com os ícones `9:7` (olho), `9:12` (olho-fechado), `9:16`
(copiar), `9:19` (check), `9:22` (fechar), `9:25` (chevron-baixo), `9:28`
(chevron-direita), `9:32` (alerta), `9:36` (erro), `9:40` (sucesso), `9:44` (cadeado),
`9:48` (busca), `9:51` (mais), `9:55` (relogio), `9:58` (clipe), `9:62` (tipo-login),
`9:66` (tipo-banco), `9:71` (tipo-servidor), `9:74` (tipo-api), `9:77` (tipo-aws), `9:81`
(tipo-smtp), `9:84` (tipo-ftp), `9:88` (tipo-nota), `9:92` (tipo-generico).

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/FIGMA) Conferido contra o Figma (screenshot de cada seção; auditoria de ligações; Inspirações reconferida)
- [x] Ficou dentro do escopo da fase
