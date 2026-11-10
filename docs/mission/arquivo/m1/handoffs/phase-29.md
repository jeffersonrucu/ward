# Fase 29 — Esqueleto Tauri + Svelte (`app/`)

## Implementado
- **`app/src-tauri` (crate `ward-app`):**
  - Tauri `=2.12.5` sem features padrão (`wry`, `compression`, `custom-protocol`). `tauri-build` `=2.7.4` com `AppManifest::commands(COMANDOS)`, que nega qualquer comando fora da lista.
  - Workspace e `Cargo.lock` próprios. O `Cargo.toml` da raiz tem `exclude`. Motivo: o gtk-rs e o WebKitGTK não entram no `make check` da raiz, que tem de rodar sem os headers do WebKit.
  - Janela `principal` de 1280×800 (mínimo 1024×640), tema escuro e drag-and-drop desligado.
  - `on_navigation` só deixa passar `tauri://localhost`.
  - `.devtools(cfg!(feature = "modo-teste"))`. A feature `tauri/devtools` só entra com `modo-teste`.
  - No `RunEvent::Exit`, a área de transferência é limpa se ainda tiver o valor copiado.
- **`tauri.conf.json`:**
  - `withGlobalTauri: false`, `freezePrototype: true`, assetProtocol desligado, `plugins: {}` e `windows: []` (a janela é criada no código).
  - CSP: `default-src 'none'`; `script`/`style`/`img`/`font` só `'self'`; `connect-src ipc: http://ipc.localhost`; `base-uri`, `form-action`, `frame-src`, `worker-src`, `object-src`, `media-src` e `manifest-src` todos `'none'`. Não há `unsafe-inline`, `unsafe-eval`, `data:`, `blob:` nem origem remota.
- **Capability única:** `capabilities/principal.json` (`local: true`, janela `principal`) com só `allow-estado-do-cofre` e `allow-copiar-campo`.
- **`app/nucleo` (crate `ward-app-nucleo`, no workspace da raiz e no `make check`):** a lógica sem Tauri, testável sem WebKit.
  - `comandos`: lista única dos comandos, que serve ao `build.rs`, à capability e ao frontend. `Referencia` valida entrada e campo: tamanho, controle, bidi e zero-width, espaço nas pontas.
  - `daemon`: `trait Daemon` e `DaemonSocket`. O socket fica em `/run/user/<uid>/ward/wardd.sock`, ou `ward-teste/` com `modo-teste`. O uid vem do dono de `/proc/self`. O diretório tem de ser real, do uid e 0700; o socket tem de ser socket e do uid. Timeouts de 5 s.
  - `area_transferencia`: "copiar" resolvido em Rust (D4). O valor vai do daemon para o `wl-copy` por stdin, com env limpo e binário em `/usr/bin` de dono root sem escrita de grupo/outros. Depois de 30 s a área é limpa só se ainda tiver o mesmo valor (comparação em tempo constante, `ctutils`), e só o timer da cópia mais recente age.
  - `segredo`: `Segredo(Zeroizing<Vec<u8>>)` sem `Debug` do valor.
  - `servico`: `estado_do_cofre` e `copiar_campo`, cuja resposta é `()`.
  - `erro`: `ErroApp` com código estável e mensagem fixa, sem o pedido nem o valor.
- **Frontend Svelte 5 + Vite 8:**
  - Tokens gerados do FigCraft: `app/scripts/gerar_design.py` lê `app/design/figcraft-export.jsonl` e grava `app/src/lib/estilo/tokens.css` e `icones.js`. Os 28 ícones estão em `app/design/icones/`.
  - Componentes do M0 só com tokens: Botão, Campo, Campo secreto, Ação de campo, Select, Badge de ambiente, Badge de tipo, Toast, Item de lista, Modal (`<dialog>`), Campo de PIN, Logo e Ícone. O Campo secreto nunca busca o valor e só mostra a máscara.
  - Fontes Inter e JetBrains Mono locais (fontsource, OFL).
  - Tela-esqueleto: mostra o estado do daemon, com erro e "Tentar de novo".
  - `lib/ponte`: `chamar()` aceita só comandos da lista e normaliza erro para `{codigo, mensagem}`. O transporte é o `invoke` do Tauri. No build `--mode teste` sem Tauri, o transporte é a ponte HTTP.
  - O build de produção (`dist/`) não tem ponte nem vitrine, que somem por dead-code. O build de teste (`dist-teste/`) tem as duas.
- **Ponte de teste (D7), `crates/ward-ponte-teste`, só com a feature `modo-teste`:**
  - Escuta só em 127.0.0.1, com porta aleatória.
  - Token de 32 bytes (`getrandom`) no fragmento da URL. A página o lê uma vez e o tira da URL.
  - Confere Host (421), Origin (403), `Authorization: Bearer` em tempo constante (401), Content-Type (415), comando da lista (404) e corpo JSON objeto (400).
  - Limites: cabeçalho de 8 KiB, corpo de 16 KiB, 16 conexões. Só HTTP/1.1. Transfer-Encoding dá 501. Content-Length duplicado é recusado.
  - Arquivos estáticos só de dentro do `dist` canonicalizado.
  - Mesma CSP do app e cabeçalhos de segurança.
  - O binário exige `--features modo-teste`.
- **Makefile:**
  - `app-front`: `npm ci --ignore-scripts`, os dois builds, `dev/testes/app-dist.py` e `npm audit`.
  - `app-check`: `app-front`, depois fmt, clippy, testes, audit, deny e release do Tauri, mais `dev/testes/app-release.sh` (V5.6).
  - `app-ponte`: sobe a ponte servindo `app/dist-teste`.
  - `CLAUDE.md` documenta os três.
- **`docs/mission/design-nodes.md`:** seção da Fase 29 com a origem dos tokens e dos ícones.

### Pontos de ligação para as Fases 20–22 (protocolo e daemon)
1. **`app/nucleo/src/daemon.rs`, `impl Daemon for DaemonSocket`:**
   - `estado()` e `valor_para_copiar()` hoje conectam (com toda a conferência do socket) e devolvem `ErroApp::SemProtocolo`. A Fase 20 troca o corpo pelos pedidos do protocolo: o estado e a leitura de um campo para cópia.
   - O valor tem de chegar em `Segredo` (Zeroizing) e não pode ser logado.
2. **Conferência do par (Fase 21):** em `DaemonSocket::conectar`, depois do `connect`, conferir pelo `SO_PEERCRED` que o pid/uid do outro lado é o `wardd` do mesmo uid. O comentário `// Fase 21:` marca o lugar, e o `unsafe`, se houver, vai no módulo de plataforma.
3. **Nome do socket:** `SOCKET = "wardd.sock"` é provisório. Alinhar com o nome que a Fase 20 fixar. Os diretórios `ward/` e `ward-teste/` já seguem `CLAUDE.md`.
4. **Papel do app (D5):** o app fala com o daemon como "app" (destravar, copiar). A ponte de teste usa o mesmo `Servico` e portanto o mesmo papel. A Fase 20/22 decide se o daemon distingue app de CLI pelo binário.
5. **Comandos novos:** acrescentar em `comandos::COMANDOS`, `capabilities/principal.json` e `app/src/lib/ponte/comandos.js`. O teste `configuracao_tauri` falha se os três divergirem.

## Segurança
- **Invariantes tocadas:**
  - **S11** (interface sem segredo, sem conteúdo remoto, sem devtools/ponte no release):
    - capability mínima e `AppManifest::commands`;
    - CSP estrita igual no app e na ponte;
    - `on_navigation`;
    - `withGlobalTauri` desligado;
    - nada em `localStorage`/`sessionStorage`/cookie/URL; o token sai da URL;
    - "copiar" devolve `()`;
    - devtools e ponte só com `modo-teste`;
    - `app-dist.py` e `app-release.sh` conferem os pacotes.
  - **S12** (área de transferência limpa em 30 s sem apagar o texto do usuário): limpeza condicional por comparação em tempo constante e geração da cópia. Valor por stdin, nunca em argv.
  - **D4** (copiar no Rust), **D7** (ponte só em modo de teste).
  - Entrada não confiável validada: `Referencia` no comando Tauri e, na ponte, cabeçalhos, caminho, corpo e comando.
- **Testes de ataque escritos:**
  - `app/src-tauri/src/main.rs`:
    - `ataque_plugins_e_comando_inventado_sao_negados` (V5.1: fs, shell, http, clipboard, window, event, app e comando inventado negados pelo runtime falso do Tauri com o contexto real);
    - `ataque_comandos_proprios_so_na_janela_e_origem_certas`;
    - `ataque_navegacao_para_fora_do_app_e_recusada`;
    - `ataque_falha_nao_carrega_valor`.
  - `app/nucleo/tests/configuracao_tauri.rs`:
    - `ataque_capability_com_permissao_a_mais`;
    - `ataque_csp_frouxa_ou_conteudo_remoto`;
    - `ataque_devtools_ou_acl_dinamica_no_release`;
    - `ataque_interface_guarda_ou_injeta_conteudo`;
    - `lista_de_comandos_da_interface_e_a_mesma`.
  - `app/nucleo/src/area_transferencia.rs`:
    - `ataque_texto_do_usuario_copiado_depois_nao_e_apagado` (V5.5);
    - `ataque_prefixo_do_valor_nao_conta_como_igual`;
    - `ataque_valor_vazio_ou_grande_demais_e_recusado`;
    - `ataque_binario_fora_do_padrao_e_recusado`;
    - mais os testes de limpeza em 30 s e de troca de cópia.
  - `app/nucleo/src/daemon.rs`:
    - `ataque_diretorio_aberto_para_outros_e_recusado`;
    - `ataque_diretorio_por_link_simbolico_e_recusado`;
    - `ataque_arquivo_comum_no_lugar_do_socket_e_recusado`;
    - `ataque_dono_diferente_e_recusado`.
  - `app/nucleo/src/comandos.rs`: `ataque_comando_inventado_ou_de_plugin_nao_existe`, `ataque_referencia_fora_do_formato`.
  - `app/nucleo/src/servico.rs`: `ataque_copiar_nao_devolve_o_valor` (V5.4 com daemon e área falsos), `ataque_erro_nao_carrega_o_pedido`.
  - `app/nucleo/src/segredo.rs`: `ataque_debug_nao_mostra_o_valor`.
  - `crates/ward-ponte-teste/tests/ataques.rs`:
    - `ataque_sem_token_token_errado_ou_repetido`;
    - `ataque_host_ou_origem_de_outro_site`;
    - `ataque_comando_de_plugin_ou_inventado`;
    - `ataque_arquivo_fora_do_dist` (inclui link simbólico);
    - `ataque_pedido_fora_dos_limites`;
    - `csp_da_ponte_e_a_do_app`;
    - `tcp_so_em_127_0_0_1_com_cabecalhos_de_seguranca`;
    - mais o caminho feliz do copiar, sem valor na resposta.
  - `dev/testes/app-dist.py`: conferido com isca (URL remota e `/ponte/` plantadas em `dist/` fazem sair 1).
  - `dev/testes/app-release.sh`: conferido nos dois sentidos. O release normal passa. Um release com `--features modo-teste` falha (achou `ward-teste`).
  - QA no Chrome headless (Playwright em Python, fora do repositório), contra a ponte, em 1280×800:
    - injeção de `<img>`/`<script>` remotos, `fetch` remoto e script inline: os quatro bloqueados pela CSP (`securitypolicyviolation`), e nenhuma requisição chegou à rede;
    - `fetch('/ponte/…')` sem token dá 401;
    - token fora da URL depois do carregamento;
    - `localStorage`/`sessionStorage`/cookie vazios;
    - fundo escuro mesmo com o sistema em tema claro.
- **Dependências novas e motivo:**
  - `tauri` 2.12.5 e `tauri-build` 2.7.4: o app desktop do escopo. Sem features padrão, para não puxar devtools, `dynamic-acl`, x11 ou dbus.
  - `serde` (derive), no app: (de)serializar os argumentos e as respostas dos comandos Tauri. Já estava no lock da raiz.
  - `serde_json`: só em dev-deps do app e do núcleo, e opcional na ponte (corpo JSON). Já estava no lock.
  - `getrandom`, na ponte: token da ponte por CSPRNG. Já estava no lock.
  - `ctutils` 0.4.8: comparação em tempo constante do valor na área de transferência e do token da ponte. Já estava no lock (D40).
  - `zeroize` 1.9.1: `Segredo`. Já estava no lock.
  - npm, versões exatas e lockfile, `.npmrc` com `ignore-scripts`, `save-exact` e `engine-strict`, instalado com `--before`:
    - `@tauri-apps/api` 2.12.4: `invoke` do Tauri, a única dependência de runtime;
    - `svelte` 5.57.2, `vite` 8.3.5 e `@sveltejs/vite-plugin-svelte` 7.3.1: interface e build;
    - `@fontsource/inter` 5.3.0 e `@fontsource/jetbrains-mono` 5.3.0: fontes do design servidas localmente, sem Google Fonts.
  - `cargo audit` e `cargo deny` limpos na raiz. No app, limpos com duas exceções escritas em `app/src-tauri/deny.toml` e no `make app-check` (ver Problemas).
  - `npm audit --omit=dev` dá 0. O completo dá 2 low, só de build (ver Problemas).

## Ficou de fora
- **Protocolo e daemon (Fases 20–22):** os métodos do `DaemonSocket` devolvem `SemProtocolo` depois de conectar. Hoje o app mostra "o daemon do Ward não está rodando".
- **V5.4 e V5.5 de ponta a ponta com `wl-paste` real:** dependem do daemon. A lógica está coberta com daemon e área falsos. O `Wayland` real (`wl-copy`/`wl-paste`) não foi exercido nesta fase.
- **Telas do M0 (Fases 30–39) e conferência pixel a pixel dos componentes contra cada node:** os componentes usam os tokens e a estrutura da Design System, mas a conferência fina fica para as fases de UI.
- **Empacotamento (`bundle`):** desligado. Fica para a fase de instalação.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make fmt` e `cargo fmt` em `app/src-tauri` | 0 |
| `make check` (1ª vez: fmt; 2ª: clippy `useless_borrows_in_formatting` na ponte, corrigido) | 2, 2 |
| `make check` | 0 |
| `make app-check` (com `PKG_CONFIG_PATH` para os .pc extraídos no scratchpad) | 0 |
| `npm run build` / `npm run build:teste` | 0 |
| `python3 -I -B dev/testes/app-dist.py` | 0 (1 com a isca plantada) |
| `dev/testes/app-release.sh app/src-tauri/target/release/ward-app` | 0 |
| `dev/testes/app-release.sh <release com --features modo-teste>` | 1 (esperado) |
| `cargo test -p ward-ponte-teste --features modo-teste` | 0 |
| `make app-ponte` + Playwright (Python, Chrome headless) | 0 |

## Problemas descobertos
- **Headers do WebKitGTK ausentes na máquina:** `make app-check` precisa de `sudo apt install libwebkit2gtk-4.1-dev` (que traz gtk3, soup3 e javascriptcore). Aqui, sem sudo, usei os .deb de desenvolvimento extraídos no scratchpad via `PKG_CONFIG_PATH`. Por isso o crate do Tauri ficou fora do `make check`.
- **Advisories do gtk-rs 0.18, exigido pelo Tauri 2 no Linux:**
  - RUSTSEC-2024-0370: `proc-macro-error` sem manutenção, só em tempo de build;
  - RUSTSEC-2024-0429: `glib::VariantStrIter` unsound; o app não usa `Variant`.
  Os dois estão ignorados com motivo só no `deny.toml` do app e no `make app-check`. Some quando o Tauri migrar para gtk-rs ≥ 0.20. Precisa de aceite do usuário.
- **Licenças MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`, `option-ext`): exceções por crate no `deny.toml` do app. São dependências do wry/tauri-utils.
- **`npm audit` completo:** 2 low (`deepmerge`, via `vite-plugin-svelte`), só em build e sem correção disponível. O de runtime está limpo.
- **Texto da V5.2:** o contrato pede que `grep -rE 'https?://' dist/` não ache nada, mas o Svelte embute os namespaces XML (`http://www.w3.org/2000/svg` e `http://www.w3.org/1999/xhtml`). São nomes, não endereços. O `app-dist.py` aceita só esses namespaces. Sugiro ajustar o texto da V5.2. Não escolhi lado.
- **`eval` não é bloqueado no Chrome da ponte:** o QA viu `eval('1')` rodar na página servida pela ponte, apesar de a CSP não ter `unsafe-eval`. A causa provável é o Playwright/CDP, que avalia no mundo isolado do DevTools. Vale o auditor confirmar com um `<script src>` próprio.
- **Corrida entre ler e limpar a área de transferência:** entre o `wl-paste` e o `wl-copy --clear`, o usuário pode copiar algo que acaba apagado. A janela é de milissegundos, e o Wayland não tem compare-and-swap.
- **`wl-copy` no COSMIC:** confirmar que o `wl-copy` e o `wl-copy --clear` funcionam no compositor do usuário (V5.5 de ponta a ponta).
- **Isolation pattern do Tauri não usado:** a CSP estrita, sem conteúdo remoto, e a capability mínima cobrem o caso. Fica como decisão aberta para o auditor.
- **Papel do app no daemon (D5) e conferência por `SO_PEERCRED`:** ver os pontos de ligação 2 e 4.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `.gitignore` | alterado (`dist-teste/`, `app/src-tauri/gen/`, `permissions/autogenerated/`) |
| `Cargo.toml`, `Cargo.lock` | alterado (membro `app/nucleo`, `exclude` de `app/src-tauri`) |
| `Makefile` | alterado (`app-front`, `app-check`, `app-ponte`) |
| `CLAUDE.md` | alterado (comandos do app) |
| `dev/gitleaks-arvore.toml`, `dev/gitleaks-extra-arvore.toml` | alterado (diretórios gerados do app) |
| `dev/testes/app-dist.py`, `dev/testes/app-release.sh` | criado |
| `docs/mission/design-nodes.md` | alterado (seção da Fase 29) |
| `docs/mission/handoffs/phase-29.md` | criado |
| `crates/ward-ponte-teste/Cargo.toml`, `src/lib.rs` | alterado |
| `crates/ward-ponte-teste/src/http.rs`, `src/main.rs`, `tests/ataques.rs` | criado |
| `app/.npmrc`, `app/package.json`, `app/package-lock.json`, `app/vite.config.js`, `app/index.html` | criado |
| `app/design/figcraft-export.jsonl`, `app/design/icones/*.svg` (28), `app/scripts/gerar_design.py` | criado |
| `app/src/main.js`, `app/src/App.svelte`, `app/src/lib/estilo/{tokens,base}.css` | criado |
| `app/src/lib/componentes/*` (13 componentes, `tipos.js`, `icones.js`, `ward-simbolo.svg`) | criado |
| `app/src/lib/ponte/{index,comandos,ponte-teste}.js`, `app/src/lib/teste/Vitrine.svelte` | criado |
| `app/nucleo/Cargo.toml`, `app/nucleo/src/*.rs` (7), `app/nucleo/tests/configuracao_tauri.rs` | criado |
| `app/src-tauri/{Cargo.toml,Cargo.lock,build.rs,tauri.conf.json,deny.toml}`, `capabilities/principal.json`, `icons/*.png` (3), `src/main.rs` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks), e `make app-check` também
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado; dados falsos `clientex`)
- [x] Teste de ataque para cada (SEG) tocado (V5.1, V5.2, V5.4, V5.5 na lógica, V5.6)
- [x] (UI/DESIGN) Tokens e ícones lidos do FigCraft (cópia; original intacto, sem `save_file`). Vitrine conferida no Chrome em 1280×800
- [x] Ficou dentro do escopo da fase
