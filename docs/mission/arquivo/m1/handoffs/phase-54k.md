# Fase 54k — Rodapé do Diálogo de nível (DESIGN)

## Implementado
- **Botões do `Diálogo de nível` a 44 no FigCraft.** Nos três componentes do set `967`,
  os botões do rodapé tinham um override `size` com a altura em `controle/md` (`6:141`,
  40). Tirei com `component.resetOverride {group: "size"}`; agora seguem o `Botão` `263`
  (`controle/padrao`, `VariableID:47`, 44).
  - `Etapa=Escolher` `968`: rodapé `1018`, botões `1019`/`1023`.
  - `Etapa=PIN` `1027`: rodapé `1069`, botões `1070`/`1074`.
  - `Etapa=PIN errado` `1078`: rodapé `1120`, botões `1121`/`1125`.
  - O `paddingTop` de 4 (`espaco/4`, `6:129`) ficou, como no `DialogoDeNivel.svelte`
    (`padding-top: var(--espaco-4)`). Rodapé de 48; diálogos com 648 (Escolher) e 412
    (PIN, PIN errado), 4 px mais altos que antes.
  - As instâncias `5935` (`5615`), `6314` (`5994`) e `6693` (`6373`) herdaram sem
    override (rodapés `5985`/`6364`/`6743` com 48, botões com 44). São as únicas
    instâncias do set no arquivo (conferido no `document.json`). Como a sobreposição é
    auto layout centrado, os diálogos ficam em y 76 (Escolher) e y 194 (PIN), iguais ao
    app.
- **Conferência no Chrome headless (1280×800)** com os binários já em `target/debug/`
  (socket `/run/user/1000/ward-54k`), cofre de teste, `acme-pg-prod` › "Mudar nível" ›
  "Livre":
  - Escolher: diálogo 360,76 560×648; rodapé y 575, 48 de altura, padding-top 4;
    botões em y 579 com 44. Igual ao `5935`.
  - PIN: diálogo 360,194 560×412; rodapé y 339, 48; botões em y 343 com 44. Igual ao
    `6314`.
  - O PIN errado não foi provocado no app (um erro de PIN conta para a trava de 5 do
    cofre de teste compartilhado); o rodapé dele é o mesmo do PIN no componente e no
    `DialogoDeNivel.svelte`.
- **Comentário do `TrocarPin.svelte`.** O cabeçalho dizia "Sem frame próprio no
  FigCraft"; agora cita `Configurações / Trocar PIN` (`20193`, Fase 39d) e o
  `Diálogo de troca` › `Tipo=PIN` (`19979`, instância `20471`). Só o comentário mudou.
- **Como salvei.** Copiei o arquivo para o scratchpad e abri a cópia numa aba nova do
  headless (índice 14). Antes de salvar, o sha256 do original era `a6d1c8fa…b8c2`, igual
  ao da cópia de partida (o mesmo do fim da 54j). Salvei por cima com `save_file`; o
  sha256 novo é `e52f15bf…5691`.
- **Conferência do arquivo salvo.** `unzip -t` sem erro; no `document.json` em disco os
  12 botões estão com 44 e altura ligada a `VariableID:47`, os 6 rodapés com 48, os
  diálogos com 648/412; os nós das fases 54d (`22962`), 54f (`22964`, `23051`), 54i
  (`23144`) e 54j (`20463` com 44) continuam lá; `cli info` abre (16994 nós); os PNG do
  FigCraft foram gerados do arquivo em disco por `cli export --node`.
- **Evidência** em `docs/mission/evidence/fase-54k/`: `medidas.json`, `nivel-escolher.png`,
  `nivel-pin.png` (app) e `figcraft-967.png`, `figcraft-5615.png`, `figcraft-5994.png`,
  `figcraft-6373.png` (FigCraft).
- Ids em `docs/mission/design-nodes.md` › "Fase 54k".

## Segurança
- Invariantes tocadas: nenhuma SEG. O design não tem segredo (o PIN só como máscara); no
  código só mudou um comentário.
- Testes de ataque: não se aplicam. `grep -c -F -f .dev/variacoes.txt` no `medidas.json`
  deu 0. A URL da ponte (com o token) ficou só num arquivo 0600 do scratchpad e foi
  apagada no fim. Só o cofre de teste, com a senha de teste; nenhum PIN digitado.
- Dependências novas: nenhuma.

## Ficou de fora
- **Cargo e `make check`.** Não rodados, por instrução (a 54c compila em paralelo).
- **PIN errado no app.** Não provocado, pelo motivo acima.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do original antes de salvar (igual ao da cópia) | 0 |
| `make app-front` (build, `app-dist.py`, `npm audit` com os 2 avisos low conhecidos) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54k WARD_DEV_APP=$PWD/target/debug/ward-ponte-teste dev/daemon.sh subir` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54k dev/ponte.sh` (URL num arquivo 0600 do scratchpad) | 0 |
| `python3 -I -B <scratchpad>/54k/pw/medir.py docs/mission/evidence/fase-54k < url` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54k dev/daemon.sh parar` e `rm -rf /run/user/1000/ward-54k` | 0 |
| `grep -c -F -f .dev/variacoes.txt docs/mission/evidence/fase-54k/medidas.json` | 0 achados |
| `unzip -t` e `unzip -p … document.json` no arquivo salvo | 0 |
| `FigCraft.AppImage cli export … --node <id>` (967, 5615, 5994, 6373) e `cli info` | 0 |
| `make check` / cargo | não rodados (instrução do orchestrator) |

## Problemas descobertos
- **Largura dos botões, 2,4–2,7 px** (um pouco acima dos 1–2 px da regra, mesma causa: a
  medida do texto). Continuar 107,7 no app contra 105 no `5990`; Confirmar 109,4 contra
  107 no `6369`; Cancelar 101,8 contra 102; Voltar 81,6 contra 81. O x do primeiro
  botão fica 2,5–3 px à esquerda (288,5 contra 291; 307 contra 310). Não é medida de
  layout; não abri fase.
- **Fundo da tela, fora do diálogo.** No `5615` o rodapé do detalhe tem "Criada em
  02/11/2026 · Alterada em 06/11/2026"; na captura do app esse texto não aparece atrás
  da sobreposição (só Duplicar/Excluir). Pode ser só a entrada do cofre de teste sem as
  datas; não investiguei, fica fora da fase.
- **Abas do headless.** O FigCraft headless tem 15 abas; a 14 é a cópia desta fase.
- **Mudanças de outros workers na árvore** (`crates/…` da 54c e docs) não são desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `app/src/telas/TrocarPin.svelte` | alterado (só o comentário do cabeçalho) |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 54k") |
| `docs/mission/evidence/fase-54k/medidas.json` | criado |
| `docs/mission/evidence/fase-54k/nivel-escolher.png` | criado |
| `docs/mission/evidence/fase-54k/nivel-pin.png` | criado |
| `docs/mission/evidence/fase-54k/figcraft-967.png` | criado |
| `docs/mission/evidence/fase-54k/figcraft-5615.png` | criado |
| `docs/mission/evidence/fase-54k/figcraft-5994.png` | criado |
| `docs/mission/evidence/fase-54k/figcraft-6373.png` | criado |
| `docs/mission/handoffs/phase-54k.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução: a 54c compila em paralelo; `make app-front` = 0)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada)
- [x] (UI/DESIGN) Conferido contra o FigCraft (`save_file` feito, arquivo reaberto pelo `document.json`/`cli info`/`cli export`)
- [x] Ficou dentro do escopo da fase
