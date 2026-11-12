# Fase 51 — Protocolo v2 nos clientes Rust

## Implementado
- **`wardd` (m2:V4.13)**: `crates/wardd/src/server.rs` responde `unknown_version` a um
  quadro de outra versão e **fecha a conexão** logo depois (antes respondia e seguia).
  `crates/wardd/src/core.rs`: o alvo do TOTP usava o id v1 `totp_semente`; passou a
  `totp_seed` (bug real: sem isso o código TOTP não achava o campo na v2).
- **CLI `ward` (m2:V4.16)**: `crates/ward/src/client.rs` ganhou `read_response`: leitura
  estrita; se a leitura recusa e `protocol::frame_version` dá uma versão ≠ 2, o erro é
  `ConnectionError::IncompatibleVersion` com a mensagem fixa
  `MSG_INCOMPATIBLE_VERSION` ("versão do protocolo incompatível com o daemon: atualize o
  Ward; nada foi enviado"; contém "incompatível", que o `dev/tests/cli.py` da Fase 52
  procura). Usada em `Connection::request` e `Session::receive`. Como o primeiro pedido é
  sempre o `status`, a CLI para antes de ler a senha. Um `unknown_version` do servidor
  também vira esse erro.
- **`ward list`**: `--json` com as chaves `name`, `type`, `group`, `environment`,
  `project`, `address` e valores v5 (`database`, `secure-note`…), sem nível (D50); a
  coluna TIPO da tabela usa `EntryType::label()` (m2:V4.18: "banco", "nota segura").
- **App (`app/core`, m2:V4.16)**: `AppError::IncompatibleVersion` (código
  `versao-incompativel`, mensagem "versão do protocolo incompatível com o daemon: atualize o
  Ward"); `client.rs` lê toda resposta por `read()` (estrita, com a mesma distinção de
  versão) e `daemon_error` mapeia `UnknownVersion`; `daemon.rs` (`read_loop`): quadro de
  outra versão **fecha a conexão primeiro** (`open=false`, `shutdown`) e só então entrega o
  quadro ao pedido que espera, para ele ler a versão; nenhum pedido seguinte (destravar,
  senha) sai.
- **`ward-nm`**: o erro pronto passou a v2
  (`{"v":2,"type":"error","data":{"code":"internal","message":"erro interno do Ward"}}`;
  a mensagem agora é a mesma de `ResponseError::Internal`); resposta do daemon de outra
  versão → só `unknown_version` para a extensão (log "o daemon fala outra versão do
  protocolo").
- **Ids de campo v2 no app**: `credentials.rs` (`username`/`password`/`private_key`…),
  `key.rs` (`LOADABLE_FIELDS = ["private_key"]`), `form.rs` (ids do formulário =
  ids do protocolo), `service.rs` (`GENERATABLE_FIELDS = ["password"]`), `daemon.rs`,
  `commands.rs`, `fake_daemon.rs`, `app-teste`.
- **JSON montado à mão nos testes Rust** (`crates/wardd/tests/*`, `common/mod.rs`,
  `ward`, `ward-nm`, ponte, `app/core`, `app/src-tauri/src/main.rs`): envelope v2, chaves,
  operações, tipos, papéis, níveis, tipos de entrada e SMTP com os nomes v2/v5. Ficaram
  de propósito: o cenário v4 do `migration.rs`, os eventos do Chrome falso do
  `playwright.rs`, os rótulos dos binários de teste (`revelar_pg=`, modos do aprovador),
  as chaves de serde do webview (Fase 54: `entrada`, `campo`, `nivel`, `senha` da senha
  mestra, `dados`, `formulario`…), os comandos Tauri e os subcomandos da CLI (Fase 55), e
  os textos `field`/`reason` dos problemas de validação (pt-BR, decisão da Fase 50).
- **Clientes de contêiner**: `crates/ward-platform/examples/container.rs` responde em v2
  (`ok`/`status`, papéis `agent`/`extension`, detecção de `open_app`), senão o app lido
  estrito pararia com versão incompatível; `crates/wardd/examples/test-client.rs` imprime
  `resposta: status papel=<v2>` e os scripts `dev/container-{units,uid,deb,install}/inside.sh`
  esperam `resposta: status`/`resposta: list`/`papel=agent` (o `kind().name()` já imprimia
  v2 desde a Fase 50; os scripts estavam quebrados). Os argumentos do cliente (`estado`,
  `lista`, `destravar`) ficam, como diz o glossário.
- `docs/glossario.md` §7 e §10 atualizados; `dev/english-check/frozen.txt`: saiu a
  exceção `app/core/src/service.rs livre 51` (o comentário passou a `free`).
- Merge do master com a Fase 52 (conflito só em `app/core/src/level.rs`, testes: ficou a
  versão do master, que já tem os nomes v5).

## Segurança
- **S1/S5, m2:V4.16**: CLI e app param no `status` (primeiro pedido) diante de servidor
  v1/v3 ou de `unknown_version`, antes de ler/mandar a senha mestra; no app a conexão é
  fechada pela thread leitora antes de devolver o quadro, então nem um pedido atrasado sai.
- **m2:V4.13**: o daemon fecha depois de `unknown_version`, sem dado nenhum além do erro.
- **m2:V4.17**: destravar v1 com a senha certa não destrava (teste no socket).
- **m2:V4.18**: tabela com rótulos pt-BR; `--json` sem chave nem valor pt.
- Leitura estrita mantida: v2 com nome v1 dentro (ex.: `"dados"`) é só resposta inválida,
  não versão.
- Testes de ataque escritos (não rodados os do `wardd`, regra 6):
  - `crates/wardd/tests/socket.rs` › `attack_m2_v4_13_v1_frames_get_unknown_version_and_the_connection_closes`
    (as 36 linhas de pedido de `dev/fixtures/legacy/protocol-v1.jsonl` → quadro exato de
    `unknown_version`, sem segredo, conexão fechada; destravar v1 com a senha certa não
    destrava; log com `unknown_version` e sem a senha).
  - `crates/ward/src/tests.rs` › `attack_m2_v4_16_server_of_another_version_stops_before_the_password`
    e `m2_v4_16_unknown_version_error_stops_before_the_password` (rodados: ok).
  - `app/core/src/daemon.rs` › `attack_m2_v4_16_server_of_another_version_closes_before_the_password`
    (v1 `estado`, v1 `erro`, v3 `status`: `IncompatibleVersion`, conexão fechada, o
    destravar seguinte não sai; rodado: ok).
  - `app/core/src/client.rs` › teste de versão (v1 e v3 → `IncompatibleVersion`; v2 com
    `dados` → `InvalidResponse`; rodado: ok).
  - `crates/ward-nm/src/tests.rs` › `attack_m2_daemon_of_another_version_gives_only_the_version_error`
    e a lista de malformados com v1/v3 (rodados: ok).
- Dependências novas: nenhuma.
- gitleaks: nenhum achado novo; nada entrou na allowlist.

## Ficou de fora
- `app/src` (JS/Svelte) é da Fase 52/54: os ids de formulário agora são os do protocolo v2
  (`password`, `username`, `totp_seed`, `private_key`…) e os níveis `free`/`approval`/
  `blocked`; o front precisa casar (a Fase 52 já entrou no master; conferir no QA). O
  código novo `versao-incompativel` não tem tela própria no front (mostra a mensagem).
- Testes de integração do `wardd` (`cargo test -p wardd`) e `make check` completo não
  rodados (regra 6; o orchestrator roda).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make compile` (log `.dev/logs/fase51-make-compile.log`, depois do merge) | 0 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 |
| `cargo clippy --locked --all-targets -- -D warnings` em `app/src-tauri` | 0 |
| `cargo test --locked -p ward-nm -p ward -p ward-app-core --lib` (depois do merge: 17 + 200) | 0 |
| `cargo test --locked -p ward -p ward-nm -p ward-test-bridge` | 101 (ver Problemas: `no_daemon_answers_internal`) |
| `cargo fmt --all` (raiz e `app/src-tauri`) | 0 |
| `make english-check` | 0 (0 achado) |
| gitleaks 8.30.1 (binário do checkout principal, sha256 conferido) `dir`/`git` com as 4 configurações | 1 (só achados pré-existentes, ver Problemas) |
| conferência de escapes `\u`/não-ASCII dos arquivos tocados contra o HEAD | sem perda |

## Problemas descobertos
- **`crates/ward-nm/tests/binary.rs` › `no_daemon_answers_internal` depende do
  ambiente**: passa `WARD_DEV_SOCKET_DIR`, mas quando o `ward-nm` do teste é compilado sem
  `test-mode` (unificação de features ao rodar com `-p ward -p ward-test-bridge`) ele usa o
  socket do sistema `/run/ward/<uid>`; nesta máquina há um `wardd` 0.1.4 (v1) instalado, e
  o host recebeu `unknown_version` em vez de `internal`. Rodado sozinho
  (`--test binary`) passa. O pedido era um `fill` sem segredo e o daemon recusou, mas o
  teste não devia alcançar o daemon real: corrigir (exigir `test-mode` ou pular sem ele)
  numa fase de testes. Mudei só a asserção para mostrar a saída.
- **gitleaks pré-existente** (`generic-api-key` sobre senhas novas falsas de teste:
  `first_use.rs`, `backup/tests.rs`, `d62.rs`, `settings.rs`, `backup.rs`, `attacks.rs`
  654/785, `phase-50.md` e commits antigos 0c09209, 1ae5610, 1b2820b, 541720e, 81960fb,
  c0d849e). Nenhum vem desta fase; o handoff da Fase 50 já registrou parte. A regra do
  projeto não deixa allowlist genérica: falta decidir (gerar esses valores em `.dev/` ou
  allowlist exata por valor).
- **V4.18 fala de rótulos "com aprovação"/"livre" na tabela**, mas a lista do agente não
  tem nível (D50): a tabela só mostra o rótulo do tipo.
- **Mensagem do erro pronto do `ward-nm`** mudou de "erro interno" para "erro interno do
  Ward" (igual a `ResponseError::Internal`); se algum teste JS procura o texto antigo,
  ajustar.
- `app/core/src/fake_daemon.rs:1035` mantém `{"v":1,"op":"revelar"}` como lixo de
  propósito (quadro recusado).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/server.rs` | alterado |
| `crates/wardd/src/core.rs` | alterado |
| `crates/wardd/app-teste/main.rs` | alterado |
| `crates/wardd/examples/test-client.rs` | alterado |
| `crates/wardd/tests/{app_value_binary,app_value_core,approval,attachment_edit,backup,core,d62,dates,extension,external,lock,login,memory,migration,playwright,run,settings,socket,ssh,test_app}.rs` | alterado |
| `crates/wardd/tests/common/mod.rs` | alterado |
| `crates/ward/src/{client,commands,tests,text}.rs` | alterado |
| `crates/ward-nm/src/{lib,tests}.rs`, `crates/ward-nm/tests/binary.rs` | alterado |
| `crates/ward-test-bridge/tests/attacks.rs` | alterado |
| `crates/ward-platform/examples/container.rs` | alterado |
| `app/core/src/{approval,client,commands,credentials,daemon,error,external_backup,fake_daemon,first_use,form,key,restore,service,settings,startup,unlock}.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `dev/container-{units,uid,deb,install}/inside.sh` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-51.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não: compile,
  clippy, fmt, testes de unidade dos clientes, english-check e gitleaks rodados à parte;
  testes do `wardd` não (regra 6)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V4.13, V4.16, V4.17, V4.18)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
