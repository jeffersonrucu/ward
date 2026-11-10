# Fase 54j — Rodapé dos modais e Trocar PIN (DESIGN)

## Implementado
- **Rodapé dos modais no FigCraft.** Os botões do rodapé voltaram a 44 px, como o
  `Botão` `263` (`controle/padrao`, `VariableID:47`), e o rodapé perdeu o padding de 4 px
  no topo (`espaco/4`, `6:129`). As mudanças foram feitas nos componentes, e as
  instâncias herdaram.
  - `Modal` › `Tipo=padrão` `612`: rodapé `619`, botões `620` e `624`.
  - `Modal` › `Tipo=perigo` `628`: rodapé `638`, botões `639` e `643`.
  - `Diálogo de troca` › `Tipo=senha mestra` `19918`: rodapé `19925`, botões `19926` e
    `19930`.
  - `Diálogo de troca` › `Tipo=PIN` `19979`: rodapé `20030`, botões `20031` e `20035`.
  - Os frames herdaram: `20085` › `20411` (rodapé `20462`, botões `20463`/`20467`) e
    `20193` › `20471` (rodapé `20525`, botões `20526`/`20530`).
  - Os botões tinham um override `size` com a altura ligada a `controle/md` (`6:141`,
    40). Tirei esse override com `component.resetOverride {group: "size"}`. Nos dois
    "Cancelar" do `Diálogo de troca` (`19926`, `20031`) não havia override registrado, mas
    a altura também estava ligada a `6:141`; nesses, religuei a altura a `VariableID:47`.
  - Os rodapés continuam com 44 de altura, e os diálogos ficaram com a mesma altura de
    antes (186, 608 e 558). Os botões subiram 4 px: agora ficam em y 635 no `20085` e em
    y 489 no `20193`, iguais ao app.
- **Trocar PIN, diferença de 4 px.** Medi os dois lados e o erro estava no app.
  - No design, a explicação `20478` usa `8:12` (14/20, duas linhas, 40 de altura). O
    `TrocarPin.svelte` usava `--tamanho-sm`/`--altura-linha-sm` (13/18, 36 de altura).
  - Em `app/src/telas/TrocarPin.svelte`, a `.explicacao` passou a usar
    `--tamanho-base`/`--altura-linha-base`. A `.nota` continua 13/18, que é o `8:14` do
    `20524`.
  - Conferi no Chrome headless em 1280×800. O diálogo agora mede 400,121 480×558. Os
    filhos do corpo ficam em y 77/133/223/337/451 com alturas 40/74/98/98/18, o rodapé
    em y 489 com 44 de altura e padding-top 0. Tudo isso é igual ao `20471`.
  - O "Trocar senha mestra" continua batendo com o `20085`: diálogo de 608, rodapé e
    botões em y 539 relativo ao diálogo (635 na tela).
- **Como salvei.** Copiei o arquivo para o scratchpad e abri a cópia numa aba nova do
  headless (índice 13). Editei a cópia e confirmei que o sha256 do original não tinha
  mudado (`5bec641a…02f6`, igual ao da cópia de partida). Depois salvei por cima com
  `save_file`; o sha256 novo é `a6d1c8fa…b8c2`.
- **Conferência do arquivo salvo.**
  - `unzip -t` não apontou erro.
  - No `document.json` em disco, os seis rodapés estão com padding 0 e sem `paddingTop`
    ligado, e não sobrou nenhum botão de 40 em rodapé de `Modal`/`Diálogo de troca`.
  - Os nós das fases 54d, 54f e 54i continuam lá.
  - `cli info` abre o arquivo (16994 nós).
  - Os PNG foram gerados do arquivo em disco por `cli export --node`.
- **Evidência** em `docs/mission/evidence/fase-54j/`:
  - `medidas.json`;
  - capturas do app: `trocar-pin.png` e `trocar-senha-mestra.png`;
  - capturas do FigCraft: `figcraft-611.png`, `figcraft-19978.png`,
    `figcraft-20085.png` e `figcraft-20193.png`.
- Os ids estão em `docs/mission/design-nodes.md` › "Fase 54j".

## Segurança
- Invariantes tocadas: nenhuma SEG.
  - O design não tem segredo; o PIN aparece só como máscara.
  - A mudança no app é só de CSS (tokens, sem medida solta). Não mudei estado, comando
    Tauri ou CSP.
- Testes de ataque: não se aplicam, porque nenhuma SEG foi tocada.
  - `grep -c -F -f .dev/variacoes.txt` no `medidas.json` deu 0.
  - A URL da ponte, que tem o token, ficou só num arquivo 0600 do scratchpad e foi
    apagada no fim.
  - Usei só o cofre de teste, com a senha de teste.
- Dependências novas: nenhuma.

## Ficou de fora
- **Cargo e `make check`.** Não rodei nenhum dos dois, por instrução, porque a 54c compila
  em paralelo. O daemon de teste subiu com os binários que já estavam em `target/debug/`:
  `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54j WARD_DEV_APP=…/ward-ponte-teste dev/daemon.sh subir`
  e depois `dev/ponte.sh`. No fim, parei o daemon e apaguei `/run/user/1000/ward-54j`.
- **`Diálogo de nível`** (`967`: `968`/`1027`/`1078` e as instâncias `5935`, `6314` e
  `6693`). Não mexi, porque fica fora do texto da fase e tem um rodapé próprio no app
  (ver Problemas).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do original antes de salvar (igual ao da cópia) | 0 |
| `make app-front` (build, `app-dist.py`, `npm audit` com os 2 avisos low conhecidos) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54j WARD_DEV_APP=$PWD/target/debug/ward-ponte-teste dev/daemon.sh subir` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54j dev/ponte.sh` (URL num arquivo 0600 do scratchpad) | 0 |
| `python3 -I -B <scratchpad>/pw54j/medir.py docs/mission/evidence/fase-54j < url` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54j dev/daemon.sh parar` | 0 |
| `grep -c -F -f .dev/variacoes.txt docs/mission/evidence/fase-54j/medidas.json` | 0 achados |
| `unzip -t` e `unzip -p … document.json` no arquivo salvo | 0 |
| `FigCraft.AppImage cli export … --node <id>` (611, 19978, 20085, 20193) e `cli info` | 0 |
| `make check` / cargo | não rodados (instrução do orchestrator) |

## Problemas descobertos
- **`Diálogo de nível`: o app fica 4 px mais alto que o design.**
  - No design (`967` e instâncias), o rodapé tem padding-top 4 com botões de 40 (override
    `size`), o que dá 44.
  - O `DialogoDeNivel.svelte` tem rodapé próprio, com `padding-top: var(--espaco-4)` e o
    `Botao` de 44, o que dá 48.
  - O lado certo é decisão de uma fase própria. Uma saída é botões de 44 no design com o
    padding de 4 mantido. A outra é tirar o padding nos dois lados.
- **`Diálogo de filtros`** (`5222`/`5578`): o design tem botões de 44 com padding 4 (48)
  e o `DialogoFiltros.svelte` faz o mesmo. Não há divergência, mas fica diferente do
  `Modal`.
- **Largura dos botões no app.** Fica 1 a 2 px fora da do design por causa da medida do
  texto (Cancelar 101,8 contra 102; Trocar senha 129,8 contra 127; Trocar PIN 112,7
  contra 111). Isso desloca o x do primeiro botão em até 2 px. É coisa da renderização da
  fonte, não de medida.
- **Comentário velho no `TrocarPin.svelte`.** O cabeçalho ainda diz "Sem frame próprio no
  FigCraft", mas o frame `20193` existe (Fase 39d). Não mexi, porque está fora do escopo.
- **Abas do headless.** O FigCraft headless agora tem 14 abas; a 13 é a cópia desta fase.
- **Mudanças de outros workers na árvore.** Os `crates/…` da 54c e as mudanças das fases
  anteriores ainda não commitadas não são desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `app/src/telas/TrocarPin.svelte` | alterado (`.explicacao` 14/20) |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 54j") |
| `docs/mission/evidence/fase-54j/medidas.json` | criado |
| `docs/mission/evidence/fase-54j/trocar-pin.png` | criado |
| `docs/mission/evidence/fase-54j/trocar-senha-mestra.png` | criado |
| `docs/mission/evidence/fase-54j/figcraft-611.png` | criado |
| `docs/mission/evidence/fase-54j/figcraft-19978.png` | criado |
| `docs/mission/evidence/fase-54j/figcraft-20085.png` | criado |
| `docs/mission/evidence/fase-54j/figcraft-20193.png` | criado |
| `docs/mission/handoffs/phase-54j.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução: a 54c compila em paralelo; `make app-front` = 0)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada)
- [x] (UI/DESIGN) Conferido contra o FigCraft (`save_file` feito, arquivo reaberto pelo `document.json`/`cli info`/`cli export`)
- [x] Ficou dentro do escopo da fase
