# Fase 42 — `variations.py`, `variations_test.py`, `login.py` e `revealed_selection.py` (Missão 2, trilha S)

## Implementado

Troca 1:1 pelo `docs/glossario.md`, sem mudar comportamento. O worktree nasceu em
`b4d5e1b`; com a árvore limpa, `git reset --hard 1b5735d` (master). Os renomes foram
feitos por `tokenize` (só tokens `NAME`, por um mapa por arquivo; strings e comentários
intactos) e as docstrings e os comentários foram reescritos em inglês. As mensagens de
saída (`dev-variations: …`, `ok …`, `FALHA …`, `OK …`, `selecao-revelado: …`, mensagens
das asserções) continuam em pt-BR, iguais.

- **`dev/variations.py`** (o oráculo de D54): `RAIZ` → `ROOT`, `SAIDA_PADRAO` →
  `DEFAULT_OUTPUT`, `BASE_CHAVES`/`BASE_ANEXOS` → `KEYS_BASE`/`ATTACHMENTS_BASE`,
  `MIN_PADRAO` → `MIN_PATTERN`, `MAX_ARQUIVO` → `MAX_FILE`, `ESQUEMAS` → `SCHEMES`,
  `Recusa` → `Refusal`, `falhar` → `fail`, `sem_link` → `no_links`, `validar_saida` →
  `validate_output`, `ler_do_repositorio(caminho, base)` → `read_from_repo(path,
  base_dir)`, `gravar` → `write_file`, `BARRA` → `BACKSLASH`, `BRANCO` → `WHITESPACE`,
  `miolos_b64` → `b64_cores`, `minusculo_hex` → `lowercase_hex`, `texto` → `as_text`,
  `caracteres` → `char_count`, `aspas_duplas`/`aspas_simples` →
  `double_quoted`/`single_quoted`, `PRINTF_Q_SEMPRE` → `PRINTF_Q_ALWAYS`, `escapadas` →
  `escaped_forms`, `inteiro` → `whole`, `aparar` → `trim`, `CHAVE_IGUAL`/`CHAVE_JSON`/
  `CHAVE_YAML`/`CHAVE_YAML_COMPOSTA` → `KEY_EQUALS`/`KEY_JSON`/`KEY_YAML`/
  `KEY_YAML_COMPOSITE`, `COMENTARIO` → `COMMENT`, `ENTRE_ASPAS_COMENTARIO` →
  `QUOTED_COMMENT`, `valores_da_linha` → `line_values`, `comentario_e_aspas` →
  `comment_and_quotes`, `valores_json` → `json_values`, `campos_de_tab`/
  `campos_de_tab_binarios` → `tab_fields`/`binary_tab_fields`, `linhas_de_anexo` →
  `attachment_lines`, `por_linha` → `per_line`, `de_linha` → `line_patterns`,
  `variacoes(v, usuario, anexo)` → `variations(v, username, attachment)`, `listavel` →
  `listable`, `da_entrada` → `entry_patterns`, `coletar` → `collect`,
  `conferir_padroes` → `verify_patterns`; locais pela seção 2 (`saida` → `out`,
  `linha(s)` → `line(s)`, `valor(es)` → `value(s)`, `miolo` → `core`, `crus` →
  `raw_fields`, `base` do banco → `db_name`, `cifrada` → `encoded` …).
- **`dev/tests/variations_test.py`**: os usos acima; `RAIZ`/`FONTE` → `ROOT`/`SOURCE`,
  `carregar` → `load`, `SENHA_GO` → `GO_PASSWORD`, `ACENTO` → `ACCENTED`, `LINHA`/
  `LINHA_ASCII` → `LINE`/`LINE_ASCII`, classes `D37FormasEscapadas` →
  `D37EscapedForms`, `D36PassadaPorLinha` → `D36PerLinePass`, `D11DeAntes` →
  `D11FromBefore`, `D54TabelaUnica` → `D54SingleTable`, `Independencia` →
  `Independence`, `tem` → `has_form`, `varias` → `multi_line`, e os 43 `test_*` em inglês
  pelo sentido (ex.: `test_linha_nao_ganha_base64_nem_miolos` →
  `test_line_gets_no_base64_or_cores`, `test_nao_chama_o_ward_core` →
  `test_does_not_call_the_core_crate`).
- **`dev/tests/dev-vault.sh`** (arquivo da Fase 44, só o uso, PRD regra 3): o teste
  cruzado V2.57 chama `v.da_entrada(e)` no heredoc em Python → `v.entry_patterns(e)`.
  `v.database_url(e)` não mudou.
- **`dev/tests/login.py`**: como no `extension.py` da Fase 40 (`ROOT`, `WORK_DIR`,
  `PATTERN_LIST`, `SOCKET_PATH`, `EVIDENCE`, `failures`, `report`, `fail`, `verify`,
  `has_secret`, `ENTRIES`, `unlock`, `APPROVER_LOG`/`APPROVER_MODE`, `set_window_mode`,
  `window_requests`, classe `Cdp` com `send`/`_read`/`pump`/`events`/`on_paused`/
  `ignored`, `log_console`, `evaluate`, `open_tab`, `navigate`), e `SENHA_PROD` →
  `PROD_PASSWORD`, `PAGINAS` → `PAGES`, `envios` → `submissions`, `servir_paginas` →
  `serve_pages`, `atender_pausa` → `handle_pause`, `ward_nm_vivo` → `ward_nm_alive`,
  `form(acao, metodo, extra_form, extra_campos, botao)` → `form(action, method,
  extra_form, extra_fields, button)`, e os locais (`perfil` → `profile`, `painel` →
  `panel`, `vistos`/`novos` → `seen`/`new_requests`, `achados` → `found` …).
- **`dev/tests/revealed_selection.py`**: `RAIZ` → `ROOT`, `PASTA` → `WORK_DIR`, `SENHA`
  → `PASSWORD`, `ENTRADA` → `ENTRY`, `PRAZO_FILHO` → `CHILD_DEADLINE`, `modo_ler`/
  `modo_webkit` → `read_mode`/`webkit_mode`, `no_principal` → `on_main`, `esperar` →
  `wait_for`, `ler` → `read_selections`, `tecla` → `key_press`, `clique` → `click`,
  `principal`/`passos` → `worker`/`steps`, `revelar` → `reveal`, `neutralizar` →
  `neutralize`, `sem_valor` → `no_value`, `prova` → `proof`, `caixa` → `box`, e os locais
  (`agulhas` → `needles`, `controle`/`neutro` → `control`/`neutral`, `passos_arrasto` →
  `drag_steps` …).
- **Glossário**: parágrafo da Fase 42 na seção 4 com os nomes recorrentes e os sentidos
  próprios (`BRANCO` → `WHITESPACE`, `BARRA` = `chr(92)` → `BACKSLASH`, `ESQUEMAS` de URL
  → `SCHEMES`).

### Literais alterados (PRD regra 5)

- `revealed_selection.py`: a opção interna `--ler` (o script chama a si mesmo para ler as
  seleções) → `--read`, nos dois lugares (quem chama e quem confere o `argv`). Nenhum
  outro arquivo usa a opção.
- Fora isso, nenhum literal mudou. Conferido por AST (docstrings à parte): o original com
  o mesmo mapa aplicado e o arquivo novo têm a mesma árvore, com diferença só no
  parâmetro `base` → `base_dir` do `read_from_repo` (acima) e no `--read`. As f-strings
  mudam só o nome capturado (`{caminho}` → `{path}`, `{dica}` → `{hint}`, `{codigo}` →
  `{code}`, `{rotulo}` → `{label}` …); o texto impresso é o mesmo.
- Ficam de propósito: chaves do `entries.json` (`entradas`, `segredos`, `campos`,
  `usuario`, `motor`, `porta`, `base`, `senha_mestra`, `versao` …; Fase 48), as operações
  e campos do protocolo (`destravar`, `dados`, `senha`, `tipo`; Fase 52), as chaves do
  `qa.json`, o caminho `/ward-teste/` das páginas servidas pelo CDP, `aprovador.modo` e o
  log `wardd-aprovador-teste.log` (Fase 56), `pub const ID_EXTENSAO` (Fase 28),
  `[data-entry]`, `.revealed`, `.field`, os `aria-label` "Revelar"/"Ocultar" e os
  diretórios internos de `TRAB` (`perfil`, `home`) do `login.py`. O nome de módulo
  `"variacoes"` do `spec_from_file_location` (no teste e no `dev-vault.sh`) também ficou:
  não é lido por ninguém.

Conferido contra o master `1b5735d`: o `login.sh` lê `chrome.log`, `console.log`,
`cli.log` e `logs/wardd-aprovador-teste.log` (iguais); o `revealed-selection.sh` passa a
URL pela entrada padrão e não procura saída do `.py`; o `ward-nm` ainda tem
`pub const ID_EXTENSAO`; a interface ainda usa `#master-password`, `[data-entry]`,
`.revealed`, `.field` e os rótulos "Revelar "/"Ocultar ".

### Exceções

Nenhuma entrada nova em `allow.txt` nem em `frozen.txt`. Comentários que citavam um nome
congelado foram reescritos pelo sentido (o caminho `/ward-teste/` da docstring do
`login.py` → "a fixed path under `http://localhost:8081/`"; `wardd-aprovador-teste` →
"the test approver binary"; o `mascara.rs` do `variations.py` → "the mask code").

## Segurança

- **Oráculo de D54 intocado** (PRD regra 7): a tabela única (segredo inteiro, Basic, por
  linha, valor de linha, anexo inteiro, formas de texto) tem a mesma lógica, conferida
  por AST. O `variations.py` continua sem `subprocess`, `os.system`, `cargo `,
  `ward_core` nem `ward-core/src` (o `test_does_not_call_the_core_crate` procura esses
  textos; a docstring nova diz "the core crate"). A gravação continua 0600, atômica, só
  em `.dev/`, sem link (S15), e nada da lista vai para stdout/stderr.
- **S2, S12 (`revealed_selection.py`)** e **D9, S1, S13 (`login.py`)**: os ataques são os
  mesmos, na mesma ordem, com as mesmas expectativas e os mesmos prazos.
- **Testes de ataque**: nenhum novo (fase só de nomes). Escritos, não rodados (PRD regra
  6): `make test-dev-vault`, `make test-login` e `make test-revealed-selection` rodam no
  M6.
- **Dependências novas**: nenhuma.

## Ficou de fora

- Literais congelados (acima): mudam nas Fases 28, 48, 52, 53 e 56.
- Suítes: Milestone 6.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 1b5735d` (o worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `python3 -I dev/english_check.py --paths dev/variations.py dev/tests/variations_test.py dev/tests/login.py dev/tests/revealed_selection.py` (= `make english-check PATHS=…`) | 0 (0 achados) |
| conferência por AST (original com o mapa × novo, sem docstrings) | só as 2 diferenças listadas |
| `make compile` (log em `.dev/fase42/fase42-make-compile.log`) | 0 (origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python e 7 JS: 0 erro) |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- O `dev/tests/dev-vault.sh` (Fase 44) importa o `variations.py` e chama
  `entry_patterns`/`database_url`; quem renomear funções do oráculo de novo (Fase 53) tem
  de ajustar o heredoc do teste cruzado na mesma fase.
- Fase 53 (variáveis entregues) e Fase 48 (corpo v5, `entries.json` com chaves em
  inglês): o `variations.py` lê `entradas`, `segredos`, `campos`, `extras`, `anexos`,
  `senha_mestra`, `pin`, `versao`, os sufixos `_arquivo` e as chaves de `campos`
  (`motor`, `usuario`, `host`, `porta`, `base`) do `entries.json`; o `login.py` lê
  `entradas`, `nome`, `segredos`, `senha` e `senha_mestra`. Os dois precisam acompanhar.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/variations.py` | alterado |
| `dev/tests/variations_test.py` | alterado |
| `dev/tests/login.py` | alterado |
| `dev/tests/revealed_selection.py` | alterado |
| `dev/tests/dev-vault.sh` | alterado (só o uso de `entry_patterns`) |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-42.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram `make compile` e o
  `english-check` nos arquivos da fase.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem iguais; nenhum
  novo, porque a fase só renomeia.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
