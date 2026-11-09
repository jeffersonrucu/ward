# Fase 10d — (DESIGN) Ajustes do scrutiny e do QA do M0

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no `Ward.figcraft` que já estava aberto, sem `open_file`. Não houve timeout de UI. Fiz 3 `save_file`; o último veio depois da última mudança e o app ficou com `dirty: false`. O arquivo salvo foi reaberto pela CLI headless (`cli info`) e auditado numa cópia com o mesmo sha256 (`dadd7a7d…0081`).
- **Scrutiny:**
  - **Role ARN nos formulários aws:** a ajuda oculta copiada do `api` ("O token vai nesta variável e em WARD_TOKEN.") virou "Vai em AWS_ROLE_ARN no ward run." (prop `Ajuda` das instâncias `12015` e `12512`; os textos são `12027` e `12524`). O plano citava `10894`/`11594`, mas esses são a ajuda legítima do `api`, que é a origem da cópia. Esses dois ficaram como estavam.
  - **Handoff da Fase 10:** a linha V0.20 agora diz que os valores falsos do arquivo são `falso-exemplo-9f3k` e `FALSOACCESSKEY01` (Access Key ID falso, visível pela decisão 5) e que nenhum dos dois é segredo do cofre de teste. A instrução de uso dos scripts também mudou.
  - **Scripts de evidência** (`evidence/fase-10/scripts/`):
    - `v0.py` e `forms.py` acham o `lib.py` pelo próprio caminho (`__file__`) e ligam `sys.dont_write_bytecode`, então não geram mais `__pycache__`;
    - `forms.py` exige o arquivo como argumento e grava em `[saida.json]`, ou ao lado do script se não houver saída;
    - `lib.load` não tem mais caminho padrão;
    - o regex de V0.20 passou a pegar também `FALSO[A-Z0-9]+`;
    - apaguei a pasta `__pycache__`;
    - rodei os dois scripts a partir de `/tmp`, sem `__pycache__` novo (rc 0).
- **QA do M0:**
  - **Copiar em todos os campos não secretos do detalhe:** liguei `Mostrar copiar` em Porta e Base no detalhe de Credenciais / Cheia, Filtrada e Filtros e de Nível de acesso / Escolher, PIN e PIN errado. Também em Porta (banco, servidor, smtp, ftp/sftp), Base (banco) e Nome da variável (api) nos formulários de Edição.
  - **Esmaecer na lista:**
    - componente novo `Esmaecer rolagem` (`18948`, na `Seção / Item de lista`), um gradiente de `fundo/sutil-transparente` para `fundo/sutil` com altura `espaco/48`;
    - 7 instâncias, absolutas no fim de `itens`, nas listas cheias (12 entradas, 7 visíveis): Credenciais / Cheia e Filtros, Nova credencial / Escolher tipo e Tipo escolhido, e Nível de acesso / Escolher, PIN e PIN errado.
  - **Rodapé cortando o cartão de nível:** o corpo de 4 formulários passava do rodapé. Os frames ficaram mais altos: `login / Novo` 1370→1404, `banco / Novo` 1436→1454, `aws / Novo` 1462→1478, `ftp/sftp / Novo` 1462→1470.
  - **Dados de exemplo coerentes:**
    - a máscara do gerador agora tem 24 pontos, como diz "24 caracteres" (`13822`, `15144`);
    - as 5 aprovações e o `Destravar / Pedido do agente` usam a entrada `acme-pg-aprovacao` (stage, banco, com aprovação), que está na lista. A aprovação prod usa `acme-pg-prod`. O nome e o `ward run …` foram trocados.
  - **PIN errado com comprimento variável:** `Aprovação / PIN errado` mostra 4 dígitos e `Nível de acesso / PIN errado` mostra 5. As máscaras sobrando ficaram ocultas na instância, e as caixas continuam com a borda de erro.
  - **Tempo de trava:** o `Select` da variante `Controle=select` da `Linha de opção` passou a usar `largura/select-compacto` (120, variable nova) em vez de `largura/select-filtro` (160). A coluna de texto foi de 124 para 166 px nas 4 telas que usam o card (Configurações ×2, Restaurar ×2).
  - **Respiro entre "De … para …" e o PIN:** o `gap` do `corpo` do `Diálogo de nível` passou de `espaco/12` para `espaco/20`, nas variantes PIN errado (`1097`) e PIN (`1046`).
- **Design System:** a `Seção / Item de lista` cresceu com o componente novo e já sobrepunha a `Seção / Modal` antes da fase. Desci 7 seções em 364 (posições em `design-nodes.md`). A descrição `552` cita o `Esmaecer rolagem`.
- **`design-nodes.md`:** nova seção "Fase 10d", com ids novos, alterados e posições. A auditoria (`evidence/fase-10d/v0-auditoria.json`) dá V0.2 sem valores soltos nos 51 frames e V0.20 sem segredo do contrato. Em V0.22, todo frame e componente tem id e os ausentes são só os já marcados como apagados.

### Frames alterados
- **Layout:**
  - Credenciais / Cheia `4063`, Filtrada `4410`, Filtros `5259`;
  - Nova credencial / Escolher tipo `17657`, Tipo escolhido `18055`;
  - Nível de acesso / Escolher `5615`, PIN `5994`, PIN errado `6373`;
  - Formulário / login / Novo `7450`, banco / Novo `8537`, banco / Edição `7908`, servidor / Edição `8981`, smtp / Edição `9454`, ftp/sftp / Novo `14743`, ftp/sftp / Edição `10895`, api / Edição `10389`, aws / Novo `12170`, aws / Edição `11705`;
  - Gerador de senha / Avulso `13141`, No formulário `14307`;
  - Aprovação / Normal `16701`, prod `16810`, PIN errado `16885`, PIN bloqueado `16960`, Expirando `17035`;
  - Destravar / Pedido do agente `3450`;
  - Configurações / Geral `15242` e Backup externo ligado `16178`, Restaurar backup / Versões `16349` e Confirmar `16512`. Nesses quatro a mudança vem do componente `Linha de opção`.
- **Design System:**
  - `Seção / Item de lista` `550`, com o componente novo `18948`;
  - componentes `Linha de opção` / `Controle=select` `10825` e `Diálogo de nível` (variantes `1078`, `1027`);
  - posições de `608` `647` `716` `760` `3286` `5219` `6796`.

## Segurança
- **Invariantes:** a fase é só de design e não toca nenhuma (SEG).
  - Os campos que ganharam Copiar não são secretos (porta, base, nome da variável).
  - A máscara do gerador mostra o comprimento porque a própria tela já mostra "24 caracteres". Nas outras telas a máscara continua fixa e não revela o comprimento.
  - Nenhum segredo do cofre de teste aparece no arquivo (V0.20).
- **Testes de ataque:** não se aplicam, porque o M0 não tem asserção (SEG).
- **Dependências novas:** nenhuma. Os scripts continuam só com a biblioteca padrão.

## Ficou de fora
- **`.gitignore` com `__pycache__`:** fica com a Fase 11, como foi combinado. Não editei `.gitignore`, `CLAUDE.md` nem `Cargo.toml`.
- **Corpo 2 px maior que o espaço** em `login / Edição`, `api / Novo`, `api / Edição`, `banco / Edição`, `servidor / Edição` e `servidor / Novo`: come 2 px dos 24 de padding inferior e não corta nada visível, então não mexi.
- **Exemplos da Design System com `clientex / stage / banco`** (`Seção / Item de lista`, `Seção / Aprovação` `16342`, `Seção / Modal`): são amostras de componente, fora das telas. Ficaram.
- **Listas altas dos formulários** (7 itens com espaço sobrando): não estão cheias, então ficaram sem esmaecer.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não rodado: a Fase 11 (em paralelo) ainda cria o `Makefile`; fase só de design |
| `variable.create` ×3, `variable.update` ×3, `create_node` (componente), `variable.bind`, `instance.create` ×7, `instance.setProperties`, `node.set` (batches `10d: …`) | ok |
| `save_file` (3×) | ok |
| `python3 -I …/v0.py final.figcraft design-nodes.md v0-10d.json` (rodado em `/tmp`) | 0 |
| `python3 -I …/forms.py final.figcraft forms-10d.json` (rodado em `/tmp`) | 0 |
| `FigCraft.AppImage cli info` na cópia salva | 0 |

## Problemas descobertos
- O plano cita `10894`/`11594` como ids do Role ARN, mas os nós corrigidos são `12015`/`12512`, nos formulários aws. Os ids do plano são a ajuda do `api`, que está correta.
- A seção "Posições (Fase 4d)" do `design-nodes.md` já estava desatualizada antes desta fase. As posições atuais da coluna x = 640 estão na seção 10d.
- JetBrains Mono continua sem instalar (como antes); os exports usam a fonte de reserva.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/handoffs/phase-10.md` | alterado (frase de V0.20 e uso dos scripts) |
| `docs/mission/handoffs/phase-10d.md` | criado |
| `docs/mission/evidence/fase-10/scripts/v0.py` | alterado |
| `docs/mission/evidence/fase-10/scripts/forms.py` | alterado |
| `docs/mission/evidence/fase-10/scripts/lib.py` | alterado |
| `docs/mission/evidence/fase-10/scripts/__pycache__/` | apagado |
| `docs/mission/evidence/fase-10d/v0-auditoria.json` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): o `Makefile` ainda não existe (Fase 11 em paralelo)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) no M0
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots no app, cópia salva reaberta pela CLI; `save_file` feito)
- [x] Ficou dentro do escopo da fase
