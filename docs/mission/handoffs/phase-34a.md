# Fase 34a — Resto do Rust (Missão 2, Milestone 2)

## Implementado
- **Base**: worktree resetado para o master `1c4a963` (o worktree partia de `b4d5e1b`).
  Rodou em paralelo com a Fase 48 (corpo v5); desta fase só um comentário em
  `crates/ward-core/src/atomic_write/tests.rs` e os comentários do
  `crates/ward-core/Cargo.toml` tocam o `ward-core`. Nada em `body`/`vault` nem em
  `dev/fixtures/entries.json`.
- **Comentários dos `Cargo.toml`** (`crates/wardd`, `crates/ward-core`, `app/core`) em
  inglês; as linhas que não são comentário ficam byte a byte iguais (conferido por
  script). As `description` dos pacotes são texto (pt-BR) e ficam.
- **Binários de teste do `wardd`** (`app-teste/main.rs`, `aprovador-teste/main.rs`):
  comentários e identificadores em inglês. Pastas e nomes de binário
  (`wardd-app-teste`/`wardd-aprovador-teste`) ficam para a Fase 56 (entradas no
  `frozen.txt`). As chaves que imprimem (`papel=`, `revelar=`, `pedido operacao=…`,
  `resposta=`, `fim=1`…), o arquivo `aprovador.modo` e as palavras de modo (`confirmar`,
  `negar`, `uma_vez`, `uma_vez_ciente`, `nada`) são literais que as suítes procuram e
  ficam; a doc deixou de citá-las uma por uma e descreve pelo sentido.
- **`app/core/tests/configuracao_tauri.rs` → `tauri_config.rs`** (`git mv`; não há
  `[[test]]` com o nome nem script que rode os testes por nome), com comentários,
  locais e nomes de teste em inglês; o comentário de `dev/tests/app-release.sh` cita o
  nome novo.
- **Ligações locais pt-BR** que sobraram das Fases 17/18 (padrões de `Request`/`Response`
  com `campo: nome_pt`) viraram a forma curta com o nome do campo, em
  `app/core/src/{client,fake_daemon,service}.rs`, `crates/ward/src/{commands,tests}.rs`,
  `crates/ward-nm/src/lib.rs`, `crates/wardd/examples/test-client.rs`,
  `crates/wardd/src/{execution,ssh}.rs` e nos testes do `wardd` (`app_value_core`,
  `backup`, `core`, `extension`, `external`, `login`, `playwright`, `run`, `settings`,
  `ssh`). Antes de cada troca conferi que o nome novo não era usado no mesmo braço (sem
  captura por sombra).
- **Comentários soltos**: `Retrocesso` → `Rollback` (nome atual da variante) em
  `atomic_write/tests.rs`; `server.rs` deixou de citar o texto `UidRecusado` do log
  (o log continua igual, o `dev/container-uid/inside.sh` o procura); `tests/common/mod.rs`
  descreve as entradas do cofre de teste pelo nível em vez de citar `t-aprov`/`t-livre`/
  `t-bloq`.
- **`dev/english-check/frozen.txt`**: 5 entradas novas (Fase 56): `WARD_DEV_EXTENSAO` em
  `crates/wardd/src/server.rs`, `WARD_DEV_DBUS_SISTEMA` em `crates/wardd/src/udisks.rs`, e
  as pastas/nomes dos binários de teste (`crates/wardd/app-teste*` `teste`;
  `crates/wardd/aprovador-teste*` `aprovador` e `teste`). Nada novo no `allow.txt`.
- **Glossário**: parágrafo "Fase 34a" na seção 15 e o caminho do `tauri_config.rs` no
  parágrafo da Fase 34.

Renomes (antigo → novo):
- Testes de `tauri_config.rs`: `limites_do_gerador_na_interface_sao_os_do_rust` →
  `generator_limits_in_the_interface_are_the_rust_ones`,
  `janela_de_aprovacao_segue_o_rust_e_nao_usa_html` →
  `approval_window_follows_rust_and_does_not_use_html`,
  `ataque_capability_com_permissao_a_mais` → `attack_capability_with_extra_permission`,
  `ataque_csp_frouxa_ou_conteudo_remoto` → `attack_loose_csp_or_remote_content`,
  `ataque_devtools_ou_acl_dinamica_no_release` → `attack_devtools_or_dynamic_acl_in_release`,
  `lista_de_comandos_da_interface_e_a_mesma` → `interface_command_list_is_the_same`,
  `ataque_interface_guarda_ou_injeta_conteudo` → `attack_interface_stores_or_injects_content`,
  `fontes` → `sources` (locais `caminho`, `tela`, `arquivos`, `janela`, `comandos`,
  `esperadas`, `seg`, `proibido`, `achou`, `linha_tauri`, `modo`, `lista`, `saida` →
  `path`, `screen`, `files`, `window`, `commands`, `expected`, `sec`, `forbidden`,
  `found`, `tauri_line`, `mode_line`, `list`, `out`).
- `app-teste/main.rs`: `pedir` → `ask`, `descrever` → `describe`, `esperado` →
  `expected`, `comparar` → `compare`, `contem` → `contains` (`palheiro`/`agulha` →
  `haystack`/`needle`), `valores_do_app` → `app_values`; locais `corpo`, `tamanho`,
  `papel`, `entradas`, `nome`, `alvo`, `senha`, `rotulo`, `revelar`, `semente`, `antes`,
  `depois`, `bate`, `lista`, `itens`, `nomes` → `body`, `size`, `role`, `entries`,
  `name`, `target`, `password`, `label`, `reveal`, `seed`, `before`, `after`, `matches`,
  `list`, `items`, `names`.
- `aprovador-teste/main.rs`: `mandar` → `send`, `ler` → `read`, `modo` → `mode`; locais
  `corpo`, `tamanho`, `texto`, `linha`, `caminho`, `papel`, `escolha`, `decisao`,
  `entrada_confirmada`, `pedido` → `body`, `size`, `text`, `line`, `path`, `role`,
  `choice`, `decision`, `confirmed_entry`, `request`.
- Ligações de padrão: `codigo` → `code`, `restante` → `remaining`, `papel` → `role`,
  `dados` → `data`, `fluxo` → `stream`, `caminho` → `path`, `senha` → `password`,
  `usuario` → `username`, `entrada` → `entry`, `alvo` → `target`, `comando` →
  `command`, `janela` → `window_size`, `tipo_entrada` → `stdin_kind`, `anterior` →
  `previous`, `rascunho` → `draft`, `manter_anexos` → `keep_attachments`, `nivel` →
  `level`, `pedido` → `request`, `decisao` → `decision`, `entrada_confirmada` →
  `confirmed_entry`, `ligado` → `enabled`, `versao` → `version`, `minutos` → `minutes`,
  `atual` → `current`, `novo`/`nova` → `new`, `minutos_trava` → `lock_minutes`,
  `backup_externo` → `external_backup`, `backup_local` → `local_backup`, `travado` →
  `locked`, `cofre_existe` → `vault_exists`; no `service.rs` (testes) `vazio`/`campos` →
  `empty`/`fields`.

## Literais alterados
Só capturas de `format!`/`assert!` (o texto produzido é o mesmo; precedente da Fase 7):
- `crates/ward/src/commands.rs`: `"{caminho}"` → `"{path}"`.
- `crates/wardd/app-teste/main.rs`: `"{rotulo}={}"` → `"{label}={}"`,
  `"lista={lista}"` → `"lista={list}"`.
- `app/core/tests/tauri_config.rs`: `{nome} {valor}` → `{name} {value}` (2),
  `{arquivo}` → `{file}`, `{proibido}` → `{forbidden}` (2), `{linha_tauri}` →
  `{tauri_line}`, `{lista}` → `{list}`.

Conferência por script (scratchpad, `cmplit34a.py`): multiconjunto de literais de cada
`.rs` tocado contra `1c4a963` só difere nos itens acima (os literais com acento e os
escapes estão byte a byte iguais); nos três `Cargo.toml` as linhas sem comentário são
iguais. Sequência de tokens sem comentários (`seq34a.py`) em `execution.rs`, `ssh.rs`,
`server.rs`, `ward-nm/src/lib.rs`, `client.rs` e `commands.rs`: só nomes, formas curtas
de campo e vírgulas do rustfmt.

## Segurança
- Invariantes tocadas (só nomes e comentários): S1/S3 (`ssh.rs::prepare_ssh` do `wardd`:
  nível, token do Conectar e entrada padrão), S4/S8 (`execution.rs`: entrada padrão nula
  mata o grupo), S5 (`server.rs`: recusa por uid; só o comentário), S11
  (`tauri_config.rs`: os ataques de configuração continuam com as mesmas listas e
  padrões), D63 (`ward-nm`: papel extensão). Nenhuma conferência, ordem de passo,
  limite ou mensagem mudou (tokenizador acima).
- Testes de ataque: nenhum novo (a fase não muda comportamento nem fio); os de
  `tauri_config.rs` foram só renomeados (`attack_…`).
- Dependências novas: nenhuma.

## Ficou de fora
- Pastas `crates/wardd/app-teste/`/`aprovador-teste/` e nomes de binário: Fase 56
  (`frozen.txt`).
- `WARD_DEV_EXTENSAO`/`WARD_DEV_DBUS_SISTEMA` citados em comentário: Fase 56.
- Suíte de testes e `make check` (regra 6 do PRD); rodei `cargo clippy` do workspace e
  `cargo fmt --check`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo fmt --all` e depois `cargo fmt --all --check` | 0 |
| `make compile` (`<scratchpad>/compile-fase34a.log`) | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (`<scratchpad>/clippy-fase34a.log`) | 0 |
| `make english-check PATHS="crates app/core app/src-tauri/src"` (`<scratchpad>/english-fase34a.log`) | 0 (0 achado, 0 problema nas exceções) |
| `python3 -I dev/english_check.py` (repositório inteiro) | 1 (444 achados de outras fases; 0 problema nas exceções) |

## Problemas descobertos
- O `make english-check` do repositório inteiro ainda acusa 444 achados fora dos
  caminhos do Rust (scripts de `dev/tests/`, `dev/fixtures/entries.json` etc.), de outras
  fases.
- Os logs ficaram no scratchpad da sessão (o worktree não tem `.dev/`), com "fase34a" no
  nome.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/core/Cargo.toml` | alterado |
| `app/core/src/client.rs` | alterado |
| `app/core/src/fake_daemon.rs` | alterado |
| `app/core/src/service.rs` | alterado |
| `app/core/tests/tauri_config.rs` (era `configuracao_tauri.rs`) | renomeado e alterado |
| `crates/ward-core/Cargo.toml` | alterado |
| `crates/ward-core/src/atomic_write/tests.rs` | alterado |
| `crates/ward-nm/src/lib.rs` | alterado |
| `crates/ward/src/commands.rs` | alterado |
| `crates/ward/src/tests.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/app-teste/main.rs` | alterado |
| `crates/wardd/aprovador-teste/main.rs` | alterado |
| `crates/wardd/examples/test-client.rs` | alterado |
| `crates/wardd/src/execution.rs` | alterado |
| `crates/wardd/src/server.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/tests/{app_value_core,backup,core,extension,external,login,playwright,run,settings,ssh}.rs` | alterados |
| `crates/wardd/tests/common/mod.rs` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `dev/tests/app-release.sh` | alterado (comentário) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-34a.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não (regra 6 do PRD); `make compile` 0, clippy 0, fmt 0
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum comportamento mudou; ataques
  existentes só renomeados)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
