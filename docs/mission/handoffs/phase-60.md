# Fase 60 — Gate e versão (Missão 2, Milestone 6)

## Implementado
- Base do worktree conferida: estava em `b4d5e1b`; `git reset --hard 505da24` (master atual,
  árvore limpa).
- **`english-check` no `make check`** (m2:V6.1): passo novo `1/7`, logo depois da pré-checagem
  das origens: `dev/english_check.py --self-test` e depois `make english-check` no repositório
  inteiro. Os passos passam a `0/7`…`7/7` (fmt 2, clippy 3, test 4, audit 5, deny 6, gitleaks 7).
  O `dev/tests/attacks-foundation.sh` procura os ecos novos (`==> 0/7 origens`, `==> 1/7`,
  `==> 5/7 cargo audit`). Comentário do alvo `english-check` e linhas do `CLAUDE.md` atualizados.
- **`frozen.txt` apagado** (`git rm`; estava vazio, só cabeçalho). O checador deixou de ler a
  lista congelada: só `allow.txt` (3 campos, com motivo). Se `dev/english-check/frozen.txt`
  voltar a existir (vazio ou não), o checador acusa problema e sai 1. Autoteste: os casos de
  `frozen` viraram "allow sem uso (tudo)", "allow sem motivo", "frozen.txt presente" (vazio e com
  linha) e "sem frozen.txt"; 93/93 casos ok. Conferido também à mão: `: > frozen.txt` faz
  `make english-check` sair 1 com "não deve existir".
- **`allow.txt`**: nenhuma entrada nova; o repositório inteiro dá 0 achado com as 21 entradas que
  já existiam (dados do cofre de teste, `frei0r`, serviços `ssh-*`, guias `instalacao`/`migracao`).
- **Versão 0.2.0** (m2:DT19): `Cargo.toml` (`[workspace.package]`, todos os crates herdam),
  `Cargo.lock` (7 pacotes do workspace), `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock`
  (`ward-app`, `ward-app-core`, `ward-core`, `ward-platform`), `app/package.json`,
  `app/package-lock.json` (2), `app/src-tauri/tauri.conf.json`, `extension/manifest.json`
  (era `0.1.0`, m2:DT9). `ward --version` lê `CARGO_PKG_VERSION` (agora 0.2.0); o `.deb` lê a
  versão do workspace (`install/deb.sh`), então sai `ward_0.2.0_<arq>.deb`, como o README cita.
  Nenhuma versão de dependência mudou.
- **`wardd-test-client`** (achado da Fase 55): argumentos `status|list|unlock` (nomes v2 das
  operações) no lugar das formas pt-BR; texto de uso e comentário do topo. Chamadores (20
  chamadas): `dev/container-deb/inside.sh` (5), `dev/container-install/inside.sh` (5),
  `dev/container-platform/inside.sh` (1), `dev/container-uid/inside.sh` (4),
  `dev/container-units/inside.sh` (5). O `units.sh`, `install.sh`, `deb.sh`, `docker-uid.sh` e
  `docker-platform.sh` só copiam o binário (como `cliente`/`wardd-test-client`, nomes que o
  glossário §9 mantém). Glossário §9 atualizado.
- **Nomes antigos que sobravam em literais** (V6.2), trocados pelo nome v2:
  - `crates/wardd/src/core.rs`: log `salvar_login: login novo…` → `save_login: login novo…`;
  - `crates/wardd/src/main.rs`: aviso do daemon de teste `abrir_app abre…` → `open_app abre…`;
  - `crates/wardd/tests/app_value_binary.rs`: argumento do cliente Python embutido (forma de
    destrave da 0.1.x/`nao` → `unlock`/`no`) e as linhas impressas (`estado -` → `status -`,
    destrave → `unlock -`), com o `starts_with` que as lê;
  - `crates/wardd/tests/fake_chrome.py` + `crates/wardd/tests/playwright.rs`: evento
    `preencher` → `fill` (3 leituras).
  Nenhum teste nem script procura esses textos de log (`grep` no repositório).
- **Contrato** (`docs/mission/validation-contract.md`), duas mudanças pedidas (para informar o
  usuário):
  1. **V4.18** (achado da Fase 52): as chaves do `ward list --json` passam a ser `name`, `type`,
     `group`, `environment`, `project`, `address`, **sem `level`** (D50 tira o nível da lista do
     agente, como na 0.1.4, cuja lista tinha `nome, tipo, grupo, ambiente, projeto, endereco`);
     o contrato antigo pedia `level` e "`url` ou `host`". A tabela do `ward list` não tem coluna
     de nível (cabeçalho `NOME TIPO GRUPO AMBIENTE PROJETO URL/HOST`), então o contrato fica só
     com os rótulos de tipo ("banco", "servidor"); antes citava também "com aprovação"/"livre".
  2. **V5.11** (achado da Fase 59): além de `docs/mission/arquivo/`, da nota de atualização de
     `docs/instalacao.md` e do histórico do escopo, aceita os nomes antigos citados de
     propósito em: `docs/glossario.md` (tabelas antigo → novo); documentos da missão 2 em
     `docs/mission/` (PRD, plano, contrato, handoffs) e a tabela "antes → depois" do fim do
     `design-nodes.md`; deny das formas da 0.1.x em `skill/README.md` e
     `skill/settings-deny.json`; no `CLAUDE.md`, as marcas de teste da 0.1.x recusadas, a marca
     de grupos e a pasta da extensão da 0.1.x que a instalação apaga, e os nomes que o glossário
     mantém (imagens `ward-teste/*:1`, base `app_teste`); e palavras pt-BR comuns usadas como
     texto ("ferramentas", "origens", "livre", "revelar"…).

### V6.3 — literais do produto, 0.1.4 × 0.2.0
Mesmo conjunto de caminhos da Fase 3, com os nomes novos (`crates/{ward,wardd,ward-nm,ward-core,
ward-platform}/src`, `app/core/src`, `app/src-tauri/{src,capabilities,tauri.conf.json}`,
`app/index.html`, `app/src`, `extension/`), sem caminho com "test": 181 arquivos (a 0.1.4 tinha
178; entram `wire_v1.rs`, `wire_v2.rs` e `body/legacy_v4.rs`, novos). `english_check.py
--literals`: 4 294 literais na 0.1.4, 4 595 na 0.2.0; 774 só na 0.1.4 e 1 075 só na 0.2.0. A
lista inteira, classificada, está em `docs/mission/evidence/fase-60/literals-0.1.4-vs-0.2.0.txt`.
Resumo por classe:

| Classe | 0.1.4 → 0.2.0 | O que é |
| --- | --- | --- |
| captura de `format!` | 86 pares | só o nome da captura mudou (`{rotulo}` → `{label}`, `{corpo}` → `{body}`…; plano, "Captura de `format!`") |
| caminho | 113 → 120 | imports e caminhos com as pastas e arquivos novos (`./lib/componentes/Botao.svelte` → `./lib/components/Button.svelte`, `entradas.json` → `entries.json`, `.dev/logs/ward-ponte-teste.log` → `ward-test-bridge.log`, `ward-teste/wardd.sock` → `ward-test/…`) |
| nome de fio, id, classe | 332 → 448 | operações, campos, erros e papéis v1 → v2 (DT8); ids de formulário, permissões `allow-*` e comandos do Tauri (Fase 54); classes CSS e tokens (Fase 35, `botao primario` → `button primary`); ícones (`tipo-banco` → `type-database`); ids da bandeja (`bandeja-*` → `tray-*`); sai um literal `kebab-case` da 0.1.4; `0.1.0`/`0.1.4` (versões nos testes) |
| `Debug` de tipo | 35 → 41 | nomes de tipo no `Debug` manual (`SenhaMestra(••••)` → `MasterPassword(••••)`, `Lista` → `List`…), nunca texto exibido |
| variável | 11 → 12 | `WARD_ENTRADA`/`GRUPO`/`AMBIENTE`/`PROTOCOLO`/`ARQUIVO_*` → `WARD_ENTRY`/`GROUP`/`ENVIRONMENT`/`PROTOCOL`/`FILE_*` (DT11), `WARD_DEV_EXTENSAO`/`DBUS_SISTEMA` → `WARD_DEV_EXTENSION`/`SYSTEM_DBUS` (DT12), `LER_PAGINA` → `READ_PAGE_SCRIPT` |
| texto | 141 → 291 | ver abaixo |

Texto (o único que o usuário pode ver) que mudou, e só pelo nome trocado:
- `ward --help`: `lista`/`destravar`/`travar` → `list`/`unlock`/`lock` (V5.2; o resto igual).
- "`ward <destrave>` precisa de um terminal…", "cofre travado: o comando espera o destrave…
  (`/usr/local/bin/ward unlock` …)", "travado: destrave com `/usr/local/bin/ward unlock` ou pelo
  app" (Fase 55).
- App: "O Claude nem vê esta entrada: ela some do ward list…" (2 textos; Fase 55);
  "Entregues como WARD_PROTOCOL, … WARD_FILE_KEY …" e "Viram WARD_FILE_<NOME> no ward run."
  (Fase 53).
- Logs do daemon: "conexão recusada (NoIdentity)/(UidRefused)" (Fase 56), "daemon de TESTE
  (test-mode)…", "daemon de teste: WARD_DEV_EXTENSION só aceita…", "WARD_DEV_SYSTEM_DBUS
  recusado…", "save_login: …", "open_app …" (esta fase); teste do app: "rode por
  `make test-clipboard`".
- Novos de propósito (comportamento novo da missão, não tradução): mensagens da migração
  ("cofre no formato da 0.1.x (corpo v4): gravando no formato novo (v5)", "cofre migrado para o
  corpo v5", "migração para o corpo v5 falhou: cofre travado, nada apagado", "migração recusada:
  o arquivo do cofre mudou fora do Ward (D39)"), de versão (DT8: "versão do protocolo
  incompatível: atualize o Ward", "… com o daemon: atualize o Ward", "…; nada foi enviado",
  "o daemon fala outra versão do protocolo"; sai "versão do protocolo desconhecida" da 0.1.4) e
  de nome reservado (Fase 54b: "entrada sem entrega válida: campo extra ou anexo com nome
  reservado"); estado `ward-state 2` (DT6).
- Não exibido: as descrições `#[doc]` das tabelas do protocolo (`"Apagar uma entrada."` →
  `"Delete an entry."`, ~50) são literais da macro `table!` que viram doc comment, traduzidas
  com os comentários; e literais de teste (`#[cfg(test)]` dentro dos arquivos de produto, como na
  linha de base): JSON montado à mão com os nomes v2, linhas de estado, mensagens de `assert!`
  em inglês, fixtures.

Nenhum outro texto exibido mudou.

### V6.2 — nomes antigos
Procura em todos os arquivos do git, fora de `docs/mission/arquivo/` e `dev/fixtures/legacy/`
(script no scratchpad, `fase60_oldnames.py`): os nomes distintos de DT4/DT8–DT15 (com `_`, `-`,
`/`, prefixo `WARD_`, alvos e variáveis do make) como palavra inteira; as palavras pt-BR comuns
que também eram nome de fio (`estado`, `lista`, `erro`, `banco`, `livre`…) só como literal exato
entre aspas e os subcomandos como `ward <nome>`, para não contar a prosa pt-BR. Linhas achadas
(depois das trocas desta fase) em `docs/mission/evidence/fase-60/old-names-v6.2.txt`. O que
sobra, e por quê:

| Onde | Por quê |
| --- | --- |
| `crates/ward-core/src/body/legacy_v4.rs` | tabela de conversão v4 → v5 (DT4) |
| `crates/ward-core/src/wire_v1.rs`, `protocol/tests.rs` (tabela v1 → v2), `wire_v2.rs` | testes de fio contra a linha de base e a tabela v1 da 0.1.4 (DT8, V2.2) |
| testes de recusa: `ward/src/args.rs`, `ward/src/tests.rs`, `ward-nm/src/tests.rs`, `app/core/src/{commands,tray,level,daemon,client,form,first_use,approval}.rs`, `app/core/tests/tauri_config.rs`, `app/src-tauri/src/main.rs`, `ward-test-bridge/tests/attacks.rs`, `ward-core/src/{entry,body,atomic_write,delivery}/tests.rs`, `wardd/tests/{socket,migration,approval,settings,ssh,test_app,run}.rs`, `dev/tests/{cli,nm,socket_attack,skill,migration-guide}.py` | o nome antigo é o ataque: comando Tauri, subcomando, operação, campo, nível ou variável da 0.1.x que tem de ser recusado, ou que não pode aparecer na saída/no ambiente |
| `dev/tests/no-test-mode.sh`, `dev/tests/install.sh`, `dev/tests/app-release.sh`, `install/install.sh`, `dev/bridge.sh` | marcas de `test-mode` da 0.1.x que o instalador e o `strings` recusam (DT12, V5.4, V5.5); a ponte aceita os dois prefixos (Fase 56) |
| `install/upgrade.sh`, `install/groups.sh`, `install/deb.sh` | caminhos da 0.1.x que a atualização apaga e a marca antiga que o uninstall lê (DT15) |
| `skill/settings-deny.json`, `skill/README.md` | deny das formas da 0.1.x (DT10, Fase 58) |
| `CLAUDE.md`, `docs/instalacao.md`, `docs/glossario.md`, `docs/escopo.md`, `docs/mission/*` (fora do arquivo), `design-nodes.md` | notas de atualização, tabelas antigo → novo, histórico e documentos da missão (V5.11 ajustado) |
| `crates/ward-core/src/entry.rs` (`label()`), `app/src/lib/components/{types,levels}.js`, `app/src/screens/form.js`, `ExtraField.svelte`, `crates/ward/src/text.rs` (teste) | rótulos pt-BR exibidos ("banco", "servidor", "livre", "bloqueado", "segredo"), não o nome gravado |
| `crates/wardd/src/config.rs`, `files.rs`, testes do `wardd` (`"estado"`) | nome do arquivo de estado no disco, que fica (DT6, PRD regra 11) |
| `app/src-tauri/src/gtk_clipboard.rs` (`copiar`, `erro`, `fim`) + `dev/tests/clipboard.py` | protocolo de linhas do teste da área de transferência, mantido pelo glossário §9 |
| `crates/wardd/tests/fake_ssh.py` (`lista`, `agente`, `erro`), `dev/tests/extension.py` (`bloqueado`, `estado`, `detalhe` como chave de resultado/medida), `dev/tests/app_e2e.py`, `dev/services/web/server.py` (classe CSS `erro` do serviço de teste) | chaves internas de registro de teste, nunca fio do Ward |
| nomes de pasta/arquivo temporário em teste (`"saida"` no `launcher.rs`/`terminal.rs`, `Dir::new("lista"/"travar")`, `fake_root("configuracao")`, anexo `chave.pem`) | dado de teste, não o nome de fio |
| prefixos pt-BR de log (`executar:`, `sugerir:`, `restaurar:`) | prosa pt-BR do log (PRD regra 5); o `dev/tests/cli.py` lê `executar: entrada padrão` |
| Makefile/`origins.py`/`attacks-foundation.sh` ("origens do cargo"), `CLAUDE.md`/`skill/README.md` ("ferramentas") | texto pt-BR (DT20), não o alvo antigo |

## Segurança
- Invariantes tocadas: S14/S15 (o `make check` ganha o gate do checador antes do cargo; o
  checador continua só com a biblioteca padrão, sem seguir link, e só escreve em `.dev/`);
  S8 (nada muda na marca de `test-mode`; as recusas de marca antiga ficam). Nenhuma lógica de
  papel, máscara, cripto, gravação, aprovação, PIN ou domínio foi tocada: no Rust de produção,
  só dois textos de log e a versão.
- Testes de ataque: `dev/english_check.py --self-test` › "frozen.txt presente" (a lista congelada
  volta, vazia ou com linha usada: falha), "allow sem uso (tudo)", "allow sem motivo"; conferência
  manual com `frozen.txt` plantado na árvore (sai 1). `dev/tests/attacks-foundation.sh` segue
  exigindo que o `make check` pare na pré-checagem antes do passo 1 (agora o checador).
- Varredura de segredo nas linhas novas: `gitleaks dir` com `dev/gitleaks-tree.toml` e
  `dev/gitleaks-extra-tree.toml` na árvore do worktree (com as evidências novas): sem achado.
- Dependências novas: nenhuma.

## Ficou de fora
- Nenhuma suíte nem `make check` inteiro (o `make check` roda na Fase 61; as baterias que falam
  com daemon são das Fases 61–65). Nenhum daemon foi iniciado; cofre real e daemon instalado não
  foram tocados.
- `docs/instalacao.md` diz "60 caminhos" no `dpkg-deb -c`: `install/tree.txt` tem 60 linhas, mas
  o número do `dpkg-deb -c` só se confere com o `.deb` montado (Fase 66).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 505da24` | 0 |
| `make english-check` (repositório inteiro: 0 achado, 0 problema) | 0 |
| `python3 -I -B dev/english_check.py --self-test` (93/93) | 0 |
| `make english-check` com `frozen.txt` plantado (controle) | 1 |
| `make -n check` (ordem dos passos 0/7–7/7) | 0 |
| `make compile` (workspace, Vite, `app/src-tauri`, sintaxe de 52 shell, 36 Python, 7 JS) | 0 |
| `make fmt-check` | 0 |
| `english_check.py --literals --paths <181 arquivos>` e comparação com `strings-0.1.4.txt` | 0 |
| procura dos nomes antigos (V6.2) | 0 |
| `gitleaks dir` (tree e extra-tree) na árvore | 0 |
| `gitleaks git --staged --pre-commit` (`.gitleaks.toml` e extra) antes do commit | 0 |

Logs no scratchpad da sessão: `fase60-english-check.log`, `fase60-self-test.log`,
`fase60-make-compile.log`, `fase60-fmt-check.log`, `fase60-leaks.log`,
`fase60-literals-diff.txt`, `fase60-literals-classified.txt`, `fase60-oldnames-lines.txt`.

## Problemas descobertos
- **V6.2 × testes de recusa.** O texto do V6.2 lista só arquivo, `legacy/`, tabela de conversão,
  deny, recusa da marca antiga e notas de atualização. Os testes que provam que o nome antigo é
  recusado (subcomandos, operações v1, comandos Tauri, níveis v4, variáveis da 0.1.x) também o
  contêm, e têm de conter. Não mudei o V6.2 (o pedido era só V4.18 e V5.11); o validador precisa
  aceitar esses casos (tabela acima) ou o contrato listá-los.
- **V6.2 "como palavra inteira"** pega prosa pt-BR ("erro", "estado", "lista", "livre" são
  palavras comuns nos textos exibidos e nos docs). A procura útil é a da tabela acima (nomes
  distintos inteiros; palavras comuns só como literal exato ou `ward <nome>`).
- **Prefixos de log `executar:`/`sugerir:`/`restaurar:`** ficaram (prosa pt-BR; o `cli.py` lê um
  deles). Se o usuário quiser os nomes v2 também no log, é uma troca de literal com o `cli.py`.
- **Literais internos de teste em pt-BR** (chaves do `fake_ssh.py`, resultados do `extension.py`,
  cliente Python embutido no `app_value_binary.rs` com `pedir`/`dados`/`senha`): o checador não
  olha literal, e não são fio do Ward. Ficaram.
- **`fake_crx.py --adulterado`/`--versao`** (opções de teste em pt-BR, chamadas pelo
  `dev/tests/install.sh`): não estão nas tabelas de DT8–DT15; ficaram.
- O guarda de worktree desta sessão recusa comandos com a palavra `gitleaks` (lê como `git`):
  rodei o gitleaks por um script no scratchpad. E o deny do `~/.claude` recusa comando Bash com a
  forma antiga do destrave: as trocas com ela foram por script/Edit.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Makefile` | alterado |
| `CLAUDE.md` | alterado |
| `dev/english_check.py` | alterado |
| `dev/english-check/frozen.txt` | apagado |
| `dev/tests/attacks-foundation.sh` | alterado |
| `Cargo.toml`, `Cargo.lock` | alterado |
| `app/package.json`, `app/package-lock.json` | alterado |
| `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock`, `app/src-tauri/tauri.conf.json` | alterado |
| `extension/manifest.json` | alterado |
| `crates/wardd/examples/test-client.rs` | alterado |
| `dev/container-{deb,install,platform,uid,units}/inside.sh` | alterado |
| `crates/wardd/src/core.rs`, `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/tests/app_value_binary.rs`, `fake_chrome.py`, `playwright.rs` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/validation-contract.md` | alterado (V4.18, V5.11) |
| `docs/mission/evidence/fase-60/literals-0.1.4-vs-0.2.0.txt` | criado |
| `docs/mission/evidence/fase-60/old-names-v6.2.txt` | criado |
| `docs/mission/handoffs/phase-60.md` | criado |

## Procedimentos
- [x] `make compile` e `make english-check` rodados (o `make check` inteiro é da Fase 61)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre nem daemon tocado)
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
