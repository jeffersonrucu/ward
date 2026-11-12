# Fase 66a — Porte da correção da 0.1.5 (`show_window`) e artefatos 0.2.0 refeitos

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b` (o commit a portar); com a árvore limpa,
  `git reset --hard 8c7e6c2` (master, "fase 66 concluída; fase 66a"). Sem merge nem
  cherry-pick: o `b4d5e1b` foi lido com `git show` e reescrito nos nomes do master.
- **Protocolo v2** (`crates/ward-core`): tipo de resposta novo `show_window` (`APP_ONLY`,
  sem credencial) e `Response::ShowWindow`, sem dado (`check` aceita, `write` não
  escreve campo, `read` não lê campo; o leitor estrito recusa qualquer campo).
- **Daemon** (`crates/wardd/src/approval.rs`): `open` passa a usar
  `open_via_launcher`; `Engine::open_app` com o lançador dizendo `AlreadyOpen` chama
  `Engine::show_window`, que entrega `ShowWindow` só ao assinante (a conexão do papel
  app, pelo `SocketSubscriber`, que codifica com `Role::App`); entrega que falha tira o
  assinante, como nos outros avisos. Quem pediu o `open_app` recebe só `ok`.
  `Unavailable` continua `approval_unavailable`.
- **Núcleo do app** (`app/core/src/daemon.rs`): a leitora marca `Queue::show` ao
  receber `ShowWindow` (vários seguidos são um) fora da fila de avisos;
  `NoticeQueue::wait_show(prazo)` espera e tira a marca.
- **App Tauri** (`app/src-tauri/src/main.rs`, `watch_notices`): thread `ward-app-show`
  espera `wait_show` e chama `show_main` na thread principal (desminimiza, mostra e
  dá foco, ou recria a janela).
- **Versão**: segue 0.2.0 (as mudanças de versão do `b4d5e1b` não foram portadas).
- **Glossário**: `mostrar_janela` (0.1.5) → `show_window` na tabela de tipos de
  resposta (seção 10), `MostrarJanela` → `ShowWindow` (seção 15) e parágrafo da Fase 66a
  com os nomes de funções (`open_via_launcher`, `Engine::show_window`, `request_window`,
  `wait_show`, thread `ward-app-show`).
- **Fixture de fio**: `protocol-v2.jsonl` é a tradução linha a linha da 0.1.4 e fica
  como está; `wire_v2::v2_responses_read_and_written_byte_for_byte` exclui só
  `show_window` da exigência "todo tipo tem linha", com comentário apontando o teste que
  confere o quadro byte a byte.
- **Docs**: `docs/mission/prd.md` (DT7): a 0.1.x recusa o v5 com `cofre_invalido` ("o
  conteúdo do cofre é inválido") com `ward-estado 1` e como retrocesso com
  `ward-state 2`. `docs/mission/roteiro-usuario.md`: sem o aviso de não instalar, diz
  que a 0.2.0 traz a correção da 0.1.5, tabela com os sha256 novos e, no passo 7.1,
  conferir que abrir pelo menu com o app na bandeja (e minimizado) mostra a janela.
- **Artefatos 0.2.0 refeitos** (como na 66, sem `sudo`): `make release`, `make
  extension` (id `nhkehjnmlgkebhmelajfpmpblepaichf`), `make extension-crx` (versão
  0.2.0), `make deb EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf
  EXTENSION_CRX=…/target/extension/ward.crx`. Conferência: `Package: ward`,
  `Version: 0.2.0`, `Architecture: amd64`, `Depends: sshpass, openssh-client, systemd,
  libwebkit2gtk-4.1-0`; 61 linhas no `dpkg-deb -c`, todas `root/root`, nenhum modo com
  `s`/`S`; os binários e o `.crx` do staging têm os mesmos sha256 dos artefatos;
  `strings` dos 4 binários sem `WARD_DEV_`, `ward-test-bridge`, `wardd-test` nem
  `ward-teste` (0 em cada; `app-release.sh` passou) e com `show_window` (1 em cada);
  `ward --version` = `ward 0.2.0`.
- **Cópia para o usuário** em `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/`
  (substituiu os da 66), `SHA256SUMS` novo, `sha256sum -c`: todos SUCESSO.

  | Artefato | sha256 |
  | --- | --- |
  | `ward_0.2.0_amd64.deb` | `9ff2397fc0813b0ea92c92bc50eebd57680039e5eec720015fbf774f60ddc0c2` |
  | `ward.crx` | `bd6e92f180ef90d5fb57ce33ba108ce9bac4c19ab75de2fb7b04c3b2e09fc9c0` |
  | `bin/ward` | `994f8d29eaabe5e1eae5b6bdabd925eae9f2736621759c735cb6e4bc886bd70b` |
  | `bin/wardd` | `d6bcb03dc0d5cea085a38951ddc9062f250ac37ddd3e250d7126742692cd2190` |
  | `bin/ward-nm` | `c073f81c88cae15bff34b3491772e9f5b1048543822b2b2c78ebef324cd76e97` |
  | `bin/ward-app` | `54ef9d199b7016eec6af469911760c9d744bbc74fd4cf74bdb36c371fa6dd004` |

## Segurança

- Invariantes: S1/S9 (matriz tipo × papel): `show_window` é só do app e não carrega
  credencial; a linha nova entrou na tabela do V3.15
  (`protocol/tests.rs › v3_15_operation_role_table`). Nenhum dado sai: o quadro é
  exatamente `{"v":2,"type":"show_window","data":{}}`. O `open_app` não ganhou campo
  (D57): nenhum chamador escolhe o destino; o destino é sempre o assinante, que só pode
  ser a conexão do papel app (`server.rs`, `role == Role::App`).
- Testes de ataque:
  - `crates/ward-core/src/protocol/tests.rs › attack_show_window_app_only_and_without_data`:
    agente e extensão não escrevem nem leem o quadro; o quadro escrito é o vazio exato;
    quadro com `pid`, `window`, `entry`, senha em base64, `data` não objeto, o nome
    `mostrar_janela` em v2 e o quadro v1 da 0.1.5 são recusados.
  - `crates/wardd/tests/approval.rs › attack_show_window_only_to_the_subscribed_app`:
    sem assinante nada é guardado para depois; de cada papel (agente, app, extensão)
    o chamador recebe só `ok` e o assinante recebe um `ShowWindow` por pedido; `open_app`
    com `pid`/`window`/`id`/`binario`/`entry` vindo do agente ou da extensão dá
    `invalid_request` e nada chega ao app; assinante novo toma o lugar e o antigo não
    recebe; entrega que falha tira o assinante.
  - `crates/wardd/src/server.rs › tests` (entrega pelo `SocketSubscriber`): o quadro no
    socket é o vazio e agente/extensão não o leem.
  - `app/core/src/daemon.rs › attack_show_window_with_data_closes_without_showing`:
    `show_window` forjado com dado ou com o nome 0.1.5 fecha a conexão e não marca o
    pedido de janela.
  - Traduzidos do `b4d5e1b`: `wardd/tests/approval.rs › d57_open_app` (já aberto: o
    assinante recebe `ShowWindow`; aberto agora: nada) e
    `app/core/src/daemon.rs › show_window_marks_the_request_outside_the_queue`.
- Dependências novas: nenhuma.

## Ficou de fora

- Conferir a janela de verdade (abrir pelo menu com o app na bandeja) no app instalado:
  exige instalar; é do usuário (passo 7.1 do roteiro).
- `make test-deb`/`test-install` com os artefatos novos: exigem Docker e a linha de
  base; são dos validadores (como na 66).
- `make app-check` (clippy do `app/src-tauri`): não rodado; o `make compile` fez o
  `cargo check` do `app/src-tauri` e o `make release` compilou o app sem aviso.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 8c7e6c2` | 0 |
| `cargo test -p ward-core` / `-p wardd --test approval` / `-p ward-app-core show_window` | 0 / 0 / 0 |
| `make fmt` | 0 |
| `make check` | 0 ("tudo limpo") |
| `make compile` | 0 |
| `make english-check` | 0 (0 achados) |
| `df -h /` antes do release | 0 (15 GB livres) |
| `make release` | 0 |
| `make extension` | 0 |
| `make extension-crx` | 0 |
| `make deb EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSION_CRX=…/ward.crx` | 0 |
| `dpkg-deb -f/-c`, `strings`, `sha256sum` (conferência) | 0 |
| `sha256sum -c SHA256SUMS` em `target/release-0.2.0/` | 0 |

## Problemas descobertos

1. O `ward.crx` mudou de sha256 (66: `c4187c…`; 66a: `bd6e92…`) sem mudança na
   extensão: a assinatura/empacotamento do Chrome não é reprodutível byte a byte. Só
   registro; o id e a versão conferem.
2. O `b4d5e1b` também subia a versão para 0.1.5 e acrescentava a linha 0.1.5 no
   metainfo; não portado (a fase pede 0.2.0). O
   `install/desktop/local.ward.app.metainfo.xml` termina na 0.1.4: não tem a 0.1.5 nem
   a própria 0.2.0 (a janela "Sobre"/central de software mostraria 0.1.4 como a última).
   Fora do escopo; sugestão: acrescentar `<release version="0.2.0" …/>` (e a 0.1.5)
   antes de publicar, e refazer o `.deb`.
3. `target/release-0.2.0/` continua 0755 (Problema 4 da 66); `bin/` 0700.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocol.rs` | alterado |
| `crates/ward-core/src/protocol/response.rs` | alterado |
| `crates/ward-core/src/protocol/tests.rs` | alterado |
| `crates/ward-core/src/wire_v2.rs` | alterado |
| `crates/wardd/src/approval.rs` | alterado |
| `crates/wardd/src/server.rs` | alterado |
| `crates/wardd/tests/approval.rs` | alterado |
| `app/core/src/daemon.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/roteiro-usuario.md` | alterado |
| `docs/mission/handoffs/phase-66a.md` | criado |
| `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/` (fora do git) | substituído |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica (sem tela nova)
- [x] Ficou dentro do escopo da fase
