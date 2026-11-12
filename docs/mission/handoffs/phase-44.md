# Fase 44 — `dev-vault.sh`, `ssh_home.py`, `install.sh`, `attacks-services.sh`, `chrome_policy.py`, `app_e2e.py` e `playwright.py` (Missão 2, trilha S)

## Implementado

Troca 1:1 pelo `docs/glossario.md`, sem mudar comportamento. O worktree nasceu em
`b4d5e1b`; com a árvore limpa, `git reset --hard 662621f` (master). Identificadores,
comentários e docstrings em inglês; as mensagens de saída (`ok …`, `FALHA …`, `FALHOU: …`,
`test-…: …`, mensagens das conferências) continuam em pt-BR, iguais. No Python, os renomes
foram feitos por token `NAME` (strings intactas) e conferidos por comparação de tokens
(original × novo: mesma sequência, cada nome antigo vira um só nome novo, sem colisão); no
shell, por mapa de palavras aplicado ao original e `diff` sem comentários (sobram só os
literais listados abaixo).

- **`dev/tests/dev-vault.sh`**: `RAIZ` → `ROOT`, `COFRE` → `VAULT`, `LISTA` →
  `PATTERN_LIST`, `TRAVA` → `LOCK_FILE`, `limpar` → `cleanup`, `falhas`/`falhou` →
  `failures`/`failed`, `sem_segredo` → `no_secret`, `dev_cofre` → `dev_vault`, `variacoes`
  → `variations`, `variacoes_d36_d37` → `variations_d36_d37`, `cofre_duas_vezes` →
  `vault_twice`, `simbolos_d59` → `symbols_d59`, `cruzado` → `cross_check`, `saida_fora`/
  `saida_dentro` → `output_outside`/`output_inside`; locais (`faltou` → `missing`,
  `esperadas` → `expected`, `chave` → `key`, `alvo` → `target`, `fora` → `outside`,
  `pedido(s)` → `request(s)`, `nada` → `clean`) e, nos heredocs em Python, `linha` →
  `line`, `saida` → `out`, `formas` → `forms`, `texto`/`secao` → `text`/`section`,
  `tipos`/`niveis` → `kinds`/`levels`, `raiz` → `root`, `dados` → `data`, `tipo, padroes` →
  `kind, patterns`.
- **`dev/tests/ssh_home.py`**: `RAIZ`/`CASA`/`TRAB` → `ROOT`/`PASSWD_HOME`/`WORK_DIR`,
  `PASTA_SSH` → `SSH_DIR`, `VAR_AGENTE` → `AGENT_VAR`, `SENTINELA(_LOCAL)` →
  `SENTINEL`/`LOCAL_SENTINEL`, `TRACOS` → `TRACES`, `SENHA` → `PASSWORD`, `PADROES` →
  `PATTERNS`, `conferir` → `verify`, `dentro` → `inside`, `fase` → `stage`, `escritor` →
  `writer`, `agente_falso` → `fake_agent`, `vigia` → `watcher`, `encerrar_daemon` →
  `stop_daemon`, `pty_rodar(args, passos, prazo)` → `pty_run(args, steps, timeout)`,
  `ward(args, entrada, prazo)` → `ward(args, stdin_data, timeout)`, `acusados(traco)` →
  `flagged(trace)`, `LINHA`/`CAMINHO`/`RETORNO`/`COM_DIRFD` →
  `LINE`/`QUOTED_PATH`/`RETURN_VALUE`/`WITH_DIRFD`, e os locais (lista completa pela
  comparação de tokens: `aberturas` → `openings`, `eventos` → `events`, `pastas_vigiadas`
  → `watched_dirs`, `cwd_de`/`pai_de` → `cwd_of`/`parent_of` …).
- **`dev/tests/install.sh`**: `RAIZ` → `ROOT`, `SO_DESTDIR` → `DESTDIR_ONLY`, `SEM` →
  `NO_TEST_MODE`, `CLIENTE` → `CLIENT`, `WARDD_TESTE` → `TEST_WARDD`, `PLATAFORMA` →
  `PLATFORM`, `NOME` → `CONTAINER_NAME`, `confere` → `verify`, `nao_tem` → `lacks`,
  `recusa` → `expect_refusal`, `nada_do_ward` → `nothing_from_ward`, `despejo` → `dump`,
  `nova` → `new_dir`, `politica_outra` → `other_policy`, `nao_saiu0` → `not_exit0`, `ARV`/
  `ARQUIVOS` → `TREE`/`FILES`, `LISTA_APPORT` → `APPORT_LIST`, `ARQ_DPKG` → `DPKG_ARCH`,
  `AUX_PACOTE` → `PACKAGE_HELPERS`, `COFRE_D`/`COFRE_TESTE` → `VAULT_DIR`/`TEST_VAULT`,
  `IMAGEM` → `IMAGE`, e os locais (`acao` → `action`, `rotulo` → `label`, `codigo` →
  `code`, `pasta` → `dir`, `outro` → `other_id`, `raiz` do XML → `root_el` …).
- **`dev/tests/attacks-services.sh`**: `RAIZ` → `ROOT`, `falhas`/`falhou`, `recusado` →
  `expect_refused`, `docker_sem_ambiente_do_usuario` → `docker_without_user_env`,
  `portas_so_loopback` → `ports_loopback_only`, `interno_inalcancavel` →
  `internal_unreachable`, `metodos` → `auth_methods`, `autenticacao` → `authentication`,
  `sftp_restrito` → `sftp_restricted`, `canal_abre` → `channel_opens`, `tuneis` →
  `tunnels`, `chave_host` → `host_key`, `codigo_totp` → `totp_code`,
  `sem_segredo_exposto` → `no_exposed_secret`, `permissoes` → `permissions`,
  `sem_arquivos_nao_sobe` → `no_files_no_start`, e os locais (`mau` → `bad`, `marcas` →
  `marks`, `falsos` → `fakes`, `hostil` → `hostile`, `cab` → `headers`, `bom`/`ruim` →
  `good`/`bad_code`, `contas` → `accounts` …).
- **`dev/tests/chrome_policy.py`**: `main()` com `root, work_dir, scenario, policy`,
  `fora` → `outside`, `marca` → `mark`, `rotulo` → `label`, `perfil` → `profile`,
  `manifesto(caminho, destino)` → `manifest(path, dest)`, `quem` → `who`, `atacante` →
  `attacker`, `sistema` → `system_host`, `alvos` → `targets`, `respostas` → `answers`,
  `com_segredo` → `with_secret`, `roubo` → `stolen`, `POLITICAS` → `POLICIES`, `FOLHAS` →
  `LEAVES`, `ler_pagina_de_politicas` → `read_policy_page`, `cmd(metodo, params, sessao,
  prazo)` → `cmd(method, params, session, timeout)`; dentro do script do atacante (string)
  `SAIDA`/`MARCA` → `OUTPUT`/`MARK`, `troca` → `exchange`, `origem` → `origin`, `nome` →
  `name`; no JS de `LEAVES`, `andar` → `walk`. Duas expressões de f-string viraram
  variáveis antes da f-string (`policy_kind`, `received`), com o mesmo texto: o checador lia
  os literais de dentro do `{…}` como identificadores.
- **`dev/tests/app_e2e.py`**: `RAIZ` → `ROOT`, `MARCA` → `MARK`, `SENHA` → `PASSWORD`,
  `EVID` → `EVIDENCE`, `LOG_TERMINAL` → `TERMINAL_LOG`, `resultados` → `results`,
  `checar(nome, ok, detalhe)` → `check(name, ok, detail)`, `esperar` → `wait_for`,
  `texto` → `page_text`, `pedido_na_janela` → `request_in_window`, `sem_pedido` →
  `no_request`, `responder(apr, botao, pin)` → `answer(apr, button, pin)`, `fim_do_run` →
  `run_result`, `run_em_fundo` → `run_in_background`, `recarregar` → `reload_page`,
  `destravar` → `unlock`, `abrir_entrada` → `open_entry`, `linhas_terminal` →
  `terminal_lines`, `modo_terminal` → `set_terminal_mode`, `conectar` → `connect`,
  `novas_linhas` → `new_lines`, `PAGINAS` → `PAGES`, `passos` → `steps`, `pag` → `page`,
  `nav` → `browser` …
- **`dev/tests/playwright.py`**: como no `login.py` da Fase 42 (`ROOT`, `WORK_DIR`,
  `PASSWORD`, `PATTERNS`, `failures`, `APPROVER_LOG`/`APPROVER_MODE`, `set_window_mode`,
  `window_requests`, `verify`, `has_secret`, `environment`, `run_pty`), e `Vigia` →
  `Watcher` (atributos `stop`, `listening`, `profile_with_secret`, `profiles`,
  `pipe_outside`, `reads`), `pids_do_chrome` → `chrome_pids`, `inodes_escutando` →
  `listening_inodes`, `SAIDA_FALSO` → `FAKE_OUTPUT`, `esperado` → `expected`, `estado` →
  `state_json`, `roteiro` → `script` …
- **Glossário**: parágrafo da Fase 44 na seção 4 (`dev/`).

### Literais alterados (PRD regra 5; todos nomes internos do próprio script)

- `dev-vault.sh`: arquivos do `$TMP` (`literais.txt` → `literals.txt`, `var-teste.log` →
  `var-test.log`, `cofre$i.log` → `vault$i.log`, `tabela.txt` → `table.txt`,
  `conferir.log` → `verify.log`, `lidas.txt` → `listed.txt`, `senha-certa`/`senha-errada`
  → `right-password`/`wrong-password`, `certa.log`/`errada.log` → `right.log`/`wrong.log`,
  `cruzado.log` → `cross.log`, `simbolos.env`/`-json.txt`/`-shell.txt` → `symbols.*`,
  `fora-$1` → `outside-$1`, `vitima` → `victim`, `roubado` → `stolen`, `injetado` →
  `injected`, `nao-existe` → `does-not-exist`); nomes de ataque em `.dev/`
  (`test-attack-arquivo`/`-quebrado`/`.outro`/`-lista.txt` →
  `test-attack-file`/`-broken`/`.other`/`-list.txt`) e na raiz (`fora-teste-ataque.*` →
  `outside-test-attack.*`, no `cleanup` e nos pedidos).
- `install.sh`: opção `--so-destdir` → `--destdir-only` (ninguém a passa: nem o Makefile
  nem outro script) e a mensagem que a cita; arquivos do `$TMP` (`arvore` → `tree`, `raiz`
  → `root`, `cp-arquivo`/`cp-outro` → `cp-file`/`cp-other` (e o laço `arquivo` → `file`),
  `alvo-do-link` → `link-target`, `*-marcado` → `*-marked`, `nao-existe` →
  `does-not-exist`, `outro.crx`/`adulterado.crx`/`id-adulterado`/`versao.crx` →
  `other.crx`/`tampered.crx`/`tampered-id`/`version.crx`, `r-ponto-ponto` → `r-dot-dot`,
  `alvo-bin`/`alvo-$n` → `bin-target`/`target-$n`, `conteiner` → `container`), DESTDIR
  relativo `relativo` → `relative` (e a mensagem que o cita), arquivo plantado no DESTDIR
  `usr/local/bin/outro` → `other`, políticas plantadas (`outra.json`, `00-empresa.json`,
  `.escondido`, `senhas.json`, `vazamento.json` → `other.json`, `00-company.json`,
  `.hidden`, `passwords.json`, `leak.json`).
- `attacks-services.sh`: arquivos do `$TMP` (`senha-errada` → `wrong-password`,
  `docker-mau` → `docker-bad`, `marcas-docker`/`falsos-docker` →
  `docker-marks`/`docker-fakes`, `known_hosts_vazio`/`_antigo` →
  `known_hosts_empty`/`_old`, `segredos.txt` → `secrets.txt`, `copia` → `copy`), valores
  do ambiente hostil (`credsStore`/`DOCKER_CONTEXT` `mau` → `bad`, `/nao/existe.*` →
  `/does/not/exist.*`, `outro-projeto` → `other-project`) e o projeto compose
  `ward-ataque-$$` → `ward-attack-$$`.
- `ssh_home.py`: rótulos internos de etapa (`preparo`, `depois`, `controle-config`,
  `controle-padrao`, `fim` → `setup`, `after`, `control-config`, `control-default`, `end`;
  só comparados dentro do script).
- `chrome_policy.py`: dentro das strings de código gerado, os identificadores acima
  (script do atacante, JS de `LEAVES`).
- `app_e2e.py`, `playwright.py`: nenhum (só docstrings).

### Ficaram de propósito (conferidos contra o master `662621f`)

- O que outro arquivo lê ou escreve: `--example dev-cofre`, `--conferir`, `--senha-stdin`,
  `--senha`, `--cruzado`, o tipo `inteiro`/`url` do pipe, `dev-cofre: configuração: …`,
  `não abriu` e `os N padrões de N entradas` (exemplo `dev-cofre`, Fase 43); `recusado` da
  recusa do Makefile; o nome de módulo `"variacoes"`; a pasta `tracos` (criada pelo
  `ssh-home.sh`); `WARD_SSH_HOME_DENTRO`, `WARD_DEV_EXTENSAO`, `ATQ_*` (Fase 56); os
  cenários `precedencia`/`roubo`/`pagina`, as chaves do JSON impresso (`sistema`,
  `usuario`, `pulado`, `atacante_rodou`, `sistema_rodou`, `respostas`, `com_segredo`,
  `valor`) e a pasta `roubo-sem-politica` (lidos pelo `chrome-policy.sh`, Fase 46);
  `wardd-aprovador-teste`, `aprovador.modo` e os modos `negar`/`confirmar` (Fase 56);
  `terminal-teste.modo`/`.log`, os modos `normal`/`reuso`/`outra-entrada`/`expirado` e os
  campos `uso=`/`saida=`/`conectou=`/`proc=` (`terminal-test.py`); as chaves do `qa.json`
  e os nomes das capturas de evidência; os nomes dentro de `$C` do `install.sh`
  (`bin/cliente`, `bin/plataforma`, `bin/falso`, `libmarca.so`, `wardd-test-mode`,
  `cofre.ward`, `senha`, `id`, `/opt/teste`), lidos pelo `dev/container-install/inside.sh`
  (Fase 43); `lista_apport`, `avisar_core_pattern`, `erro`/`aviso` (`install/dump.sh`,
  Fase 47); `--adulterado`/`--versao` (`fake_crx.py`); `totp-contas.json` e os campos
  `usuario`/`senha`/`codigo`/`totp_semente`; protocolo (`destravar`, `dados`, `senha`,
  `tipo`, `sugerir`, `preencher`, `origem`, `itens`, `nome`, `entrada`, Fase 52); chaves
  do `entries.json` (Fase 48); `pub const ID_EXTENSAO` (Fase 28); a imagem
  `ward-teste/install-systemd:1`; o caminho real `cofre.ward`; `/tmp/sentinela-ssh` e
  `/tmp/agente-falso` (paths que aparecem nas mensagens).

### Exceções

Nenhuma entrada nova em `allow.txt` nem em `frozen.txt`.

## Segurança

- **Lógica intocada** (PRD regra 7): os ataques e as conferências são os mesmos, na mesma
  ordem, com as mesmas expectativas, prazos e mensagens. Python conferido por sequência de
  tokens (só nomes, docstrings e os literais internos listados mudam); shell por mapa
  reverso e `diff`. Nenhum segredo novo: os valores falsos de D15 que já estavam nos
  scripts continuam iguais.
- **Testes de ataque**: nenhum novo (fase só de nomes). Escritos, não rodados (PRD regra
  6): `test-dev-vault`, `test-ssh-home`, `test-install`, `test-attacks-services`,
  `test-chrome-policy`, `test-app-e2e` e `test-playwright` rodam no M6.
- **Dependências novas**: nenhuma.

## Ficou de fora

- `RAIZ` (interface do `dev/services/common.sh`, que monta `DS`, `SEG`, `COMPOSE` e o
  `DOCKER_CONFIG` a partir dele): `install.sh` e `attacks-services.sh` usam `ROOT` e fazem
  `RAIZ="$ROOT"` antes do `.` (2 achados do `english-check`). A Fase 46 renomeia no
  `common.sh` e em todos os que o leem e apaga essas duas linhas.
- `CASA_PASSWD` (`dev/env.sh`) no `dev-vault.sh` (2 achados): Fase 46, como combinado.
- Funções e variáveis do `common.sh` usadas no `attacks-services.sh` (`exigir`,
  `esperar_wordpress`, `cliente`, `ssh_senha`, `montar_opcoes_ssh`, `OPCOES_SSH`,
  `wp_login`, `SEG`): o checador não as acusa nesse arquivo, mas são pt-BR; Fase 46.
- Suítes: Milestone 6.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 662621f` (o worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `make compile` (log em `<scratchpad>/fase44/fase44-make-compile.log`) | 0 (origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python e 7 JS: 0 erro) |
| `make english-check PATHS="<os 7 arquivos>"` | 2 (4 achados: `RAIZ` ×2, `CASA_PASSWD` ×2; Fase 46) |
| comparação de tokens (Python) e mapa reverso + `diff` (shell) contra o original | só as diferenças listadas |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- **Conflito de merge previsto com a Fase 43** (paralela): ela renomeia o exemplo
  `dev-cofre` e os da `container-*` "com todos os usos". Linhas deste trabalho que ela
  também deve tocar: `dev-vault.sh` (definição de `dev_vault` com `--example dev-cofre`, as
  chamadas com `--conferir`/`--senha-stdin`/`--senha`/`--cruzado`, o `grep` de
  `dev-cofre: configuração: …`) e `install.sh` (`APP=…/partida-conteiner`,
  `CLIENT=…/wardd-cliente-teste`, as linhas `install … "$C/bin/cliente"`/`plataforma`/
  `falso`/`libmarca.so`, `cofre.ward`, `senha`). A resolução: nomes de variável/função
  desta fase, literais da 43.
- O checador lê literais dentro de `{…}` de f-string como identificadores (achado no
  `chrome_policy.py`); contornado tirando a expressão da f-string.
- O checador não acusa nomes pt-BR usados só como comando no shell (`exigir`, `cliente`,
  `ssh_senha` …) num arquivo que não os define; a Fase 46 precisa procurar os usos por
  `grep`, não pelo checador.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/tests/dev-vault.sh` | alterado |
| `dev/tests/ssh_home.py` | alterado |
| `dev/tests/install.sh` | alterado |
| `dev/tests/attacks-services.sh` | alterado |
| `dev/tests/chrome_policy.py` | alterado |
| `dev/tests/app_e2e.py` | alterado |
| `dev/tests/playwright.py` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-44.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram `make compile` e o
  `english-check` nos arquivos da fase.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem iguais; nenhum
  novo, porque a fase só renomeia.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
