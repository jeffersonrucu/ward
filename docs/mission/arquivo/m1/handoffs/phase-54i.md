# Fase 54i — "Diretório de trabalho" nos frames de aprovação (DESIGN)

## Implementado
- Os cinco frames de aprovação do `executar` na Layout do `Ward.figcraft` ganharam a
  `Caixa de código` "Diretório de trabalho", que o app mostra desde a 26a
  (`JanelaAprovacao.svelte`, `{#if pedido.diretorio}`). A caixa entrou na `pedido`, entre
  "Comando" e "Quem pediu", na mesma ordem do app e dos frames da 54f:
  - `Aprovação / Normal` `16701`: caixa `23144` (em `pedido` `16704`).
  - `Aprovação / prod` `16810`: caixa `23147` (`16826`).
  - `Aprovação / PIN errado` `16885`: caixa `23150` (`16901`).
  - `Aprovação / PIN bloqueado` `16960`: caixa `23153` (`16976`).
  - `Aprovação / Expirando` `17035`: caixa `23156` (`17051`).
- Cada caixa é uma instância do `Caixa de código` `16340` da Design System, duplicada da
  `23048` da 54f, com Rótulo "Diretório de trabalho" e Conteúdo
  `"/home/dev/projetos/acme"` (o mesmo do `daemon_falso`). Nenhum componente ou variable
  novo. Os frames são `hug` e cresceram 88 px (72 da caixa e 16 do gap): 872, 944, 872,
  966 e 962. Abaixo deles, na Layout, não há nada que pudesse ficar sobreposto.
- Conferi que, entre todos os frames, só os sete com "Comando" são aprovações de
  `executar`, e todos têm a caixa agora. Os três de origem `http` (`22733`, `22817` e
  `22882`) não têm comando nem diretório, como no app.
- Código: só comentários.
  - `app/src/telas/aprovacao.js`: "sem frame no FigCraft ainda" virou "frames 22964 e
    23051 no FigCraft".
  - `app/src/telas/JanelaAprovacao.svelte`: o cabeçalho cita `22964` (desmarcada) e
    `23051` (marcada) no lugar de "Sem frame no FigCraft ainda". O comentário do
    `Destaque` da entrada padrão cita os dois frames, e o do diretório (Fase 26a) cita os
    frames que têm a caixa.
- Ids estão em `docs/mission/design-nodes.md` › "Fase 54i".
- Como salvei: copiei o arquivo para o scratchpad e abri a cópia numa aba nova do
  headless (índice 12). Editei e conferi que o sha256 do original não tinha mudado
  (`24addb5f…7a2`, igual ao da cópia). Depois salvei por cima com `save_file`
  (`5bec641a…2f6`). Para conferir: `unzip -t` sem erro; no `document.json` do arquivo em
  disco, as cinco caixas estão no índice 2 da `pedido` e as alturas são as novas; os
  frames da 54d (`22962`/`22963`) e da 54f (`22964`/`23051`) continuam lá; `cli info`
  abre o arquivo (16994 nós); os PNG saíram do arquivo em disco pelo
  `cli export --node`.

## Segurança
- Invariantes tocadas: nenhuma de código. O design mostra o diretório que o daemon manda
  e o app já exibe (S9: ver o que vai rodar antes de permitir). O arquivo não tem segredo.
  O PIN aparece só como máscara e o caminho é falso.
- Testes de ataque: não se aplicam (fase só de design e comentários; o ataque do diretório
  com bidi é `ataque_diretorio_com_bidi_vira_escape_visivel`, da 26a).
- Dependências novas: nenhuma.

## Ficou de fora
- `make app-front` não rodou. A mudança no código é só em comentários, e o worker da 54h
  edita e builda `app/src/` em paralelo; um build meu poderia disputar o `app/dist` com o
  dele. Cargo e `make check` não rodaram, por instrução.
- Nada foi commitado.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do original antes de salvar (igual ao da cópia) | 0 |
| `unzip -t` e `unzip -p … document.json` no arquivo salvo | 0 |
| `FigCraft.AppImage cli export … --node <id>` (5 frames, um por vez) | 0 |
| `FigCraft.AppImage cli info` no arquivo salvo | 0 |
| `make check` / cargo / `make app-front` | não rodados (instrução do orchestrator; ver acima) |

## Problemas descobertos
- **Propriedade "Conteúdo" defasada nas caixas "Comando" antigas.** Nos frames `16701`,
  `16810`, `16885`, `16960` e `17035`, a propriedade `Conteúdo` da instância ainda diz
  `ward run clientex-stage-banco -- psql …` (e `clientex-prod-banco` no prod). Já o texto
  desenhado diz `ward run acme-pg-aprovacao …`, porque há um override de texto por cima.
  O desenho está certo, mas quem ler as propriedades por `get_node` vai ver o nome antigo.
  Não mexi, porque está fora do escopo da fase.
- Em uma primeira tentativa, rodar o `cli export` em laço saiu 1 sem gerar arquivo.
  Rodando um por vez, saiu 0. Pode ser a mesma instância do AppImage disputada. Os PNG
  finais estão conferidos (752 de largura, alturas que batem com os frames mais a sombra).
- O headless do FigCraft agora tem 13 abas. A 12 é a cópia desta fase, e as abas velhas
  continuam lá.
- A árvore tem mudanças de outros workers em paralelo (`Modal.svelte`, `crates/…`) e o
  `docs/mission/evidence/fase-54h/`; não são desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `app/src/telas/aprovacao.js` | alterado (só comentário) |
| `app/src/telas/JanelaAprovacao.svelte` | alterado (só comentários) |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 54i") |
| `docs/mission/evidence/fase-54i/aprovacao-normal.png` | criado |
| `docs/mission/evidence/fase-54i/aprovacao-prod.png` | criado |
| `docs/mission/evidence/fase-54i/aprovacao-pin-errado.png` | criado |
| `docs/mission/evidence/fase-54i/aprovacao-pin-bloqueado.png` | criado |
| `docs/mission/evidence/fase-54i/aprovacao-expirando.png` | criado |
| `docs/mission/handoffs/phase-54i.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução do orchestrator)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada em código)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
