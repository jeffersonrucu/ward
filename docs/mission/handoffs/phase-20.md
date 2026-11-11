# Fase 20 — `wardd`: `core`, `approval` (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 93e49b3`
  (master de então). No fim, merge do master `cd5a330` (Fase 34) sem conflito.
- **Arquivos** (glossário seção 4, `git mv`): `src/nucleo.rs` → `src/core.rs`,
  `src/aprovacao.rs` → `src/approval.rs`, os dois traduzidos por inteiro (comentários,
  locais, parâmetros, campos). `lib.rs`: `pub mod core`/`pub mod approval` e os links da
  doc (`[`core`](crate::core)`, [`approval`]). O módulo `core` convive com o crate `core`
  da biblioteca: só a raiz do crate veria os dois, e lá ninguém usa `core::`.
- **Renomes (antigo → novo)**: lista completa no parágrafo "Fase 20" do glossário (seção
  15, antes de "Funções e métodos…"). Os principais:
  - `approval`: `Motor` → `Engine` (`pedir_aprovacao`/`esperar_destrave`/`decidir`/
    `assinar`/`desassinar`/`pendente`/`e_confirmacao`/`e_entrada_oculta`/
    `cancelar_pendentes` → `request_approval`/`wait_for_unlock`/`decide`/`subscribe`/
    `unsubscribe`/`pending`/`is_confirmation`/`is_hidden_stdin`/`cancel_pending`),
    `RelogioMotor`/`RelogioMonotonico` → `EngineClock`/`MonotonicClock`, `AbridorApp` →
    `AppOpener` (`abrir` → `open`), `ErroAbrirApp { JaAberto, Indisponivel }` →
    `OpenAppError { AlreadyOpen, Unavailable }`, `Assinante` → `Subscriber` (`entregar` →
    `deliver`), `Cliente` → `Client` (`cadeia`/`saiu` → `chain`/`exited`), `SemApp`/
    `SemCliente` → `NoApp`/`NoClient`, `PRAZO_APROVACAO`/`PRAZO_DESTRAVE`/`UMA_HORA` →
    `APPROVAL_DEADLINE`/`UNLOCK_DEADLINE`/`ONE_HOUR`.
  - `core`: `Relogio`/`RelogioReal` → `Clock`/`RealClock` (`dormir`/`agora`/
    `segundos_unix` → `sleep`/`now`/`unix_seconds`), `Caminhos { cofre, estado }` → `Paths
    { vault, state }`, `Aberto` → `Open` (`entradas`/`liberacoes` → `entries`/`grants`),
    `Estado` → `State` (`aberto` → `open`…), `SessoesRun` → `RunSessions`, `Gancho` →
    `Hook`, `MAX_ERROS_PIN`/`ESPERA_MAXIMA`/`espera_destrave` → `MAX_PIN_ERRORS`/
    `MAX_WAIT`/`unlock_wait`; `Daemon::novo`/`atender`/`atender_de`/`travar`/`travado`/
    `travar_se_inativo`/`conferir_pin`/`gravar_ou_desfazer`/`preencher`/`motor`/`com_motor`/
    `com_temporarios`/`com_pendrives`/`sincronizar_externo`/`ligar_copia_externa`/
    `cutucar_externo` → `new`/`serve`/`serve_from`/`lock`/`is_locked`/`lock_if_idle`/
    `verify_pin`/`write_or_undo`/`fill`/`engine`/`with_engine`/`with_temp_root`/
    `with_usb_drives`/`sync_external`/`enable_external_copy`/`poke_external`; campo
    `aprovacao` → `approval`; ganchos `com_chrome`/`com_gancho_depois_de_gravar`/
    `com_prazos_login`/`com_programas_ssh` → `with_chrome`/`with_hook_after_write`/
    `with_login_deadlines`/`with_ssh_programs`.
  - "liberação de 1 hora" virou `grant` (como o `without_grant` da Fase 19), não o `release`
    da seção 2 do glossário; "vez" de cada papel virou `turn`. Os dois estão no parágrafo.
- **Usos ajustados** pelo método dos diagnósticos do `cargo check --workspace --all-targets
  --all-features` (script que troca o nome antigo só no span do erro; depois `-p wardd --lib
  --bins` sem features): `wardd/src/{config,execution,launch,lock,login,main,playwright,
  server,ssh,udisks,usage}.rs` e os testes `app_value_core`, `approval`, `attachment_edit`,
  `backup`, `common/mod`, `core`, `d62`, `dates`, `extension`, `external`, `lock`, `login`,
  `memory`, `playwright`, `run`, `settings`, `socket`, `ssh`. Fora do `wardd` nenhum uso
  (o `app/core/src/destrave.rs` citava `PRAZO_DESTRAVE` num comentário; a Fase 34 já o
  reescreveu). Comentários com nomes meus: `Cargo.toml` do `wardd` (os dois ganchos),
  `udisks.rs` (link `Daemon::sync_external`), `tests/playwright.rs` (`Daemon::with_chrome`).
- **V3.15** (`tests/approval.rs`): `"nucleo.rs"` (×7) → `"core.rs"`, `"aprovacao.rs"` →
  `"approval.rs"`, `TEST_HOOKS` com os nomes novos (continua em ordem alfabética), e os
  casos de ataque com `with_ssh_programs`/`with_hook_after_write`. Sem isso o teste deixaria
  de cobrir os arquivos renomeados.
- **Comentários com nomes de fio**: reescritos em inglês descritivo, como na Fase 19
  (`vista`/`queimada` → `seen`/`burned`; `revelar`/`copiar`/`sugerir`/`preencher`/
  `salvar_login`, `travado`, `entrega_falhou`, `pendrives_indisponiveis`,
  `origem_diferente`, `restante`, `abrir_app`, `ward destravar` → a descrição da operação ou
  do código). O link quebrado `[`ErroResposta::CofreMudou`]` virou
  `[`ResponseError::VaultChanged`]`. Nada entrou em `frozen.txt` nem em `allow.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo (léxico próprio, fora de comentários) contra o master:
- Só capturas de `format!` (valor igual), em `core.rs`: `{mortos}` → `{killed}`,
  `{falhas_agente}` → `{agent_failures}`, `{minutos}` → `{minutes}`, `{nome}` → `{name}`,
  `{primeira}` → `{first}`, `{MAX_ERROS_PIN}` → `{MAX_PIN_ERRORS}` (este está em
  `dev/fixtures/legacy/strings-0.1.4.txt`; o V6.3 aceita diferença só de nome de captura).
- `tests/approval.rs` (V3.15): nomes de arquivo e de gancho acima.
- **Ficaram**: todas as mensagens de log (as que os scripts procuram), os textos de erro
  (`SHORT_PASSWORD` igual), os campos e nomes do protocolo e os nomes de arquivo no disco.
  Nos outros arquivos tocados nenhum literal mudou. `approval.rs` e `core.rs` não têm
  escape `\u`.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S9/S10/D2/D16/D21/D57 (`approval`): mesmo prazo de 2 min por relógio injetado, "fechar
    nega", app que sai sem receber dá "aprovação indisponível", cliente que cai cancela,
    nova assinatura reentrega o não respondido, espera do destrave em separado.
  - S8/S9/D20/D39/D53/D62 (`core`): trava zera e mata as sessões e órfãos, 5 erros de PIN
    travam, espera do destravar por papel com contador do agente no disco antes da resposta,
    gerações queimadas antes de gravar, identidade conferida, restaurar com PIN e
    cancelamento dos pendentes, revelar/copiar/totp só app, sugerir/preencher só extensão
    e por origem.
  - V3.15: os ganchos continuam só com `test-hooks` e só no `core.rs`; a lista acompanha.
- **Conferência**: (1) multiconjunto de literais (acima); (2) sequência de tokens de
  `core.rs`/`approval.rs` com todo identificador trocado por `ID`, sem comentários,
  normalizando o atalho de campo (`x: x` → `x`), vírgula final e chaves de braço de `match`
  que o `rustfmt` mexeu: idêntica à do master fora das capturas acima; (3) com os
  identificadores, o mapa antigo → novo é uma função (as exceções são nomes do protocolo que
  ficam, como `Resposta::Estado { travado }`, e homônimos legítimos, como
  `FalhaGravar::Resposta` → `WriteFailure::Response`, outro tipo); (4) a mesma conferência
  nos outros arquivos do `wardd` tocados mostra só os meus nomes e a reordenação de `use`
  do `rustfmt`; (5) `cargo clippy -p wardd -D warnings` com todas as features e sem
  features (release).
- **Testes de ataque**: nenhum novo (só rename); os existentes acompanham os nomes. Não
  rodaram (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Identificadores da Fase 18 que o checador ainda acusa em `core.rs`/`approval.rs` (146
  achados, todos de `ward-core::protocol`: `Resposta` e variantes/campos, `PedidoDeAprovacao`,
  `PedidoDeDestrave`, `MAX_RESTANTE_DESTRAVE`, `ItemLista`, `DetalheEntrada`, `Sugestao`,
  `VersaoBackup`, `Pendrive`…). Nenhum achado de comentário.
- Comentários pt-BR que já existiam e não são da fase: `crates/wardd/Cargo.toml` (só os dois
  nomes de gancho mudaram), `app-teste/`, `aprovador-teste/`, alguns de `server.rs`,
  `udisks.rs` e `tests/common/mod.rs`.
- `make check` e testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 93e49b3` (worktree nasceu em `b4d5e1b`) | 0 |
| `cargo check --workspace --all-targets --all-features --locked` em laço, com troca no span dos erros | 0 no fim |
| `cargo check -p wardd --lib --bins --locked` (sem features) em laço | 0 no fim |
| `cargo fmt -p wardd`; `cargo fmt --all -- --check` (log `.dev/logs/fase20-fmt.log`) | 0 |
| `cargo clippy -p wardd --all-targets --all-features --locked -- -D warnings` (`fase20-clippy.log`) | 0 |
| `cargo clippy -p wardd --lib --bins --locked -- -D warnings` (`fase20-clippy-release.log`) | 0 |
| `make compile` (`fase20-make-compile.log`; 0 warnings) | 0 |
| `git merge master` (`cd5a330`, Fase 34; sem conflito) | 0 |
| `make compile` depois do merge (`fase20-make-compile-merge.log`; 0 warnings) | 0 |
| `python3 -I dev/english_check.py --paths crates/wardd/src/{core,approval,lib}.rs` (`fase20-english-check.log`) | 1 (146 identificadores da Fase 18) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos
- O worktree nasceu em `b4d5e1b`, não no master; reset com a árvore limpa.
- **Merge com a Fase 18**: o `git mv` com reescrita vira apagar+criar, então quem fizer
  merge da Fase 18 (que troca `Resposta` → `Response` etc. em `wardd/src/nucleo.rs` e
  `aprovacao.rs`) vai ver modify/delete nesses dois arquivos: levar só os usos dos nomes do
  protocolo para `core.rs`/`approval.rs`. Cuidado: `FalhaGravar::Resposta` virou
  `WriteFailure::Response` (outro tipo, não o do protocolo); e os comentários que citavam
  `Resposta::Segredo`/`MAX_RESTANTE_DESTRAVE` já foram reescritos sem o nome.
- O `rustdoc` não foi rodado; os links da doc trocados (`crate::core`, `Engine::subscribe`,
  `Daemon::serve`, `Daemon::sync_external`…) existem, conferidos à mão.
- A ferramenta de escrita decodifica `\uXXXX`; nenhum dos arquivos tocados tinha (a
  comparação de literais confirma).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/core.rs` (era `nucleo.rs`) | renomeado e reescrito |
| `crates/wardd/src/approval.rs` (era `aprovacao.rs`) | renomeado e reescrito |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/{config,execution,launch,lock,login,main,playwright,server,ssh,udisks,usage}.rs` | alterado (usos) |
| `crates/wardd/tests/{app_value_core,approval,attachment_edit,backup,core,d62,dates,extension,external,lock,login,memory,playwright,run,settings,socket,ssh}.rs`, `tests/common/mod.rs` | alterado (usos; V3.15) |
| `crates/wardd/Cargo.toml` | alterado (nomes dos ganchos no comentário) |
| `docs/glossario.md` | alterado (parágrafo "Fase 20") |
| `docs/mission/handoffs/phase-20.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não, por PRD regra 6; rodou `make compile` (0), `cargo fmt` e `cargo clippy -p wardd -D warnings` com e sem features (0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado — os existentes acompanham os nomes; a lógica não mudou
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
