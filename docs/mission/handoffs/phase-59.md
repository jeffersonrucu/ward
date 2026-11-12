# Fase 59 — Documentação com os nomes novos (Missão 2, Milestone 5)

## Implementado
- Base do worktree conferida: estava em `b4d5e1b`; `git reset --hard 4ece237` (master atual,
  árvore limpa).
- **`docs/instalacao.md`** (achados das Fases 49, 54b, 55 e 57; m2:DT7, m2:V5.11): seção nova
  "Atualizar da 0.1.x para a 0.2.0", única parte do roteiro com os nomes da 0.1.x:
  - o que muda: subcomandos (`list`/`unlock`/`lock`/`help`, os antigos dão "comando
    desconhecido"), variáveis entregues (`WARD_ENTRY`… `WARD_FILE_KEY`), `ward list --json` em
    inglês, migração do cofre na primeira abertura (a 0.1.x não abre mais), caminhos instalados
    (`extension/`, `created-groups`) e a extensão 0.1.x sem dados até o Chrome atualizar;
  - **nomes reservados** (achado da 54b): extra `ENTRY`/`GROUP`/`ENVIRONMENT`/`FILE`/`FILE_*`,
    `PROTOCOL` em `ftp-sftp` e anexo `key` em `servidor`/`ftp-sftp`: o cofre abre e migra, o
    `ward run` dessa entrada dá erro genérico, o motivo está no journal do `wardd`, o app não
    salva até renomear; procurar antes e renomear depois;
  - **passo 1: copiar o cofre antes de atualizar** (achado da 49): para o `wardd@<uid>`
    (socket e serviço) e `cp -a` da pasta inteira (cofre, `estado`, `backups/`) para
    `~/ward-copia-0.1.x` (0700), com o motivo (a migração não guarda v4 no backup e a rotação
    tira as v4 depois de 30 gravações);
  - instalar por cima pelo mesmo meio, atualizar skill e deny, destravar por
    `/usr/local/bin/ward unlock`, conferir;
  - **voltar para a 0.1.x** (DT7): desinstalar, instalar a 0.1.x, parar o `wardd`, guardar o
    cofre migrado à parte e repor a cópia; sem a cópia, a versão v4 do backup esbarra no
    `estado` 2 (registrado como caminho não conferido).
  - resto do roteiro com os nomes novos (`/usr/local/bin/ward unlock`, `ward list`,
    `/usr/local/share/ward/extension/`, `created-groups` com a marca da 0.1.x no uninstall).
- **`docs/migracao.md`**: `/usr/local/bin/ward unlock` e `ward list` (3×).
- **`dev/tests/migration-guide.py`**: RA8 passa a exigir `/usr/local/bin/ward unlock` e recusar
  `ward unlock` fora do caminho completo; nova regra 8 (`OLD_SUBCOMMANDS`): nenhum subcomando da
  0.1.x no guia, nem pelo caminho completo; ataques 54a reescritos para `unlock` e 4 ataques
  novos (59: `destravar` pelo caminho completo, `lista`, `travar`, `ajuda`). 92 ataques
  recusados.
- **`docs/escopo.md`** (achados das 53 e 55): `ward list`, `ward lock`, `WARD_FILE_<NAME>`
  (linha 79), `detail` e `fill` (nomes do protocolo v2), RA8 com `ward unlock` (a frase da
  auditoria final diz que na 0.1.x o subcomando era outro: histórico), deny nega `ward unlock` e
  a forma da 0.1.x; na seção "Missão 2", a exceção do design (textos com nome externo
  acompanham o app).
- **`README.md`**: `ward list`, `.deb` 0.2.0 e o aviso de atualização com link ao roteiro.
- **`CLAUDE.md`**: `open_app` (2×) e `unauthorized` no lugar dos nomes do protocolo v1.
- **`.claude/agents/planner.md`** (`ward list`, `ward lock`) e **`user-testing-validator.md`**
  (`ward list`).
- **`docs/glossario.md`** §8 (achado da 53): nomes reservados novos, os antigos livres, onde a
  recusa acontece, o temporário `-chave` e o que acompanha na 59.
- **`docs/mission/design-nodes.md`** (achados das 55 e 56): 116 linhas com caminhos de arquivo
  novos (`app/src/screens/`, `lib/components/`, `lib/style/`, `app/design/icons/`, componentes e
  telas pelo glossário §4, `dev/fixtures/entries.json`, evidências da missão 1 em
  `docs/mission/arquivo/m1/evidence/`), `--fake-daemon <modo>` com os modos novos,
  `--attachments`, `&screen=`, texto "some do ward list" (linha 854) e seção nova no fim com os
  nodes trocados no FigCraft. Nomes de node, componente, variable e estilo não mudaram; os
  caminhos de `docs/design/` (em pt-BR, ficam) também não.
- **`Ward.figcraft`** (FigCraft headless; cópia de antes em
  `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/6c255186-b7ea-4545-82a5-080e9b1cf5c7/scratchpad/Ward-antes-fase59.figcraft`):
  sha256 `e52f15bf…` → `026cd7f7…`.

  | O quê | Ids |
  | --- | --- |
  | "some do ward lista" → "some do ward list" | `19017` (`Cartão de nível` › `Nível=bloqueado` `19011`), `919`, `930` (`Opção de nível`), `998`, `5965`, `19581` (`Credenciais / Bloqueada` `19359`) |
  | `WARD_ARQUIVO_*` → `WARD_FILE_*` | texto `6860` e componente `Anexo` `6849` (descrição e padrão de `Detalhe`); 27 instâncias (`Descrição`/`Detalhe`), de `7292` a `22690` |
  | `WARD_PROTOCOLO`/`WARD_ARQUIVO_CHAVE` → `WARD_PROTOCOL`/`WARD_FILE_KEY` | `11108`/`11111`, `15027`/`15030` |

  Salvo e reaberto; o `document.json` do arquivo salvo não tem `ward lista`, `WARD_ARQUIVO` nem
  `WARD_PROTOCOLO`. PNG (export da CLI) em `docs/mission/evidence/fase-59/`:
  `fase59-19359.png` (Credenciais / Bloqueada), `fase59-19011.png` (Cartão de nível bloqueado),
  `fase59-920.png` (Opção de nível bloqueado), `fase59-13459.png` (Formulário / genérico /
  Edição, com `WARD_FILE_<NOME>` e `WARD_FILE_APP_ENV`).

## Segurança
- Invariantes tocadas: S3 e RA8 (guias destravam só por `/usr/local/bin/ward unlock`; o checador
  do guia recusa outro caminho e qualquer subcomando da 0.1.x), S2 (a cópia do cofre antes de
  atualizar é o arquivo cifrado, numa pasta 0700 do home, nunca `/tmp` nem nuvem), S15 (nenhum
  segredo nos docs: as linhas novas do diff não casam com `.dev/variations.txt`; os dois
  arquivos que casam, `CLAUDE.md` e `migration-guide.py`, já casavam antes, com valores de D15).
- Testes de ataque: `dev/tests/migration-guide.py` › ataques "54a: …" (agora com `unlock`) e
  "59: destravar da 0.1.x pelo caminho completo", "59: ward lista", "59: ward travar",
  "59: ward ajuda".
- Dependências novas: nenhuma.

## Ficou de fora
- `skill/` não foi tocada (a Fase 58 já a fez); por isso o `make test-skill` não rodou.
- Nenhuma suíte que fala com daemon (regra 6).
- `docs/mission/arquivo/` (não se edita), handoffs, plano, PRD e contrato.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-variations` | 0 |
| `make test-migration-guide` (92 ataques recusados, guia ok) | 0 |
| `make english-check PATHS="dev/tests/migration-guide.py"` | 0 |
| `make english-check` | 0 |
| `make compile` (workspace, Vite, `app/src-tauri`, sintaxe de 52 shell, 36 Python, 7 JS) | 0 |
| `grep -F -f .dev/variations.txt` nas linhas novas do diff | 0 achados |

Logs no scratchpad da sessão: `fase59-dev-variations.log`, `fase59-test-migration-guide.log`,
`fase59-english-check.log`, `fase59-english-check-paths.log`, `fase59-make-compile.log`.

## Problemas descobertos
- **V5.11 × textos que citam a forma antiga de propósito.** Fora de `docs/mission/arquivo/`, da
  nota de `docs/instalacao.md` e do histórico do escopo, os nomes antigos ainda aparecem, de
  propósito: `skill/README.md` (o deny nega `ward destravar` e `WARD_ARQUIVO_*` para a 0.1.x,
  DT10, Fase 58); `CLAUDE.md` (o `test-no-test-mode` e o `install.sh` procuram as marcas da
  0.1.x; "sem ela, a da 0.1.x"; a imagem Docker `ward-teste/*:1` e a base `app_teste`, que ficam
  pelo glossário §9); `docs/mission/design-nodes.md` (a tabela "antes → depois" da seção nova)
  e os handoffs/plano/PRD de `docs/mission/`. O validador do V5.11 precisa aceitar esses casos
  (ou o contrato listá-los).
- **PRD regra 2 × Fase 59.** A regra diz que o `Ward.figcraft` fica como está; o pedido desta
  fase (e a nota da 55) mandou acompanhar o texto do app. Além do "ward list", troquei também os
  `WARD_ARQUIVO_*`/`WARD_PROTOCOLO` dos formulários, que o app já mostra como `WARD_FILE_*`/
  `WARD_PROTOCOL` desde a Fase 53. O texto `11111` do design diz "WARD_PASS (ou WARD_FILE_KEY,
  com chave)", e o app (`form.js`) diz "WARD_FILE_KEY (+ WARD_PASSPHRASE) ou WARD_PASS": a
  diferença já existia e não é de nome (não mexi).
- **O `edit.findReplace` do FigCraft só procura na página atual.** Na primeira passada (página
  Layout) ele não pegou os nodes da Design System; repeti com `page.select`. As propriedades de
  componente das instâncias (`Descrição`, `Detalhe`) não mudam com o texto: troquei por
  `instance.setProperties`, e o padrão do `Anexo` por `component.editProperty`.
- **Volta para a 0.1.x sem a cópia.** Repor à mão uma versão v4 de `backups/` esbarra no
  `estado` gravado pela 0.2.0 (`ward-state 2`, geração maior), que a 0.1.4 recusa. O roteiro diz
  que esse caminho não está conferido; a Fase 64 (V4.7, DT7) pode conferir e, se der, o roteiro
  ganha os passos (talvez apagar o `estado`, se a 0.1.4 aceitar começar sem ele).
- `docs/instalacao.md` ainda diz "60 caminhos" no `dpkg-deb -c` e o README cita
  `ward_0.2.0_amd64.deb` antes da Fase 60 mudar a versão (DT19): conferir na Fase 60/66.
- O deny instalado no `~/.claude` desta máquina recusa comandos Bash que citem o nome antigo do
  destrave (e até `sed`/`grep` com ele): usei o Edit.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/instalacao.md` | alterado |
| `docs/migracao.md` | alterado |
| `dev/tests/migration-guide.py` | alterado |
| `docs/escopo.md` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `README.md` | alterado |
| `CLAUDE.md` | alterado |
| `.claude/agents/planner.md` | alterado |
| `.claude/agents/user-testing-validator.md` | alterado |
| `docs/mission/evidence/fase-59/fase59-{19011,19359,920,13459}.png` | criado |
| `docs/mission/handoffs/phase-59.md` | criado |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` (fora do repositório) | alterado |

## Procedimentos
- [x] `make compile` e `make english-check` rodados (regra 6: sem `make check` no M5)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (DESIGN) Conferido no FigCraft, `save_file` feito e arquivo reaberto
- [x] Ficou dentro do escopo da fase
