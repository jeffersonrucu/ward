# Fase 44d — Design: janela de aprovação com origem `http`

## Implementado
- **Layout** (linha y 4000, ao lado dos outros `Aprovação / *`; 560 de largura, altura `hug`):
  - `Aprovação / Origem http / Confirmar` `22733` (x 10400): `ward playwright
    acme-wp-prod` no nível livre (prod). Cabeçalho com contagem e fechar; `Destaque`
    prod; **`Destaque` aviso** com o texto de `origemDestaque`; resumo "Confira a
    entrada e a origem antes de confirmar."; pedido com `acme-wp-prod` (login, prod),
    nível "confirmar a origem", `Caixa de código` "Origem que recebe usuário e senha" =
    `http://localhost:8081` e "Quem pediu" `claude → bash → ward playwright`; sem campo
    de PIN; só "Negar" (secundário, com foco) e "Confirmar a origem" (primário); nota
    `notaConfirmacao`.
  - `Aprovação / Origem http / Popup` `22817` (x 11040): o mesmo pedido vindo do popup,
    com o subtítulo "A extensão do Chrome quer preencher uma senha" (quebra em duas
    linhas, como no app) e "Quem pediu" `chrome → ward-nm`.
  - `Aprovação / Origem http / PIN` `22882` (x 11680): `ward login acme-wp-dev` (com
    aprovação, dev): aviso de `http` logo abaixo do cabeçalho, origem no lugar do
    comando, `claude → bash → ward login`, `Campo PIN` e os quatro botões de sempre.
- Tudo por instância de componentes que já existiam (`Destaque` aviso `18684`,
  `Caixa de código` `16340`, `Botão`, `Campo PIN`, `Badge tipo`/`Badge ambiente`,
  `Ícone/tipo-login` `219`). Nenhum componente, estilo ou variable novo.
- `docs/mission/design-nodes.md`: seção "Fase 44d" com os ids e a tabela das
  diferenças para o app.
- PNG (exportados pela CLI do arquivo salvo no disco):
  - `/home/jeffersonoliveira/projetos/cofre/.claude/worktrees/agent-a08c83702ef819445/docs/mission/evidence/fase-44d/aprovacao-origem-confirmar.png`
  - `/home/jeffersonoliveira/projetos/cofre/.claude/worktrees/agent-a08c83702ef819445/docs/mission/evidence/fase-44d/aprovacao-origem-popup.png`
  - `/home/jeffersonoliveira/projetos/cofre/.claude/worktrees/agent-a08c83702ef819445/docs/mission/evidence/fase-44d/aprovacao-origem-pin.png`

## Segurança
- **Invariantes tocadas:** nenhuma em código. O desenho segue S1/S9/S10 da 44c: a
  origem fica à vista em caixa própria; na confirmação não há PIN nem "permitir"; o foco
  inicial é "Negar"; o pedido não mostra segredo (só nome da entrada, origem e cadeia).
- **Decisão de design por segurança:** o aviso de `http` passou de `Destaque` neutro
  (app) para `Destaque` **aviso** (âmbar). A 44c registrou como risco residual o
  usuário que confirma sem olhar; o aviso precisa se destacar da nota neutra do rodapé.
- **Testes de ataque:** não se aplicam (fase só de design).
- **Dependências novas:** nenhuma.
- Valores falsos do cofre de teste (`acme-wp-prod`, `acme-wp-dev`,
  `http://localhost:8081`). Nenhum padrão de `.dev/variacoes.txt` no `document.json` do
  arquivo salvo nem no `design-nodes.md` (`grep -F -f`, 0 linhas).

## Ficou de fora
- **Ajuste do app aos três textos/tom novos** (ver Problemas): é UI, não esta fase.
- **Confirmação `http` com prod em "com aprovação"** e estados de erro/expiração com
  origem: o plano pede só a confirmação e o pedido com PIN; os estados de erro são os
  mesmos dos frames `Aprovação / PIN errado`/`Expirando`.
- **`make check`:** não rodado. A fase não muda código e o disco está apertado (3 GB
  livres; pedido do orchestrator para não compilar). Só `.md` e `.png` mudaram.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do `Ward.figcraft` antes de copiar e antes de gravar (iguais: `7efbd69b…`, o da 53c) | 0 |
| `save_file` na cópia do scratchpad; `cp` + `mv` da cópia sobre o original (só se o sha256 ainda fosse `7efbd69b…`) | 0 |
| `sha256sum` original = cópia (`3a2afbfb…`); `unzip -t` do original | 0 |
| `unzip -p … document.json` (3 frames da 44d e frames da 49b/53c presentes) | 0 |
| `FigCraft.AppImage cli export <Ward.figcraft> --node 22733/22817/22882` | 0 |
| `grep -c -F -f .dev/variacoes.txt` no `document.json` e no `design-nodes.md` | 1 (nenhuma linha) |
| `make check` | não rodado (ver "Ficou de fora") |

## Problemas descobertos
- **O app diverge do design em três pontos** (fase de UI depois desta, em
  `app/src/telas/JanelaAprovacao.svelte` e `aprovacao.js`):
  1. `origemDestaque` com `tipo="aviso"` (hoje neutro, igual à nota do rodapé);
  2. `prodDestaque` na confirmação diz "confira o comando com atenção", mas a
     confirmação não tem comando. No design: "Credencial de produção. Confira a origem
     com atenção antes de confirmar.";
  3. `resumo` no pedido com PIN e origem diz "o comando exato", e não há comando. No
     design: "Confira a entrada, a origem e quem pediu antes de permitir.".
- **Daemon falso:** `na_origem` (`app/nucleo/src/daemon_falso.rs`) põe
  `Ambiente::Prod` em todo pedido, inclusive em `aprovacao-origem-pin` com
  `acme-wp-dev`, que no cofre de teste é dev. O frame usa dev. Quem conferir a tela pelo
  modo `aprovacao-origem-pin` verá o selo prod e a borda vermelha, diferentes do frame.
- **Nível na confirmação:** a tela mostra "Nível: confirmar a origem", mas o nível da
  entrada é "livre". Mantive o texto do app. Fica a pergunta se o rótulo devia ser
  "livre · confirmar a origem"; não escolhi.
- **Abas velhas no MCP headless:** abri a cópia do scratchpad (aba nova, índice 5) e
  nunca salvei de outra aba. Gravação no original só por `cp` da cópia salva, com o
  sha256 conferido antes.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório; sha256 `7efbd69b…` → `3a2afbfb…`) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-44d/aprovacao-origem-confirmar.png` | criado |
| `docs/mission/evidence/fase-44d/aprovacao-origem-popup.png` | criado |
| `docs/mission/evidence/fase-44d/aprovacao-origem-pin.png` | criado |
| `docs/mission/handoffs/phase-44d.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não rodado, a fase não toca código e o disco está apertado
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado em código)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito na cópia, posta no lugar do original e conferida pelo disco)
- [x] Ficou dentro do escopo da fase
