# Fase 50 — Protocolo v2 no `ward-core` (DT8)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 485f888`
  (master da época, Fase 49 concluída).
- **`crates/ward-core/src/protocol.rs`**:
  - `VERSION` 1 → 2;
  - tabelas `Operation` e `ResponseType` com os nomes v2 de DT8 (glossário, seção 10). Os
    papéis de cada linha não mudaram (conferido no diff: só o primeiro literal de cada linha);
  - papéis `agente`/`extensao` → `agent`/`extension`;
  - envelope `dados`/`tipo` → `data`/`type`;
  - documentação do módulo com o envelope v2 e a regra de nome antigo;
  - função nova `frame_version(bytes) -> Option<u64>`: a versão `v` de um quadro do daemon
    que o `decode_response` recusou. Serve para a CLI e o app pararem com a mensagem de
    versão antes de mandar a senha (V4.16, Fase 51). Lê só o JSON e a chave `v`.
- **`protocol/errors.rs`**:
  - os 33 códigos pelos nomes v2 (`unauthorized`, `locked`, …, `validation`, `internal`);
  - a mensagem de `UnknownVersion` passa a "versão do protocolo incompatível: atualize o
    Ward" (texto de DT8);
  - as outras mensagens pt-BR não mudaram. "travado" chegou a ser trocado pelo script, mas
    voltou: é texto exibido, não código.
- **`protocol/request.rs`**:
  - chaves de todos os pedidos pela tabela de campos, com `janela` (campo do `ssh`) →
    `window_size`;
  - valores de `stdin_kind` (`null`, `file`, `other`), `decision` (`deny`, `once`,
    `one_hour`, `always`, `confirm`) e `result` (`submitted`, `refused`, `no_form`,
    `tab_changed`, `failure`);
  - identificadores de campo do alvo de revelar/copiar e do rascunho (`password`,
    `totp_seed`, `private_key`, `text`, `username`, `engine`, `port`, `db_name`, `tunnel`,
    `jump`, `variable`, `region`, `security`, `protocol`); `SECRET_FIELDS` com eles.
- **`protocol/response.rs`**:
  - chaves de todas as respostas;
  - `Stream` (`stdout`/`stderr`);
  - `TEXT_FIELDS` com os nomes v2, que passou a `pub(super)` para os testes do protocolo
    listarem os nomes.
- **Fixture novo `dev/fixtures/protocol-v2.jsonl`** (91 linhas, sha256 `0d3c3969…b478`):
  - é o `protocol-v1.jsonl` com cada nome traduzido, `v` = 2, os papéis das linhas em
    inglês e a mensagem nova de `unknown_version`;
  - gerado uma vez por um script Python fora do repositório, com tabelas escritas à mão a
    partir do glossário e independentes do código Rust;
  - o script confere que o próprio serializador reproduz byte a byte cada linha v1 antes de
    traduzir, e nenhum nome ficou fora das tabelas;
  - os valores (base64 dos segredos falsos, textos, números) são os mesmos;
  - o `protocol-v1.jsonl` não mudou.
- **Testes** (escritos; os de unidade do protocolo e de fio rodaram, ver Comandos):
  - **`src/wire_v2.rs`** (novo; `mod wire_v2` em `lib.rs`): byte a byte contra o fixture
    novo:
    - pedidos (uma linha por operação, na ordem da tabela), respostas (23, todos os tipos)
      e erros (`SIMPLE`);
    - o fixture v2 tem as mesmas linhas e os mesmos tipos do v1;
    - papéis das linhas pela tabela;
    - ataques: operação renomeada → `unknown_operation`; `entry` → `entrada` e `data` →
      `dados` → `invalid_request`; quadro v2 rotulado `v:1` → `unknown_version` para todo
      papel, e resposta v1 recusada com `frame_version` = 1.
  - **`src/wire_v1.rs`**: o fixture v1 virou o teste de recusa (V4.13):
    - `dt8_v1_requests_answer_unknown_version`: cada pedido v1, com cada papel, inclusive
      o da linha;
    - `dt8_v1_responses_refused_by_the_v2_client`: cada resposta e erro v1 recusados por
      todo papel, `frame_version` = `Some(1)`, e `None` sem JSON ou sem `v`;
    - `dt8_unknown_version_error_has_no_data`: o quadro do erro é exatamente
      `{"v":2,"type":"error","data":{"code":"unknown_version","message":"…"}}`, igual para
      os três papéis;
    - os papéis v1 da linha saem de uma tabela à mão (`v1_role`);
    - o `attack_renamed_wire_name_is_refused` foi para o `wire_v2`;
    - os testes do cofre e do backup 0.1.4 não mudaram.
  - **`protocol/tests.rs`**:
    - todos os quadros e expectativas na v2, por script com contexto: chave pela tabela,
      valor pela chave da frente, e literal solto conferido um a um;
    - de propósito, ficaram em pt-BR: o `field`/`reason` dos problemas de validação (texto
      exibido), o projeto `portal "novo"`, o extra `usuario` (dado) e a mensagem
      "travado";
    - seção nova "m2:DT8": `dt8_attack_v1_field_name_in_v2_request_refused`,
      `dt8_attack_v1_operation_names_refused`, `dt8_attack_v1_values_refused`,
      `dt8_attack_v1_names_in_v2_response_refused`, `dt8_no_portuguese_name_in_protocol_v2`
      e `dt8_role_matrix_same_as_v1`;
    - nos testes antigos: `envelope_refused` com v1, v3 e `{"v":1,"op":"lista"}` →
      `unknown_version`; campos inventados também com `"reveal":true` e
      `"generation":1`; datas também com `created_at`/`modified_at`/`dates`; `stdin_kind`
      recusa `nula`/`arquivo`/`outra`/`Null`; `unlock_request` recusa `executar`; a
      conferência "nenhuma operação de aprovar/nível/token do agente" procura `approv`/
      `level`/`token`.
  - **`tests/body_memory.rs`**: procura a chave `"password":"` no quadro do destrave, no
    lugar de `"senha":"`.
- **`dev/english-check/frozen.txt`**: saíram as 5 entradas da Fase 50 (`dados`, `tipo`,
  `erro`, `codigo`, `mensagem` do doc de `protocol.rs`) e o comentário da seção.
- **`docs/glossario.md`**, seção 10: diz que o protocolo v2 vale no `ward-core` desde esta
  fase e registra `frame_version`, `wire_v2.rs`, `protocol-v2.jsonl`, a mensagem nova e
  `ambiente` (lista de variáveis do `run`) → `environment`.
- **`.gitleaks.toml`**: entraram as formas base64 exatas de dois valores falsos de D15,
  casadas inteiras (ver Problemas).

## Literais alterados (PRD regra 5)

- Os literais de fio de `protocol.rs`, `errors.rs`, `request.rs` e `response.rs` foram
  trocados por script, só literal inteiro `"[a-z_]+"` fora de comentário. O script
  imprimiu cada troca (445 linhas) e eu as conferi pela tabela.
- Fora dos nomes de fio, só mudaram a mensagem de `UnknownVersion` (DT8) e o literal de
  `body_memory.rs`.
- Conferi por script os escapes (`\u`, `\x`, `\0`, `\n`, `\r`, `\t`) e os caracteres não
  ASCII de todos os arquivos tocados contra o `HEAD`: nada se perdeu. Os testes e o fixture
  só foram editados por Python, e o `wire_v2.rs` novo não tem `\u`.

## Segurança

- **Invariantes tocadas**:
  - S1/S9/D5: a matriz operação × papel e tipo × papel é a mesma. `v3_15_*` foi conferido
    só nos nomes, e `dt8_role_matrix_same_as_v1` compara com a matriz da 0.1.4 escrita à
    mão.
  - A asserção de compilação (nenhuma resposta com credencial para o agente) não mudou.
  - A ordem de leitura continua JSON → versão → operação → papel → campos. Um quadro v1 dá
    `unknown_version` antes de olhar operação, papel ou campos; a resposta não tem dado
    nenhum além do código e da mensagem fixa.
  - Leitura estrita: um nome v1 num quadro v2 é campo desconhecido e recusa com
    `invalid_request` (chave do envelope, chave de dado, ou os dois juntos). Valor v1 (tipo
    de entrada, decisão, resultado, nível, tipo, alvo) também recusa; operação v1 dá
    `unknown_operation`.
  - Um `run` sem `stdin_kind`, ou com `tipo_entrada`, recusa e nunca vira terminal (RA11).
  - O lado cliente recusa resposta com nome v1.
  - Limites, `Zeroizing`, base64 e JSON estrito não mudaram: `json.rs`, `fields.rs` e
    `frame.rs` estão intocados.
- **Testes de ataque** (todos em `crates/ward-core/src/`):
  - V4.13 (S1, RM3): `wire_v1.rs` › `dt8_v1_requests_answer_unknown_version`,
    `dt8_v1_responses_refused_by_the_v2_client` e `dt8_unknown_version_error_has_no_data`;
    `wire_v2.rs` › `attack_v2_frame_labelled_1_is_unknown_version`;
  - V4.14 (RM1, RA11): `protocol/tests.rs` › `dt8_attack_v1_field_name_in_v2_request_refused`,
    `dt8_attack_v1_operation_names_refused`, `dt8_attack_v1_values_refused`,
    `dt8_attack_v1_names_in_v2_response_refused` e `attack_stdin_kind_out_of_format`;
    `wire_v2.rs` › `attack_renamed_wire_name_is_refused`;
  - V4.15 (S1, S9): `protocol/tests.rs` › `dt8_no_portuguese_name_in_protocol_v2` lê
    `dev/english-check/pt-words.txt` com as regras do checador para identificador e tem
    como controle os nomes v1, que a mesma regra acusa;
    `protocol/tests.rs` › `dt8_role_matrix_same_as_v1`, `v3_15_operation_role_table`;
    `wire_v2.rs` › `v2_fixture_roles_follow_the_table`.
- **Dependências novas**: nenhuma.

## Ficou de fora

- **Clientes (Fases 51–52)**. O workspace compila, porque nenhuma API sumiu, mas os
  clientes ainda falam v1 no fio e quebram em execução até essas fases:
  - JSON montado à mão nos testes Rust de `wardd`, `ward`, `ward-nm`, ponte e `app/core`;
  - nomes de campo usados como identificador no `app/core` (`credentials.rs`: `"usuario"`,
    `"senha"`, `"chave_privada"`…; `key.rs`: `LOADABLE_FIELDS`), que agora chegam como
    `username`, `password`, `private_key`;
  - a resposta pronta de erro interno do `ward-nm` (`{"v":1,"tipo":"erro",…}`);
  - o `dev/tests/socket_matrix.py`, que lê a tabela de `protocol.rs` e passa a receber os
    nomes v2;
  - extensão, `app/src` e `dev/tests/*.py`.
  - `rg -n '"v":1|"dados"|"tipo"' crates app dev extension` acha a maior parte.
- **V4.13 no daemon (Fase 51)**: o `wardd/src/server.rs` responde ao erro de decodificação
  e **continua** lendo a conexão. O contrato pede fechar depois de `unknown_version`.
- **V4.16 (Fase 51)**: a CLI e o app devem chamar `protocol::frame_version` quando o
  `decode_response` da primeira resposta (`status`) falhar, e parar antes da senha se for
  ≠ `VERSION`.
- Não rodei a bateria inteira nem o `tests/body_memory.rs` (PRD regra 6).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 485f888` (árvore limpa) | 0 |
| `python3 -I <scratchpad>/fase50/make_v2_fixture.py …` (gera o fixture; 91 linhas, nenhum nome sem tabela) | 0 |
| `cargo test -p ward-core --lib --locked -- protocol wire` (log `.dev/logs/fase50-test-protocol.log`): 72 passaram, inclusive os 24 de fio e os 6 `dt8_*` novos | 0 |
| `cargo clippy -p ward-core --all-targets --all-features --locked -- -D warnings` (log `fase50-clippy.log`) | 0 |
| `cargo fmt --all` / `cargo fmt --all -- --check` | 0 / 0 |
| `make compile` (log `fase50-make-compile.log`; workspace inteiro, `app/src-tauri`, Vite, sintaxe 48 shell/35 Python/7 JS, 0 erro) | 0 |
| `make english-check` (repositório inteiro; log `fase50-english-check.log`): 0 achado, 0 problema nas exceções | 0 |
| gitleaks 8 (binário conferido do checkout principal) `dir` com `.gitleaks.toml` no fixture novo e em `crates/ward-core/src/protocol`: sem achado (antes da allowlist nova: 6 no fixture) | 0 |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target` (disco) | 0 |

## Problemas descobertos

- **gitleaks no fixture v2**. A regra `generic-api-key` acusa o valor base64 sob a chave
  `"password"`; sob `"senha"`, na v1, não acusava. São só dois valores, base64 de
  `falso-mestra-ward-01` e de `falso-WP-senha-01`. Entraram no `.gitleaks.toml` casados
  inteiros (`^…$`), sem padrão genérico nem caminho, como os outros de D15. As Fases 51–52
  podem ter o mesmo efeito nos testes que montam JSON com `"password":"<base64>"`.
  Decisão do orchestrator se prefere outro caminho.
- **Achado de gitleaks pré-existente**: `crates/ward-core/src/backup/tests.rs:369`
  (`new_password = "falso-mestra-ward-03"`) é acusado pela `generic-api-key` no `dir` com
  `.gitleaks.toml`. O valor não está na allowlist de D15 e o arquivo não é desta fase.
  Fica para o `make check` da Fase 61 confirmar.
- `ambiente` → `environment` também no `run`, onde é a lista de variáveis da CLI e não o
  ambiente da entrada. Segui o glossário (troca 1:1). Se o planner preferir `env`, muda
  aqui e nos clientes juntos.
- A mensagem de validação "deve ser nenhuma, starttls ou tls" (achado da Fase 48) fica: a
  segurança SMTP é texto no protocolo e só a validação do daemon a confere.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocol.rs` | alterado |
| `crates/ward-core/src/protocol/errors.rs` | alterado |
| `crates/ward-core/src/protocol/request.rs` | alterado |
| `crates/ward-core/src/protocol/response.rs` | alterado |
| `crates/ward-core/src/protocol/tests.rs` | alterado |
| `crates/ward-core/src/wire_v1.rs` | alterado |
| `crates/ward-core/src/wire_v2.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/tests/body_memory.rs` | alterado |
| `dev/fixtures/protocol-v2.jsonl` | criado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `.gitleaks.toml` | alterado |
| `docs/mission/handoffs/phase-50.md` | criado |

## Procedimentos

- [ ] `make check` rodado — não (PRD regra 6). Rodaram `make compile`, `make english-check`,
  clippy, fmt, os testes de protocolo e de fio e o gitleaks no fixture.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto; fixtures com os
  valores falsos de D15)
- [x] Teste de ataque para cada (SEG) tocado (V4.13, V4.14, V4.15)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
