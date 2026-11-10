# Fase 39 — (UI) Extensão: `extension/*.js`, `popup.html`, `popup.css`, `manifest.json` e o montador

## Implementado

Troca 1:1 pelo `docs/glossario.md` (seções 2, 12 e 14, mais a tabela nova "Código da
extensão e do montador (Fase 39)" em "Extensão (Fases 5 e 39)"), sem mudar comportamento
nem texto visível em pt-BR. O worktree nasceu em `b4d5e1b` (fora da linha do master);
com a árvore limpa, `git reset --hard 1b5735d`.

- **`page.js`** (funções injetadas): `pedeJanela` → `needsWindow`, `sondar` → `probe`,
  `sondarDepois` → `probeAfterSubmit`, `preencherNaPagina(origemEsperada, usuario,
  senhaB64, totp, enviar)` → `fillInPage(expectedOrigin, username, passwordB64, totp,
  submit)`; auxiliares e locais em inglês (`getter`, `typeOf`, `formOf`, `visible`,
  `write`, `destinationOk`, `doSubmit`…). Resultado de `probe`/`probeAfterSubmit`
  `{ origin, top, ready, password }`; de `fillInPage` `{ ok, reason, submitted, username,
  totp }`. Ficam iguais os valores de `reason` (`'quadro'`, `'origem'`, `'sem_campo'`,
  `'formulario'`, `'falha'`): o service worker os devolve ao popup como código de erro, no
  mesmo espaço dos códigos do daemon (fio, Fase 52). A regex do TOTP (com `codigo`) ficou
  byte a byte igual. O resultado só circula dentro da extensão (e no `extension.py`,
  ajustado).
- **`submit.js`**: `aoEnviar` → `onSubmit` e os auxiliares (`MINIMUM`, `getter`, `typeOf`,
  `valueOf`, `within`, `isForm`, `visible`, `isText`…). A mensagem ao service worker é a
  mesma: `{ t: 'enviado', 'usuario': username, 'senha': password }` (chaves do fio entre
  aspas, mesma ordem).
- **`login.js`**: `iniciarLogin` → `startLogin`, `porta`/`esperando` → `port`/`waiting`,
  `pedir`/`fechar`/`caiu`/`conectar`/`laco`/`atender`/`injetar`/`depoisDoEnvio`/`dormir`
  → `request`/`close`/`dropped`/`connect`/`loop`/`serve`/`inject`/`afterSubmit`/`sleep`,
  prazos `REPLY_TIMEOUT_MS`, `PAGE_TIMEOUT_MS`, `NO_NAVIGATION_MS`, `STEP_MS`. Operações,
  campos e desfechos do fio (`aguardar_login`, `preencher_login`, `resultado_login`,
  `'enviado'`, `'recusado'`, `'aba_mudou'`, `'sem_formulario'`, `'falha'`) iguais.
- **`service.js`**: `pendentes` → `pendingLogins` (`{ origin, url, username, password,
  timer }`), `descartar` → `discard`, `enviar` → `send`, `origemDa` → `originOf`,
  `urlDaEntrada` → `entryUrl`, `doContentScript` → `fromContentScript` (`{ tab, window,
  origin, url }`), `doPopup` → `fromPopup`, `aoEnviar` → `onSubmitted`, `pedirPorPorta` →
  `requestByPort`, `preencherComJanela` → `fillWithWindow`, `salvar` → `save`, `PRAZO_MS`
  → `TIMEOUT_MS`, `PRAZO_JANELA_MS` → `WINDOW_TIMEOUT_MS`, `AMBIENTES` → `ENVIRONMENTS`.
  Pedidos ao `ward-nm` e respostas ao popup com as mesmas chaves e na mesma ordem (as
  pt-BR entre aspas).
- **`popup.js`**: funções e constantes pela tabela do glossário (`showSuggestions`,
  `showLocked`, `showSave`, `fill`, `askService`, `hostSuggestions`, `textField`,
  `selectField`, `FAILURES`, `SAVE_FAILURES`, `FIELD_OF_PROBLEM`…). Import de
  `./brand.js` (`SYMBOL`, `{ box, t, d }`). O selo de estado tinha o mesmo valor como
  classe e como texto (`estado destravado` / "destravado"): agora a classe é
  `status unlocked|locked` e o texto sai de `STATUS_LABEL` (`'destravado'`/`'travado'`),
  igual ao de antes. Chaves dos mapas de falha que são códigos do fio entre aspas.
  Exemplo de host do comentário `painel.stage…` → `admin.stage…` (só comentário).
- **`popup.css`**: as 51 classes da tabela do glossário renomeadas (1:1, conferido por
  script: sem colisão, mesmo número de seletores e o CSS sem comentários idêntico com os
  nomes mascarados); comentários em inglês, nodes pelo id. Nenhuma regra, valor ou token
  mudou.
- **`popup.html`**: comentário em inglês; `fontes.css` → `fonts.css`.
- **`manifest.json`**: só o caminho dos ícones no pacote, `icones/extensao-<n>.png` →
  `icons/extension-<n>.png` (os campos do Chrome, a CSP, as permissões, o `description`
  e a versão não mudam; o manifesto não tem comentários).
- **`dev/extension/build.py`**: identificadores e docstring em inglês (`ROOT`, `SOURCE`,
  `FILES`, `FONTS`, `FORBIDDEN`, `FORBIDDEN_PER_FILE`, `check_manifest`, `check_package`,
  `fonts_css`, `brand_js`…); saídas do `dist/` `fonts.css`, `fonts/`, `brand.js`,
  `icons/extension-<n>.png`; comentários `GENERATED` dos arquivos gerados em inglês;
  prefixo das mensagens `montar-extensao:` → `extension-build:`. Mesmas conferências, na
  mesma ordem, com as mesmas listas; as mensagens de falha continuam em pt-BR. A fonte
  `docs/design/icones/extensao-destravado-<n>.png` fica (`docs/`).
- **`dev/tests/extension.py`** (achado da Fase 40): seletores das classes novas, ids
  `#entry-name`/`#entry-username`/`#entry-type`/`#entry-environment`/`#entry-group`,
  `[data-entry=…]`, `m.fillInPage` e `{"ok": False, "reason": "origem"}`. As chaves dos
  objetos montados em JS dentro do Python (`cabecalho:`, `rotulo:`…), lidas só pelo
  Python, e as mensagens internas (`{t: 'pendente', aba: 0}`, `["origem", "usuario"]`)
  ficam.
- **Glossário**: tabela da Fase 39.

### Literais alterados (PRD regra 5; todos são nomes)

Conferido pelo `--literals` do checador, antigo × novo, arquivo a arquivo:

- Classes do popup (tabela do glossário) e `status`/`unlocked`/`locked` do selo; ids do
  formulário de salvar `nome`/`usuario`/`tipo`/`ambiente`/`grupo` → `entry-name`/
  `entry-username`/`entry-type`/`entry-environment`/`entry-group` (com prefixo, para não
  virarem propriedade com nome do `<form>`, como `form.name`/`form.type`); `data-entrada`
  → `data-entry` (`dataset.entry`).
- Arquivos do pacote: `./marca.js` → `./brand.js`, `fontes.css` → `fonts.css`, `fontes/`
  → `fonts/`, `icones/extensao-<n>.png` → `icons/extension-<n>.png`; chave `caixa` →
  `box` do `brand.js`; `SIMBOLO` → `SYMBOL`.
- `build.py`: prefixo `extension-build:`, comentários `GENERATED …` dos arquivos gerados,
  e as f-strings só nas variáveis interpoladas (`{file_name}`, `{family}`, `{weight}`,
  `{forbidden}`, `{PERMISSIONS}`, `{pattern}`).
- Literais novos sem mudança de bytes no fio: chaves pt-BR que eram shorthand ou
  identificador e passaram a chave entre aspas (`'dados'`, `'origem'`, `'entrada'`,
  `'resultado'`, `'aba'`, `'documento'`, `'nome'`, `'grupo'`, `'ambiente'`, `'tipo'`,
  `'campos'`, `'usuario'`, `'senha'`, `'erro'`, `'problemas'`, `'campo'`, `'motivo'`).
- `extension.py`: só os seletores, ids, o export e a chave `reason` acima.
- Ficaram de propósito: operações, respostas, campos e códigos do protocolo e das
  mensagens internas (Fase 52), os motivos de `fillInPage` e os desfechos do `ward login`
  (viram código do fio), o regex `ID_EXTENSAO` do `build.py` (nome do Rust; Fase 28), os
  textos pt-BR do popup, do manifesto e das mensagens do montador.

### Entradas novas de `dev/english-check/frozen.txt` (todas Fase 52)

- `extension/login.js`: `tipo`, `dados`, `codigo`, `usuario`, `senha`.
- `extension/popup.js`: `tipo`, `dados`, `codigo`, `itens`, `nome`, `usuario`, `senha`,
  `ambiente`, `origem`, `erro`, `problemas`, `campo`, `motivo`.
- `extension/service.js`: `tipo`, `dados`, `codigo`, `itens`, `usuario`, `senha`, `aba`,
  `documento`, `entrada`, `origem`, `nome`, `grupo`, `ambiente`, `campos`, `problemas`,
  `campo`, `motivo`.

Todas são acesso a membro (`r.tipo`, `msg.aba`, `d.usuario`…) de campos do fio; os nomes
locais desses arquivos foram traduzidos mesmo assim (o `frozen.txt` cobre a palavra no
arquivo inteiro, então foram conferidos à mão).

## Segurança

- Invariantes: S1 (a senha não volta ao popup; o service worker é quem junta a senha no
  salvar; `fillInPage` devolve só booleanos e o motivo), S11 (CSP do manifesto e do
  `popup.html` iguais; o montador continua recusando `unsafe-*`, URL remota, `eval`,
  `innerHTML`, armazenamento, ouvintes proibidos e script/estilo inline), S13 (nada no
  console), D9 (a origem é conferida no daemon, de novo na sonda e de novo dentro de
  `fillInPage`, antes de escrever), V6.6 (só o service worker escuta mensagens e confere o
  `sender`; o content script só aceita `submit` confiável com ativação do usuário),
  DOM clobbering (todo acesso ao DOM continua pelos protótipos do mundo isolado). Só nomes
  e comentários mudaram: a ordem das conferências, os limites (`MAX` 2048, `MINIMUM` 4,
  3 min, 130 s, 25/60 s), a limpeza dos valores (`args.fill('')`, `d.senha = ''`,
  `bytes.fill(0)`, `password = ''` ao descartar) e os ramos de erro são os mesmos linha a
  linha. As listas de padrões proibidos por arquivo do `build.py` são as mesmas (os
  comentários novos foram conferidos contra elas: `submit.js` sem `.then(`, `login.js`
  sem `console.`/`sendMessage`, `page.js` sem `sendMessage`).
- Testes de ataque: nenhum novo (fase de nomes). O `extension.py` (V6.2–V6.9, ataque com
  `fillInPage` numa origem errada) acompanha os nomes; não rodou (PRD regra 6).
- Dependências novas: nenhuma.

## Ficou de fora

- Nomes do fio (native messaging e mensagens internas `chrome.runtime`): Fase 52
  (`frozen.txt`). A versão do manifesto (0.2.0, DT9) também é de fase posterior.
- `ID_EXTENSAO` lido pelo `build.py` (regex no fonte do `ward-nm`): muda com a Fase 28.
- `docs/mission/design-nodes.md` (cita classes e o caminho do popup): Fase 59.
- Suítes (`make extension`, `make test-extension`, `make test-login`, QA do V3.5):
  Milestone 6.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 1b5735d` (worktree nasceu em `b4d5e1b`, árvore limpa) | 0 |
| `python3 -I dev/english_check.py --paths extension dev/extension dev/tests/extension.py` (= `make english-check PATHS=…`) | 0 (0 achado, 0 problema nas exceções; antes: 2672 achados; `app/src` continua com 0) |
| `--literals` antigo × novo dos 8 arquivos (checador copiado para a scratchpad) | — (só as diferenças listadas) |
| conferência do `popup.css` (classes 1:1, CSS sem comentários igual com nomes mascarados) e das classes usadas no `popup.js` | — (ok) |
| `make compile` (log `compile-fase39.log` na scratchpad) | 0 (cargo check do workspace e do `app/src-tauri`, `npm ci`, `vite build`, `syntax_check.py`: 0 erro, nenhum aviso) |
| `node` importando `extension/page.js` (conferência rápida dos exports `needsWindow`, `probe`, `probeAfterSubmit`, `fillInPage`) | 0 |
| `python3 -I -B dev/extension/build.py` (montagem e conferências do pacote; o `dist/` foi apagado depois) | 0 (id `nhkehjnmlgkebhmelajfpmpblepaichf`, igual ao do `ward-nm`; `dist/` com `brand.js`, `fonts.css`, `fonts/`, `icons/extension-<n>.png`) |

## Problemas descobertos

- **Fase 28 (trilha R)**: ao trocar `ID_EXTENSAO` → `EXTENSION_ID` no `ward-nm`, além do
  `extension.py` tem de mudar o regex e as mensagens de `dev/extension/build.py`
  (`host_id`/`check_manifest`).
- **Fase 52**: além das chaves do fio, decide os motivos de `fillInPage` e os códigos
  locais do service worker (`'sem_resposta'`, `'pedido_invalido'`, `'expirou'`), que
  dividem com os códigos do daemon as chaves de `FAILURES`/`SAVE_FAILURES` do popup; e o
  `extension.py` compara `{"ok": False, "reason": "origem"}`.
- **Fase 59**: `design-nodes.md` cita as classes antigas do popup.
- **Merge com a Fase 40** já está no master: o `extension.py` desta fase parte dele; com a
  Fase 52/42 rodando ao mesmo tempo, conflito textual possível nas linhas dos seletores do
  `extension.py` e no fim do `frozen.txt`.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `extension/login.js`, `page.js`, `popup.js`, `service.js`, `submit.js` | alterado |
| `extension/popup.css`, `popup.html`, `manifest.json` | alterado |
| `dev/extension/build.py` | alterado |
| `dev/tests/extension.py` | alterado |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-39.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma conferência mudou; os ataques
  existentes do `extension.py` acompanham os nomes)
- [x] (UI) Conferido contra o FigCraft: nenhuma medida, cor ou token mudou (CSS igual com
  os nomes mascarados); o QA visual (V3.5) é do Milestone 6
- [x] Ficou dentro do escopo da fase (exceção: os literais do `extension.py`, pedidos
  pelo plano)
