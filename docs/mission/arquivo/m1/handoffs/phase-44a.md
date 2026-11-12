# Fase 44a — Correções do scrutiny do M6

## Implementado
- `dev/testes/playwright.py`: `posts_log()` devolve `b""` quando o phish ainda não criou o
  `/registro/posts.log` ("No such file or directory"), como o `posts_phish()` de
  `dev/testes/login.py`. Outro erro (contêiner fora) continua `None` e falha. Com os
  serviços recém-subidos (`/registro/` vazio, conferido) `make teste-playwright
  SEM_PLAYWRIGHT_PYTHON=1` deu 0.
- `crates/wardd/src/playwright.rs`, `PREENCHER` e `LER_PAGINA`: nada é lido pelo objeto.
  A busca usa `Document.prototype.querySelectorAll.call(document, …)` e
  `Array.prototype.slice`. Getters e métodos saem dos protótipos: `HTMLInputElement`
  (`type`, `form`, `autocomplete`, `disabled`, `readOnly`, `value`), `HTMLButtonElement`
  (`type`, `disabled`), `HTMLFormElement`, `Element` (`getClientRects`, `hasAttribute`),
  `HTMLElement.focus` e `EventTarget.dispatchEvent`. O `localStorage` é lido por
  `Storage.prototype`.
- `dev/testes/extensao.py`: o achado em `Default/Code Cache/js/…` foi investigado e
  agora é filtrado com justificativa:
  - Origem: é o `wp-includes/js/zxcvbn.min.js` do WordPress de teste (8081). O medidor de
    força de senha traz uma lista de senhas comuns em rot13, e o Chrome guarda o código
    compilado no Code Cache. O padrão casado é a linha 81 de `.dev/variacoes.txt`, que é
    o PIN de teste (6 dígitos, palavra da lista). Não vem do cofre.
  - Critério: o arquivo é do `ZxcvbnData/` ou do `Code Cache/` com `zxcvbn.min.js` dentro.
    Todo padrão achado tem de ser palavra do dicionário, lido do próprio
    `zxcvbn.min.js` do contêiner (`compose.sh exec … cat`) e do `ZxcvbnData/` do perfil,
    em claro e em rot13. Cada ocorrência tem de estar cercada de palavras do dicionário
    (pelo menos 5 tokens do meio de uma janela de ±60 bytes).
  - Saída: o achado vai para `perfil_dicionario_zxcvbn` no `qa.json`, com a linha, o
    tamanho e "só dígitos", nunca o valor. Fica uma linha `info` no log. O resto continua
    em `perfil_com_padrao_fora_da_extensao`.
- `docs/mission/evidence/M6/roteiro-usuario.md` (novo): V6.11 (UI) para o usuário.
  Inclui:
  - o daemon de teste com o app nativo e `WARD_DEV_EXTENSAO=chrome`;
  - um Chrome visível com HOME e perfil em `.dev/roteiro-m6/`, o manifesto de native
    messaging para o `ward-nm` de teste e a extensão carregada sem compactação (id
    conferido);
  - `ward login acme-wp-dev`: negar pelo botão, por Esc e pelo X deixa os campos
    vazios; permitir com o PIN chega ao `/wp-admin/`; PIN errado não permite;
  - o log do daemon sem padrão e a limpeza no fim.

## Segurança
- Invariantes tocadas:
  - D9/S1 (V6.15): o preenchimento do `ward playwright` não pode ser desviado nem
    derrubado por DOM clobbering.
  - V6.12: o `posts.log` ausente não pode mais virar um falso FALHA nem esconder a
    conferência. Ele vale `b""`, e "não mudou" continua exigido.
  - V6.5/S13: o filtro do dicionário não pode esconder vazamento.
- Medição (Chrome 156.0.8078.79, CDP no pipe, script descartável no scratchpad):
  - no mundo isolado, `typeof document.querySelectorAll` dá `function` mesmo com
    `<img name="querySelectorAll">` (o Chrome de hoje não leva as propriedades com nome
    do `document` ao mundo isolado);
  - as do `<form>` (`form.action` com `<input name="action">`) e as do `window`, sim;
  - no mundo principal, as duas são trocadas.
  - Consequência: o código antigo ainda não era explorável pelo `document` neste Chrome.
    A troca é defesa em profundidade e não depende desse detalhe do Blink. O comentário
    do `PREENCHER` registra isso.
- Testes de ataque escritos:
  - `crates/wardd/src/playwright.rs` › `testes::ataque_dom_clobbering_scripts_so_pelos_prototipos`
    (no `cargo test`): `PREENCHER` e `LER_PAGINA` sem `document.`, `form.`,
    `localStorage.`, `pw.`, `botao.`, `.elements` nem leitura direta de propriedade de
    campo, e com `Document.prototype.querySelectorAll.call(document`. Falha no código
    antigo.
  - `crates/wardd/tests/playwright.rs` › `chrome_real_login_e_ataques` (`--ignored`,
    Chrome real) ganhou dois casos:
    - `dom`: `<img name="querySelectorAll">` e `<img name="getElementsByTagName">` no
      `document`, mais campos `elements`, `method=get`, `target=_blank`,
      `requestSubmit` e `querySelectorAll` dentro do `<form>`. O login certo entra, a
      sessão é gravada sem a senha e o phishing fica sem `POST`.
    - `dom-recusa`: o site recusa e devolve o login com o mesmo clobbering no
      `document`. A senha visível é vista depois do envio (`LoginRecusado`) e nada é
      gravado.
    - A contagem de `POST` ao site foi de 4 para 6.
    - Esses dois casos também passam com o código antigo neste Chrome (ver a medição);
      quem pega a regressão é o teste estático.
  - `dev/testes/extensao.py` › `f44a_filtro_dicionario_autoteste` (6 casos). Nenhum
    destes passa como dicionário:
    - segredo que não é palavra plantado no meio da lista num Code Cache com
      `zxcvbn.min.js`;
    - o mesmo em `Default/History`;
    - o PIN em Code Cache sem `zxcvbn.min.js`;
    - o PIN cercado de lixo;
    - o PIN em `Default/Preferences`.
    - E o PIN no meio da lista do zxcvbn passa.
  - `dev/testes/extensao.py` › `v65_dicionario_fora_da_extensao`: nenhum achado
    classificado como dicionário está em arquivo da extensão.
- Dependências novas: nenhuma. O `extensao.py` usa `codecs` e `re`, da biblioteca padrão.

## Ficou de fora
- A execução do roteiro do V6.11 é do usuário (janela nativa na sessão COSMIC).
- O V6.12 com o Playwright do Python continua sem rodar: ele não está no host. Rodei com
  `SEM_PLAYWRIGHT_PYTHON=1`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p wardd --all-features --test playwright -- --ignored` (código novo) | 0 |
| o mesmo com o `playwright.rs` antigo (contraprova; `dom` passa também, ver Segurança) | 0 |
| `cargo test -p wardd --all-features --lib playwright` | 0 |
| `make dev-cofre`, `make dev-variacoes`, `make dev-servicos-subir`, `make app-front` | 0 |
| `make teste-playwright SEM_PLAYWRIGHT_PYTHON=1` (serviços recém-subidos, sem `posts.log`) | 0 |
| `make teste-extensao` (3 rodadas, a última com o autoteste) | 0 |
| `make ferramentas` (gitleaks faltava no worktree) | 0 |
| `make check` (1ª: parou no 6/6 sem gitleaks; fmt, clippy, testes, audit, deny ok) | 2 |
| `make check` (2ª, com gitleaks: "tudo limpo") | 0 |

## Problemas descobertos
- O comentário de `extensao/pagina.js` (linhas 9–13) diz que `document.querySelectorAll`
  "pode" ser trocado no mundo isolado. No Chrome 156 o `document` não é afetado; o
  `<form>` e o `window`, sim. A extensão já usa os protótipos, então nada muda; vale só
  ajustar o texto.
- Os casos `dom`/`dom-recusa` do teste com o Chrome real não distinguem o código novo
  do antigo neste Chrome; quem segura a regressão é o teste estático.
- `make teste-extensao` regrava os PNGs da `fase-42`. Restaurei com `git checkout`. O
  `fase-41/qa.json` ficou com as chaves novas desta fase (`perfil_dicionario_zxcvbn`,
  `f44a_filtro_dicionario_autoteste`, `v65_dicionario_fora_da_extensao`) e com
  `perfil_com_padrao_fora_da_extensao` vazio.
- O `make app-front` avisa do `npm audit` ("Some issues need review"), mas sai 0. Isso é
  anterior e fica fora desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/playwright.rs` | alterado |
| `crates/wardd/tests/playwright.rs` | alterado |
| `dev/testes/playwright.py` | alterado |
| `dev/testes/extensao.py` | alterado |
| `docs/mission/evidence/fase-41/qa.json` | alterado (regravado pela suíte) |
| `docs/mission/evidence/M6/roteiro-usuario.md` | criado |
| `docs/mission/handoffs/phase-44a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
