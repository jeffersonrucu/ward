# Fase 39f — Área de transferência sem disco e baixos da auditoria do M5

## Implementado
- **Copiar sem `wl-copy` (alto S2):** `app/src-tauri/src/area_gtk.rs` (novo), `AreaGtk`,
  implementa `AreaTransferencia` com o `gtk::Clipboard` (`set_with_data`). O valor fica
  num `Zeroizing` na memória do app, e o GTK o escreve direto no cano do compositor a
  cada colagem. O app não roda nenhum processo e não cria nenhum arquivo.
  - Tipos oferecidos: `text/plain;charset=utf-8`, `UTF8_STRING`, `text/plain` e
    `x-kde-passwordManagerHint` = `secret`.
  - O "só apaga o que o Ward copiou" passou a se basear em quem é o dono da seleção. O
    conteúdo entregue ao GTK carrega uma `Guarda`, e o GTK a solta (`clear_func`) quando
    outro programa copia algo, quando o app limpa ou quando o app copia de novo. `ler`
    devolve o valor só enquanto a cópia é a atual. `limpar` confere o id da cópia já
    dentro da thread principal. Com isso some a corrida entre ler e limpar que o par
    `wl-paste` + `wl-copy --clear` tinha (Problemas da Fase 29).
  - O GTK só roda na thread principal. `escrever` fora dela é recusado. O comando do
    Tauri é síncrono e por isso roda na principal, como o diálogo de arquivo da Fase 34.
    `limpar` chamado de outra thread (prazo de 30 s, bandeja, vigia) segue pelo
    `glib::MainContext::default().invoke`, sem esperar.
  - O `Copiador` do núcleo continua igual: limpeza condicional de 30 s, comparação em
    tempo constante e geração da cópia.
- `app/nucleo/src/area_transferencia.rs`: saiu o `Wayland` (`wl-copy`/`wl-paste`).
  Entraram `MAX_VALOR` e o re-export de `Zeroizing`, que o app usa sem depender do
  `zeroize`. A ponte de teste ficou com a sua cópia do `wl-copy`, em
  `crates/ward-ponte-teste/src/area_wl.rs` (`WlCopy`). Ela só existe com `modo-teste` e
  só vê valores falsos.
- **Trava vinda de fora (baixo):** `Servico::na_passagem_da_bandeja` chama `ao_sair()`.
  Isso limpa a área de transferência na hora, se o valor ainda for do Ward, e descarta o
  formulário.
- **`anexos.rs` (baixo):** `ler_limitado` abre com `O_NOFOLLOW|O_NONBLOCK|O_NOCTTY`, como
  o `chave.rs`. Link e FIFO dão o aviso fixo na hora.
- **D51, U+202E no nome do processo (baixo):** `ward-plataforma::processo` não troca
  mais controle e bidi por `?` (a função virou `nome_do_status`). Ela passa o nome como
  o kernel dá, tirando só o `\t` separador e cortando em `MAX_NOME_CADEIA`. O escape
  visível continua a cargo do `wardd` (`exibicao::exibir_texto`), o único consumidor.
- **D51, letras RTL (pendência da 36):** `app/src/lib/bidi.js` (`partesBidi`) e
  `app/src/lib/componentes/TextoDoAgente.svelte` isolam cada sequência de letras RTL num
  `<bdi dir="rtl">`, dentro da caixa LTR. Faixas usadas: U+0590–08FF, U+FB1D–FDFF,
  U+FE70–FEFC, U+10800–10FFF e U+1E800–1EFFF. Assim `echo א 1 ב` não aparece como
  `echo ב 1 א`. Juntos, os trechos dão o texto original. Usado na janela de aprovação
  (nome da entrada, comando, diretório, cada processo da cadeia) e no pedido do agente
  da tela Destravar (entrada e comando). O `.nome` da aprovação ganhou
  `direction: ltr; unicode-bidi: isolate`. O visual do texto LTR não muda.
- **Teste na sessão real:** `make teste-area-transferencia` (alvo novo) chama
  `dev/testes/area-transferencia.sh` e `dev/testes/area_transferencia.py`.
- `docs/mission/roteiro-usuario.md`: seção 4 com os passos 6–8 (sem
  `/tmp/wl-copy-buffer-*` nem `wl-copy`, `ward travar` por fora limpa, "Sair" limpa).

## Segurança
- **S2:** nenhum arquivo recebe o valor copiado. O `make teste-area-transferencia`
  conferiu, com o GTK real na sessão COSMIC:
  - autoteste: o vigia (inotify em `/tmp`, `/var/tmp` e `/run/user/1000`, com
    subpastas) acha o valor que o `wl-copy` grava em `/tmp/wl-copy-buffer-*/`. É o
    ataque da auditoria, e ele é achado;
  - `strace -f` do processo do teste: nenhum `execve` (nada de `wl-copy`) e nenhum
    `open` para escrita nessas pastas, nos backends X11 e Wayland;
  - inotify durante a rodada e varredura no fim: nenhum arquivo com o valor.
- **S12 / V5.4 / V5.5:**
  - `wl-paste` devolve o valor, e os tipos oferecidos incluem
    `x-kde-passwordManagerHint` (`secret`, lido pelo X11);
  - o valor some no prazo, e o texto do usuário copiado depois fica;
  - limpar fora da thread principal (o caminho da bandeja) tira o valor na hora;
  - "copiar" fora da thread principal é recusado;
  - ao sair do app o valor some.
  - No fim a área de transferência ficou vazia (`wl-paste`: "Nothing is copied").
- **S11:** a resposta do "copiar" continua `()`, e a interface não recebe o valor.
- **V5.12:** link e FIFO como anexo são recusados sem prender o app.
- **V5.29 / D51:** U+202E no nome do processo chega como escape visível, e as letras RTL
  ficam isoladas.
- Testes de ataque:
  - `app/src-tauri/src/area_gtk.rs` ›
    - `ataque_texto_copiado_por_outro_programa_deixa_de_ser_do_ward`
    - `ataque_guarda_antiga_nao_solta_a_copia_nova`
    - `ataque_copiar_fora_da_thread_principal_e_recusado`
    - `limpar_fora_da_thread_principal_nao_toca_no_gtk`
    - `sessao_real` (ignorado; roda pelo `make teste-area-transferencia`)
  - `app/nucleo/src/servico.rs` › `ataque_trava_de_fora_limpa_a_area_na_hora`
  - `app/nucleo/src/anexos.rs` › `ataque_link_e_fifo_sao_recusados_sem_prender`
  - `crates/ward-plataforma/src/processo.rs` ›
    `ataque_processo_com_u202e_no_nome_chega_inteiro_na_cadeia` (processo de verdade
    com U+202E no `comm`) e `nome_da_cadeia_sem_troca_por_interrogacao`
  - `crates/ward-ponte-teste/src/area_wl.rs` › `ataque_binario_fora_do_padrao_e_recusado`
    (veio do núcleo)
  - `dev/testes/bidi.mjs` (no `make app-front`): hebraico em volta de número, árabe,
    formas de apresentação, plano suplementar, sem perda e sem reordenar
  - `dev/testes/area_transferencia.py`: autoteste mais as conferências acima. Também
    conferi o parser do strace com um log plantado (`execve` do `wl-copy` e `openat`
    `O_WRONLY|O_CREAT` em `/tmp`), e os dois foram apontados.
- **Dependências novas: nenhuma.** O `gtk` 0.18.2 já era dependência direta do app
  (Fase 34). `gdk` e `glib` vêm pelo re-export do `gtk`, e o `zeroize` pelo re-export do
  núcleo. Os `Cargo.lock` não mudaram. Não usei o `wl-clipboard-rs`: ele traria a árvore
  `wayland-client`/`wayland-protocols`/`tempfile` ao app, e o `copy` dele também passa
  por arquivo temporário, o mesmo problema do `wl-copy`.

## Ficou de fora
- **Linha do `CLAUDE.md`** para `make teste-area-transferencia`: não editei, pela mesma
  regra das Fases 39, 39c e 39e (mudar o `CLAUDE.md` precisa da autorização do usuário).
  Texto proposto:
  `| Fase 39f (S2, S12; V5.4, V5.5): a área de transferência do GTK do app na sessão gráfica sob strace -f, com inotify em /tmp, /var/tmp e $XDG_RUNTIME_DIR (autoteste: acha o que o wl-copy grava); nenhum programa executado, nenhum arquivo com o valor, wl-paste devolve o valor com x-kde-passwordManagerHint, some no prazo, texto do usuário fica, trava de fora limpa na hora; só valores falsos, limpa a área no fim. Exige strace, wl-clipboard, python3-gi, DISPLAY e WAYLAND_DISPLAY; fora do make check | make teste-area-transferencia | pronto |`
- **Copiar no app real com o backend Wayland e clique:** fica com o roteiro do usuário
  (seção 4, passos 3–8). Um teste sem clique não ganha foco de teclado no COSMIC (ver
  Problemas).
- Fase 51c (core dump do WebKit): é outra fase.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test --locked -p ward-app-nucleo` | 0 |
| `cargo test --locked -p ward-plataforma --lib processo` | 0 |
| `cargo test --locked -p wardd --test aprovacao d51` | 0 |
| `cargo test --locked -p ward-ponte-teste --features modo-teste` | 0 |
| `node dev/testes/bidi.mjs` | 0 |
| `make teste-area-transferencia` | 0 |
| `make app-check` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **COSMIC só aceita a seleção Wayland de um cliente com foco de teclado.** Uma janela
  aberta por teste não ganha foco: o COSMIC impede o roubo de foco, e o teste não tem
  token de ativação. Por isso a rodada completa do teste usa o backend X11 do GTK no
  Xwayland da sessão. O COSMIC repassa a seleção para o Wayland, e o `wl-paste` lê por
  lá. A rodada Wayland confere só o strace e o vigia, e o compositor recusa a seleção.
  O app real copia logo depois de um clique na própria janela (com foco e serial), que é
  o caso normal do GTK, mas isso só se confirma pelo roteiro do usuário.
- **Limpeza de seleção que outro programa assumiu:** se um gerenciador de área de
  transferência pegar o valor e o servir no lugar do Ward, o Ward não é mais o dono e
  não o apaga. O `x-kde-passwordManagerHint=secret` pede para ele não guardar. Antes, o
  `wl-copy --clear` apagava qualquer conteúdo igual ao valor.
- **Cópias do valor dentro do GTK:** a cada colagem, o GTK/GDK copia o valor nos
  próprios buffers (`GtkSelectionData`, saída para o cano), sem zerar. É só memória do
  processo, que já é não-dumpable (D47, com o resto da 51c para o apport). Antes o valor
  passava também pela memória do `wl-copy`.
- **`limpar` de outra thread quando o laço do GLib não está de pé** (antes do
  `tauri::Builder` ou depois do fim do laço): o pedido roda na própria thread e não
  toca no GTK, então não limpa. Nesses momentos o processo está saindo ou
  reexecutando, e a seleção some quando a conexão com o compositor fecha. O teste
  confirmou que ao sair o valor some.
- **Comandos síncronos do Tauri rodam na thread principal:** o "copiar" depende disso,
  como o diálogo de arquivo da Fase 34. Se uma versão nova do Tauri mudar isso, o
  copiar falha com "não foi possível usar a área de transferência", sem vazar nada.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/src-tauri/src/area_gtk.rs` | criado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/nucleo/src/area_transferencia.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/anexos.rs` | alterado |
| `crates/ward-plataforma/src/processo.rs` | alterado |
| `crates/ward-ponte-teste/src/area_wl.rs` | criado |
| `crates/ward-ponte-teste/src/lib.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `app/src/lib/bidi.js` | criado |
| `app/src/lib/componentes/TextoDoAgente.svelte` | criado |
| `app/src/telas/JanelaAprovacao.svelte` | alterado |
| `app/src/telas/Destravar.svelte` | alterado |
| `dev/testes/bidi.mjs` | criado |
| `dev/testes/area-transferencia.sh` | criado |
| `dev/testes/area_transferencia.py` | criado |
| `Makefile` | alterado (`teste-area-transferencia`; `bidi.mjs` no `app-front`) |
| `docs/mission/roteiro-usuario.md` | alterado (seção 4, passos 6–8) |
| `docs/mission/handoffs/phase-39f.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (valores `falso-39f-*` gerados na hora; área limpa no fim)
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Não conferi contra o FigCraft nem por screenshot: a fase não é de UI. A única mudança na tela é o `<bdi>` nas letras RTL (texto LTR igual).
- [x] Ficou dentro do escopo da fase
