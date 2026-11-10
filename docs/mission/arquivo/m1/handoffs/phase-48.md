# Fase 48 — Configurações › backup externo (UI)

## Implementado
- **O interruptor "Backup externo em pendrive" agora funciona** em
  `app/src/telas/Configuracoes.svelte`. A tela segue o frame `16178`, lido de uma cópia
  do `Ward.figcraft`; os ids estão em `docs/mission/design-nodes.md` › "Fase 48".
  - **Ligar:** abre "Escolher pendrive" (`EscolherPendrive.svelte`). O interruptor só
    liga depois que um pendrive é escolhido e o daemon confirma.
  - **Ligado:** aparece o cartão do pendrive (`CartaoDePendrive.svelte`, frame `16318`)
    com o rótulo, "UUID … · conectado e montado / conectado, mas não montado / não
    conectado / não deu para ver os pendrives agora", a linha de estado e o botão
    "Trocar", que reabre a escolha como "Trocar pendrive" com o atual já marcado.
  - **Desligar:** grava na hora, mantém o UUID guardado e não apaga nada do pendrive.
    Ao religar, a escolha já vem com o último pendrive marcado.
  - **Toasts:** "Backup externo ligado.", "Pendrive do backup externo trocado." e
    "Backup externo desligado. As cópias no pendrive continuam lá.". Com o cofre
    travado, a tela vai para o destravar.
- **Diálogo de escolha** (não tem frame no FigCraft): é montado com o `Modal` e as
  opções no desenho do cartão `16318`, com a escolhida em `borda/foco`.
  - É um `radiogroup` com os botões "Atualizar", "Cancelar" e "Ligar" (ou "Usar este
    pendrive"). "Ligar" fica desabilitado até haver escolha.
  - Mensagens de lista vazia: "Nenhum pendrive conectado…"; udisks2 sem resposta:
    "Não deu para ver os pendrives (o udisks não respondeu)."
  - Se o pendrive sair, o diálogo mostra "Esse pendrive não está conectado…" e
    atualiza a lista.
- **`LinhaDeOpcao` virou controlado:** o clique só pede a mudança, e quem usa a linha
  decide. O QA conferiu que os interruptores do gerador continuam funcionando.
- **Núcleo do app** (`app/nucleo/src/backup_externo.rs`, novo):
  - `PendriveVisto { uuid, rotulo, montado }`: o rótulo passa por
    `ward_core::exibicao::neutralizar` (D51) e é aparado.
  - `BackupExternoVisto { ligado, uuid, pendrive, pendrives_visiveis }`.
  - `EscolhaDoPendrive::nova(ligado, uuid)`: só aceita ligar com UUID no formato de D45
    (via `BackupExterno::novo`) ou desligar sem UUID.
  - `achar`: busca sem diferenciar maiúsculas. Um UUID repetido conta como nenhum,
    igual ao `wardd`.
- **`cliente.rs`:** `backup_externo`, `pendrives` e `definir_backup_externo`. O erro
  `pendrives_indisponiveis` vira o novo `ErroApp::PendrivesIndisponiveis`. Também foi
  criado `ErroApp::PendriveNaoConectado`.
- **`servico.rs`:**
  - `backup_externo`: lê a configuração e, havendo UUID, também `pendrives`. O udisks2
    sem resposta não dá erro: o resultado vem com `pendrives_visiveis: false`.
  - `listar_pendrives`.
  - `definir_backup_externo`: para ligar, consulta `pendrives` de novo e exige que o
    UUID esteja na lista; envia o UUID como o daemon o deu. Para desligar, lê o UUID
    atual e o reenvia. A resposta é o estado lido de novo.
- **Comandos novos `backup_externo`, `listar_pendrives` e
  `definir_backup_externo { ligado, uuid? }`:**
  - entraram em `comandos.rs` (`COMANDOS_PRINCIPAL` e `COMANDOS`), `comandos.js`, na
    capability `principal` (`allow-*`) e no `generate_handler!`;
  - na ponte, campo a mais ou tipo errado é recusado.
- **Daemon falso:**
  - atende `backup_externo`, `definir_backup_externo` (mesma regra do `wardd`:
    `BackupExterno::novo`) e `pendrives`, e `configuracao` passa a refletir o ligado;
  - os pendrives falsos de `pendrives_de_teste()` são só dados: `WARD-BACKUP` montado,
    `FOTOS 2024` não montado, um com rótulo `<img src=x onerror=alert(1)>` + U+202E +
    U+200B e um sem rótulo;
  - modos novos da ponte: `--daemon-falso backup-externo-ligado` e `sem-udisks`.

## Segurança
- **Invariantes tocadas**
  - **V7.8/D5:** os três comandos só valem na janela `principal` com a origem local.
    As operações do daemon já eram só do papel app (Fase 46).
  - **S11:**
    - nenhum caminho de montagem chega à interface (o protocolo nem o manda), e nada
      de segredo;
    - o rótulo do pendrive é dado não confiável: o invisível vira escape no Rust, e a
      tela mostra o resto como texto (interpolação do Svelte, `<bdi>` com
      `unicode-bidi: isolate`), nunca como HTML;
    - nada vai para `localStorage`, `sessionStorage` ou URL (conferido no QA);
    - não foi criada capability além dos três comandos que a tela usa.
  - **Entrada não confiável:** o UUID vindo da interface é conferido no Rust em duas
    etapas, antes de chegar ao `definir_backup_externo`:
    - formato de D45, sem consultar o daemon;
    - presença na lista atual do daemon.
  - **D17:** desligado por padrão; desligar não apaga cópias.
- **Testes de ataque** (todos falham do lado do atacante)
  - `app/nucleo/src/backup_externo.rs`:
    - `ataque_uuid_forjado_fora_do_formato_e_recusado`: `../etc`, caminho, U+202E,
      espaço, `\n`, dígito largo, HTML, 37 caracteres, ligar sem UUID e desligar com
      UUID.
    - `ataque_rotulo_com_direcao_e_largura_zero_vira_escape_visivel`.
    - `ataque_resposta_pendrives_forjada_e_recusada`: um "daemon" que manda rótulo com
      ESC ou `\n`, um campo `montagem` a mais, UUID `../etc`, rótulo acima de
      `MAX_ROTULO` ou 33 itens recebe `RespostaInvalida`.
    - `pelo_servico::ataque_uuid_forjado_pela_interface_nao_chega_ao_daemon`
      (espião): fora do formato, zero quadros; no formato mas fora da lista, recusado
      como `PendriveNaoConectado` sem nenhum quadro `definir_backup_externo`, e a
      configuração do falso fica intacta.
    - `pelo_servico::ataque_pendrive_clonado_com_uuid_repetido_nao_e_escolhido`.
    - `pelo_servico::ataque_rotulo_do_pendrive_chega_neutralizado`.
    - `pelo_servico::sem_udisks_nao_liga_e_o_cartao_diz_que_nao_viu`.
    - `pelo_servico::travado_nao_le_nem_muda`.
    - `pelo_servico::v7_9_escolher_ligar_e_desligar`.
  - `app/src-tauri/src/main.rs` ›
    `ataque_backup_externo_so_na_janela_certa_e_uuid_conferido_no_rust`:
    - janela `outra`, janela `aprovacao` e origem remota são negadas pela ACL;
    - UUID forjado dá `pedido-invalido` no núcleo, antes do daemon;
    - tipo errado ou campo faltando é recusado na desserialização.
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_backup_externo_pela_ponte`:
    - nenhuma resposta tem `/media`, `/run/`, `montagem`, U+202E ou U+200B;
    - tipo errado, campo a mais e UUID forjado são recusados;
    - um UUID fora da lista dá `pendrive-nao-conectado`.
  - **QA no Chrome:** uma rota do Playwright troca o UUID do pedido por `DEAD-BEEF`. A
    tela mostra "Esse pendrive não está conectado…" e não liga
    (`ataque-uuid-forjado-recusado.png`). O rótulo com HTML aparece como texto, sem
    `<img>` no DOM, sem `alert` e sem os caracteres invisíveis no HTML.
- **Dependências novas:** nenhuma, nem no Rust nem no npm. O Playwright do QA é o já
  existente no scratchpad (`py/site`) e fica fora do repositório.

## Ficou de fora
- **Caminho de montagem e "Última cópia hoje às 14:32 · 30 versões":** o frame `16178`
  mostra "conectado em /media/usuario/WARD-BACKUP" e a data da última cópia. O
  protocolo da Fase 46 não manda caminho ao cliente (de propósito) nem a data da última
  cópia externa. O cartão mostra só o UUID, a situação (conectado / montado) e uma linha
  de estado fixa. Ver Problemas.
- **Ponta a ponta com `wardd` real e pendrive real:** não montei dispositivo nenhum. O
  QA usou o daemon falso. V7.4/V7.5 com vfat real continuam com o validador do M7
  (receita no handoff 46).
- **Frame para o diálogo "Escolher pendrive":** não existe; montei com componentes da
  Design System.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-app-nucleo --all-features backup_externo` | 0 |
| `cargo test -p ward-ponte-teste --all-features --test ataques backup` | 0 |
| `cargo test` do `app/src-tauri` (R32): a 1ª rodada falhou porque não havia `app/dist`; rodei depois do `make app-front` | 101, depois 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso cheio` + `qa48.py`: a 1ª rodada achou um bug (o `carregar()` apagava a mensagem do pendrive não conectado); corrigido. A 2ª acusou só o 404 do `/favicon.ico` no console, e o filtro passou a olhar a URL | 1, depois 0 |
| ponte `--daemon-falso backup-externo-ligado` e `sem-udisks` + `qa48.py` | 0 |
| `make fmt` | 0 |
| `make ferramentas` | 0 |
| `make check` | 0 |
| `make app-check` (R32: `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do scratchpad) | 0 |

## Problemas descobertos
- **Design × protocolo no cartão do pendrive:** o frame `16178`/`16334` mostra o
  caminho de montagem (`/media/usuario/WARD-BACKUP`) e "Última cópia hoje às 14:32 · 30
  versões em ward-backup/". A Fase 46 decidiu que nenhum caminho de montagem vai ao
  cliente, e não há operação que diga a última cópia externa. Segui o protocolo. O
  FigCraft precisa ser ajustado numa fase de design, ou o planner cria uma operação de
  "última cópia" (só data e contagem, sem caminho).
- **Diálogo "Escolher pendrive" sem frame:** vale desenhá-lo (os estados lista, vazia,
  udisks sem resposta e pendrive saiu) numa fase de design.
- **Pendrive conectado mas não montado:** o Ward não monta (pendência da 46). O cartão
  diz "Abra o pendrive no gerenciador de arquivos para montá-lo e receber a cópia." Se
  o planner decidir montar pelo udisks2, esse texto muda.
- **Escolher exige o pendrive conectado:** a tela só deixa ligar com um pendrive que o
  daemon vê na hora. O `wardd` aceita qualquer UUID no formato. Isso é mais restrito,
  e de propósito (evita UUID forjado pela interface); um app comprometido ainda pode
  falar com o socket direto (papel app), como em todas as operações do app.
- **Loops do sistema (snap) na lista:** continuam aparecendo (pendência da 46). Eles
  aparecem na escolha com o rótulo deles, sem perigo de cópia sem escolha.
- **Escape de D51 no rótulo:** `neutralizar` não escapa `\`. Um rótulo que já traga o
  texto literal `\u{202E}` fica igual a um neutralizado. É só cosmético, porque nada é
  invisível.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/backup_externo.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `app/src/lib/componentes/CartaoDePendrive.svelte` | criado |
| `app/src/lib/componentes/LinhaDeOpcao.svelte` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/telas/Configuracoes.svelte` | alterado |
| `app/src/telas/EscolherPendrive.svelte` | criado |
| `app/src/telas/configuracoes.js` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-48/*` (8 PNG do app, 1 PNG do frame `16178`, `qa.json`) | criado |
| `docs/mission/handoffs/phase-48.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (daemon falso; nenhum dispositivo montado)
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia do `Ward.figcraft`; nada salvo)
- [x] Ficou dentro do escopo da fase
