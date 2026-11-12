# Fase 40 — `dev/tests/extension.py` e `dev/tests/cli.py` (Missão 2, Milestone 3, trilha S)

## Implementado

Troca 1:1 pelo `docs/glossario.md` dos identificadores, docstrings e comentários dos dois
scripts, sem mudar comportamento nem nenhum literal procurado em saída de Rust/JS. O
worktree nasceu em `b4d5e1b` (fora da linha do master); com a árvore limpa, `git reset
--hard 6d8cda8`.

- **`dev/tests/cli.py`**: `RAIZ` → `ROOT`, `SENHA` → `PASSWORD`, `falhas` → `failures`,
  `conferir` → `verify`, `PASTA_SOCKET` → `SOCKET_DIR`, `ambiente()` → `environment()`,
  `rodar_pty(args, digitar, env, prazo)` → `run_pty(args, to_type, env, deadline)`,
  `app_do_daemon_de_teste` → `test_daemon_app`, `falso` → `fake_server`, `registro` →
  `record`, `sem_variacao(r, rotulo)` → `no_variation(r, label)`, `tipo_no_log` →
  `kinds_in_log`, `padroes` → `patterns`, `ESPERADO` → `EXPECTED`, `ARQUIVOS` → `FILES`,
  `RT_TESTE` → `RT_TEST`, `SENHA_SSH` → `SSH_PASSWORD`, `porta_aberta` → `port_open`,
  `ssh(args, entrada, env, prazo)` → `ssh(args, input_data, env, deadline)`,
  `run(args, env, entrada)` → `run(args, env, input_data)`, `TERMINAL_FALSO` →
  `FAKE_TERMINAL`, `ssh_com_canal` → `ssh_with_channel`, `Varredor` → `Scanner`
  (`parar`/`achados`/`vistos` → `stop`/`found`/`seen`), `agentes` → `agents`,
  `esperar` → `wait_for`, `interativo` → `interactive`, `da_sessao` → `of_session`,
  `capturar` → `capture`, `sftp_devagar` → `slow_sftp`, `LOG_DAEMON` → `DAEMON_LOG`,
  `MSG_TOKEN_INVALIDO` → `INVALID_TOKEN_MSG`, `linhas_token_invalido` →
  `invalid_token_lines`, `LeitorDeProc` → `ProcReader` (`agulha` → `needle`,
  `varridos` → `scanned`, `fds_tentados` → `fds_tried`), e os locais (`tela` →
  `screen`, `cod` → `code`, `lote` → `batch`, `atrasos` → `delays`, `cand` →
  `candidate`, `principal`/`salto` → `main_ssh`/`jump`, `ponta` → `endpoint`…). A
  variável `s` do ataque à ponte do túnel (que sombreava a função `s()`, já sem uso
  depois) virou `conn`.
- **`dev/tests/extension.py`**: `RAIZ`/`TRAB`/`FALSO`/`LISTA` → `ROOT`/`WORK_DIR`/
  `FAKE_CHROME`/`PATTERN_LIST`, `CAMINHO` → `SOCKET_PATH`, `EVID`/`EVID42` →
  `EVIDENCE`/`EVIDENCE_POPUP`, `falhas`/`relato` → `failures`/`report`, `falhou` →
  `fail`, `confere(cond, msg, chave)` → `verify(cond, msg, key)`, `tem_segredo` →
  `has_secret`, `palavras` → `words`, `dicionario_zxcvbn` → `zxcvbn_dictionary`,
  `achado_do_dicionario` → `dictionary_finding`, `ENTRADAS` → `ENTRIES`,
  `APROVADOR_LOG`/`APROVADOR_MODO` → `APPROVER_LOG`/`APPROVER_MODE`, `modo_janela` →
  `set_window_mode`, `pedidos_da_janela` → `window_requests`, `destravar` → `unlock`,
  classe `Cdp` (`enviar` → `send`, `_ler` → `_read`, `esperar` → `wait_for`, `bombear`
  → `pump`, `eventos` → `events`, `ao_pausar` → `on_paused`, `ignorar` → `ignored`;
  parâmetros `metodo`/`sessao`/`prazo` → `method`/`session`/`deadline`),
  `registrar_console` → `log_console`, `avaliar(cdp, sessao, expr, prazo, contexto)` →
  `evaluate(cdp, session, expr, deadline, context)`, `PAGINAS` → `PAGES`,
  `servir_paginas` → `serve_pages`, `atender_pausa`/`atender_fetch` →
  `handle_pause`/`handle_fetch`, `GEOMETRIA`/`GEOMETRIA_SALVAR` →
  `GEOMETRY`/`SAVE_GEOMETRY`, `abrir_aba` → `open_tab`, `navegar` → `navigate`,
  `ativar` → `activate`, `abrir_popup(t, esperar)` → `open_popup(t, wait_selector)`,
  `fechou` → `closed`, `fechar_popup` → `close_popup`, `captura(ps, nome, altura,
  pasta, alem, y)` → `screenshot(ps, name, height, dir_, beyond, y)`, `valores` →
  `values`, `clicar_entrada` → `click_entry`, `SENHA`/`USUARIO` → `PASSWORD`/`USERNAME`,
  `esperar_valor` → `wait_for_value`, `preencher_forcado` → `forced_fill`,
  `tema_continua_escuro` → `theme_stays_dark`, `sem_prender` → `unstick`,
  `popup_novo` → `new_popup`, `anexar_popup` → `attach_popup`, `digitar` →
  `type_text`, `clicar` → `click`, `esperar_carga` → `wait_for_load`,
  `pendente_do_popup` → `pending_from_popup`, `contexto_do_content_script` →
  `content_script_context`, `secao_fase_42` → `phase_42_section`, `novos` →
  `new_passwords`, `quadro` → `frame`, e os locais (`perfil` → `profile`, `nota` →
  `note`, `difs` → `diffs`, `iscas` → `decoys`, `achou` → `found_files`, `autoteste` →
  `self_test`…).
- Comentários e docstrings dos dois em inglês ("Fase" → "Phase", IDs da missão 1 sem
  prefixo, DT2). Os comentários não citam nomes de fio nem textos pt-BR entre aspas
  (descrevem em inglês: "the unlock", "approval unavailable").
- **Glossário**: parágrafo da Fase 40 na seção 4 (`dev/`), com os nomes recorrentes.
- **Exceções**: nenhuma em `allow.txt` nem em `frozen.txt`.

### Literais (PRD regra 5)

Nenhum literal mudou. Conferido por comparação dos tokens `STRING`/`FSTRING_MIDDLE`/
`NUMBER`/`OP` (`tokenize`) do arquivo antigo e do novo: as únicas diferenças são as
docstrings e quatro rearranjos sem efeito, todos para tirar subscritos com chave pt-BR de
dentro de expressão de f-string (o checador lê a expressão como identificador):

- `cli.py`: `dict(vars_, WARD_ENTRADA=…, WARD_GRUPO=…, WARD_AMBIENTE=…)` →
  `dict(vars_, **{"WARD_ENTRADA": …, "WARD_GRUPO": …, "WARD_AMBIENTE": …})` (mesmo dict;
  as chaves eram identificadores de kwargs).
- `extension.py`: `{tx['nome']}`, `{g['rodape']}`, `{g['botoes']}`, `{g2['rodape']}` e
  `{[(x['amb'], x['det']) …]}` dentro de f-strings passaram para variáveis calculadas na
  linha de cima (`suggested`, `footer`, `buttons`, `footer2`, `env_details`); o texto das
  mensagens é o mesmo.

Conferido contra o código atual do master (`6d8cda8`): as mensagens procuradas
(`servidor não é o wardd`, `cofre travado`, `Senha mestra`, `executar: entrada padrão
<tipo>` com `nula`/`pipe`/`arquivo`/`outra`/`terminal`, `tipo sem entrega`, `passam de
750 KiB`, `pedido negado`, `aprovação indisponível`, `o cofre travou; o comando foi
encerrado`, `ssh: token inválido, vencido…`, `linha de comando`, `fora do formato`,
`veio vazio`, `passa do tamanho`, `não é o do app`, `não está aberto`, `wardd[debug]`,
`/usr/local/bin/ward destravar`), as classes e ids do popup (`.cabecalho`, `.corpo-travado`,
`.popup-salvar`, `.rodape-salvar`, `[data-entrada]`, `#popup`, `#nome`…), o export
`preencherNaPagina` do `extension/page.js`, `pub const ID_EXTENSAO` do
`crates/ward-nm/src/lib.rs`, `aprovador.modo` e o log `wardd-aprovador-teste.log` existem
com esses nomes.

## Segurança

- Invariantes: os scripts são testes de ataque de S1, S2, S3, S4, S5, S8, S13 (CLI) e
  de D9/V6.2–V6.9 (extensão). Nenhuma conferência mudou: mesmos literais, números e
  operadores (comparação por tokens acima), mesmos padrões de `.dev/variations.txt`,
  mesmos limites de tempo. Nenhum segredo novo; o token da 27a e as senhas do "salvar
  login novo" continuam gerados na hora e nunca impressos.
- Testes de ataque escritos: nenhum novo (fase de nomes); os existentes foram renomeados.
  Não rodaram (PRD regra 6; `make test-cli` e `make test-extension` ficam para o M6).
- Dependências novas: nenhuma.

## Ficou de fora

- Literais pt-BR (subcomandos `destravar`/`lista`/`travar`, chaves do `entries.json`,
  `WARD_ENTRADA`/`WARD_ARQUIVO_*`, operações e campos do protocolo, chaves do `qa.json`,
  `WARD_TESTE_MANTER_PERFIL`, `ward-teste`, `aprovador.modo`): ficam até as fases de fio e
  de nomes externos (48, 52, 53, 55, 56).
- Suítes (`make test-cli`, `make test-extension`): Milestone 6.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 6d8cda8` (o worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `python3 -I dev/english_check.py --paths dev/tests/extension.py dev/tests/cli.py` (= `make english-check PATHS=…`) | 0 (0 achados, 0 problema nas exceções; antes: 3167) |
| comparação de tokens antigo × novo (script na scratchpad) | — (só as diferenças listadas) |
| `make compile` (cargo check do workspace e do `app/src-tauri`, `npm ci`, `vite build`, `syntax_check.py`: 0 erro) | 0 |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- **Conflito previsível com a Fase 39 (trilha W)**: o `extension.py` usa `data-entrada`,
  as classes do popup e o export `preencherNaPagina` do `extension/page.js`. Se a Fase 39
  renomear isso, ela (ou o merge) precisa ajustar os literais deste arquivo; quase todas
  as linhas daqui mudaram por nome, então o merge vai dar conflito textual nessas linhas.
- **Fase 28 (trilha R)**: troca `ID_EXTENSAO` → `EXTENSION_ID` no `ward-nm`; o
  `extension.py` lê `pub const ID_EXTENSAO` do fonte e precisa acompanhar (o
  `make extension` e outros scripts também).
- **Fases 52/53/55/56**: o `cli.py` procura no stdout/stderr textos e nomes de fio
  (`nome`, `entradas`, `tipo_entrada` no log, `WARD_ENTRADA`, subcomandos pt-BR); as
  fases que mudam o fio têm de mudar os literais daqui.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `dev/tests/cli.py` | alterado |
| `dev/tests/extension.py` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-40.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma conferência mudou; os ataques
  existentes acompanham os nomes)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
