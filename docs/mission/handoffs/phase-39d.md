# Fase 39d — Correções do design do scrutiny do M5 (DESIGN)

## Implementado
Tudo foi feito no `/home/jeffersonoliveira/documentos/Ward.figcraft` original, pelo MCP
`figcraft` headless. Antes de editar, fiz uma cópia de segurança no scratchpad
(`Ward.backup-fase39d.figcraft`, sha256 `382bafe5…0360`). No fim, salvei com `save_file`
no caminho original, reabri com `open_file` e conferi pelo JSON do arquivo, por
`unzip -t` (sem erro) e por screenshot. Os ids estão em `docs/mission/design-nodes.md` ›
"Fase 39d".

- **Contagens (D59, 22 entradas):**
  - a fixture tem acme 13, beta 4, gama 3, delta 2;
  - 26 textos "21 entradas · 4 grupos" viraram "22 entradas · 4 grupos";
  - os resumos filtrados viraram "2 de 22" (Filtrada), "0 de 22" (Sem resultado) e
    "3 de 22" (Bloqueada);
  - nenhum frame mostra o grupo delta, e acme/beta/gama já batiam;
  - no JSON salvo não sobra "21 entradas" nem "de 21".
- **`Cartão de nível` nos 18 formulários e em `Gerador de senha / No formulário`:**
  - os 19 cartões soltos foram apagados e trocados por instâncias do set `18998`, no
    mesmo índice do `corpo`, com largura `fill`;
  - Novo usa `Nível=com aprovação`, com override "Toda entrada nova começa assim. Mudar
    o nível pede o PIN.";
  - Edição usa a variante do nível da entrada na fixture, com o texto do componente
    (o mesmo de `NIVEIS` do app);
  - o botão "Mudar nível" fica `Estado=desabilitado`, como no `Formulario.svelte`;
  - o arquivo agora tem 26 instâncias do cartão e nenhum frame `nível de acesso` solto;
  - o ícone errado dos cartões "Com aprovação" (handoff 37a) sumiu.
- **`15242` (PIN, não senha mestra):**
  - a descrição do PIN de aprovação é "…Trocar pede o PIN atual.", pela propriedade
    `Descrição` da `Linha de opção`;
  - mudei também as três cópias da mesma linha: `16178`, `16349` e `16512`.
- **`3393` (Senha errada):**
  - o `Campo senha` erro mostra o campo vazio (placeholder "Digite a senha mestra" em
    `texto/secundario`);
  - o contorno e a mensagem de erro continuam, como o app faz (handoff 31).
- **`6373` (PIN errado):**
  - no componente `Campo PIN` › `Estado=erro` (`690`), as máscaras ficaram ocultas,
    então as caixas aparecem vazias com o contorno de erro;
  - isso vale também para `Aprovação / PIN errado`, porque a `JanelaAprovacao` também
    esvazia o PIN;
  - no `Diálogo de nível` › `Etapa=PIN errado`, o "Confirmar" ficou desabilitado,
    porque o app desabilita com menos de 4 dígitos.
- **Diálogos "Trocar senha mestra" e "Trocar PIN":**
  - na Design System, a seção nova `Seção / Diálogos de troca` (`19915`) tem o set
    `Diálogo de troca` (`19978`) com as variantes `Tipo=senha mestra` (`19918`) e
    `Tipo=PIN` (`19979`);
  - o set foi feito a partir do `Modal` padrão, com os mesmos tokens, e usa instâncias
    de `Campo senha`, `Campo PIN` e `Botão`, mais o medidor de força do primeiro uso;
  - os textos são os de `TrocarSenha.svelte` e `TrocarPin.svelte`;
  - no Layout ficam `Configurações / Trocar senha mestra` (`20085`) e
    `Configurações / Trocar PIN` (`20193`), em 1280×800, com o fundo de `Configurações /
    Geral` sob a `sobreposição`.
- **PNG:** 30 arquivos em `docs/mission/evidence/fase-39d/`, entre telas novas, telas que
  mudaram e o componente novo.

## Segurança
- Nenhuma invariante de código tocada.
- Só usei dados falsos da fixture (`acme-wp-dev`, `acme-pg-prod` etc.). Senhas e PINs
  aparecem só como máscara, e não há segredo em texto.
- Os diálogos mostram o comportamento do app: o PIN e a senha errados saem da tela, e
  trocar o PIN pede o PIN atual (S9, V5.22).
- Testes de ataque: não se aplicam, porque a fase é só de design.
- Dependências novas: nenhuma.

## Ficou de fora
- **Estados de erro dos diálogos de troca** ("Senha mestra incorreta. Nada foi
  alterado.", "PIN incorreto…" com a nota de aviso): o pedido era desenhar os diálogos,
  e desenhei o estado de preenchimento. Os componentes já têm as variantes de erro, se
  uma fase quiser esses frames.
- **Grupo delta numa lista:** nenhum frame mostra o grupo delta, então não havia
  contagem "2 entradas" a mudar.
- **Seção "Contagens da fixture" da 37a e linha da Cheia em `design-nodes.md`:** citam
  21. São registro histórico, e a seção da 39d corrige o valor.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cp -p Ward.figcraft <scratchpad>/Ward.backup-fase39d.figcraft` + `sha256sum` | 0 |
| MCP `figcraft` headless: `open_file`, `batch`, `run_command`, `save_file` (original) e reabertura | ok |
| `unzip -tq Ward.figcraft` | 0 |
| `FigCraft.AppImage cli export … --node <id>` (30 PNG, `timeout 120` cada; uma 1ª rodada falhou com o disco cheio, veja Problemas) | 0 |
| `make check` (2 rodadas) | 2: só `v3_33_kill_9_entre_as_senhas_erradas` falha (tempo); fmt, clippy e os demais testes passam |
| `make audit` / `make deny` / `make gitleaks` (rodados à parte, porque o `make check` para no teste) | 0 / 0 / 0 |

## Problemas descobertos
- **`make check` sai 2 no teste de tempo `crates/wardd/tests/socket.rs` › `v3_33_kill_9_entre_as_senhas_erradas`**, nas duas rodadas, com a máquina sobrecarregada pelas fases em paralelo (load 39 e 21). Esperas medidas: `[2.24s, 3.10s, 3.66s]` e `[2.30s, 2.59s, 3.23s]`. A 1ª espera (só o Argon2) passa do limite de 1,5 s, e na 2ª rodada a 3ª ficou menos de 1,2 s acima da 1ª. Esta fase não tocou em código Rust, só no `.figcraft` e em docs. É flakiness de tempo sob carga, e convém rodar de novo com a máquina livre. Não mexi no teste, porque está fora do escopo e afrouxar um limite de S9 é decisão de segurança.
- **Disco cheio durante a fase:** a partição `/` (onde ficam `/tmp` e o home) chegou a
  0 MB livres, provavelmente por builds das fases em paralelo. A 1ª rodada de export de
  PNG saiu com arquivos vazios. Apaguei só as minhas extrações temporárias no
  scratchpad e refiz os exports quando o espaço voltou. O `Ward.figcraft` já estava
  salvo antes disso, e o `unzip -t` não acusou erro.
- **`node.set` com `characters` e vários `ids` num `batch` do FigCraft devolve `ok`, mas
  não grava o texto.** Com um id por passo funciona. Pelo `set_properties` com a lista
  de ids também funciona. Quem editar textos em massa precisa conferir.
- **Botão "Mudar nível" desabilitado no formulário:** o design segue o
  `Formulario.svelte`, em que o nível muda só pelo detalhe de Credenciais. Se o planner
  quiser o botão ativo no formulário, o app e o design mudam juntos.
- **"(máximo 4 h)" × D45** (handoff 38): continua no texto do tempo de trava. Não mexi,
  porque é decisão pendente do planner.
- **Ajuda do `Campo senha` em `3393`:** a propriedade "Só a senha mestra destrava. O PIN
  não serve aqui." está com `Mostrar ajuda` desligado no frame. Não conferi se o app
  mostra essa ajuda no erro; fica para uma fase de UI.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório) |
| `docs/mission/design-nodes.md` | alterado (seção Fase 39d; linha de atualização no topo) |
| `docs/mission/evidence/fase-39d/*.png` (30) | criado |
| `docs/mission/handoffs/phase-39d.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): rodado, sai 2 só no teste de tempo V3.33 sob carga; audit, deny e gitleaks limpos à parte
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito e arquivo reaberto)
- [x] Ficou dentro do escopo da fase
