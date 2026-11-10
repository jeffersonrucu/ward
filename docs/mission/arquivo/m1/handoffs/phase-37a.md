# Fase 37a — Correções do design (D65 f e design pendente da Fase 32) (DESIGN)

## Implementado
Tudo foi feito no `/home/jeffersonoliveira/documentos/Ward.figcraft` original, pelo MCP
`figcraft` headless. Antes de editar, fiz uma cópia de segurança no scratchpad
(`Ward.backup-fase37a.figcraft`, sha256 `dadd7a7d…0081`). No fim, salvei com `save_file`
no caminho original, reabri e conferi pelo JSON do arquivo e por screenshot. Os ids
estão em `docs/mission/design-nodes.md` › "Fase 37a".

- **Símbolos (D35, D65 f)**, com o mesmo texto de `ward_core::gerador::SIMBOLOS`,
  `!#$%&()*+,-./:;<=>?@[]^_{|}~`:
  - `Gerador de senha / Avulso`: a descrição `13815` passa a mostrar o conjunto e, na
    linha de baixo, "Desligue se o painel não aceitar.".
  - `Popover do gerador` (componente `11671`):
    - o rótulo `12124` passa de "Símbolos (!@#$%)" para "Símbolos";
    - texto novo `descrição símbolos` `18956` no fim de `conjuntos`, com estilo `8:14`
      e cor `6:65`;
    - a instância `15135` de `Gerador de senha / No formulário` herda as duas mudanças.
  - Nos três textos, `liga` e `calt` estão desligados. Sem isso, a Inter desenha `<=>`
    como ⇔, igual ao que a Fase 37 achou no app.
- **Componente `Cartão de nível`** (set `18998`, variantes `Nível=livre` `18986`,
  `Nível=com aprovação` `18999` e `Nível=bloqueado` `19011`):
  - fica na seção nova `Seção / Cartão de nível` `18958` da Design System;
  - foi feito a partir do cartão solto de `Credenciais / Cheia`, com os mesmos tokens;
  - o bloqueado usa `Ícone/olho-fechado`, o mesmo ícone do `Diálogo de nível`, e o texto
    de `NIVEIS.bloqueado` do app (Fase 32);
  - os seis cartões soltos dos detalhes de Credenciais e de Nível de acesso viraram
    instâncias.
- **Frame `Credenciais / Sem resultado`** (`19102`, x 5440, y 2000):
  - busca "mysql" com Tipo banco e ambiente prod, e "0 de 21 entradas · filtros
    ativos";
  - na lista, "Nenhuma entrada com essa busca ou esses filtros.";
  - no detalhe, o estado vazio com `Ícone/busca`, o título "Nenhuma entrada
    encontrada" e o texto "Limpe a busca ou os filtros para ver todas as entradas.";
  - os textos são os da tela da Fase 32.
- **Frame `Credenciais / Bloqueada`** (`19359`, x 4080, y 3000):
  - busca "gama", com o grupo gama ("3 de 3 entradas");
  - `gama-bloqueada` selecionada (login, prod, URL `https://bloqueada.invalid`,
    usuário `x`, como na fixture);
  - no detalhe, a instância `Nível=bloqueado` `19574`.
- **Contagens da fixture**:
  - `dev/fixtures/entradas.json` tem 21 entradas: acme 13, beta 4, gama 3, delta 1;
  - "20 entradas · 4 grupos" virou "21 entradas · 4 grupos", e "12 entradas" virou
    "13 entradas" em 27 frames;
  - no `Cabeçalho de grupo` mudaram o texto e a propriedade `Contagem`;
  - a Filtrada mostra "2 de 21 entradas · filtros ativos" e "2 de 13 entradas";
  - procurei no JSON do arquivo e não sobrou "20 entradas" nem "12 entradas".
- **PNG** de 30 telas e 2 componentes em `docs/mission/evidence/fase-37a/`: as telas
  novas e todas as que mudaram.

## Segurança
- Invariantes tocadas: nenhuma de código. O D35 aparece no design com o conjunto exato
  do núcleo (conferido contra `crates/ward-core/src/gerador.rs:35`), sem espaço, aspas,
  crase e `\`. Os textos com símbolos ficaram sem ligaduras, para o usuário ler o mesmo
  caractere que copia.
- Credenciais falsas da fixture (`gama-bloqueada`, `bloqueada.invalid`, `x`); nenhum
  segredo em texto. A senha só aparece como máscara.
- Testes de ataque: não se aplicam (fase só de design).
- Dependências novas: nenhuma.

## Ficou de fora
- **Cartões de nível dos formulários (18 frames e `Gerador de senha / No formulário`)**:
  - continuam frames soltos, porque têm textos próprios por tipo ("Toda entrada nova
    começa assim…", "O túnel segue também o nível do servidor.");
  - trocar por instância com override seria uma revisão dos formulários, fora do pedido.
- **Botão "Limpar" no estado vazio do detalhe em Sem resultado**: o app não tem, e o
  "Limpar" da linha de filtros já cobre. Não inventei elemento novo.
- **Rodapé "Criada em … · Alterada em …"**: mantido. D65 (d) diz que só sai se o
  usuário dispensar as datas.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cp -p Ward.figcraft <scratchpad>/Ward.backup-fase37a.figcraft` + `sha256sum` | 0 |
| MCP `figcraft` headless: `open_file`, `batch`, `run_command`, `save_file` (original) e reabertura | ok |
| `FigCraft.AppImage cli export … --node <id>` (32 PNG, `timeout 120` cada) | 0 |
| `make check` (só docs mudaram; a árvore principal tem mudanças de outras fases, não commitadas) | 0 |

## Problemas descobertos
- **Ícone dos cartões de nível dos formulários não bate com o nível.** Em
  `Gerador de senha / No formulário` (e nos formulários "Com aprovação" em geral), o
  cartão mostra "Com aprovação" com `Ícone/check`, o ícone de "Livre". Vem do M0 e não
  foi corrigido aqui (fora do pedido). Ao trocar esses cartões por instâncias do
  `Cartão de nível`, o ícone se corrige.
- **`instance.swap` do FigCraft guarda override de posição** quando o ícone de origem é
  redimensionado (24 px). Trocar `Ícone/chave` por `Ícone/busca` deixou o vetor
  deslocado. A solução foi apagar e criar a instância de novo. Vale para quem editar
  ícones de avatar.
- **Contagens mudam de novo na Fase 22a** (22 entradas, D65). Esta fase usou as 21 da
  fixture atual.
- O app (Fase 32) mostra o conjunto de símbolos colado e mais "Desligue se o painel não
  aceitar." na mesma descrição. O design agora põe o conjunto numa linha e a frase na
  linha de baixo. Uma fase de UI pode alinhar a quebra; o conteúdo já é o mesmo.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório) |
| `docs/mission/design-nodes.md` | alterado (seção Fase 37a; linhas de Cheia e Filtrada) |
| `docs/mission/evidence/fase-37a/*.png` (32) | criado |
| `docs/mission/handoffs/phase-37a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito e arquivo reaberto)
- [x] Ficou dentro do escopo da fase
