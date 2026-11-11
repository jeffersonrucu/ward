# Plano — Ward

Status: aprovado

Missão 2: código em inglês (PRD `docs/mission/prd.md`; contrato
`docs/mission/validation-contract.md`; escopo "Missão 2 — Código em inglês"). A missão
1 está em `docs/mission/arquivo/m1/`.

## Como as fases trabalham (vale para todas)

- **Sem suíte por fase nem por milestone** (pedido do usuário, 2026-11-10; PRD regra 6).
  Cada fase termina com **uma** compilação, `make compile` (a partir da Fase 2), e com
  `make english-check PATHS="<caminhos da fase>"` limpo. Testes novos ou alterados são
  escritos, mas não rodam. O orchestrator **não chama validadores** nos Milestones 1–5:
  marca a fase, fecha o milestone e segue. A bateria inteira e os validadores
  (scrutiny, security-auditor, QA) rodam só no Milestone 6. O que falhar lá vira fase
  de correção no fim do Milestone 6.
- **Troca 1:1** (PRD regra 3): renomeie no lugar e ajuste todos os usos no repositório
  inteiro na mesma fase, pelo glossário `docs/glossario.md`. Termo novo entra no
  glossário na mesma fase.
- **Fio congelado até a Fase 48** (PRD regra 4, DT3): nada que cruze fronteira muda nos
  Milestones 1–3. Campo `serde` renomeado leva `#[serde(rename = "<antigo>")]`. Nome de
  fio que o checador acusa vai para `dev/english-check/frozen.txt` com a fase que o
  muda.
- **Handoff** (`docs/mission/handoffs/phase-<N>.md`): renomes feitos (antigo → novo),
  literais alterados com o motivo (PRD regra 5), entradas novas de `allow.txt`/
  `frozen.txt`, o resultado do `make compile` e do `english-check`, e qualquer ponto de
  segurança em que o nome ficou de propósito.
- **Lógica de segurança intocada** (PRD regra 7): nas conferências de papel, máscara,
  cripto, gravação atômica, endurecimento, aprovação, PIN e domínio, o diff tem de ser
  só de nomes e comentários.
- **Paralelismo** (worktrees, commit local por fase, sem push): o Milestone 1 é
  sequencial. Depois dele há três trilhas que podem rodar juntas: **R** (Rust,
  Milestone 2, sequencial dentro da trilha, de baixo para cima), **W** (web, Fases
  35–39) e **S** (scripts e instalação, Fases 40–47). W e S não dependem de R porque o
  fio está congelado. Dentro de W e de S, fases diferentes podem rodar juntas (cada uma
  tem os seus arquivos), exceto a 35 (tokens e pastas do `app/src`), que vem antes das
  outras de W. O Milestone 4 começa quando R, W e S acabaram e é sequencial. O 5 também.
  O 6 roda em sequência porque usa o mesmo daemon e o mesmo cofre de teste.
- **Testes de fio** (`crates/ward-core/src/wire_v1.rs`, achado da Fase 3): as Fases
  10–18 renomeiam ali só os usos da API atual; fixture e expectativas não mudam (V2.2
  aceita diff só de nomes nesse arquivo).
- **Captura de `format!`** (achado da Fase 7): renomear variável capturada muda o
  literal (`{rt_texto}` → `{rt_text}`); o V6.3 aceita diferença só de nome de captura.
- **Base do worktree**: o worker confere que o worktree parte do master atual
  (`git reset --hard <hash>` com a árvore limpa, se não).
- Tamanho: cada fase tem ~2,5–4 mil linhas para ler e reescrever (código + testes).

## Milestone 0 — Design no FigCraft

Pulado (resposta 1 do usuário, 2026-11-10): nenhuma tela muda. `Ward.figcraft` e
`docs/mission/design-nodes.md` ficam como estão. Sem fases.

## Milestone 1 — Base da tradução

Contrato: V1.1–V1.10. Sequencial.

- [x] Fase 1 — Glossário. Inventário de todo nome pt-BR do repositório (grep por crate,
  módulo, arquivo, pasta, tipo e função públicos, alvo e variável do make, subcomando,
  variável de ambiente, operação/campo/erro/papel do protocolo, nome gravado no cofre,
  comando e evento do Tauri, classe CSS e token, mensagem de native messaging) e
  `docs/glossario.md` (pt-BR, tabela termo → inglês, mais uma seção por categoria com
  todos os nomes externos). Inclui as tabelas DT4, DT8, DT11–DT15 do PRD, completadas
  (campos e erros do protocolo, outros nomes gravados). Se achar um nome gravado fora
  de DT4, atualiza a tabela no PRD. Escreve também `dev/english-check/pt-words.txt`
  (radicais do domínio e palavras funcionais). Não muda código. Sem compilação. — V1.1
- [x] Fase 2 — Checador e `make compile`. `dev/english_check.py` (DT17: por linguagem,
  comentários/identificadores/literais; nomes de arquivo e pasta; `allow.txt`,
  `frozen.txt`, exceção sem uso falha; `--paths`, `--literals`, `--self-test` que
  planta em cópia em `.dev/`) e alvos `english-check` (só relatório, ainda fora do
  `make check`) e `compile` (V1.5; passa pelas origens; sem teste). Só biblioteca padrão.
  Compilação: `make compile`. — V1.2, V1.3, V1.4, V1.5
- [x] Fase 3 — Linha de base legada (DT18). `git worktree` da tag `v0.1.4` em
  `.dev/baseline/src`. Gera, com o código de lá: `dev/fixtures/legacy/`
  (`vault-0.1.4.ward`, `state-0.1.4`, `backup-0.1.4/`, `protocol-v1.jsonl`,
  `list-v1.json`, `help-0.1.4.txt`, `strings-0.1.4.txt` pelo `--literals`,
  `SHA256SUMS`) e `.dev/baseline/` (binários 0.1.4 de release e de `modo-teste`, `.deb`
  0.1.4 com CRX3 falso, a extensão 0.1.4 montada). Escreve no `ward-core` os testes de
  fio que comparam a serialização dos tipos do protocolo com `protocol-v1.jsonl` e o
  corpo serializado com o de `vault-0.1.4.ward` (V2.2). Roda o `make gitleaks` só sobre
  as fixtures novas (se acusar, para e pergunta). Compilação: `make compile`. — V1.6,
  V1.7
- [x] Fase 4 — Crates e feature (DT13). `git mv` de `crates/ward-plataforma` →
  `crates/ward-platform`, `crates/ward-ponte-teste` → `crates/ward-test-bridge`,
  `app/nucleo` → `app/core` (pacote `ward-app-core`); nomes de pacote e de lib,
  `[workspace]`, dependências, `use`, `app/src-tauri/Cargo.toml`, os dois `Cargo.lock`
  (só os nomes; nenhuma versão muda), `deny.toml`. Feature `modo-teste` → `test-mode`
  em Cargo, `cfg`, Makefile, scripts e `dev/origens.py`. A marca nos binários (literal)
  fica para a Fase 56. Compilação: `make compile`. — V1.8
- [x] Fase 5 — Pastas e arquivos fora dos crates e do `app/src` (DT13, DT1). `git mv` de
  `instalacao/` → `install/` (e os arquivos dela: `arvore.txt` → `tree.txt`,
  `instalar.sh` → `install.sh`, `grupos.sh` → `groups.sh`, `despejo.sh` → `dump.sh`,
  `empacotar-extensao.sh` …), `extensao/` → `extension/` (`chave-publica.txt` →
  `public-key.txt`, `servico.js`, `pagina.js`, `envio.js` …), `dev/testes/` →
  `dev/tests/`, `dev/servicos/` → `dev/services/`, `dev/conteiner-*` →
  `dev/container-*`, `dev/fixtures/entradas.json` → `entries.json`, `dev/ambiente.sh` →
  `dev/env.sh`, `dev/origens.py` → `dev/origins.py`, `dev/variacoes.py` →
  `dev/variations.py`, e os demais arquivos de `dev/` pelo glossário; todos os usos
  (Makefile, scripts, `CLAUDE.md`, `.gitleaks.toml` e `dev/gitleaks-*.toml`, `.gitignore`,
  docs; achado da Fase 2: também `dev/fixtures/anexos/` e `app/design/icones/*.svg`). Caminhos de `.dev/` pelo glossário em scripts e no `config.rs` de teste (o da
  release, `~/.local/share/ward/cofre.ward` e `estado`, não muda), com `mv` dos arquivos
  existentes; a chave da extensão vai por `mv` para `.dev/extension/key.pem`, 0600, e a
  fase confere o id (V1.9; nunca imprime a chave). Evidência dos scripts em
  `docs/mission/evidence/<suíte>/`. Compilação: `make compile`. — V1.8, V1.9
- [x] Fase 6 — Makefile e `CLAUDE.md` (DT14, DT2). Alvos e variáveis pelo glossário,
  sem alias, com todos os usos (scripts, mensagens de "rode `make …` antes", docs,
  `.claude/agents/*.md`, `.claude/skills/mission/SKILL.md`, `skill/README.md`). A tabela
  de comandos do `CLAUDE.md` com os nomes novos e a convenção de IDs (`m2:`; IDs sem
  prefixo = missão 1, em `docs/mission/arquivo/m1/`). Alvo `extension` no `.PHONY` (existe a pasta `extension/`; achado da
  Fase 5); `target/sem-modo-teste` pelo glossário. Compilação: `make compile`. —
  V1.10

## Milestone 2 — Rust em inglês (trilha R, fio congelado)

Contrato: V2.1–V2.8. Sequencial, de baixo para cima: cada fase traduz os
identificadores, os comentários e os nomes de módulo/arquivo (`testes.rs` → `tests.rs`)
dos arquivos listados, e ajusta os usos nos crates que dependem deles (só os usos).
Compilação de cada fase: `make compile`. Checador: os arquivos da fase.

`ward-platform`:
- [x] Fase 7 — `launcher` (`lancador.rs`), `child` (`filho.rs`), `relogio.rs`, `teste.rs`,
  `lib.rs`. — V2.1, V2.3
- [x] Fase 8 — `sys.rs`, `processo.rs`, `descendentes.rs`, `memoria.rs`. — V2.1, V2.6, V2.7
- [x] Fase 9 — `socket.rs`, `papel.rs`, `conectar.rs`, `terminal.rs`, `endurecimento.rs`,
  `servidor.rs`, `app.rs`, `pasta.rs` e os testes do crate; checador também em `process.rs`, `launcher.rs`, `child.rs`, `clock.rs`,
  `testing.rs` e `lib.rs` (achado da Fase 7). — V2.1, V2.3

`ward-core`:
- [x] Fase 10 — `lib.rs`, `limites` (+ testes), `cripto`, `totp` (+ testes), `gerador`
  (+ testes), `origem`, `exibicao`. — V2.1
- [x] Fase 11 — `formato` (+ testes), `gravacao` (+ testes). Gravação atômica intocada.
  — V2.1
- [x] Fase 12 — `entrada.rs`, `entrada/segredo.rs`, `entrada/validacao.rs` (os nomes
  gravados de `nome()`/`de_nome()` são literais congelados; os rótulos pt-BR ficam). —
  V2.1
- [x] Fase 13 — `entrada/testes.rs`, `configuracao` (+ testes). — V2.1
- [x] Fase 14 — `corpo.rs`, `corpo/cofre_teste.rs`, `corpo/testes.rs` (`VERSAO_CORPO`
  continua 4). — V2.1, V2.2
- [x] Fase 15 — `backup` (+ testes), `entrega` (+ testes; nomes das variáveis entregues
  congelados). — V2.1
- [x] Fase 16 — `mascara.rs`, `mascara/automato.rs`, `mascara/testes.rs`. Lógica da
  máscara intocada. — V2.1, V2.5
- [x] Fase 17 — `protocolo.rs`, `campos.rs`, `quadro.rs`, `json.rs`, `erros.rs`,
  `pedido.rs` (nomes de operação, campo, erro e papel congelados como literais; o
  `dev/tests/socket_matrix.py` lê `protocolo.rs`, `Operacao, `, `tabela!`, `TODOS`/
  `AGENTE_E_APP`/`SO_AGENTE` e muda junto, achado da Fase 45). —
  V2.1, V2.2
- [x] Fase 18 — `protocol/response.rs`, `protocol/tests.rs`; `crates/ward-core/tests/sources.rs`
  e `tests/body_memory.rs`, antes `tests/fontes.rs` e `tests/corpo_memoria.rs` (achado da
  Fase 10; o `sources.rs` já exclui `tests`/`tests.rs`). — V2.1, V2.2

`wardd`:
- [x] Fase 19 — `main.rs`, `lib.rs`, `config.rs`, `log.rs`, `lancamento.rs`,
  `arquivos.rs`, `uso.rs`, `trava.rs` (formato do estado congelado; o V3.15 em
  `tests/approval.rs` lista `uso.rs` pelo nome e muda junto, achado da Fase 23). — V2.1, V2.7
- [x] Fase 20 — `nucleo.rs`, `aprovacao.rs` (o V3.15 em `tests/approval.rs` lista os dois
  pelo nome e muda junto, achado da Fase 23). — V2.1
- [x] Fase 21 — `execucao.rs`, `agente_ssh.rs`, `ssh.rs`, `ssh_testes.rs`. — V2.1, V2.7
- [x] Fase 22 — `playwright.rs`, `cdp.rs`, `login.rs`, `ponte.rs` (`serde` do CDP: campos
  do Chrome não mudam). — V2.1
- [x] Fase 23 — `servidor.rs`, `externo.rs`, `udisks.rs`; testes `comum/`, `units.rs`,
  `memoria.rs`, `socket.rs`, `datas.rs`. — V2.1, V2.4
- [x] Fase 24 — testes `ssh.rs`, `run.rs`, `ssh_falso.py`. — V2.1
- [x] Fase 25 — testes `aprovacao.rs`, `trava.rs`, `nucleo.rs` (com o `[[test]]` `trava` no `Cargo.toml`, achado da Fase 4). — V2.1
- [x] Fase 26 — testes `externo.rs`, `backup.rs`, `configuracoes.rs`, `d62.rs` (o
  `externo_nao_usa_link` procura `rename_no_replace(destination.as_fd()` em `src/external.rs`;
  achado da Fase 23). — V2.1
- [x] Fase 27 — testes `playwright.rs`, `extensao.rs`, `login.rs`, `chrome_falso.py`,
  `app_valor.rs`, `valor_app.rs`, `app_teste.rs`, `anexos_edicao.rs` (com os `[[test]]` `app_teste` e `app_valor` no
  `Cargo.toml`, achado da Fase 4). — V2.1

Clientes Rust:
- [x] Fase 28 — `crates/ward` (`args`, `cliente`, `comandos`, `texto`, testes;
  subcomandos e textos congelados) e `crates/ward-nm` (`ID_EXTENSAO` → `EXTENSION_ID`, com a leitura do
  `pub const` em `dev/tests/extension.py` e o regex e as mensagens de
  `dev/extension/build.py` (achado da Fase 39); mensagens congeladas). — V2.1
- [x] Fase 29 — `crates/ward-test-bridge` (`http.rs`, `main.rs`, `simulados.rs`,
  `tests/ataques.rs`). — V2.1
- [x] Fase 30 — `app/core`: `servico.rs`, `segredo.rs`, `erro.rs`, `lib.rs`. — V2.1
- [x] Fase 31 — `app/core`: `daemon_falso.rs` (com o reexport `SECRETS as SEGREDOS` e os usos, achado da Fase 14), `daemon.rs`, `cliente.rs`. — V2.1
- [x] Fase 32 — `app/core`: `formulario.rs`, `aprovacao.rs`, `anexos.rs`,
  `restaurar.rs`, `bandeja.rs` (`serde` para a webview com `rename`). — V2.1
- [x] Fase 33 — `app/core`: `comandos.rs`, `configuracoes.rs`, `backup_externo.rs`,
  `terminal.rs`, `chave.rs`, `partida.rs`, `credenciais.rs`. — V2.1
- [x] Fase 34 — `app/core`: `destrave.rs`, `primeiro_uso.rs`, `gerador.rs`,
  `area_transferencia.rs`, `nivel.rs`; `app/src-tauri/src` (`main.rs` — o heredoc do
  `dev/tests/webkit-real.sh` procura `partida::iniciar()`, `tomar_daemon`, `vigiar`,
  `na_vigia`, `ErroApp::ConexaoPerdida` e muda junto, achado da Fase 45; o
  `dev/tests/clipboard.py` roda `area_gtk::testes::sessao_real` pelo nome, achado da Fase 46 —, `area_gtk.rs`,
  `build.rs`; nomes das funções `#[tauri::command]`, dos parâmetros e dos eventos
  congelados até a Fase 54). Compilação: `make compile` (com o `cargo check` do
  `app/src-tauri`). — V2.1

- [x] Fase 34a — Resto do Rust (achado do fechamento do M2: `make english-check PATHS="crates app/core app/src-tauri/src"`
  dá 1158 achados): comentários dos `Cargo.toml` (`crates/wardd`, `crates/ward-core`, `app/core`), os binários de teste
  `crates/wardd/app-teste/main.rs` e `aprovador-teste/main.rs` (pastas e nomes de binário ficam para a Fase 56),
  `app/core/tests/configuracao_tauri.rs`, e os identificadores que sobraram em `fake_daemon.rs`, `client.rs`,
  `ward/src/{tests,commands}.rs`, `wardd/src/ssh.rs` e testes do `wardd`. Nome de fio, de comando Tauri, de
  variável entregue ou de teste congelado continua literal (exceção em `frozen.txt` com a fase que o muda).
  No fim, o checador nesses caminhos dá 0 achado. — V2.1

## Milestone 3 — Web e scripts em inglês (trilhas W e S, fio congelado)

Contrato: V3.1–V3.7. Pode rodar em paralelo com o Milestone 2 (depois da Fase 6).

Trilha W (a 35 primeiro, depois 36–39 em paralelo):
- [x] Fase 35 — **(UI)** Base do `app/src`: pastas `telas` → `screens`,
  `lib/componentes` → `lib/components`, `lib/estilo` → `lib/style`, `lib/ponte` →
  `lib/bridge`, `lib/teste` → `lib/test`; nomes dos componentes (`.svelte`) e dos
  módulos JS com todos os imports; tokens CSS (`tokens.css`: propriedades custom e
  classes) com todos os usos no app e em `extension/popup.css` e no montador da
  extensão; nomes dos `.svg` de `app/design/icons/` (chaves de
  `ICONES`, inclusive as montadas como `tipo-${tipo}`; achado da Fase 5); ícones da bandeja `app/src-tauri/icons/bandeja-*-32.png` com o caminho no Tauri (achado da Fase 2); `bidi.js`, `main.js`, `App.svelte`, `lib/components/*` (identificadores e
  comentários). Nomes dos comandos Tauri e chaves de mensagem congelados. Compilação:
  `make compile`. — V3.1, V3.2, V3.6
- [x] Fase 36 — **(UI)** `screens/Form.svelte` (Formulario), `form.js`,
  `Credentials.svelte`, `credentials.js`, `ChooseType.svelte`. — V3.1, V3.2, V3.4
- [x] Fase 37 — **(UI)** `ApprovalWindow.svelte` (JanelaAprovacao), `approval.js`,
  `PasswordGenerator.svelte`, `FirstUse.svelte`, `first-use.js`,
  `RestoreBackup.svelte`, `restore.js`, `EmergencyKit.svelte`, `emergency-kit.js`. —
  V3.1, V3.2, V3.3, V3.4
- [x] Fase 38 — **(UI)** `Settings.svelte`, `settings.js`, `Unlock.svelte`,
  `ChangePassword.svelte`, `ChangePin.svelte`, `ChooseUsbDrive.svelte`,
  `lib/bridge/*`, `lib/test/Showcase.svelte`. — V3.1, V3.2, V3.3
- [x] Fase 39 — **(UI)** Extensão: `extension/*.js`, `popup.html`, `popup.css` (classes),
  `manifest.json` (só comentários do montador; campos do Chrome não mudam), montador
  `dev/extension/` (`montar.py` → `build.py`). Mensagens de native messaging
  congeladas. Ajusta os literais de `dev/tests/extension.py` (`data-entrada`, classes do
  popup, `preencherNaPagina`; achado da Fase 40). Compilação: `make compile`. — V3.1, V3.5

Trilha S (fases independentes entre si):
- [x] Fase 40 — `dev/tests/extension.py`, `dev/tests/cli.py`. — V3.1
- [x] Fase 41 — `dev/tests/migration-guide.py` (`migracao.py`),
  `attacks-foundation.sh`, `skill.py`. — V3.1, V3.7
- [x] Fase 42 — `dev/variations.py`, `dev/tests/variations_test.py`, `login.py`,
  `revealed_selection.py`. — V3.1
- [x] Fase 43 — `dev/container-*` (`inside.sh` de plataforma, install e deb; units, uid;
  `falso.c` → `fake.c`), `dev/tests/clipboard.py`; exemplos `wardd-cliente-teste`,
  `pai-chrome`, `chrome-falso`, `dev-cofre` e `partida-conteiner` com todos os usos
  (achado da Fase 4). — V3.1, V3.7
- [x] Fase 44 — `dev/tests/dev-vault.sh`, `ssh_home.py`, `install.sh`,
  `attacks-services.sh`, `chrome_policy.py`, `app_e2e.py`, `playwright.py`. — V3.1,
  V3.7
- [x] Fase 45 — `dev/tests/real-home.sh`, `webkit-real.sh`, `dev/origins.py`,
  `dev/services/web/server.py`, `nm.py`, `socket_attack.py`, `dev/services/generate.py`,
  `deb.sh`, `dev/compose.yml`, `socket_matrix.py`; `make test-webkit-real NO_MPS=1
  NO_39C=1` passa `--no-mps--no-39c` grudado (achado da Fase 6). — V3.1, V3.7
- [x] Fase 46 — o resto de `dev/` (scripts com menos de 150 linhas, `daemon.sh`,
  `bridge.sh`, `open-app.py`, `dev/services/*`, Dockerfiles, `entrypoint.sh`,
  `pg_select.py`, `bidi*`, `gst-channels.sh`, `terminal-test.py`, `log-scan.py` …); `CASA_PASSWD` → `PASSWD_HOME`, `OPCOES_SSH`, `IMAGEM_PG` e `RAIZ` → `ROOT` (apagar os `RAIZ="$ROOT"` do `install.sh`/`attacks-services.sh`) e as funções do `common.sh` (`exigir`, `cliente`, `ssh_senha`…, achar por grep) em todos os usos
  (achado da Fase 41). —
  V3.1, V3.7
- [x] Fase 47 — `install/` (`install.sh`, `groups.sh`, `dump.sh`, `chrome.sh`,
  `crx.py`, `deb.sh`, `deb/*`, `systemd/*`, `tmpfiles.d`, `coredump.conf.d`; caminhos
  instalados congelados até a Fase 57), comentários do `Makefile`, `deny.toml`,
  `app/src-tauri/deny.toml`, `.gitleaks.toml`, `dev/gitleaks-*.toml`,
  `rust-toolchain.toml`, `Cargo.toml` raiz. Textos do Makefile/`origins.py` que o
  `attacks-foundation.sh` procura mudam junto com o `grep` (achado da Fase 41). — V3.1

- [x] Fase 47a — Correção (achado da Fase 47): a varredura do V1.5 em
  `dev/tests/attacks-foundation.sh` exclui `dev/fixtures/legacy/` (linha de base da Fase 3:
  caminho do cofre real, variável do agente SSH e pasta do ssh na ajuda 0.1.4), com
  controle de que a varredura ainda acusa o mesmo padrão plantado fora dessa pasta.
  Compilação: `make compile`. — V1.6, V1.7

- [x] Fase 47b — Resto fora do Rust (achado do fechamento do M2: `make english-check` no repositório dá 444
  achados): comentários de `.gitignore`, `app/.npmrc`, `app/vite.config.js`, `app/src-tauri/Cargo.toml`, `app/package.json`
  (`build:teste`), `app/src-tauri/capabilities/aprovacao.json` e dos scripts de `dev/` (`daemon.sh`, `bridge.sh`,
  `open-app.py`, `dev/tests/*.sh`). Nome que só muda depois (variáveis `WARD_*`, binários e pastas de teste: Fase 56;
  comandos e capabilities do Tauri: Fase 54; `dev/fixtures/entries.json`: Fases 48 e 53) vai para `frozen.txt` com a fase
  que o muda; comentário que só cita esse nome é reescrito sem ele. No fim, `make english-check` no repositório só acusa
  `dev/fixtures/entries.json` (da Fase 48). — V2.1

## Milestone 4 — Formato e protocolo novos

Contrato: V4.1–V4.24. Sequencial. A partir daqui o fio muda.

- [x] Fase 48 — Corpo v5 no `ward-core` (DT4). `BODY_VERSION` 5; nomes v5 em
  `name()`/`from_name()`; leitura do v4 por uma tabela fechada de conversão (só no
  caminho de leitura do v4); nome desconhecido recusa; a gravação só gera v5;
  `dev/fixtures/entries.json` com os nomes v5 (chaves em inglês, valores falsos iguais);
  testes novos (v4 → v5 igual em tudo, v5 com nome pt recusado, v4 com nome inglês
  recusado, byte alterado, vetores) escritos, não rodados. Atualiza o teste de fio do
  corpo (o v4 fica como leitura). Compilação: `make compile`. — V4.1, V4.2, V4.8, V4.9
- [x] Fase 49 — (achado da Fase 48: a API pública do corpo ainda não diz se o lido era v4; p.ex. `open_entries_with_generation` devolver a versão) Migração no daemon e estado v2 (DT5, DT6; o `dev/daemon.sh` grava `ward-estado 1` quando falta o estado e muda junto, achado da Fase 46). Destrave de um v4 → grava v5
  pelo caminho atômico, geração + 1, estado, backup; erro na gravação = destrave com
  erro e v4 intacto; sem regravação de v5; restaurar v4 grava v5; rotação local e do
  pendrive lendo v4 e v5; `ward-state 2` lendo `ward-estado 1`; log sem segredo. Testes
  do `wardd` (morte em cada ponto da gravação por pontos de falha de `test-mode`,
  retrocesso, idempotência) escritos, não rodados. Compilação: `make compile`. — V4.3,
  V4.4, V4.5, V4.6, V4.7, V4.10, V4.11, V4.12
- [ ] Fase 50 — (achado da Fase 48: os valores de tipo, nível e segurança SMTP já saem nos nomes v5 pelo `name()`/`from_name()`; as linhas de `protocol-v1.jsonl` com `servidor`/`livre`/`bloqueado` falham no `wire_v1` até esta fase) Protocolo v2 no `ward-core` (DT8). `VERSION` 2; operações, respostas,
  campos, erros e papéis pela tabela; campo desconhecido recusado; v1 →
  `unknown_version`; testes do protocolo (matriz de papéis igual, nenhum nome pt-BR,
  linhas de `protocol-v1.jsonl` recusadas) escritos. Compilação: só `cargo check` do
  `ward-core` (os clientes quebram até a Fase 51; o handoff diz isso). — V4.13, V4.14,
  V4.15
- [ ] Fase 51 — (achado da Fase 48: testes Rust que montam JSON com `banco`/`servidor`/`nota-segura`/`generico`/`bloqueado`/`com-aprovacao`/`livre`/`nenhuma` em `crates/wardd/tests/*`, `app/core`, `app/src-tauri/src/main.rs`, ponte, `ward-nm` e `ward` passam aos nomes v5; coluna TIPO do `ward list` por `label()`) Protocolo v2 nos clientes Rust: `wardd` (servidor, sessões, extensão,
  ponte), `ward` (conferência da versão antes da senha; `list --json` com chaves e
  valores em inglês e tabela com rótulos pt-BR: imprime `label()`, não `nome()`, achado da Fase 1), `ward-nm`, `ward-test-bridge`,
  `app/core` (com o `daemon_falso`), testes Rust que montam JSON à mão. Compilação:
  `make compile`. — V4.13, V4.16, V4.17, V4.18
- [ ] Fase 52 — (achado da Fase 48: o front-end `app/src` e `app/core/src/level.rs` ainda mandam `com-aprovacao`/`livre`/`banco`/`nenhuma`, que o `from_name` recusa desde a Fase 48; trocar pelos nomes v5 aqui ou na 54; `grep -rnE '"(banco|servidor|nota-segura|generico|bloqueado|com-aprovacao|livre|nenhuma)"'`) Protocolo v2 em JS e Python: `extension/` (native messaging; motivos de `fillInPage` e códigos locais do service worker, que dividem `FAILURES`/`SAVE_FAILURES` do popup com os do daemon, achado da Fase 39),
  `app/src/lib/bridge`, ponte de teste do front-end, `dev/tests/*.py` que falam com o
  socket ou com o `ward-nm` (`socket_matrix.py`, `socket_attack.py`, `nm.py`,
  `extension.py`, `login.py`, `cli.py`, `chrome_policy.py`, `open-app.py`,
  `fake_chrome.py`, `fake_ssh.py`), servidor falso v1 para o V4.16. Esvazia de
  `frozen.txt` as chaves de mensagem. Compilação: `make compile`. — V4.13, V4.16, V5.10
- [ ] Fase 53 — (achado da Fase 48: o `SMTP_SECURITY` entregue passa de `nenhuma` a `none`; registrar na tabela "Entrega esperada") Variáveis entregues (DT11) no `ward-core` `delivery` e no `wardd`;
  `make dev-vault` e `dev-vault-verify` com o `entries.json` novo (chaves lidas por
  `dev/variations.py` e `dev/tests/login.py`; heredoc do `dev-vault.sh` chama
  `entry_patterns`/`database_url`; achado da Fase 42); oráculo
  `dev/variations.py` (formas das variáveis novas); o parser da tabela "Cofre de teste"
  em `dev/tests/dev-vault.sh` com os nomes v5 deste contrato; expectativas de
  `dev/tests/cli.py` e dos testes do `wardd` pela tabela "Entrega esperada". Compilação:
  `make compile`. — V4.19, V4.20, V4.21
- [ ] Fase 54 — **(UI)** Comandos e eventos do Tauri e `serde` da webview: (tirar do `dev/english-check/frozen.txt` as 52 entradas da Fase 54 que a Fase 34 pôs para os comandos e parâmetros do `main.rs`; e as dos `#[serde(rename)]` de app/core das Fases 32–34) (procurar nome de fio como chave entre aspas e como acesso por
  ponto, achado da Fase 36; `frozen.txt` lista os da W) nomes das
  funções `#[tauri::command]` e dos parâmetros, eventos, `capabilities/*.json`,
  `permissions/`, chamadas `invoke`/`listen` no `app/src`, ponte de teste, tira os
  `#[serde(rename = "<antigo>")]`; lista do `strings` do V5.6 (missão 1) com os nomes
  novos. `frozen.txt` vazio e apagado. Compilação: `make compile`. — V4.22, V4.23, V4.24

## Milestone 5 — Nomes externos e documentação

Contrato: V5.1–V5.11. Sequencial.

- [ ] Fase 55 — Subcomandos da CLI (DT10): `list`, `lock`, `unlock`, `help`; sem alias;
  `ward --help` (texto pt-BR igual, só os nomes); mensagens que citam subcomando (CLI,
  daemon, app: "rode `ward unlock`"); `dev/tests/cli.py` e testes Rust. Compilação:
  `make compile`. — V5.1, V5.2
- [ ] Fase 56 — (também, achado da Fase 47b: o caminho `/cadastro` do serviço totp 8083 e o arquivo `aprovador.modo` do aprovador de teste, e as exceções de Fase 56 no `frozen.txt`) Variáveis internas e de teste e marca de `test-mode` (DT12): renomes com
  todos os usos (Rust, scripts, Makefile, units); recusa da marca nova e da antiga no
  `install/install.sh` e no `make release`; `dev/tests/no-test-mode.sh` procurando nomes
  novos e antigos; binários `wardd-app-teste`/`wardd-aprovador-teste`, texto de uso e
  mensagens `ward-ponte-teste:` da ponte (com o `dev/ponte.sh` aceitando as duas formas)
  e a mensagem "modo-teste" do `install.sh` (achados da Fase 4); pasta do socket de teste
  `ward-teste`, logs `wardd-*-teste.log` (achados da Fase 5); guarda `WARD_HOME_COBERTO`
  em `real-home.sh`, `ssh-home.sh` e `home-decoys.sh` de uma vez (achado da Fase 45); os
  `WARD_*`/`ward-teste*` dos scripts de `dev/` e o prefixo `ward-ponte-teste:` que o
  `dev/bridge.sh` procura (achado da Fase 46). Textos de log
  `UidRecusado`/`SemIdentidade` do `wardd` com o `dev/container-uid/inside.sh`; variáveis
  `WARD_ATIVACAO`, `WARD_CAMINHO`, `WARD_ESPERADO`, `WARD_PLATAFORMA_FILHO` (achados da Fase 9). Compilação:
  `make compile`. — V5.4, V5.5
- [ ] Fase 57 — Caminhos instalados e atualização (DT15, DT16): seis funções de `install/` que o
  `dev/tests/deb.sh` procura (`garantir_grupo`, `conferir_grupo`, `conferir_politicas_chrome`,
  `apagar_grupos_marcados`, `avisar_manifestos_de_usuario`, `avisar_core_pattern`) e o
  `grupos-criados` nos dois lados, tirando as entradas do `frozen.txt` (achado da Fase 47); `install/tree.txt`,
  `install.sh` (apaga `extensao/` e `grupos-criados` da 0.1.x, passa a marca para
  `created-groups`, para o `wardd@*` antes de trocar binários; uninstall lê as duas
  marcas), política do Chrome com o caminho novo, `install/deb/*` (preinst, postinst,
  prerm, postrm) e `install/deb.sh`; testes de contêiner (`dev/container-install`,
  `dev/container-deb`, `dev/tests/install.sh`, `deb.sh`) com os cenários de atualização a
  partir dos artefatos 0.1.4 de `.dev/baseline/`. Compilação: `make compile`. — V5.6,
  V5.7, V5.8, V5.9
- [ ] Fase 58 — Skill e deny (DT10): `skill/ward/SKILL.md` e `skill/README.md` com os
  comandos novos e `ward list --json` em inglês; `skill/settings-deny.json` negando
  `ward unlock` em todas as formas e mantendo `destravar`; `dev/tests/skill.py` (lista
  de ataques com as duas formas; autoteste). Compilação: `make compile`. — V5.3
- [ ] Fase 59 — (achado da Fase 49: a migração não guarda a v4 atual no backup antes de gravar e a rotação tira as v4 depois de 30 gravações; `docs/instalacao.md` manda copiar o cofre antes de atualizar, para a volta de DT7) Documentação: `docs/escopo.md` (nomes novos: comandos, variáveis,
  tipos, caminhos; o histórico das decisões fica), `docs/instalacao.md` (atualização
  0.1.x → 0.2.0, volta pelo backup v4, DT7), `docs/migracao.md` (comandos novos;
  `make test-migration-guide` com as expectativas novas), caminhos de arquivo em
  `docs/mission/design-nodes.md`, `README.md`, `.claude/agents/*.md` (nomes de alvo,
  caminho e arquivo). Compilação: `make compile`. — V5.11

## Milestone 6 — Validação final

Contrato: V6.1–V6.13 e a conferência de **todas** as asserções V1–V5. Sequencial. As
Fases 61–65 só rodam e registram (comando, saída resumida, evidência em
`docs/mission/evidence/M6/`). Não corrigem: cada falha vira uma fase de correção no fim
deste milestone, e a suíte roda de novo depois dela. Antes de build pesado: `df -h` e
limpeza de `target/` velho. Rajadas de processos com teto (`systemd-run --user
-p TasksMax=…`). No fim de cada fase: serviços de teste parados e worktrees limpos.
Depois das Fases 61–66 (e das correções), o orchestrator chama os três validadores.

- [ ] Fase 60 — Gate e versão: `english-check` (com o autoteste) dentro do `make check`;
  achados restantes no repositório inteiro corrigidos ou em `allow.txt` com motivo;
  versão 0.2.0 (DT19); comparação dos literais com `strings-0.1.4.txt` e lista da
  diferença no handoff; procura dos nomes antigos (V6.2). Compilação: `make compile`. —
  V6.1, V6.2, V6.3
- [ ] Fase 61 — Bateria A (estática e unitária): `make check`, `make app-check`,
  `make test-no-test-mode`, `make test-skill`, `make test-migration-guide`,
  `make dev-vault` + `dev-variations` + `test-dev-vault`,
  `make test-attacks-foundation`, testes de fio no commit do fim do M3 (V2.2),
  `english_check.py --self-test`. — V1.*, V2.1, V2.2, V3.1, V3.6, V3.7, V4.1, V4.2,
  V4.8, V4.9, V4.15, V5.3, V5.5, V5.11, V6.1, V6.11
- [ ] Fase 62 — Bateria B (daemon, CLI e serviços): `make dev-services` +
  `dev-services-up` + `dev-services-verify`, `make dev-daemon`,
  `make test-attacks-services`, `test-home-decoys`, `test-real-home`, `test-cli`,
  `test-ssh-home`, `test-socket-attacks`, `test-docker-uid`; V6.6, V6.7, V6.10 à mão
  pela CLI de teste. — V2.4–V2.8, V4.13, V4.14, V4.16–V4.21, V5.1, V5.2, V6.4, V6.6,
  V6.7, V6.10
- [ ] Fase 63 — Bateria C (contêineres e instalação): `make test-docker-platform`,
  `test-units`, `test-install`, `test-deb` (com a atualização a partir da 0.1.4). —
  V2.3, V2.6, V2.7, V4.4, V5.4, V5.6–V5.9, V6.4
- [ ] Fase 64 — (achado da Fase 49: os scripts de dev agora criam `ward-state 2`, que o `wardd` 0.1.4 recusa; para subir o 0.1.4 use `ward-estado 1`, p.ex. o fixture `state-0.1.4`; a parte em contêiner do V4.4 (`inotifywait`/`strace`) entra aqui ou no `test-units`) Bateria D (migração de ponta a ponta e versões misturadas): `wardd`
  0.2.0 de teste com cópias de `vault-0.1.4.ward`, `state-0.1.4` e `backup-0.1.4/` em
  `.dev/`; migração comparada com o `wardd` 0.1.4 (V4.5); morte em cada ponto (V4.3);
  observação do disco (V4.4); retrocesso (V4.7); restauração e rotação (V4.10); estado
  (V4.11); `wardd` 0.1.4 no cofre migrado (V4.12); CLI 0.1.4 × daemon 0.2.0 e CLI 0.2.0
  × daemon 0.1.4 (V4.16, V4.17). — V4.3–V4.7, V4.10–V4.12, V4.16, V4.17
- [ ] Fase 65 — Bateria E (navegador e app) **(UI)**: `make app-front`, `test-nm` (com a
  extensão 0.1.4 também, V5.10), `extension`, `test-extension`, `test-login`,
  `test-playwright`, `test-chrome-policy`, `test-webkit-real`, `test-app-e2e`,
  `test-clipboard`, `test-revealed-selection`, `test-app-logs`, `test-gst-channels`;
  capturas de cada tela comparadas com `docs/mission/arquivo/m1/evidence/final/`. —
  V3.2–V3.5, V4.22–V4.24, V5.10, V6.4, V6.5, V6.8, V6.9
- [ ] Fase 66 — Artefatos 0.2.0 e roteiro do usuário: `make release`, `make extension`,
  `make extension-crx` (chave `.dev/extension/key.pem`, id conferido), `make deb
  EXTENSION_ID=… EXTENSION_CRX=…`; `docs/mission/roteiro-usuario.md` (instalar por cima
  da 0.1.4, primeiro destrave pelo app ou por `/usr/local/bin/ward unlock` = migração,
  conferir as entradas, atualizar a skill e o deny em `~/.claude` a partir de `skill/`,
  janela nativa, bandeja e aprovação, e como voltar pela versão v4 do backup).
  Publicar a release e instalar são do usuário. — V6.12, V6.13
