# Fase 1 — (FIGMA) Inspirações lidas e tokens como variables

## Implementado
- Sessão do Figma criada no arquivo `bzu9PxxFur2tU3vlNCHvsY`.
- Página Inspirações (`1:3`) lida **só com leitura** (nenhuma escrita nela). Snapshot dos
  filhos salvo em `docs/mission/evidence/fase-1-inspiracoes.json` (para V0.21): 4
  retângulos com imagem (`2:3` image 1, `2:9` image 3, `2:12` image 4, `2:15` image 5),
  com posição, tamanho, rotação, visibilidade, trava, opacidade e `imageHash`, além do
  script de conferência. Reconferido no fim da fase: idêntico.
- Referências observadas: RoboForm (escuro, grade de cartões), Bitwarden (escuro, lista +
  detalhe com campo de senha mascarado e ações copiar/revelar), Defender (claro, limpo) e
  uma tela de login. Direção adotada: neutros frios escuros, primária azul-índigo, raios
  médios, Inter.
- Coleções de variables (todas com `scopes` explícitos e code syntax WEB `var(--…)`):

| Coleção | Id | Modos | Variables |
| --- | --- | --- | --- |
| `Primitivas` (apoio, escondida dos pickers: `scopes = []`) | `VariableCollectionId:6:2` | `Valor` | 51 (neutro, azul, vermelho, âmbar, verde, ciano, sobreposição) |
| `Cores` | `VariableCollectionId:6:54` | **`Escuro` (único/padrão, modeId `6:2`)** | 43 semânticas, todas alias das Primitivas |
| `Tipografia` | `VariableCollectionId:6:98` | `Valor` | 27 |
| `Espaçamento` | `VariableCollectionId:6:126` | `Valor` | 21 |
| `Raio` | `VariableCollectionId:6:148` | `Valor` | 7 |

- `Cores` cobre: `fundo/{padrao,sutil,sobreposicao}`,
  `superficie/{padrao,elevada,campo,hover,selecionada,desabilitada}`,
  `texto/{primario,secundario,desabilitado,inverso,marca}`, `borda/{padrao,forte,foco}`,
  `primaria/{padrao,hover,sutil,sobre}`, `perigo/{padrao,hover,sutil,texto,sobre}`,
  `aviso/{padrao,sutil,texto}`, `sucesso/{padrao,sutil,texto}`,
  `ambiente/{dev,stage}/{fundo,texto,borda}` e
  `ambiente/prod/{fundo,texto,borda,destaque,sobre}` (prod tem cor sólida de destaque
  para a janela de aprovação). dev = ciano, stage = âmbar, prod = vermelho.
- `Tipografia`: `familia/texto` = Inter, `familia/mono` = JetBrains Mono (comando exato
  na aprovação), `estilo/*` (Regular, Medium, Semi Bold, Bold), `peso/*` (400–700),
  `tamanho/{xs,sm,base,md,lg,xl,2xl}` = 12/13/14/16/18/22/28,
  `altura-linha/*` = 16/18/20/24/26/30/36, `espacamento-letra/{normal,titulo,caixa-alta}`.
- `Espaçamento`: `espaco/{0,2,4,6,8,12,16,20,24,32,40,48,64}` (GAP),
  `controle/{sm,md,lg}` = 32/40/48 e `icone/{sm,md,lg}` = 16/20/24 (WIDTH_HEIGHT),
  `linha/{fina,grossa}` = 1/2 (STROKE_FLOAT).
- `Raio`: `raio/{nenhum,sm,md,lg,xl,2xl,total}` = 0/4/6/10/14/20/9999.
- Amostra visual na página Design System: frame `Tokens / Cores` (`6:156`) com a coluna
  `Cores / Escuro` (`6:331`), cada linha com amostra ligada à variable, nome e code
  syntax. Tudo ligado a variables (cores, padding, gap, raio, espessura de borda).

### Mudança de decisão aplicada durante a fase (pedido do coordinator)
- O usuário decidiu **só tema escuro**. A coleção `Cores` foi criada inicialmente com
  `Claro` e `Escuro`; o modo `Claro` foi **removido** e `Escuro` ficou como modo único e
  padrão. A coluna `Cores / Claro` da amostra (`6:157`) foi apagada.
- `prd.md`, `plan.md` e `validation-contract.md` **não** foram editados (fica com o
  orchestrator/planner).

## Segurança
- Invariantes tocadas: nenhuma S1–S15 diretamente (fase só de design). S15: nenhum
  segredo em arquivo novo; o snapshot só tem ids, nomes e hashes de imagem do Figma.
- Testes de ataque: não se aplica (nenhum (SEG) do M0).
- Dependências novas: nenhuma. Fontes escolhidas (Inter, JetBrains Mono) são OFL e
  deverão ser **empacotadas localmente** no app/extensão (CSP sem origem remota, S11);
  nada de Google Fonts em runtime.

## Ficou de fora
- Text styles e effect styles (sombra de modal): ficam para a Fase 2 junto com os
  componentes, que é quem os usa.
- Componentes e telas (Fases 2+).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: ainda não existe `Makefile` (criado na Fase 11) |
| `use_figma` leitura de `1:3` (antes e depois) | ok, idêntico |
| `use_figma` criação das 5 coleções e da amostra | ok |

## Problemas descobertos
- **Divergência com o escopo/contrato (tema)**: `docs/escopo.md` (Missão 0, Pendências),
  PRD D1 e o contrato V0.1 ("`Cores` (modos `Claro` e `Escuro`)") e V0.19 ("modo
  `Escuro`") ainda falam de dois modos. O arquivo agora tem só `Escuro`. V0.1 vai falhar
  como está escrito até o planner ajustar D1/V0.1/V0.19 e o escopo. Não escolhi lado nos
  documentos.
- Com tema único, V0.19 ("cada grupo tem um frame no modo Escuro") vira trivial; as
  fases 3, 6, 7, 8, 9 ainda citam "uma no modo Escuro" no plano.
- A página `1:3` se chama **"Inspiration"** no arquivo, não "Inspirações" como no
  escopo/PRD. O id bate; só o nome difere. Não renomeei (página é só leitura).
- Existe uma 5ª coleção, `Primitivas`, além das quatro do contrato. Ela é de apoio
  (variables escondidas dos pickers) e as semânticas de `Cores` são alias dela. Se o QA
  exigir exatamente quatro coleções, registrar; o contrato diz "existem", não "só".
- Primitivas de tom claro (ex.: `neutro/50`, `azul/50`, `sobreposicao/claro`) ficaram
  sem uso após a remoção do modo Claro. Mantidas por ora; podem ser podadas na Fase 10.
- O plano Figma do arquivo aceitou dois modos (não é Free); irrelevante agora com modo
  único.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/mission/evidence/fase-1-inspiracoes.json` | criado |
| `docs/mission/handoffs/phase-1.md` | criado |

Nodes/ids no Figma:
| Item | Id |
| --- | --- |
| Frame `Tokens / Cores` (página Design System `1:2`) | `6:156` |
| Frame `Cores / Escuro` (dentro do anterior) | `6:331` |
| Coleção `Primitivas` | `VariableCollectionId:6:2` |
| Coleção `Cores` (modo `Escuro` `6:2`) | `VariableCollectionId:6:54` |
| Coleção `Tipografia` | `VariableCollectionId:6:98` |
| Coleção `Espaçamento` | `VariableCollectionId:6:126` |
| Coleção `Raio` | `VariableCollectionId:6:148` |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado — nenhum (SEG) tocado
- [x] (UI/FIGMA) Conferido contra o Figma (screenshot da amostra; Inspirações reconferida)
- [x] Ficou dentro do escopo da fase
