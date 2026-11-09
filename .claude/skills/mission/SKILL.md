---
name: mission
description: Ponto único de entrada da missão do Ward. Planeja, executa a próxima fase ou valida o milestone conforme o estado de docs/mission/. Use quando o usuário pedir /mission.
disable-model-invocation: false
---

Você é o orchestrator. Não explore código nem implemente: só despache agentes, leia
os retornos e atualize o plano. Assim você fica com o contexto limpo.

Argumento: $ARGUMENTS

Descubra o estado por `docs/mission/plan.md` e siga **um** caso:

## 1. Sem plano → planejar

1. Chame o `planner` com o objetivo (`$ARGUMENTS`; sem argumento, "Planejar a missão
   a partir de docs/escopo.md").
2. Se ele devolver perguntas, faça-as ao usuário **uma por vez** e retome o **mesmo**
   planner (SendMessage) com as respostas. Repita até ele escrever os arquivos.
3. Mostre o resumo do plano e **pare**.

## 2. Plano em `Status: rascunho` → aprovar

Pergunte ao usuário se aprova o plano. Ajustes: retome o planner com eles.
Aprovado: troque para `Status: aprovado` e **pare**.

## 3. Plano aprovado com fase pendente → executar

1. Pegue a primeira fase `- [ ]` (ou a de número `$ARGUMENTS`).
2. Chame o `worker`: "Leia docs/mission/prd.md, plan.md e validation-contract.md. Faça a
   Fase <N>." Se a fase é (DESIGN) e o caminho do `Ward.figcraft` não está no PRD,
   pergunte ao usuário antes e passe o caminho.
3. Leia o handoff. Procedimentos incompletos ou bloqueio: pare e reporte.
4. Marque `- [x]`. Problemas fora do escopo viram novas fases no fim do milestone.
5. Se o milestone fechou, valide **em sequência** (cada um usa o mesmo daemon e o
   mesmo cofre de teste):
   1. `scrutiny-validator`.
   2. `security-auditor`, se o milestone tem asserção (SEG). Pule no milestone 0.
   3. `user-testing-validator`: "Milestone <M>. Modo **milestone**: Playwright Python,
      sem MCP. Evidência em `docs/mission/evidence/M<M>/`."
   - Algum reprovado: crie fases de correção no milestone e pare. Se o milestone já
     reprovou duas vezes, não crie mais fases: pare e chame o usuário.
   - Achado **crítico** do security-auditor: pare e mostre ao usuário mesmo que
     haja fase de correção.
   - Todos aprovados: marque os itens do contrato como `- [x]`.
6. **Milestone 0 (FigCraft)** aprovado pelos validadores: peça ao usuário para revisar
   o `Ward.figcraft` no FigCraft. Só ele fecha o milestone 0. Ajustes viram fases
   (DESIGN).
7. Milestone com UI aprovado: mostre o caminho do `roteiro-usuario.md` e peça ao
   usuário para conferir a janela nativa antes da próxima fase.
8. Reporte em até 5 linhas: fase feita, veredito (se houve), próxima fase. **Pare.**

## 4. Tudo concluído

1. Chame o `security-auditor` com "Auditoria **final**: o produto inteiro, todas as
   invariantes S1–S15." e depois o `user-testing-validator` com "Modo **final**:
   Playwright MCP. Evidência em `docs/mission/evidence/final/`." Reprovado: crie fases
   de correção e pare.
2. Troque para `Status: concluído` e resuma a missão em até 5 linhas.
