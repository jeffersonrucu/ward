# Fase 49 — Kit de emergência para imprimir (UI)

## Implementado
- **"Imprimir kit" ligado** em `Configurações › Backup › Kit de emergência`
  (`app/src/telas/Configuracoes.svelte`): o botão abre o kit (`dialogo = 'kit'`). A
  mudança na tela ficou pequena: um import, o botão e o `<KitEmergencia>`, ao lado dos
  diálogos de troca, para o merge com a Fase 48 ser simples.
- **`app/src/telas/KitEmergencia.svelte`** (novo), a partir do frame `16773`
  (`Kit de emergência / Impressão`, lido de uma cópia do `Ward.figcraft`; ids em
  `docs/mission/design-nodes.md` › "Fase 49"):
  - É uma janela `<dialog>` modal que ocupa o app: Esc fecha e o foco fica preso nela.
    No alto, uma barra com "Kit de emergência", uma explicação curta e os botões
    "Fechar" e "Imprimir". Abaixo vem a folha A4 (794×1123 por `pagina/a4-*`, papel e
    tinta `impressao/*`).
  - A folha tem os mesmos textos do frame:
    - logo com o nome em `impressao/texto` e "Impresso em DD/MM/AAAA", na data local;
    - "Onde está o cofre": `~/.local/share/ward/`;
    - "Onde estão os backups": cópias locais em `~/.local/share/ward/backups/` e o pendrive;
    - "Senha mestra", com duas linhas **em branco** e "Escreva à mão. O Ward nunca preenche
      este espaço.";
    - "Como restaurar", em 4 passos, e o rodapé sobre o PIN.
  - O item do pendrive sai de `configuracoes` (o `backup_externo` booleano que já
    existia):
    - desligado: "Pendrive · backup externo desligado / Nenhuma cópia em pendrive. Ligue
      em Configurações › Backup externo em pendrive.";
    - ligado: "Pendrive · copiado sempre que é conectado" e
      `[rótulo · ][UUID … · ]pasta ward-backup/`. O rótulo e o UUID só aparecem quando o
      app os tiver (ver "Ficou de fora").
  - "Imprimir" chama `window.print()` e fica desabilitado até a leitura terminar. O
    sistema abre a janela de impressão, e PDF só existe se o usuário escolher imprimir
    em arquivo e o lugar. O app não grava nada.
  - Na impressão (`@media print`), `@page { size: A4; margin: 0 }`. Enquanto o kit está
    aberto, tudo que é filho do `<body>` some, menos o kit (`body:has(> dialog.kit[open])`).
    A barra também some e sai só a folha. Para isso o `<dialog>` vai para o fim do
    `<body>` (action `noBody`, que o remove ao desmontar). As linhas em branco são
    `border-top`, para sair mesmo sem "imprimir fundos".
  - Com o cofre travado, o kit fecha e vai para o destravar. Se a leitura der outro erro,
    aparece um toast com "Tentar de novo".
- **`app/src/telas/kit-emergencia.js`** (novo): pastas fixas (D17/D18), passos,
  `dataImpressa` e `linhaDoPendrive`. Esta última aceita só UUID no formato de D45 (outra
  coisa não é impressa como UUID) e troca qualquer caractere de controle do rótulo por
  espaço, além de cortá-lo em 64.
- **`Logo.svelte`:** ganhou a prop `impressao`, com o nome em `--cor-impressao-texto`
  (override da instância `16781`).

## Segurança
- **Invariantes tocadas:**
  - **S2/S11 (V7.10):** a folha não tem segredo. As pastas são texto fixo. Do daemon vem
    só um booleano, pela operação `configuracoes`, que já existia e não traz hash, PIN nem
    UUID. A senha mestra não tem campo, valor nem estado: são duas `div` vazias. Nada vai
    para `localStorage`, `sessionStorage`, URL, log ou arquivo, e o PDF só existe se o
    usuário escolher na janela de impressão.
  - **Entrada não confiável:** o rótulo e o UUID do pendrive (quando chegarem) passam por
    `linhaDoPendrive`: UUID só no formato de D45, rótulo sem controle e cortado, mostrado
    por interpolação dentro de `<bdi>` (nunca como HTML).
  - **Nenhum comando ou capability nova no Tauri.** O CSP não mudou (`style-src 'self'`,
    sem atributo `style`).
- **Testes de ataque** (QA no Chrome headless pela ponte `--daemon-falso cheio`, que usa
  as entradas de `dev/fixtures/entradas.json`; roteiro em
  `docs/mission/evidence/fase-49/qa49.py`; resultado em `qa-falso.json`, `"ok": true`):
  - **V7.10, DOM e PDF:** a busca usa os 279 padrões de `.dev/variacoes.txt` mais a senha
    mestra e o PIN de teste (281 ao todo).
    - `dom_achados` 0 no HTML e no `innerText`, com o kit aberto.
    - `pdf_achados` 0 no texto do PDF (`page.pdf` em memória, depois `pdftotext`).
  - **Isca fora da folha:** com o kit aberto, um elemento com a senha mestra de teste é
    posto no `#app`.
    - A busca acha a isca no DOM (`controle_busca_acha_isca_no_dom`), então a busca
      funciona.
    - O PDF não leva a isca (`isca_fora_do_pdf`): a impressão só leva a folha.
  - **Linhas em branco:** `senha_linhas_em_branco` (texto vazio) e `sem_campo_na_folha`
    (nenhum `input`, `textarea` ou `contenteditable`).
  - **Rastro:** `sem_storage` e `url_limpa` depois de abrir e fechar,
    `so_pedidos_locais`, `sem_http_erro` e `console_sem_erro` (fora o 404 do
    `/favicon.ico`, que a ponte não serve).
  - **Impressão:** `window_print_chamado` (1 chamada) e `pdf_uma_pagina` (1 página A4).
    O texto da barra e o resto da tela não estão no PDF (`pdf_so_folha`).
- **Dependências novas:** nenhuma (Rust e npm). O Playwright do QA é o dos wheels já
  existentes no scratchpad, desempacotados fora do repositório.

## Ficou de fora
- **Rótulo e UUID do pendrive na folha:** o comando `backup_externo` do app é da Fase
  48, que ainda não está no master. Hoje o kit lê só o booleano (`configuracoes`) e,
  ligado, imprime "pasta ward-backup/". `linhaDoPendrive` já aceita
  `{ ligado, uuid, pendrive: { rotulo } }`, o formato do `BackupExternoVisto` da 48.
  Depois do `git merge master`, basta trocar em `KitEmergencia.svelte › carregar` a
  chamada `configuracoes` por `backup_externo`. Faço isso quando o merge for pedido.
- **Pré-visualização sem frame:** a barra "Fechar/Imprimir" por cima da folha não tem
  frame no FigCraft. Foi montada com `Botao` e tokens.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make app-front` | 0 |
| `cargo build -p ward-ponte-teste --features modo-teste` (1ª vez: disco cheio, ENOSPC; o coordenador liberou espaço) | 101, depois 0 |
| `make dev-cofre` e `make dev-variacoes` (no `.dev/` do worktree) | 0 |
| ponte `--daemon-falso cheio` + `qa49.py`. As 3 primeiras rodadas falharam só em `console_sem_erro`, pelo 404 do `/favicon.ico`; o filtro passou a descontar só a URL do favicon. Depois vieram a isca e o controle | 1, depois 0 |
| `make check` | 0 |
| `make app-check` (R32, `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do scratchpad) | 0 |

## Problemas descobertos
- **`window.print()` no WebKitGTK não conferido no app real.** No Chrome, o QA conferiu
  a chamada e o PDF de `@media print`. No Tauri/WebKitGTK, `window.print()` deveria
  abrir a janela de impressão do GTK, que tem "Imprimir em arquivo". Não abri o app de
  verdade (janela na sessão do usuário). O validador do M7 deve abrir o app, clicar
  "Imprimir" e conferir que a janela do GTK aparece e que o PDF sai com uma página. Se o
  wry não atender `print`, vai precisar de um comando nativo
  (`WebView::print`/`webkit_print_operation`).
- **Pastas fixas, não as do daemon:** a folha mostra `~/.local/share/ward/` e
  `…/backups/` (D17/D18), como a tela de Configurações já mostrava. O daemon de teste
  usa `.dev/`, e isso não aparece na folha. Mostrar o caminho real exigiria uma
  operação nova. Não criei, porque o escopo fixa as pastas.
- **O passo 2 do "Como restaurar"** (copiar um `ward-*.ward` do pendrive para
  `~/.local/share/ward/backups/` e restaurar pelo app) depende de haver um cofre
  destravado. A Fase 45 registrou que restaurar exige o cofre destravado e o PIN atual.
  Numa máquina nova, isso quer dizer criar um cofre novo antes, o que o passo 1 já diz.
  Vale o planner confirmar o texto quando a Fase 47 decidir o restaurar.
- **Disco cheio durante a fase:** a partição `/` chegou a 100% (vários `target/` de
  worktrees). O coordenador liberou espaço. Vale um `cargo clean` dos worktrees ao fim
  de cada fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/src/telas/KitEmergencia.svelte` | criado |
| `app/src/telas/kit-emergencia.js` | criado |
| `app/src/telas/Configuracoes.svelte` | alterado |
| `app/src/lib/componentes/Logo.svelte` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-49/qa49.py` | criado |
| `docs/mission/evidence/fase-49/qa-falso.json` | criado |
| `docs/mission/evidence/fase-49/configuracoes-kit-habilitado-falso.png` | criado |
| `docs/mission/evidence/fase-49/kit-de-emergencia-1280x800-falso.png` | criado |
| `docs/mission/evidence/fase-49/kit-de-emergencia-folha-falso.png` | criado |
| `docs/mission/evidence/fase-49/kit-impresso-pdf-falso.png` | criado |
| `docs/mission/handoffs/phase-49.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V7.10: DOM e PDF sem segredo, isca fora da folha não imprime)
- [x] (UI/DESIGN) Conferido contra o FigCraft (frame `16773`, cópia lida; a folha renderizada bate com o screenshot do node)
- [x] Ficou dentro do escopo da fase
