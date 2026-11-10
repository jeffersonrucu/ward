# Fase 54m — Ordem do RA10 no escopo

## Implementado
- `docs/escopo.md` › "Fora do modelo de ameaça": o parágrafo "Vale também para o próprio
  agente (ameaça 1)…" e o subitem "**pipes do Chrome por `/proc/<pid>/fd`**" voltaram
  para o item do RA10, logo depois de "Os dois dão o que a extensão já pode pedir…" e
  antes de "Crash do Chrome com a senha preenchida". Só movimento de linhas (10 linhas do
  item do crash trocaram de lugar com as 16 do RA10); nenhuma palavra mudou.

## Segurança
- Nenhuma invariante tocada (só ordem de texto). Sem testes nem dependências novas.

## Ficou de fora
- Nada.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `diff <(sort escopo antigo) <(sort escopo novo)` (mesmas linhas, mesma contagem: 549) | 0 |
| `git diff --word-diff=porcelain docs/escopo.md` (cada linha `-` tem a `+` idêntica) | 0 |
| `make check` | não rodado (pedido do orchestrator: a 54l roda em paralelo; fase só de doc) |

## Problemas descobertos
- Nenhum.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/escopo.md` | alterado |
| `docs/mission/handoffs/phase-54m.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não: pedido do orchestrator, fase só de texto)
- [x] Só cofre de teste e credenciais falsas usados (nenhum usado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum tocado)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
