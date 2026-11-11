# Fase 26 — `wardd`: testes `external.rs`, `backup.rs`, `settings.rs`, `d62.rs` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 55c4645`
  (master atual). Fase em paralelo com as Fases 17, 25 e 33: só os nomes próprios destes
  testes mudaram; os do `ward-core` (`protocolo`: `Papel`, `Resposta`, `ErroResposta`,
  `Pedido`, `VersaoBackup` e os campos `nome`/`conferida`/`fora_de_ordem`/`ligado`/
  `montado`/`minutos_trava`…), do `nucleo` (`Daemon::novo`, `com_pendrives`,
  `sincronizar_externo`, `ligar_copia_externa`, `com_gancho_depois_de_gravar`, `travar`,
  `travado`, `atender`, `Caminhos { cofre, estado }`, `Relogio` com `dormir`/`agora`,
  `ESPERA_MAXIMA`, `espera_destrave`) e do `arquivos` (`Geracoes { vista, queimada }`,
  `ler_estado_completo`, `gravar_estado`, `falhas_agente`) ficaram como no master.
- **Arquivos** (glossário seção 4, `git mv`): `tests/externo.rs` → `tests/external.rs`,
  `tests/configuracoes.rs` → `tests/settings.rs`; `backup.rs` e `d62.rs` mantêm o nome.
  Nenhum deles tem `[[test]]` no `Cargo.toml`; nenhum alvo do Makefile nem script chama
  estes testes pelo arquivo ou pelo nome da função (procurado fora de
  `docs/mission/arquivo/` e dos handoffs).
- **Comentários, locais, parâmetros, tipos auxiliares e nomes de teste** dos quatro
  arquivos em inglês. Comentários que citavam nome pt-BR de fio ou de outro módulo
  (`criar_cofre`, `configuracao`, `pendrives`, `pendrives_indisponiveis`,
  `pedido_invalido`, `nucleo::backups`, `dormir`, `vista`/`queimada`, o conteúdo
  `antiga`) foram reescritos com o sentido em inglês, sem o nome (como nas Fases 21, 24
  e 27).
- **Renomes (antigo → novo)**, todos no glossário (parágrafo "Fase 26", depois do da
  Fase 27):
  - `external.rs`: `Fonte { vistos, proximas, consultas }` → `Source { seen, next,
    queries }` (`com`/`por` → `with`/`set`), `pendrive` → `usb_drive`, `montagem` →
    `mount_point`, `ligar` → `turn_on`, `locais` → `local_versions`, `arvore` → `tree`,
    `pasta_externa` → `external_dir`, `no_pendrive` → `on_drive`, `BarramentoFalso {
    filho }` → `FakeBus { child }` (`subir`/`endereco` → `start`/`address`), `Objeto {
    caminho, rotulo, dispositivo, montagem, sistema, de_loop }` → `Object { path, label,
    device, mount, system, is_loop }`, `Objetos`/`objetos`/`objeto` →
    `Objects`/`objects`/`object`, `Gerente` → `Manager`, `UdisksFalso` → `FakeUdisks`
    (`novo`/`montou` → `new`/`mounted`), `cutucoes` → `nudges`.
  - `backup.rs`: `pasta` → `backup_dir`, `modo` → `mode`, `versoes` → `versions`,
    `restaurar` → `restore`, `nomes_da_lista` → `list_names`, `tempo_trava` →
    `lock_timeout`, `versao_em` → `version_at`, `plantar` → `plant`.
  - `settings.rs`: `configuracao` → `settings`, `tempo_trava` → `lock_timeout`,
    `trocar_pin` → `change_pin`, `trocar_senha` → `change_password`.
  - `d62.rs`: `RelogioMovel { agora, esperas }` → `MovableClock { now, waits }` (`andar`
    → `advance`), `Preso { entrou, soltar, esperas }` → `Stuck { entered, release,
    waits }`, `daemon_movel` → `movable_daemon`, `destravar_como` → `unlock_as`,
    `texto_estado` → `state_text`, `falhas_no_disco` → `failures_on_disk`,
    `cofre_travado` → `locked_vault`, `trocar_por_copia` → `swap_for_copy`.
  - Testes (45): `v7_4_desligado_por_padrao_nao_grava_nada` →
    `v7_4_off_by_default_writes_nothing`,
    `v7_5_v7_6_copia_so_para_o_pendrive_escolhido_e_sem_segredo` →
    `v7_5_v7_6_copies_only_to_the_chosen_drive_and_without_secret`,
    `ataque_pendrive_trocado_ou_uuid_repetido_nao_recebe_nada` →
    `attack_swapped_drive_or_repeated_uuid_gets_nothing`,
    `ataque_montagem_ou_pasta_que_e_link` → `attack_mount_or_folder_that_is_a_link`,
    `ataque_versao_existente_no_pendrive_nao_e_sobrescrita` →
    `attack_existing_version_on_the_drive_is_not_overwritten`,
    `pendrive_fica_com_30_versoes` → `drive_keeps_30_versions`,
    `f54c_ataque_nomes_plantados_no_pendrive_nao_apagam_copias_reais` →
    `f54c_attack_names_planted_on_the_drive_do_not_delete_real_copies`,
    `ataque_versoes_com_data_futura_no_pendrive_nao_apagam_as_reais` →
    `attack_future_dated_versions_on_the_drive_do_not_delete_the_real_ones`,
    `ataque_lixo_no_backup_local_nao_gira_o_pendrive` →
    `attack_junk_in_the_local_backup_does_not_rotate_the_drive`,
    `travado_ou_desligado_nao_copia` → `locked_or_off_does_not_copy`,
    `v7_8_agente_e_extensao_nao_ligam_nem_leem` →
    `v7_8_agent_and_extension_neither_turn_on_nor_read`,
    `pendrives_sem_udisks_e_uuid_invalido` → `usb_drives_without_udisks_and_invalid_uuid`,
    `udisks_falso_e_lido_com_as_conferencias` → `fake_udisks_is_read_with_the_checks`,
    `v7_5_sinal_do_udisks_copia_para_o_pendrive_montado` →
    `v7_5_udisks_signal_copies_to_the_mounted_drive`, `externo_nao_usa_link` →
    `external_does_not_use_link`; `v7_1_cada_gravacao_do_daemon_vai_para_o_backup` →
    `v7_1_each_daemon_write_goes_to_the_backup`, `v7_8_agente_e_extensao_nao_restauram`
    → `v7_8_agent_and_extension_do_not_restore`,
    `v7_11_restaurar_sela_geracao_nova_e_copia_a_mao_e_retrocesso` →
    `v7_11_restore_seals_a_new_generation_and_hand_copy_is_rollback`,
    `ataque_restaurar_sem_pin_senha_errada_e_travado` →
    `attack_restore_without_pin_wrong_password_and_locked`,
    `ataque_pasta_de_backups_trocada_por_link` → `attack_backups_folder_swapped_for_a_link`,
    `ataque_versao_trocada_por_link` → `attack_version_swapped_for_a_link`,
    `ataque_restaurar_com_cofre_trocado_fora_do_ward` →
    `attack_restore_with_vault_swapped_outside_ward`,
    `ataque_restaurar_sem_gravar_o_estado_nao_troca_o_cofre` →
    `attack_restore_without_writing_the_state_does_not_swap_the_vault`,
    `restaurar_pede_a_senha_da_versao` → `restore_asks_for_the_version_password`,
    `ataque_versoes_com_data_futura_nao_apagam_as_reais_nem_restauram` →
    `attack_future_dated_versions_neither_delete_the_real_ones_nor_restore`,
    `restaurar_mantem_tempo_de_trava_e_backup_externo_atuais` →
    `restore_keeps_the_current_lock_timeout_and_external_backup`,
    `pasta_de_backups_insegura_aparece_na_configuracao` →
    `unsafe_backups_folder_shows_in_the_settings`,
    `temporarios_do_estado_somem_na_partida` → `state_temp_files_go_away_at_startup`,
    `f54l_ataque_fifo_trocado_no_lugar_da_versao_nao_prende_o_daemon` →
    `f54l_attack_fifo_swapped_in_place_of_the_version_does_not_hold_the_daemon`;
    `v5_22_padrao_4h_desligado_e_tempo_alteravel_gravado_no_cofre` →
    `v5_22_default_4h_off_and_changeable_timeout_saved_in_the_vault`,
    `ataque_agente_e_extensao_nao_leem_nem_mudam_a_configuracao` →
    `attack_agent_and_extension_neither_read_nor_change_the_settings`,
    `ataque_configuracao_com_o_cofre_travado` → `attack_settings_with_the_vault_locked`,
    `ataque_resposta_configuracao_sem_hash_nem_pin` →
    `attack_settings_reply_without_hash_or_pin`, `v5_22_trocar_pin_pede_o_pin_atual` →
    `v5_22_change_pin_asks_for_the_current_pin`,
    `ataque_adivinhar_pin_pela_troca_trava_no_quinto_erro` →
    `attack_guessing_the_pin_through_the_change_locks_on_the_fifth_error`,
    `v5_22_trocar_senha_pede_a_atual_e_mantem_a_configuracao` →
    `v5_22_change_password_asks_for_the_current_one_and_keeps_the_settings`,
    `ataque_senha_mestra_nova_curta_recusada_pelo_daemon` →
    `attack_short_new_master_password_refused_by_the_daemon`,
    `senha_mestra_composta_abre_decomposta_pelo_daemon` →
    `composed_master_password_opens_decomposed_through_the_daemon`,
    `f55_ataque_fifo_no_lugar_do_cofre_nao_prende_a_troca_de_senha` →
    `f55_attack_fifo_in_place_of_the_vault_does_not_hold_the_password_change`;
    `v3_33_espera_do_agente_sobrevive_ao_reinicio` →
    `v3_33_agent_wait_survives_the_restart`,
    `ataque_agente_no_meio_da_espera_nao_atrasa_o_app` →
    `attack_agent_in_the_middle_of_the_wait_does_not_delay_the_app`,
    `v3_33_formato_antigo_e_estado_sem_segredo` →
    `v3_33_old_format_and_state_without_secret`,
    `v3_35_criar_cofre_com_estado_velho_e_copia_antiga` →
    `v3_35_create_vault_with_old_state_and_old_copy`,
    `v3_35_criar_cofre_sem_estado_e_com_estado_invalido` →
    `v3_35_create_vault_without_state_and_with_invalid_state`,
    `v3_35_troca_logo_depois_do_rename_e_recusada` →
    `v3_35_swap_right_after_the_rename_is_refused`.
- **Literais** (PRD regra 5): conferidos por script (multiconjunto dos literais de cada
  arquivo contra o master). Só mudam nomes de captura de `format!`/`assert!` (achado da
  Fase 7; o texto produzido é o mesmo): `{proibido}` → `{forbidden}` (`external.rs`);
  `{versao}`/`{alvo}`/`{minutos}`/`{lista:?}`/`{outra:?}`/`{nome}` (8 nomes de
  temporário)/`{trocas} trocas, {listas} listas` → `{version}`/`{target}`/`{minutes}`/
  `{list:?}`/`{other:?}`/`{name}`/`{swaps} trocas, {lists} listas` (`backup.rs`);
  `{minutos}`/`{fora}`/`{papel:?}`/`{agulha}: {texto}`/`{texto}`/`{novo:?}`/
  `{curta:?}`/`{outra:?}`/`{trocas} trocas, {pedidos} pedidos, {lidos} lidos` →
  `{minutes}`/`{out_of_range}`/`{role:?}`/`{needle}: {text}`/`{text}`/`{new:?}`/
  `{short:?}`/`{other:?}`/`{swaps} trocas, {requests} pedidos, {read_ok} lidos`
  (`settings.rs`); `{criar:?}` → `{create:?}` (`d62.rs`). O `"PEN\u{1b}[2J"` continua
  escapado (nenhum caractere de controle cru nos arquivos), e as operações, campos de
  fio, nomes de pasta (`backups`, `ward-backup`, `pendrive-a`…), mensagens e valores
  falsos ficam iguais.
- `external_does_not_use_link` continua lendo `../src/external.rs` e procurando
  `rename_no_replace(destination.as_fd()` (presente 1 vez no `src/external.rs` atual) e os
  mesmos quatro proibidos.
- `allow.txt`/`frozen.txt`: nenhuma entrada nova.

## Segurança
- Invariantes tocadas: nenhuma lógica. Os testes de ataque de V3.33, V3.35, V5.22, V7.1,
  V7.4–V7.6, V7.8, V7.11 e das Fases 49a, 54, 54c, 54l e 55 (S1, S7, S9, S13) mudaram só
  nomes e comentários: mesmos passos, mesmos valores, mesmas asserções e as mesmas
  mensagens.
- Testes de ataque escritos: nenhum novo (fase de tradução); os existentes renomeados
  acima. Não rodaram (PRD regra 6).
- Dependências novas: nenhuma.

## Ficou de fora
- Os nomes do `nucleo`, do `arquivos` (Fases 19–20) e do `ward-core::protocolo` (Fases
  17–18) usados por estes testes: são dessas fases. O checador ainda acusa 667 linhas de
  identificador nos quatro arquivos, todas desses nomes (lista no log).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make fmt` (log `.dev/f26/fmt-fase26.log`; só os quatro arquivos da fase mudaram) | 0 |
| `make compile` (log `.dev/f26/compile-fase26.log`; 0 warning; inclui `cargo check --workspace --all-targets --all-features`, com `test-hooks`) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (extra, log `.dev/f26/clippy-fase26.log`) | 0 |
| `make english-check PATHS="crates/wardd/tests/external.rs crates/wardd/tests/backup.rs crates/wardd/tests/settings.rs crates/wardd/tests/d62.rs"` (log `.dev/f26/english-check-fase26.log`) | 2 (667 identificadores de outras fases; 0 comentário, 0 arquivo, 0 problema nas exceções) |

`make check` não rodou: pelo plano (PRD regra 6), as fases dos Milestones 1–5 terminam
com `make compile` e o checador; a bateria fica para o Milestone 6.

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `55c4645`; reset com a árvore limpa.
- `docs/mission/plan.md` (linha da Fase 26) ainda cita `externo.rs`, `configuracoes.rs` e
  o teste `externo_nao_usa_link` (agora `tests/external.rs`, `tests/settings.rs` e
  `external_does_not_use_link`); o plano é do orchestrator.
- Homônimos em `app/core/src/configuracoes.rs` (`v5_22_padrao_4h_desligado_e_tempo_alteravel`,
  `v5_22_trocar_pin_pede_o_pin_atual`) e em `crates/wardd/src/arquivos.rs`
  (`f54l_ataque_fifo_no_lugar_nao_prende_a_leitura`) são outros testes (Fases 33 e 19) e
  não mudaram.
- **Arquivo compartilhado**: `docs/glossario.md` (parágrafo novo depois do da Fase 27;
  conflito de merge trivial se a Fase 25 também escrever ali).
- Quando as Fases 17–20 renomearem `Papel`, `Resposta`, `nucleo`, `arquivos` etc., estes
  quatro arquivos mudam só nos usos (nada local depende desses nomes).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/tests/externo.rs` → `crates/wardd/tests/external.rs` | renomeado e alterado |
| `crates/wardd/tests/configuracoes.rs` → `crates/wardd/tests/settings.rs` | renomeado e alterado |
| `crates/wardd/tests/backup.rs` | alterado |
| `crates/wardd/tests/d62.rs` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-26.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não, pela regra 6
  do PRD; rodaram `make compile`, `make fmt`, `cargo clippy -p wardd -D warnings` e o
  checador
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes, renomeados; não rodaram
  (regra 6)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
