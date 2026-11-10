# Fase 36 — (UI) `Form`, `Credentials` e `ChooseType`

## Implementado

Troca 1:1 pelo `docs/glossario.md` (seção 4, mais a tabela nova "Telas `Form`,
`Credentials` e `ChooseType` (Fase 36)"), sem mudar comportamento nem texto visível em
pt-BR. Parte de `master` 6c046d4.

- **Arquivos** (`git mv`): `screens/Formulario.svelte` → `Form.svelte`, `formulario.js` →
  `form.js`, `Credenciais.svelte` → `Credentials.svelte`, `credenciais.js` →
  `credentials.js`, `EscolherTipo.svelte` → `ChooseType.svelte`. `App.svelte` importa
  `./screens/Credentials.svelte`.
- **`form.js`**: exports em inglês (`FORM_TYPES`, `FORM_ENVIRONMENTS`, `AWS_REGIONS`,
  `SECURITY_OPTIONS`, `ENGINES`, `PROTOCOLS`, `AUTHENTICATIONS`, `EXTRAS_TEXTS`,
  `ATTACHMENTS_TEXTS`, `TYPE_CARDS`, `databaseAccess`, `defaultPort`, `fieldsOfType`,
  `fieldVisible`, `readableSize`, `attachmentVariable`, `urlWithPunycode`, `tagsOf`,
  `newIsEmpty`, `buildForm`, `errorsByField`; `nfc` fica). Definição de campo `{ id,
  label, kind, options, local, generate, auth, multiline, port, inputMode, help, … }`;
  `kind` `text/secret/select/totp/note`; select local `authentication` com
  `password`/`key`; linha TOTP `code`. Chaves de tipo do fio entre aspas (`'banco'`…),
  como em `types.js`.
- **`buildForm`** devolve exatamente o mesmo objeto de `salvar_credencial` (chaves do fio
  entre aspas: `tipo`, `nome`, `grupo`, `ambiente`, `projeto`, `tags`, `campos`,
  `segredos`, `extras`, `anexos`, `anexos_mantidos`; valores `vazio`/`manter`/`gerada`/
  `carregada`/`{ novo }`). O estado interno do segredo virou `{ mode, typed, length }`
  com `saved`/`typing`/`generated`/`loaded`, e o mapa para o fio é o mesmo braço a braço.
- **`credentials.js`**: `ENVIRONMENTS`, `EMPTY_FILTERS` (`{ group, kind, environment }`),
  `normalize`, `countEntries`, `activeFilters`, `matchesSearch`, `filterItems`,
  `groupsOf` (`{ name, color }`), `groupItems` (`{ …, items, count }`), `topSummary`,
  `filtersSummary`, `LEVELS` (`{ label, icon, explanation }`). Os filtros agora usam o
  mesmo formato do `FiltersDialog`; a conversão `grupo/tipo/ambiente` da Fase 35 saiu
  (o `aplicarFiltros` e o `filters={{ … }}` do diálogo ficam com as mesmas chaves).
- **Telas**: identificadores, props (`Form` `kind`/`detail`/`groups`/`servers`/
  `oncancel`/`onsaved`; `ChooseType` `onchoose`/`oncancel`; `Credentials` sem o alias
  `onlocked: ontravado`), modos da tela (`detail`/`choose`/`form`), classes locais e
  comentários em inglês. Comentários citam os nodes do FigCraft pelo id (regra da Fase
  35) e não citam nomes de fio em pt-BR (comandos descritos em inglês).
- **Glossário**: tabela da Fase 36 (exports, campos, valores internos, props, modos, ids
  e `data-*`).

### Literais alterados (PRD regra 5; todos são nomes)

- Caminhos de import: `./formulario.js` → `./form.js`, `./credenciais.js` →
  `./credentials.js`, `./Formulario.svelte` → `./Form.svelte`, `./EscolherTipo.svelte` →
  `./ChooseType.svelte`, `./screens/Credenciais.svelte` → `./screens/Credentials.svelte`.
- Valores internos (nunca saem da tela): `kind` do campo `texto/segredo/nota` →
  `text/secret/note`; `auth`/opções do select local `senha`/`chave` → `password`/`key`;
  id local `autenticacao` → `authentication` (filtrado por `local: true`, não vai no
  pedido) e `codigo` (linha TOTP, sem valor) → `code`; chaves de opções `regioes`,
  `segurancas`, `motores`, `protocolos`, `autenticacoes`, `servidores`, `saltos` →
  `regions`, `security`, `engines`, `protocols`, `authentications`, `servers`, `jumps`;
  modo do segredo `salvo/digitando/gerada/carregada` → `saved/typing/generated/loaded`;
  modo da tela `detalhe/escolher/formulario` → `detail/choose/form`.
- Ids do DOM: `form-nome/tipo/grupo/ambiente/projeto/codigo` →
  `form-name/kind/group/environment/project/code`; `formulario-credencial` (e o `form=`
  do botão) → `credential-form`; `aviso-anexo` → `attachment-notice`; `erro-anexos` →
  `attachments-error`; `secao-acesso/extras/anexos` → `section-access/extras/attachments`;
  `titulo-credenciais` → `credentials-title`; `busca` → `search`; `campo-<id>` →
  `field-<id>`; `segredo-<id>` → `secret-<id>`; `grupo-<cor>-<nome>` →
  `group-<color>-<name>`. `form-texto` virou `form-{c.id}` (o único campo nota tem id
  `texto`: mesmo valor). `form-<id do fio>` e `extra-<…>` ficam.
- `data-*` do app: `data-entrada` → `data-entry` (usos em `dev/tests/app_e2e.py` e
  `revealed_selection.py` ajustados), `data-acao="carregar-chave"` →
  `data-action="load-key"`, `data-conectado="sim"` → `data-connected="yes"` (ninguém lê
  esses dois; conferido em `app/`, `dev/`, `extension/`, `crates/`).
- Classes locais (escopadas pelo Svelte; nenhuma global nem lida fora): `cabecalho` →
  `header`, `corpo` → `body`, `grade` → `grid`, `rodape` → `footer` etc.
- Chaves do fio passaram de shorthand a chave entre aspas (`{ entrada, campo }` →
  `{ 'entrada': …, 'campo': id }`, `formulario` → `'formulario': form`): mesmos bytes.
- Template strings só mudaram nas variáveis interpoladas; o texto pt-BR é o mesmo.

### Entradas novas de `dev/english-check/frozen.txt` (todas Fase 54)

- `app/src/screens/credentials.js`: `nome`, `tipo`, `grupo`, `ambiente`, `endereco`,
  `usuario`.
- `app/src/screens/Credentials.svelte`: `nome`, `tipo`, `ambiente`, `resumo`,
  `subtitulo`, `campos`, `segredos`, `segredo`, `rotulo`, `valor`, `nivel`, `criada_em`,
  `alterada_em`, `mensagem`.
- `app/src/screens/form.js`: `motor`, `protocolo`, `seguranca`, `campo`, `posicao`,
  `motivo`.
- `app/src/screens/Form.svelte`: `nome`, `grupo`, `ambiente`, `projeto`, `campos`,
  `segredos`, `segredo`, `valor`, `anexos`, `tamanho`, `nivel`, `resultado`, `motivo`,
  `restante`, `codigo`, `salvo`, `erros`, `formulario`, `mensagem`, `motor`,
  `protocolo`, `regiao`, `tunel`, `salto`, `totp_semente`.

## Segurança

- Invariantes: S11 (o formulário nunca recebe segredo salvo; revelar só por pedido,
  valor some em 30 s, ao esconder e ao sair; o que é digitado fica só no estado da tela e
  `descartar_formulario` no `onDestroy`), S2 (chave carregada fica no Rust) e S9 (nível
  só pelo `LevelDialog`, com PIN) mantidas: só nomes e comentários mudaram. O mapa
  modo → valor do fio de `buildForm` é o mesmo braço a braço (`saved`→`manter`,
  `generated`→`gerada`, `loaded`→`carregada`, digitado vazio→`vazio`, autenticação da
  outra forma→`vazio`, nota em NFC). Nenhum segredo em estado global, armazenamento ou
  URL; nenhum comando Tauri novo.
- Testes de ataque: nenhum novo (fase de nomes); `revealed_selection.py` (ataque à
  seleção do valor revelado, V3.4) acompanha `data-entry`.
- Dependências novas: nenhuma.

## Ficou de fora

- Nomes de fio (comandos Tauri, campos, valores e ids de campo): Fase 54 (`frozen.txt`).
- `data-entrada` do popup da extensão (`extension/popup.js`, `dev/tests/extension.py`):
  Fase 39.
- `docs/mission/design-nodes.md` cita `app/src/telas/Credenciais.svelte`,
  `Formulario.svelte`, `formulario.js`, `EscolherTipo.svelte` e a classe `.esmaecer`:
  Fase 59 (PRD regra 2).
- Suítes (`app_e2e.py`, `revealed_selection.py`, QA visual V3.2/V3.3): Milestone 6
  (PRD regra 6).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `python3 -I dev/english_check.py --paths app/src/screens/{Form.svelte,form.js,Credentials.svelte,credentials.js,ChooseType.svelte}` (= `make english-check PATHS=…`) | 0 (0 achados, 0 problema nas exceções) |
| `make compile` (cargo check do workspace e do `app/src-tauri`, `npm ci`, `vite build`, `syntax_check.py`: 0 erro) | 0 |
| `npm run build:teste` (conferência rápida do pacote de teste) | 0 |
| comparação de literais antigo × novo dos 5 arquivos (script na scratchpad) | — (só os nomes listados acima) |

O `vite build` mostra os mesmos 10 avisos `state_referenced_locally` de antes, agora em
`screens/Form.svelte` (linhas 75–86).

## Problemas descobertos

- O checador trata valor estático de atributo (`id="form-texto"`) como identificador;
  resolvido usando `form-{c.id}`, que dá o mesmo valor.
- Chave de fio em literal de objeto vai entre aspas (como `types.js` na Fase 35) e acesso
  de propriedade do fio vai por ponto com entrada no `frozen.txt`: a Fase 54 precisa
  procurar os dois (o `--literals` acha os entre aspas).
- `make compile` com o disco em ~7 GB livres (outra trilha compilando ao mesmo tempo);
  `target/`, `app/src-tauri/target`, `app/node_modules`, `app/dist*` apagados no fim.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/src/screens/Formulario.svelte` → `app/src/screens/Form.svelte` | renomeado e reescrito |
| `app/src/screens/formulario.js` → `app/src/screens/form.js` | renomeado e reescrito |
| `app/src/screens/Credenciais.svelte` → `app/src/screens/Credentials.svelte` | renomeado e reescrito |
| `app/src/screens/credenciais.js` → `app/src/screens/credentials.js` | renomeado e reescrito |
| `app/src/screens/EscolherTipo.svelte` → `app/src/screens/ChooseType.svelte` | renomeado e reescrito |
| `app/src/App.svelte` | alterado (import) |
| `dev/tests/app_e2e.py`, `dev/tests/revealed_selection.py` | alterado (`data-entry`, `#form-name`) |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-36.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma lógica de segurança mudou; o teste
  existente de V3.4 acompanha os nomes)
- [x] (UI) Conferido contra o FigCraft: nenhuma medida, cor ou token mudou (só nomes de
  classe locais; mesmas regras CSS)
- [x] Ficou dentro do escopo da fase
