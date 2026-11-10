# Fase 39g — Restos da 2ª auditoria do M5

## Implementado
- **Médio, valor revelado na PRIMARY/CLIPBOARD (S2, S12):** `CampoSecreto.svelte`.
  Primeiro conferi no WebKitGTK real a sugestão do plano: `user-select: none` mais
  cancelar `copy`/`cut`/`selectstart`. Ela **não basta**: Ctrl+A, o arrasto que começa
  fora do campo e a seleção que atravessa o campo ainda levam o valor à PRIMARY, e
  Ctrl+C o leva à CLIPBOARD. O teste novo roda contra essa variante e dá 6 FALHAs.
  - Por isso o valor revelado deixou de ser nó de texto. Ele vai no atributo
    `data-valor` e é desenhado por `.revelado::before { content: attr(data-valor) }`.
    Conteúdo gerado não entra em seleção, cópia, arrasto nem `innerText`.
  - Ficaram também `user-select: none` e o cancelamento de `copy`, `cut` e
    `selectstart` no campo.
  - No campo de digitação visível (o olho do formulário), cancelo `copy`/`cut` e
    `dragstart`.
  - O botão copiar não mudou. O CSS continua sem estilo inline (a CSP segue igual).
- **Teste novo, `make teste-selecao-revelado`** (`dev/testes/selecao-revelado.sh` +
  `dev/testes/selecao_revelado.py`). Ele veio do `scratchpad/primary.py` da auditoria.
  - Sobe um `wardd` de teste só dele com a ponte. Abre a interface de teste de verdade
    num WebKitGTK 4.1 (python3-gi) dentro de um **Xwayland privado** (rootful, cookie
    MIT, sem TCP). HOME, XDG e contexto WebKit são efêmeros.
  - A PRIMARY e a CLIPBOARD da sessão do usuário nunca são lidas nem escritas. Um
    processo X separado faz o papel do "outro programa": lê as seleções e só responde
    se cada agulha está lá.
  - Teclado e mouse entram como eventos reais do GDK.
  - Autoteste:
    - cliente sem cookie é recusado;
    - o valor como texto (o componente antigo) é achado na PRIMARY e, com Ctrl+C, na
      CLIPBOARD.
  - Ataques: seleção por script, Ctrl+A, Ctrl+A+Ctrl+C, arrasto do mouse, clique
    triplo (com e sem Ctrl+C), seleção que atravessa o campo e `execCommand('copy')`
    dela, e `innerText`. Para cada um, o teste confere que o ataque teve efeito.
  - Positivos: o campo mostra o valor (`::before`) e "Ocultar" o tira do documento.
  - No fim, varre os logs (V5.27).
- **Baixo, U+FEFD/U+FEFE:**
  - `bidi.js`: a faixa vai até U+FEFE;
  - `exibicao.rs` (`invisivel`): `0xFEFD | 0xFEFE` agora têm escape;
  - teste novo `dev/testes/bidi_icu.py` (no `make app-front`). Ele traz o
    `scratchpad/bidi.c` sem compilar nada: usa por ctypes a `libicuuc` do sistema, a
    mesma do WebKitGTK. Lê as faixas dos dois fontes e varre U+0000–U+10FFFF. Falha se
    algum código R/AL/AN/B ou controle bidi ficar sem isolamento e sem escape. O
    autoteste com as faixas antigas acha FEFD e FEFE. Também conferi contra os fontes
    do HEAD: sai 1.
- **Baixo, trava de fora sem bandeja:**
  - `Servico::ver_passagem(&Mutex<EstadoBandeja>)` (núcleo) troca o último estado visto
    e chama `na_passagem_da_bandeja`;
  - `Estado::visto` (main.rs) guarda esse estado fora da `Bandeja`, e o
    `atualizar_bandeja` o consulta antes do `try_state`;
  - a vigia agora roda também sem bandeja. Sem bandeja, a principal só é avisada
    quando o estado muda.
- **Baixo, `anexos.rs`:** `ler_ate` lê num buffer fixo de `max + 1` em `Zeroizing`,
  como o `chave.rs`. A leitura nunca realoca, e o `Segredo` toma posse do mesmo bloco.
- **Escopo:** item novo em "Fora do modelo de ameaça", "Crash do Chrome com a senha
  preenchida": apport e crashpad no perfil, o Chrome fica fora da lista do apport,
  junto do RA10. O texto da 51c remete a ele.
- Roteiro do usuário, seção 4, passo 9: selecionar, Ctrl+A e Ctrl+C no valor revelado
  do app nativo.

## Segurança
- **S2/S12:** o valor revelado só chega à área de transferência pelo botão copiar, com
  prazo e dica `secret`. Testado no WebKitGTK real. Rodadas do teste:

  | Versão do componente | FALHAs |
  | --- | --- |
  | antiga (HEAD) | 11 |
  | só `user-select: none` + eventos (a sugestão do plano) | 6 |
  | atual | 0 |

- **S12/S2 (trava de fora):** sem bandeja, a trava de fora limpa a área e o formulário
  na hora.
- **S2 (anexos):** nenhuma cópia do anexo fica num bloco antigo do heap sem zerar.
- **D51/V5.29:** U+FEFD e U+FEFE ficam isolados (JS) e com escape (Rust).
- Testes de ataque:
  - `dev/testes/selecao_revelado.py` (todos os ataques acima, com autoteste)
  - `dev/testes/bidi_icu.py` (varredura ICU com autoteste)
  - `dev/testes/bidi.mjs` › caso `FEFD/FEFE`
  - `crates/ward-core/src/exibicao.rs` › `ataque_controle_e_bidi_viram_escape_visivel`
    (caso novo)
  - `app/nucleo/src/servico.rs` › `ataque_trava_de_fora_sem_bandeja_limpa_a_area_na_hora`
  - `app/src-tauri/src/main.rs` › `sem_bandeja_a_vigia_mede_a_passagem`
  - `app/nucleo/src/anexos.rs` › `ataque_arquivo_que_cresce_nao_realoca_o_buffer`
- Dependências novas: nenhuma. O teste usa só ferramentas do sistema: Xwayland, xauth,
  mcookie, python3-gi/WebKit2 4.1 e libicuuc por ctypes.

## Ficou de fora
- **Linha do `CLAUDE.md`** para `make teste-selecao-revelado` (mudar o `CLAUDE.md`
  precisa de autorização do usuário). Texto proposto:
  `| Fase 39g (S2, S12): o valor revelado não vai à PRIMARY nem à CLIPBOARD por seleção, Ctrl+A, Ctrl+C, arrasto ou clique triplo, no WebKitGTK 4.1 real (python3-gi) pela ponte, num Xwayland privado com cookie (a área de transferência da sessão não é tocada; uma janela aparece por ~20 s); autoteste com o valor como texto. wardd de teste só dele (WARD_DEV_SOCKET_DIR, padrão /run/user/<uid>/ward-teste-selecao). Precisa de make dev-cofre, dev-variacoes, app-front, Xwayland e xauth; fora do make check | make teste-selecao-revelado | pronto |`
- O backend Wayland do GTK não é exercitado pelo teste novo, para isolar a área de
  transferência do usuário. O caminho (seleção do DOM → editor do WebKit → GtkClipboard)
  é o mesmo nos dois backends. Nos experimentos de investigação, o vazamento antigo
  apareceu igual no Wayland e no X11 da sessão. O passo 9 do roteiro cobre o app nativo
  no COSMIC.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test --locked -p ward-app-nucleo` | 0 |
| `cargo test --locked -p ward-core --lib exibicao` | 0 |
| `cargo test --all-features --locked` (app/src-tauri) | 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` (app/src-tauri) | 0 |
| `python3 -I -B dev/testes/bidi_icu.py` | 0 (com os fontes antigos: 1) |
| `make app-front` | 0 |
| `make teste-selecao-revelado WARD_DEV_SOCKET_DIR=/run/user/1000/ward-39g` | 0 (componente antigo: 1; só user-select: 1) |
| `make check` | 0 |
| `make app-check` | 0 |

## Problemas descobertos
- **A correção do plano (`user-select: none` + `copy`/`selectstart`) não fecha o
  médio.** O WebKit põe na PRIMARY o texto de nós `user-select: none` quando a seleção
  os inclui (Ctrl+A, arrasto vindo de fora, range que atravessa). Por isso a correção
  usa o `::before`.
- **QA/validadores:** o valor revelado não está mais no texto do elemento. Quem lia
  `.revelado` por `innerText`/`textContent` deve ler `data-valor` ou o `::before`
  (`getComputedStyle(el, '::before').content`). Nenhum teste do repositório lia.
- **Campo de digitação visível (formulário, olho aberto):** a seleção dentro do
  `<input type=text>` ainda vai à PRIMARY, que é o comportamento do WebKitGTK para
  campo editável. Ctrl+C/recortar/arrastar estão cancelados. O texto é o que o
  usuário digita (ou colou). Com o olho fechado (`type=password`), nada vai.
- **Escopo:** o item "Crash do Chrome" foi escrito a partir do plano (parecer do
  auditor: baixo). O escopo não registra decisão explícita do usuário com data, ao
  contrário dos outros itens da seção. Confirmar com o usuário.
- **Outras listas de invisíveis** (`crates/ward/src/texto.rs`,
  `app/nucleo/src/formulario.rs`, `app/nucleo/src/comandos.rs`) não foram tocadas (fora
  da fase). Elas servem a outros textos, não à janela de aprovação.
- **Sessão do usuário:** só os experimentos iniciais (valores `falso-39g-exp-*`)
  tocaram a PRIMARY da sessão, porque eram processos do GTK que já saíram. No fim, a
  CLIPBOARD e a PRIMARY não tinham nenhum `falso-*`, e nada foi apagado. Um eventual
  gerenciador de área de transferência do COSMIC pode ter guardado esses valores falsos
  no histórico.
- Durante a investigação, rodei `npm run build` com o componente do HEAD (stash). Depois
  o `make app-front` refez `app/dist` e `app/dist-teste` com o código final.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/src/lib/componentes/CampoSecreto.svelte` | alterado |
| `app/src/lib/bidi.js` | alterado |
| `crates/ward-core/src/exibicao.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/anexos.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `dev/testes/bidi.mjs` | alterado |
| `dev/testes/bidi_icu.py` | criado |
| `dev/testes/selecao_revelado.py` | criado |
| `dev/testes/selecao-revelado.sh` | criado |
| `Makefile` | alterado (`teste-selecao-revelado`; `bidi_icu.py` no `app-front`) |
| `docs/escopo.md` | alterado (Fora do modelo: crash do Chrome) |
| `docs/mission/roteiro-usuario.md` | alterado (seção 4, passo 9) |
| `docs/mission/handoffs/phase-39g.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) O visual do campo revelado não foi conferido contra o FigCraft por
  screenshot. Os estilos não mudaram, só o texto passou para o `::before`, e o teste
  confere que o valor aparece.
- [x] Ficou dentro do escopo da fase
