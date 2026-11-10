# Fase 43 — `ward login`: daemon → `ward-nm` → aba ativa, preenche, envia e limpa

## Implementado
- **Protocolo** (`ward-core`): três operações só do papel extensão, a saber `aguardar_login`, `preencher_login{id, origem}` e `resultado_login{id, resultado}`. Uma resposta nova, `pedido_de_login{id, entrada}`, que é só da extensão e não leva valor. Mais `ResultadoLogin` (`enviado`, `recusado`, `sem_formulario`, `aba_mudou`, `falha`) e `id_login_ok` (32 caracteres alfanuméricos). Formato estrito, como no resto. Asserções de compilação: nenhuma das novas é do agente ou do app. A tabela do V3.15 foi atualizada.
- **Daemon** (`crates/wardd/src/login.rs`): `Pedido::Login` do agente segue esta ordem.
  1. Tipo sem entrega → recusa antes da aprovação. Bloqueada → "entrada inexistente".
  2. `autorizar_uso` (níveis e janela de aprovação de D3) e `registrar_uso`.
  3. Cria um trabalho com id de `token_conectar()`, com só um por vez (`Ocupado`).
  4. O daemon não chama a extensão. O service worker dela pergunta `aguardar_login` em long-poll de 25 s pela porta de native messaging, recebe `{id, entrada}`, sonda a aba ativa e manda `preencher_login` com o id e a origem do quadro principal.
  5. O daemon compara o id em tempo constante (`tokens_iguais`) e a origem pela mesma regra de D9 do `preencher` (`Origem::da_url`). Só então entrega o preenchimento, e uma vez só (fase `Entregue → Preenchido`).
  6. O desfecho volta por `resultado_login`:
     - `enviado` → `ok`;
     - `recusado` ou `sem_formulario` → `login_recusado`;
     - `aba_mudou` → `origem_diferente`;
     - `falha` → `navegador_falhou`.
  7. Prazos: 10 s para a extensão pegar o pedido e 60 s no total. O pedido morre se a CLI cair (`cliente.saiu()`) ou se o cofre travar. `aguardar_login` não conta como uso para a trava por inatividade (`trava.rs`).
- **`ward-nm`**: as três operações na lista do host e `resposta_cabe` estrito: `aguardar_login → pedido_de_login|ok`, `preencher_login → preenchimento`, `resultado_login → ok`.
- **CLI**: `ward login <entrada>` (args, `--help`, mensagem de uso). Pede pelo socket com o prazo do playwright e imprime `login enviado: <entrada> (aba ativa do Chrome)`. As mensagens de erro:
  - origem diferente → "domínio não confere: a origem da aba ativa não é a da entrada; nada foi preenchido";
  - sem extensão → "a extensão do Ward no Chrome não respondeu (o Chrome está aberto?)";
  - recusado → "a página não tem um formulário de login seguro ou o site não aceitou o login".
- **Extensão**: reaproveita o service worker `servico.js` da Fase 42, que só importa e chama `iniciarLogin()`. O content script fixo (`envio.js`) não foi tocado. O módulo novo é `extensao/login.js`:
  - porta `connectNative`, que também mantém o SW vivo, com fila de respostas, prazo de 60 s e reconexão;
  - laço `aguardar_login`;
  - `atender`: aba ativa da última janela em foco, sonda só no quadro principal (`frameIds:[0]`), `preencher_login`, injeção só no `documentId` sondado (se a aba navegou, o Chrome recusa), apagar os argumentos, esperar a página seguinte (`tabs.get().status` e depois `sondarDepois`) e mandar o `resultado_login`.
- **`pagina.js`** (função injetada): `preencherNaPagina(..., enviar = true)` exige estas condições.
  - Origem conferida de novo no documento.
  - Exatamente um campo de senha visível dentro de um `<form>`, mais o campo de usuário se a entrada tem usuário.
  - `action`, `method` e `target` lidos pelos getters de `HTMLFormElement.prototype`, para que `<input name="action">` não tape o valor: mesma origem, POST, sem target ou `_self`. O `formaction`, `formmethod` e `formtarget` do botão de envio passam pela mesma regra.
  - Então preenche, chama `requestSubmit` (o corpo é montado de forma síncrona) e limpa usuário, senha e TOTP num `finally`.
  - `sondarDepois` diz origem, topo, `readyState` e se ainda há senha visível.
- **E2E** `make teste-login` (`dev/testes/login.sh` e `login.py`): Chrome real headless (perfil em `.dev/login.*`, `--remote-debugging-pipe`), extensão por `Extensions.loadUnpacked`, `ward-nm` filho do Chrome e um `wardd` só dele (`WARD_DEV_SOCKET_DIR` próprio, `WARD_DEV_EXTENSAO=chrome`). Páginas hostis na origem certa são servidas pelo CDP (`Fetch.fulfillRequest`), que também registra o que cada POST mandou (por sha256). Resultado: **tudo certo**, 31 checagens em `docs/mission/evidence/fase-43/qa.json` e o screenshot `v6-10-painel-wordpress.png`.

## Segurança
- **Invariantes tocadas**
  - **S1/D9**: quem compara a origem é o daemon, na mesma função do `preencher`. A extensão só manda a origem do quadro principal e a confere de novo dentro do documento antes de preencher. O formulário só é enviado à mesma origem por POST.
  - **S13**: o daemon registra só o nome da entrada e o desfecho, nunca a origem da aba nem valor. O `login.js` não usa o console (o `montar.py` recusa `console.` nele).
  - **D3/V6.11**: o nível passa por `autorizar_uso` antes de existir o trabalho. Negado, a extensão nunca recebe o pedido.
  - **D5/D47**: as operações novas são só do papel extensão (tabela e asserção de compilação). O agente e o app recebem `nao_autorizado`.
  - **S2**: nada é guardado; o perfil foi varrido.
  - **V3.17**: bloqueada responde como inexistente.
- **Testes de ataque** (todos passam):
  - `crates/ward-core/src/protocolo/testes.rs` › `ataque_fase_43_login_so_extensao_e_formato_estrito`, mais a tabela do V3.15.
  - `crates/wardd/tests/login.rs`:
    - › `v6_3_v6_15_ataque_origem_diferente_nao_preenche`
    - › `ataque_id_forjado_nao_preenche`
    - › `ataque_agente_e_app_nas_operacoes_da_extensao`
    - › `recusas_antes_da_extensao`
    - › `travar_no_meio_mata_o_pedido`
    - › `cli_que_cai_mata_o_pedido`
    - › `um_pedido_por_vez`
    - › `v6_11_com_aprovacao_negar_nao_chega_a_extensao_e_permitir_chega`
    - › `v6_10_login_livre_preenche_na_origem_certa_e_volta_ok` (inclui "preenche uma vez só")
    - › `aguardar_nao_conta_como_uso`
  - `crates/wardd/src/login.rs` › `id_de_outro_pedido_nao_casa`.
  - `crates/ward-nm/src/testes.rs`: preenchimento como resposta de `aguardar_login`/`resultado_login` e segredo como resposta de `preencher_login`, todos recusados.
  - `make teste-login`:
    - V6.3: phishing 8082 → "domínio não confere", nada preenchido;
    - V6.15: `delta-redir` 8084→8082 → o mesmo;
    - `action` para fora, `formaction` para fora, GET, `target=_blank`, duas senhas e `action` mascarado por `<input name=action>`: nada preenchido, nada enviado, a aba fica;
    - `posts.log` sem mudança;
    - `acme-wp-dev` sem aprovação não preenche;
    - logs do daemon, Chrome, `ward-nm` e CLI e o console sem padrão de `.dev/variacoes.txt`;
    - log do daemon sem origens;
    - perfil sem valor.
- **Dependências novas**: nenhuma.
- **Decisão de segurança nova (o planner deve registrar como D-novo)**: o manifesto ganhou `host_permissions: ["https://*/*", "http://localhost/*", "http://127.0.0.1/*"]`, exatamente as origens que D9 aceita.
  - Motivo: sem clique do usuário não há `activeTab`, e `chrome.scripting.executeScript` falhou na prática ("Extension manifest must request permission to access the respective host").
  - O `montar.py` aceita só essa lista exata. `optional_host_permissions` continua proibido.
  - Efeito: a extensão pode injetar em qualquer página https sem o clique. É o mínimo para o `ward login` ser feito pelo agente, mas aumenta a superfície se o service worker for comprometido.

## Ficou de fora
- **V6.11 (UI)**: a janela de aprovação de verdade (app Tauri) não entra no e2e, porque o `wardd` de teste não tem app. Coberto assim:
  - no Rust, com um aprovador falso do mesmo canal (`Assinante`), negar não chega à extensão e permitir chega;
  - no e2e, sem aprovador, `acme-wp-dev` sai ≠ 0 e nada é preenchido.
  - Abrir a janela e clicar em negar fica para o QA do milestone.
- **Senha recusada no WordPress real**: a fixture não tem entrada WP com senha errada, e nada de segredo novo no repositório. A "tentativa com senha recusada pelo site" é uma página na origem certa (`localhost:8081/ward-teste/recusa.html`, servida pelo CDP) que recebe o POST com a senha certa e volta ao formulário. O teste confere: enviado uma vez à origem certa, campos vazios, CLI ≠ 0.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make app-front` | 0 |
| `make dev-servicos`, `make dev-cofre`, `make dev-variacoes` | 0 |
| `make extensao` | 0 |
| `make teste-login` | 0 (tudo certo) |
| `make teste-extensao` (regressão das Fases 41/42 com o `pagina.js`/`servico.js` novos) | 0 |
| `make teste-skill` (sem pendência de `login`) | 0 |
| `make fmt` | 0 |
| `make check` (1ª vez) | 2 (V3.15: `com_prazos_login` fora da lista de ganchos; corrigido) |
| `make check` (2ª vez) | 2 (`tests/extensao.rs`: tabela de operações da extensão sem as 3 novas; corrigido) |
| `make check` (3ª vez) | 2 (gitleaks não instalado no worktree; `make ferramentas`, sha256 conferido) |
| `make ferramentas` | 0 |
| `make check` (final: fmt, clippy, testes, audit, deny, gitleaks) | 0 ("tudo limpo") |

## Problemas descobertos
- **`host_permissions`**: decisão nova, com o motivo acima. Divergência com o comentário antigo do `montar.py` e com `CLAUDE.md` (já atualizados). O planner precisa registrar no escopo.
- **Gerenciador de senhas do Chrome**:
  - No e2e, depois do login no WordPress, o Chrome do perfil de teste preencheu sozinho o usuário em `wp-login.php` na aba seguinte. O teste agora desliga `credentials_enable_service` e o autofill no perfil temporário.
  - No Chrome do usuário, o gerenciador nativo pode oferecer guardar (ou guardar) a senha que o `ward login` enviou: cópia do segredo fora do cofre (S1/S2).
  - Sugestão para a política do Chrome da Fase 51: `PasswordManagerEnabled=false`, ou ao menos registrar o risco. Fora desta fase.
- **Redirecionamento 307**: a extensão não tem `webRequest` e não consegue barrar um 307 do próprio site certo para outra origem depois do envio. O POST sai para a origem certa, mas um servidor comprometido da origem certa já tem a senha de qualquer jeito.
- **SPA**: limpar os campos logo depois do `requestSubmit` pode quebrar logins que leem os campos de forma assíncrona após o submit. É o comportamento pedido pelo V6.10; anotar para o QA.
- **Ganchos de teste**: a lista do V3.15 (`crates/wardd/tests/aprovacao.rs`) ganhou `com_prazos_login`, que só encurta prazos e não toca a aprovação.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/wardd/src/login.rs` | criado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/trava.rs` | alterado |
| `crates/wardd/tests/login.rs` | criado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/wardd/tests/extensao.rs` | alterado (tabela de operações da extensão) |
| `crates/ward-nm/src/lib.rs` | alterado |
| `crates/ward-nm/src/testes.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `extensao/login.js` | criado |
| `extensao/pagina.js` | alterado |
| `extensao/servico.js` | alterado |
| `extensao/manifest.json` | alterado |
| `dev/extensao/montar.py` | alterado |
| `dev/testes/login.sh` | criado |
| `dev/testes/login.py` | criado |
| `dev/testes/skill.py` | alterado |
| `Makefile` | alterado (`teste-login`) |
| `CLAUDE.md` | alterado |
| `docs/mission/evidence/fase-43/qa.json`, `v6-10-painel-wordpress.png` | criado |
| `docs/mission/handoffs/phase-43.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica (sem tela nova)
- [x] Ficou dentro do escopo da fase
