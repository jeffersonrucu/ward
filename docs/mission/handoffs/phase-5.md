# Fase 5 — Pastas e arquivos fora dos crates e do `app/src` (DT13, DT1) (Missão 2, Milestone 1)

## Implementado
- **Pastas e arquivos, por `git mv`** (glossário, seções 4 e 5):

  | Antigo | Novo |
  |---|---|
  | `instalacao/` | `install/` |
  | `instalacao/{arvore.txt,instalar.sh,grupos.sh,despejo.sh,empacotar-extensao.sh}` | `install/{tree.txt,install.sh,groups.sh,dump.sh,pack-extension.sh}` |
  | `extensao/` | `extension/` |
  | `extensao/{chave-publica.txt,servico.js,pagina.js,envio.js}` | `extension/{public-key.txt,service.js,page.js,submit.js}` |
  | `dev/extensao/montar.py` | `dev/extension/build.py` |
  | `dev/{ambiente.sh,origens.py,variacoes.py,ponte.sh,abrir-app.py}` | `dev/{env.sh,origins.py,variations.py,bridge.sh,open-app.py}` |
  | `dev/gitleaks-arvore.toml`, `dev/gitleaks-extra-arvore.toml` | `dev/gitleaks-tree.toml`, `dev/gitleaks-extra-tree.toml` |
  | `dev/fixtures/entradas.json`, `dev/fixtures/anexos/` | `dev/fixtures/entries.json`, `dev/fixtures/attachments/` |
  | `dev/conteiner-{deb,install,plataforma,uid,units}/` | `dev/container-{deb,install,platform,uid,units}/` |
  | `dev/conteiner-*/dentro.sh`, `conteiner-units/iniciar.sh` | `inside.sh`, `start.sh` |
  | `dev/conteiner-plataforma/{falso,marca}.c` | `dev/container-platform/{fake,mark}.c` |
  | `dev/servicos/` (`comum.sh`, `conferir.sh`, `gerar.py`, `ssh/entrada.sh`, `web/servidor.py`, `wordpress/instalar.sh`) | `dev/services/` (`common.sh`, `verify.sh`, `generate.py`, `ssh/entrypoint.sh`, `web/server.py`, `wordpress/install.sh`) |
  | `dev/testes/` e os 26 arquivos da tabela do glossário (`ataques-fundacao.sh` → `attacks-foundation.sh`, `sem-modo-teste.sh` → `no-test-mode.sh`, `socket_ataque.py` → `socket_attack.py`, `migracao.py` → `migration-guide.py` …) | `dev/tests/` |
  | `app/design/icones/` | `app/design/icons/` (só a pasta; ver "Ficou de fora") |

- **Usos ajustados** (script de substituição com regras fechadas, mais correções à mão e revisão linha a linha das ocorrências):
  - Makefile (só caminhos: receitas e comentários; nomes de alvo e de variável ficam para a Fase 6), `CLAUDE.md`, `README.md`, `docs/instalacao.md`.
  - Todos os scripts de `dev/`, `install/`, `extension/` (imports `./page.js`, `service_worker: service.js`, `content_scripts: submit.js` do manifesto), Dockerfiles, `dev/compose.yml` (`./services/…` e `../.dev/services/{secrets,keys}/…`).
  - Rust: `include_str!`/`concat!` de `dev/fixtures/entries.json` e `dev/variations.py`, `crates/wardd/tests/units.rs` (`install/systemd`), `ward-test-bridge` (`dev/tests/terminal-test.py`), comentários.
  - `dev/english_check.py`: plantio JS do autoteste em `extension/` (o glob antigo não casaria mais).
  - `app/scripts/gerar_design.py` e o comentário gerado em `app/src/lib/componentes/icones.js` (`app/design/icons/`).
  - Formas compostas que não têm o caminho inteiro: `os.path.join(RAIZ, "extension", …)`, `"dev", "tests", "fake_v6.py"`, `RAIZ / ".dev" / "services"`, `DESTINO / "keys"|"secrets"|"client-home"`, `"$DS/keys"`, `SEG="$DS/secrets"`, `"$C/repo/install"`.
- **Caminhos de `.dev/`** (glossário, seção 5) em scripts, Makefile, `CLAUDE.md` e no `config.rs` de teste do `wardd` (`$WARD_DEV_DIR/test-vault.ward` e `test-vault.state`; o da release não muda): `test-vault.ward`/`.state`, `variations.txt`, `services/{keys,secrets,client-home}`, `extension/key.pem`, `tools/`, `clipboard/`, `revealed-selection/`, `gst-channels/`, `log-scan-self-test/`, `.test-dev-vault.lock`, prefixos `chrome-policy.`, `socket-attacks.`, `test-deb.`, `test-install.`, `container-*.`, `extension.`, `test-attack-*`, `e2e-mark`.
- **`mv` dos arquivos existentes em `.dev/`** (nada copiado, nada recriado): `cofre-teste.ward` → `test-vault.ward`, `cofre-teste.estado` → `test-vault.state`, `variacoes.txt` → `variations.txt`, `servicos/` → `services/` (com `chaves` → `keys`, `segredos` → `secrets`, `home-cliente` → `client-home`), `ferramentas/` → `tools/`, `area-transferencia/` → `clipboard/`, `.teste-dev-cofre.trava` → `.test-dev-vault.lock`, `logs/ward-ponte-teste.log` → `logs/ward-test-bridge.log` (não havia log novo). Modos e donos mantidos.
- **Chave da extensão (V1.9):** `mv -n .dev/extensao/chave.pem .dev/extension/key.pem` (pasta nova 0700), `.dev/extensao/` removida (vazia). Conferido sem imprimir a chave: arquivo comum, 0600, do usuário, não é link; id calculado (sha-256 do DER da pública, 32 dígitos `a`–`p`) = `nhkehjnmlgkebhmelajfpmpblepaichf` antes e depois do `mv`, igual ao `ID_EXTENSAO` do `ward-nm`, ao id de `extension/public-key.txt` e ao da `key` do manifesto 0.1.4 em `.dev/baseline/extension-0.1.4/`. Nenhuma chave gerada.
- **Evidência dos scripts (DT1):** `docs/mission/evidence/fase-41` → `evidence/test-extension`, `fase-42` (estados do popup, outro `qa.json`) → `evidence/test-extension/popup`, `fase-43` → `evidence/test-login`, `fase-39c` → `evidence/test-app-e2e` (`extension.py`, `extension.sh`, `login.py`, `app_e2e.py`, Makefile, `CLAUDE.md`). Nenhum script lê referência das pastas antigas.
- **Limpeza da Fase 4:** apagados `target/debug/ward-ponte-teste` e os `libward_ponte_teste.*` de `target/debug`.
- **Glossário:** linhas novas para `app/design/icons/`, `target/extension/ward.crx`, `mark.c` gerado pelos testes, evidência `fase-42`/`fase-39c`; nota de que a pasta do socket de teste e os logs `wardd-app-teste`/`wardd-aprovador-teste` mudam na Fase 56.
- **Literais alterados (PRD regra 5).** Todos são caminho ou nome de arquivo:
  - Rust: `crates/wardd/src/config.rs` (`"test-vault.ward"`, `"test-vault.state"`, só com `test-mode`, e o teste); `crates/ward-core/examples/dev-cofre/{main,testes}.rs` (`.dev/test-vault.ward`, `.dev/services/keys`, `dev/fixtures/{entries.json,attachments}` e os casos de recusa); `crates/ward-core/src/mascara/testes.rs` (`dev/variations.py`); `crates/wardd/tests/{app_valor,backup,units}.rs`; `crates/ward-test-bridge/src/simulados.rs`, `tests/ataques.rs` e a mensagem `"ward-ponte-teste: sem dev/tests/terminal-test.py …"` de `main.rs` (prefixo da marca mantido); `app/core/src/{cliente,servico,daemon_falso}.rs` (`dev/fixtures/entries.json`).
  - `install/install.sh`: mensagens que citam o arquivo da árvore (`"tree.txt vazia"`, `"tree.txt: linha inválida …"` …) e o uso `"uso: install.sh instalar|desinstalar"`; `install/dump.sh` (`install/dump.sh` na mensagem); `install/pack-extension.sh` (`target/extension/ward.crx`, `extension/dist`, `.dev/extension/key.pem`, uso). O texto da recusa de `modo-teste` e as marcas não mudaram.
  - `extension/manifest.json`: `"service_worker": "service.js"`, `content_scripts` `"submit.js"`; imports `./page.js` (campos do Chrome iguais; o id não depende dos nomes dos arquivos).
  - `dev/`: mensagens de uso e de erro que citam o próprio script (`uso: clipboard.sh <cargo>`, `fake_crx.py não deu um id`, `inside.sh roda como root…`, `server.py phish|totp|redir` …), caminhos e prefixos de `.dev/`, `.dev/<pasta>/mark.c` gerado por `gst-channels.sh` e `webkit-real.sh`, os casos de recusa do `variations_test.py` (`/tmp/variations.txt` …).
- **Exceções:** nenhuma entrada nova em `allow.txt` nem em `frozen.txt` (as duas de `dev/fixtures/*/gama-generico` continuam casando em `attachments/`).

## Segurança
- **Invariantes tocadas:**
  - **S15 / V1.9:** chave da extensão só por `mv`, 0600, id conferido igual (acima). Nunca impressa.
  - **S2:** os `.dev/` movidos continuam 0600/0700 (`services/` 0700, segredos 0600). Nada em claro saiu de `.dev/`.
  - **S8 / V9.2 (marcas de `test-mode`):** a pasta do socket de teste **não** mudou (`ward-teste`, `NOME_DIRETORIO_TESTE`). O nome é a marca `ward-teste` que `install/install.sh` e `app-release.sh` procuram nos binários; trocar agora deixaria um binário `test-mode` sem essa marca até a Fase 56. Ficou de propósito (PRD regra 7).
  - **S14:** nenhuma dependência nova.
  - Lógica de segurança intocada: diff só de caminhos e comentários nos scripts de recusa (`install.sh`, `pack-extension.sh`, `no-test-mode.sh`, `dev-vault.sh`, `origins.py`).
- **Testes de ataque:** nenhum novo nesta fase (renomes de caminho). Os ataques existentes que dependem dos caminhos foram ajustados junto: `attacks-foundation.sh` (gitleaks em `.dev/tools/`, `dev/origins.py`, `.dev/test-vault.ward`), `dev-vault.sh` (saída fora de `.dev/`, links `.dev/test-attack-*`, flock `.dev/.test-dev-vault.lock`), `attacks-services.sh` (modos de `.dev/services/{keys,secrets}`), os casos de recusa do `dev-cofre` (`testes.rs`). Escritos, não rodados (regra 6).
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Nomes dos `.svg` em `app/design/icons/`** (`cadeado.svg`, `tipo-banco.svg` …): são as chaves de `ICONES` em `icones.js`, usadas em 78 lugares do `app/src` e da extensão, algumas montadas do nome gravado do tipo (`tipo-${tipo}`); `ICONES[nome] ?? []` some com o ícone sem erro se uma chave faltar. Vão com a Fase 35 (chaves) e a 48 (nomes de tipo). Únicos achados de nome do `english-check` nos caminhos da fase.
- **Pasta do socket de teste `$XDG_RUNTIME_DIR/ward-teste/`** (e `ward-teste-e2e`, `ward-teste-selecao`): Fase 56, com a marca (ver Segurança).
- **`.dev/logs/wardd-aprovador-teste.log`** (e `wardd-app-teste.log`): saem do nome do binário; mudam com ele na Fase 56. Não movidos.
- **`target/sem-modo-teste/`**: não está no glossário; é usado por Makefile, `CLAUDE.md` e scripts. Fica para a Fase 6 (Makefile) ou 56.
- **`.dev/logs/terminal-teste.{log,saida}`, `.dev/terminal-teste.modo`, `.dev/e2e-guarda`, `.dev/selecao-guarda`, `atacante-26a-`**: não estão no glossário; conteúdo das Fases 40–46.
- **`icones/` dentro da extensão montada** (`manifest.json` → `icones/extensao-*.png`, copiados de `docs/design/icones/`): campo do manifesto, Fase 39.
- **Nomes de alvo e de variável do Makefile** (`teste-…`, `FERRAMENTAS_DIR`, `COFRE`, `CHAVE_EXTENSAO` …): Fase 6. Só os valores de caminho mudaram.
- **`docs/mission/design-nodes.md`**: os caminhos citados mudam na Fase 59 (PRD regra 2).
- Conteúdo (comentários e identificadores) dos arquivos renomeados: Fases 39, 40–47.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git mv` (pastas e arquivos da tabela) | 0 |
| script de substituição (117 arquivos) + correções à mão; grep dos nomes antigos sem sobra fora do que está em "Ficou de fora" | 0 |
| `mv -n` dos arquivos de `.dev/`; id da chave antes/depois (`openssl pkey -pubout -outform DER \| sha256sum`) = `ID_EXTENSAO` = id 0.1.4 | 0 |
| `make compile`: origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python, 7 JS | 0 |
| `make english-check PATHS="install extension dev/tests dev/services dev/container-* dev/fixtures/entries.json dev/fixtures/attachments dev/extension dev/{env.sh,origins.py,variations.py,bridge.sh,open-app.py,gitleaks-*.toml,daemon.sh,compose.yml} app/design"`: nenhum achado de nome além dos 32 `.svg` dos ícones; os de comentário/identificador são do conteúdo (Fases 39–47) | 1 (relatório, esperado) |
| `make english-check PATHS=dev/fixtures/attachments` (as entradas de `allow.txt` continuam em uso) | 0 |
| `python3 -I -B dev/english_check.py --self-test` (plantio JS movido para `extension/`) 91/91 | 0 |
| `make -n install`, `make -n extensao` (a pasta `install/` não esconde o alvo `install`, que é `.PHONY`) | 0 |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos
1. **Glossário em conflito sobre o socket de teste:** a seção 5 põe `ward-teste/` → `ward-test/` na Fase 5, e a seção 9 trata o mesmo nome como marca da Fase 56. Segui a 9 e anotei no glossário. A Fase 56 precisa trocar `NOME_DIRETORIO_TESTE`, os scripts e as duas recusas (nova e antiga) juntos.
2. **Fase 6:** com a pasta `extension/` na raiz, o alvo novo `extension` tem de ficar no `.PHONY` (como `install` já está), senão o make o dá como "atualizado".
3. **Ícones (`app/design/icons/*.svg`)**: precisam de dono explícito no plano (sugestão: Fase 35, trocando as chaves de `ICONES` e o `tipo-${tipo}` junto com o nome v5 dos tipos, ou entrada em `allow.txt` com motivo).
4. **`.dev/auditor55.sn9p86/`**: pasta antiga de outro trabalho em `.dev/`; não mexi.
5. Estado do repositório: as mudanças da Fase 4 continuam não commitadas (renomes indexados e conteúdo no worktree). Os `git mv` desta fase também ficaram indexados.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `instalacao/**` → `install/**` (5 arquivos com nome novo) | movido (`git mv`) e alterado |
| `extensao/**` → `extension/**` (4 arquivos com nome novo) | movido e alterado |
| `dev/extensao/montar.py` → `dev/extension/build.py` | movido e alterado |
| `dev/{ambiente.sh,origens.py,variacoes.py,ponte.sh,abrir-app.py,gitleaks-arvore.toml,gitleaks-extra-arvore.toml}` → nomes novos | movido e alterado |
| `dev/fixtures/entradas.json` → `entries.json`, `dev/fixtures/anexos/` → `attachments/` | movido (e alterado o `entries.json`) |
| `dev/conteiner-*/**` → `dev/container-*/**` | movido e alterado |
| `dev/servicos/**` → `dev/services/**` | movido e alterado |
| `dev/testes/**` → `dev/tests/**` | movido e alterado |
| `app/design/icones/` → `app/design/icons/` | movido |
| `dev/compose.yml`, `dev/daemon.sh`, `dev/gitleaks-extra.toml`, `dev/english_check.py` | alterado |
| `Makefile`, `CLAUDE.md`, `README.md`, `docs/instalacao.md`, `docs/glossario.md` | alterado |
| `app/scripts/gerar_design.py`, `app/vite.config.js`, `app/src/lib/bidi.js`, `app/src/lib/componentes/{CampoSecreto.svelte,icones.js}` (só comentário/caminho), `app/src-tauri/src/area_gtk.rs` (comentário) | alterado |
| `app/core/src/{cliente,daemon_falso,servico}.rs` | alterado |
| `crates/ward-core/examples/dev-cofre/{main,testes}.rs`, `crates/ward-core/src/{entrada.rs,mascara/testes.rs}` | alterado |
| `crates/ward-nm/src/lib.rs`, `crates/ward-test-bridge/src/{main,simulados}.rs`, `crates/ward-test-bridge/tests/ataques.rs` | alterado |
| `crates/wardd/src/{config,playwright,trava}.rs`, `crates/wardd/tests/{app_valor,backup,playwright,units}.rs` | alterado |
| `.dev/` (fora do git): `test-vault.ward`, `test-vault.state`, `variations.txt`, `services/{keys,secrets,client-home}`, `tools/`, `clipboard/`, `.test-dev-vault.lock`, `extension/key.pem`, `logs/ward-test-bridge.log` | movido (`mv`) |
| `target/debug/ward-ponte-teste`, `libward_ponte_teste.*` | apagado |
| `docs/mission/handoffs/phase-5.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram `make compile` (0), o `english-check` nos caminhos da fase (só os `.svg` dos ícones como achado de nome) e o autoteste do checador (91/91).
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon de teste iniciado; o `wardd` do usuário em `/usr/local/bin` não foi tocado).
- [x] Teste de ataque para cada (SEG) tocado: V1.9 conferido à mão (id, modo, link); os ataques existentes seguem os caminhos novos. Nenhum novo.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase (os pontos adiados estão em "Ficou de fora" e "Problemas").
