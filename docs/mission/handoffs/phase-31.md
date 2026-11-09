# Fase 31 — Destravar, inclusive o estado "Pedido do agente" (UI)

## Implementado
- **Nenhuma operação nova no protocolo.** A fase usa `destravar` (agente e app) e o aviso
  `pedido_de_destrave` (só app), que já existem em `ward_core::protocolo`.
- **`app/nucleo/src/destrave.rs` (novo):**
  - `SenhaMestra`: a senha que vem da interface, em `Zeroizing<String>`. Não tem `Clone`,
    `Display` nem `Serialize`; o `Debug` mostra `SenhaMestra(••••)`. O `Deserialize`
    recusa vazio, acima de `MAX_SEGREDO`, número, booleano, nulo, lista e objeto, sempre
    com o texto fixo "senha mestra fora do formato" (sem eco);
  - `PedidoPendente`: guarda o prazo do `pedido_de_destrave` (120 s, o `PRAZO_DESTRAVE`
    do `wardd`, contado da chegada do aviso) e um id que muda a cada aviso. `info()`
    devolve `{id, restante}` ou `None` depois do prazo; `limpar()` ao destravar;
  - `vigiar(fila, pedido, ao_chegar)`: thread que tira os `pedido_de_destrave` da fila da
    conexão (D65 a), marca o pedido e chama `ao_chegar`.
  - O aviso pode chegar antes de a interface existir: o daemon abre o app por causa
    dele e o entrega assim que a conexão assina (`Motor::assinar` → `entregar_pendentes`).
    Por isso o prazo fica no lado Rust e a tela o lê no `estado_do_cofre`.
- **`FilaAvisos::esperar_destrave(prazo)`** (`daemon.rs`): tira só os `Destrave`; os
  `Aprovacao` ficam na fila, na ordem, para a Fase 36. `simular(aviso)` existe só em
  teste e `modo-teste`.
- **`cliente::destravar`** e **`servico::destravar(SenhaMestra)`**: a senha vai num
  `Segredo` do núcleo, só no pedido `destravar`. `ErroResposta::SenhaIncorreta` vira
  `ErroApp::SenhaIncorreta` ("Senha mestra incorreta.") e `Retrocesso` vira
  `ErroApp::CofreRetrocesso`. `servico::info_do_pedido(estado)` só devolve o pedido com
  o cofre travado.
- **Daemon falso:** `DaemonFalso::travado()` e `destravar` com a mesma regra do `wardd`.
  A espera de D20 vem antes de olhar a senha, a senha de teste (D15) é conferida em tempo
  constante (`ctutils`) e o acerto zera as falhas de senha e de PIN.
- **Comandos novos** em `comandos.rs`, na capability (`allow-destravar`,
  `allow-ouvir-avisos`), em `ponte/comandos.js`, no Tauri e na ponte:
  - `destravar { senha }`: resposta `null`;
  - `ouvir_avisos { canal }`: canal do Tauri (`tauri::ipc::Channel`) por onde a vigia
    avisa `{"aviso":"pedido-de-destrave"}`.
  - `estado_do_cofre` agora responde `{estado, pedido}`, com `pedido` igual a
    `{id, restante}` ou `null`.
- **Ponte de teste:**
  - novos modos `--daemon-falso travado` e `travado-com-pedido` (o aviso chega 3 s
    depois de subir);
  - com o daemon de verdade, a vigia lê a fila do `DaemonSocket`;
  - `ouvir_avisos` responde `pedido-invalido`. Na ponte, a tela consulta o estado a cada
    2 s, uma consulta por vez (`ponte-teste.js` › `ouvirPorConsulta`).
- **Interface:**
  - `PainelDeMarca.svelte` (`717`) e `CampoSenha.svelte` (set `17502`: vazio, foco,
    erro);
  - `telas/Destravar.svelte` com os três estados:
    - **Padrão:** selo, campo, "Destravar", nota da trava automática;
    - **Senha errada:** erro no campo, campo esvaziado, foco no campo, botão desabilitado
      "Tente de novo em N s" (1, 2, 4… até 30, a regra de D20) e nota de D20;
    - **Pedido do agente:** cartão com contagem "m:ss restantes", "Agora não" (esconde
      aquele pedido pelo id) e "Destravar", mais a nota de alerta.
  - `App.svelte`:
    - travado vai para `Destravar`;
    - `ouvirAvisos` consulta o estado quando chega o aviso;
    - **o 5º PIN errado da Fase 35 leva a `Destravar / Padrão`** (o `ontravado` →
      `consultar()` já existia e agora cai na tela nova).
- **QA no Chrome** (Playwright Python headless, 1280×800, ponte com `--daemon-falso
  travado-com-pedido`): as 38 checagens deram `true` (`docs/mission/evidence/fase-31/qa.json`).
  - Pedido do agente: chega e a contagem anda.
  - "Agora não" esconde o pedido, e ele não volta na consulta.
  - Campo vazio pede a senha.
  - Senha errada 3 vezes: espera de 1, 2 e 4 s, campo vazio, foco no campo,
    `aria-invalid`.
  - Senha certa destrava e mostra a casca.
  - O 5º PIN errado no diálogo de nível leva a `Destravar / Padrão`.
  - Não ficou senha no DOM, nos `value`, no storage, no cookie, na URL nem no console.
  - Só houve pedidos locais.

  Capturas: `destravar-padrao.png`, `destravar-senha-errada.png`,
  `destravar-pedido-do-agente.png`, `destravado-credenciais.png` e
  `quinto-pin-destravar-padrao.png`, ao lado dos PNG dos frames `3342`, `3393` e `3450`
  exportados da cópia.

## Segurança
- **Invariantes tocadas:**
  - **S13 / D20:**
    - o erro do destravar tem mensagem fixa, sem a senha e sem contagem; a 1ª e a 5ª
      falha dizem o mesmo;
    - a espera é do daemon. A tela só mostra a mesma regra, contada pelos erros dela;
  - **senha em memória:**
    - no Rust, fica em `SenhaMestra` (`Zeroizing`), depois em `Segredo` no pedido, e o
      quadro sai como `Zeroizing`;
    - nenhuma resposta, erro ou log a carrega;
    - na tela, fica só no estado local de `Destravar.svelte` e sai dele no clique, antes
      da resposta. Também é apagada ao sair da tela. Nada vai a `localStorage`, URL ou
      estado global, e o campo tem `autocomplete="off"`;
  - **S1:** a entrada do comando não é confiável. Formato errado é recusado na leitura
    (Tauri) ou antes do serviço (ponte), e não conta como tentativa;
  - **S11 / ACL:**
    - `destravar` e `ouvir_avisos` só passam na janela `principal` com a origem local;
    - o aviso usa um `Channel` do Tauri e **não** `emit`/`listen`: a capability continua
      sem nenhuma permissão `core:*`, e `plugin:event|listen/emit/emit_to` seguem negados;
  - **D65 a:** o aviso leva só o nome. O pedido de aprovação não é consumido aqui; segue
    na fila para a 36.
- **Testes de ataque:**
  - `app/nucleo/src/destrave.rs`:
    - `ataque_senha_fora_do_formato_e_recusada_sem_eco`;
    - `ataque_debug_da_senha_nao_mostra_o_valor`;
    - `pedido_conta_de_120_s_recomeca_e_some`;
    - `vigia_tira_o_destrave_e_marca_o_pedido`.
  - `app/nucleo/src/daemon.rs` › `esperar_destrave_deixa_a_aprovacao_na_fila`.
  - `app/nucleo/src/servico.rs` › `testes_destravar`:
    - `senha_errada_nao_destrava_e_a_certa_destrava`;
    - `ataque_erro_do_destravar_sem_senha_nem_contagem`;
    - `ataque_a_senha_so_vai_no_pedido_destravar` (nenhum outro quadro leva a senha,
      nem em base64);
    - `pedido_do_agente_aparece_so_travado_e_some_ao_destravar`;
    - `daemon_falso_espera_d20_antes_de_olhar_a_senha`.
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_destravar_pela_ponte`: formatos
    errados, campo a mais, sem eco, D20 e `ouvir_avisos` recusado.
  - `app/src-tauri/src/main.rs` › `ataque_destravar_so_na_janela_certa_e_senha_sem_eco`:
    outra janela, origem remota, senha vazia, acima do limite, número, nulo ou lista,
    sem eco; `plugin:event|*` negado.
  - QA no Chrome (`qa.json`): `sem_senha_no_dom:*`, `armazenamento_vazio`,
    `console_sem_senha`, `urls_sem_senha`, `so_pedidos_locais`.
- **Dependências novas:** nenhuma. `tauri::ipc::Channel` e o `Channel` de
  `@tauri-apps/api/core` já vinham nos pacotes. `ctutils` já era dependência do núcleo.

## Ficou de fora
- **Entrada, tipo, ambiente e comando no cartão "Pedido do agente".** O
  `pedido_de_destrave` do protocolo não tem campo (`Resposta::PedidoDeDestrave`), e com o
  cofre travado o daemon nem sabe tipo e ambiente. O cartão mostra "Pedido do agente", a
  contagem e um texto fixo no lugar do avatar, dos selos e do comando. **O V5.8 pede
  "entrada, comando e contagem" e o design os mostra: o V5.8 fica parcial.** Decisão do
  planner (ver Problemas).
- **Teste pelo daemon real (`make app-ponte`).** Havia um `wardd` de teste de outro worker
  no socket fixo do uid (`/run/user/1000/ward-teste/wardd.sock`, pid do checkout
  principal). Subir o meu derrubaria o dele, então não foi feito. O caminho real está
  coberto por unidade (fila → vigia → pedido) e pelo transporte da 23a. Falta conferir
  ponta a ponta com um `executar` do agente esperando o destrave.
- **App Tauri real** (janela WebKitGTK): não aberto nesta fase. A entrega do aviso pelo
  `Channel` só foi conferida na ACL (runtime falso) e na ponte (por consulta).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (o download do gitleaks deu 504 no GitHub; binário copiado do `.dev/ferramentas` do checkout principal e conferido por sha256 em `make gitleaks-binario`) | 2, depois 0 (`gitleaks-binario`) |
| `cargo test -p ward-app-nucleo --all-features` (1ª: contagem de quadros no teste, corrigida) | 101, depois 0 |
| `cargo test -p ward-ponte-teste --all-features` | 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso travado-com-pedido` + Playwright (`qa31.py`; as 2 primeiras rodadas falharam no seletor do roteiro do diálogo de nível) | 0 |
| `make fmt` e `cargo fmt` em `app/src-tauri` | 0 |
| `make app-check` com o `PKG_CONFIG_PATH` do scratchpad (R32): 1ª falhou no `fmt --check` do `main.rs`; a 2ª passou nos 8 testes de ACL (o `ataque_comandos_proprios_so_na_janela_e_origem_certas` passou mesmo com o `wardd` de outro worker no ar) | 2, depois 0 |
| `make check` | 0 |
| `make app-front` (final, depois do `make check`) | 0 |

## Problemas descobertos
- **V5.8 × protocolo (para o planner).** O contrato e o frame `3450` mostram entrada,
  tipo, ambiente e comando, mas o `pedido_de_destrave` não tem campo. D65 a diz "o evento
  leva só o texto do aviso", e esse texto não existe. Para fechar o V5.8, o aviso
  precisaria levar ao menos o nome da entrada e o comando já neutralizado (D51), como o
  `PedidoDeAprovacao`. Tipo e ambiente não existem com o cofre travado (o índice é
  cifrado). Isso mexe em `ward-core` (protocolo), `wardd` (`uso.rs`, `aprovacao.rs`) e
  no leitor do app. Não fiz por ser mudança de protocolo, fora da fase.
- **"Agora não" não nega na hora.** O protocolo não tem operação para o app recusar o
  destrave. "Agora não" só esconde o cartão, e o pedido do agente expira no daemon em
  até 2 min (`travado_tempo_esgotado`). A nota do design diz que "Agora não … nega o
  pedido". Ela fica certa no resultado (negado), mas não no tempo.
- **Contagem do app × prazo do daemon.** O app conta 120 s a partir da chegada do aviso,
  e o daemon conta do começo da espera. A diferença é a latência do aviso, então o app
  pode mostrar alguns décimos a mais.
- **Espera de D20 em dobro na interface.** O daemon dorme a espera **antes** de olhar a
  senha da tentativa seguinte. A tela segue o design: botão desabilitado "Tente de novo
  em N s" logo depois do erro. Quem tenta de novo espera N s na tela e mais N s no
  daemon. É seguro, mas lento: no limite, 30 s + 30 s. Alternativa: só mostrar a
  contagem durante o pedido em curso. Decisão de UX.
- **Canal do Tauri em vez de `emit`/`listen`.** D65 a fala em "evento do Tauri". O
  `listen` exige `core:event:allow-listen`, e o teste de ACL existente proíbe qualquer
  permissão do núcleo (`plugin:event|listen` negado). Por isso usei
  `tauri::ipc::Channel`, que entra como argumento de um comando próprio
  (`ouvir_avisos`). Não testei a entrega pelo canal no app Tauri real, só a ACL no
  runtime falso.
- **Senha errada esvazia o campo.** O frame `3393` mostra os pontos no campo. O app
  esvazia, para não segurar a senha errada; o foco fica (V5.8). É a mesma decisão da 35
  com o PIN. Ajustar o design.
- **Botão "Destravar" com o campo vazio fica clicável**, como no design. O clique mostra
  "Digite a senha mestra." sem chamar o daemon.
- **Largura do `Painel de marca`.** No FigCraft a largura é fixa em 560, sem variable
  própria. Usei `largura/dialogo-largo`, que tem o mesmo valor. Vale criar uma variable
  numa fase de design.
- **Conflito provável com a Fase 30 (em paralelo).** O Primeiro uso também usa `Painel de
  marca` e `Campo senha`. Criei `PainelDeMarca.svelte` e `CampoSenha.svelte` com esses
  nomes para que o merge acuse o conflito, em vez de duplicar com nomes diferentes.
- **Conflito provável com a 22c** em `daemon_falso.rs`, `cliente.rs` e `servico.rs`: só
  acréscimos localizados.
- **Fila de avisos antes da 36.** Até a janela de aprovação consumir, os
  `pedido_de_aprovacao` ficam na fila. Com 16 deles parados, um `pedido_de_destrave`
  novo seria descartado e o pedido do agente expiraria (falha fechada, D65 a). Isso some
  quando a 36 consumir.
- **Ponte:** o corpo HTTP do `destravar` (`Pedido::corpo`, `Vec<u8>`) não é zerado. É
  só `modo-teste` com a senha falsa. O mesmo vale para o PIN da 35.
- **Senha no JS:** a string do JS não zera, e a cópia no motor fica até o coletor de
  lixo. Não há como evitar na webview; é o mesmo caso do PIN e do formulário.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/destrave.rs` | criado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/lib/componentes/CampoSenha.svelte` | criado |
| `app/src/lib/componentes/PainelDeMarca.svelte` | criado |
| `app/src/telas/Destravar.svelte` | criado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/lib/ponte/index.js` | alterado |
| `app/src/lib/ponte/ponte-teste.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-31/*` (PNG e `qa.json`) | criado |
| `docs/mission/handoffs/phase-31.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; nada gravado no original)
- [x] Ficou dentro do escopo da fase
