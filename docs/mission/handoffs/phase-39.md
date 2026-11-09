# Fase 39 — (UI) Bandeja, roteiro do usuário da janela nativa e varredura de logs do app

## Implementado
- **Bandeja (V5.23).** Ícone e menu nativos pelo AppIndicator, com a feature `tray-icon` do Tauri.
  - Modelo sem Tauri em `app/nucleo/src/bandeja.rs`: textos por estado (frames `Bandeja / Destravado` `17269` e `/ Travado` `17300`), ids dos itens, ações e a pasta do ícone.
  - Menu destravado: "Cofre destravado" e "Trava sozinho após o tempo sem uso" (os dois desabilitados), separador, "Abrir o Ward", "Travar agora", separador, "Sair".
  - Menu travado: "Cofre travado", "Destrave com a senha mestra", "Abrir o Ward", "Destravar…", "Sair".
  - Estados extras: "Nenhum cofre criado" e "Daemon indisponível". Nos dois, o ícone é o de travado e a ação fica desabilitada (falha fechada).
  - Ícones `Ícone da bandeja` branco (`17138` e `17142`) embutidos com `include_image!` a partir de `docs/design/icones/bandeja-{destravado,travado}-32.png`, copiados para `app/src-tauri/icons/`.
  - `app/src-tauri/src/main.rs` cria o menu (`criar_bandeja`). Uma vigia consulta o estado a cada 2 s (`vigiar_bandeja` e `atualizar_bandeja`). Quando o estado muda, troca textos e ícone e manda o aviso `estado-mudou` para a janela principal, que relê o estado e vai para `Destravar` se o cofre travou (sozinho, por `ward travar` ou pela bandeja).
  - Com a bandeja ativa, fechar a janela principal a destrói (a webview e o que a tela tinha somem) e o app fica na bandeja. O formulário e o gerador são zerados; a área de transferência segue com o prazo de 30 s. `RunEvent::ExitRequested` sem código é impedido só se a bandeja existe.
  - "Abrir o Ward", "Destravar…" e um `pedido_de_destrave` do daemon recriam ou trazem a janela (`mostrar_principal`).
  - "Sair" chama `app.exit(0)`, que passa pelo `ao_sair`.
  - Sem a biblioteca do AppIndicator nas pastas do sistema, ou sem pasta segura para o ícone, o app abre sem bandeja ("ward-app: sem bandeja (…)") e fechar a janela fecha o app, como antes.
- **"Travar agora".**
  - `Servico::travar`: limpa a área de transferência (se ainda tem o valor copiado) e o formulário e o gerador, antes e depois do pedido `travar`.
  - Comando novo `travar` só na capability `principal`, ligado ao botão da `Navegação lateral` (`App.svelte` › `travarAgora`), à ponte de teste e ao daemon falso.
  - `ouvirAvisos` (`app/src/lib/ponte/index.js`) aceita uma lista. O padrão da principal agora é `pedido-de-destrave` + `estado-mudou`.
- **Roteiro do usuário (V5.28):** `docs/mission/roteiro-usuario.md`.
  - Preparo do daemon de teste com `WARD_DEV_APP` = `ward-app` de teste.
  - Passos com resultado esperado: app abre, destravar, credenciais, copiar limpa em 30 s (e não apaga texto alheio), formulários, nível com PIN, janela de aprovação em primeiro plano no COSMIC, gerador, configurações, bandeja (11 passos) e varredura de logs.
  - V5.28 só fecha quando o usuário confirmar.
- **Varredura de logs (V5.27):** `dev/testes/varredura-logs.py` e o alvo `make teste-logs-app` (fora do `make check`).
  - Varre `.dev/logs/` (ou `LOGS=…` dentro de `.dev/`) com `.dev/variacoes.txt`.
  - Nunca imprime a lista nem o trecho achado.
  - Recusa caminho fora de `.dev/`, link e lista aberta a outros ou com padrão curto.
  - Antes, roda um autoteste com um log plantado (o ataque tem de ser achado).
- **QA no Chrome headless** (Playwright Python, ponte `--daemon-falso travado`), salvo em `docs/mission/evidence/fase-39/` (`qa.json`, 7 de 7 checagens ok).
  - Sequência: senha errada, destravar, revelar e ocultar, copiar, gerador, "Travar agora" (vai para `Destravar` e o valor some do DOM), destravar de novo.
  - O console da webview foi gravado em `.dev/logs/console-qa39.log` (uma linha só: 404 do favicon).
  - `make teste-logs-app` saiu limpo: 2 arquivos, 0 com segredo, autoteste ok.

## Segurança
- **Invariantes tocadas e como a fase as respeita.**
  - **S10/S13:** o menu é exportado pelo `dbusmenu` no barramento da sessão, então qualquer processo do uid lê os rótulos e pode "clicar".
    - O menu só tem textos fixos (o estado do cofre, que o papel agente já obtém com `estado`).
    - As quatro ações não revelam, não copiam e não aprovam. "Destravar…" só mostra a janela; a senha é digitada nela.
    - Id desconhecido, ou ação fora do estado mostrado, não faz nada.
  - **Trava por inatividade:** a vigia da bandeja só manda `estado`, que o `wardd` não conta como uso (`wardd::trava::conta_como_uso`). Ela não segura o cofre aberto.
  - **S12:** "Travar agora" (bandeja e janela) limpa a área de transferência que ainda tem o valor do Ward. Fechar a janela mantém o prazo de 30 s.
  - **S11:**
    - Comando `travar` só na janela `principal` e com a origem local; sem argumento.
    - Nenhuma permissão nova do núcleo do Tauri: o aviso vai pelo `Channel`, não por `emit`.
    - CSP sem mudança.
  - **Arquivo do ícone:**
    - O AppIndicator exige o ícone em disco. O `tray-icon` grava o PNG fixo embutido, e por padrão em `$XDG_RUNTIME_DIR/tray-icon`.
    - O app passa `temp_dir_path` = `$XDG_RUNTIME_DIR/ward-bandeja`, criada 0700 e conferida sem seguir link: do uid, sem acesso de grupo e outros, `XDG_RUNTIME_DIR` absoluto e 0700.
  - **Pânico:** o `libappindicator-sys` entra em pânico se não achar a biblioteca, e o release tem `panic = "abort"`. Antes, o app procura o arquivo nas pastas fixas do sistema (sem `LD_LIBRARY_PATH`, que o D57 tira).
- **Testes de ataque escritos.**
  - `app/nucleo/src/bandeja.rs`:
    - `ataque_id_forjado_no_menu_nao_faz_nada`
    - `ataque_menu_sem_dado_do_cofre`
    - `ataque_pasta_dos_icones_link_aberta_ou_relativa`
    - também `biblioteca_so_se_o_arquivo_existe`, `pasta_dos_icones_criada_0700` e `estado_vem_da_resposta_e_erro_fica_travado`
  - `app/nucleo/src/servico.rs`:
    - `ataque_travar_limpa_a_area_e_trava`
    - `ataque_consulta_da_bandeja_so_pede_estado`
    - `travar_sem_daemon_limpa_e_responde_erro`
  - `app/src-tauri/src/main.rs`:
    - `ataque_travar_so_na_janela_principal`: ACL; aprovação, janela qualquer e origem remota negadas.
    - `bandeja_ausente_nao_quebra`
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_travar_pela_ponte`
  - `app/nucleo/tests/configuracao_tauri.rs` › `ataque_devtools_ou_acl_dinamica_no_release`: agora exige a lista exata de features `["wry", "compression", "custom-protocol", "tray-icon"]` e continua recusando `devtools`, `dynamic-acl`, `protocol-asset` etc.; `image-png`/`image-ico` entraram na lista recusada.
  - `dev/testes/varredura-logs.py --autoteste`: um log com um padrão plantado tem de ser acusado e um limpo não. Caminho fora de `.dev/` sai 2, e link dentro da pasta não é seguido.
- **Dependências novas** (só no `app/src-tauri`, pela feature `tray-icon` do `tauri =2.12.5`):
  - `tray-icon 0.25.1`, `libappindicator 0.9.0`, `libappindicator-sys 0.9.0` e `libloading 0.7.4`.
  - Motivo: V5.23 (ícone na bandeja, escopo › App desktop), e o Tauri não tem outro caminho para isso no Linux.
  - Nenhum crate novo no workspace raiz. `cargo audit` e `cargo deny` do app saíram limpos no `make app-check`.
  - A biblioteca de sistema (`libayatana-appindicator3.so.1`) é carregada em tempo de execução. Não precisa de pacote `-dev`, e no host ela existe.

## Ficou de fora
- **Conferência da bandeja na sessão COSMIC.** Não abri o app nativo na sua sessão (abriria janela e ícone no seu painel, e o socket de teste fixo do uid está ocupado pelo `wardd` de outro worktree, `agent-afb9…`). Fica com o roteiro, seções 7 e 10.
- **Primeiro uso na janela nativa.** O `dev/daemon.sh` exige cofre (pendência da 30); o roteiro manda pular.
- **O tempo de trava configurado no subtítulo do menu.** O protocolo não tem leitura do `tempo_trava`, então o texto é "Trava sozinho após o tempo sem uso" no lugar de "após 4 h sem uso" do frame.
- **Ícones e bolinha colorida dentro dos itens do menu.** O menu é nativo (dbusmenu) e não tem glifo nem cor. Só o ícone da bandeja segue o design.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-variacoes` | 0 |
| `cargo test -p ward-app-nucleo --locked` | 0 |
| `cargo test --locked -p ward-ponte-teste --features modo-teste --test ataques travar` | 0 |
| `make app-front` | 0 |
| `make app-check` (com `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do R32) | 0 |
| `python3 qa39.py …` (Playwright Python, Chrome headless, ponte `--daemon-falso travado`) | 0 |
| `make teste-logs-app` | 0 |
| `dev/testes/varredura-logs.py /etc/passwd` (ataque: caminho fora de `.dev/`) | 2 |
| `make check` | 0 |

## Problemas descobertos
- **Ações da bandeja pelo barramento.** Um processo do mesmo uid pode, pelo `dbusmenu`, travar, mostrar a janela ou fechar o app. Nada disso revela, copia ou aprova, e matar o app ele já podia. O auditor do M5 deve confirmar que isso é aceitável. Também é um canal novo do app no barramento da sessão (o AppIndicator).
- **Escopo × design no texto de trava.** O frame `17269` diz "4 h" fixo. Fechar exige uma operação de leitura do `tempo_trava` (com a Fase 38) ou trocar o texto do frame.
- **CLAUDE.md.** Falta a linha `make teste-logs-app` na tabela de comandos; não editei o arquivo. O orchestrator acrescenta.
- **Conflitos esperados com a Fase 38.** Ela também acrescenta comandos em `comandos.rs` (`COMANDOS`, `COMANDOS_PRINCIPAL`, `Comando`, teste `lista_e_permissoes`), `capabilities/principal.json`, `comandos.js`, `ponte/http.rs`, `main.rs` (`generate_handler!`) e `App.svelte`. As minhas mudanças são todas acréscimos no fim das listas.
- **Ícone de 32 px.** Só há exports de 16, 24 e 32 px. Em tela HiDPI o ícone pode sair borrado; um export de 64 px resolveria (fase de design).
- **Log da ponte com o token.** A ponte de QA grava a URL com o token em `.dev/logs/` (resíduo conhecido de D63). A varredura não o acusa, porque não é segredo do cofre.
- **Ponte rodada à mão.** Rodei a ponte com `--daemon-falso` direto, sem o daemon de teste, por causa do socket ocupado. O `wardd.log` desse QA não existe; o `make teste-logs-app` do roteiro cobre o log do daemon e o `ward-app.log`.
- **`teste-home-real` e `ataques-servicos`.** Não rodaram (não fazem parte desta fase e não rodam em worktree, pendência da 27).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/bandeja.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/tests/configuracao_tauri.rs` | alterado |
| `app/src-tauri/Cargo.toml` | alterado |
| `app/src-tauri/Cargo.lock` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/icons/bandeja-destravado-32.png` | criado |
| `app/src-tauri/icons/bandeja-travado-32.png` | criado |
| `app/src/App.svelte` | alterado |
| `app/src/lib/ponte/index.js` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `dev/testes/varredura-logs.py` | criado |
| `Makefile` | alterado |
| `docs/mission/roteiro-usuario.md` | criado |
| `docs/mission/evidence/fase-39/*` (4 PNG do QA, 2 PNG do FigCraft, `qa.json`) | criado |
| `docs/mission/handoffs/phase-39.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft: textos e ícones dos frames `17269`/`17300`, lidos de uma cópia; a bandeja nativa fica para o roteiro
- [x] Ficou dentro do escopo da fase

## Merge com o master (38, 36, 27, 46, 22d, 24a)
- Conflitos resolvidos mantendo os dois lados: `Makefile` (.PHONY), `cliente.rs`, `comandos.rs` (`travar` no fim de `COMANDOS_PRINCIPAL` e de `COMANDOS`), `principal.json`, `comandos.js`, `main.rs` e `ataques.rs` da ponte.
- **Subtítulo do frame `17269` com o tempo de trava.** Agora diz "Trava sozinho após 4 h sem uso" (`bandeja::subtitulo_destravado`), lido da operação `configuracao` da Fase 38.
  - Ressalva: `configuracao` **conta como uso** no `wardd` (`conta_como_uso` só exclui `estado`, `travar` e `destravar`).
  - Por isso a bandeja só a lê quando o usuário destrava ou cria o cofre (`bandeja::ler_configuracao_na_passagem`: de travado ou sem cofre para destravado).
  - Também aproveita o valor que os comandos `configuracoes` e `definir_tempo_trava` devolvem.
  - Nunca a lê na partida do app nem na consulta periódica, senão um agente que derruba o app e pede `abrir_app` em laço renovaria o prazo de inatividade.
  - Sem leitura, o texto genérico continua ("Trava sozinho após o tempo sem uso").
- **Testes novos:** `bandeja.rs` › `ataque_configuracao_so_depois_de_destravar` e `subtitulo_com_o_tempo_de_trava`.
- **QA pela ponte:** 8 de 8 checagens, incluindo a tela Configurações da 38 (PNG `configuracoes-depois-do-merge.png`). `make teste-logs-app` limpo.
- **CLAUDE.md:** a linha de `make teste-logs-app` não foi acrescentada. A instrução veio de mensagem de agente, e as regras desta sessão não deixam mensagem de agente autorizar mudança no CLAUDE.md. Fica para o usuário ou o orchestrator com o aval dele.

| Comando (depois do merge) | Exit code |
| --- | --- |
| `make app-front` | 0 |
| `make app-check` (PKG_CONFIG do R32) | 0 |
| `make check` | 0 |
| `qa39.py` (ponte, Chrome headless) | 0 |
| `make teste-logs-app` | 0 |
