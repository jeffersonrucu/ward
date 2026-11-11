# Fase 22 — `wardd`: `playwright`, `cdp`, `login`, `bridge` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard c5cd3d1`
  (master atual).
- **Arquivos**: `src/ponte.rs` → `src/bridge.rs` (`git mv`; `lib.rs`: `pub mod bridge`).
  `cdp.rs`, `login.rs` e `playwright.rs` ficam com o nome (glossário seção 4). Os quatro
  traduzidos por inteiro (comentários, locais, parâmetros, campos, nomes de teste,
  `mod testes` → `mod tests`).
- **Renomes** (antigo → novo), com os usos no `wardd`:
  - `cdp`: `MAX_MENSAGEM` → `MAX_MESSAGE`, `PEDACO` → `CHUNK`; `ErroCdp` → `CdpError`
    (`Prazo`/`Fechado`/`Protocolo`/`Comando`/`Io` → `Deadline`/`Closed`/`Protocol`/
    `Command`/`Io`); `Escritor` → `Writer` (`virgula`/`estourou` → `comma`/`overflowed`;
    `novo`/`por`/`separar`/`abre_objeto`/`fecha_objeto`/`abre_lista`/`fecha_lista`/
    `chave`/`texto`/`texto_cru`/`inteiro`/`logico`/`nulo`/`fim` → `new`/`put`/`separate`/
    `open_object`/`close_object`/`open_list`/`close_list`/`key`/`text`/`raw_text`/
    `integer`/`boolean`/`null`/`finish`); `pior_caso` → `worst_case`; `ComResultado`/
    `ComParametros`/`Pausa`/`PedidoPausado` → `WithResult`/`WithParams`/`Pause`/
    `PausedRequest` (campos `pedido`/`tipo`/`quadro` → `request_id`/`resource_type`/
    `frame_id`); `Envelope.sessao` → `session`; `Regra { origem, quadro }` → `Rule {
    origin, frame }`; `Cdp { escrita, leitura, proximo, prazo, sessao, regra, bloqueou,
    cargas }` → `Cdp { write_end, read_end, next_id, deadline, session, rule, blocked,
    loads }`; `novo`/`enviar`/`comando`/`comando_de`/`esperar_carga`/`tratar`/`decidir`/
    `proxima`/`proxima_ate` → `new`/`send`/`command`/`command_as`/`wait_load`/`handle`/
    `decide`/`next_message`/`next_message_until`.
  - `playwright`: `PASTA_SESSOES`/`PREFIXO_PERFIL`/`PRAZO_SESSAO`/`PRAZO_CARGA`/
    `PATH_FILHO`/`PREFERENCIAS`/`PREENCHER`/`LER_PAGINA` → `SESSIONS_DIR`/
    `PROFILE_PREFIX`/`SESSION_DEADLINE`/`LOAD_DEADLINE`/`CHILD_PATH`/`PREFERENCES`/
    `FILL_SCRIPT`/`READ_PAGE_SCRIPT` (`CHROME` fica); `pasta_sessoes`/`apagar_sessoes`/
    `limpar`/`argv_chrome`/`abrir_navegador`/`erro_cdp`/`origem_confere`/
    `no_mundo_isolado`/`entrar`/`montar_estado`/`tem_segredo`/`gravar_estado` →
    `sessions_dir`/`delete_sessions`/`clean_up`/`chrome_argv`/`open_browser`/`cdp_error`/
    `origin_matches`/`in_isolated_world`/`log_in`/`build_state`/`has_secret`/
    `write_state`; tipos `Navegador { grupo, perfil }` → `Browser { group, profile }`,
    `Credencial { usuario, senha, codigo }` → `Credential { username, password, code }`,
    `Contexto`/`Alvo`/`Anexo`/`Arvore`/`NoQuadro`/`Quadro`/`Navegacao`/`Mundo`/
    `Avaliacao`/`ValorRemoto`/`Pagina`/`EstadoArmazenado`/`OrigemPw` → `Context`/`Target`/
    `Attached`/`FrameTree`/`FrameNode`/`Frame`/`Navigation`/`World`/`Evaluation`/
    `RemoteValue`/`PageState`/`StorageState`/`OriginPw` (`CookieCdp`, `CookiePw`,
    `ItemPw`, `Cookies` ficam).
  - `login`: `PrazosLogin { aguardar, retirada, total }` → `LoginDeadlines { wait,
    pickup, total }`; `PASSO` → `STEP`; `Fase { Esperando, Entregue, Preenchido }` →
    `Phase { Waiting, Delivered, Filled }`; `Trabalho { entrada, fase, resultado }` →
    `Job { entry, phase, result }`; `Logins { atual, sinal, prazos }` → `{ current,
    signal, deadlines }` (`novo`/`trava`/`do_id`/`terminar` → `new`/`lock`/`by_id`/
    `finish`); `Apagar` → `DeleteOnDrop`; `Daemon::esperar_desfecho`/`aguardar_login`/
    `preencher_login`/`resultado_login` → `wait_outcome`/`await_login`/`fill_login`/
    `login_result` (`Daemon::login` fica).
  - `bridge`: `Ponte` → `Bridge` (`ouvinte`/`porta`/`plano`/`mascara`/`limites`/`travas`/
    `inicial` → `listener`/`port`/`plan`/`mask`/`limits`/`locks`/`initial`; `abrir`/
    `porta`/`iniciar` → `open`/`port`/`start`); `PonteRodando { parar, grupos }` →
    `RunningBridge { stop, groups }`; `Abertura` → `Opening` (`alvo_host`/`alvo_porta`/
    `programas`/`mascara`/`limites`/`travas` → `target_host`/`target_port`/`programs`/
    `mask`/`limits`/`locks`); `Intermediario { porta, usuario }` → `Intermediate { port,
    username }`; `Plano` → `Plan`; `Limites { simultaneas }` → `Limits { concurrent }`;
    `LIMITES_SALTO`/`LIMITES_TUNEL` → `JUMP_LIMITS`/`TUNNEL_LIMITS`; `SaidaDeErro` →
    `Stderr` (glossário); `PASSO_MS`/`PEDACO`/`PATH_FILHO` → `STEP_MS`/`CHUNK`/
    `CHILD_PATH`; `argv_da_ponte`/`escutar`/`conectar`/`copiar`/`ler_erro` →
    `bridge_argv`/`listen`/`connect`/`copy`/`read_stderr`; `FimDeEscrita::fechar_escrita`
    → `WriteShutdown::close_write`; `par_do_grupo`/`socket_do_par`/`endereco`/
    `processo_do_grupo_com_socket`/`filho_direto_com_sessao_propria`/`pai_do_stat`/
    `grupo_do_stat`/`sessao_do_stat`/`campo_do_stat` → `peer_in_group`/`peer_socket`/
    `address`/`group_process_has_socket`/`direct_child_with_own_session`/`stat_parent`/
    `stat_group`/`stat_session`/`stat_field`.
  - Nomes de teste: `escritor_escapa_e_nao_realoca` → `writer_escapes_and_does_not_reallocate`,
    `pausa_le_so_url_tipo_e_quadro` → `pause_reads_only_url_type_and_frame`,
    `argv_so_fixo_com_pipe_e_sem_porta` → `argv_only_fixed_with_pipe_and_no_port`,
    `ataque_apagar_sessoes_nao_segue_link` → `attack_delete_sessions_does_not_follow_link`,
    `estado_no_formato_do_playwright` → `state_in_playwright_format`,
    `ataque_dom_clobbering_scripts_so_pelos_prototipos` →
    `attack_dom_clobbering_scripts_only_through_prototypes`,
    `ataque_nome_fora_da_pasta_recusado` → `attack_name_outside_dir_refused`,
    `id_de_outro_pedido_nao_casa` → `id_of_another_request_does_not_match`,
    `endereco_do_proc_net_tcp` → `proc_net_tcp_address`,
    `ataque_socket_de_outro_uid_ou_ambiguo` → `attack_socket_of_another_uid_or_ambiguous`,
    `grupo_do_stat_com_parenteses_no_nome` → `stat_group_with_parentheses_in_name`,
    `ataque_processo_no_grupo_de_outra_sessao_e_recusado` →
    `attack_process_in_group_of_another_session_is_refused`,
    `f28g_filho_direto_do_lider_com_sessao_propria` →
    `f28g_direct_child_of_leader_with_own_session`, `par_do_grupo_de_verdade` →
    `peer_in_group_for_real`. Nenhum script nem contrato cita esses nomes (procurado em
    `dev/`, `Makefile`, `install/`, `crates/`).
- **Usos ajustados** (só os nomes desta fase): `wardd/src/lib.rs` (`pub mod bridge`),
  `main.rs` (`playwright::clean_up`), `nucleo.rs` (`playwright::delete_sessions`,
  `login::LoginDeadlines`/`Logins::new` no `com_prazos_login`, despacho para
  `await_login`/`fill_login`/`login_result`), `ssh.rs` (import, `Intermediate { host,
  port, username, cred }`, `Bridge::open(Opening { … })`, `p.port()`, link de doc),
  `execucao.rs` (import, `Option<Bridge>` nos dois campos `ponte`, `Bridge::open(Opening
  { … })`, `ponte.port()`, `p.start(…)`, link de doc), `tests/login.rs`
  (`LoginDeadlines { wait, pickup, total }`), `tests/playwright.rs` (`clean_up`, 2 vezes).
  Os locais e campos desses arquivos (ex.: o campo `ponte` do `Preparo`, `mascara_ponte`,
  `com_prazos_login`) são das Fases 19–21/27 e ficaram.
- **Comentários** que citavam nomes de fio pt-BR (`aguardar_login`, `preencher_login`,
  `resultado_login`, `pedido_de_login`, `origem_diferente`, `ocupado`,
  `navegador_falhou`, os códigos `"origem"`/`"formulario"` do script, `banco`/`servidor`,
  `Daemon::autorizar_uso`, `Estado::travar`, `entrar_grupo.py`, o caso `dom-recusa`)
  foram reescritos sem o nome, apontando para os métodos novos quando havia
  (`Daemon::await_login`, `fill_login`, `login_result`).
- **Glossário**: parágrafo "Fase 22" depois do da Fase 28 (seção 15). Duas escolhas fora
  da tabela de termos, registradas lá: `aguardar` → `wait` no campo de `LoginDeadlines`
  (`await` é palavra reservada) e `Trabalho` → `Job` (pedido de login pendente).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Conferido por multiconjunto de literais por arquivo (HEAD × worktree,
`scratchpad/f22/shape.py`):
- `Debug` manual (como nas Fases 8/9): `"Cdp({} cargas, ••••)"` → `"Cdp({} loads, ••••)"`,
  `"Ponte(127.0.0.1:{}, ••••)"` → `"Bridge(127.0.0.1:{}, ••••)"`, `"PonteRodando"` →
  `"RunningBridge"`. Nenhum script nem teste procura esses textos (só
  `strings-0.1.4.txt`, que o V6.3 deve aceitar).
- Capturas de `format!` (V6.3): `{nome}` → `{name}` nos três `aviso!` do `login`
  (texto do log igual), `{PREFIXO_PERFIL}{sufixo}` → `{PROFILE_PREFIX}{suffix}`,
  `.{entrada}.{sufixo}.tmp` → `.{entry}.{suffix}.tmp`, `{entrada}.json` → `{entry}.json`
  (nomes no disco iguais); nos testes `{remoto}`/`{cab}`/`{grupo}`/`{sessao}`/`{pai}`/
  `{nome}`/`{proibido}`/`{ruim}` → `{remote}`/`{head}`/`{group}`/`{session}`/`{parent}`/
  `{name}`/`{forbidden}`/`{bad}`.
- Testes: pastas temporárias `"ward-pw-teste-{}"` → `"ward-pw-test-{}"`,
  `"ponte-proc-{}"`/`"ponte-proc-28g-{}"` → `"bridge-proc-{}"`/`"bridge-proc-28g-{}"`; nomes
  de arquivo do teste de link `"alvo"`, `"precioso.txt"`, `"sub/outro.txt"`, `"ligacao"` →
  `"target"`, `"precious.txt"`, `"sub/other.txt"`, `"link"`; rótulos `"PREENCHER"`/
  `"LER_PAGINA"` → `"FILL_SCRIPT"`/`"READ_PAGE_SCRIPT"` (nomes das constantes).
- Novo: `#[serde(rename = "senha")]` em `PageState.password` (a chave do JSON que o
  `READ_PAGE_SCRIPT` devolve, que não mudou).
- **Ficaram**: o JavaScript de `FILL_SCRIPT`/`READ_PAGE_SCRIPT` byte a byte (conferido:
  os dois literais do HEAD estão inteiros no arquivo novo; os identificadores pt-BR de
  dentro são literal e mudar o script é mudar lógica de segurança), todas as mensagens
  de `aviso!`/`depurar!` e de erro (`"salto ou túnel: …"`, `"ponte: …"`, `"playwright: …"`),
  os nomes das threads `wardd-ponte*`, todos os nomes de método e campo do CDP
  (`Fetch.requestPaused`, `sessionId`, `requestId`, `frameTree`, `securityOrigin`…), o
  argv do Chrome, as preferências do perfil e os textos de `assert!`.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários). Forma do código
  conferida com os identificadores trocados por um marcador e sem comentários: só
  vírgulas finais do `cargo fmt`, campos abreviados (`origin: origem` → `origin`) e o
  inverso nos padrões (`Pedido::Login { entrada: entry }`,
  `Resposta::SessaoPlaywright { caminho: path }`), e o `serde(rename)` acima.
  - D9 (origem): `Cdp::decide` (documento do quadro principal só da origem da entrada,
    `Fetch.failRequest` com `BlockedByClient`), `origin_matches` (URL **e**
    `securityOrigin`), as três conferências do `log_in` (antes, mesmo quadro, depois do
    envio) e o `blocked` → `OrigemDiferente`: mesma ordem e mesmas comparações.
  - Formulário, `action`/`formaction`/`target`/método, DOM clobbering: o script do mundo
    isolado é o mesmo literal; o teste que proíbe `document.`, `form.`, `botao.`… no
    script continua com a mesma lista.
  - S3/D6 (pipe sem porta TCP): `chrome_argv` com as mesmas opções na mesma ordem
    (`--remote-debugging-pipe`, sem `remote-debugging-port`, `--host-resolver-rules`),
    ambiente montado do zero igual, pontas `CLOEXEC` pelo `launch_with_debugging`.
  - S1/S2 (storage state): `has_secret` pela máscara antes de gravar; `write_state` com
    o mesmo filtro de nome, `create_new` 0600 + `O_NOFOLLOW` + `sync_all` + `rename`;
    gravação só com o cofre aberto e a contagem de travas igual; `delete_sessions` com
    `O_DIRECTORY | O_NOFOLLOW` e apagando pelo fd; `clean_up` na partida.
  - Login pela extensão (D9, S1): papel `Extensao` conferido nas três operações, id
    comparado por `tokens_equal` (tempo constante), um preenchimento por id, prazos e
    fim do pedido ao travar/CLI cair iguais.
  - Ponte (S1, S3, S5): `peer_in_group` (uid, grupo **e** sessão, filho direto com
    sessão própria), conexão de fora fechada sem byte, limites por sessão, `SSHPASS` só
    no ambiente, parada quando a contagem de travas muda: mesma lógica.
  - S8/S13: buffers `Zeroizing` com a mesma capacidade, `Debug` sem valor (`••••`),
    nenhum texto do navegador nos erros.
- **Testes de ataque**: nenhum novo (só rename); os existentes (link no lugar da pasta
  das sessões, nome fora da pasta, DOM clobbering, escritor que estoura sem realocar,
  socket de outro uid/ambíguo, processo no grupo de outra sessão, filho direto do líder,
  id de outro pedido) acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Identificadores de outras fases que aparecem nestes arquivos (checador: 470 achados,
  todos usos): `ErroResposta`/`Resposta`/`Pedido`/`Papel`/`ResultadoLogin` e variantes
  (Fase 17), `Mascara` e métodos (Fase 16), `aprovacao::Cliente`, `nucleo`,
  `autorizar_uso`/`registrar_uso`/`estado`/`travas_agora`/`preencher`/`temporarios`/
  `travado` (Fases 19–20), `ssh::{Autent, Cred, Destino, Programa, ProgramasSsh,
  montar_argv}`, `agente_ssh::{AgenteSsh, Permitido}`, `execucao::{LOTE, ler_lote}`
  (Fase 21), macros `aviso!`/`depurar!` (Fase 19).
- O JavaScript dentro de `FILL_SCRIPT`/`READ_PAGE_SCRIPT` (literal; ver acima).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo fmt -p wardd` | 0 |
| `make compile` (log `scratchpad/f22/compile-fase22.log`; 0 warning) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (extra, log `clippy-fase22.log`) | 0 |
| `cargo fmt --all --check` | 0 |
| `make english-check PATHS="crates/wardd/src/cdp.rs crates/wardd/src/login.rs crates/wardd/src/playwright.rs crates/wardd/src/bridge.rs"` (log `english-check-fase22.log`) | 2 (470 achados, todos identificadores de outras fases; 0 comentário, 0 arquivo, 0 problema nas exceções) |

`make check` não rodou: pelo plano (PRD regra 6), as fases dos Milestones 1–5 terminam
com `make compile` e o checador; a bateria fica para o Milestone 6.

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `c5cd3d1`; reset com a árvore limpa.
- **Arquivos de fases paralelas tocados** (só usos, conflito de merge esperado e
  pequeno): `crates/wardd/src/ssh.rs` e `execucao.rs` (Fase 21: import, literais de
  `Opening`/`Intermediate`, `Option<Bridge>`, `.port()`, `.start(`, dois links de doc),
  `nucleo.rs` (Fase 20: 5 linhas), `main.rs` e `lib.rs` (Fase 19: 1 linha cada),
  `tests/login.rs` e `tests/playwright.rs` (Fase 27). Se a Fase 21 renomear o campo
  `ponte` do `Preparo`/`RecursosRun` ou a variável `mascara_ponte`, o merge leva os
  tipos novos (`Bridge`, `Opening { target_host, target_port, programs, mask, limits,
  locks }`).
- `Stderr` (glossário, para `SaidaDeErro`) tem o mesmo nome de `std::io::Stderr`; não
  colide hoje (`bridge.rs` não importa o do `std`), mas quem importar os dois precisa de
  caminho qualificado.
- Os links de doc para `Daemon::await_login`/`fill_login`/`login_result` (métodos
  `pub(crate)`) em comentários de itens `pub` podem dar `rustdoc::private_intra_doc_links`
  num `cargo doc`; não afeta `check`/`clippy`.
- `docs/mission/plan.md` ainda cita `ponte.rs` na linha da Fase 22 (o plano é do
  orchestrator).
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 28; as
  Fases 15/21/31 devem pôr os delas no mesmo ponto, conflito de merge trivial).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/ponte.rs` → `crates/wardd/src/bridge.rs` | renomeado e alterado |
| `crates/wardd/src/cdp.rs` | alterado |
| `crates/wardd/src/login.rs` | alterado |
| `crates/wardd/src/playwright.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/tests/login.rs` | alterado |
| `crates/wardd/tests/playwright.rs` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-22.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não, pela regra 6
  do PRD; rodaram `make compile`, `cargo clippy -p wardd -D warnings` e `fmt --check`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes, renomeados; não rodaram
  (regra 6)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase

## Merge com o master `efa6865` (Fases 15 e 21)
- `execucao.rs`/`ponte.rs` apagados; os usos da ponte reaplicados no `execution.rs`
  (`Bridge`, `Opening { target_host, target_port, programs, mask, limits, locks }`,
  `TUNNEL_LIMITS`, `.port()`, `.start(`) e no `ssh.rs` da Fase 21 (`Intermediate { host,
  port, username, cred }`, `Bridge::open`, `JUMP_LIMITS`, `p.port()`). Os campos
  `ponte`/`mascara_ponte` já tinham virado `bridge`/`bridge_mask` na Fase 21.
- `bridge.rs` passa a usar os nomes da Fase 21: `ssh::{Cred::{Password, Key { key }},
  Destination, Program::Forward, SshAuth::{Password, Key { agent, public_key }},
  SshPrograms, build_argv}`, `ssh_agent::{SshAgent::create, Allowed { child_of_child },
  start, public_key}`, `execution::{BATCH, read_batch}`.
- Glossário com os parágrafos das Fases 22, 21 e 15.
- `make compile` depois do merge: 0 (log `scratchpad/f22/compile-merge-fase22.log`).
