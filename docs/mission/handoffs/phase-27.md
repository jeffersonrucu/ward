# Fase 27 — `wardd`: testes `playwright.rs`, `extensao.rs`, `login.rs`, `chrome_falso.py`, `app_valor.rs`, `valor_app.rs`, `app_teste.rs`, `anexos_edicao.rs` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 5162340`
  (master atual). Fase adiantada em paralelo com as Fases 16, 23 e 32: só os nomes
  próprios destes testes mudaram; os que vêm do `comum` (Fase 23), do `nucleo`/
  `aprovacao`/`trava`/`arquivos` (Fases 19–20) e do `ward-core` (`protocolo`, Fases 17–18)
  ficaram como no master.
- **Arquivos** (glossário seção 4, `git mv`): `extensao.rs` → `extension.rs`,
  `app_teste.rs` → `test_app.rs`, `app_valor.rs` → `app_value_binary.rs`, `valor_app.rs`
  → `app_value_core.rs`, `anexos_edicao.rs` → `attachment_edit.rs`, `chrome_falso.py` →
  `fake_chrome.py`. `playwright.rs` e `login.rs` mantêm o nome (glossário: "ficam").
- **`Cargo.toml`** do `wardd` (achado da Fase 4): `[[test]]` `app_teste` → `test_app` e
  `app_valor` → `app_value_binary` (glossário seção 3). Os binários `wardd-app-teste` e
  `wardd-aprovador-teste` ficam (não são desta fase).
- **Usos fora dos arquivos**: `playwright.rs` (caminho `tests/fake_chrome.py`) e
  `docs/mission/plan.md` (Fase 52 cita `fake_chrome.py`). Nenhum alvo do Makefile nem script
  chama estes testes por nome de arquivo renomeado: o `make test-playwright` usa
  `--test playwright -- --ignored` (nome igual); nenhum script cita nome de função de teste
  (procurado com `grep -r` fora de `docs/mission/arquivo/`). O rótulo
  `excecao_chrome_falso` de `dev/tests/extension.py` é um literal daquele script, não o
  arquivo; fica.
- **Comentários, locais, parâmetros e nomes de teste** dos oito arquivos em inglês.
  Comentários que citavam nome pt-BR de fio ou de outra fase (`revelar`, `copiar`,
  `lista`, `abrir_app`, `sugerir`, `preencher`, `salvar_login`, `salvar_entrada`,
  `manter_anexos`, `aguardar_login`, `atender`, `origem_diferente`, `navegador_falhou`,
  `ocupado`, `travado`, `t-livre`/`t-aprov`/`t-bloq`, `gama-bloqueada`, `igual`,
  `comum::cofre_de_teste`, `SEGREDOS`, `/entrar`, os modos e os arquivos `modo`/`registro`
  do Chrome falso) foram reescritos com o sentido em inglês, sem o nome (como nas Fases 21
  e 24).
- O doc "`https` livre segue sem janela nenhuma." que estava colado ao do teste
  `https_loopback` de `playwright.rs` foi para o teste `playwright_https_free_without_window`,
  que estava sem doc. Só comentário.
- **Renomes (antigo → novo)**, todos no glossário (parágrafo "Fase 27", depois do da
  Fase 24):
  - `playwright.rs`: `SENHA_LOGIN` → `LOGIN_PASSWORD`, `Rt::novo` → `Rt::new`, `Cena`/`cena`
    → `Scene`/`scene` (`registro`/`eventos`/`sessao`/`sem_perfil` → `records`/`events`/
    `session`/`no_profile`), `formas_da_senha` → `password_forms`, `site::{Pedido { metodo,
    caminho, corpo }, porta, pedidos, Rota, subir, atender}` → `site::{Request { method,
    path, body }, port, requests, Route, start, serve}`, `formulario` → `form`,
    `com_clobber` → `with_clobber`, `login_em` → `login_at`, `CenaReal`/`cena_real` →
    `RealScene`/`real_scene` (`phish_limpo`/`senha_fora_da_url` → `phish_clean`/
    `password_out_of_url`), `CASOS` → `CASES`, `pagina` → `page`.
  - `extension.rs`: `SEMENTE`/`AGORA` → `SEED`/`NOW`, `RelogioFixo` → `FixedClock`, `cofre`
    → `vault`, `sugerir` → `suggest`, `preencher_cru`/`preencher` → `fill_raw`/`fill`,
    `nomes` → `names`, `sem_segredo` → `no_secret`, `salvar_login` → `save_login`,
    `nivel_na_lista` → `level_in_list`, `SENHA_NOVA` → `NEW_PASSWORD`, `SaiLogo` →
    `LeavesSoon`.
  - `login.rs`: `ORIGEM` → `ORIGIN`, `prazos` → `deadlines` (`retirada_ms` → `pickup_ms`),
    `cofre` → `vault`, `login_do_agente` → `agent_login`, `aguardar` → `await_login`,
    `pegar` → `take`, `preencher` → `fill`, `resultado` → `outcome`, `sem_valor` →
    `no_value`, `ClienteQueSai` → `LeavingClient`, `RelogioParado` → `StoppedClock`,
    `AppFalso` → `FakeApp`, `SemAbrir` → `NoOpen`, `responder` → `answer`, `login_http`/
    `login_na_url`/`login_http_completo` → `http_login`/`login_at_url`/`full_http_login`,
    `CaiDepois` → `DropsLater`.
  - `app_value_binary.rs` e `test_app.rs`: `APP_TESTE` → `TEST_APP`, `CLIENTE` → `CLIENT`,
    `DirSocket::novo` → `SocketDir::new`, `esperar` → `wait_for`, `ler` → `read`,
    `agulhas` → `needles`, `sem_segredo` → `no_secret`, `cliente_python` →
    `python_client`, `mandar` → `send`, `pedir` (local de `test_app.rs`) → `request`,
    `a_mao` → `by_hand`.
  - `app_value_core.rs`: `SEMENTE` → `SEED`, `RelogioFixo` → `FixedClock`, `com_totp` →
    `with_totp`, `cofre` → `vault`, `alvo_campo` → `field_target`, `valor` → `value`,
    `pedido_valor` → `value_request`.
  - `attachment_edit.rs`: `ANEXO_A`/`ANEXO_B` → `ATTACHMENT_A`/`ATTACHMENT_B`, `generico`
    → `generic`, `editar` → `edit`, `no_disco`/`anexos_no_disco` → `on_disk`/
    `attachments_on_disk`.
  - `fake_chrome.py`: `PASTA`/`MODO` → `FOLDER`/`MODE`, `registrar(**kw)` →
    `record(fields)` (as chamadas passam um dict com as mesmas chaves, na mesma ordem: o
    JSON gravado é byte a byte o mesmo), `origem` → `origin`, `ler`/`escrever` →
    `read_message`/`write_message`, `responder` → `reply`, `evento` → `event`,
    `pendentes`/`contador` → `pending`/`counter`, `pausar` → `pause`, `url_atual` →
    `current_url`, `usuario_senha` → `credentials`, `proximo` → `next_command`.
  - Testes: `v6_12_sessao_gravada_so_o_caminho_volta` →
    `v6_12_session_saved_only_the_path_comes_back`,
    `v6_15_redirecionamento_para_outra_origem_recusado` →
    `v6_15_redirect_to_another_origin_refused`, `d9_origem_da_pagina_e_do_envio` →
    `d9_origin_of_the_page_and_of_the_submission`, `s1_sessao_com_a_senha_nao_e_gravada`
    → `s1_session_with_the_password_is_not_saved`, `navegador_que_cai_ou_manda_lixo` →
    `browser_that_crashes_or_sends_garbage`, `d6_travar_durante_a_sessao_nada_gravado` →
    `d6_lock_during_the_session_nothing_saved`, `recusas_sem_abrir_o_navegador` →
    `refusals_without_opening_the_browser`, `d6_partida_limpa_sessoes_e_perfis` →
    `d6_startup_cleans_sessions_and_profiles`, `chrome_real_login_e_ataques` →
    `real_chrome_login_and_attacks`, `ataque_playwright_http_livre_so_com_confirmacao` →
    `attack_playwright_http_free_only_with_confirmation`,
    `ataque_playwright_https_loopback_livre_so_com_confirmacao` →
    `attack_playwright_https_loopback_free_only_with_confirmation`,
    `playwright_https_livre_sem_janela` → `playwright_https_free_without_window`;
    `sugestao_traz_ambiente_e_se_tem_totp` →
    `suggestion_carries_environment_and_whether_it_has_totp`,
    `v6_2_sugerir_so_a_origem_da_aba` → `v6_2_suggest_only_the_tab_origin`,
    `v6_2_preencher_so_na_origem_certa` → `v6_2_fill_only_on_the_right_origin`,
    `v6_1_agente_e_app_nao_sugerem_nem_preenchem` →
    `v6_1_agent_and_app_neither_suggest_nor_fill`,
    `ataque_extensao_sem_operacoes_de_app_nem_de_agente` →
    `attack_extension_without_app_or_agent_operations`,
    `v6_8_salvar_login_nasce_com_aprovacao` → `v6_8_save_login_starts_with_approval`,
    `ataque_salvar_login_nao_sobrescreve_entrada` →
    `attack_save_login_does_not_overwrite_entry`,
    `ataque_salvar_login_so_os_campos_do_popup` → `attack_save_login_only_the_popup_fields`,
    `ataque_salvar_login_so_extensao_e_destravado` →
    `attack_save_login_extension_only_and_unlocked`,
    `ataque_preencher_http_sem_confirmar_nada_sai` →
    `attack_fill_http_without_confirming_nothing_leaves`,
    `ataque_preencher_https_loopback_sem_confirmar_nada_sai` →
    `attack_fill_https_loopback_without_confirming_nothing_leaves`;
    `v6_10_login_livre_preenche_na_origem_certa_e_volta_ok` →
    `v6_10_free_login_fills_on_the_right_origin_and_returns_ok`,
    `v6_3_v6_15_ataque_origem_diferente_nao_preenche` →
    `v6_3_v6_15_attack_different_origin_does_not_fill`, `ataque_id_forjado_nao_preenche` →
    `attack_forged_id_does_not_fill`, `desfechos_da_extensao` → `extension_outcomes`,
    `ataque_agente_e_app_nas_operacoes_da_extensao` →
    `attack_agent_and_app_on_the_extension_operations`, `recusas_antes_da_extensao` →
    `refusals_before_the_extension`, `sem_extensao_o_pedido_morre` →
    `without_extension_the_request_dies`, `um_pedido_por_vez` → `one_request_at_a_time`,
    `travar_no_meio_mata_o_pedido` → `lock_in_the_middle_kills_the_request`,
    `cli_que_cai_mata_o_pedido` → `cli_that_drops_kills_the_request`,
    `aguardar_nao_conta_como_uso` → `await_does_not_count_as_use`,
    `v6_11_com_aprovacao_negar_nao_chega_a_extensao_e_permitir_chega` →
    `v6_11_with_approval_deny_does_not_reach_the_extension_and_allow_does`,
    `ataque_login_http_livre_so_com_confirmacao` →
    `attack_login_http_free_only_with_confirmation`,
    `ataque_login_http_com_aprovacao_pede_pin_e_mostra_a_origem` →
    `attack_login_http_approval_asks_pin_and_shows_the_origin`,
    `ataque_login_https_loopback_livre_so_com_confirmacao` →
    `attack_login_https_loopback_free_only_with_confirmation`,
    `login_https_livre_sem_janela` → `login_https_free_without_window`;
    `v3_39_v3_40_valores_e_lista_do_app_pelo_daemon_de_teste` →
    `v3_39_v3_40_app_values_and_list_through_the_test_daemon`;
    `v3_39_app_revela_e_copia_sem_pin_nem_aprovacao_ate_a_bloqueada` →
    `v3_39_app_reveals_and_copies_without_pin_or_approval_even_the_blocked_one`,
    `v3_39_totp_do_app_pelo_relogio_do_daemon` → `v3_39_app_totp_by_the_daemon_clock`,
    `ataque_v3_39_agente_e_extensao_nao_revelam_nem_copiam_nem_tiram_totp` →
    `attack_v3_39_agent_and_extension_neither_reveal_nor_copy_nor_take_totp`,
    `v3_40_lista_do_app_com_resumo_e_do_agente_sem_nivel_nem_resumo` →
    `v3_40_app_list_with_summary_and_agent_list_without_level_or_summary`;
    `v3_37_so_o_app_aberto_pelo_daemon_de_teste_e_app` →
    `v3_37_only_the_app_opened_by_the_test_daemon_is_app`;
    `v5_12_editar_mantem_adiciona_e_remove_anexo` →
    `v5_12_edit_keeps_adds_and_removes_attachment`,
    `v5_12_ataque_manter_anexos_forjado` → `v5_12_attack_forged_keep_attachments`.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo, fora de comentários, comparado com `5162340`
(extrator próprio no scratchpad; Rust por um léxico de strings normais e cruas, Python
pelo `tokenize`). Nenhum escape `\u` nos originais nem nos novos (aviso do orchestrator,
achado da Fase 16: conferido, nada a restaurar).
- `"tests/chrome_falso.py"` → `"tests/fake_chrome.py"` (caminho do arquivo renomeado).
- Só capturas de `format!` (valor igual): `{rotulo}` → `{label}`, `{caso}` → `{case}`,
  `{entrada}` → `{entry}`, `{nome}` → `{name}`, `{modo}` → `{mode}`, `{outra:?}` →
  `{other:?}`, `{fora}` → `{outside}`, `{atributos}` → `{attributes}`, `{inicio}` →
  `{start}`, `{vistos:?}` → `{seen:?}`, `{decisoes:?}` → `{decisions:?}`, `{aba}`/
  `{aba:?}` → `{tab}`/`{tab:?}`, `{origem}` → `{origin}`, `{ORIGEM}` → `{ORIGIN}`,
  `{decisao}` → `{decision}`, `{criar:?}` → `{create:?}`, `{valor}` → `{value}`,
  `{semente}` → `{seed}`, `{usuario}` → `{user}`, `{o_que}` → `{what}`, `{nao}` →
  `{not_authorized}`, `{dados}` → `{data}`, `{texto}` → `{text}`, `{saida}` → `{output}`,
  `{linha:?}` → `{line:?}`, `{onde}` → `{place}`, `{lista}` → `{list}`, `{pedido}` →
  `{request}`, `{proibido}` → `{forbidden}`, `{alvo}` → `{target}`, `{anterior}` →
  `{previous}`, `{papel:?}` → `{role:?}`.
- `fake_chrome.py`: as chaves do registro (`evento`, `argv`, `ambiente`, `home`, `cwd`,
  `fds`, `alvos`, `metodo`, `url`, `seguiu`, `sha256`, `origem`, `contexto`) eram nomes de
  argumento (`registrar(evento=…)`) e viraram chaves de dict literais com o mesmo texto;
  o JSON gravado é o mesmo. Nenhuma outra string mudou (arquivos `modo`/`registro`, modos,
  `"usuario, senha"` que o falso procura no script do daemon, chaves `senha`/`ls`, cookie
  `sessao`…).
- **Ficaram**: todas as mensagens que os testes procuram (log do daemon, saída do app de
  teste `papel=app`, `revelar=…`, `) saiu com código 0`…), as mensagens de `assert!`, os
  nomes de entrada, de arquivos e de pastas de teste (`wpw-…`, `wardd-v-…`,
  `copia-do-app`, `grande.bin`…), o HTML dos sites locais (`/entrar`, `recusa=1`…), o
  Python embutido no `CLIENT` de `app_value_binary.rs` e os JSON do protocolo (fio
  congelado).

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários): os testes de
  ataque de S1 (nenhum valor à CLI/agente/extensão; storage state sem a senha), S2/S3
  (argv e ambiente do Chrome, perfil apagado), S13 (log do daemon sem padrão), D4/D65
  (valores só pelo app), D6 (trava apaga sessão), D9 (origem, redirecionamento, `action`/
  `formaction`, clobbering, confirmação `http` e `https` de loopback), D29 (anexos
  mantidos), D63 (só o app aberto pelo daemon é app), V3.17, V3.37–V3.40, V5.12, V6.1–V6.15.
  Mesma lógica, mesmas asserções, mesmos valores, prazos e contagens.
- **Conferência**: multiconjunto de literais (acima) e leitura lado a lado; o
  `make compile` passa (0 warnings).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes que estes testes usam e que são de outras fases: `comum` e o que ele exporta
  (`Dir::novo`, `cofre_de_teste`, `ped`, `pedir`, `erro`, `ok`, `codificado`, `contem`,
  `mudar_nivel`, `destravar`, `com_janela`, `login`, `servidor`, `banco`, `SENHA`,
  `SEGREDOS`, `PIN`; Fase 23), `nucleo::{Daemon, Relogio}` e os métodos `novo`,
  `com_chrome`, `com_temporarios`, `com_prazos_login`, `com_motor`, `motor`, `atender`,
  `atender_de`, `travar`, `caminhos` (Fases 19–20), `aprovacao` (`Motor`, `RelogioMotor`,
  `Assinante`, `AbridorApp`, `ErroAbrirApp`, `Cliente::{cadeia, saiu}`, `SemCliente`;
  Fase 20), `trava::conta_como_uso`, `arquivos::{gravar_estado, Geracoes}` (Fase 19) e o
  `ward_core::protocolo` (`Papel`, `Resposta`, `ErroResposta`, `Operacao`, `Pedido`,
  `Alvo`, campos `nome`/`usuario`/`nivel`/`resumo`/`origem`/`confirmacao`…; Fases 17–18).
  O checador ainda acusa esses identificadores (todos de nomes de fora; 0 de comentário;
  `fake_chrome.py` limpo).
- Os binários `wardd-app-teste` e `wardd-aprovador-teste` (o glossário os põe em outra
  fase).
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 5162340` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `make fmt` (log `.dev/logs/fase27-make-fmt.log`) | 0 |
| `make compile` (log `.dev/logs/fase27-make-compile.log`; 0 warnings) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/tests/fake_chrome.py` | 0 |
| `python3 -I dev/english_check.py --paths` nos sete `.rs` da fase | 1 (só identificadores de outras fases; 0 de comentário) |
| comparação de literais com `5162340` (script no scratchpad) | só as diferenças listadas acima |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não em `5162340`; reset com a árvore limpa.
- **Checador não limpo** nos `.rs` da fase: sobram só identificadores dos módulos ainda
  não traduzidos (lista em "Ficou de fora"); somem com as Fases 17–20 e 23.
- **Conflitos prováveis no merge**: `docs/glossario.md` (parágrafo "Fase 27" depois do da
  Fase 24, antes de "Funções e métodos…"). A Fase 23 (`comum/` → `common/`) vai mexer no
  `mod comum`/`use comum::*` destes testes; as 19–20, nos construtores do `Daemon`, no
  `aprovacao` e no `trava`; as 17–18, no `protocolo`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/Cargo.toml` | alterado (`[[test]]` `test_app`, `app_value_binary`) |
| `crates/wardd/tests/extension.rs` (era `extensao.rs`) | renomeado e reescrito |
| `crates/wardd/tests/test_app.rs` (era `app_teste.rs`) | renomeado e reescrito |
| `crates/wardd/tests/app_value_binary.rs` (era `app_valor.rs`) | renomeado e reescrito |
| `crates/wardd/tests/app_value_core.rs` (era `valor_app.rs`) | renomeado e reescrito |
| `crates/wardd/tests/attachment_edit.rs` (era `anexos_edicao.rs`) | renomeado e reescrito |
| `crates/wardd/tests/fake_chrome.py` (era `chrome_falso.py`) | renomeado e reescrito |
| `crates/wardd/tests/playwright.rs` | reescrito |
| `crates/wardd/tests/login.rs` | reescrito |
| `docs/glossario.md` | alterado (parágrafo "Fase 27") |
| `docs/mission/plan.md` | alterado (`fake_chrome.py` na Fase 52) |
| `docs/mission/handoffs/phase-27.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make fmt` e `make compile` (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
