# Fase 35 — Nível de acesso com PIN (UI)

## Implementado
- **Nenhuma operação nova no protocolo.** A fase usa `mudar_nivel`, que já existe em
  `ward_core::protocolo` (só papel app, com PIN).
- **`app/nucleo/src/nivel.rs` (novo):**
  - `Pin`: o PIN que vem da interface, guardado em `Zeroizing<String>`. Não tem `Clone`,
    `Display` nem `Serialize`, e o `Debug` mostra `Pin(••••)`;
  - o `Deserialize` já recusa o que não é PIN (de 4 a 6 dígitos ASCII, os mesmos
    `MIN/MAX_DIGITOS_PIN` do `ward_core::limites`). O erro tem texto fixo, "PIN fora do
    formato". Um número no lugar do texto também cai nesse erro fixo (`deserialize_any`
    + `visit_u64`/`i64`/`f64`), e não no erro padrão do serde, que ecoaria o valor;
  - `nivel_de_nome` aceita só `livre`, `com-aprovacao` e `bloqueado`;
  - `NivelMudado { entrada, nivel }` é a resposta, sem PIN.
- **`cliente::mudar_nivel`:** monta o pedido com o PIN num `Segredo` do núcleo (que zera)
  e devolve `()` só na resposta `ok`. `ErroResposta::PinIncorreto` passa a ser
  `ErroApp::PinIncorreto` (código `pin-incorreto`, mensagem fixa "PIN incorreto. Nada
  foi alterado.").
- **`daemon_falso` (só `modo-teste`/testes):** responde `mudar_nivel` com a mesma regra do
  `wardd`:
  - o PIN é conferido no hash Argon2id da configuração do cofre de teste
    (`cofre_teste::configuracao_de_teste`), em tempo constante pela biblioteca;
  - o PIN é conferido antes da entrada;
  - o acerto zera a contagem;
  - o 5º erro seguido trava. Depois disso `estado` responde `travado: true` e o resto
    responde "travado" (`MAX_ERROS_PIN = 5`, o mesmo valor do `wardd`).
- **`servico::mudar_nivel(entrada, nivel, Pin)`:** confere o nome (`validar_nome_entrada`)
  e o nível antes do daemon. Por isso um pedido forjado não conta como erro de PIN.
- **Comando `mudar_nivel`:** entrou em `comandos.rs`, em `capabilities/principal.json`
  (`allow-mudar-nivel`), em `ponte/comandos.js`, no `generate_handler!` do Tauri
  (argumento `pin: Pin`) e no despacho da ponte de teste. Na ponte, o PIN sai do mapa
  JSON por `remove` (fica com a posse, sem copiar) e vira `Pin`.
- **Interface:**
  - `CartaoDeNivel.svelte` (set `18998`) substitui o cartão solto do detalhe de
    Credenciais;
  - `OpcaoDeNivel.svelte` (set `909`) usa `<input type="radio">` de verdade;
  - `DialogoDeNivel.svelte` (set `967`) é um `<dialog>` nativo com as etapas Escolher,
    PIN e PIN errado:
    - os três níveis aparecem sempre, inclusive em prod;
    - o nível atual vem marcado, e "Continuar" fica desabilitado até a escolha mudar;
    - "De … para …", `Campo PIN` e a nota "5 erros de PIN seguidos travam o cofre.";
    - com PIN errado, aparece o erro no campo e a nota em aviso "O nível continua …";
  - `niveis.js` guarda os textos dos níveis tirados do FigCraft;
  - `CampoPin.svelte`: dígito cheio com `borda/padrao` e erro com `perigo/texto`, como
    as instâncias `1055`/`1106`;
  - `Credenciais.svelte`: "Mudar nível" abre o diálogo. Ao mudar, recarrega a lista e o
    detalhe e mostra o Toast "Nível de acesso alterado.". Ctrl+K não age com o diálogo
    aberto. A prop nova `ontravado` avisa o `App.svelte`, que sai da casca e consulta o
    estado de novo.
- **5º erro:** depois de cada `pin-incorreto`, o diálogo consulta `estado_do_cofre`. Se o
  cofre travou (ou não houver resposta), chama `ontravado`. O mesmo vale se o daemon
  responder `travado`.
- **QA no Chrome:** Playwright Python headless, 1280×800, ponte com `--daemon-falso
  cheio`. As 37 checagens deram `true` (`docs/mission/evidence/fase-35/qa.json`):
  - os três cartões;
  - os três níveis em prod;
  - PIN errado mostra o erro, o campo fica vazio e o nível não muda;
  - fechar não muda nada;
  - o PIN certo muda o nível (livre, bloqueado, com aprovação);
  - 4 erros e 1 acerto não travam;
  - o 5º erro seguido trava;
  - nada no DOM, nos `value`, no storage, no cookie, na URL nem no console.

  As capturas ficam ao lado do PNG dos frames `5615`, `5994`, `6373` e do componente
  `18998`.

## Segurança
- **Invariantes tocadas:**
  - **S9 / V5.15:**
    - mudar o nível pede o PIN;
    - com PIN errado, o nível não muda e a resposta não diz quantas tentativas faltam;
    - o 5º erro seguido trava (no daemon falso, com a regra do `wardd`; no real, a
      contagem é do `wardd`).
  - **Formato antes do daemon:** um PIN fora do formato, um nível inventado ou um nome
    inválido são recusados antes de chegar ao daemon e não contam como erro de PIN.
  - **PIN em memória:**
    - no Rust, fica em `Pin` (`Zeroizing`) e em `ward_core::entrada::Segredo` no pedido;
      o quadro sai de `codificar_pedido` como `Zeroizing`;
    - nenhuma resposta e nenhum erro o carregam, e nada o escreve em log;
    - na tela, o PIN fica só no estado local do diálogo e sai dele antes da resposta (o
      campo volta vazio) e ao fechar ou voltar;
    - nada vai para `localStorage`, URL ou estado global.
  - **S1:** toda entrada do comando é tratada como não confiável (nome, nível e PIN).
  - **ACL:** `mudar_nivel` só passa na janela `principal`, com a origem local.
- **Testes de ataque:**
  - `app/nucleo/src/nivel.rs`:
    - `ataque_pin_fora_do_formato_e_recusado_sem_eco`
    - `ataque_debug_do_pin_nao_mostra_o_valor`
    - `nivel_so_pelos_tres_nomes_do_protocolo`
  - `app/nucleo/src/servico.rs`:
    - `ataque_pin_errado_nao_muda_e_o_quinto_erro_trava` (4 erros + 1 acerto zera; 5º
      trava; travado, nem o PIN certo muda)
    - `ataque_nome_ou_nivel_forjado_nem_chega_ao_daemon`
    - `ataque_resposta_e_erro_do_mudar_nivel_sem_pin`
    - `mudar_nivel_com_pin_e_os_tres_niveis_tambem_em_prod`
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_mudar_nivel_pela_ponte`
  - `app/src-tauri/src/main.rs` › `ataque_mudar_nivel_so_na_janela_certa_e_pin_no_formato`
    (outra janela, origem remota, PIN `12`/`1357901`/`13579a`/número/nulo recusados na
    leitura, sem eco)
  - QA no Chrome (`qa.json`): `sem_pin_nem_segredo:*`, `armazenamento_vazio`,
    `url_sem_token_nem_pin`, `pin_fora_das_urls`, `console_sem_pin`,
    `so_pedidos_locais`.
- **Dependências novas:** nenhuma. O Playwright do QA roda fora do repositório, no
  scratchpad.

## Ficou de fora
- **Tela `Destravar / Padrão` (Fase 31):** o V5.15 pede que o 5º erro leve a ela. Hoje a
  casca sai e o App mostra o esqueleto "O cofre está travado.", o estado que existe
  antes da 31. Quando a 31 chegar, ela entra no lugar desse esqueleto, e o
  `ontravado` → `consultar()` já leva para lá.
- **Daemon real:** o transporte do app é da 23a. Até lá, só o daemon falso responde.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `cargo test -p ward-app-nucleo -p ward-ponte-teste --all-features`: a 1ª rodada falhou no eco do número no erro do `Pin` (corrigido com `deserialize_any`) | 101, depois 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso cheio` + Playwright (`qa35.py`, Chrome headless, 1280×800): as 2 primeiras rodadas falharam no seletor do roteiro (`dialog` pegava o de filtros) | 0 |
| `make fmt` e `cargo fmt` em `app/src-tauri` | 0 |
| `make check`: a 1ª rodada falhou num `needless_borrow` do clippy em `ataques.rs`, corrigido | 2, depois 0 |
| `make app-check` com o `PKG_CONFIG_PATH` do scratchpad (R32): fmt, clippy, ACL (7 testes, com o `ataque_mudar_nivel_so_na_janela_certa_e_pin_no_formato`), audit, deny e release com a checagem de strings | 0 |
| `make app-front` (final) | 0 |

## Problemas descobertos
- **Fase 31:** o "5º erro leva ao `Destravar / Padrão`" depende dela (veja "Ficou de
  fora").
- **PIN no JS:** uma string de JS não zera. O PIN sai do estado do diálogo antes da
  resposta, mas a cópia no motor do JS (e no corpo do IPC/HTTP) fica até o coletor de
  lixo. É inevitável na webview, e é o mesmo caso do valor digitado no formulário da 33.
- **PIN errado no design (6373):** o frame mostra os dígitos ainda preenchidos. O app
  esvazia o campo (só o contorno de erro fica), para não segurar o PIN errado.
- **Erro de leitura do Tauri:** um PIN fora do formato volta como texto do Tauri (não
  `{codigo, mensagem}`), e a ponte JS o mostra como "falha". A interface nunca manda
  esse formato (o `CampoPin` só aceita dígitos e o botão pede 4 ou mais).
- **Duas vias para o cartão de nível:** o `NIVEIS` de `telas/credenciais.js` continua
  usado pelo `Formulario.svelte` (cartões dos formulários, ainda frames soltos no
  design, handoff 37a). Os textos do `Cartão de nível` estão em `niveis.js`. Uma fase
  que troque os cartões dos formulários por instâncias pode juntar as duas.
- **Contagem:** o cofre de teste agora tem 22 entradas (D59 `delta-api-simbolos`); os
  frames dizem 21. A tela mostra a contagem real.
- **Console do QA:** um 404, do `favicon.ico` (o mesmo das fases 32 e 33).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/nivel.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/lib/componentes/CartaoDeNivel.svelte` | criado |
| `app/src/lib/componentes/OpcaoDeNivel.svelte` | criado |
| `app/src/lib/componentes/DialogoDeNivel.svelte` | criado |
| `app/src/lib/componentes/niveis.js` | criado |
| `app/src/lib/componentes/CampoPin.svelte` | alterado |
| `app/src/telas/Credenciais.svelte` | alterado |
| `app/src/App.svelte` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado (seção Fase 35) |
| `docs/mission/evidence/fase-35/` (9 capturas do app, 4 PNG do design, `qa.json`) | criado |
| `docs/mission/handoffs/phase-35.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; o original não foi tocado, sha256 igual antes e depois)
- [x] Ficou dentro do escopo da fase
