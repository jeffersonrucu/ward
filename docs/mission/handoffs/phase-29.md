# Fase 29 — `crates/ward-test-bridge` em inglês (Missão 2, Milestone 2, trilha R)

## Implementado
- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 88a9a48`.
  Fase adiantada em paralelo com as Fases 14, 30 e 47a.
- **Escopo**: o crate inteiro (o plano lista `http.rs`, `main.rs`, `simulados.rs` e
  `tests/ataques.rs`; `lib.rs`, `area_wl.rs` e `partida.rs` entraram também, porque o
  V2.1 olha o crate todo e o glossário (seção 4) já trazia os nomes deles). Só os nomes
  próprios da ponte mudaram; tudo o que vem do `ward_app_core` ficou como está no master
  (as Fases 30–34 trocam os usos).
- **Arquivos** (`git mv`, glossário seção 4): `src/area_wl.rs` → `src/wl_clipboard.rs`,
  `src/partida.rs` → `src/startup.rs`, `src/simulados.rs` → `src/simulated.rs`,
  `tests/ataques.rs` → `tests/attacks.rs`; `lib.rs` com `pub mod http, simulated, startup,
  wl_clipboard`. Comentários do `Cargo.toml` em inglês (a `description` é literal e fica).
- **Renomes** (antigo → novo):
  - `http.rs`: `Ponte` → `Bridge` (`abrir`/`url_de_abertura`/`origem`/`token_para_teste`/
    `servir`/`atender`/`responder`/`estatico`/`comando` → `open`/`opening_url`/`origin`/
    `token_for_test`/`serve`/`handle`/`respond`/`static_file`/`command`; campos
    `servico`/`origem`/`abertas` → `service`/`origin`/`open_conns`); `Pedido { metodo,
    caminho, cabecalhos, corpo }` → `Request { method, path, headers, body }` (`novo`/
    `cabecalho` → `new`/`header`); `Resposta { status, tipo, corpo }` → `Response {
    status, content_type, body }` (`texto`/`json`/`erro`/`corpo_json`/`escrever` →
    `text`/`json`/`error`/`body_json`/`write_to`); `ler_pedido` → `read_request`;
    `MAX_CABECALHO`/`MAX_CORPO`/`MAX_CONEXOES`/`PRAZO` → `MAX_HEADER`/`MAX_BODY`/
    `MAX_CONNECTIONS`/`TIMEOUT`; `iguais` → `ct_equal`, `nome_de_asset_valido` →
    `valid_asset_name`, `tipo_de` → `content_type_of`, `motivo` → `reason`; locais em
    inglês (`senha` → `password`, `troca_pin` → `pin_change`, `restauracao` → `restore`,
    `marcado` → `flag`, `so_conhecidos` → `only_known`…). Os campos do `Opcoes` do
    `app/core` (`tamanho`, `minusculas`…) ficaram, com os locais novos por
    `Opcoes { tamanho: length, … }`.
  - `startup.rs`: `SEM_PAPEL_APP` → `NO_APP_ROLE`, `conectar_com_papel_app` →
    `connect_with_app_role`, `dist_do_repositorio` → `repository_dist`, `repositorio` →
    `repository`; teste `dist_so_de_dev_absoluto` → `dist_only_from_absolute_dev`.
  - `wl_clipboard.rs`: `WlCopy::novo` → `WlCopy::new` (`WlCopy` fica, glossário seção 15),
    `binario_confiavel` → `trusted_binary`, `comando` → `command`, `TIPO` → `MIME_TYPE`,
    `_privado` → `_private`; teste `ataque_binario_fora_do_padrao_e_recusado` →
    `attack_binary_off_the_rules_is_refused`.
  - `simulated.rs`: `SeletorDePasta` → `DirPicker`, `TerminalDeTeste` → `TestTerminal`,
    `TerminalDoRepositorio` → `RepoTerminal` (`do_repositorio` → `from_repository`,
    campo `sistema` → `system`), `executavel` → `executable`; teste
    `ataque_terminal_do_repositorio_so_com_arquivos_do_repositorio` →
    `attack_repo_terminal_only_with_repository_files`.
  - `main.rs`: `USO` → `USAGE`, `com_o_daemon` → `with_daemon`, `servir`/`servir_com` →
    `serve`/`serve_with`, locais (`anexos` → `attachments`, `fila` → `queue`, `aviso` →
    `notice`, `aprovacoes` → `approvals`, `decorrido` → `elapsed`…).
  - `tests/attacks.rs`: os 21 testes com nome em inglês (`ataque_*_pela_ponte` →
    `attack_*_through_the_bridge`, `copiar_passa_e_a_resposta_nao_tem_o_valor` →
    `copy_works_and_the_response_has_no_value`, `csp_da_ponte_e_a_do_app` →
    `bridge_csp_is_the_app_one`, `tcp_so_em_127_0_0_1_com_cabecalhos_de_seguranca` →
    `tcp_only_on_127_0_0_1_with_security_headers`…); `Area` → `FakeClipboard`,
    `SEGREDO` → `SECRET`, `tem_segredo` → `has_secret`, ajudantes `ponte`/`post` →
    `bridge`/`post`, locais e closures em inglês (`pedir` → `ask`, `estado` →
    `vault_status`, `mudar` → `changes`/`set_level`…).
- **Usos fora do crate**: nenhum. Procurei os nomes antigos (`ward_test_bridge::`, os
  módulos, os tipos, os nomes de teste, `--test ataques`) em `Makefile`, `dev/`,
  `install/`, `app/` e nos outros crates: só este crate os usa. O Makefile e os scripts
  chamam só o binário `ward-test-bridge` (nome que não mudou).
- **Comentários**: todos em inglês. Os que citavam nomes de fio, textos de tela ou nomes
  do `app/core` (`/ponte/<comando>`, `dist-teste`, `abrir_app`, `estado`, "Conectar",
  "Travar agora", `SenhaMestra`, `TerminalDoSistema`, `conectar_agora`…) foram reescritos
  pelo sentido, sem o nome, para não precisar de `frozen.txt` nem de `allow.txt`. O
  comentário que citava `ponte-teste.js` › `ouvirAvisos` agora cita `test-bridge.js` ›
  `listenByPolling` (achado das Fases 35 e 38).
- **Glossário**: parágrafo "Fase 29" depois do da Fase 13 (seção 15).
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

## Literais alterados (PRD regra 5)
Multiconjunto de literais por arquivo, antigo × novo (fora de comentários):
- Capturas de `format!` (V6.3, só o nome da captura; o texto impresso é o mesmo):
  `{porta}` → `{port}`, `{USO}` → `{USAGE}`, `{ruim}` → `{bad}`, `{entrada}` → `{entry}`
  (nas duas mensagens `ward-ponte-teste: conectar …` do `simulated.rs`), e nos testes
  `{nome}`/`{corpo}`/`{mudar:?}`/`{comando}`/`{agulha}`/`{texto}`/`{recebidos:?}`/
  `{rotulo}`/`{miolo}`/`{papel}` → `{name}`/`{body}`/`{changes:?}`/`{command}`/
  `{needle}`/`{text}`/`{received:?}`/`{label}`/`{key_body}`/`{role}`.
- Caminhos temporários dos testes (nomes, regra 5): `ward-ponte-{}-{nome}` →
  `ward-bridge-{}-{name}`, `ward-ponte-wl-{}` → `ward-bridge-wl-{}`,
  `ward-ponte-terminal-{}` → `ward-bridge-terminal-{}`,
  `/run/user/{uid}/ward-ponte-papel-{}-{papel}` → `/run/user/{uid}/ward-bridge-role-{}-{role}`,
  `terminal-do-atacante` → `attacker-terminal`. Nenhum script os procura.
- **Ficaram** (fio e marca): o caminho `/ponte/` (e `"/ponte/".len()`), o texto do `USAGE`
  (`uso: ward-ponte-teste …`, as opções `--dist`/`--daemon-falso`/`--anexos` e todos os
  modos), o prefixo `ward-ponte-teste:` de todas as mensagens (o `dev/bridge.sh` procura;
  Fase 56), o nome da thread `ward-ponte`, todas as chaves e valores JSON das respostas
  e dos pedidos de teste, os códigos de erro, as mensagens dos `assert!`, o `NO_APP_ROLE`
  (`"o daemon não deu o papel app"`), a CSP, os cabeçalhos HTTP e os valores falsos de D15.

## Segurança
- **Invariantes tocadas** (PRD regra 7; diff só de nomes e comentários):
  - S11 / D7: a ponte segue só em `127.0.0.1`, token de 32 bytes do `getrandom` comparado
    em tempo constante (`ctutils::CtEq`, agora em `ct_equal`), `Host` e `Origin`
    conferidos antes de qualquer comando, `Content-Type` JSON, só comandos de
    `Comando::de_nome`, mesmos limites (`MAX_HEADER` 8 KiB, `MAX_BODY` 16 KiB,
    `MAX_CONNECTIONS` 16, prazo 5 s), mesmas recusas (`Transfer-Encoding`,
    `Content-Length` repetido/negativo, caminho com `..`/`%`/`?`/`#`/`\`, link para fora
    do `dist`). Mesma ordem dos ramos do `respond` e do `command`.
  - S8/S13: PIN, senha mestra, trocas e restauração seguem saindo do mapa por `remove`
    (posse, sem cópia) e virando os tipos que zeram; o token em `Zeroizing<String>`; a
    URL com o token só na saída padrão.
  - D63 / V3.38: `connect_with_app_role` conecta uma vez e sai com a mensagem fixa sem o
    papel app, antes de abrir a porta HTTP.
  - S2/S3 (`wl_clipboard`): binários de `/usr/bin` de dono root sem escrita de grupo e
    outros, sem link; valor pela entrada padrão; ambiente limpo. Mesma lógica.
  - `RepoTerminal::from_repository`: mesmo `WARD_DEV_DIR` limpo e mesmos arquivos
    regulares executáveis, sem seguir link.
- **Testes de ataque**: nenhum novo (só rename); os 21 de `tests/attacks.rs` e os 3 de
  unidade acompanham os nomes. Não rodaram (PRD regra 6); compilam
  (`cargo check --all-targets --all-features` e `clippy -D warnings`).
- **Dependências novas**: nenhuma.

## Ficou de fora
- Os nomes que vêm do `ward_app_core` (`Servico`, `ErroApp`, `Comando` e variantes,
  `AreaTransferencia`/`escrever`/`ler`/`limpar`, `Seletor`/`escolher`, `Terminal::abrir`,
  `TerminalDoSistema`, `DaemonFalso`, `FilaAvisos`, `Aviso`, `aprovacoes_simuladas`,
  `Opcoes { tamanho, … }`, `MAX_VALOR`, os métodos do serviço…): pedido do orchestrator;
  as Fases 30–34 os trocam aqui junto. São os 612 achados que sobram no `english-check`
  do crate (todos identificadores; nenhum comentário, nenhum nome de arquivo).
- O prefixo `ward-ponte-teste:` das mensagens, o texto do `USAGE` e o nome da thread
  `ward-ponte`: Fase 56 (o `dev/bridge.sh` procura o prefixo).
- Testes: não rodaram (PRD regra 6).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 88a9a48` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `cargo check -p ward-test-bridge --all-targets --all-features --locked` (log `.dev/logs/fase29-check.log`; a 1ª rodada acusou 2 `use` do `app/core` trocados por engano, corrigidos) | 0 |
| `cargo fmt --all --check` (log `fase29-fmt.log`) | 0 |
| `cargo clippy -p ward-test-bridge --all-targets --all-features --locked -- -D warnings` (extra; log `fase29-clippy.log`) | 0 |
| `make compile` (log `fase29-make-compile.log`; `npm ci`, Vite, `cargo check` do `app/src-tauri`, `syntax_check.py`) | 0 |
| `make english-check PATHS="crates/ward-test-bridge"` (log `fase29-english.log`) | 2 (612 achados, todos identificadores do `ward_app_core`, Fases 30–34; 0 comentário, 0 nome de arquivo, 0 problema nas exceções) |

## Problemas descobertos
- **Conflito com a Fase 30 (paralela)**: ela renomeia `Servico`/`ErroApp`/`servico::` e
  troca os usos em todo o repositório, inclusive aqui. Como os arquivos daqui mudaram de
  nome (`git mv`) e as linhas foram reescritas, o merge vai conflitar nos `use` e nas
  linhas que citam esses nomes; a resolução é aplicar os nomes da Fase 30 sobre esta
  versão. O mesmo vale para o parágrafo do glossário (as Fases 14 e 29 escrevem depois
  do da Fase 13).
- O plano lista só `http.rs`, `main.rs`, `simulados.rs` e `tests/ataques.rs`; `lib.rs`,
  `area_wl.rs` e `partida.rs` não estavam em fase nenhuma. Entraram aqui.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-test-bridge/Cargo.toml` | alterado (comentários) |
| `crates/ward-test-bridge/src/lib.rs` | alterado |
| `crates/ward-test-bridge/src/http.rs` | alterado |
| `crates/ward-test-bridge/src/main.rs` | alterado |
| `crates/ward-test-bridge/src/area_wl.rs` → `src/wl_clipboard.rs` | renomeado e alterado |
| `crates/ward-test-bridge/src/partida.rs` → `src/startup.rs` | renomeado e alterado |
| `crates/ward-test-bridge/src/simulados.rs` → `src/simulated.rs` | renomeado e alterado |
| `crates/ward-test-bridge/tests/ataques.rs` → `tests/attacks.rs` | renomeado e alterado |
| `docs/glossario.md` | alterado (parágrafo da Fase 29) |
| `docs/mission/handoffs/phase-29.md` | criado |

## Procedimentos
- [x] `make compile` rodado (a fase não roda a suíte, PRD regra 6; `fmt --check` e
  `clippy` do crate também)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto)
- [x] Teste de ataque para cada (SEG) tocado (os existentes, renomeados; lógica intocada)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase (mais `lib.rs`, `area_wl.rs` e `partida.rs`, ver
  "Problemas")
