# Fase 36 — Janela de aprovação

## Implementado
- **Fila → janela (D65 a).** `FilaAvisos::esperar_aprovacao` (`app/nucleo/src/daemon.rs`) tira só os `Aviso::Aprovacao`, na ordem, e deixa os `Destrave` para a vigia da Fase 31 (`esperar_destrave`). As duas vigias leem a mesma fila sem disputar item.
- **Núcleo `app/nucleo/src/aprovacao.rs` (novo).**
  - `Aprovacoes` guarda os pendentes por ordem de chegada. Ignora id repetido, tem limite de 16 (`MAX_AVISOS`), prazo de 120 s contado no app só para mostrar (o daemon é quem expira) e marca o estado `bloqueado`.
  - `exibir()` passa nome, comando e cada processo da cadeia por `ward_core::exibicao::neutralizar` (D51), mesmo que o daemon já tenha neutralizado. Uma `→` dentro do nome de um processo vira `\u{2192}`, para não imitar o separador.
  - `vigiar()` roda a thread `ward-app-aprovacao`.
  - `decisao_de_nome` aceita só `negar`, `uma_vez`, `uma_hora` e `sempre`.
- **Serviço (`servico.rs`).**
  - `pedido_de_aprovacao()` devolve o pedido já neutralizado e busca o tipo da entrada pelo detalhe.
  - `responder_aprovacao(id, decisao, pin)`:
    - id desconhecido dá `PedidoInvalido` e não chega ao daemon;
    - Negar vai sem PIN; os três Permitir vão com `Pin`, que zera ao sair;
    - PIN errado deixa o pedido pendente; no 5º erro (cofre travado), o pedido vira `bloqueado`;
    - num pedido bloqueado, Permitir dá `Travado` e Negar só o tira da lista.
  - `negar_aprovacoes_pendentes()` responde ao fechar a janela (S10).
- **Cliente (`cliente.rs`).** `responder_aprovacao` monta `Pedido::ResponderAprovacao`, recusa quando PIN e decisão não combinam, larga o quadro logo após a troca e mapeia `PinIncorreto`/`Expirado`.
- **Tauri (`app/src-tauri`).**
  - Janela `aprovacao`: 560×880, centrada, sempre no topo, focada, tema escuro, sem arrastar arquivo, devtools só com `modo-teste` e navegação só para `tauri://localhost`. A vigia a abre, ou traz para a frente, pela thread principal.
  - Capability nova `capabilities/aprovacao.json` (só a janela `aprovacao`, `local`) com `allow-ouvir-avisos`, `allow-pedido-de-aprovacao` e `allow-responder-aprovacao`.
  - Os comandos conferem o rótulo da janela de novo (`na_janela_de_aprovacao`). Quando não há mais pedido, a janela é destruída.
  - `WindowEvent::Destroyed` da janela tira o canal dela e nega todos os pendentes (S10).
- **Depois do merge da 31 (pedido do orchestrator): o mesmo `ouvir_avisos`/canal, sem segundo mecanismo.**
  - O canal virou um por janela (`Canais { principal, aprovacao }`). `ouvir_avisos` guarda o canal pelo rótulo dado pelo Tauri e recusa qualquer outra janela.
  - `avisar(canais, janela, aviso)` manda só o nome: `pedido-de-destrave` para a principal, `pedido-de-aprovacao` para a de aprovação.
  - No JS, `ouvirAvisos(aoChegar, aviso)` ganhou o nome do aviso, conferido numa lista fixa (`AVISOS`).
  - A janela consulta ao abrir, a cada aviso, depois de responder e quando a contagem chega a zero. A contagem anda localmente só para exibição.
  - Na ponte de teste, que não tem canal, vale a consulta periódica da 31 (`ouvirPorConsulta`).
- **Interface.**
  - `app/src/telas/JanelaAprovacao.svelte` e `aprovacao.js` (textos do design) cobrem os 5 estados.
  - Componentes novos `Contagem`, `CaixaDeCodigo` (texto puro, `direction:ltr; unicode-bidi:isolate`, ligaduras desligadas, quebra em qualquer ponto) e `BotaoIcone`. `Destaque` ganhou os tipos `perigo` e `prod`.
  - `App.svelte` escolhe a janela pelo rótulo do Tauri ou, no build de teste, por `&tela=aprovacao`.
  - Foco inicial em Negar; Esc nega; não há atalho de Enter; o X nega. O PIN é limpo a cada envio.
- **Ponte de teste.**
  - `--daemon-falso aprovacao | aprovacao-prod | aprovacao-expirando | aprovacao-ataque`, com os pedidos em `daemon_falso::aprovacoes_simuladas`.
  - O `DaemonFalso` responde `responder_aprovacao` com a regra do `wardd`: id desconhecido dá `Expirado`, o 5º PIN errado trava e nega, `sempre` deixa a entrada livre.
  - Com o daemon de teste real, a ponte também vigia as aprovações.
- **QA (Chrome headless, Playwright Python) em `docs/mission/evidence/fase-36/`.** Todas as 54 checagens de `qa.json` passaram, nenhuma falha.
  - PNGs da janela: `aprovacao-normal`, `-prod`, `-pin-errado`, `-pin-bloqueado`, `-expirando` e `-ataque-d51`.
  - PNGs dos frames do FigCraft: `figcraft-16701…17035`.
- `docs/mission/design-nodes.md` ganhou a seção "Fase 36" ligando componentes e frames ao código.
- **Testes do `app/src-tauri` isolados do daemon de teste do uid (pedido do orchestrator).** O `app_de_teste()` monta o serviço com `sem_daemon()`, só em `#[cfg(test)]`: um `DaemonSocket::sistema_em(<app/src-tauri/target/sem-daemon-<pid>/wardd.sock>)`, num caminho que não existe (conferido por `assert!`). Antes ele usava `DaemonSocket::padrao()`, que no `modo-teste` é o socket compartilhado `/run/user/<uid>/ward-teste/`. O caminho do release e o do `modo-teste` em produção não mudaram. Os testes da ponte usam o `DaemonFalso` e não precisaram de mudança.

## Segurança
- **Invariantes tocadas e como a fase as respeita.**
  - **S9:** o PIN só vai num `Pin`, que zera e cujo `Deserialize` não ecoa o valor. É limpo da tela a cada envio, nunca volta na resposta, e a contagem e a trava ficam no daemon. Depois do 5º erro, os Permitir são recusados no núcleo (`Travado`), não só desligados na tela.
  - **S10:**
    - a janela abre em primeiro plano e sempre no topo;
    - fechar (o X ou destruir a janela) nega todos os pendentes, sempre do lado de quem falha fechado;
    - Esc nega e não há atalho de Enter;
    - id que o daemon não mandou é recusado antes do daemon;
    - os 120 s são do daemon; a contagem é só exibição.
  - **S11:** cada janela tem sua capability. A principal não tem os comandos de aprovação; a de aprovação não alcança nenhum outro comando, nem plugin, nem `core:event`. Em comum, só `ouvir_avisos`, e o canal é guardado por rótulo de janela. Origem remota é negada. O comando confere o rótulo de novo.
  - **D51 / V5.29:** todo texto do agente (nome, comando, cada processo da cadeia) passa por `neutralizar` no Rust e entra na tela só como texto Svelte (`{…}`, nunca `{@html}`). U+202E, U+2066, largura zero, ESC e `\n` aparecem como escape visível, e HTML vira texto. A caixa força LTR isolado.
  - **S13:** nenhuma resposta da janela traz segredo ou PIN, e não há `localStorage`, URL nem estado global com o pedido.
- **Testes de ataque.**
  - `app/nucleo/src/aprovacao.rs`:
    - `ataque_texto_do_agente_vira_escape_visivel` (HTML, bidi, largura zero, ESC, `\n`, seta falsa na cadeia);
    - `ataque_pin_errado_cinco_vezes_trava_e_nega`;
    - `ataque_resposta_forjada_nao_chega_ao_daemon` (id inventado, decisão inventada, PIN no Negar, Permitir sem PIN);
    - `ataque_nada_de_segredo_ou_pin_na_janela` (nenhum valor de `SEGREDOS` nem o PIN em pedido ou resposta).
  - Mais em `aprovacao.rs`: `prazo_de_120_s_e_expirado_sai`, `ordem_de_chegada_sem_repetir_e_com_limite`, `vigia_tira_a_aprovacao_e_deixa_o_destrave`, `sempre_deixa_livre_e_fechar_nega_todos`.
  - `app/nucleo/src/daemon.rs` › `esperar_aprovacao_deixa_o_destrave_na_fila`.
  - `app/src-tauri/src/main.rs`:
    - `ataque_aprovacao_so_pela_janela_certa`: principal e outra janela negadas pela ACL, origem remota negada, a janela de aprovação não chega a 16 comandos e plugins, PIN e id fora do formato recusados na leitura e sem eco, resposta bem formada com id que o daemon não mandou recusada pelo serviço;
    - `ataque_aviso_de_uma_janela_nao_vai_para_a_outra`: canal por janela; rótulo desconhecido não recebe; canal tirado na destruição.
  - `app/nucleo/tests/configuracao_tauri.rs`:
    - `ataque_capability_com_permissao_a_mais`: duas capabilities exatas; em comum só `ouvir_avisos`; a principal sem os comandos de aprovação;
    - `janela_de_aprovacao_segue_o_rust_e_nao_usa_html`.
  - QA (`qa.json`):
    - nenhum caractere invisível no DOM e nenhum elemento `<b>`;
    - nem o PIN, nem o PIN errado, nem algum segredo do cofre de teste no DOM, nos inputs, no armazenamento, na URL ou no console;
    - Enter (com PIN vazio e com PIN certo) não permite.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **V5.16 de ponta a ponta com o `ward run` e o `wardd` reais** (CLI saindo ≠ 0 e marca inexistente; "1 hora" sem janela até travar). Não foi rodado: depende do binário do app com WebKitGTK e de sessão gráfica. O caminho foi conferido com o `DaemonFalso`, que segue a regra do `wardd`, e pela ACL no MockRuntime.
- **V5.20** (o papel agente não responde aprovação) é do daemon (23/23a). Esta fase não mexeu nisso.
- **V5.28** (foco e primeiro plano no COSMIC/Wayland) só se confere com o usuário.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `cargo clippy --all-targets --locked -- -D warnings` (app/src-tauri, PKG_CONFIG da R32) | 0 |
| `cargo test --locked` (app/src-tauri, 11 testes) | 0 |
| `cargo test -p ward-ponte-teste --all-features` | 0 |
| `python3 -I qa36.py …` (Playwright, Chrome headless) | 0 |
| `make app-front` | 0 |
| `make check` | 0 |
| `make app-check` (antes do isolamento, com o `wardd` de outro worktree no ar) | 2: 9/11 testes |
| `make app-check` (depois do `sem_daemon()`, com o mesmo `wardd` alheio ainda no ar) | 0: 11/11 |

## Problemas descobertos
- **Caracteres RTL fortes** (hebraico, árabe) não são escapados por D51. Mesmo com `unicode-bidi:isolate`, eles podem reordenar visualmente o trecho em que estão. Proposta: decidir no escopo se `neutralizar` escapa as letras RTL ou se basta marcar a presença delas.
- **Texto do bloqueio:** o `CampoPin` (Fase 35) diz "PIN bloqueado: o cofre foi travado.", e o frame `16960` diz "PIN bloqueado após 5 erros. O cofre foi travado.". A janela usa o componente compartilhado.
- **Fechar a janela nega todos os pendentes,** inclusive os da fila que o usuário ainda não viu. É fail-closed, mas o escopo fala em "o pedido".
- **Corrida na destruição:** um pedido que chegue entre a destruição da janela e a negação pode ser negado sem ter sido mostrado. É fail-closed.
- **Comandos síncronos do Tauri** falam com o daemon na thread de IPC, com a mesma limitação das fases anteriores.
- **O `PedidoDeAprovacao` do protocolo não traz o prazo.** O app conta 120 s desde a chegada; a contagem pode adiantar ou atrasar alguns ms em relação ao daemon.
- **Restaurar backup (Fase 45)** não cancela aprovações pendentes no app. O daemon responde `Expirado`, e a janela trata.
- **`npm audit`** mostra 2 vulnerabilidades low em dependência de desenvolvimento (`deepmerge`, que veio do master). `make app-front` sai 0 (o limite é moderate).
- **(Resolvido nesta fase.) `make app-check` dependia de não haver daemon de teste no ar.** Durante a fase, outro worktree (`agent-ae99eafcfad2810a2`) deixou um `wardd` de teste escutando em `/run/user/1000/ward-teste/wardd.sock`. Esse socket é compartilhado: o caminho vem do uid, e trocar o `XDG_RUNTIME_DIR` não muda nada.
  - Com o daemon no ar, dois testes do master pressupõem "sem daemon" e falharam: `ataque_comandos_proprios_so_na_janela_e_origem_certas` (Fase 29) e `ataque_destravar_so_na_janela_certa_e_senha_sem_eco` (Fase 31).
  - O segundo chegou a mandar a senha mestra de teste (falsa) ao daemon do outro agente e o destravou.
  - Com um tmpfs sobre o diretório (bwrap), os dois passam. Sem bwrap, os outros 9 passam; o único que falha dentro do bwrap (`criar_cofre`, por falta de Wayland) passa fora dele.
  - Não mexi no processo alheio. Corrigido com o `sem_daemon()` (ver Implementado). Continua valendo para quem roda `make dev-daemon`/`teste-cli`: o socket de teste é um só por uid, compartilhado entre worktrees.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/aprovacao.rs` | criado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/tests/configuracao_tauri.rs` | alterado |
| `app/src-tauri/capabilities/aprovacao.json` | criado |
| `app/src-tauri/tauri.conf.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/lib/ponte/index.js` | alterado |
| `app/src/lib/componentes/Contagem.svelte` | criado |
| `app/src/lib/componentes/CaixaDeCodigo.svelte` | criado |
| `app/src/lib/componentes/BotaoIcone.svelte` | criado |
| `app/src/lib/componentes/Destaque.svelte` | alterado |
| `app/src/telas/JanelaAprovacao.svelte` | criado |
| `app/src/telas/aprovacao.js` | criado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-36/*` (11 PNG, `qa.json`) | criado |
| `docs/mission/handoffs/phase-36.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks); `make app-check` e `make app-front` saem 0
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia, só leitura)
- [x] Ficou dentro do escopo da fase
