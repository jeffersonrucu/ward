# Fase 41 — (UI) Extensão MV3 e popup "Sugestões do domínio"

## Implementado
- **Extensão MV3** em `extensao/`, montada em `extensao/dist/` por `make extensao` (`dev/extensao/montar.py`).
  - **Manifesto.** Permissões só `nativeMessaging`, `activeTab` e `scripting`, sem `host_permissions`. A extensão não tem `content_scripts`, `background`, `externally_connectable`, `web_accessible_resources` nem a permissão `storage`, e não roda em janela anônima (`incognito: not_allowed`). A CSP das páginas é `default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; img-src 'self'; connect-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'`, repetida num `<meta>` do `popup.html`.
  - **`key` fixo.** Só a chave pública RSA 2048 fica no repositório, em `extensao/chave-publica.txt`, e o `montar.py` a põe como `key` no `dist/manifest.json`. O id fica `nhkehjnmlgkebhmelajfpmpblepaichf`. O manifesto do repositório não leva a `key`, porque o scanner de segredos (regra `generic-api-key`) acusa `"key": <base64>` e um padrão no allowlist iria contra a regra do `.gitleaks.toml`. O `montar.py` recusa o manifesto que tiver `key`. A chave privada foi gerada em `.dev/extensao/chave.pem` (0600, fora do git) e só serve para empacotar o `.crx`.
  - **Conferências do `montar.py`** (ele recusa montar se alguma falhar):
    - o id da chave tem de ser igual ao `ID_EXTENSAO` do `ward-nm`;
    - o manifesto não pode ter chave nem permissão proibida;
    - a CSP não pode ser frouxa;
    - nenhum arquivo pode ter URL remota (só o namespace SVG passa);
    - nenhum pode ter `eval`, `new Function`, `innerHTML`/`outerHTML`/`insertAdjacentHTML`/`document.write`, `localStorage`/`sessionStorage`/`indexedDB`/`chrome.storage` ou ouvinte de mensagem (`onMessage`, `onMessageExternal`, `onConnectExternal`, `message`);
    - o HTML não pode ter script nem estilo inline;
    - não pode haver link simbólico nem tipo de arquivo inesperado.
  - **Uma fonte só para tokens, ícones e fontes.** Os tokens são o `tokens.css` gerado do FigCraft (`app/src/lib/estilo/tokens.css`), copiado sem mudança. Os ícones vêm de `icones.js` e o símbolo de `ward-simbolo.svg`, ambos do app. As fontes Inter 400/500/600 e JetBrains Mono 400 vêm do `@fontsource` do app. Os ícones da extensão são `docs/design/icones/extensao-destravado-*.png`. O `popup.css` não tem cor nem medida solta: só `var(--…)`.
- **Popup** (`popup.html`, `popup.css`, `popup.js`), feito pelos frames do FigCraft:
  - **`Extensão / Sugestões do domínio` (17332).** Cabeçalho do popup no estado destravado, cartão "Origem da aba", contagem, `Item de lista` compacto com avatar, nome, `Badge ambiente`, `Badge tipo` login e "usuário · com TOTP", a nota e o rodapé "Abrir o Ward" (`abrir_app`).
  - **`Extensão / Cofre travado` (17392).** O popup precisa tratar a resposta `travado`, então o estado entrou com as medidas do frame. O V6.9 e a conferência do tema continuam com a Fase 42.
  - **Estados sem frame:** "Nenhuma entrada para esta origem", mensagem de erro na nota e aviso de host ausente ou não reconhecido (ver Problemas).
  - **Textos do daemon** entram só por `textContent`.
- **Fluxo do preenchimento** (`popup.js` e `pagina.js`; a página não tem com quem falar, porque não há service worker, content script fixo nem ouvinte):
  1. A origem da aba sai de `tab.url` (dado pelo `activeTab`) e vai em `sugerir`.
  2. No clique, uma sonda roda em `frameIds: [0]` e devolve `location.origin`, se o documento é o topo e o `documentId`. A origem tem de ser a mesma do passo 1.
  3. `preencher` vai com essa origem, e o daemon compara (D9).
  4. Usuário, senha (em base64) e TOTP seguem como argumentos para `preencherNaPagina`, injetada só naquele `documentId`. Se a aba navegou, o Chrome recusa a injeção. A função confere `window === window.top` e `location.origin` de novo antes de escrever.
  5. O popup apaga os campos da resposta e fecha.
- **`preencherNaPagina`** roda no mundo isolado e usa só os protótipos de lá (`Document.prototype.querySelectorAll.call`, os descritores de `HTMLInputElement.prototype`, `EventTarget.prototype.dispatchEvent`), contra DOM clobbering.
  - **Campos.** Preenche um campo de senha só: o primeiro visível. O usuário é o campo de texto visível antes dele, no mesmo formulário, de preferência com `autocomplete=username`. O TOTP só entra num campo `one-time-code` (ou com nome ou id do tipo otp/código) do mesmo formulário.
  - **O que conta como visível.** O campo precisa:
    - passar no `checkVisibility` com opacidade e visibilidade, inclusive dos ancestrais;
    - ter pelo menos 4×4;
    - estar dentro da página;
    - não ter `clip`/`clip-path`;
    - não estar coberto por outro elemento no centro (um `<label>` do próprio campo pode cobrir);
    - não estar desabilitado nem só leitura.
  - **Senha.** Os bytes decodificados ficam num `Uint8Array` zerado no `finally`.
  - **Envio.** Não envia o formulário: quem clica em Entrar é o usuário. O envio com limpeza é do `ward login` (Fase 43).
- **Protocolo** (`ward-core`): a `Sugestao` ganhou `ambiente` (dev/stage/prod) e `totp` (bool, só o fato de a entrada ter semente). O frame mostra o selo de ambiente e "com TOTP". Nenhum valor entra. A codificação e a decodificação são estritas, o daemon preenche os dois campos, e os testes do protocolo e do `ward-nm` foram atualizados.
- **`ward-nm` com id fixo.** `ID_EXTENSAO` agora é obrigatório no argumento `chrome-extension://<id>/`: outra extensão, mesmo com o id bem formado, sai com 2 sem nada no stdout. Isso fecha a pendência "host não confere o id" da 40. O id dos testes (`binario.rs`, `testes.rs`, `chrome-falso.rs`, `dentro.sh`, `nm.py`) passou a ser o do Ward, e a extensão de teste do `nm.py` leva a mesma `key` (lida de `extensao/chave-publica.txt`).
- **Decisão sobre o papel extensão no host de desenvolvimento:** opção **(a)**, uma exceção de `modo-teste` no modelo de D63. A justificativa está em Segurança.
  - `WARD_DEV_EXTENSAO=chrome` no daemon de teste. Qualquer outro valor recusa a partida.
  - `ExtensaoDeTeste` é inabitável sem a feature (`Infallible`), como o `AppDeTeste`.
  - `ProcessoPar::classificar_host_extensao_de_teste` (só com `modo-teste`) exige o mesmo uid, a política do kernel, nenhum depurador e o pai igual ao `/opt/google/chrome/chrome` instalado (`conferir_pai`, pelo `/proc`). É o release menos o egid.
  - O `dev/daemon.sh` repassa `WARD_DEV_EXTENSAO` só se vier definida. O padrão continua desligado.
- **`make teste-extensao`** (`dev/testes/extensao.sh` e `extensao.py`, fora do `make check`), com 57 conferências e relatório em `docs/mission/evidence/fase-41/qa.json`.
  - **Montagem.** Um `wardd` de teste só dele, com cópia do cofre e socket próprio, sobe com `WARD_DEV_EXTENSAO=chrome`.
  - **Chrome.** Roda headless com perfil e HOME em `.dev/extensao.*` e o manifesto de native messaging só no perfil. A extensão entra por `Extensions.loadUnpacked`, e o clique no ícone é o `Extensions.triggerAction` (no alvo `tab`), que dá o `activeTab` como um clique de verdade. Tudo pelo `--remote-debugging-pipe`, sem porta.
  - **Páginas hostis.** São servidas pelo CDP (`Fetch.fulfillRequest`) na origem certa `http://localhost:8081/ward-teste/…`.
  - **Medidas** conferidas com `getBoundingClientRect` contra os nodes do FigCraft: 17332 inteiro (popup 360×494, cabeçalho 57, origem 328×70, itens 328×76 com gap 4, avatar 40, nota 48, rodapé 77, botão 328×44) e 17392 (360×343, avatar 48, título, texto, botão 312×44 e nota). Também as cores, as fontes carregadas e o hover do item.
- **Daemon de teste da CLI.** O `make dev-daemon` repassa `WARD_DEV_EXTENSAO`, e o `CLAUDE.md` ganhou as linhas de `make extensao` e `make teste-extensao`, mais as notas em `teste-sem-modo-teste` e `dev-daemon`.

### Receita do QA (V6.2 (UI), V6.7 no Chrome de teste)
1. `make app-front` e depois `make extensao`.
2. `cargo build -p ward-nm --features modo-teste`.
3. `WARD_DEV_EXTENSAO=chrome make dev-daemon`, opcionalmente com `WARD_DEV_SOCKET_DIR` próprio.
4. Chrome com `--user-data-dir=.dev/<perfil>` e, em `<perfil>/NativeMessagingHosts/br.ward.nm.json`, um manifesto com `path` = `target/debug/ward-nm` e `allowed_origins` = `chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/`.
5. O Chrome precisa ter no ambiente o `WARD_DEV_SOCKET_DIR` do daemon, se ele não usar o padrão.
6. Carregar a extensão por `Extensions.loadUnpacked` (pipe com `--enable-unsafe-extension-debugging`). O `make teste-extensao` faz tudo isso sozinho.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1 (D5, D9).** Segredo só sai do daemon em `preenchimento`, para o papel extensão e com a origem igual à da entrada.
    - A extensão manda a origem do documento do quadro principal, e a injeção fica presa ao `documentId` sondado e confere a origem de novo.
    - Iframe de outra origem não recebe nada, porque só o documento principal é lido.
    - O `ward-nm` só aceita o id do Ward.
    - A página não tem canal até a extensão: sem `externally_connectable`, sem ouvinte, sem `web_accessible_resources`.
  - **S2 e S13.** Nada é guardado.
    - Não há a permissão `storage` (o `chrome.storage` nem existe no popup), nem `localStorage`, `sessionStorage` ou IndexedDB.
    - O DOM do popup só tem nome, usuário e ambiente.
    - A resposta é apagada e o popup fecha depois do preenchimento.
    - No perfil temporário inteiro, nada da extensão tem padrão de `.dev/variacoes.txt`.
    - Os logs do daemon, do Chrome e do `ward-nm`, e o console, ficam sem padrão; o log do daemon não tem as origens pedidas.
  - **S11.** CSP sem `unsafe-*` nem origem remota, e sem inline, conferido no build.
- **Decisão (a), exceção de `modo-teste`, e o rigor comparado com D63**
  - **Mesmo princípio de D63** ("o caminho de teste é o do release menos o egid"). No release, o papel extensão exige egid `ward-nm`, política do kernel, nenhum depurador e o pai Chrome instalado conferido pelo `/proc`. No teste, exige o mesmo menos o egid.
  - **Contra vazar para o release:**
    - tudo fica atrás de `cfg(feature = "modo-teste")`, e o tipo é inabitável sem a feature;
    - a exceção só liga com `WARD_DEV_EXTENSAO=chrome`, explícito, e nada a liga por padrão;
    - o `make teste-sem-modo-teste` passou a procurar `WARD_DEV_EXTENSAO` e `classificar_host_extensao_de_teste` nas strings do release. Rodou com 0, e o autoteste confere que o `target/debug/wardd` de teste contém a marca e o release não;
    - o V3.15 (`aprovacao.rs`) continua sem `modo-teste` em `servidor.rs`: o teste do servidor usa `ExtensaoDeTeste::do_chrome`, que dá `None` sem a feature.
  - **Diferença para D63, explícita.** O D63 ainda amarra o pid ao processo que o lançador abriu, e o binário é escolhido pelo daemon. Aqui não há como amarrar ao binário do `ward-nm`: o par não é dumpable, então o `exe` dele não se lê, e lê-lo seria a conferência pelo `/proc` que D47 descartou. Daí o resíduo, que vale **só no daemon de teste** (cofre de teste, credenciais falsas): qualquer processo do uid que abra o Chrome instalado com um manifesto de native messaging próprio ganha o papel extensão no daemon de teste. É o mesmo resíduo RA6 do release (aceito) e é menor que o da ponte de D63, em que qualquer processo do uid que leia `.dev/logs` vira app. **O planner precisa registrar como D-novo.**
- **Testes de ataque escritos**
  - `crates/ward-plataforma/tests/identificacao.rs`, `ataque_host_extensao_de_teste_so_com_o_pai_chrome`: o pai certo passa, outro exe recusa (`PaiNaoPermitido`), a política fraca recusa (`PoliticaKernel`), o pai sendo o teste recusa, e o mesmo par pelo grupo (release) fica comum.
  - `crates/wardd/src/servidor.rs`, `d63_par_filho_sem_egid_fica_agente` (ampliado): com a exceção ligada, o par que não é filho do Chrome continua agente; sem a feature, a exceção não se constrói.
  - `crates/wardd/src/config.rs`, `extensao_de_teste_so_com_chrome`: `""`, `1`, `Chrome`, `chrome ` e o caminho são recusados.
  - `crates/wardd/tests/extensao.rs`, `sugestao_traz_ambiente_e_se_tem_totp`: só ambiente e o booleano, sem segredo nos bytes.
  - `crates/ward-nm/src/testes.rs`, `origem_da_extensao`: outro id bem formado (`aaaa…`, o antigo de teste) é recusado. `crates/ward-nm/tests/binario.rs` também mudou.
  - `dev/testes/extensao.py` (`make teste-extensao`):
    - **V6.2:** em 127.0.0.1 nada é sugerido, e o preencher forçado recebe `origem_diferente`. A injeção com origem esperada diferente da do documento não escreve nada.
    - **Phishing 8082 e redir 8084:** nada sugerido, o preencher forçado é recusado e o `posts.log` fica sem mudança.
    - **Aba que navega para o phishing com o popup aberto:** nada preenchido.
    - **V6.4:** o iframe do phishing na origem certa fica vazio. Com formulário só no iframe, nada é preenchido.
    - **DOM clobbering:** `form name=querySelectorAll`, `img name=documentElement`/`elementFromPoint`, `input name=querySelectorAll/elements/action`, mais os protótipos `value`, `querySelectorAll`, `checkVisibility` e `atob` trocados pela página. Os campos certos são preenchidos, a isca fica vazia e nenhum protótipo da página é chamado.
    - **Campo escondido:** dez iscas antes do campo real (`display`, `visibility`, `opacity`, pai com opacidade 0, 1×1, fora da página, `clip-path`, coberta, desabilitada e usuário escondido) ficam vazias. Uma página só com campos escondidos não recebe nada e o popup avisa.
    - **V6.6:** a página não tem `chrome.runtime`. O `sendMessage(<id>)` dá erro. O `fetch`/iframe de `popup.html` e `manifest.json` é bloqueado. O `postMessage` pedindo preencher não faz nada.
    - **V6.5:** sem `chrome.storage` e sem armazenamento, o DOM do popup fica sem segredo e o perfil sem segredo da extensão.
    - **Exceção:** o `ward-nm` aberto à mão e o filho do `chrome-falso` continuam agentes. O `ward-nm` com outro id sai com 2.
- **Dependências novas:** nenhuma crate, nenhum pacote npm. As fontes vêm do `@fontsource` que o app já tem.

## Ficou de fora
- **V6.8 e V6.9:** Fase 42. O estado travado já existe com as medidas do 17392, mas a conferência de tema claro e escuro e o "abre o app no destravar" são da 42.
- **Envio do formulário e limpeza dos campos:** é o `ward login`, na Fase 43 (V6.3, V6.10). O popup só preenche, e o login do 8083 entra quando o usuário (ou o teste) envia.
- **Instalação por política do Chrome e manifesto de native messaging do sistema:** Fase 51.
- **Pacote `.crx` assinado:** Fase 51/52, com a chave de `.dev/extensao/chave.pem`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make gitleaks-binario`, `make ferramentas`, `make dev-cofre`, `make dev-variacoes`, `make app-front` | 0 |
| `make extensao` | 0 |
| `make teste-extensao` (57 conferências) | 0 |
| `make teste-nm` (com o id fixo) | 0 |
| `make teste-sem-modo-teste` | 0 |
| `make teste-docker-plataforma` (Fase 40 no contêiner com o id novo) | 0 |
| `cargo test -p ward-core --lib protocolo`, `-p wardd --all-features` (lib, `extensao`, `aprovacao`), `-p ward-plataforma --features modo-teste --test identificacao`, `-p ward-nm --all-features` | 0 |
| `make check` (1ª rodada) | 2 (V3.15: `cfg(feature = "modo-teste")` no teste de `servidor.rs`; corrigido com `ExtensaoDeTeste::do_chrome`) |
| `make check` (2ª e 3ª rodadas, máquina com carga ~13 de outros workers) | 2 (testes de tempo `d34_pty_prefixo_parado…` e `v3_33_kill_9…`, fora desta fase; passaram sozinhos) |
| `RUST_TEST_THREADS=2 make check` (4ª rodada) | 2 (scanner de segredos: `"key"` do manifesto; corrigido movendo a chave pública para `extensao/chave-publica.txt`) |
| `RUST_TEST_THREADS=2 make check` (5ª rodada) | 2 (`v3_33_kill_9…` de novo: primeira espera 1,506 s contra o limite de 1,5 s, com outro worker usando ~7 núcleos) |
| `RUST_TEST_THREADS=1 make check` (final) | 0 ("make check: tudo limpo") |

## Problemas descobertos
- **Chave privada da extensão no worktree.** Ela está em `.dev/extensao/chave.pem`, fora do git. Antes de apagar o worktree, mova o arquivo para o `.dev/extensao/` do checkout principal. Sem a chave, empacotar o `.crx` da Fase 51 exige gerar outra, e aí mudam o `key`, o `ID_EXTENSAO` do `ward-nm` e os testes.
- **Estados do popup sem frame no FigCraft:**
  - lista vazia: "Nenhuma entrada para esta origem", com outra nota;
  - erro na nota, em vermelho: "A aba mudou de endereço…", "Não achei um campo de senha visível…", "A origem da aba não confere…";
  - aviso de host ausente ou não reconhecido, ou de aba sem http(s): só o cabeçalho sem pílula, a nota e o rodapé.

  Fiz com os mesmos componentes e tokens. O design precisa desenhar esses frames.
- **Fonte da origem.** O node `17384` diz JetBrains Mono 13, mas o render do FigCraft (`figcraft-17332-…png`) mostra uma fonte sem serifa, provavelmente substituta. Segui o node.
- **Protocolo.** A `Sugestao` ganhou `ambiente` e `totp` (bool). É preciso atualizar o texto do PRD/contrato, se descreverem o `sugerir` como "nome e usuário".
- **Strings em JS não se zeram.** Só os bytes da senha decodificada são zerados. As strings (o argumento base64, a senha decodificada, o valor no campo) ficam até o GC do mundo isolado e do popup, que fecha logo. É um limite da plataforma, e o escopo já põe a leitura do campo pela página fora do modelo.
- **O Local Network Access do Chrome** barra o iframe de `localhost:8082` dentro de uma página servida pelo CDP, porque ela fica sem IP e o Chrome a trata como "pública". O teste usa `--disable-features=LocalNetworkAccessChecks` só no Chrome de teste. Não é isso que protege o preenchimento.
- **Code Cache do Chrome.** No perfil temporário, `Default/Code Cache/js/…` casou um padrão de `.dev/variacoes.txt` dentro do dicionário de senhas comuns (rot13) que o próprio Chrome traz. Não é dado da extensão nem do cofre; provavelmente um padrão curto (o PIN de teste, só dígitos) que existe nesse dicionário. O teste só falha quando há padrão em arquivo da extensão e lista o resto no `qa.json` (`perfil_com_padrao_fora_da_extensao`). Vale olhar no `dev-variacoes` se o PIN deveria ser padrão.
- **Pai Chrome no teste.** O `conferir_pai` aceita qualquer processo cujo pai tenha o `exe` do Chrome, então os auxiliares (zygote, GPU, utilitários) também passariam se conectassem ao socket. No release o egid barra isso. No daemon de teste, é parte do resíduo acima, só com credenciais falsas.
- **Testes sensíveis a tempo.** Com a máquina carregada (load ~13, outros workers em paralelo), dois testes falharam uma vez cada no `make check` e passaram sozinhos. Não tocam esta fase, mas ficam instáveis sob carga:
  - `wardd --test ssh`, `d34_pty_prefixo_parado_vira_marca_e_o_resto_nao_sai`;
  - `wardd --test socket`, `v3_33_kill_9_entre_as_senhas_erradas`: a primeira espera, que deve ficar abaixo de 1,5 s, mediu 3,58 s.
- **O daemon de teste registra `papel extensão de teste (pid N)` a cada conexão do host** (uma por mensagem), em nível `aviso`. Só existe no `modo-teste`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `extensao/manifest.json` | criado |
| `extensao/chave-publica.txt` | criado (só a chave pública, em PEM; `.txt` porque o `.gitignore` ignora `*.pem`) |
| `extensao/popup.html` | criado |
| `extensao/popup.css` | criado |
| `extensao/popup.js` | criado |
| `extensao/pagina.js` | criado |
| `dev/extensao/montar.py` | criado |
| `dev/testes/extensao.sh` | criado |
| `dev/testes/extensao.py` | criado |
| `docs/mission/evidence/fase-41/*` (7 PNG e `qa.json`) | criado |
| `docs/mission/handoffs/phase-41.md` | criado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado (`Sugestao.ambiente` e `totp`) |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-nm/src/lib.rs` | alterado (`ID_EXTENSAO`) |
| `crates/ward-nm/src/testes.rs` | alterado |
| `crates/ward-nm/tests/binario.rs` | alterado |
| `crates/ward-nm/examples/chrome-falso.rs` | alterado (id) |
| `crates/ward-plataforma/src/processo.rs` | alterado (`classificar_host_extensao_de_teste`, só com `modo-teste`) |
| `crates/ward-plataforma/tests/identificacao.rs` | alterado |
| `crates/wardd/src/config.rs` | alterado (`WARD_DEV_EXTENSAO`) |
| `crates/wardd/src/lancamento.rs` | alterado (`ExtensaoDeTeste`) |
| `crates/wardd/src/servidor.rs` | alterado (`Papeis.extensao_de_teste`) |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado (sugestão com ambiente e TOTP) |
| `crates/wardd/tests/extensao.rs` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado (id) |
| `dev/daemon.sh` | alterado (repassa `WARD_DEV_EXTENSAO`) |
| `dev/testes/nm.py` | alterado (extensão de teste com a `key`) |
| `dev/testes/sem-modo-teste.sh` | alterado |
| `Makefile` | alterado (`extensao`, `teste-extensao`) |
| `CLAUDE.md` | alterado |

Screenshots (`docs/mission/evidence/fase-41/`):
- `figcraft-17332-sugestoes-do-dominio.png` ↔ `sugestoes-do-dominio-localhost-8081.png`, 360×494, primeiro item em hover como no frame.
- `figcraft-17392-cofre-travado.png` ↔ `cofre-travado-localhost-8081.png`, 360×343.
- `sugestoes-do-dominio-localhost-8083.png`: `acme-totp-dev`.
- `sugestoes-do-dominio-sem-entrada-127.png`: V6.2.
- `sugestoes-do-dominio-sem-campo.png`: aviso do campo escondido.

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados. O Chrome rodou só com perfil e HOME em `.dev/extensao.*`. Nada foi instalado no Chrome do usuário, em `~/.config/google-chrome` ou em `/etc`, e nenhum `sudo` rodou.
- [x] Teste de ataque para cada (SEG) tocado: V6.2, V6.4, V6.5, V6.6, S1, S13 e a exceção de teste.
- [x] (UI) Conferido contra o FigCraft: nodes 17332 e 17392 medidos no teste e screenshots 1:1. O arquivo do FigCraft não foi alterado.
- [x] Ficou dentro do escopo da fase. Fora dela: o estado travado (necessário para o fluxo, medido contra o 17392) e o id fixo no `ward-nm` (pendência da 40).
