# Fase 38 — (UI) `Settings`, `Unlock`, `ChangePassword`, `ChangePin`, `ChooseUsbDrive`, `lib/bridge` e `Showcase`

## Implementado

Troca 1:1 pelo `docs/glossario.md` (seções 2 e 4, mais a tabela nova "Telas `Settings`,
`Unlock`, `ChangePassword`, `ChangePin`, `ChooseUsbDrive`, `lib/bridge` e `Showcase`
(Fase 38)"), sem mudar comportamento nem texto visível em pt-BR. Parte de `master`
6d8cda8.

- **Arquivos** (`git mv`): `screens/Configuracoes.svelte` → `Settings.svelte`,
  `configuracoes.js` → `settings.js`, `Destravar.svelte` → `Unlock.svelte`,
  `TrocarSenha.svelte` → `ChangePassword.svelte`, `TrocarPin.svelte` → `ChangePin.svelte`,
  `EscolherPendrive.svelte` → `ChooseUsbDrive.svelte`. `App.svelte` importa
  `./screens/Settings.svelte` e `./screens/Unlock.svelte`.
- **`settings.js`**: `MIN_LOCK_MINUTES`, `MAX_LOCK_MINUTES`, `LOCK_OPTIONS`,
  `minutesLabel`, `lockOptions`, `usbDriveName`, `situationInList`, `usbDriveCard`
  (`{ name, situation, status, mounted }`), `LOCAL_BACKUP_WARNING`. Os campos do fio
  (`rotulo`, `montado`, `pendrive`, `pendrives_visiveis`) ficam.
- **`Settings.svelte`**: estado e funções em inglês; a config local virou
  `{ lockMinutes, externalBackup, localBackup }` (lida da resposta de `configuracoes`,
  cujos campos ficam); `dialog` com `password`/`pin`/`usb-drive`/`restore`/`kit`; prop
  `onlocked` sem o alias da Fase 35; classes locais e ids do DOM em inglês.
- **`Unlock.svelte`**: props `request`, `onunlocked`, `onerror` sem o alias da Fase 35;
  `MAX_WAIT`, `OPERATION_COMMAND` (a chave do fio `'executar'` entre aspas),
  `operationCommand`, `withRequest`, `consecutiveErrors`, `dismissed`, `buttonLabel`
  etc.; id `senha-mestra` → `master-password` (com os usos em `dev/tests/app_e2e.py` e
  `revealed_selection.py`).
- **`ChangePassword.svelte`/`ChangePin.svelte`**: props `open`, `onclose`, `onchanged`,
  `onfailure` (e `onlocked` no PIN); valores locais `current`/`next`/`confirmation`;
  `errors` com as chaves do fio entre aspas (`'atual'`, `'nova'`/`'novo'`,
  `'confirmacao'`), porque o `erros[].campo` da resposta indexa esse objeto; o pedido
  `{ 'dados': { 'atual', 'nova'|'novo', 'confirmacao' } }` tem as mesmas chaves e a mesma
  ordem; `refusal(field)`, `focusField(field)`, `wrongPin`; ids `change-password-*`,
  `change-pin-*`; `data-test` e `data-tone` (antes `data-teste`, `data-tom`).
- **`ChooseUsbDrive.svelte`**: props `open`, `replace`, `current`, `onclose`, `onchosen`,
  `onlocked`; `usbDrives`, `chosen`, `load`, `confirm` etc.
- **`lib/bridge/*`**: `COMANDOS` → `COMMANDS` (a lista de comandos é a mesma, byte a
  byte; o teste `configuracao_tauri.rs` confere só o `Object.freeze([...])`);
  `AVISOS` → `NOTICES`, `obterTransporte` → `getTransport`, `falha` → `failure`;
  `criarTransporte` → `createTransport`, `lerToken` → `readToken`, `ouvirPorConsulta` →
  `listenByPolling`; parâmetros e locais em inglês; comentários em inglês. O objeto de
  erro continua `{ codigo, mensagem }` (do fio), na mesma ordem.
- **`lib/test/Showcase.svelte`**: comentário, `modalOpen`, `environment`, classes
  `.showcase`/`.row`/`.list`, ids `v-group`/`v-password`/`v-environment`.
- **Comentários**: em inglês; nodes do FigCraft pelo id (regra da Fase 35); sem nome de
  fio nem de identificador Rust em pt-BR (os comandos e módulos são descritos em inglês).
- **Glossário**: tabela da Fase 38 (exports, estado local, props, ids, `data-*` e a
  ponte).

### Literais alterados (PRD regra 5; todos são nomes)

- Caminhos de import: `./screens/Configuracoes.svelte` → `./screens/Settings.svelte`,
  `./screens/Destravar.svelte` → `./screens/Unlock.svelte`, `./configuracoes.js` →
  `./settings.js`, `./TrocarPin.svelte`/`./TrocarSenha.svelte`/`./EscolherPendrive.svelte`
  → `./ChangePin.svelte`/`./ChangePassword.svelte`/`./ChooseUsbDrive.svelte`.
- Valores internos do `dialog` de Configurações: `senha`/`pendrive`/`restaurar` →
  `password`/`usb-drive`/`restore` (`pin`, `kit` ficam). Nunca saem da tela.
- Ids do DOM: `senha-mestra` → `master-password` (usos nos dois scripts de teste
  ajustados); `configuracoes-titulo`, `acesso-titulo`, `backup-titulo` →
  `settings-title`, `access-title`, `backup-title`; `config-senha`, `config-trava`,
  `config-tempo-trava`, `config-backup-local`, `config-backup-local-aviso`,
  `config-backup-externo` → `config-password`, `config-lock`, `config-lock-timeout`,
  `config-local-backup`, `config-local-backup-warning`, `config-external-backup`;
  `trocar-senha-atual/nova/confirmacao` → `change-password-current/new/confirmation`;
  `trocar-pin-atual/novo/confirmacao` → `change-pin-current/new/confirmation`;
  `v-grupo`/`v-senha`/`v-ambiente` → `v-group`/`v-password`/`v-environment`. Nenhum
  outro é lido fora da tela (conferido em `app/`, `dev/`, `extension/`, `crates/`).
- `data-teste="trocar-senha"`, `"trocar-senha-aviso-backups"`, `"trocar-pin"` →
  `data-test="change-password"`, `"change-password-backups-notice"`, `"change-pin"`
  (ninguém lê); `data-tom` → `data-tone` (atributo e seletores CSS do mesmo arquivo).
- Classes locais (escopadas pelo Svelte; nenhuma global nem lida fora): `principal` →
  `main`, `topo` → `top`, `colunas` → `columns`, `acesso` → `access`, `nota` → `note`,
  `janela` → `window`, `formulario` → `form`, `com-pedido` → `with-request`, `corpo` →
  `body`, `forca` → `strength`, `barra(s)` → `bar(s)`, `legenda` → `caption`, `opcao` →
  `option`, `escolhida` → `chosen`, `vitrine` → `showcase`, `linha` → `row` etc. Mesmas
  regras CSS; nenhum choque com as classes globais de `tokens.css`/`base.css`.
- Chaves do fio passaram de shorthand a chave entre aspas (`{ ligado: … }` →
  `{ 'ligado': … }`, `{ minutos: novo }` → `{ 'minutos': next }`, `{ senha: enviada }` →
  `{ 'senha': sent }`, `{ dados: { atual, nova, confirmacao } }` →
  `{ 'dados': { 'atual': current, 'nova': next, 'confirmacao': confirmation } }`,
  `{ canal }` → `{ 'canal': channel }`, `{ codigo, mensagem }` →
  `{ 'codigo': code, 'mensagem': message }`): mesmos bytes no fio.
- Template strings só mudaram nas variáveis interpoladas (`${espera}` → `${wait}`,
  `${resto}` → `${rest}`, `${forca.rotulo}` → `${strength.rotulo}`, `/ponte/${comando}` →
  `/ponte/${command}`); o texto é o mesmo.
- Ficaram de propósito: `name="pendrive"` do grupo de rádio (o checador não acusa), o
  caminho `/ponte/<comando>`, o `&tela=` da URL de teste, o modo `'teste'` do Vite, os
  códigos de erro (`'travado'`, `'senha-incorreta'`, `'pin-incorreto'`,
  `'pedido-invalido'`, `'pendrive-nao-conectado'`, `'comando-desconhecido'`,
  `'aviso-desconhecido'`, `'ponte-indisponivel'`, `'ponte-invalida'`, `'falha'`), os
  nomes de aviso e de comando Tauri.

### Entradas novas de `dev/english-check/frozen.txt` (todas Fase 54)

- `app/src/lib/bridge/index.js`: `codigo`, `mensagem`, `aviso`.
- `app/src/lib/bridge/test-bridge.js`: `codigo`, `mensagem`, `valor`.
- `app/src/screens/settings.js`: `rotulo`, `montado`, `pendrive`, `pendrives_visiveis`.
- `app/src/screens/Settings.svelte`: `minutos_trava`, `backup_externo`, `ligado`,
  `estado`, `instante`, `entradas`, `codigo`, `mensagem`.
- `app/src/screens/Unlock.svelte`: `restante`, `entrada`, `operacao`, `comando`,
  `codigo`, `mensagem`.
- `app/src/screens/ChangePassword.svelte`: `dados`, `atual`, `nova`, `confirmacao`,
  `trocado`, `erros`, `campo`, `motivo`, `codigo`, `mensagem`.
- `app/src/screens/ChangePin.svelte`: `dados`, `atual`, `novo`, `confirmacao`, `trocado`,
  `erros`, `campo`, `motivo`, `estado`, `codigo`, `mensagem`.
- `app/src/screens/ChooseUsbDrive.svelte`: `codigo`, `mensagem`.

## Segurança

- Invariantes: S1/S11/S13 (senha mestra só no estado local de `Unlock`, sai antes da
  resposta e no `onDestroy`; nada em estado global, armazenamento ou URL), S9 (PIN atual
  e novo só no estado local de `ChangePin`, apagados antes da resposta e ao fechar; o
  5.º erro seguido trava o cofre, conferido pelo `estado_do_cofre`), D52 (senha atual
  na troca), D51 (texto do agente e rótulo do udisks só como texto, isolados) e D7 (o
  token da ponte só na variável do módulo, tirado da URL e do histórico; `credentials:
  'omit'`, `redirect: 'error'`, `no-referrer`). Só nomes e comentários mudaram: a ordem
  das operações (copiar o valor, apagar o estado, enviar, `args.dados = null` no
  `finally`), as conferências e os ramos de erro são os mesmos linha a linha. A ponte
  segue recusando comando fora de `COMMANDS` e aviso fora de `NOTICES`; o ramo de teste
  continua atrás de `import.meta.env.MODE === 'teste'`, então some do `dist/`.
- Testes de ataque: nenhum novo (fase de nomes). `revealed_selection.py` (V3.4) e
  `app_e2e.py` acompanham o id `#master-password`.
- Dependências novas: nenhuma.

## Ficou de fora

- Nomes exportados por módulos da Fase 37 (rodando ao mesmo tempo noutro worktree):
  `primeiro-uso.js` (`MENSAGENS` e as chaves `senhaDiferente`, `pinComLetra`,
  `pinLongo`, `pinInvalido`, `pinDiferente`; `erroDaSenha`, `forcaDaSenha` e o objeto de
  força `{ rotulo, segmentos, tom }` com os valores `perigo`/`aviso`/`sucesso` que os
  seletores `[data-tone=…]` de `ChangePassword` usam; `mensagemDoRust`; `MAX_DIGITOS_PIN`;
  `pinValido`), `restaurar.js` (`mensagemRestaurado`) e as props de
  `KitEmergencia.svelte`/`RestaurarBackup.svelte` (`aberto`, `onfechar`, `ontravado`,
  `onrestaurado`). Aqui eles são importados com alias em inglês
  (`mensagemDoRust as rustMessage`, `MENSAGENS as MESSAGES`, `erroDaSenha as
  passwordError`, `forcaDaSenha as passwordStrength`, `MAX_DIGITOS_PIN as
  MAX_PIN_DIGITS`, `pinValido as validPin`, `mensagemRestaurado as restoredMessage`;
  `KitEmergencia`/`RestaurarBackup` como `EmergencyKit`/`RestoreBackup`, apontando para
  o arquivo antigo), então os usos nas minhas telas já estão em inglês. São os 25
  achados que sobram no `english-check` (lista em "Comandos").
- Nomes de fio (comandos Tauri, campos, avisos, códigos de erro, `/ponte/`, `&tela=`):
  Fase 54 (`frozen.txt`).
- `docs/mission/design-nodes.md` cita `app/src/telas/Destravar.svelte`,
  `Configuracoes.svelte`, `TrocarSenha.svelte`, `TrocarPin.svelte`,
  `EscolherPendrive.svelte`, `configuracoes.js` e `config-kit`: Fase 59 (PRD regra 2).
- Suítes (`app_e2e.py`, `revealed_selection.py`, QA visual e de teclado V3.2/V3.3):
  Milestone 6 (PRD regra 6).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `python3 -I dev/english_check.py --paths app/src/screens/{Settings.svelte,settings.js,Unlock.svelte,ChangePassword.svelte,ChangePin.svelte,ChooseUsbDrive.svelte} app/src/lib/bridge app/src/lib/test` (= `make english-check PATHS=…`) | 1 (25 achados, todos nomes da Fase 37 listados em "Ficou de fora"; 0 problema nas exceções; `lib/bridge`, `lib/test`, `settings.js`, `Unlock.svelte` e `ChooseUsbDrive.svelte` com 0 achado) |
| `make compile` (cargo check do workspace e do `app/src-tauri`, `npm ci`, `vite build`, `syntax_check.py`) | 0 (0 erro; só os 10 avisos `state_referenced_locally` de `screens/Form.svelte` que já existiam) |
| `npm run build:teste` (conferência rápida: o `dist-teste` tem `test-bridge-*.js` e `Showcase-*.js`, e os nomes `createTransport`, `listenByPolling`, `requestedScreen` batem entre o chunk da ponte e o `index`) | 0 |
| comparação de literais antigo × novo dos 11 arquivos (script na scratchpad) | — (só os nomes listados acima) |

Achados que sobram (todos da Fase 37): `ChangePassword.svelte` `MENSAGENS`,
`erroDaSenha`, `forcaDaSenha`, `senhaDiferente`, `segmentos`, `rotulo`;
`ChangePin.svelte` `MAX_DIGITOS_PIN`, `MENSAGENS`, `pinValido`, `pinComLetra`,
`pinLongo`, `pinInvalido`, `pinDiferente`; `Settings.svelte` `mensagemRestaurado`,
`aberto`, `onfechar`, `ontravado`, `onrestaurado` (nas instâncias de `KitEmergencia` e
`RestaurarBackup`). `mensagemDoRust` não aparece porque as entradas `mensagem` do
`frozen.txt` cobrem a palavra.

## Problemas descobertos

- **Merge com a Fase 37** (mesmos arquivos, linhas próximas): conflitos esperados em
  `app/src/App.svelte` (imports das telas, linhas 13–18), `docs/glossario.md` (as duas
  tabelas novas entram antes de "### Extensão"), `dev/english-check/frozen.txt` (fim do
  arquivo) e nas linhas de import e de uso dos nomes da Fase 37 em `Settings.svelte`,
  `ChangePassword.svelte`, `ChangePin.svelte` e `ChooseUsbDrive.svelte` (a Fase 37 os
  troca nos arquivos com o nome antigo). Receita: ficar com a versão da Fase 38 desses
  quatro arquivos e trocar só os imports (`./primeiro-uso.js` → `./first-use.js`,
  `./restaurar.js` → `./restore.js`, `./KitEmergencia.svelte`/`./RestaurarBackup.svelte`
  → `./EmergencyKit.svelte`/`./RestoreBackup.svelte`, tirando os aliases se os nomes da
  37 forem os mesmos), as chaves de `MESSAGES`, o objeto de força
  (`strength.segmentos/tom/rotulo` e os valores dos seletores `[data-tone=…]`) e as props
  de `EmergencyKit`/`RestoreBackup` em `Settings.svelte` pelos nomes que a 37 escolheu.
  Depois, `english-check` nesses quatro arquivos tem de dar 0.
- `crates/ward-test-bridge/src/http.rs:392` cita `ponte-teste.js` › `ouvirAvisos`
  (agora `test-bridge.js` › `listenByPolling`): é da trilha R (Fase 29), não mexi.
- As entradas `mensagem` do `frozen.txt` cobrem também a palavra dentro de
  `mensagemDoRust`/`mensagemRestaurado` (casamento por palavra): o checador não os acusa
  enquanto o `frozen.txt` tiver `mensagem` para o arquivo.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/src/screens/Configuracoes.svelte` → `app/src/screens/Settings.svelte` | renomeado e reescrito |
| `app/src/screens/configuracoes.js` → `app/src/screens/settings.js` | renomeado e reescrito |
| `app/src/screens/Destravar.svelte` → `app/src/screens/Unlock.svelte` | renomeado e reescrito |
| `app/src/screens/TrocarSenha.svelte` → `app/src/screens/ChangePassword.svelte` | renomeado e reescrito |
| `app/src/screens/TrocarPin.svelte` → `app/src/screens/ChangePin.svelte` | renomeado e reescrito |
| `app/src/screens/EscolherPendrive.svelte` → `app/src/screens/ChooseUsbDrive.svelte` | renomeado e reescrito |
| `app/src/lib/bridge/commands.js`, `index.js`, `test-bridge.js` | alterado |
| `app/src/lib/test/Showcase.svelte` | alterado |
| `app/src/App.svelte` | alterado (imports) |
| `dev/tests/app_e2e.py`, `dev/tests/revealed_selection.py` | alterado (`#master-password`) |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-38.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma lógica de segurança mudou; o teste
  existente de V3.4 acompanha o id novo)
- [x] (UI) Conferido contra o FigCraft: nenhuma medida, cor ou token mudou (só nomes de
  classe locais; mesmas regras CSS)
- [x] Ficou dentro do escopo da fase
