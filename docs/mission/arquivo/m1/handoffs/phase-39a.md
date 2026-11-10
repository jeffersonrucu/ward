# Fase 39a — Aviso `pedido_de_destrave` com entrada e comando (D51), V5.8

## Implementado
- **Protocolo (`ward-core`)**: `Resposta::PedidoDeDestrave` passou de unidade para
  `PedidoDeDestrave { entrada, operacao, comando, restante }`. As constantes novas
  `PedidoDeDestrave` e `MAX_RESTANTE_DESTRAVE` (120) são exportadas em `protocolo.rs`. O
  tipo continua só do papel app (`SO_APP`).
  - Leitura estrita: entrada não vazia e de até `MAX_TEXTO`; operação só `executar`,
    `ssh`, `login` ou `playwright`; comando de até `MAX_TEXTO_EXIBIDO`; `restante` em
    1..=120; nenhum campo a mais.
  - A escrita (`conferir`) aplica a mesma regra: o daemon não consegue montar um aviso
    fora do formato.
- **`wardd`**:
  - `uso.rs`: com o cofre travado, o daemon monta o aviso. A entrada é o nome que o
    agente mandou, por `exibicao::exibir_texto(.., MAX_TEXTO)`. O comando é o mesmo que a
    janela de aprovação usa (`exibir_argumentos`; vazio no `login` e no `playwright`).
    Segunda camada: nome vazio responde `entrada_inexistente` antes de qualquer espera.
  - `aprovacao.rs`: o contador `esperando_destrave` virou uma lista de esperas
    (id, prazo, aviso). `esperar_destrave(aviso, travado, cliente)` entrega o aviso com
    `restante` calculado pelo relógio do motor. Ao assinar, o app recebe só o mais novo,
    com o que ainda falta do prazo. Cada espera sai da lista pelo id.
- **App (`ward-app-nucleo`)**:
  - `Aviso::Destrave(PedidoDeDestrave)`.
  - `FilaAvisos::esperar_destrave` devolve o mais novo (`Option`). Os de aprovação
    continuam na fila, na ordem.
  - `PedidoPendente::marcar(&p)`: passa entrada e comando de novo por `neutralizar` (D51).
    O prazo é o `restante` do daemon, limitado a 120 s, e não mais 120 s contados da
    chegada. Isso resolve o "contagem do app × prazo do daemon" do handoff 31.
  - `InfoPedido` agora é `{id, restante, entrada, operacao, comando}`. Deixou de ser
    `Copy`.
  - Só em teste ou `modo-teste`: `Aviso::destrave_de_teste` e
    `Aviso::destrave_hostil_de_teste`.
- **Interface**:
  - `Destravar.svelte`, cartão `Pedido do agente` (frame `3450`, cartão `3501`):
    - linha `entrada` (`3503`): avatar `controle-md` com `Ícone/chave`, o nome da entrada
      e, embaixo, o comando da operação (`ward run`, `ward ssh`, `ward login`,
      `ward playwright`);
    - `Comando` (`3504`) pelo `CaixaDeCodigo.svelte` da Fase 36, só quando há comando;
    - contagem como antes.
  - O texto fixo "sem detalhe" saiu. O nome é texto puro, com `unicode-bidi: isolate`,
    `direction: ltr` e sem ligaduras.
  - `App.svelte`: `pedidoDe()` copia só os cinco campos conhecidos. Os textos que não
    vierem como string viram `''`.
- **Ponte de teste**: modo novo `--daemon-falso travado-com-pedido-ataque`, com entrada
  e comando crus: `ESC`, U+202E, U+200B e HTML. O `travado-com-pedido` agora traz
  `acme-pg-aprovacao` e o comando `psql`.
- **QA no Chrome** (headless, 1280×800, ponte com o daemon falso; detalhes em
  "Problemas"):
  - `travado-com-pedido`: o cartão mostra `acme-pg-aprovacao`, `ward run`, o comando e
    a contagem;
  - `-ataque`: o DOM tem `acme-pg-\u{202E}vorpa\u{200B}&lt;b&gt;…`. Nenhum caractere
    invisível ficou no HTML, e o único `<img>` é o logo, sem o `onerror`;
  - capturas em `docs/mission/evidence/fase-39a/` (`destravar-pedido-do-agente.png`,
    `destravar-pedido-do-agente-ataque.png`), ao lado do PNG do frame `3450` exportado
    de uma **cópia** do `Ward.figcraft` (`figcraft-3450-destravar-pedido-do-agente.png`).
    A 39d mexe no original em paralelo.

## Segurança
- **Invariantes tocadas:**
  - **S1**: o aviso não leva segredo. A entrada é o nome que o agente mandou (com o
    cofre travado nem se sabe se existe), e o comando é o mesmo que a janela de
    aprovação já mostra. Tipo e ambiente não vão: o índice está cifrado.
  - **S11 / D47**: o tipo continua só do papel app. Agente e extensão não codificam nem
    leem esse quadro.
  - **D51**: a fronteira é dupla. O daemon formata com escape visível e o app passa por
    `neutralizar` antes da tela. A tela mostra como texto, isolado de bidi.
  - **D65 a**: o aviso continua um quadro inteiro e vai para a fila limitada a 16.
  - **S10**: o prazo de 2 min fica no daemon. O app só mostra o `restante` que recebeu,
    nunca acima de 120.
  - **Negação de serviço, achada e fechada na fase**: um aviso fora do formato (por
    exemplo, entrada vazia) faria `AssinanteSocket::entregar` falhar e fechar a conexão
    do app, o que nega o que ele tinha na tela. Hoje o protocolo recusa entrada vazia
    na leitura, e o `uso.rs` recusa de novo antes de montar o aviso.
- **Testes de ataque:**
  - `crates/ward-core/src/protocolo/testes.rs` ›
    `ataque_fase_39a_pedido_de_destrave_so_app_e_formato_estrito`: agente e extensão
    não recebem; recusa `restante` 0, 121, negativo e texto; recusa operação `lista`,
    `destravar` e maiúscula; recusa entrada vazia, grande e numérica; recusa campo a
    mais (`senha` em base64, `ambiente`); recusa o aviso antigo vazio e o aviso sem
    comando; a escrita recusa aviso fora do formato.
  - `crates/wardd/tests/aprovacao.rs`:
    - `ataque_fase_39a_destrave_com_entrada_e_comando_neutralizados`: nome com
      U+202E, ESC, U+200B e tag U+E0041, argv com aspas que fingem fechar, ESC e
      U+202E em bytes. Nada invisível chega e a forma é exata. Reassinar aos 50,4 s
      dá `restante` 70. O segundo uso (`ssh`) toma o lugar. Reassinar entrega só o
      mais novo. Sem espera, ninguém recebe nada;
    - `ataque_fase_39a_entrada_vazia_nao_derruba_o_app`: `executar` com entrada vazia
      dá `pedido_invalido`; `login`/`playwright` montados direto dão
      `entrada_inexistente`. Nenhum aviso sai, o app segue assinado e o pedido seguinte
      chega.
  - `app/nucleo/src/destrave.rs`:
    - `ataque_entrada_e_comando_crus_chegam_neutralizados`: controle, bidi, BOM, C1 e
      DEL crus viram escape; o que já era escape não dobra; o JSON leva só as 5 chaves;
    - `prazo_vem_do_daemon_e_nunca_passa_de_2_min`.
  - Atualizados:
    - `crates/wardd/src/servidor.rs` › `assinante_escreve_quadro_inteiro_e_falha_fechado`
      (ida e volta do aviso com campos);
    - `app/nucleo/src/daemon.rs` (`esperar_destrave` devolve o mais novo; V5.33 com o
      aviso novo);
    - `app/nucleo/src/servico.rs` (chaves do JSON da tela).
  - QA no Chrome, modo `-ataque`: escape visível no DOM, HTML como texto.
- **Dependências novas:** nenhuma no repositório. Para o QA tentei o Playwright do
  Python em wheels no scratchpad (sha256 conferido com o PyPI), mas o classificador de
  permissões bloqueou executá-lo. Usei o Chrome do sistema headless
  (`--screenshot`/`--dump-dom`, perfil temporário no scratchpad).

## Ficou de fora
- **Selos de tipo e ambiente do cartão** (`Badge tipo` `3520`, `Badge ambiente` `3525`):
  não existem com o cofre travado. No lugar vai o comando da operação, e o avatar usa
  `Ícone/chave` em vez do ícone do tipo.
- **`make app-check` / app Tauri real**: `libwebkit2gtk-4.1-dev` não está instalada
  (pendência do usuário no plano). Em `app/src-tauri` só mudou um comentário, e o
  `InfoPedido` deixou de ser `Copy` sem uso que dependa disso (`main.rs:84,148`).
  Falta rodar quando a biblioteca existir.
- **Ponta a ponta com o `wardd` de teste e a ponte** (`ward run` travado → tela): fica
  para a 39c, que cobre o ponta a ponta. Não subi daemon de teste nesta fase. O caminho
  daemon → quadro → fila → tela está coberto por unidade e integração (motor com app
  falso, `AssinanteSocket` real, transporte do app).
- **Playwright interativo** (digitar a senha, "Agora não", destravar): não refeito. A
  lógica da tela não mudou desde a 31, só o conteúdo do cartão.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --lib protocolo` | 0 |
| `cargo test -p wardd --test aprovacao` (1ª do teste de entrada vazia esperava `entrada_inexistente`, mas o protocolo já dá `pedido_invalido`; teste ajustado) | 101, depois 0 |
| `cargo test -p wardd --lib servidor` | 0 |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `cargo test -p ward-ponte-teste --all-features` | 0 |
| `make ferramentas` | 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso travado-com-pedido` e `travado-com-pedido-ataque` + Chrome headless (`shot.sh` no scratchpad) | 0 |
| `make check` (1ª: clippy `redundant_closure` no teste novo; 2ª: `v3_33_kill_9_entre_as_senhas_erradas` por tempo, com a máquina carregada pelos workers em paralelo, `[2.24s, 3.10s, 3.66s]`; fora do caminho desta fase) | 2, 2, depois 0 |

## Problemas descobertos
- **Para a 39d (design)**: o frame `3450` mostra `Badge tipo`/`Badge ambiente` e o
  comando no formato `ward run acme-pg-aprovacao -- psql -c "…"`. Com o cofre travado
  não há tipo nem ambiente. O comando segue D51 como na janela de aprovação: argv entre
  aspas (`"psql" "-c" "select count(*) from pedidos"`) mais a linha `ward run`. Vale
  ajustar o frame: avatar com `Ícone/chave`, sem selos, a linha da operação e o
  comando no formato D51. O `3450` não está na lista da 39d.
- **Vários usos esperando o destrave**: o app mostra só o mais novo. Destravar libera
  todos, e os "livre" rodam sem a janela. Já era assim; agora o cartão nomeia um só.
  Uma linha "e mais N pedidos" pediria contagem no aviso e design. Decisão do planner.
- **"Agora não" ainda não nega na hora** (handoff 31): segue igual, falta operação no
  protocolo.
- **Ids do motor**: as esperas de destrave usam o mesmo contador `proximo_id` das
  aprovações. Os ids de aprovação continuam únicos e crescentes, só pulam números.
- **`v3_33_kill_9_entre_as_senhas_erradas`** é sensível à carga da máquina: falhou uma
  vez com vários `cargo` de outros worktrees em paralelo e passou na rodada seguinte.
- **Merge**: `protocolo.rs` (linha do `pub use resposta::{…}` e descrição do tipo na
  `tabela!`) e `resposta.rs` (`PedidoDeDestrave` no enum, `tipo`, `conferir`,
  `escrever`, `ler`) podem conflitar com a 39b se ela mexer nas mesmas listas. As
  mudanças estão nesses pontos e no struct novo, logo antes de `Sugestao`.
- O PRD (D65 a) diz "o evento leva só o texto do aviso". O evento do Tauri continua só
  com o nome (`pedido-de-destrave`), e os dados vão pelo `estado_do_cofre` › `pedido`.
  Não mexi no PRD.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/wardd/src/aprovacao.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado (teste) |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/destrave.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado (teste) |
| `app/nucleo/src/aprovacao.rs` | alterado (teste) |
| `app/src-tauri/src/main.rs` | alterado (comentário) |
| `app/src/App.svelte` | alterado |
| `app/src/telas/Destravar.svelte` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado (linha do `3450`) |
| `docs/mission/evidence/fase-39a/*.png` (3) | criado |
| `docs/mission/handoffs/phase-39a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (nenhum daemon de teste subido;
  daemon falso da ponte)
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI) Conferido contra o FigCraft (cópia do arquivo; medidas do `3501` por
  `get_node`)
- [x] Ficou dentro do escopo da fase
