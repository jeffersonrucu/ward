# Fase 52 — Protocolo v2 em JS e Python (Milestone 4)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, `git reset --hard c8e3cb9`
  (master atual, Fase 50 concluída).
- **Extensão** (`extension/`), protocolo v2 no native messaging (DT8, DT9; glossário,
  seções 10 e 12):
  - `service.js`:
    - pedidos `suggest`, `fill`, `save_login` com `v: 2`, `data`, `origin`, `entry`;
    - rascunho do `save_login`: `name`, `group`, `environment`, `type`, `fields`
      (`url`, `username`, `password`);
    - respostas lidas por `type`/`data`/`code`/`items`/`username`/`password`/`problems`
      (com `field` e `reason`);
    - mensagens internas: `t` = `submitted`/`pending`/`keep`/`discard`/`save`/`fill`;
      campos `tab`, `document`, `entry`, `origin`, `name`, `username`, `group`,
      `environment`, `error`, `problems`;
    - códigos locais: `no_response`, `invalid_request`, `pending_expired`.
  - `popup.js`:
    - os mesmos pedidos (`suggest`, `fill`, `open_app`) e as mesmas mensagens internas;
    - `FAILURES` com as chaves v2 do daemon (`origin_mismatch`, `entry_not_found`,
      `denied`, `expired`, `approval_unavailable`) e os motivos novos de `fillInPage`
      (`origin`, `frame`, `no_field`);
    - `SAVE_FAILURES` com os códigos locais novos;
    - `locked`, `unauthorized` e `validation` no lugar de `travado`, `nao_autorizado` e
      `validacao`;
    - as chaves de `FIELD_OF_PROBLEM` ficam pt-BR: são os valores de `field` da validação
      do daemon, texto exibido que a Fase 50 não mudou (comentário no código).
  - `login.js`:
    - `await_login`, `login_request`, `fill_login`, `fill`, `login_result`, `result`;
    - desfechos `submitted`, `refused`, `tab_changed`, `no_form`, `failure`.
  - `page.js`: motivos de `fillInPage` `origin`, `frame`, `no_field`, `form`, `failure`.
  - `submit.js`: `{ t: 'submitted', username, password }`.
  - A lógica não mudou: as mesmas conferências de `sender`, origem, `documentId`, tamanho
    e o mesmo apagamento da resposta. O diff é só de nomes.
- **Front-end** (`app/src`), valores do corpo v5 (DT4; achado da Fase 48):
  - tipo: `database`, `server`, `secure-note`, `generic`, em `types.js`, `form.js`,
    `Form.svelte`, `Credentials.svelte`, `ApprovalWindow.svelte` e `Showcase.svelte`;
  - nível: `blocked`, `approval`, `free`, em `levels.js`, `credentials.js`, `Form.svelte`,
    `LevelDialog.svelte` e `LevelCard.svelte`;
  - segurança SMTP: `none`, em `form.js` (opção e porta padrão);
  - os rótulos exibidos ("banco", "servidor", "nota segura", "genérico", "Livre"…) não
    mudaram;
  - os campos `serde` (`tipo`, `nivel`, `nome`…) e os códigos de erro dos comandos ficam
    para a Fase 54, assim como o `request?.estado === 'bloqueado'` da janela de aprovação,
    que é valor do `app/core` (`approval.rs`);
  - comentários de `form.js` e `credentials.js` atualizados.
- **`app/core/src/level.rs`** (o único arquivo Rust tocado, pela nota do plano). Só os
  testes mudaram:
  - `level_only_by_the_three_protocol_names` aceita `free`/`approval`/`blocked` e recusa
    também os nomes v4 (`livre`, `com-aprovacao`, `bloqueado`);
  - `attack_new_rust_names_are_not_wire_names` usa `free`.
  - O código (`level_from_name` → `AccessLevel::from_name`) já era o do v5 desde a Fase 48.
  - **Aviso**: a Fase 51 roda junto e pode mexer no `app/core`. Se ela também tocar no
    `level.rs`, o merge precisa juntar as duas versões.
- **`app/src/lib/bridge`** e a ponte de teste do front-end (`test-bridge.js`): nada
  mudou. Falam os comandos do Tauri (`codigo`/`mensagem` do `ErroApp`), que são da Fase
  54, e não o protocolo do daemon.
- **Python e shell que falam com o socket ou com o `ward-nm`**:
  - `dev/tests/socket_matrix.py`:
    - a matriz em v2 (`unlock`, `list`, campos `entry`/`field`/`target`/`request`/`level`,
      campos inventados `reveal`/`field`, `run` com `stdin_kind: "null"` e
      `environment`);
    - tipos de credencial `secret`/`token`/`totp_code`/`fill`;
    - `unauthorized`;
    - teste novo **m2:V4.13**: cada pedido v1 de `dev/fixtures/legacy/protocol-v1.jsonl`,
      com o cofre destravado, recebe só `{"v":2,"type":"error","data":{"code":"unknown_version","message":…}}`,
      sem padrão de `.dev/variations.txt`, e o daemon fecha a conexão. No fim, um `list`
      v2 ainda responde (o `destravar`/`travar` v1 não mexeram no cofre).
  - `dev/tests/socket_attack.py`:
    - os pedidos em v2;
    - "versão errada" (v99) agora exige `unknown_version` e o fechamento da conexão
      (DT8);
    - caso novo "quadro v1 (0.1.x)": só `unknown_version`, com `code`/`message`, e a
      conexão fecha.
  - `dev/tests/nm.py`:
    - pedidos v2 (`status`, `fill`, `suggest`, `reveal`) e códigos `unauthorized`,
      `invalid_frame`, `invalid_request`;
    - teste novo **m2:V5.10**: os pedidos v1 da extensão 0.1.x (`estado`, `preencher`,
      `sugerir`) pelo `ward-nm` aberto direto e pelo Chrome real (`fill_v1`,
      `suggest_v1` no `sw.js` de teste) recebem só `unknown_version`, sem `password`,
      `senha`, `username`, `usuario` nem `totp`, e nenhum padrão da lista.
  - `dev/tests/extension.py`:
    - socket (`unlock`, `lock`, `login`) e `forced_fill` em v2;
    - as mensagens de ataque da página e do content script com os nomes novos (o ataque
      `pending`/`save`/`discard` do content script precisa falar a língua que o service
      worker entende, senão ele falha de graça);
    - `pending_from_popup`, `origin_mismatch`, `reason: 'origin'`, `open_app` e
      `denied`;
    - o log do aprovador de teste com `operacao=fill` e `decisao=deny`;
    - `ward lista --json` lido por `name`/`group`/`environment`/`type` (sem `level`, D50);
    - o `ward-nm` aberto à mão e o Chrome falso em v2.
  - `dev/tests/login.py` e `dev/tests/playwright.py`:
    - `unlock` v2;
    - o log do aprovador de teste com `decisao=deny`/`decisao=confirm`. O aprovador
      (Rust, `crates/wardd/aprovador-teste`) imprime `Decision::name()`, que desde a Fase
      50 é v2. O arquivo de modo (`negar`/`confirmar`) é lido pelo próprio aprovador e não
      mudou.
  - `dev/tests/chrome_policy.py`: o host atacante e o `sw.js` em v2 (`suggest`, `fill`,
    `password`), para o controle sem política continuar provando o roubo.
  - `dev/tests/cli.py`:
    - V4.1 com as chaves em inglês do `list --json` (`name`, `type`, `group`,
      `environment`, `project`, `address`) e o `type` entre os nomes v5 (m2:V4.18);
    - teste novo **m2:V4.16**, o servidor falso v1 em Python. Ele roda num processo filho
      não dumpable (`prctl(PR_SET_DUMPABLE, 0)` por `ctypes`) e do mesmo uid, então passa
      pela conferência do servidor do `test-mode`, como passaria o `wardd` 0.1.x.
      Responde a todo quadro com o `estado` v1 e grava os bytes num arquivo 0600 de uma
      pasta 0700 própria;
    - o `ward destravar` no pty tem de: sair ≠ 0, mostrar "incompatível" sem o prompt da
      senha, e não mandar ao falso a senha nem o base64 dela; o falso só pode ter recebido
      `status`. Depois, `lista --json` e `travar` também recusam.
  - `dev/open-app.py`: `open_app` v2 (mensagens `open_app: …`).
  - `dev/container-platform/inside.sh` (shell, mas fala com o `ward-nm`):
    - as mensagens do `M` em v2;
    - as conferências por `"role":"extension"`, `"name":`, `"items":[]`, `"code":…`
      (`denied`, `approval_unavailable`, `origin_mismatch`, `unauthorized`,
      `invalid_frame`), `"type":"fill"` e `"password"`.
  - `crates/wardd/tests/fake_chrome.py` e `fake_ssh.py` (Python) **não** mudaram: falam
    CDP e o agente SSH, não o protocolo do Ward. O `"usuario, senha"` do `fake_chrome.py`
    casa com o script de preenchimento de `playwright.rs` (Rust), que não é desta fase.
- **`dev/english-check/frozen.txt`**: saíram as 35 entradas da Fase 52 (`extension/*.js`)
  e o comentário da seção. Sobra uma entrada de fase anterior à 54: a da Fase 51
  (`app/core/src/service.rs livre 51`).
- **`docs/glossario.md`**, seção 12: códigos locais (`invalid_request`,
  `pending_expired`), campos das mensagens internas, motivos de `fillInPage` e desfechos
  do `ward login`, com a nota de que a seção vale desde a Fase 52.

## Literais alterados (PRD regra 5)

- Extensão: todo nome de fio e de mensagem interna listado acima. Motivo: o protocolo v2
  (DT8, DT9) e a seção 12 do glossário.
- `app/src`: valores de tipo, nível e segurança SMTP. Motivo: o corpo v5 (DT4), que o
  `from_name` exige desde a Fase 48.
- Python/shell: os nomes de fio v1 → v2. Motivo: DT8.
- Mensagens de teste (pt-BR, DT20) que citavam a operação pelo nome: `executar` → `run`,
  `abrir_app` → `open_app`, `preencher`/`sugerir`/`revelar`/`lista` → `fill`/`suggest`/
  `reveal`/`list`, `nao_autorizado` → `unauthorized`, `quadro_invalido` →
  `invalid_frame`.
- `level.rs` (testes): `livre` → `free` e os nomes v4 na lista de recusa.
- Conferi com `grep` que nenhum quadro v1 sobrou fora dos fixtures, dos testes de recusa
  novos (V4.13, V4.16, V5.10) e de `docs/mission/arquivo/`.

## Segurança

- **Invariantes tocadas**:
  - S1 e D9 (extensão): o preenchimento continua só pela origem conferida pelo daemon,
    `documentId` fixo, `fillInPage` conferindo a origem de novo e a resposta apagada
    depois.
  - Mensagens internas (V6.6): `sender` conferido como antes (content script do quadro
    principal, popup da própria extensão), sem `externally_connectable`. A senha nunca
    vai ao popup.
  - S5 (V4.16): o teste prova que a CLI para antes do prompt.
  - S1/RM3 (V4.13, V5.10): v1 recebe só o erro de versão e nada mais.
  - S9 (front-end): os níveis são só os três nomes v5; o `app/core` continua recusando
    qualquer outro (teste de `level.rs`).
  - Nenhuma conferência mudou de lógica.
- **Testes de ataque escritos** (não rodados, PRD regra 6):
  - V4.13:
    - `dev/tests/socket_matrix.py` › seção "m2:V4.13" (cada pedido v1 do fixture,
      destravado, só `unknown_version`, conexão fechada);
    - `dev/tests/socket_attack.py` › casos "versão errada" e "quadro v1 (0.1.x)".
  - V4.16: `dev/tests/cli.py` › seção "1b. m2:V4.16", o servidor falso v1 não dumpable.
  - V5.10: `dev/tests/nm.py` › `only_unknown_version`, os pedidos v1 direto no `ward-nm` e
    `fill_v1`/`suggest_v1` pelo Chrome real.
  - V5.10/S1 (extensão): `dev/tests/extension.py` › ataque do content script com as
    mensagens novas e `forced_fill` v2.
  - S9: `app/core/src/level.rs` › `level_only_by_the_three_protocol_names` (nomes v4
    recusados).
- **Dependências novas**: nenhuma. O `ctypes` do V4.16 é da biblioteca padrão do Python.

## Ficou de fora

- **Rust (Fase 51, em paralelo)**:
  - `wardd`, `ward`, `ward-nm`, `ward-test-bridge`, `app/core` (fora o `level.rs`) e os
    testes Rust;
  - a mensagem da CLI para versão incompatível. O V4.16 do `cli.py` procura
    "incompatível" na tela; se a Fase 51 escrever outro texto, ajustar ali;
  - as chaves do `ward list --json`. O `cli.py` espera `name`, `type`, `group`,
    `environment`, `project`, `address` (glossário: `endereco` → `address`);
  - o V4.16 do app (`ward-app-core`).
- **V4.16 e V5.10 com os binários 0.1.4 de verdade**: o `wardd` 0.1.4 no socket de teste
  e a extensão 0.1.4 no perfil precisam de `.dev/baseline/` (Fase 64). Os testes desta
  fase usam o servidor falso v1 e os quadros v1 da extensão 0.1.x.
- **Versão do manifesto 0.2.0**: DT19, Fase 60.
- **Subcomandos da CLI** (`destravar`, `lista`, `travar` nos testes): Fase 55.
- **Comandos e `serde` do Tauri** (`app/src/lib/bridge`, campos `tipo`/`nivel`, códigos
  do `ErroApp`, `estado === 'bloqueado'` da aprovação): Fase 54.
- **Saídas de binários Rust de teste** que os scripts procuram e que não são protocolo:
  `papel=agente`/`papel=extensao` no `inside.sh` (o servidor de teste da plataforma),
  `wardd-test-client … destravar`, o log `preencher <entrada>` do `wardd` que o
  `nm.sh` procura, e o `"usuario, senha"` do `fake_chrome.py`. Ficam com quem muda o Rust.
- `CLAUDE.md` cita `abrir_app` na linha do `make app-bridge`: documentação, Fases 58–59.
- Nada rodou além da compilação (PRD regra 6).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard c8e3cb9` (árvore limpa) | 0 |
| `make english-check` (repositório inteiro; log `.dev/logs/fase52-english-check.log`): 0 achado, 0 problema nas exceções | 0 |
| `make compile` (log `.dev/logs/fase52-make-compile.log`; workspace, `app/src-tauri`, Vite, sintaxe 48 shell/35 Python/7 JS, 0 erro) | 0 |
| `make extension` (log `.dev/logs/fase52-make-extension.log`; id `nhkehjnmlgkebhmelajfpmpblepaichf`) | 0 |
| `make check` | não rodado (PRD regra 6) |
| `rm -rf target app/src-tauri/target app/node_modules app/dist app/dist-teste extension/dist` (disco) | 0 |

## Problemas descobertos

- **V4.18 × D50**: o contrato do V4.18 lista `level` entre as chaves do `ward list --json`,
  mas o V4.1 e o D50 (missão 1) dizem que a lista não tem nível. Os testes desta fase
  seguem o D50: o `cli.py` espera seis chaves sem `level`, e o `extension.py` confere
  `"level" not in item`. O planner decide. Se o `level` entrar, mudam o `cli.py` e a
  Fase 51.
- **Local `expirou` → `pending_expired`**, e não `expired`. `expired` já é código do
  daemon (a janela de aprovação expirou) e tem texto próprio no `FAILURES`; o local
  continua com o texto "O pedido para salvar expirou". Registrado no glossário.
- **Coordenação com a Fase 51**:
  - `cli.py`: o texto "incompatível" e as chaves do `list --json`;
  - `level.rs`: possível conflito de merge;
  - `inside.sh`: depende do `ward-nm` decodificar v2 e repassar o `role`.
- **`nm.sh`** procura `preencher acme-wp-dev$` no log do daemon. É um texto de log pt-BR
  do `core.rs`, não o nome da operação, e continua valendo. Se uma fase futura mudar o
  log, muda o `nm.sh` junto.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `extension/service.js` | alterado |
| `extension/popup.js` | alterado |
| `extension/login.js` | alterado |
| `extension/page.js` | alterado |
| `extension/submit.js` | alterado |
| `app/src/lib/components/levels.js` | alterado |
| `app/src/lib/components/types.js` | alterado |
| `app/src/lib/components/LevelCard.svelte` | alterado |
| `app/src/lib/components/LevelDialog.svelte` | alterado |
| `app/src/lib/test/Showcase.svelte` | alterado |
| `app/src/screens/ApprovalWindow.svelte` | alterado |
| `app/src/screens/Credentials.svelte` | alterado |
| `app/src/screens/Form.svelte` | alterado |
| `app/src/screens/credentials.js` | alterado |
| `app/src/screens/form.js` | alterado |
| `app/core/src/level.rs` | alterado (só testes) |
| `dev/tests/socket_matrix.py` | alterado |
| `dev/tests/socket_attack.py` | alterado |
| `dev/tests/nm.py` | alterado |
| `dev/tests/extension.py` | alterado |
| `dev/tests/login.py` | alterado |
| `dev/tests/playwright.py` | alterado |
| `dev/tests/chrome_policy.py` | alterado |
| `dev/tests/cli.py` | alterado |
| `dev/open-app.py` | alterado |
| `dev/container-platform/inside.sh` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-52.md` | criado |

## Procedimentos

- [ ] `make check` rodado: não (PRD regra 6). Rodaram `make compile`, `make english-check`
  e `make extension`.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto; os testes leem
  os valores de D15 do `entries.json` em tempo de execução)
- [x] Teste de ataque para cada (SEG) tocado (V4.13, V4.16, V5.10, S9 do `level.rs`)
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica (nenhum texto nem layout
  mudou)
- [x] Ficou dentro do escopo da fase
