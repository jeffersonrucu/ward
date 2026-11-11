# Fase 24 — `wardd`: testes `ssh.rs`, `run.rs`, `ssh_falso.py` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard e70805b`
  (master atual). Fase adiantada em paralelo com as Fases 15, 22 e 31: só os nomes
  próprios destes três testes mudaram; os que vêm do `comum` (Fase 23), do
  `nucleo`/`aprovacao` (Fase 20) e do `ward-core` (`protocolo`, Fases 17–18) ficaram como
  no master.
- **Arquivo** (glossário seção 4, `git mv`): `crates/wardd/tests/ssh_falso.py` →
  `fake_ssh.py`; o único uso (`tests/ssh.rs`, caminho do script) acompanha. `ssh.rs` e
  `run.rs` mantêm o nome (glossário: "ficam").
- **Comentários, locais, parâmetros e nomes de teste** dos três arquivos em inglês.
  Comentários que citavam nome pt-BR de fio, de entrada ou de outra fase (`aprovacao_indisponivel`,
  `quadro_invalido`, `destravar`, `entrada_padrao`, `executar`, `t-livre`/`t-aprov`,
  `wardd-aprovador-teste`, "Permitir uma vez", "encerrado", `comum::servidor`,
  `Daemon::com_programas_ssh`, `acme-ssh-interno`, `tempo.py`, os arquivos `modo`/
  `registro-…` e as chaves `agente.x`/`eco:`) foram reescritos com o sentido em inglês,
  sem o nome (como nas Fases 13 e 21).
- O parágrafo do V4.15 que estava colado no doc do teste da Fase 26a (os dois viravam
  doc do `f26a…`) foi para o teste `v4_15_…`, que estava sem doc. Só comentário.
- **Renomes (antigo → novo)**, todos no glossário (parágrafo "Fase 24", depois do da
  Fase 21):
  - `ssh.rs`: `SENHA_SSH`/`SENHA_BANCO` → `SSH_PASSWORD`/`DB_PASSWORD`; `RelogioManual` →
    `ManualClock` (`avancar` → `advance`); `Rt::novo` → `Rt::new`; `Cena { d, dir, rt, base,
    relogio }` → `Scene { …, clock }` (`cena` → `scene`; `modo`/`registros`/`registro`/
    `nada_executado` → `set_mode`/`records`/`record`/`nothing_ran`); `Reg` → `Rec`
    (`todos`/`um` → `all`/`one`); `Sessao { saida, erro, pedacos, fim, falha }` → `Session
    { stdout, stderr, chunks, end, failure }` (`texto` → `text`); `salvar` → `save`,
    `servidor_json` → `server_json`, `auth_senha` → `password_auth`, `gerar_chave` →
    `generate_key`, `pedido_ssh` → `ssh_request`, `mandar` → `send`, `sessao`/`sessao_de`
    → `session`/`session_of`, `fechar_entrada` → `close_stdin`, `opcoes` → `options`,
    `esperar` → `wait_for`, `rt_sem_agente` → `rt_without_agent`, `esperar_registro` →
    `wait_record`, `depois_de` → `after`, `estranho_e_recusado` → `stranger_is_refused`,
    `inode_da_escuta`/`escuta_fechada` → `listener_inode`/`listener_closed`,
    `ninguem_com_w` → `nobody_with_w`, `conferir_argv_da_ponte` → `verify_bridge_argv`.
  - `run.rs`: `SENHA_LIVRE`/`LICENCA`/`ANEXO`/`APROVADOR` → `FREE_PASSWORD`/`LICENSE`/
    `ATTACHMENT`/`APPROVER`; `DirSocket::novo` → `SocketDir::new`; `Wardd { filho }` →
    `Wardd { child }`; `subir`/`subir_com` → `start`/`start_with`, `mandar` → `send`,
    `receber` → `receive`, `conectar` → `connect`, `destravar_por` → `unlock_through`,
    `pedido_executar(_com)` → `run_request(_with)`, `entrada_padrao` → `send_stdin`,
    `Sessao { saida, erro, fim, falha }` → `Session { stdout, stderr, end, failure }`,
    `ler_sessao` → `read_session`, `cofre` → `vault`, `texto` → `text`, `tmp_vazio` →
    `tmp_empty`, `esperar` → `wait_for`, `vivo` → `alive` (`run`, `sh`, `sha256` ficam).
  - `fake_ssh.py`: `NOME` → `NAME`, `ler` → `read_file`, `registrar` → `record`, `opcao`
    → `option`, `pedir` → `ask`, `cliente_do_agente` → `agent_client`, `NETO` →
    `GRANDCHILD`, `esperar_continuar`/`esperar_arquivo` → `wait_continue`/`wait_file`,
    `valor` → `value`, `ponte_do_lado_do_salto` → `bridge_on_the_jump_side`, `conversar`
    → `talk`, `principal` → `main` (`string`, `bang` ficam).
  - Testes: `v4_35_d43_usuario_com_arroba_e_senha_so_no_ambiente` →
    `v4_35_d43_user_with_at_sign_and_password_only_in_env`,
    `v4_37_d56_sftp_com_user_e_host_depois_de_traco_traco` →
    `v4_37_d56_sftp_with_user_and_host_after_double_dash`, `recusas_sem_executar_nada` →
    `refusals_without_running_anything`, `v4_24_d12_agente_so_do_filho` →
    `v4_24_d12_agent_only_for_the_child`, `d12_travar_derruba_o_agente` →
    `d12_lock_brings_down_the_agent`, `f54c_ataque_ward_ssh_morre_na_trava` →
    `f54c_attack_ward_ssh_dies_on_lock`, `f54l_ataque_cli_cai_no_ward_ssh_nenhum_ssh_sobra`
    → `f54l_attack_cli_drops_in_ward_ssh_no_ssh_left`,
    `d12_sftp_com_chave_atende_o_ssh_do_sftp` → `d12_sftp_with_key_serves_the_ssh_of_sftp`,
    `d34_ataque_pty_prefixo_certo_e_errado_saem_iguais_e_na_hora` →
    `d34_attack_pty_right_and_wrong_prefix_come_out_alike_and_at_once`,
    `d34_ataque_tempo_sem_pty_prefixo_nao_atrasa` →
    `d34_attack_timing_without_pty_prefix_not_delayed`,
    `ataque_known_hosts_frouxo_recusa_sem_executar` →
    `attack_loose_known_hosts_refused_without_running`,
    `ataque_sftp_bang_nao_usa_o_agente` → `attack_sftp_bang_does_not_use_the_agent`,
    `v5_24_token_do_conectar` → `v5_24_connect_token`,
    `fase28_salto_com_senha_pela_ponte` → `phase28_jump_with_password_through_the_bridge`,
    `fase28_salto_com_chave_agente_de_cada_ssh` → `phase28_jump_with_key_agent_per_ssh`,
    `fase28_tunel_do_banco_no_run` → `phase28_database_tunnel_in_run`,
    `fase28_travar_fecha_o_tunel_para_conexoes_novas` →
    `phase28_lock_closes_the_tunnel_to_new_connections`;
    `v4_entrega_mascara_codigo_e_recusas` → `v4_delivery_mask_code_and_refusals`,
    `f26a_ataque_ambiente_da_cli_nao_muda_o_comando` →
    `f26a_attack_cli_environment_does_not_change_the_command`,
    `v4_15_cli_que_cai_e_daemon_morto` → `v4_15_cli_that_drops_and_killed_daemon`,
    `ra1_registro_environ_do_filho_legivel_pelo_mesmo_uid` →
    `ra1_record_child_environ_readable_by_same_uid`,
    `d52_pedido_grande_demais_recusado_pelo_daemon` →
    `d52_oversized_request_refused_by_daemon`, `v4_21_travado_espera_o_destravar_da_cli`
    → `v4_21_locked_waits_for_cli_unlock`,
    `f28g_ataque_write_unico_grande_nao_mostra_o_comeco_da_senha` →
    `f28g_attack_single_large_write_does_not_show_password_start`,
    `f54_ataque_ward_run_morre_na_trava` → `f54_attack_ward_run_dies_on_lock`,
    `f55_ataque_descendente_fora_do_grupo_morre_no_fim_e_na_trava` →
    `f55_attack_descendant_outside_group_dies_at_end_and_on_lock`,
    `f54b_ataque_entrada_nula_com_dado_mata_o_comando` →
    `f54b_attack_null_stdin_with_data_kills_the_command`,
    `f54b_ataque_script_por_pipe_na_aprovacao` → `f54b_attack_piped_script_on_approval`.
    Nenhum script, alvo do Makefile nem outro teste cita esses nomes (procurado com
    `grep -r` fora de `docs/mission/arquivo/`).
- **Usos fora dos três arquivos**: `docs/mission/plan.md` (Fase 52 cita `fake_ssh.py`).
  O `chrome_falso.py` da mesma linha fica (Fase 27).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo, fora de comentários, comparado com `e70805b`
(extrator próprio no scratchpad; Python pelo `tokenize`):
- `"tests/ssh_falso.py"` → `"tests/fake_ssh.py"` (caminho do arquivo renomeado).
- Só capturas de `format!` (valor igual): em `ssh.rs`, `{rotulo}` → `{label}`, `{nome}`
  → `{name}`, `{entrada}` → `{entry}`, `{porta}` → `{port}`, `{marca}` → `{mark}`,
  `{campo}` → `{field}`, `{esperado}` → `{expected}`, `{modo:o}` → `{mode:o}`,
  `{papel:?}` → `{role:?}`, `{outra:?}` → `{other:?}`, `{erro}` → `{stderr}`,
  `{publica}` → `{public_key}`, `{principal:?}`/`{salto:?}` → `{main:?}`/`{jump:?}`,
  `{depois:?}` → `{after:?}`, `{linha_do_tempo:?}` → `{timeline:?}`, `<{errado}` →
  `<{wrong}`, `{certo}••••]` → `{right}••••]`; no JSON das entradas, `{usuario}`/
  `{protocolo}`/`{campos_auth}` → `{user}`/`{protocol}`/`{auth_fields}`; nos scripts
  Python dos testes de túnel, os argumentos nomeados `{arq:?}`/`{marca:?}`/`{servidor}`
  → `{file:?}`/`{mark:?}`/`{server}`. Em `run.rs`, `{rotulo}` → `{label}`, `{entrada}`/
  `{tipo}` → `{entry}`/`{kind}` no JSON do `executar`, `{esperado}` → `{expected}`,
  `{proibido}` → `{forbidden}`, `{segredo}` → `{secret}`, `{linhas}` → `{lines}`,
  `{outra:?}` → `{other:?}`.
- `fake_ssh.py`: só as docstrings (comentários para o checador); nenhuma string de
  código mudou (os arquivos `registro-<nome>-<pid>.txt`, `modo`, `esperar`,
  `continuar`, `pty-pronto`, `esperar-ponte`, as chaves `agente.*`, `neto`, `depois`,
  `bang_*` e as saídas `eco:`, `ponte=`, `erro-salto`, `conectado`… ficam).
- **Ficaram**: todas as mensagens que os testes procuram (log do daemon, avisos da CLI,
  "o cofre travou…", `resposta=pedido_invalido`…), as mensagens de `assert!`, os nomes de
  entrada (`s-livre`, `t-aprov`…), os nomes de arquivos e pastas de teste (`falsos`,
  `marca-*`, `atacante`, `chave_teste`…) e os JSON do protocolo (fio congelado).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários): os testes de
  ataque de S1–S4, S8, S13, D3 (token do "Conectar"), D12 (agente só do filho, `!cmd`
  do `sftp`), D13/D60 (ambiente do zero, `known_hosts` próprio, Fase 26a), D19 (ponte e
  túnel), D34 (oráculo de prefixo e de tempo), D43/D56 (argv), D52, D68 (entrada nula e
  caixa da aprovação), RA1, Fases 28g/54/54c/54l/55 (trava e CLI que cai). Mesma
  lógica, mesmas asserções, mesmos valores e prazos.
- **Conferência**: multiconjunto de literais (acima) e leitura lado a lado; o
  `make compile` passa (os tipos e a ordem dos campos dos pedidos não mudaram).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes que estes testes usam e que são de outras fases: `comum` e o que ele exporta
  (`Dir::novo`, `cofre_de_teste`, `pedir`, `mudar_nivel`, `destravar`, `contem`, `erro`,
  `SENHA`, `SEGREDOS`, `PIN`; Fase 23), `nucleo::Daemon` e os construtores
  `com_motor`/`com_programas_ssh`/`com_temporarios`/`atender`/`travar` (Fase 20),
  `aprovacao` (`Motor`, `RelogioMotor::agora`, `SemApp`, `SemCliente`; Fase 20) e o
  `ward_core::protocolo` (`Pedido`, `Resposta`, `ErroResposta`, `Papel`, `Janela`,
  `quadro`…; Fases 17–18). O checador ainda acusa esses identificadores (360 achados em
  `ssh.rs`/`run.rs`, todos de nomes de fora; 0 de comentário; `fake_ssh.py` limpo).
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard e70805b` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `make fmt` | 0 |
| `make compile` (log `.dev/logs/fase24-make-compile.log`; 0 warnings) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/tests/fake_ssh.py` | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/tests/ssh.rs crates/wardd/tests/run.rs` | 1 (360 identificadores de outras fases; 0 de comentário) |
| `cargo clippy` à parte | não rodado (o ambiente negou a chamada direta) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `e70805b`; reset com a árvore limpa.
- **Checador não limpo** em `ssh.rs`/`run.rs`: sobram só identificadores dos módulos
  ainda não traduzidos (lista em "Ficou de fora"); somem com as Fases 17–20 e 23.
- "estranho" no glossário é `odd`; aqui é o processo de fora da sessão e virou
  `stranger` (registrado no parágrafo "Fase 24" do glossário).
- **Conflitos prováveis no merge**: `docs/glossario.md` (parágrafo "Fase 24" depois do da
  Fase 21, antes de "Funções e métodos…"). A Fase 23 (`comum/` → `common/`) vai mexer no
  `mod comum`/`use comum::*` destes dois testes; a 20, nos construtores do `Daemon`; a
  17–18, no `protocolo`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/tests/fake_ssh.py` (era `ssh_falso.py`) | renomeado e reescrito |
| `crates/wardd/tests/ssh.rs` | reescrito |
| `crates/wardd/tests/run.rs` | reescrito |
| `docs/glossario.md` | alterado |
| `docs/mission/plan.md` | alterado (`fake_ssh.py` na Fase 52) |
| `docs/mission/handoffs/phase-24.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make fmt` e `make compile` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
