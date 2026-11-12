# Fase 49b — Estados novos da 49a no FigCraft (DESIGN)

## Implementado
- **`Item de versão` (set `11264`, Design System):** duas variantes novas.
  - `Estado=fora de ordem` `20534`: cópia do padrão, `detalhe` `20539`.
  - `Estado=fora de ordem selecionado` `20541`: cópia do selecionado, `detalhe` `20546`.
  - Nas duas, o `detalhe` usa `aviso/texto` (`6:83`) com o texto "Cópia local · gravação mais antiga do que a data diz", igual ao app (`restaurar.js` › `detalheDaVersao`, `ItemDeVersao.svelte` › `alerta`).
- **`Diálogo de restauração` › `Etapa=confirmar` (`13034`):** o `Destaque` aviso `18770` ganhou a frase do app (`avisoDaTroca`): "O tempo de trava e o backup externo continuam os de agora. A versão volta como uma gravação nova do cofre." Ela chega por herança ao `Restaurar backup / Confirmar` `16512` (aviso `18775`).
- **Telas novas no Layout (1280×800, y 13300), feitas com instâncias:**
  - `Restaurar backup / Versões com cópia plantada` `20548` (x 0):
    - o item do topo `20665` é a variante `fora de ordem` ("08/11/2026 às 14:35", "agora");
    - os outros itens são "Cópia local", como no app, sem tamanho nem pendrive;
    - a legenda é a do app, "31 versões, da mais nova para a mais antiga. …";
    - "Continuar" `20707` aparece desabilitado, porque nada está escolhido.
  - `Restaurar backup / Confirmar cópia plantada` `20711` (x 1360):
    - item `20828` na variante `fora de ordem selecionado`;
    - `Campo senha` `20835` com a ajuda na data da versão;
    - aviso `20847` com a frase nova.
  - `Configurações / Backup local inseguro` `20863` (x 2720):
    - cópia de `Configurações / Geral`;
    - `Destaque` aviso `20971` logo abaixo da linha "Backup local" `20942`, com o texto de `AVISO_BACKUP_LOCAL`;
    - descrição da linha sem "Último: …", como no app.
- Ids registrados em `docs/mission/design-nodes.md` › "Fase 49b". PNG dos frames e do componente em `docs/mission/evidence/fase-49b/`.

## Segurança
- **Invariantes tocadas:** nenhuma em código. As telas só mostram o que a 49a decidiu:
  - o item mostra só a marca, nunca a geração (D39);
  - o aviso da pasta não traz caminho nenhum além do padrão documentado;
  - a senha aparece mascarada; o "Valor revelado" falso do `Campo senha` já existia.
- **Testes de ataque:** não se aplicam (fase só de design).
- **Dependências novas:** nenhuma.

## Ficou de fora
- **`Campo PIN` "PIN atual" no `Restaurar backup / Confirmar`:** o app tem e o frame não. A divergência já constava na Fase 47 e não é desta fase.
- **Tamanho ("· 2,4 MB") e item "Pendrive WARD-BACKUP" nos frames antigos `16349`/`16512`:** o app não mostra nenhum dos dois. Só os frames novos seguem o app. Mudar os antigos é decisão do planner.
- **`make check`:** não rodou, por pedido do orchestrator (fase só de design). Conferi que só mudaram o FigCraft, o `design-nodes.md`, a evidência e este handoff.
- **Página Inspirações:** não reli. O arquivo só tem as páginas Design System e Layout, e a fase só acrescentou estados com componentes e tokens que já existiam.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum Ward.figcraft` antes de abrir a cópia e antes de salvar por cima: `a2549658…4771` nas duas vezes | 0 |
| `save_file` em `/home/jeffersonoliveira/documentos/Ward.figcraft` (3 749 010 bytes) | ok |
| `unzip -t` do arquivo salvo e leitura do `document.json`: nós da 39d (`20085`, `20193`, `19978`, `18998`) e da 49b presentes, com os textos conferidos | 0 |
| `git status --short`: só `docs/mission/design-nodes.md`, `docs/mission/evidence/fase-49b/` e este handoff | 0 |

## Problemas descobertos
- **Grave para as próximas fases de design: o MCP `figcraft` headless é compartilhado e guarda abas velhas.**
  - O `open_file` no `Ward.figcraft` caiu numa aba "Ward" que já estava aberta na sessão. Ela estava suja (dirty), com o histórico parado em "39d contagens 22" e **sem** a 39d (sem `Configurações / Trocar …`, `Diálogo de troca` e os `Cartão de nível` dos formulários).
  - O `doc.list` mostra três abas: Untitled, "Ward" suja e outra "Ward".
  - Um `save_file` nessa aba apagaria a 39d do disco. Desfiz nela as 3 mudanças que tinha feito e trabalhei numa cópia do disco aberta por outro caminho.
  - O FigCraft não tem `doc.close`, então a aba velha continua na sessão.
  - Regra sugerida: sempre abrir uma cópia com caminho próprio, conferir o sha256 do original antes de salvar por cima e conferir o resultado no `document.json`.
- **Ids reaproveitados:** na aba velha, `component.addVariant` devolveu `19657`, o mesmo id que a 39d deu a um `Cartão de nível`. Ids só valem dentro do arquivo salvo.
- **Divergências antigas com o app** (frames da Fase 8 e 47): tamanho no item, item de pendrive na lista do restaurar e ausência do "PIN atual" no confirmar. Ver "Ficou de fora".

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório) |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 49b") |
| `docs/mission/evidence/fase-49b/restaurar-backup-versoes-copia-plantada.png` | criado |
| `docs/mission/evidence/fase-49b/restaurar-backup-confirmar-copia-plantada.png` | criado |
| `docs/mission/evidence/fase-49b/restaurar-backup-confirmar.png` | criado |
| `docs/mission/evidence/fase-49b/configuracoes-backup-local-inseguro.png` | criado |
| `docs/mission/evidence/fase-49b/componente-item-de-versao.png` | criado |
| `docs/mission/handoffs/phase-49b.md` | criado |

## Procedimentos
- [ ] `make check` rodado (dispensado pelo orchestrator: fase só de design)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado; textos e datas falsos)
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
