# Fase 44c — Correções da auditoria de segurança do M6 (origem `http`, RA10, política do Chrome)

## Implementado
- **Origem `http` com a janela (D9).** `ward login`, `ward playwright` e o "preencher"
  do popup, numa origem `http` (`localhost`/`127.0.0.1`), passam sempre pela janela de
  aprovação com a origem à vista, mesmo no nível livre.
  - **Protocolo** (`ward-core`):
    - `PedidoDeAprovacao` ganha `origem` (até `MAX_TEXTO`) e `confirmacao`.
    - Nova decisão `confirmar`, sem PIN.
    - Validação estrita: origem só em login/playwright/preencher; confirmação exige
      origem; `preencher` só como confirmação.
  - **Daemon** (`uso.rs`, `nucleo.rs`, `aprovacao.rs`):
    - Livre + `http`: pede confirmação (só `negar`/`confirmar`).
    - "Com aprovação"/"sempre PIN": o pedido de sempre, agora com a origem.
    - Sem o app, "aprovação indisponível".
    - `responder_aprovacao` recusa `confirmar` num pedido com PIN e as três "permitir"
      numa confirmação.
    - O `preencher` do papel extensão em `http` confere a origem (`origem_diferente`
      antes de perguntar), depois pede a confirmação e só então devolve.
    - `https` não muda.
  - **App:**
    - `JanelaAprovacao.svelte` mostra a origem e o aviso de `http`; na confirmação, sem
      campo de PIN, só "Negar" e "Confirmar a origem".
    - O `servico.rs` do app confere que a decisão casa com o tipo do pedido.
    - O daemon falso tem os modos `aprovacao-origem`, `aprovacao-origem-popup` e
      `aprovacao-origem-pin`.
  - **Extensão:**
    - O popup fecha quando a janela pega o foco. Por isso o "preencher" `http` vai ao
      service worker (`servico.js`): ele espera a resposta por uma porta
      `connectNative` (até 130 s), injeta no `documentId` que a sonda do popup achou e
      apaga os valores.
    - O popup mostra "aguardando a janela" e os erros `negado`, `expirado` e
      `aprovacao_indisponivel`.
  - **`ward playwright`:** o Chrome roda com `--host-resolver-rules=MAP localhost
    127.0.0.1`.
- **Janela de teste para as suítes de ponta a ponta:**
  - `crates/wardd/aprovador-teste/main.rs` é o binário `wardd-aprovador-teste`, com
    `required-features = ["modo-teste"]`.
  - O daemon de teste o abre por `WARD_DEV_APP` (D63).
  - Ele responde o que estiver em `$WARD_DEV_DIR/aprovador.modo` e registra uma linha
    `pedido …` por pedido, sem nenhum valor do cofre.
  - Ligada em `teste-login`, `teste-extensao`, `teste-playwright` e
    `teste-politica-chrome`.
- **Ataque `[::1]`:** `dev/testes/falso_v6.py` veio do scratchpad da auditoria. Ele
  escuta só em `[::1]:<porta>` e registra os POST em 0600.
  - `teste-login`: a aba de `localhost:8081` cai no falso (título `falso-v6`, controle
    de que o ataque vale); negado, nada é preenchido, o falso fica vazio e o pedido
    mostra `http://localhost:8081`.
  - `teste-playwright`: negado, sai 1 com "pedido negado" e o Chrome não abre;
    confirmado, o login vai ao WordPress de 127.0.0.1 e o falso recebe 0 bytes.
- **Política do Chrome:**
  - `PasswordManagerEnabled: false` e `PasswordLeakDetectionEnabled: false` em
    `instalar.sh`; o `.deb` usa o mesmo staging.
  - Os dois nomes entraram em `POLITICAS_DO_WARD`, então outro arquivo de
    `policies/managed` que os cite recusa a instalação.
  - Conferido em `install.sh`, `deb.sh`, `conteiner-install/dentro.sh`,
    `conteiner-deb/dentro.sh` e `politica-chrome.sh`.
  - O efeito para o usuário está explicado em `docs/instalacao.md`, novo passo 4b; o
    antigo 4b virou 4c.
- **Baixo:** `apagar_sessoes` só lista se `symlink_metadata` disser que é pasta.
- **RA10 ampliado:**
  - `docs/escopo.md` › "Fora do modelo de ameaça" ganha o item dos pipes do Chrome por
    `/proc/<pid>/fd`: o pipe CDP do `ward playwright` e o stdout do `ward-nm`.
  - O texto do Yama foi corrigido: ele protege contra `ptrace`, não contra os fds.
  - A ameaça 4 foi reescrita.
  - PRD: D9 e RA10.
  - Contrato: V6.17 e V6.18 novos, e V9.5 com as duas políticas.
- **44b:**
  - Roteiro M6 com a mensagem real do negado (`ward: pedido negado`, sai 1) e a seção
    2a (confirmação no livre e no popup).
  - O comentário de `extensao/pagina.js:9-13` foi corrigido.
- **Evidência:** em `docs/mission/evidence/fase-44c/` (os `qa.json` das suítes de
  login e extensão e o log do `teste-playwright`, sem padrão de
  `.dev/variacoes.txt`). As evidências das fases 41, 42 e 43, que as suítes regravaram,
  foram restauradas.

## Segurança
- **Invariantes tocadas:**
  - **S1:** a senha só vai à origem certa. Numa origem `http`, só depois de o usuário
    ver a origem e confirmar.
  - **S9:** aprovação na janela. O socket continua sem porta dos fundos, e a janela de
    teste só existe com `modo-teste`.
  - **S10:** o pedido de aprovação não leva segredo; conferido pela forma codificada
    nos testes.
  - **V9.5:** as duas políticas.
- **Testes de ataque escritos:**
  - `crates/ward-core/src/protocolo/testes.rs` › `ataque_pedido_de_aprovacao_com_origem_fora_da_forma`
  - `crates/wardd/tests/login.rs` › `ataque_login_http_livre_so_com_confirmacao`,
    `ataque_login_http_com_aprovacao_pede_pin_e_mostra_a_origem`,
    `login_https_livre_sem_janela`
  - `crates/wardd/tests/playwright.rs` › `ataque_playwright_http_livre_so_com_confirmacao`,
    `playwright_https_livre_sem_janela`; `chrome_real_login_e_ataques` confere a
    confirmação de cada caso
  - `crates/wardd/tests/extensao.rs` › `ataque_preencher_http_sem_confirmar_nada_sai`
  - `crates/wardd/src/playwright.rs` › `ataque_apagar_sessoes_nao_segue_link`, mais o
    argv com `--host-resolver-rules`
  - `app/nucleo/src/aprovacao.rs` › `ataque_confirmacao_so_por_confirmar_e_pedido_com_pin_nunca`
  - `dev/testes/login.py`: seção `[::1]` (`c44_v6_*`), V6.11 negado com a linha do
    pedido
  - `dev/testes/playwright.py`: "ataque [::1]" (negado e confirmado)
  - `dev/testes/extensao.py`: `c44_popup_negado`, `c44_popup_pedido`
  - `dev/testes/install.sh`: recusa de outro arquivo com `PasswordManagerEnabled` e com
    `PasswordLeakDetectionEnabled`
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Frame no FigCraft para o estado "confirmação da origem" da janela:** não existe. A
  tela foi montada com os tokens e componentes que já existem (CaixaDeCodigo,
  Destaque, botões). Não há conferência contra um node.
- **Conferência visual da janela nova no Chrome (Playwright headless):** não feita.
  Faltou disco e não há frame contra o qual comparar.
- **`make app-check`** (Tauri, ACL, release): não rodado por falta de disco. O
  `app/nucleo` está no workspace e passou no `make check`; o `make app-front` passou.
- **`make teste-sem-modo-teste`:** não rodado. Só ganhou `wardd-aprovador-teste` na
  lista de textos proibidos. O binário tem `required-features = ["modo-teste"]`, então
  não entra no release.
- **Expiração da janela de teste (modo `nada`) nas suítes:** não testada, para não
  somar 2 min. A expiração já está coberta nos testes Rust do motor de aprovação.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` (0/6–5/6; o 6/6 falhou na 1ª vez por falta do gitleaks no worktree) | 2 → 0 |
| `make gitleaks` (gitleaks copiado de `.dev/ferramentas/` do checkout principal; sha256 conferido pelo Makefile) | 0 |
| `make fmt-check` (no fim) | 0 |
| `cargo clippy -p wardd --all-features --all-targets -- -D warnings` | 0 |
| `cargo test -p wardd --all-features --test playwright --test login --test extensao` | 0 |
| `make app-front` | 0 |
| `make extensao` | 0 (1ª vez 2: `'http://'` literal; trocado por `protocol === 'http:'`) |
| `make teste-playwright SEM_PLAYWRIGHT_PYTHON=1` | 0 |
| `make teste-login` | 0 (1ª vez 2: ver Problemas, Chrome e `::1`) |
| `make teste-extensao` | 0 (1ª vez 2: variável sombreada no `extensao.py`) |
| `make teste-politica-chrome` | 0 (1ª vez 2: o controle da rodada 3 não reproduzia; ver Problemas) |
| `make teste-install` | 0 (1ª vez 2: `dentro.sh` esperava só 2 políticas) |
| `make teste-deb` | 0 |

## Problemas descobertos
- **Risco residual da confirmação.** Um usuário que confirma a origem sem olhar ainda
  manda a senha ao processo de `[::1]` no `ward login` e no popup. O Chrome do usuário
  continua tentando `::1` antes de `127.0.0.1`. Só o `ward playwright` fecha isso de
  vez, com o `--host-resolver-rules`. A janela mostra a origem e o aviso de `http`, mas
  não tem como saber qual socket o Chrome usou.
- **Controle do `teste-politica-chrome` mudou de sentido.** Todas as entradas `login`
  do cofre de teste são `http://localhost`. Com a Fase 44c, o ataque da auditoria do M9
  (troca de pai) sem janela não tira mais nada. Ele recebe "aprovação indisponível", ou
  fica parado até o usuário confirmar. O controle da rodada 3 agora usa a janela de
  teste em `confirmar` para continuar provando que o ataque é visto. Numa entrada
  `https` o RA10 segue igual, sem janela.
- **Chrome e `::1`.** Depois de uma conexão a `127.0.0.1` para `localhost:<porta>`, o
  Chrome reaproveita o endereço e nem tenta `[::1]`. A seção `[::1]` do `login.py` tem
  de rodar antes de toda outra visita a `localhost:8081`, e um comentário diz isso. O
  ataque real depende do momento: vale num Chrome que ainda não falou com aquela porta.
- **A ponte de teste não serve às suítes.** Ela exige `WARD_DEV_DIR` igual ao `.dev` do
  repositório. Por isso existe um binário de teste próprio (`wardd-aprovador-teste`).
- **Escopo × PRD no RA10.** O escopo dizia que o Yama protege o Chrome contra quem não
  é ancestral, e isso não vale para `/proc/<pid>/fd`. Os dois textos foram corrigidos
  juntos, sem escolher lado: o novo é o que o usuário decidiu (plan.md, "Decisões do
  usuário (2026-11-09)").
- **Popup e níveis.** O "preencher" do popup em `http` pede só a confirmação, sem PIN,
  em qualquer nível. Antes o popup não passava pelos níveis, e o nível vale para o
  agente. Se o usuário quiser PIN no popup em "sempre PIN", isso volta ao planner.
- **Serviços de teste.** Os contêineres `ward-teste-*` aparecem como criados há ~22
  min, depois que eu os subi. Pode ter sido outro checkout, então **não** os parei.
  Confira `docker ps` antes de `make dev-servicos-parar`.
- **Disco.** Ficaram em torno de 2 GB livres. Rodou tudo o que estava pedido, menos
  `app-check` e `teste-sem-modo-teste` (acima).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `Makefile` | alterado |
| `app/nucleo/src/aprovacao.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/tests/configuracao_tauri.rs` | alterado |
| `app/src/telas/JanelaAprovacao.svelte` | alterado |
| `app/src/telas/aprovacao.js` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/aprovador-teste/main.rs` | criado |
| `crates/wardd/src/aprovacao.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/playwright.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/tests/comum/mod.rs` | alterado |
| `crates/wardd/tests/extensao.rs` | alterado |
| `crates/wardd/tests/login.rs` | alterado |
| `crates/wardd/tests/playwright.rs` | alterado |
| `dev/conteiner-deb/dentro.sh` | alterado |
| `dev/conteiner-install/dentro.sh` | alterado |
| `dev/testes/deb.sh` | alterado |
| `dev/testes/extensao.py` | alterado |
| `dev/testes/extensao.sh` | alterado |
| `dev/testes/falso_v6.py` | criado |
| `dev/testes/install.sh` | alterado |
| `dev/testes/login.py` | alterado |
| `dev/testes/login.sh` | alterado |
| `dev/testes/playwright.py` | alterado |
| `dev/testes/playwright.sh` | alterado |
| `dev/testes/politica-chrome.sh` | alterado |
| `dev/testes/politica_chrome.py` | alterado |
| `dev/testes/sem-modo-teste.sh` | alterado |
| `docs/escopo.md` | alterado |
| `docs/instalacao.md` | alterado |
| `docs/mission/evidence/M6/roteiro-usuario.md` | alterado |
| `docs/mission/evidence/fase-44c/` (`qa-login.json`, `qa-extensao.json`, `teste-playwright.log`) | criado |
| `docs/mission/handoffs/phase-44c.md` | criado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/validation-contract.md` | alterado |
| `extensao/pagina.js` | alterado |
| `extensao/popup.js` | alterado |
| `extensao/servico.js` | alterado |
| `instalacao/chrome.sh` | alterado |
| `instalacao/instalar.sh` | alterado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não há frame para o estado de
  confirmação (ver "Ficou de fora")
- [x] Ficou dentro do escopo da fase
