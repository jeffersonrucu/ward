# Fase 25 — `wardd`: testes `aprovacao.rs`, `trava.rs`, `nucleo.rs` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard f40e6c6`
  (master de então). Com a Fase 23 no master (`55c4645`), o trabalho foi refeito sobre ele
  (`git reset --hard master` e os mesmos scripts de rename), em vez de merge: o commit da
  fase fica direto sobre `55c4645`. Os nomes do `common` (Fase 23) vêm do master; os de
  `nucleo`/`aprovacao`/`trava`/`arquivos`/`uso` (Fases 19–20) e do `ward-core`
  (`protocolo`, Fases 17–18) ficaram como no master.
- **Arquivos** (glossário seção 4, `git mv`): `crates/wardd/tests/aprovacao.rs` →
  `approval.rs`, `trava.rs` → `lock.rs`, `nucleo.rs` → `core.rs`.
- **`Cargo.toml`** do `wardd` (achado da Fase 4; glossário seção 3): `[[test]]` `trava` →
  `lock` (`required-features = ["test-mode"]` igual). `approval` e `core` não têm
  `[[test]]` (não tinham antes).
- **Usos fora dos arquivos**: nenhum. Nenhum alvo do Makefile, script de `dev/`/`install/`
  ou `CLAUDE.md` chama `--test trava|nucleo|aprovacao` nem cita nome de função destes
  testes (procurado com `grep -r` fora de `target/`, `.git/` e `docs/mission/arquivo/`).
  `docs/mission/plan.md` só cita os arquivos de `src/` (Fases 19–20), que não mudam aqui.
- **Comentários** dos três arquivos em inglês. Comentários que citavam nome pt-BR de fio,
  de método de fora ou de outra fase (`estado`, `destravar`, `ward lista`, `detalhe`,
  `totp`, `agora`/`dormir`, `mudar_nivel`, `trocar_pin`, `trocar_senha`,
  `senha_incorreta`, `cofre_inexistente`, `abrir_app`, `pedido_de_destrave`,
  `pedido_invalido`, `executar`, `entrada_oculta`, `Assinante`, `Daemon::atender`,
  `Daemon::novo`, `com_gancho_depois_de_gravar`/`com_programas_ssh`/`com_prazos_login`,
  `trava.rs`/`config.rs`… do V3.15, `acme-pg-aprovacao`, os botões "Permitir…") foram
  reescritos com o sentido em inglês, sem o nome (como nas Fases 21, 24 e 27).
- **Renomes (antigo → novo)**, todos no glossário (parágrafo "Fase 25", depois do da
  Fase 27):
  - `lock.rs`: macro `emitir` → `emit`; `SESSAO`/`IFACE_SESSAO` → `SESSION`/
    `SESSION_IFACE` (`H`, `LOGIND`, `WARDD` ficam); `RelogioManual` → `ManualClock`
    (`novo`/`esperas`/`por`/`ilegivel` → `new`/`waits`/`set`/`unreadable`);
    `cofre_com_relogio` → `vault_with_clock`; `lista_ok` → `list_ok`; `executar` →
    `run_request`; `BarramentoFalso { filho }` → `FakeBus { child }` (`subir`/`endereco`/
    `cliente`/`matar` → `start`/`address`/`client`/`kill`); `Inibidores` → `Inhibitors`;
    `Manager { inibidores }` → `Manager { inhibitors }`; `LogindFalso` → `FakeLogind`
    (`novo`/`inibidor` → `new`/`inhibitor`); `daemon_vigiado` → `watched_daemon`;
    `espera_travar` → `wait_locked`; `mandar` → `send`; `pedir_por` → `request_over`;
    `Filho` → `KillOnDrop`; `esperar_no_log` → `wait_in_log`; locais (`criar`, `negado`,
    `vistos`, `fim`, `mudou`, `valor`, `fechou`, `ini`, `limite`, `forjador`, `caminho`,
    `membro`, `texto`, `seg`, `_pronto`…) em inglês.
  - `core.rs`: `lista` → `list`, `nivel_de` → `level_of`, `sou_root` → `is_root`,
    `cofre_n2` → `vault_n2`, `geracoes_em` → `generations_at`; locais (`papel`, `dados`,
    `esperadas`, `quadros`, `esperas`, `relogio`, `cofre`, `antes`, `sem_pin`, `agente`,
    `executar`, `bloqueada`, `com_nivel`, `editar`, `renomear`, `copia_n`, `outro`,
    `dir_estado`, `copia_a`, `nova`, `troca`, `criar`, `arq`…) em inglês.
  - `approval.rs`: `PRAZO` → `DEADLINE`; `RelogioManual` → `ManualClock` (`ir`/`avancar`
    → `set`/`advance`); `Recebido { Aprovacao, Destrave }` → `Received { Approval,
    Unlock }`; `AppFalso` → `FakeApp`; `AoSair` → `OnExit`; `AbridorFalso { resultado,
    aberturas, ao_sair }` → `FakeOpener { result, opens, on_exit }` (`novo`/`sair` →
    `new`/`exit`); `Cena { relogio, abridor }` → `Scene { clock, opener }`; `cena`/
    `cena_com` → `scene`/`scene_with`; `assinar` → `subscribe`; `executar_json(_em)` →
    `run_json(_in)`; `usar` → `use_entry`; `pedido` → `approval_request`; `responder` →
    `answer`; `nivel` → `level`; `ExpirarSeCair` → `ExpireOnPanic`; `com_janela` →
    `with_approval_window` (não `with_window`, que é o do `common` usado pelo `lock.rs`);
    `fontes_rs` → `rs_sources`; `GANCHOS_DE_TESTE`/`CFG_GANCHOS` → `TEST_HOOKS`/
    `CFG_HOOKS`; `conferir_v3_15` → `check_v3_15`; `destrave` → `unlock_request`;
    `usar_com_entrada` → `use_with_stdin`; `com_janela_e_entrada` →
    `with_window_and_stdin`; `responder_ciente` → `answer_confirmed`; `ClienteForjado` →
    `ForgedClient`; `Caiu` → `Dropped`; locais em inglês.
  - Testes de `lock.rs`: `v3_19_inatividade_com_relogio_injetado` →
    `v3_19_inactivity_with_injected_clock`, `v3_19_tempo_lido_da_configuracao_do_cofre` →
    `v3_19_timeout_read_from_the_vault_settings`,
    `ataque_agente_nao_segura_o_cofre_aberto_com_estado_ou_erro` →
    `attack_agent_does_not_keep_the_vault_open_with_status_or_errors`,
    `ataque_lista_periodica_do_agente_nao_adia_a_trava` →
    `attack_periodic_agent_list_does_not_delay_the_lock`,
    `ataque_f55_ward_run_negado_e_janela_nao_adiam_a_trava` →
    `attack_f55_denied_ward_run_and_window_do_not_delay_the_lock`,
    `d62_trava_automatica_nao_mexe_na_espera_do_d20` →
    `d62_auto_lock_does_not_touch_the_d20_wait`, `ataque_relogio_que_volta_ou_ilegivel_trava`
    → `attack_clock_that_goes_back_or_is_unreadable_locks`,
    `v3_19_lock_e_locked_hint_travam_em_ate_2s` → `v3_19_lock_and_locked_hint_lock_within_2s`,
    `v3_19_prepare_for_sleep_trava_antes_de_soltar_o_inibidor` →
    `v3_19_prepare_for_sleep_locks_before_releasing_the_inhibitor`,
    `ataque_barramento_cai_com_o_cofre_aberto_trava` →
    `attack_bus_drops_with_the_vault_open_locks`,
    `ataque_sinal_forjado_nao_destrava_nem_segura` →
    `attack_forged_signal_neither_unlocks_nor_holds`,
    `v3_19_binario_trava_com_lock_session_do_logind_falso` →
    `v3_19_binary_locks_on_lock_session_of_the_fake_logind`,
    `daemon_de_teste_sem_barramento_falso_nao_liga_o_logind` →
    `test_daemon_without_fake_bus_does_not_enable_logind`.
  - Testes de `core.rs`: `v3_10_travar_e_tudo_responde_travado` →
    `v3_10_lock_and_everything_answers_locked`,
    `v3_9_senha_errada_espera_crescente_sem_contagem` →
    `v3_9_wrong_password_growing_wait_without_count`,
    `v3_11_mudar_nivel_exige_pin_e_papel_app` → `v3_11_change_level_requires_pin_and_app_role`,
    `v3_12_cinco_erros_de_pin_travam` → `v3_12_five_pin_errors_lock`,
    `v3_17_v3_26_lista_do_agente_e_bloqueada_igual_inexistente` →
    `v3_17_v3_26_agent_list_and_blocked_equals_missing`,
    `v3_27_entrada_nova_nasce_com_aprovacao` → `v3_27_new_entry_starts_with_approval`,
    `v3_22_rollback_e_arquivo_trocado` → `v3_22_rollback_and_swapped_file`,
    `v3_28_geracao_queimada_no_estado` → `v3_28_burned_generation_in_the_state`,
    `d52_trocar_senha_com_senha_errada` → `d52_change_password_with_wrong_password`,
    `primeiro_uso_e_pin_so_como_hash` → `first_use_and_pin_only_as_hash`.
  - Testes de `approval.rs`: `v3_14_sem_resposta_expira_em_2_min` →
    `v3_14_no_answer_expires_in_2_min`, `v3_14_negar_e_fechar_negam` →
    `v3_14_deny_and_close_deny`, `v3_14_uma_vez_so_aquele_pedido` →
    `v3_14_once_only_that_request`, `v3_14_uma_hora_ate_59_59_e_travar_encerra` →
    `v3_14_one_hour_until_59_59_and_lock_ends_it`, `v3_14_sempre_deixa_livre_no_arquivo` →
    `v3_14_always_leaves_free_in_the_file`, `ataque_49a_restaurar_cancela_pedido_pendente` →
    `attack_49a_restore_cancels_pending_request`, `ataque_permitir_sem_pin_valido` →
    `attack_allow_without_valid_pin`, `ataque_agente_responde_a_aprovacao` →
    `attack_agent_answers_the_approval`, `v3_15_sem_aprovador_no_binario` →
    `v3_15_no_approver_in_the_binary`, `ataque_v3_15_gancho_sem_feature_ou_fora_do_lugar` →
    `attack_v3_15_hook_without_feature_or_out_of_place`,
    `v3_16_prod_tem_os_mesmos_niveis_e_botoes` → `v3_16_prod_has_the_same_levels_and_buttons`,
    `d19_nivel_combinado` → `d19_combined_level`, `d21_aprovacao_indisponivel` →
    `d21_approval_unavailable`, `app_aberto_recebe_o_pedido_ao_assinar` →
    `opened_app_receives_the_request_on_subscribe`,
    `d51_comando_e_cadeia_com_escape_visivel` → `d51_command_and_chain_with_visible_escape`,
    `f26a_diretorio_de_trabalho_vai_a_janela_com_escape_visivel` →
    `f26a_working_directory_goes_to_the_window_with_visible_escape`,
    `cliente_que_cai_cancela` → `client_that_drops_cancels`,
    `v3_18_espera_o_destrave_ate_2_min` → `v3_18_waits_for_unlock_up_to_2_min`,
    `ataque_fase_39a_destrave_com_entrada_e_comando_neutralizados` →
    `attack_phase_39a_unlock_with_entry_and_command_neutralized`,
    `ataque_fase_39a_entrada_vazia_nao_derruba_o_app` →
    `attack_phase_39a_empty_entry_does_not_bring_down_the_app`, `d57_abrir_app` →
    `d57_open_app`, `v3_36_so_o_pid_do_lancador_e_app` → `v3_36_only_the_launcher_pid_is_app`,
    `v3_38_estado_traz_o_papel` → `v3_38_status_carries_the_role`,
    `f54b_ataque_pipe_na_aprovacao_sem_a_caixa_nao_passa` →
    `f54b_attack_pipe_on_approval_without_the_box_does_not_pass`,
    `f54b_terminal_e_nula_passam_como_antes` → `f54b_terminal_and_null_pass_as_before`,
    `f54b_liberacao_de_1_hora_nao_cobre_pipe` → `f54b_1_hour_grant_does_not_cover_pipe`,
    `f54b_ssh_com_pipe_avisa` → `f54b_ssh_with_pipe_warns`.
- **Ficaram** (nomes de fora): métodos dos traits do `wardd` implementados aqui
  (`Relogio::agora`/`dormir`, `RelogioMotor::agora`, `Assinante::entregar`,
  `AbridorApp::abrir`, `Cliente::cadeia`/`saiu`), campos de tipos de fora em padrões e
  literais de struct (`Resposta::Estado { papel, cofre_existe, travado }`,
  `Pedido::ResponderAprovacao { pedido, decisao, … }`, `i.nome`, `i.nivel`, `c.cofre`,
  `c.estado`, `Geracoes { vista, queimada }`), e os construtores `Daemon::novo`,
  `Motor::novo`, `com_motor`, `motor()`, `travar_se_inativo`, `vigiar_logind`…
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo comparado com `55c4645` (léxico próprio no
scratchpad: strings normais, cruas, de bytes e de caractere, fora de comentários). Só
capturas de `format!` (valor igual):
- `lock.rs`: `{limite:?}` → `{limit:?}`, `{texto:?}` → `{text:?}`, `{criar:?}` →
  `{create:?}`, `{outra:?}` → `{other:?}`, `{entrada}` → `{entry}` (JSON do `executar`),
  `{dados}` → `{data}` (quadro do protocolo).
- `core.rs`: `{nome}` → `{name}` (JSON do `executar`).
- `approval.rs`: `{texto:?}` → `{text:?}`, `{outra:?}`/`{outro:?}` → `{other:?}`,
  `{arquivos:?}` → `{files:?}`, `{decisao}` → `{decision}`, `{tipo}` → `{kind}`,
  `{papel:?}` → `{role:?}`, `{proibido}` → `{forbidden}`, `{assinatura}` →
  `{signature}`, `{nome}` → `{name}`, `{versao}` → `{version}`.
- Nenhum outro literal mudou: os nomes de arquivo de `src/` e de gancho que o V3.15
  procura (`"trava.rs"`, `"nucleo.rs"`, `"aprovacao.rs"`, `"uso.rs"`, `"server.rs"` da
  Fase 23, `"com_chrome"`…), as palavras proibidas (`"WARD_APROV"`…), as mensagens do log
  que os testes procuram (`"logind: vigiando"`, `"tela bloqueada: cofre travado"`,
  `"logind desligado"`, `"WARD_DEV_DBUS_SISTEMA recusado"`), os JSON do protocolo e as
  mensagens de `assert!`. Os literais com escape `\u{202E}`/`\\u202e` (D51, Fase 39a) não
  passaram por ferramenta de escrita: o rename foi feito por script que só mexe fora das
  strings, e a comparação confirma que estão byte a byte iguais.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários): os testes de
  ataque de S1 (lista do agente, bloqueada igual a inexistente), S7 (gravação, geração
  queimada, retrocesso), S8/S9 (trava, PIN, 5 erros), S10 (aprovação expira, fechar nega),
  S13 (log sem segredo), D2, D8/V3.15 (sem aprovador no binário), D16, D19, D20, D21, D51,
  D57, D62, D63, D68 e V3.19 (logind falso). Mesma lógica, mesmas asserções, mesmos valores
  e prazos.
- **Conferência**: multiconjunto de literais (acima), lista de identificadores que o
  checador ainda acusa (todos de fora) e `make compile` (0, 0 warnings).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os identificadores de fora que o checador ainda acusa (1077 achados de identificador, 0
  de comentário, nos três arquivos): `ward_core::protocolo` (Fases 17–18), `wardd::nucleo`,
  `aprovacao`, `trava`, `arquivos`, `uso` (Fases 19–20). Somem com essas fases.
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard f40e6c6` (worktree nasceu em `b4d5e1b`) e depois `git reset --hard master` (`55c4645`, com a Fase 23), árvore sem commit da fase | 0 |
| `make fmt` | 0 |
| `make compile` (log `.dev/logs/fase25-make-compile.log`; 0 warnings) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/tests/{lock,core,approval}.rs` (log `.dev/logs/fase25-english-check.log`) | 1 (1077 identificadores de outras fases; 0 de comentário) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não no master; reset com a árvore limpa.
- **Teste de integração chamado `core`** (glossário seção 4: `tests/nucleo.rs` →
  `core.rs`): o crate do teste se chama `core`, como o da biblioteca padrão. O
  `cargo check --all-targets` passa (o `rmeta` `libcore-*.rmeta` do teste é gerado), mas o
  build e a execução (`cargo test --test core`) só rodam no M6. Se der conflito lá, o
  nome alternativo é `daemon_core.rs` (glossário e handoff juntos).
- O comentário do V3.15 citava `trava.rs`/`config.rs`… pelos nomes de `src/`, que as
  Fases 19–20 renomeiam; o comentário agora fala dos módulos sem o nome de arquivo, mas os
  literais do teste (`"trava.rs"`, `"nucleo.rs"`, `"aprovacao.rs"`, `"uso.rs"`) e os
  nomes dos ganchos têm de mudar nas Fases 19–20 junto com os arquivos de `src/`, senão o
  V3.15 passa a não achar o que procura.
- **Conflitos prováveis no merge**: `docs/glossario.md` (parágrafo "Fase 25" depois do da
  Fase 27). As Fases 17–20 vão mexer nos `use` e nas chamadas dos três testes.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/tests/approval.rs` (era `aprovacao.rs`) | renomeado e reescrito |
| `crates/wardd/tests/lock.rs` (era `trava.rs`) | renomeado e reescrito |
| `crates/wardd/tests/core.rs` (era `nucleo.rs`) | renomeado e reescrito |
| `crates/wardd/Cargo.toml` | alterado (`[[test]]` `lock`) |
| `docs/glossario.md` | alterado (parágrafo "Fase 25") |
| `docs/mission/handoffs/phase-25.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make fmt` e `make compile` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
