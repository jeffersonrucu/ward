# Fase 54f — Aprovação com entrada padrão oculta (DESIGN)

## Implementado
- Dois frames novos na Layout do `Ward.figcraft`, na linha y 4000 das aprovações, 560 de
  largura e altura `hug` (1002, a mesma da captura do app na 54b). Os dois saíram de
  uma cópia de `Aprovação / Normal` (`16701`):
  - `Aprovação / entrada padrão` (`22964`, x 12320), com a caixa desmarcada:
    - `Destaque entrada padrão` (`23039`), instância do `Destaque` aviso `18684` com
      `Ícone/alerta`, logo abaixo do cabeçalho, com o texto de
      `aprovacao.js` › `entradaDestaque`.
    - `pedido` com "Comando" `"python3"`, "Diretório de trabalho"
      `"/home/dev/projetos/acme"` (nova `Caixa de código` `23048`) e "Quem pediu".
    - `confirmação da entrada` (`23044`): `Interruptor` `Ligado=não` (`23045`) mais o
      rótulo "Sei o que vai pela entrada padrão e permito mesmo assim" (`23047`, estilo
      `8:16`, fill `6:64`, gap `espaco/8`), entre o pedido e o `Campo PIN`, como no app.
    - `Campo PIN` vazio. "Negar" fica em foco e os três "Permitir" ficam em
      `Estado=desabilitado`.
  - `Aprovação / entrada padrão / marcada` (`23051`, x 12960): o mesmo frame com o
    `Interruptor` `Ligado=sim` (`23101`), o `Campo PIN` completo (`23104`), "Permitir
    uma vez" primário e os outros botões secundários.
- Tudo por instância da Design System (`Destaque`, `Interruptor`, `Caixa de código`,
  `Campo PIN`, `Botão`). Nenhum componente ou variable novo. A comparação com
  `docs/mission/evidence/fase-54b/` bateu em ordem, textos, estados e altura.
- `docs/escopo.md` › "Janela de aprovação": entrou um item sobre o aviso, a confirmação
  a mais, a conferência pelo daemon e o "1 hora" que não cobre esses pedidos. Nenhuma
  outra seção foi tocada.
- Os ids estão em `docs/mission/design-nodes.md` › "Fase 54f".
- Como salvei: copiei o `Ward.figcraft` para o scratchpad e abri a cópia numa aba nova
  do headless (índice 10). Editei, conferi que o sha256 do original não tinha mudado
  (`1f397e4b…bf5`) e salvei por cima com `save_file` (`24addb5f…7a2`). Na conferência:
  `unzip -t` sem erro; o `document.json` tem os dois frames e o aviso da 54d (nada da
  54d se perdeu); os PNG foram exportados do arquivo em disco pela CLI
  (`cli export --node`).

## Segurança
- Invariantes tocadas: nenhuma de código. O design documenta a D68 e a S9: a caixa é só
  a interface e quem decide é o daemon (o escopo agora diz isso). O arquivo não tem
  segredo; o PIN aparece só como máscara.
- Testes de ataque: não se aplica (fase só de design; os ataques da D68 são da 54b).
- Dependências novas: nenhuma.

## Ficou de fora
- Código do app: a tela já existe desde a 54b. O comentário "sem frame no FigCraft
  ainda", em `aprovacao.js` e `JanelaAprovacao.svelte`, pode passar a citar `22964`
  numa fase de UI; não mexi por ser código.
- Nada foi compilado, `make check` não rodou e nada foi commitado, por instrução do
  orchestrator.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do original antes de salvar (igual ao da cópia) | 0 |
| `unzip -t` e `unzip -p … document.json` no arquivo salvo | 0 |
| `FigCraft.AppImage cli export … --node 22964` / `--node 23051` | 0 |
| `make check` | não rodado (instrução do orchestrator) |

## Problemas descobertos
- **"Diretório de trabalho" falta nos outros frames de aprovação.** O app mostra essa
  caixa em todo `executar` desde a 26a. Os frames `Aprovação / Normal`, `prod`,
  `PIN errado`, `PIN bloqueado` e `Expirando` (`16701`, `16810`, `16885`, `16960`,
  `17035`) não têm a caixa; só os dois frames novos têm. É uma divergência anterior,
  que fica para uma fase de DESIGN.
- **Foco do `Campo PIN` vazio.** A variante `vazio` (`651`) desenha o primeiro dígito
  em foco. No app, quem tem o foco ao abrir é "Negar". O frame novo segue os outros
  frames de aprovação (o mesmo componente) e não escolhe um lado.
- A RA11 da 54b (o tipo "terminal" é declarado pelo cliente) continua aberta e aguarda
  o usuário. O escopo novo descreve o comportamento atual (pipe ou arquivo).
- O headless do FigCraft continua acumulando abas (11 agora). As abas 1–4 e 7 usam o
  caminho original, e a 1 está suja com a 39d.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `docs/escopo.md` | alterado (só "Janela de aprovação") |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 54f") |
| `docs/mission/evidence/fase-54f/aprovacao-entrada-padrao.png` | criado |
| `docs/mission/evidence/fase-54f/aprovacao-entrada-padrao-marcada.png` | criado |
| `docs/mission/handoffs/phase-54f.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução do orchestrator)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada em código)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
