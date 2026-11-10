# Fase 42 — (UI) Popup "Salvar login novo" e "Cofre travado"

## Implementado
- **Daemon (`salvar_login`, V6.8).** `crates/wardd/src/nucleo.rs`: a operação, que até aqui respondia `interno`, agora grava a entrada.
  - Só o papel extensão (a tabela já limitava; o núcleo confere de novo).
  - Só entrada nova, pelo `salvar_entrada(None, …)`: o nome repetido é erro de validação, então a extensão nunca troca a senha nem o nível de uma entrada existente.
  - Só os campos do popup: URL, usuário e senha. TOTP, extras, anexos, projeto e tags são recusados com `pedido_invalido`. Nível no quadro já é recusado pela leitura.
  - O nível é sempre "com aprovação" (D50, S9), com as datas do daemon. O log não leva nome nem URL.
- **Extensão** (`extensao/`):
  - **`envio.js`** (content script novo): mundo isolado, só no quadro principal, http/https, `document_start`.
    - Conta só o `submit` confiável com ativação do usuário em curso (`navigator.userActivation.isActive`).
    - Pega a senha visível preenchida; se há senhas diferentes (troca de senha), não oferece nada. O usuário é o campo de texto visível antes da senha, de preferência com `autocomplete=username`.
    - Manda `{t:'enviado', usuario, senha}` ao service worker, sem resposta.
    - Acessa o DOM pelos protótipos (contra DOM clobbering, inclusive o `addEventListener`). Não escuta `message` e não preenche nada.
  - **`servico.js`** (service worker novo, módulo):
    - Confere quem mandou cada mensagem pelo `sender` do Chrome. O envio só vale do content script do quadro 0, com `sender.origin` igual à origem do `sender.url` http/https. Pedido, salvar e descartar só valem do `popup.html`, sem aba.
    - No envio, pede `sugerir` ao daemon. Se o usuário já está entre as sugestões da origem, se o cofre está travado ou se não há resposta, descarta.
    - Senão guarda o pendente **só em memória** (um por aba, some em 3 min, ao fechar a aba, no "Agora não" ou ao salvar) e abre o popup com `chrome.action.openPopup`.
    - O popup recebe só `{origem, usuario}`. No "Salvar", o service worker junta a senha (base64 dos bytes UTF-8) e manda `salvar_login`. A URL da entrada é a origem mais o caminho do documento, sem consulta nem fragmento.
  - **`popup.js` e `popup.css`:**
    - Novo estado `Extensão / Salvar login novo` (17414/18850), pedido antes das sugestões.
    - Os campos:
      - Nome, Usuário e Grupo (texto);
      - Senha com máscara fixa de 12 pontos (nem o valor nem o tamanho chegam ao popup) e a ajuda "Vai direto para o cofre…";
      - Tipo (só `login`) e Ambiente (`dev`/`stage`/`prod`) lado a lado;
      - o cartão "Nível inicial: com aprovação";
      - o rodapé com "Agora não" e "Salvar no Ward".
    - Nome, grupo e ambiente vêm sugeridos do host. `localhost:8083` dá `localhost-8083`/`localhost`/`dev`, e `painel.stage.clientex.test` daria `clientex-stage-painel`/`clientex`/`stage`. Sem pista no host, o ambiente sugerido é `prod`.
    - Erro de validação do daemon aparece no campo (borda e texto de perigo). `travado` leva ao estado travado.
    - O cabeçalho e o rodapé ficam presos (`sticky`) e o corpo rola, como no 17414. Sem rolagem, o layout é igual ao 18850.
    - Enquanto o formulário está aberto, o popup manda `manter` a cada 20 s para o service worker não ser encerrado.
  - **Manifesto:** `background` (service worker `servico.js`) e `content_scripts` (`envio.js`). As permissões continuam só `nativeMessaging`, `activeTab` e `scripting`.
- **V6.9.** O estado `Cofre travado` (17392) já existia na 41. Agora o teste confere:
  - com `prefers-color-scheme: light`, os três estados (sugestões, salvar e travado) ficam com as cores escuras;
  - o botão pede `abrir_app` ao daemon e o popup fecha.
- **`dev/extensao/montar.py`** (recusa o pacote):
  - `background` diferente de `{service_worker: servico.js, type: module}`;
  - `content_scripts` diferente do exato (`envio.js`, `http://*/*` e `https://*/*`, quadro principal, `ISOLATED`, sem CSS);
  - `runtime.onMessage` fora do `servico.js`;
  - no `envio.js`: `sendNativeMessage`, `executeScript`, `onMessage`, `sendResponse` ou `.then(r…`;
  - `sendMessage`/`onMessage` no `pagina.js`;
  - `onConnect`, `prefers-color-scheme` e `color-scheme: light` em qualquer arquivo.

  No manifesto, só os dois padrões do `content_scripts` passam pela regra de URL remota.
- **`make teste-extensao`** passou a compilar o `ward` de teste e ganhou a seção da Fase 42 (`dev/testes/extensao.py`, `secao_fase_42`). Ao todo são 93 conferências "ok".
- **Evidência em `docs/mission/evidence/fase-42/`:** `qa.json`, os PNGs do Chrome (salvar, rolado, erro de nome, tema claro do salvar e do travado) e os exports do FigCraft 17414 e 18850.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1:** a senha do login novo vai da página ao service worker e do service worker ao daemon. Nunca vai ao popup nem à página. O content script não recebe nada.
  - **S2/S13:** nada é gravado. Não há `storage`; o pendente fica só em memória e por pouco tempo. No perfil temporário, nada da extensão (inclusive "Service Worker") tem a senha nova nem padrão de `.dev/variacoes.txt`, e os logs do daemon, do Chrome e o console também não.
  - **S9:** a entrada nova nasce "com aprovação", e a extensão não substitui entrada existente.
  - **S11:** CSP e regras do pacote sem mudança, mais as novas conferências do `montar.py`.
  - **D1/V6.9:** só tema escuro.
  - **D9:** a origem sai do `sender`, nunca do conteúdo da mensagem.
  - **V6.6:** a página continua sem canal; o content script não escuta `message`.
- **Testes de ataque escritos**
  - `crates/wardd/tests/extensao.rs`:
    - `ataque_salvar_login_nao_sobrescreve_entrada`: acme-wp-dev/prod mantêm senha e nível.
    - `ataque_salvar_login_so_os_campos_do_popup`: nível, projeto, tags, extras, anexos, TOTP e outro tipo são recusados sem gravar.
    - `ataque_salvar_login_so_extensao_e_destravado`: agente e app recusados nas duas camadas; travado não grava.
    - `v6_8_salvar_login_nasce_com_aprovacao`: o caminho feliz, que sobrevive a travar e destravar.
  - `dev/testes/extensao.py` (`make teste-extensao`):
    - `requestSubmit` do script da página sem ativação: nada é oferecido nem fica pendente.
    - Login já cadastrado (`acme-totp-dev`) enviado pelo usuário: nada.
    - Envio dentro de um iframe de outra origem: nada.
    - Cofre travado no envio: nada, nem depois de destravar.
    - Mensagens `pendente`/`salvar`/`descartar` mandadas do mundo isolado do content script na aba: ficam sem resposta e o pendente continua.
    - O DOM do popup e a resposta `pendente` sem a senha.
    - O perfil sem a senha nova.
    - O tema claro não muda nada.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Atualizar a senha de um login que já existe** (mesmo usuário, senha nova): a extensão não oferece nada. Trocar senha é só pelo app, e o `salvar_login` só cria.
- **Oferecer salvar com o cofre travado** (guardar o envio até destravar): descartado na hora, por segurança. Decisão do planner (ver Problemas).
- **`ward login` (envio e limpeza pelo daemon):** Fase 43.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p wardd --test extensao` (9 testes) | 0 |
| `make extensao` | 0 |
| `make dev-servicos-subir` | 0 |
| `make teste-extensao` (1ª: `posts.log` do phishing ausente no serviço recém-subido; corrigido; depois só ajustes de captura) | 2 → 0 |
| `make teste-extensao` (final, 93 ok) | 0 |
| `RUST_TEST_THREADS=2 make check` ("make check: tudo limpo") | 0 |

## Problemas descobertos
- **Arquitetura da extensão mudou (precisa virar decisão D-nova no PRD).** Oferecer salvar no envio exige código na página na hora do `submit`. Por isso a extensão agora tem:
  - um content script em toda página http/https, o que dá ao Ward acesso de host a todos os sites; instalada por política, sem aviso ao usuário;
  - um service worker com `runtime.onMessage`.

  Isso contraria o desenho mínimo da 41 ("sem content script fixo nem ouvinte"). A Fase 43 (`ward login`, daemon → extensão sem clique) precisaria disso de qualquer forma. As travas estão descritas em Implementado e conferidas no build e no teste.
- **Resíduo aceito, a registrar.** Um script da página que chame `requestSubmit()` dentro de um clique do usuário (em qualquer ponto da página) com valores escolhidos por ele faz a extensão oferecer salvar essas credenciais **para a própria origem da página**. O usuário precisa confirmar, e a entrada nasce "com aprovação". O impacto é ruído no cofre, sem vazamento.
- **V6.8 contra D50.** O V6.8 diz "aparece no `ward lista` com nível 'com aprovação'", mas a `lista` do agente não leva nível (D50). O teste confere três coisas:
  - a entrada no `ward lista --json`, sem nível;
  - o nível pelo uso: o `login` do agente responde `aprovacao_indisponivel`, porque abriria a janela; uma entrada livre seguiria sem perguntar;
  - o nível na `lista` do app, no teste do daemon.

  O texto do contrato deveria dizer isso.
- **Design (FigCraft):**
  - O nome de exemplo "clientex / stage / painel" (17430) viola o formato de nome (só minúsculas, números, `.`, `_`, `-`). Trocar por `clientex-stage-painel`.
  - O "Grupo" é `Select` no design, mas a extensão não pode listar grupos: `lista` é só de agente e app, e dar o inventário do cofre à extensão seria vazamento. Ficou campo de texto sem chevron. Ajustar o frame.
  - Não há frame para o erro de validação no campo (fiz com os tokens de perigo) nem para a nota geral de erro.
- **Cofre travado no envio:** o login é descartado. Se o planner preferir oferecer depois do destravar, o pendente ficaria em memória enquanto o cofre está travado. Não recomendo.
- **Senha em memória no service worker:** fica até 3 min, e strings JS não se zeram (o mesmo limite da 41). O popup nunca a tem.
- **Janela do popup no headless:** nasce com 510 de altura e o CDP não a redimensiona (`Emulation` e `Browser.setWindowBounds` não valem para o popup). O teste confere:
  - o 17414 pela regra "rodapé preso embaixo da janela": em 600, o rodapé fica em y 523;
  - o 18850 tirando o `sticky` pelo CSSOM: rodapé em y 729, documento com 806.

  A captura "além da janela" do popup repete o topo; por isso a evidência do resto é a janela rolada até o fim.
- **`chrome.action.openPopup` sem gesto** funcionou no Chrome 156 (headless). O `minimum_chrome_version` continua 120. Sem a API, o pendente aparece quando o usuário clica no ícone.
- **Bug da 41 corrigido no teste:** com o serviço `phish` recém-subido, o `/registro/posts.log` não existe e o teste falhava. Agora isso conta como vazio.
- **`docs/mission/evidence/fase-41/qa.json`** foi regravado pelo mesmo teste. Só mudou o nome do arquivo de Code Cache do Chrome (o falso positivo de dicionário já anotado na 41).
- **`CLAUDE.md`:** a linha do `make teste-extensao` cita só a Fase 41. Vale acrescentar a 42 (V6.8, V6.9) e que o alvo agora também compila o `ward` de teste. Não editei.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/nucleo.rs` | alterado (`salvar_login`) |
| `crates/wardd/tests/extensao.rs` | alterado (4 testes novos) |
| `extensao/envio.js` | criado |
| `extensao/servico.js` | criado |
| `extensao/manifest.json` | alterado |
| `extensao/popup.js` | alterado |
| `extensao/popup.css` | alterado |
| `dev/extensao/montar.py` | alterado |
| `dev/testes/extensao.py` | alterado |
| `dev/testes/extensao.sh` | alterado (4º argumento: `ward`) |
| `Makefile` | alterado (`teste-extensao` compila o `ward` de teste) |
| `docs/mission/evidence/fase-42/*` | criado |
| `docs/mission/evidence/fase-41/qa.json` | alterado (regravado) |
| `docs/mission/handoffs/phase-42.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados: cópia do cofre em `.dev/extensao.*`, Chrome com perfil temporário e senhas novas geradas na hora.
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI) Conferido contra o FigCraft: 17414, 18850 e 17392 por medidas e cores no Chrome. O arquivo do FigCraft não foi alterado.
- [x] Ficou dentro do escopo da fase. A correção do `posts.log` foi feita no teste que esta fase estende.
