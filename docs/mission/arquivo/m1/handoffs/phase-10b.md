# Fase 10b — (DESIGN) Componente `Campo senha` e troca nas telas de senha mestra

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no arquivo já aberto
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem `open_file`. Sempre com ids
  explícitos, sem seleção. Fiz 3 `save_file` (dois intermediários e um no fim), todos no
  mesmo caminho.
- **Seção nova `Seção / Campo senha` (`17499`)** na Design System em x 16000, y 0.
  Partiu de uma duplicata da `Seção / Campo` (`328`), que continua igual. A seção tem
  título, descrição e o set.
- **Set `Campo senha` (`17502`)**, com 6 variantes de `Estado`:
  - `vazio` `17503`: placeholder "Digite a senha mestra";
  - `padrão` `17511`: preenchido e mascarado;
  - `foco` `17524`: borda `borda/foco`;
  - `erro` `17537`: borda e mensagem em `perigo/texto`;
  - `desabilitado` `17550`: textos e ícones em `texto/desabilitado` e `icone/desabilitado`;
  - `revelado` `17587`: borda de foco, valor em `Mono/Corpo` e ícone `olho-fechado`.

  Os 4 estados do plano estão no set. Acrescentei `padrão` para a confirmação da senha
  e `revelado` para mostrar o que a ação revelar faz.
- **Estrutura** (a mesma do `Campo`):
  - rótulo acima, `entrada` com altura `controle/padrao` (44), raio `raio/controle` e
    padding `espaco/16` / `espaco/8`;
  - `Ícone/cadeado` à esquerda;
  - máscara fixa "••••••••••••" em `Texto/Corpo`;
  - **ação revelar** embutida à direita: o frame `ação revelar` de 32×32 (`controle/sm`,
    `raio/sm`, ícone `icone/md`), copiado do `Campo secreto`;
  - ajuda abaixo.

  **Não tem "copiar"**: tirei a `Ação de campo` e a propriedade `Mostrar copiar`.
- **Propriedades**:
  - `Rótulo` (padrão "Senha mestra"), `Placeholder` ("Digite a senha mestra"), `Ajuda`
    ("No mínimo 12 caracteres.") e `Mensagem de erro` ("Senha mestra incorreta.");
  - `Mostrar ajuda`; `Mostrar revelar` (nova, padrão verdadeiro, ligada à visibilidade
    das 6 `ação revelar`);
  - `Ícone à esquerda` (padrão verdadeiro) e `Ícone` (troca, padrão cadeado `206`);
  - `Valor revelado` (nova, só na variante `revelado`, padrão "falso-exemplo-9f3k").

  Tirei a propriedade `Valor`: a máscara não é editável por propriedade.
- **Conferência**:
  - Testei as propriedades com `component.playground` (rótulo, valor revelado, esconder
    ícone e revelar) e todas funcionam.
  - Conferi a seção por screenshot: nada encosta na borda.
  - Auditoria no `.figcraft` salvo (cópia descompactada no scratchpad): na seção
    `17499`, nenhum preenchimento ou borda sólida sem variable, nenhum texto sem text
    style, nenhum gap ou padding solto.
- **Instâncias trocadas nas telas** (`instance.swap`, depois as propriedades
  reaplicadas):

  | Tela | Instância | Variante | Propriedades aplicadas |
  | --- | --- | --- | --- |
  | `Primeiro uso / Senha mestra` (`3619`) | `3679` | foco | Ajuda "Mínimo de 12 caracteres." |
  | `Primeiro uso / Senha mestra` (`3619`) | `3712` | padrão | Rótulo "Confirmar senha mestra", sem ajuda |
  | `Destravar / Padrão` (`3342`) | `3372` | vazio | Ajuda "Só a senha mestra destrava. O PIN não serve aqui." |
  | `Destravar / Senha errada` (`3393`) | `3427` | erro | mesma ajuda, erro "Senha mestra incorreta." (padrão) |
  | `Destravar / Pedido do agente` (`3450`) | `3484` | foco | mesma ajuda |
  | `Restaurar backup / Confirmar` (`16512`) | `16683`, dentro da instância `16621` do `Diálogo de restauração` | foco | Rótulo "Senha mestra desta versão", Ajuda "É a senha que valia em 07/11/2026. O PIN não serve aqui.", Placeholder "Digite a senha mestra desta versão" |

  A troca na `16683` é um override de troca de instância aninhada, feito só na tela; o
  componente não mudou (veja Problemas). Conferi as telas `3619`, `3342`, `3393`,
  `3450` e `16512` por screenshot. Os campos continuam esticando na largura do pai.
- `Primeiro uso / PIN` usa `Campo PIN`, não campo de senha, então não mexi nele.
- Levantei no arquivo salvo onde mais se digita senha mestra:
  - não há outra tela com campo de senha mestra;
  - Configurações só descreve "Trocar pede a senha atual": as telas do fluxo de troca
    não existem (Fase 8, "Ficou de fora").

## Ids para design-nodes
(Não editei `design-nodes.md`, porque o trabalho foi em paralelo com a Fase 9a.)

- Frame de topo da Design System: `Seção / Campo senha` `17499` (título `17500`,
  descrição `17501`), em x 16000, y 0.
- Set `Campo senha` `17502`: vazio `17503`, padrão `17511`, foco `17524`, erro `17537`,
  desabilitado `17550`, revelado `17587`.
  - `entrada`: `17505` `17513` `17526` `17539` `17552` `17589`;
  - `ícone` (cadeado): `17506` `17514` `17527` `17540` `17553` `17590`;
  - `valor`: `17509` (placeholder), `17517` `17530` `17543` `17556`, `17593` (revelado);
  - `ação revelar`: `17563` `17567` `17571` `17575` `17579` `17594`.
- Linha para a tabela de componentes:

  | Componente | Id | Medidas e variables | Propriedades |
  | --- | --- | --- | --- |
  | `Campo senha` (set) — novo 10b | `17502` | largura mínima `largura/campo`, gap `espaco/8`; entrada altura `controle/padrao`, raio `raio/controle`, padding `espaco/16` / `espaco/8`; ação revelar `controle/sm`, `raio/sm`; máscara `Texto/Corpo`, valor revelado `Mono/Corpo`; sem copiar | `Rótulo`, `Placeholder`, `Ajuda`, `Mostrar ajuda`, `Mensagem de erro`, `Ícone à esquerda`, `Ícone`, `Mostrar revelar`, `Valor revelado` |

- Frames do Layout alterados (só a troca de instância; os ids dos frames não mudaram):
  `Primeiro uso / Senha mestra` `3619` (`3679` e `3712` agora são `Campo senha`),
  `Destravar / Padrão` `3342` (`3372`), `Destravar / Senha errada` `3393` (`3427`),
  `Destravar / Pedido do agente` `3450` (`3484`) e `Restaurar backup / Confirmar`
  `16512` (override `16683` dentro de `16621`).

## Segurança
- Invariantes tocadas: nenhuma de código. O design segue as regras de segredo (S1–S15
  em `docs/escopo.md`): o campo de senha mestra **não tem ação copiar**, então a senha
  mestra nunca vai para a área de transferência. O valor só aparece na variante
  `revelado`, por ação explícita. Os exemplos são falsos ("falso-exemplo-9f3k" e a
  máscara de pontos).
- Testes de ataque: não se aplicam (fase de design, sem código).
- Dependências novas: nenhuma.

## Ficou de fora
- **Troca no componente `Diálogo de restauração`** (set `13033`, variante Confirmar
  `13034`, instância interna `13088`, que ainda é `Campo` foco com `Ação de campo`
  escondida). A regra desta rodada proibia alterar componentes existentes, então fiz a
  troca só como override na tela `16512`.
- Variante `revelado` com ícone em estado ativo (fundo no botão revelar): não está no
  plano, e o ícone `olho-fechado` já indica o estado.
- Ordem das variantes no set: `revelado` ficou por último.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não existe (sem `Makefile` no Milestone 0; fase só de design) |
| `save_file` (3×) | ok |

## Problemas descobertos
- **`Diálogo de restauração` (`13034`/`13088`) ainda usa `Campo`**, com uma `Ação de
  campo` "Copiar" escondida. Qualquer outra instância desse componente, e a fase de UI
  lendo o componente, vai encontrar o `Campo` comum. Sugestão (10c ou Fase 10): trocar
  `13088` por `Campo senha` foco no próprio componente e depois tirar o override de
  `16683`.
- `Extensão / Salvar login novo` (`17414`): a senha do site está num `Campo` padrão
  (`17450`) com máscara de pontos e "Copiar" escondido, e não num `Campo secreto`. É
  senha de credencial, não senha mestra, então ficou fora desta fase. Registro para a
  10c ou a Fase 10.
- O plano lista 4 estados do `Campo senha` (vazio, foco, erro, desabilitado). Acrescentei
  `padrão` (preenchido, para a confirmação) e `revelado`; o contrato V0.3 não cita o
  `Campo senha`. Se o validador exigir só os 4, os extras não atrapalham.
- O FigCraft não aplica os padrões das propriedades de texto às camadas do componente
  principal, então editei os textos direto nas camadas.
- A fonte JetBrains Mono (`Mono/Corpo`) parece não estar instalada: o render usa a fonte
  de reserva. O mesmo vale para o `Campo secreto` revelado.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (seção e set novos, 6 instâncias trocadas) |
| `docs/mission/handoffs/phase-10b.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não se aplica (sem código nem `Makefile` no Milestone 0)
- [x] Só cofre de teste e credenciais falsas usados
- [ ] Teste de ataque para cada (SEG) tocado: não se aplica (design)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
