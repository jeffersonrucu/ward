# Fase 11 (ajuste) — evidências do M0 sem literal acusado pelo gitleaks

## Implementado
- `docs/mission/evidence/fase-10/scripts/v0.py`, bloco V0.20: a saída registra só o
  **nome** de cada valor procurado e se foi achado, nunca o literal.
  - `segredos_do_contrato_no_arquivo`: `{nome: {"achado": bool, "formas": [...]}}`
    para os 19 segredos de D15 (nomes como `senha-mestra`, `acme-pg.senha`,
    `beta-aws-dev.secret`). `formas` diz só em que forma apareceu (`literal`, `url`,
    `base64`), sem o valor.
  - `falso_strings`: `{"quantidade", "do_contrato": [nomes], "fora_do_contrato": n}`
    no lugar da lista de literais. O Access Key ID falso fica num mapa separado
    (`ident`), porque é identificador visível pela decisão 5 da Fase 10, não segredo.
    O conjunto dos 19 segredos procurados continua igual ao de antes.
- `docs/mission/evidence/fase-10d/v0-auditoria.json` gerado de novo com o script
  alterado, contra uma cópia do `Ward.figcraft` no scratchpad (só leitura). Antes de
  mudar o script, rodei a versão antiga contra a mesma cópia e a saída ficou
  byte a byte igual ao JSON da 10d. O JSON novo só difere em V0.20: nenhum dos 19
  segredos achado; 2 strings falsas, uma do contrato (`beta-aws-dev.access_key`) e
  uma fora dele (o valor de exemplo do `Campo senha`).
- `docs/mission/evidence/fase-10/v0-auditoria.json`: não tinha o arquivo da época
  para gerar de novo. Converti V0.20 para o formato novo à mão, sem mudar o sentido:
  antes era `{}` e uma única string de exemplo; agora são os 19 nomes com
  `achado: false` e `quantidade: 1, do_contrato: [], fora_do_contrato: 1`. O gitleaks
  não acusava esse arquivo; mudei por consistência, para não sobrar o literal.
- O id do arquivo do Figma virou `<id-do-arquivo-figma>` no campo `fileKey` de
  `evidence/fase-1-inspiracoes.json`, `evidence/fase-4a/inspiracoes-1-3-conferencia.json`
  e `evidence/fase-10/v0-21-inspiracoes.json`.
- `.gitleaks.toml`: removi a allowlist do id do Figma. Ficou só a allowlist dos
  valores de D15, casados inteiros. Nada foi acrescentado.

## Segurança
- S15: com a allowlist só de D15, as três varreduras do gitleaks (árvore, índice e
  histórico) não acham nada. Evidência não repete valor procurado, nem falso.
- Testes de ataque: nenhum novo. Os de `dev/testes/ataques-fundacao.sh` continuam
  passando (13 ok), inclusive `allowlist: valor falso com sufixo ainda é achado`.
- Dependências novas: nenhuma.

## Ficou de fora
- O id do Figma continua em `docs/escopo.md`, `prd.md`, `plan.md`,
  `validation-contract.md`, `design-nodes.md`, handoffs das Fases 1 a 4b e 11 e
  `.claude/agents/worker.md`. O gitleaks não acusa esses arquivos, porque é texto
  corrido, sem padrão chave=valor. O pedido cobria só as evidências e são documentos
  de outras fases e da missão.
- Os handoffs `phase-10.md`, `phase-10b.md`, `phase-10d.md`, `phase-4c.md`,
  `phase-6.md`, `phase-2.md` e `phase-11.md` citam no texto a string de exemplo do
  `Campo senha` e/ou o Access Key ID falso. O gitleaks não os acusa, então não mexi.
- Não alterei código, nem arquivos da Fase 12 (`dev/compose.yml`, fixtures, alvos
  `dev-*`).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I …/v0.py <cópia de Ward.figcraft> design-nodes.md orig.json` + `diff` com o JSON da 10d (script antigo, scratchpad) | 0 (igual) |
| `python3 -I …/v0.py <cópia de Ward.figcraft> design-nodes.md novo.json` (script novo) | 0 |
| `gitleaks dir` da árvore sem a allowlist do Figma (antes de corrigir) | 1 (4 achados: as 3 evidências com `fileKey` e o JSON da 10d) |
| `make check` (final, da raiz) | 0 |
| `make teste-ataques-fundacao` | 0 (13 ok) |

## Problemas descobertos
- Na hora do `make check` final, nenhum arquivo da Fase 12 fez o check falhar.
- Rodando a busca de V0.20 com os nomes do contrato, aparece que o Access Key ID falso
  de D15 está no `Ward.figcraft` (literal e URL-encode). Isso já é esperado pela
  decisão 5 da Fase 10 (identificador visível) e por isso fica em `ident`, fora dos
  19 segredos. Se o orchestrator decidir tratar o Access Key ID como segredo, V0.20
  passa a acusá-lo.
- `docs/mission/handoffs/phase-11.md` ainda descreve a allowlist do Figma, que não
  existe mais. Este handoff a substitui nesse ponto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/mission/evidence/fase-10/scripts/v0.py` | alterado (bloco V0.20) |
| `docs/mission/evidence/fase-10d/v0-auditoria.json` | alterado (gerado de novo) |
| `docs/mission/evidence/fase-10/v0-auditoria.json` | alterado (V0.20 convertido) |
| `docs/mission/evidence/fase-1-inspiracoes.json` | alterado (`fileKey`) |
| `docs/mission/evidence/fase-4a/inspiracoes-1-3-conferencia.json` | alterado (`fileKey`) |
| `docs/mission/evidence/fase-10/v0-21-inspiracoes.json` | alterado (`fileKey`) |
| `.gitleaks.toml` | alterado (sem a allowlist do Figma) |
| `docs/mission/handoffs/phase-11-ajuste.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (o `Ward.figcraft` foi só lido, por cópia)
- [x] Teste de ataque para cada (SEG) tocado (S15: os de `make teste-ataques-fundacao`, 0)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo do ajuste
