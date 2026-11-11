# Fase 21 — `wardd`: `execution`, `ssh_agent`, `ssh`, `ssh_tests` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 3775e98`.
  Fase adiantada em paralelo com as Fases 14, 28 e 30 (pedido do orchestrator): só os
  nomes próprios destes quatro arquivos mudaram, com os usos dentro do `wardd`.
- **Arquivos** (glossário seção 4, `git mv`): `src/execucao.rs` → `execution.rs`,
  `src/agente_ssh.rs` → `ssh_agent.rs`, `src/ssh_testes.rs` → `ssh_tests.rs` (o
  `#[path = "ssh_testes.rs"] mod testes` do `ssh.rs` → `#[path = "ssh_tests.rs"] mod
  tests`); `lib.rs`: `pub mod execution`, `pub mod ssh_agent`. Os `mod testes` internos →
  `mod tests`.
- **Comentários, locais, parâmetros e nomes de teste** dos quatro arquivos em inglês.
  Comentários que citavam nome pt-BR de fio ou de outra fase (`nota segura`, `banco`,
  `servidor`, `saida`, `entrada_padrao`, `fim`, `janela`, `executar`, `emitir_token`,
  `tipo_sem_entrega`, "Conectar", "encerrado", "aprovação indisponível",
  `Daemon::autorizar_uso`, `ward_core::entrega::montar`, `crate::ponte`) foram reescritos
  com o sentido em inglês, sem o nome (como na Fase 13). O exemplo de fuso no comentário
  de `Shape::Timezone` passou de `America/Sao_Paulo` para `Europe/Lisbon` (o checador
  acusa "Sao"); os testes continuam com `America/Sao_Paulo`.
- **Renomes (antigo → novo)**, todos no glossário (parágrafo "Fase 21", depois do da
  Fase 29):
  - `execution`: `Temporarios { pasta, arquivos }` → `TempFiles { dir, files }` (`criar`/
    `gravar` → `create`/`write`), `Arquivo` → `SessionFile`, `pasta_temporarios` →
    `temp_dir`, `PASTA_TEMPORARIOS` → `TEMP_DIR`, `zerar` → `zero`, `limpar_temporarios` →
    `clean_temp_files`; `Preparo` → `Preparation` (campos → `command`, `mask`,
    `temp_files`, `bridge`, `pty`, `agent`, `lock`, `null_stdin`); `MorreNaTrava` →
    `DiesOnLock` (`sessoes`/`travas`/`inicial`/`travou` → `sessions`/`locks`/`initial`/
    `locked`); `AgentePreparado` → `PreparedAgent` (`filho_do_filho` → `child_of_child`);
    `RecursosRun` → `RunResources`, `ContextoTunel` → `TunnelContext`; `Forma` → `Shape`
    (`Fuso`/`Sinal` → `Timezone`/`Flag`); `ENTRAM_DA_CLI` → `FROM_CLI`, `PATH_DO_COMANDO` →
    `COMMAND_PATH`, `DO_DAEMON` → `FROM_DAEMON`, `MAX_VALOR_DA_LISTA` → `MAX_LIST_VALUE`,
    `LOTE` → `BATCH`, `FILA_ENTRADA` → `STDIN_QUEUE`, `PRAZO_RESTO_DA_SAIDA` →
    `REST_OF_OUTPUT_DEADLINE`, `PASSO_LEITURA_MS` → `READ_STEP_MS`, `SESSOES` →
    `SESSIONS`; `Conta { usuario, home }` → `Account { user, home }` (`do_passwd` →
    `from_passwd`); `ajustar_locale` → `adjust_locale`, `entra_da_cli` →
    `enters_from_cli`, `valor_aceito` → `value_accepted`, `e_utf8` → `is_utf8`,
    `montar_ambiente` → `build_environment`, `visivel` → `visible`, `ler_lote` →
    `read_batch`, `ler_saida` → `read_output`, `escrever_entrada` → `write_stdin`,
    `ler_da_cli` → `read_from_cli`, `por_na_fila` → `enqueue`, `aviso_do_fim` →
    `end_notice`, `Lancado` → `Launched`, `sessao`/`sessao_com` → `session`/
    `session_with`, `Daemon::preparar_execucao` → `prepare_run`, `Cli::enviar`/
    `enviar_saida` → `send`/`send_output`.
  - `ssh_agent`: `AgenteSsh { pasta, dir, escuta, chave }` → `SshAgent { path, dir,
    listener, key }` (`criar`/`publica`/`iniciar` → `create`/`public_key`/`start`; `socket`
    fica), `Rodando` → `Running`, `Permitido` → `Allowed` (`filho_do_filho` →
    `child_of_child`), `ErroAgente` → `AgentError` (`ChaveInvalida`/
    `AlgoritmoNaoSuportado`/`Sistema` → `InvalidKey`/`UnsupportedAlgorithm`/`System`;
    `Passphrase` fica), `Chave` → `Key` (`ler`/`publica_em_texto`/`assinar` → `read`/
    `public_as_text`/`sign`), `assinar_com` → `sign_with`, `NOME_SOCKET`/`NOME_PUBLICA`/
    `PREFIXO_PASTA` → `SOCKET_NAME`/`PUBLIC_NAME`/`DIR_PREFIX`, `MAX_MENSAGEM`/
    `MAX_CONEXOES`/`PASSO_MS`/`PRAZO_MENSAGEM` → `MAX_MESSAGE`/`MAX_CONNECTIONS`/`STEP_MS`/
    `MESSAGE_DEADLINE`, `apagar_pasta` → `delete_dir`, `limpar_pastas` → `clean_dirs`,
    `deve_parar` → `should_stop`, `par_permitido(_pid)` → `peer_allowed(_pid)`, `laco` →
    `serve_loop`, `ler_exato` → `read_exact`, `atender` → `serve`, `ler_string`/
    `por_string` → `read_string`/`put_string`, `responder_mensagem`/`responder` →
    `answer_message`/`answer`.
  - `ssh`: `ProgramasSsh` → `SshPrograms` (`do_sistema` → `from_system`),
    `TokenConectar { valor, entrada, ate }` → `ConnectToken { value, entry, until }`,
    `PRAZO_TOKEN` → `TOKEN_DEADLINE`, `PATH_FILHO` → `CHILD_PATH`, `Destino { host, porta,
    usuario }` → `Destination { host, port, username }`, `Autent { Senha, Chave { agente,
    publica } }` → `SshAuth { Password, Key { agent, public_key } }`, `Programa { Ssh {
    comando }, Sftp, Encaminhar { host, porta } }` → `Program { Ssh { command }, Sftp,
    Forward { host, port } }`, `Cred { Senha, Chave { chave, passphrase } }` → `Cred {
    Password, Key { key, passphrase } }`, `montar_argv(_com_alias)` →
    `build_argv(_with_alias)`, `alias_do_destino` → `destination_alias`, `usuario_ok` →
    `username_ok`, `host_simples` → `simple_host`, `texto` → `text`, `copiar` →
    `copy_secret`, `cred_de` → `cred_from`, `intermediario` → `intermediate`,
    `conferir_tipo` → `verify_kind`, `Daemon::emitir_token`/`usar_token`/`preparar_ssh` →
    `issue_token`/`use_token`/`prepare_ssh`, `sessao` → `session`.
  - Testes: `aviso_do_fim_so_diz_encerrado_sem_sobra` →
    `end_notice_only_says_ended_with_nothing_left`, `locale_utf8_fica_e_c_vira_c_utf8` →
    `utf8_locale_stays_and_c_becomes_c_utf8`, `ataque_so_a_lista_passa_da_cli` →
    `attack_only_the_list_passes_from_the_cli`, `ataque_valor_que_vira_caminho_sai` →
    `attack_value_that_becomes_a_path_stays_out`, `temporarios_0600_zerados_e_apagados` →
    `temp_files_0600_zeroed_and_deleted`, `partida_limpa_o_que_ficou` →
    `startup_cleans_what_was_left`, `ataque_rt_aberto_ou_link_recusado` →
    `attack_open_or_linked_rt_refused`, `f28g_partida_apaga_pastas_ssh_que_sobraram` →
    `f28g_startup_deletes_leftover_ssh_dirs`,
    `ataque_sftp_bang_outro_filho_do_sftp_e_recusado` →
    `attack_sftp_bang_other_child_of_sftp_is_refused`, `so_lista_e_assina` →
    `only_lists_and_signs`, `ataque_adicionar_remover_travar_e_lixo_recusados` →
    `attack_add_remove_lock_and_garbage_refused`, `d43_usuario_com_arroba_so_depois_de_l` →
    `d43_user_with_at_sign_only_after_l`, `chave_usa_o_agente_da_sessao` →
    `key_uses_the_session_agent`, `ataque_valores_fora_do_formato_recusados` →
    `attack_values_out_of_format_refused`, `fase28_argv_da_ponte_sem_proxyjump` →
    `phase28_bridge_argv_without_proxyjump`, `fase28_ataque_alvo_e_alias_fora_do_formato`
    → `phase28_attack_target_and_alias_out_of_format` (`d56_sftp` fica). Nenhum script,
    alvo do Makefile nem teste fora destes arquivos cita esses nomes (procurado com
    `git grep`).
- **Usos ajustados** (só os nomes desta fase): `wardd/src/lib.rs` (módulos e o link do
  doc), `main.rs` (`execution::clean_temp_files`, `ssh_agent::clean_dirs`), `nucleo.rs`
  (`ConnectToken`, `SshPrograms::from_system`, `issue_token`), `servidor.rs`
  (`execution::session`, `ssh::session`), `ponte.rs` (`SshAgent`, `Allowed`, `SshAuth`,
  `Cred`, `Destination`, `Program::Forward`, `build_argv`, `execution::BATCH`/
  `read_batch`), `tests/ssh.rs` (`SshPrograms`, `execution::session`, `ssh::session` e os
  dois comentários que citavam os caminhos).
- **Homônimos que não mudaram**: `limpar_temporarios` do `externo.rs` e
  `limpar_temporarios_estado` do `arquivos.rs`; `sessao` de `trava.rs`, `udisks.rs`,
  `cdp.rs`, `playwright.rs` e `tests/ssh.rs` (o ajudante local); `Daemon::com_programas_ssh`/
  `programas_ssh` (nucleo, Fase 20, e o gancho `test-hooks` que o V3.15 lista pelo nome);
  `PATH_FILHO` do `ponte.rs`; `PonteRodando`. Os nomes de fora (protocolo, entrega,
  máscara, núcleo, aprovação, ponte, log) ficaram como no master.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo, fora de comentários, comparado com `3775e98`:
- `Debug`: `"Temporarios({} arquivo(s))"` → `"TempFiles({} arquivo(s))"`,
  `"AgenteSsh(••••)"` → `"SshAgent(••••)"`, `"Rodando(agente SSH)"` → `"Running(agente
  SSH)"` (nome do tipo, como o `"PinHash(••••)"` da Fase 13; nenhum script nem teste
  procura esses textos; o V6.3 compara com `strings-0.1.4.txt`).
- `#[path = "ssh_testes.rs"]` → `"ssh_tests.rs"` (nome de arquivo).
- Capturas de `format!` (V6.3, valor igual no log): `"executar: {erro}"` (×2) →
  `{err}`, `"ssh: {erro}"` → `{err}`, `"executar: {tirados} variável(is) …"` →
  `{removed}`, `"agente SSH: mensagem {outro} recusada …"` → `{other}`,
  `"[{host}]:{porta}"` → `{port}`, `"[{host}]:{alvo}"`/`"{host}:{alvo}"` → `{target}`,
  `"{PREFIXO_PASTA}{sufixo}"` → `"{DIR_PREFIX}{suffix}"`; nos testes
  `"{PREFIXO_PASTA}abcdefghijklmnop"`/`"{PREFIXO_PASTA}link"` → `{DIR_PREFIX}`,
  `"wardd-exec-{}-{rotulo}"` → `{label}`, `"{nome} passou"`/`"{nome}={valor}"` →
  `{name}`/`{value}`, `"{um}"`/`"{dois}"` → `{one}`/`{two}`.
- **Ficaram**: todas as mensagens de log (`aviso!`/`depurar!`), os textos para a CLI
  ("o cofre travou; o comando foi encerrado"…), as mensagens do `AgentError` (`Display`),
  os `ErroRecurso("…")`, os nomes de thread (`wardd-run-saida`, `wardd-agente-ssh`…), as
  opções do `ssh` e todos os valores dos testes.
- Nos outros arquivos tocados nenhum literal mudou.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S3/S8/S13 e D13/D60 (ambiente do filho montado do zero): `build_environment`,
    `FROM_CLI`, `value_accepted`, `adjust_locale`, `FROM_DAEMON` com a mesma lógica e a
    mesma ordem (PATH fixo, HOME/USER/LOGNAME do passwd, lista da CLI conferida, entrega
    por cima, recusa se a entrega traz nome fixo).
  - S2 (temporários): `TempFiles::write` (`create_new`, 0600, `O_NOFOLLOW`), `zero` (mesmo
    inode, um link, regular) e `clean_temp_files` iguais.
  - D12 (agente SSH): `peer_allowed_pid` (preso ao primeiro filho do `sftp`), `serve_loop`
    (`SO_PEERCRED`, fecha sem byte), `answer_message` (só lista e assina), `Key::read`
    (só Ed25519) iguais.
  - S3/D43/D56 (argv sem segredo, sem shell): `build_argv_with_alias`, `username_ok`,
    `host_ok`, `simple_host`, `alias_ok`, `text` iguais; senha só em `SSHPASS`.
  - D13/R19 (`known_hosts` próprio, `-F /dev/null`): mesmas opções na mesma ordem.
  - D3 (token do "Conectar"): `issue_token`/`use_token` com a mesma conferência de papel,
    tempo constante (`tokens_equal`) e uso único.
  - S8/V2.7 (trava mata `ward run` e `ward ssh`): `DiesOnLock`, o registro antes do
    lançamento e `end_notice` iguais.
  - D19/Fase 28 (salto e túnel): `intermediate` e a ponte com a máscara das duas
    entradas iguais.
- **Conferência**: (1) multiconjunto de literais (acima); (2) sequência de tokens dos
  quatro arquivos com todo identificador trocado por `ID` e sem comentários, comparada
  com `3775e98`: as únicas diferenças são vírgulas finais do `rustfmt`, a ordem dos `use`
  e os campos escritos por extenso (`Resposta::Saida { fluxo: stream, dados: data }`,
  `Resposta::Fim { codigo: code }`) ou abreviados (`child::Pipes { stdin, stdout, stderr
  }`) por causa dos nomes novos dos locais.
- **Testes de ataque**: nenhum novo (só rename); os existentes (lista da CLI, valor que
  vira caminho, `<rt>` aberto ou link, link trocado pelo filho, `!cmd` do `sftp`,
  mensagens do agente proibidas, valores fora do formato no argv, alvo e alias da ponte)
  acompanham os nomes. Não rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes que estes arquivos usam e que são de outras fases: `ward_core::entrega`
  (Fase 15), `mascara` (16), `protocolo` (17–18), `wardd` `log`/`aviso!`/`depurar!` (19),
  `nucleo`/`aprovacao` (`Daemon::autorizar_uso`, `estado`, `travas_agora`,
  `sessoes_run`, `temporarios`, `programas_ssh`, `SessoesRun`, `Cliente`; Fase 20),
  `ponte` (`Ponte`, `Abertura`, `Intermediario { porta, usuario }`, `LIMITES_*`; Fase 22).
  O checador ainda acusa esses identificadores nos quatro arquivos (514 achados, todos
  de nomes de fora; nenhum comentário).
- Comentários pt-BR de `lib.rs`, `main.rs`, `nucleo.rs`, `ponte.rs`, `servidor.rs` e
  `tests/ssh.rs`: das fases deles (19, 20, 22, 23, 24).
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 3775e98` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo fmt --all` | 0 |
| `make compile` (log `.dev/logs/fase21-make-compile.log`; `cargo check` do workspace com todos os alvos e features, `npm ci`, build do Vite, `cargo check` do `app/src-tauri`, `syntax_check.py`) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (extra; log `fase21-clippy.log`) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/src/{execution,ssh_agent,ssh,ssh_tests}.rs` (log `fase21-english.log`) | 1 (514 achados, todos identificadores de outras fases; 0 de comentário) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `3775e98`; reset com a árvore limpa.
- **Checador não limpo** nos arquivos da fase: sobram só identificadores que vêm de
  outros módulos ainda não traduzidos (lista em "Ficou de fora"); some quando as Fases
  15–20 e 22 rodarem. Não pus nada em `allow.txt`/`frozen.txt` por isso (não são
  exceções permanentes nem nomes de fio).
- **Conflitos prováveis no merge** (fases em paralelo): `docs/glossario.md` (parágrafo
  "Fase 21" depois do da Fase 29, antes de "Funções e métodos…"); `crates/wardd/src/
  nucleo.rs` (três linhas com `crate::ssh::…` e a chamada `issue_token`), se a Fase 14
  mexer nas mesmas linhas.
- O comentário do `ssh.rs` "o V3.15 lê só os arquivos de primeiro nível de `src/`" ficou
  traduzido como estava, mas está velho: o teste `v3_15_sem_aprovador_no_binario` desce
  nas subpastas desde o scrutiny do M3. Não mexi no sentido (fase só de nomes).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/execution.rs` (era `execucao.rs`) | renomeado e reescrito |
| `crates/wardd/src/ssh_agent.rs` (era `agente_ssh.rs`) | renomeado e reescrito |
| `crates/wardd/src/ssh.rs` | reescrito |
| `crates/wardd/src/ssh_tests.rs` (era `ssh_testes.rs`) | renomeado e reescrito |
| `crates/wardd/src/{lib,main,nucleo,ponte,servidor}.rs` | alterado (usos) |
| `crates/wardd/tests/ssh.rs` | alterado (usos) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-21.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -p wardd -D warnings` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
