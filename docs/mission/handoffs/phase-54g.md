# Fase 54g — RA11 aceito (tipo "terminal" declarado pelo cliente)

## Implementado
- `docs/escopo.md` › "Segurança" › "Fora do modelo de ameaça": item novo "Entrada
  padrão de terminal declarada pelo próprio cliente" (PRD RA11, D68; decisão do usuário,
  2026-11-10: "Aceitar como risco"), logo depois do item do processo que dribla a
  máscara. Diz os dois caminhos que passam sem aviso nem confirmação (cliente do socket
  que mente o tipo; agente com pty próprio, `script -c`/`pty.spawn`), por que o daemon não
  confere (mesmo uid), que o mesmo agente já podia passar o script como arquivo no argv
  (à vista na janela) e o que a 54b fecha: pipe e arquivo com aviso e confirmação
  conferida pelo daemon e sem "1 hora", `nula` cumprida (entrada fechada, dado depois
  mata o comando), o erro sem má intenção e o caso da auditoria, e o deny da skill como
  cinto. Fecha com a alternativa descartada (aviso também em tty) e "mudança que feche o
  caso volta ao usuário".
- `docs/mission/prd.md`:
  - RA11 sai de "proposta, aguarda o usuário" para **fechado em 2026-11-10: risco aceito
    pelo usuário**, registrado na Fase 54g em "Fora do modelo de ameaça", sem código nem
    asserção nova (mesmo formato de RA4/RA8/RA10). O "Proposta: aceitar…; alternativa…"
    virou a decisão e a alternativa descartada.
  - D68: a linha "`terminal` não é conferível… Fica em RA11" passa a dizer "(risco
    aceito, "Fora do modelo de ameaça")".

## Segurança
- Invariantes tocadas: S9 (só documentação). Nenhum comportamento muda; o texto não
  promete mais do que o código da 54b faz (conferido contra o D68 e o handoff da 54b).
- Testes de ataque escritos: nenhum (fase só de documentação; os ataques do D68 são
  V4.39/V4.40, da 54b).
- Dependências novas: nenhuma.

## Ficou de fora
- `docs/mission/plan.md` (linha "RA11 proposto" no "Feita" da 54b e a própria 54g):
  é do orchestrator.
- Handoffs antigos (54b, 54f) que citam RA11 como aberto: são histórico, não editei.
- `crates/wardd/src/uso.rs:30-31` cita "(D68, RA11)" sem dizer "proposta": não precisa
  mudar, e código está fora desta fase.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `grep -rn RA11 docs skill dev crates app` (conferir citações) | 0 |

Não rodei cargo, `make check` nem nenhum alvo de teste (pedido do orchestrator: a 54l
roda `make check` em paralelo). Nenhum alvo de teste de documentação se aplica a esta
fase: `make teste-skill` e `make teste-migracao` leem `skill/` e `docs/migracao.md`, que
não mudaram (e o `teste-skill` exige build). O gitleaks do `make check` da próxima
rodada cobre o diff (nenhum segredo nele).

## Problemas descobertos
- `docs/escopo.md` › "Fora do modelo de ameaça", item "Crash do Chrome com a senha
  preenchida": o parágrafo "Vale também para o próprio agente (ameaça 1)… o `ward-nm`
  setgid e não-dumpable." e o subitem "pipes do Chrome por `/proc/<pid>/fd`" estão
  depois desse item, mas falam do RA10 (processo que se passa pelo Chrome). Parece que
  o item do crash foi inserido no meio do item do RA10. Não mexi (fora do escopo da
  fase; o escopo é fonte de verdade do usuário); vale uma fase de doc que devolva o
  texto ao item do RA10.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/escopo.md` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/handoffs/phase-54g.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não: pedido do orchestrator, a 54l roda em paralelo)
- [x] Só cofre de teste e credenciais falsas usados (nenhum usado)
- [x] Teste de ataque para cada (SEG) tocado (não se aplica: só documentação)
- [x] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
