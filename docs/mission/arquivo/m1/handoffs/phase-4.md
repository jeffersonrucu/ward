# Fase 4 — (FIGMA) Credenciais e nível de acesso

## Implementado
- Sessão do Figma no arquivo `bzu9PxxFur2tU3vlNCHvsY`. A página Inspirações (`1:3`) foi
  lida (screenshot e script de conferência da Fase 1), sem nenhuma escrita.
- Direção tirada das Inspirações: barra lateral escura com navegação (RoboForm/Bitwarden)
  e lista com badges à direita (Defender/Bitwarden).
- **Design System (`1:2`)**: componentes novos criados antes das telas, na seção
  `Seção / App` (`22:104`), com tudo ligado a variables e text styles:
  - Ícones novos no frame `Componentes / Ícones` (`9:3`), no mesmo padrão dos ícones da
    Fase 2 (20 px em `icone/md`, traço em `linha/icone`, cor `icone/padrao`):
    `Ícone/chave` (`22:89`), `Ícone/gerador` (`22:99`), `Ícone/engrenagem` (`22:103`).
  - `Item de navegação` (set `22:125`): Estado = padrão `22:107`, hover `22:113`,
    ativo `22:119`. Propriedades Rótulo (texto) e Ícone (troca).
  - `Navegação lateral` (set `22:277`): Ativo = Credenciais `22:126`,
    Gerador de senha `22:177`, Configurações `22:227`. Tem marca (logo provisório com
    cadeado), as três telas do app (R25), "Cofre destravado" e o botão `Travar agora`
    (R23). Já serve às Fases 8 (Gerador, Configurações).
  - `Cabeçalho de grupo` (`22:279`): cliente e contagem, para a lista agrupada por cliente.
  - `Chip de filtro` (`22:282`): filtro ativo, com o x para remover.
  - `Opção de nível` (set `23:228`): Nível (bloqueado, com aprovação, livre) ×
    Selecionado (não, sim) = 6 variantes (`23:171`, `23:181`, `23:192`, `23:201`,
    `23:211`, `23:219`), cada uma com explicação curta.
  - `Diálogo de nível` (set `23:392`): Etapa = Escolher `23:229`, PIN `23:290`,
    PIN errado `23:341`. Propriedade Entrada (texto). Usa instâncias de `Badge tipo`,
    `Badge ambiente`, `Opção de nível`, `Campo PIN` e `Botão`.
- **Layout (`0:1`)**: seis frames de 1280×800, todos com o modo `Escuro` (`6:2`)
  explícito na coleção `Cores`, montados só com instâncias:

| Frame | Node | Conteúdo |
| --- | --- | --- |
| `Credenciais / Vazia` | `26:919` | Busca e filtros desabilitados; cartão "Nenhuma credencial ainda", com a explicação de que entradas novas começam "com aprovação"; `Nova credencial` |
| `Credenciais / Cheia` | `26:531` | "8 entradas · 3 clientes"; busca, `Select` Tipo e Ambiente; grupos `acme` (4), `beta` (2), `gama` (2, rolando); cada `Item de lista` com `Badge tipo`, `Badge ambiente` e o nível (livre, com aprovação, bloqueado) |
| `Credenciais / Filtrada` | `26:785` | Busca "pg", Tipo `banco`, Ambiente `stage`; chips "Busca: pg", "Tipo: banco", "Ambiente: stage" e `Limpar filtros`; resultado "2 de 8 entradas" (`acme-pg-aprovacao`, `acme-pg-tunel`) |
| `Nível de acesso / Escolher` | `26:1009` | Lista ao fundo sob `fundo/sobreposicao`; diálogo com a entrada `acme-pg-prod` (banco, PROD), as três opções com explicação, "Livre" escolhido, "Nível atual: com aprovação", o aviso "O ambiente não muda as regras: dev, stage e prod têm os mesmos três níveis. Mudar o nível pede o PIN."; `Cancelar` / `Continuar` |
| `Nível de acesso / PIN` | `26:1091` | "De com aprovação para livre"; `Campo PIN` completo; "O PIN confirma a mudança. Ele não destrava o cofre."; "5 erros de PIN seguidos travam o cofre."; `Voltar` / `Confirmar` |
| `Nível de acesso / PIN errado` | `26:1165` | `Campo PIN` em erro "PIN incorreto. Nada foi alterado."; aviso "O nível continua com aprovação. 5 erros de PIN seguidos travam o cofre."; `Voltar` / `Confirmar` |

- Auditoria por script nos 6 frames e na `Seção / App` (mais os 3 ícones): nenhuma
  pendência. Todo preenchimento e borda sólidos ligados a variable de cor, toda espessura
  de borda a `linha/*`, padding, gap e raio maiores que zero ligados a variables, todo
  texto com text style. Instâncias contadas nos 6 frames: Navegação lateral 6, Botão 20,
  Campo 6, Select 12, Cabeçalho de grupo 13, Item de lista 34, Badge tipo 37, Badge
  ambiente 37, Chip de filtro 3, Diálogo de nível 3, Opção de nível 3, Campo PIN 2.
- Conferência visual por screenshot de cada frame e dos sets novos (sem texto cortado nem
  sobreposição). Inspirações reconferida no fim: os 4 filhos idênticos ao snapshot.

## Segurança
- Invariantes tocadas: nenhuma S1–S15 diretamente (fase só de design). O desenho segue:
  - V0.7/V0.20/R6: a lista mostra só campos não secretos (nome, usuário, host, porta,
    base, URL, nome da variável, contagem de extras e anexos). Nenhuma senha, token,
    semente ou valor de extra aparece, nem mascarado. Todos os dados vêm do cofre de
    teste falso do contrato (D15).
  - R15/S9: mudar o nível é um fluxo em duas etapas, e a segunda exige o PIN. O PIN errado
    diz "Nada foi alterado" e lembra que 5 erros seguidos travam o cofre, sem mostrar
    quantas tentativas restam (mesma linha do D20 na Fase 3).
  - R16: um único `Diálogo de nível` para todos os ambientes, mostrado com uma entrada
    `prod` e os mesmos três níveis, inclusive "livre", com o texto explícito de que o
    ambiente não muda as regras.
  - R24: o diálogo diz que o PIN não destrava o cofre.
  - O app mostra as entradas bloqueadas (`gama-bloqueada`); a explicação de "Bloqueado"
    diz que ela some do `ward lista` (só o agente não a vê, R15/D3).
- Testes de ataque: não se aplica (M0 não tem asserção (SEG)).
- Dependências novas: nenhuma. Os três ícones novos foram importados como SVG a partir
  dos desenhos do Lucide (licença ISC), sem biblioteca remota no arquivo. Se o app usar o
  mesmo SVG, a licença ISC precisa constar nos avisos do pacote.

## Ficou de fora
- Formulário de cada tipo (Fases 5 e 6). No fluxo real, o nível é mudado de dentro do
  formulário da entrada (R25). Aqui o diálogo aparece sobre a lista porque o formulário
  ainda não existe. A Fase 35 (UI) deve abri-lo a partir do formulário.
- Estado de lista "busca sem resultado": o contrato não pede. Fica para a Fase 32 decidir.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` ainda não existe (é criado na Fase 11) |
| `use_figma` criação dos ícones e componentes na Design System | ok |
| `use_figma` criação dos 6 frames no Layout | ok (uma primeira tentativa falhou antes de criar qualquer nó, por id errado de componente; conferido que nada ficou parcial) |
| `use_figma` auditoria de ligações (Layout e `Seção / App`) | ok, 0 pendências |
| `use_figma` conferência de `1:3` com o script da Fase 1 | ok, idêntico |

## Problemas descobertos
- `Credenciais / Cheia` mostra 8 das 20 entradas do cofre de teste (acme, beta e gama),
  para caber em 800 px com grupos visíveis. A lista rola (`gama` aparece cortado). As
  contagens dentro do frame são coerentes entre si ("8 entradas · 3 clientes").
- No `Item de lista` os badges ficam encostados à direita conforme o tamanho do texto, por
  isso não formam colunas alinhadas entre linhas. Se o usuário quiser colunas, o
  componente precisa de larguras fixas (e não há token de largura).
- Medidas fixas sem token, como já registrado nas Fases 2 e 3: largura da
  `Navegação lateral` (240), `Item de navegação` (208), `Select` de filtro (200 e 180),
  `Diálogo de nível` (520), `Opção de nível` (472), cartão vazio (440) e o tamanho da
  sobreposição (1280×800). Padding, gap, raio, altura de controle e ícone estão ligados.
- O tracejado (`dashPattern`) do cartão vazio e dos sets tem números fixos; não há token
  para isso.
- `Nível de acesso / PIN errado` mantém os 6 pontos no campo em erro (é o estado `erro` do
  `Campo PIN` da Fase 2). A Fase 35 precisa decidir se limpa o campo depois do erro.
- O escopo não diz qual é o nível de uma entrada criada pela extensão versus pelo app;
  o texto da tela vazia usa "com aprovação" como padrão, conforme "Nível de acesso por
  entrada".

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/mission/handoffs/phase-4.md` | criado |

Nodes no Figma:
| Item | Página | Id |
| --- | --- | --- |
| `Seção / App` | Design System | `22:104` |
| `Ícone/chave`, `Ícone/gerador`, `Ícone/engrenagem` | Design System | `22:89`, `22:99`, `22:103` |
| `Item de navegação` (set) | Design System | `22:125` |
| `Navegação lateral` (set) | Design System | `22:277` |
| `Cabeçalho de grupo` | Design System | `22:279` |
| `Chip de filtro` | Design System | `22:282` |
| `Opção de nível` (set) | Design System | `23:228` |
| `Diálogo de nível` (set) | Design System | `23:392` |
| `Credenciais / Vazia` | Layout | `26:919` |
| `Credenciais / Cheia` | Layout | `26:531` |
| `Credenciais / Filtrada` | Layout | `26:785` |
| `Nível de acesso / Escolher` | Layout | `26:1009` |
| `Nível de acesso / PIN` | Layout | `26:1091` |
| `Nível de acesso / PIN errado` | Layout | `26:1165` |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): o `Makefile` só existe a partir da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/FIGMA) Conferido contra o Figma (screenshots, auditoria de ligações, Inspirações reconferida)
- [x] Ficou dentro do escopo da fase
