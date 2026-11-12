# Fase 44e — App alinhado à 44d, loopback em qualquer esquema e baixos da 2ª auditoria do M6

## Implementado
- **Janela de aprovação alinhada à 44d** (`app/src/telas/JanelaAprovacao.svelte`,
  `aprovacao.js`):
  - O aviso da origem agora usa `Destaque` com `tipo="aviso"` (frames `22812` e `22957`).
  - `prodDestaque` na confirmação é `prodDestaqueOrigem`: "Credencial de produção. Confira a
    origem com atenção antes de confirmar.".
  - O `resumo` do pedido com PIN e origem é `resumoOrigem`: "Confira a entrada, a origem e
    quem pediu antes de permitir.".
  - `avisoDaOrigem(origem)`: em `http`, mostra o texto do frame. Em `https` de loopback
    mostra `origemDestaqueLocal`, o mesmo texto sem o "http" (esse caso não tem frame). O
    esquema é lido com `new URL(...).protocol`, porque o `app-dist.py` recusa um
    `'http://'` literal no bundle.
- **Daemon falso:** `na_origem` (`app/nucleo/src/daemon_falso.rs`) busca a entrada no cofre
  de teste e usa o ambiente e a origem reais dela. `acme-wp-dev` sai como dev e
  `acme-wp-prod` como prod. Teste novo: `pedidos_com_origem_usam_a_entrada_de_verdade`.
- **Conferência pela ponte:** as três telas foram abertas pela ponte de teste
  (`--daemon-falso aprovacao-origem|-popup|-pin`, `&tela=aprovacao`) no Chrome headless,
  com o Playwright do Python. Foram comparadas com os números de `get_node` de `22733`,
  `22817` e `22882`. Nenhuma diferença:
  - largura 560; alturas 772, 790 e 874;
  - y e altura de cada bloco;
  - textos;
  - cores do aviso (`#3F2A06`, borda `#F5B544` a 32%, ícone âmbar);
  - borda do pedido (prod `#E5484D`, dev `#343A46`);
  - selos (`login`+`prod` e `login`+`dev`), botões e foco em "Negar".

  PNG e `medidas.json` estão em `docs/mission/evidence/fase-44e/`.
- **Médio (2ª auditoria), todo host de loopback pede confirmação em qualquer esquema:**
  - `ward_core::entrada::origem_local_da_url` devolve a origem quando o host é desta
    máquina:
    - `localhost` e `*.localhost`;
    - IPv4 `127/8` e `0/8`;
    - `[::1]` e `[::]`;
    - `[::ffff:<IPv4 local>]`;
    - `[::127.x.x.x]`.
  - `uso::origem_http` virou `origem_a_confirmar` e usa essa função. Isso vale para o
    `login`, o `playwright` e o `preencher` do popup (`nucleo.rs`).
  - Extensão: a nova `pedeJanela(origem)` em `extensao/pagina.js` não é injetada. O popup e
    o service worker a usam para escolher o caminho. Vai pelo service worker toda origem
    que seja:
    - `http`;
    - `https` de `localhost`/`*.localhost`;
    - IPv4 `127.*`/`0.*`;
    - IPv6 literal.

    A regra é mais larga que a do daemon de propósito: o caminho do service worker serve a
    qualquer origem. O texto "aguardando" do popup ficou "Endereço desta máquina: …".
  - D9 (PRD), ameaça 4 (escopo) e V6.17 do contrato foram atualizados.
- **Baixo, `apagar_sessoes`:** a pasta é aberta com `O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC`. O
  conteúdo é listado e apagado pelo fd, com a nova
  `ward_plataforma::pasta::apagar_conteudo`:
  - a listagem é `readdir` numa cópia `F_DUPFD_CLOEXEC` com `fdopendir`;
  - a remoção é `unlinkat`;
  - subpastas são abertas por `openat` com `O_DIRECTORY|O_NOFOLLOW`, até 16 níveis de
    profundidade;
  - o `unsafe` fica só em `ward-plataforma/src/sys.rs`, com `// SAFETY:`.
- **Baixo, `teste-politica-chrome`:** a nova rodada 2b lê `chrome://policy` do Chrome em
  execução pelo CDP do pipe (cenário `pagina` em `politica_chrome.py`).
  - Com a política, `NativeMessagingUserLevelHosts`, `PasswordManagerEnabled` e
    `PasswordLeakDetectionEnabled` aparecem `false`, obrigatórias, da máquina e OK.
  - Sem a política (controle), aparecem "Not set.".
  - V9.5 foi atualizado.
- **Baixo, `c44_popup_*` com o falso `[::1]`:** no `teste-extensao`, antes de qualquer
  visita a `localhost:8081`, o `falso_v6.py` passa a escutar em `[::1]:8081`.
  - A aba cai nele.
  - O popup é negado na janela, e nada é preenchido.
  - O pedido mostra a origem.
  - O falso recebe 0 bytes.
  - V6.18 foi atualizado.

## Segurança
- **Invariantes tocadas:**
  - **S1:** a senha só vai à origem certa. Numa origem local, em qualquer esquema, ela só
    sai depois da confirmação na janela.
  - **S9:** a decisão continua só na janela.
  - **S10:** o pedido da janela não leva segredo; os testes conferem a forma codificada.
  - **V9.5:** a política foi conferida no Chrome em execução.
  - **Na UI:** nenhum estado global novo e nenhum segredo na tela. Os textos entram só como
    texto.
- **Testes de ataque escritos:**
  - `crates/ward-core/src/entrada/testes.rs` › `ataque_origem_local_em_qualquer_esquema`
    (formas disfarçadas do loopback, e hosts parecidos que não são locais);
  - `crates/wardd/tests/login.rs` › `ataque_login_https_loopback_livre_so_com_confirmacao`;
  - `crates/wardd/tests/playwright.rs` ›
    `ataque_playwright_https_loopback_livre_so_com_confirmacao` (o Chrome nem abre);
  - `crates/wardd/tests/extensao.rs` ›
    `ataque_preencher_https_loopback_sem_confirmar_nada_sai`;
  - `crates/ward-plataforma/src/pasta.rs` › `ataque_pasta_trocada_por_link_depois_de_aberta`
    e `apaga_o_conteudo_pelo_fd_sem_seguir_link`;
  - `crates/wardd/src/playwright.rs` › `ataque_apagar_sessoes_nao_segue_link` (já existia
    e passa com a versão nova);
  - `dev/testes/extensao.py`: `c44_popup_v6_aba_no_falso`, `c44_popup_v6_negado`,
    `c44_popup_v6_pedido` e `c44_popup_v6_falso_vazio`;
  - `dev/testes/politica-chrome.sh`: a rodada 2b, com controle.
- **Dependências novas:** nenhuma. `libc` já estava no `ward-plataforma` e no `wardd`.

## Ficou de fora
- **`make teste-sem-modo-teste`:** não rodou. Nenhuma mudança toca `modo-teste`, e o
  `target/sem-modo-teste` foi apagado depois do `teste-politica-chrome` para liberar disco.
- **`teste-install` e `teste-deb`:** não rodaram. A política e o instalador não mudaram;
  a mudança está só no teste em execução do Chrome.
- **Frame para o aviso de `https` de loopback** (`origemDestaqueLocal`): não existe. O texto
  é o do frame sem o "http". Se precisar, é uma fase de DESIGN.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core -p ward-plataforma --lib` (1ª vez falhou: `[::2]` lido como local; corrigido) | 101 → 0 |
| `cargo test -p wardd --all-features --test login --test extensao --test playwright` | 0 |
| `make app-front` (1ª vez 2: `'http://'` literal no bundle; trocado por `URL.protocol`) | 2 → 0 |
| conferência das três telas pela ponte (Playwright headless + Chrome, script no scratchpad) | 0 |
| `make fmt` | 0 |
| `make check` | 0 |
| `make dev-servicos-subir` / `make dev-servicos-parar` | 0 / 0 |
| `make extensao` | 0 |
| `make teste-extensao` | 0 |
| `make teste-login` | 0 |
| `make teste-playwright PW_PYTHONPATH=.dev/pw-lib/site` | 0 |
| `make teste-politica-chrome` | 0 |
| `make app-check` | 0 |
| `grep -F -f .dev/variacoes.txt` nas evidências da 44e | 1 (nada) |
| `git checkout` das evidências das fases 41, 42 e 43 regravadas pelas suítes | 0 |

## Problemas descobertos
- **`prodDestaque` no pedido com PIN e origem:** numa entrada prod em "com aprovação" com
  origem local, o destaque ainda diz "confira o comando com atenção", e esse pedido não tem
  comando. Não há frame para esse caso: os frames da 44d têm prod só na confirmação e dev no
  PIN. Não mudei o texto.
- **Rótulo do nível na confirmação:** continua sem decisão (pergunta da 44d). A tela mostra
  "confirmar a origem", e o nível da entrada é "livre".
- **DNS para loopback:** um domínio público que resolve para `127.0.0.1` (do tipo
  `localtest.me`) não é reconhecido como local, porque o parser não consulta DNS. Ficou
  escrito em D9 e na ameaça 4.
- **`pedeJanela` e o IPv6:** a extensão manda todo IPv6 literal pelo service worker, mesmo o
  que não é local. Isso é inofensivo: o daemon decide, e o service worker só espera mais.
- **Chave `c44_popup_v6_*`:** essas chaves ficam no `qa.json` da fase 41, que foi restaurado.
  A evidência dessas chaves está em `docs/mission/evidence/fase-44e/teste-extensao-v6.txt`.
- **Playwright do Python:** o navegador empacotado (headless shell 1254) não está instalado.
  A conferência usou o Chrome de `/opt/google/chrome`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src/telas/JanelaAprovacao.svelte` | alterado |
| `app/src/telas/aprovacao.js` | alterado |
| `crates/ward-core/src/entrada.rs` | alterado |
| `crates/ward-core/src/entrada/testes.rs` | alterado |
| `crates/ward-core/src/entrada/validacao.rs` | alterado |
| `crates/ward-plataforma/src/pasta.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/playwright.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/tests/extensao.rs` | alterado |
| `crates/wardd/tests/login.rs` | alterado |
| `crates/wardd/tests/playwright.rs` | alterado |
| `dev/testes/extensao.py` | alterado |
| `dev/testes/politica-chrome.sh` | alterado |
| `dev/testes/politica_chrome.py` | alterado |
| `docs/escopo.md` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/validation-contract.md` | alterado |
| `extensao/pagina.js` | alterado |
| `extensao/popup.js` | alterado |
| `extensao/servico.js` | alterado |
| `docs/mission/evidence/fase-44e/` (3 PNG, `medidas.json`, `teste-extensao-v6.txt`, `teste-politica-chrome-2b.txt`) | criado |
| `docs/mission/handoffs/phase-44e.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (`22733`/`22817`/`22882`, pela ponte; o `Ward.figcraft` não foi alterado, sha256 `3a2afbfb…`)
- [x] Ficou dentro do escopo da fase
