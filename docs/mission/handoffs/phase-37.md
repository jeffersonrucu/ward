# Fase 37 — (UI) `ApprovalWindow`, `PasswordGenerator`, `FirstUse`, `RestoreBackup` e `EmergencyKit`

## Implementado

Troca 1:1 pelo `docs/glossario.md` (seção 4, mais a tabela nova "Telas `ApprovalWindow`,
`PasswordGenerator`, `FirstUse`, `RestoreBackup` e `EmergencyKit` (Fase 37)"), sem mudar
comportamento nem texto visível em pt-BR. Parte de `master` 3aac84d.

- **Arquivos** (`git mv`): `screens/JanelaAprovacao.svelte` → `ApprovalWindow.svelte`,
  `aprovacao.js` → `approval.js`, `GeradorSenha.svelte` → `PasswordGenerator.svelte`,
  `PrimeiroUso.svelte` → `FirstUse.svelte`, `primeiro-uso.js` → `first-use.js`,
  `RestaurarBackup.svelte` → `RestoreBackup.svelte`, `restaurar.js` → `restore.js`,
  `KitEmergencia.svelte` → `EmergencyKit.svelte`, `kit-emergencia.js` →
  `emergency-kit.js`. `App.svelte` importa os três nomes novos.
- **`approval.js`**: `URGENT_SECONDS`, `pinSufficient`, `DENY_ID`, `needsPin(decision)`,
  `SEPARATOR` (o escape `' → '` ficou igual), `TEXTS` (chaves em inglês, textos
  iguais), `originNotice`, `resultMessage`.
- **`first-use.js`**: `MIN_PASSWORD_CHARS`, `MIN_PIN_DIGITS`, `MAX_PIN_DIGITS`,
  `MESSAGES`, `charCount`, `passwordError`, `isValidPin`, `passwordStrength` (força
  `{ upTo, code, label, segments, tone }`, `tone` `danger/warning/success`),
  `rustMessage(reason)`.
- **`restore.js`**: `versionDay`, `versionDate`, `howLongAgo`, `versionDetail`,
  `versionsCaption`, `swapWarning`, `restoredMessage(instant, entries)`.
- **`emergency-kit.js`**: `VAULT_DIR`, `BACKUPS_DIR`, `USB_DRIVE_DIR`, `RESTORE_STEPS`,
  `printedDate`, `usbDriveLine(external)` → `{ label, value, mono }`. O objeto interno
  `external` virou `{ enabled, uuid, usbDrive: { label } }`; o `EmergencyKit` o monta da
  resposta do fio (`ligado`, `uuid`, `pendrive.rotulo`) como antes.
- **Telas**: identificadores, props (`RestoreBackup` `open`/`onclose`/`onrestored`
  (`{ instant, entries }`)/`onlocked`; `EmergencyKit` `open`/`onclose`/`onlocked`;
  `FirstUse` sem o alias `oncreated: oncriado`), estados, classes locais, ids e
  comentários em inglês. `PasswordGenerator` usa os mesmos nomes do `GeneratorPopover`
  (`MIN_LENGTH`, `STRENGTHS`, `charsets`…). Comentários citam os nodes do FigCraft pelo
  id (regra da Fase 35) e descrevem comandos e módulos Rust em inglês.
- **Usos fora das telas da fase** (só os nomes desta fase; o resto é da Fase 38):
  `Configuracoes.svelte` (imports, tags `<EmergencyKit>`/`<RestoreBackup>` com as props
  novas, `restoredMessage(r.instant, r.entries)`, `rustMessage`, dois comentários),
  `configuracoes.js` (comentário com `first-use.js`), `TrocarSenha.svelte` (imports,
  `forca.segments/tone/label`, atributo `data-tone` e seletores `danger/warning/success`),
  `TrocarPin.svelte` e `EscolherPendrive.svelte` (imports e chamadas).
- **Testes**: `app/core/tests/configuracao_tauri.rs` lê `PasswordGenerator.svelte`
  (`const MIN_LENGTH/MAX_LENGTH/DEFAULT_LENGTH = …;` contra as constantes Rust de mesmo
  valor), `approval.js` (`URGENT_SECONDS`, `return decision !== 'negar' && decision !==
  'confirmar';`) e `ApprovalWindow.svelte` (`{request.comando}`, `{process}`,
  `{entry.nome}`, `{request.origem}`); `dev/tests/app_e2e.py` usa `#approval-deny` e
  `#approval-pin`.
- **Glossário**: tabela da Fase 37.

### Literais alterados (PRD regra 5; todos são nomes)

- Caminhos de import: `./aprovacao.js` → `./approval.js`, `./primeiro-uso.js` →
  `./first-use.js`, `./restaurar.js` → `./restore.js`, `./kit-emergencia.js` →
  `./emergency-kit.js`, `./KitEmergencia.svelte` → `./EmergencyKit.svelte`,
  `./RestaurarBackup.svelte` → `./RestoreBackup.svelte`, `./screens/GeradorSenha.svelte`,
  `JanelaAprovacao.svelte`, `PrimeiroUso.svelte` → `PasswordGenerator`, `ApprovalWindow`,
  `FirstUse`.
- Ids do DOM: `aprovacao-titulo/negar/entrada/pin` → `approval-title/deny/stdin/pin`
  (`#aprovacao-negar`/`#aprovacao-pin` em `app_e2e.py` ajustados); `gerador-titulo`,
  `opcoes-titulo`, `resultado-titulo`, `gerador-tamanho/minusculas/maiusculas/digitos/
  simbolos/senha` → `generator-title`, `options-title`, `result-title`,
  `generator-length/lowercase/uppercase/digits/symbols/password`;
  `primeiro-uso-criar/senha/confirmar/pin/confirmar-pin` →
  `first-use-create/password/confirm/pin/confirm-pin`; `restaurar-titulo/subtitulo/
  senha/pin/versao-{i}/escolhida` → `restore-title/subtitle/password/pin/version-{i}/
  chosen`; `kit-titulo` → `kit-title`. Nenhum outro é lido fora da tela (conferido em
  `app/`, `dev/`, `extension/`, `crates/`; só o fixture legado
  `dev/fixtures/legacy/strings-0.1.4.txt` os cita, e ele não muda).
- `data-*`: `data-teste` → `data-test` (`simbolos` → `symbols`, `restaurar-backup` →
  `restore-backup`, `folha-kit` → `kit-sheet`, `kit-pendrive` → `kit-usb-drive`);
  `data-etapa` → `data-step` com `versoes`/`confirmar` → `versions`/`confirm`;
  `data-tom` → `data-tone` com `perigo`/`aviso`/`sucesso` → `danger`/`warning`/`success`
  (gerador, primeiro uso e trocar senha; só seletores locais). Ninguém lê esses
  atributos fora da tela.
- Valores internos: `tom` da força (acima); etapa do restaurar (acima).
- Classes locais (escopadas pelo Svelte; nenhuma colide com as globais de `base.css`/
  `tokens.css`): `fundo` → `background`, `janela` → `approval`/`window`, `cabecalho` →
  `header`, `corpo` → `body`, `rodape` → `footer`, `folha` → `sheet` etc. Mesmas regras
  CSS, só o seletor mudou.
- Chaves do fio passaram de shorthand a chave entre aspas (`{ decisao, ciente }` →
  `{ 'decisao': decision, 'ciente': withAcknowledgment }`, `{ tamanho, ...conjuntos }` →
  `{ 'tamanho': length, ...charsets }`, `{ dados: { senha, confirmacao, … } }` →
  `{ 'dados': { 'senha': password, 'confirmacao': confirmation, … } }`, `{ dados: {
  versao, senha, pin } }` idem, chaves de `erros`/`charsets`/`STRENGTHS`): mesmos bytes no
  fio.
- Template strings só mudaram nas variáveis interpoladas; o texto pt-BR é o mesmo.

### Entradas novas de `dev/english-check/frozen.txt` (todas Fase 54)

- `ApprovalWindow.svelte`: `estado`, `ambiente`, `confirmacao`, `operacao`,
  `entrada_oculta`, `restante`, `origem`, `entradas`, `tipo`, `nome`, `comando`,
  `diretorio`, `cadeia`, `resultado`, `mensagem`.
- `PasswordGenerator.svelte`: `tamanho`, `minusculas`, `maiusculas`, `digitos`,
  `simbolos`, `forca`, `valor`, `mensagem`.
- `FirstUse.svelte`: `senha`, `confirmacao`, `criado`, `erros`, `campo`, `motivo`,
  `mensagem`.
- `restore.js`: `conferida`, `fora_de_ordem`.
- `RestoreBackup.svelte`: `nome`, `instante`, `fora_de_ordem`, `dados`, `entradas`,
  `estado`, `codigo`, `mensagem`.
- `EmergencyKit.svelte`: `ligado`, `pendrive`, `rotulo`, `codigo`, `mensagem`.

## Segurança

- Invariantes: S9 e S10 (janela de aprovação: foco inicial em Negar, Esc nega, fechar
  nega, PIN só no estado do `PinField` e limpo a cada envio, `needsPin` só dispensa
  `negar`/`confirmar`, ciência da entrada padrão desmarcada a cada pedido, contagem só
  de exibição; restaurar com senha e PIN só no estado local, apagados antes da
  resposta), S11 (gerador: valor só no "revelar", some em 30 s, ao esconder, ao gerar e
  ao sair, `descartar_senha_gerada` no `onDestroy`; primeiro uso apaga senha e PIN
  depois do envio e no `onDestroy`; nada em estado global, armazenamento ou URL) e D51
  (texto do agente só como texto, `AgentText`, seta só desenho) mantidas: só nomes e
  comentários mudaram, linha a linha. O teste `janela_de_aprovacao_segue_o_rust_e_nao_usa_html`
  acompanha os nomes novos e continua conferindo que comando, cadeia, entrada e origem
  entram como texto e que só negar/confirmar dispensam o PIN.
- Nenhum comando Tauri, capability, campo do fio ou texto visível mudou.
- Testes de ataque: nenhum novo (fase de nomes); os existentes (`configuracao_tauri.rs`,
  `app_e2e.py`) acompanham os nomes.
- Dependências novas: nenhuma.

## Ficou de fora

- Nomes de fio (comandos Tauri, campos, valores, id da tela na URL de teste `&tela=…`,
  aviso `pedido-de-aprovacao`, decisões `negar`/`uma_vez`/…): Fase 54 (`frozen.txt`).
- Identificadores próprios de `Configuracoes.svelte`, `TrocarSenha.svelte`,
  `TrocarPin.svelte`, `EscolherPendrive.svelte` e `configuracoes.js`: Fase 38 (aqui só
  os usos dos nomes desta fase).
- `docs/mission/design-nodes.md` cita os caminhos antigos: Fase 59 (PRD regra 2).
- Suítes (`app_e2e.py`, `configuracao_tauri.rs`, QA visual V3.2/V3.3, V3.4): Milestone 6
  (PRD regra 6).

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `python3 -I dev/english_check.py --paths app/src/screens/{ApprovalWindow.svelte,approval.js,PasswordGenerator.svelte,FirstUse.svelte,first-use.js,RestoreBackup.svelte,restore.js,EmergencyKit.svelte,emergency-kit.js}` (= `make english-check PATHS=…`) | 0 (0 achados, 0 problema nas exceções) |
| `python3 -I dev/english_check.py --paths app/src` (conferir que o `frozen.txt` não tem entrada sem uso) | 0 problema nas exceções |
| `make compile` (cargo check do workspace e do `app/src-tauri`, `vite build`, `syntax_check.py`: 0 erro) | 0 |
| `cargo check -p ward-app-core --all-targets --all-features --locked` (de novo, com o `configuracao_tauri.rs` tocado) | 0 |
| `npm run build:teste` (conferência rápida do pacote de teste) | 0 |
| comparação de literais antigo × novo dos 9 arquivos e dos 4 usuários (script na scratchpad) | — (só os nomes listados acima) |

O `vite build` mostra os mesmos 10 avisos `state_referenced_locally` de antes, em
`screens/Form.svelte`; nenhum aviso nas telas desta fase.

## Problemas descobertos

- O sandbox do worktree recusa comandos de shell que contêm "git" dentro de outra
  palavra (`digitos`); usei o editor para o `frozen.txt`. Sem efeito no repositório.
- O primeiro `make compile` gravou o log num arquivo da scratchpad com nome genérico, que
  outro processo (um `make compile` no checkout principal, `/home/…/cofre`) também
  escrevia ao mesmo tempo: as saídas se misturaram. Rodei de novo com log de nome
  próprio; o resultado acima é só deste worktree. Sugestão: os workers usarem log com o
  id do worktree.
- Risco de conflito no merge com a Fase 38 (rodando em paralelo): `Configuracoes.svelte`,
  `TrocarSenha.svelte`, `TrocarPin.svelte`, `EscolherPendrive.svelte` e
  `configuracoes.js` tiveram linhas de uso alteradas aqui; e com a trilha R em
  `app/core/tests/configuracao_tauri.rs` (dois testes) e no fim do
  `dev/english-check/frozen.txt`.
- O teste `limites_do_gerador_na_interface_sao_os_do_rust` agora tem o nome JS e o
  Rust separados (`("MIN_LENGTH", TAMANHO_MIN)`); a Fase 34 renomeia só o lado Rust.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/src/screens/JanelaAprovacao.svelte` → `app/src/screens/ApprovalWindow.svelte` | renomeado e reescrito |
| `app/src/screens/aprovacao.js` → `app/src/screens/approval.js` | renomeado e reescrito |
| `app/src/screens/GeradorSenha.svelte` → `app/src/screens/PasswordGenerator.svelte` | renomeado e reescrito |
| `app/src/screens/PrimeiroUso.svelte` → `app/src/screens/FirstUse.svelte` | renomeado e reescrito |
| `app/src/screens/primeiro-uso.js` → `app/src/screens/first-use.js` | renomeado e reescrito |
| `app/src/screens/RestaurarBackup.svelte` → `app/src/screens/RestoreBackup.svelte` | renomeado e reescrito |
| `app/src/screens/restaurar.js` → `app/src/screens/restore.js` | renomeado e reescrito |
| `app/src/screens/KitEmergencia.svelte` → `app/src/screens/EmergencyKit.svelte` | renomeado e reescrito |
| `app/src/screens/kit-emergencia.js` → `app/src/screens/emergency-kit.js` | renomeado e reescrito |
| `app/src/App.svelte` | alterado (imports) |
| `app/src/screens/Configuracoes.svelte`, `configuracoes.js`, `TrocarSenha.svelte`, `TrocarPin.svelte`, `EscolherPendrive.svelte` | alterado (usos) |
| `app/core/tests/configuracao_tauri.rs` | alterado (caminhos e trechos procurados) |
| `dev/tests/app_e2e.py` | alterado (`#approval-deny`, `#approval-pin`) |
| `dev/english-check/frozen.txt` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-37.md` | criado |

## Procedimentos

- [x] `make compile` rodado (a missão 2 não roda `make check` por fase; PRD regra 6)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto nesta fase)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma lógica de segurança mudou; os
  testes existentes de S9/S10/S11 acompanham os nomes)
- [x] (UI) Conferido contra o FigCraft: nenhuma medida, cor ou token mudou (só nomes de
  classe locais; mesmas regras CSS)
- [x] Ficou dentro do escopo da fase
