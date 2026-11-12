# Fase 10a — (DESIGN) Limpeza de tokens e componentes sem uso

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no `Ward.figcraft` já aberto (`/home/jeffersonoliveira/documentos/Ward.figcraft`), sem `open_file`. Usei ids explícitos e fiz `save_file` antes de cada remoção em lote e no fim (4 no total). O arquivo terminou com `dirty: false`. Não houve timeout de UI.
- **Levantamento de uso** por script (`scratchpad/f10a/usos.py` e `comps.py`, só biblioteca padrão), no `document.json` de uma cópia do arquivo salvo. Contei como uso:
  - ligação em node (`bound`) e em estilo, inclusive campos de efeito;
  - alias em qualquer coleção, inclusive os 8 modos de `Tom do ícone`;
  - modo fixado (`var_modes`).
  - Os ids de modo (`6:3`, `6:4`, `6:5`) colidem com ids de variable (`neutro/0`, `neutro/50`…). Por isso o script lê os campos com estrutura e não faz busca de texto solto. O script também confirmou que nenhuma string igual a um id de variable aparece fora de `bound` ou de alias.
- **Removido** (tudo com zero uso comprovado):
  - `sombra/padrao` (`8:7`). Sua primitiva `sobreposicao/escuro` (`6:53`) continua em uso por `fundo/sobreposicao`;
  - `largura/select` (`VariableID:6`);
  - `espacamento-letra/caixa-alta` (`6:125`). `Texto/Selo` e `Texto/Selo grande` usam `espacamento-letra/normal` e não estão mais em maiúsculas;
  - o set `Chip de filtro` (`5164`, variante `904`, filhos `905`–`907`), que tinha zero instâncias e zero referências. A coluna `900` da `Seção / App` ficou só com o `Cabeçalho de grupo` `901`.
- **`Primitivas`**: as 45 estão em uso, então nenhuma foi removida. A poda de 24 tons já tinha sido feita na 4a. Rodei a checagem de novo depois das remoções e nenhuma primitiva ficou órfã.
- **Estilos**: os 18 estão em uso. **Modos de `Tom do ícone`**: os 8 estão em uso (o `padrão` é o modo padrão; os outros 7 estão fixados em nodes).
- **Conferência visual**:
  - exportei pela CLI headless, sobre cópias no scratchpad, os 75 frames de topo das duas páginas antes e depois das remoções. Os 74 PNGs são idênticos byte a byte. Só a `Seção / App` mudou, como esperado, porque perdeu o chip;
  - no app, conferi por screenshot `3619` (acesso), `4063` (credenciais), `7450` (formulário), `16810` (aprovação prod), `15242` (configurações), `17300` e `17414` (bandeja e extensão) e a coluna `900`. Tudo está normal.
- **Conferência no arquivo salvo**:
  - 226 variables em 8 coleções;
  - 0 ocorrências de `sombra/padrao`, `largura/select`, `caixa-alta` e `Chip de filtro`;
  - nodes e estilos inalterados depois da remoção das variables;
  - depois da remoção do chip, só saíram `904`–`907` e `5164`, e só o `900` mudou (lista de filhos).
- `design-nodes.md` atualizado:
  - nova seção "Fase 10a";
  - linhas do `Chip de filtro`, `sombra/padrao`, `largura/select` e `caixa-alta` atualizadas ou removidas;
  - contagem de variables corrigida.

## Segurança
- Invariantes: nenhuma S1–S15, porque a fase é só de design. Não toquei em cofre, credencial, `~/.ssh` ou `~/.aws`, e não há segredo nos arquivos.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
Estes itens têm zero uso, mas ficaram fora da lista da 10a. São escala ou conjunto que o código pode precisar, então não apaguei. Fica para o orchestrator ou o usuário decidir:
- `peso/regular`, `peso/medio`, `peso/semibold`, `peso/negrito` (`6:105`–`6:108`);
- `tamanho/2xl` (`6:121`) e `altura-linha/2xl` (`6:122`);
- `raio/nenhum` (`6:149`);
- `largura/conteudo` (`VariableID:3`);
- `espaco/80` e `espaco/96` (`VariableID:19`, `20`), criadas na 4a para a D23;
- `grupo/4/cor` e `grupo/5/cor` (`VariableID:75`, `76`). Os tints dos grupos 4 e 5 estão em uso, e apagar só a cor quebraria o conjunto de 5 grupos.

Também ficaram três componentes sem nenhuma instância, todos base do design system: `Toast` `525`, `Modal` `611` e `Logo / Ícone do app` `3292`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não existe antes da Fase 11 (fase só de design) |
| `save_file` (4×) | ok |
| `batch` `variable.delete` ×3 (`8:7`, `VariableID:6`, `6:125`) | ok |
| `edit.delete` `{ids:[5164]}` | ok |
| `python3 -I usos.py` / `comps.py` no arquivo salvo (antes e depois) | ok |
| `FigCraft.AppImage cli export` antes e depois (as 2 páginas) + `cmp` | ok; só `Seção _ App.png` difere |
| `screenshot` de 7 telas e da coluna `900` | ok |

## Problemas descobertos
- Os ids de modo das coleções importadas do Figma (`6:2`…`6:5`) colidem com ids de variable. Quem auditar por busca de texto vai achar falsos usos. É preciso ler `bound`, os alias e `var_modes` com estrutura.
- O plano 10a supunha haver primitivas claras sobrando, mas a 4a já tinha feito essa poda. Aqui foi só a reconferência.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (−3 variables, −set `Chip de filtro`) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-10a.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) no M0
- [x] (UI/DESIGN) Conferido contra o FigCraft (renders antes e depois idênticos; screenshots; `save_file` feito)
- [x] Ficou dentro do escopo da fase
