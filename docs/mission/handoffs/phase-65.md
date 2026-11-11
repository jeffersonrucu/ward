# Fase 65 — Bateria E (navegador e app) (UI)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard
  f70171a` (master atual: "fase 64 concluída; bateria D verde").
- **Preparação** (`fase65-prep.log`):
  - `.dev/tools` copiado do checkout principal e conferido pelo `make tools`;
  - `.dev/baseline/` copiado, com `sha256sum -c SHA256SUMS` ok em todos os arquivos;
  - `.dev/extension/key.pem` copiado (0600) e o Playwright do Python copiado para
    `.dev/pw-lib/site`;
  - `make dev-services`, `make dev-vault` (22 entradas) e `make dev-variations` (279
    padrões).
- **Nenhum código corrigido**. A fase só rodou e registrou.
- **Evidência**: `docs/mission/evidence/M6/fase-65/`, com logs `fase65-*.log`, as telas do
  `test-app-e2e` em `test-app-e2e/` e os roteiros em `scripts/`. Os caminhos do home, do
  worktree e do `/run/user/<uid>`, e o nome do usuário, viraram `<home>`, `<repo>`, `<rt>` e
  `<user>` nos logs.

### Resultado por suíte

| Suíte | Resultado | Log |
| --- | --- | --- |
| `make app-front` | 0. `npm audit --audit-level=moderate` acusa 2 avisos *low* (`deepmerge`, por `@sveltejs/vite-plugin-svelte`, sem correção) e não falha | `fase65-app-front.log` |
| `make extension` | 0 (id `nhkehjnmlgkebhmelajfpmpblepaichf`) | `fase65-extension.log` |
| `make test-nm` | **2**. A parte sem Chrome passa (20 ok), inclusive m2:V5.10 direto: `estado`/`preencher`/`sugerir` v1 recebem só `unknown_version`, sem `password`, `senha`, `username`, `usuario` nem `totp`. A parte com Chrome falha no ambiente (Problema 1) | `fase65-test-nm.log` |
| `make test-nm` num bwrap com `/etc/opt` em tmpfs (tentativa de isolar a política) | 2: o `wardd` de teste recusa `/run/user` (dono `nobody` no namespace). Não serve, e parei aqui | `fase65-test-nm-bwrap.log` |
| `make test-extension`, `make test-login` | **não rodados**, por segurança (Problema 1) | — |
| `make test-playwright PW_PYTHONPATH=…` | 0 (37 ok + `cargo test --ignored` 1/1): V6.12 (`/wp-admin/` logado pelo storage state), V6.13, V6.14, V6.15 (`delta-redir` recusado, `posts.log` igual) e V6.16 | `fase65-test-playwright.log` |
| `make test-chrome-policy` | 0. Rodadas 1, 2, 2b e 4 ok. A 3 (controle sem política, fora do bwrap) foi **pulada**: o host já tem `/etc/opt/chrome` | `fase65-test-chrome-policy.log` |
| `make test-webkit-real` | 0 (V5.32 ok; o V6.4 aceita 0 ou 2) | `fase65-test-webkit-real.log` |
| `make test-gst-channels` | 0 (controles enxergam o ataque, e com as variáveis de D57 nada carrega) | `fase65-test-gst-channels.log` |
| `make test-clipboard` | 0 (13 ok). V4.24: o valor sai depois do prazo, o texto do usuário fica, a trava limpa na hora, sem `wl-copy` e sem arquivo com o valor | `fase65-test-clipboard.log` |
| `make test-revealed-selection` | 0. Ctrl+A, Ctrl+C, arrasto, clique triplo e seleção que atravessa o campo não levam o valor à PRIMARY nem à CLIPBOARD, e "Ocultar" o tira do documento. Logs limpos | `fase65-test-revealed-selection.log` |
| `make test-app-e2e PW_PYTHONPATH=…` | **2**. 9 de 10 ok, e "Permitir uma vez" fica desabilitado (Problema 2). Rodei de novo com `< /dev/null`: mesmo resultado | `fase65-test-app-e2e.log`, `fase65-test-app-e2e-devnull.log`, `test-app-e2e/` |
| `make test-app-logs` | 0 (autoteste acha o log plantado; `.dev/logs` limpo) | `fase65-test-app-logs.log` |
| `cargo test -p ward-app-core -- m2_v4_16 v4_16` | 0 (`attack_m2_v4_16_server_of_another_version_closes_before_the_password`) | `fase65-cargo-app-core-v416.log` |
| `cd app/src-tauri && cargo test --all-features --locked` (a parte de testes do `app-check`) | 0 (23 ok, 1 ignorado: a sessão GTK real, que o `test-clipboard` roda). Inclui `attack_old_command_names_and_window_labels_are_denied` (V4.23) e `attack_plugins_and_made_up_command_are_denied` | `fase65-cargo-app-tauri-test.log` |
| `scripts/f65_caps.py` (V4.22, só leitura) | capabilities `main` 32 e `approval` 3, iguais em número às `principal`/`aprovacao` da `v0.1.4`. Nenhum comando liberado sem uso no front-end, nenhum chamado sem liberação. CSP idêntica à da `v0.1.4` | (saída no handoff) |
| `make dev-services-up` / `dev-services-verify` / `dev-services-down` | 0 / 0 / 0 | `fase65-services.log`, `fase65-services-down.log` |

### Resultado por asserção

| Asserção | Resultado |
| --- | --- |
| V2.8 (parte app/`ward-nm`/ponte) | ok onde rodou. O `test-nm` varreu o `wardd.log` e o `chrome.log` (o stderr do `ward-nm`). O `test-revealed-selection`, o `test-app-e2e` e o `test-app-logs` varreram `.dev/logs` (ponte e daemon). Nenhum padrão. Ficam de fora os logs do `ward-nm` aberto pela extensão de verdade (`test-extension`/`test-login`, Problema 1) |
| V3.2 | **parcial**. Só as telas que o `test-app-e2e` captura (abaixo). O roteiro de capturas de todas as telas foi negado (Problema 3) |
| V3.3 | não conferido à mão (Problema 3). O roteiro negado tinha foco inicial, Esc nos filtros, Esc no nível e Esc na aprovação |
| V3.4 | ok pelo `test-revealed-selection`. A varredura do DOM tela a tela ficou no roteiro negado |
| V3.5 | **não rodado** (`test-extension`, Problema 1) |
| V4.16 (app) | ok pelo `ward-app-core`. No app nativo (Tauri) contra o `wardd` 0.1.4 não rodou: precisaria subir o 0.1.4 por um roteiro próprio, como a Fase 64, e a chamada do mesmo tipo (roteiro que sobe daemon de teste) foi negada nesta sessão (Problema 3) |
| V4.22 | ok na parte estática (capabilities, CSP, `app-dist.py` no `app-front`). O `strings` do release não foi refeito (precisa do build de release do Tauri; 6,6 GB livres no disco nessa hora). A Bateria A (Fase 61) já o passou |
| V4.23 | ok (`cargo test` do `app/src-tauri`) |
| V4.24 | ok (`test-clipboard`) |
| V5.10 | **parcial**: ok direto pelo `ward-nm` (pedidos v1 da 0.1.x → só `unknown_version`). As partes pelo Chrome real (v1 e a extensão 0.2.0 preenchendo) não rodaram (Problema 1) |
| V6.4 (parte E) | `test-playwright`, `test-chrome-policy` (rodada 3 pulada) e `test-webkit-real` passam. `test-nm` falha no ambiente. `test-extension` e `test-login` não rodaram |
| V6.5 | ok para `ward playwright delta-redir` (recusa e `posts.log` igual). A mensagem é "a origem da página não bate com a da entrada", a mesma da 0.1.4 (está em `strings-0.1.4.txt`). O "domínio não confere" é do `ward login`, que não rodou |
| V6.6 (`ward login`/`ward playwright`) | `ward playwright acme-wp-prod`: ok (V6.12). `ward login acme-wp-prod`: não rodado (`test-login`, Problema 1) |
| V6.8 | parcial. A janela mostra entrada, ambiente, comando e cadeia de processos, com os 4 botões. Negar e fechar negam, e a CLI sai ≠ 0 sem executar (`test-app-e2e`). Com a entrada padrão em pipe, os botões ficam presos até a confirmação a mais, como manda o D68 (`falha-1.png`). Expirar, 1 hora e sempre não rodaram (o e2e parou antes, Problema 2) |
| V6.9 | não rodado (o roteiro negado; o e2e não chega lá) |
| Marca "fora de ordem" (achado 4 da Fase 64) | não conferida no app (roteiro negado). Pela leitura de `ward_core::backup::out_of_order` (comparação estrita `g < m`): uma cópia de geração igual com nome mais velho não é marcada, porque o conteúdo é o mesmo de uma gravação real. Mas essa cópia faz a versão **verdadeira** mais velha, que vem depois dela pela data, sair marcada como "gravação mais antiga do que a data diz". Ver Problema 5 |

### Capturas comparadas com `docs/mission/arquivo/m1/evidence/final/`

Comparei pixel a pixel (tolerância 8 por canal), com um decodificador de PNG só da
biblioteca padrão (`scratchpad/pngdiff.py`, fora do repositório):

| Fase 65 | Missão 1 | Diferença |
| --- | --- | --- |
| `test-app-e2e/falha-0.png` (credenciais, `acme-pg-aprovacao` aberta) | `credenciais-cheia-site.png` | 4941 px, todos nas linhas 724–765: o botão "Nova credencial" com outro estado de hover/foco (mais claro na m1). Textos, cores, espaçamento e ordem iguais |
| `test-app-e2e/aprovacao-ward-run-real.png` | `aprovacao-normal-site.png` | linhas 0–307: o caminho do comando (`.claude/worktrees/…/e2e-mark` × `.dev/qa-final-marca`), o diretório de trabalho e a cadeia "Quem pediu", que são esperados. Layout, botões, aviso e PIN iguais. A rolagem é a mesma |
| `test-app-e2e/falha-1.png` (pipe com PIN digitado) | `aprovacao-orfao-site.png` | linhas 0–574: o mesmo caminho e a mesma cadeia, mais os 6 pontos do PIN digitado e o foco no último dígito. O resto é igual, e o toggle "Sei o que vai pela entrada padrão" está nos dois |
| `test-app-e2e/formulario-novo-vazio.png` | sem par na m1 (a `nova-credencial-site.png` mostra a escolha de tipo, um passo antes) | a coluna da lista e a barra lateral são iguais às de `credenciais-cheia-site.png` |

Nenhuma diferença de texto, cor, espaçamento ou ordem além do nome/caminho e do estado do
cursor. As demais telas da m1 (destravar, busca, filtros, sem resultado, revelado, nível,
gerador, configurações, aprovação prod/PIN errado/expirando/sem pedido, pós-negar,
extensão) não foram capturadas (Problemas 1 e 3).

PNGs principais:
- `docs/mission/evidence/M6/fase-65/test-app-e2e/falha-0.png`
- `docs/mission/evidence/M6/fase-65/test-app-e2e/aprovacao-ward-run-real.png`
- `docs/mission/evidence/M6/fase-65/test-app-e2e/falha-1.png`
- `docs/mission/evidence/M6/fase-65/test-app-e2e/formulario-novo-vazio.png`
- `docs/mission/evidence/M6/fase-65/test-app-e2e/formulario-novo-erro-no-campo.png`

## Segurança

- **Invariantes tocadas** (só por execução):
  - S1: v1 recebe só `unknown_version`, e `delta-redir` é recusado;
  - S2 e S12: área de transferência e seleção do revelado;
  - S3: argv do Chrome do `ward playwright` sem segredo;
  - S5: V4.16 no `ward-app-core`;
  - S10: negar e fechar negam;
  - S11: ACL, comandos antigos recusados, CSP e capabilities iguais às da 0.1.4;
  - S13: logs varridos.
- **Testes de ataque escritos**: nenhum teste novo no produto. Ficam em
  `docs/mission/evidence/M6/fase-65/scripts/`:
  - `f65_caps.py`: V4.22 estático, só leitura, rodado;
  - `f65-ui.sh` e `f65_ui.py`: as capturas e conferências de V3.2/V3.3/V3.4/V6.8/V6.9 e a
    marca "fora de ordem". **Não rodaram** (a chamada foi negada). A senha e o PIN de
    teste saem de `.dev/f65-password`/`.dev/f65-pin`, nunca do arquivo.
- **Evidência**: `grep -rlF -f .dev/variations.txt docs/mission/evidence/M6/fase-65/` não
  acha nada (exit 1). O `make gitleaks` não rodou (Problema 6).
- **Dependências novas**: nenhuma.
- **Não tocados**: o cofre real, o `/usr/local/bin/wardd` instalado (que ficou no ar sem
  contato), `/usr/local`, `/etc/opt/chrome` (só lido), `~/.ssh`, `~/.aws` e o perfil real do
  Chrome. Os Chromes de teste usaram perfil e HOME em `.dev/` ou nos temporários das suítes.
  Nenhum `sudo`.

## Ficou de fora

- Correções, por regra da fase.
- `test-extension` e `test-login`, e com eles V3.5, V5.10 pelo Chrome, `ward login` do
  V6.6 e V6.5 pelo `ward login`. O motivo está no Problema 1.
- As capturas de todas as telas, V3.3, V6.9 e a marca "fora de ordem" no app (Problema 3).
- V4.16 no app nativo contra o `wardd` 0.1.4 (Problema 3).
- `make app-check` inteiro (release do Tauri e `strings`): pouco disco (6,6 GB livres no
  meio da fase) e já verde na Fase 61. Rodei só os testes do `app/src-tauri`.
- `make check`: a fase não mudou código. Só pôs evidência e roteiros em `docs/`.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard f70171a` | 0 |
| `sha256sum -c SHA256SUMS` (`.dev/baseline/`) | 0 |
| `make tools && make dev-services && make dev-vault && make dev-variations` | 0 |
| `systemd-run --user --scope -p TasksMax=1000 make app-front` | 0 |
| `make extension` | 0 |
| `systemd-run … -p TasksMax=1500 make test-nm` | 2 |
| `systemd-run … bwrap --dev-bind / / --tmpfs /etc/opt --die-with-parent -- make test-nm` | 2 (`/run/user` não confiável no namespace) |
| diagnóstico `scratchpad/f65_load.py` (Chrome headless, `Extensions.loadUnpacked`, perfil em `.dev/f65diag.*`) | 0: sem `key` carrega; com a `key` do Ward dá "is blocked by the administrator" |
| diagnóstico `scratchpad/f65_forced.py` (perfil em `.dev/f65forced.*`, só `about:blank`) | 0: sem `--disable-extensions`, a extensão 0.1.x `nhkehjnm…` é forçada no perfil de teste (service worker `servico.js`); com `--disable-extensions`, não |
| `make dev-services-up && make dev-services-verify` | 0 |
| `systemd-run … make test-playwright PW_PYTHONPATH=<repo>/.dev/pw-lib/site` | 0 |
| `systemd-run … make test-chrome-policy` | 0 |
| `systemd-run … make test-gst-channels` | 0 |
| `systemd-run … make test-webkit-real` | 0 |
| `systemd-run … make test-clipboard` | 0 |
| `systemd-run … make test-revealed-selection WARD_DEV_SOCKET_DIR=/run/user/1000/ward-fase65-sel` | **negado** pela permissão |
| `systemd-run … make test-revealed-selection` (pasta padrão própria, `ward-test-selection`) | 0 |
| `systemd-run … make test-app-e2e PW_PYTHONPATH=…` | 2 |
| o mesmo com `< /dev/null` | 2 |
| `make test-app-logs` | 0 |
| `PW_PYTHONPATH=<repo>/.dev/pw-lib/site systemd-run --user --scope -q -p TasksMax=1500 docs/mission/evidence/M6/fase-65/scripts/f65-ui.sh <repo> <repo>/docs/mission/evidence/M6/fase-65/ui < /dev/null` | **negado** pelo classificador do modo automático ("dangerous", sem explicação); não insisti nem contornei |
| `cargo test --locked -p ward-app-core -- m2_v4_16 v4_16` | 0 |
| `cd app/src-tauri && cargo test --all-features --locked` | 0 |
| `python3 -I -B scripts/f65_caps.py <repo>` | 0 |
| `echo $WAYLAND_DISPLAY …` (conferir a sessão gráfica) | negado; segui sem ele |
| `ls -l /proc/self/fd/0` (tipo da entrada padrão) | negado; o log do daemon de teste respondeu (`entrada padrão pipe`) |
| `make gitleaks` | barrado pela guarda do worktree (o texto "git" no comando) |
| `make dev-services-down` | 0 |
| `rm -rf target app/src-tauri/target app/node_modules .dev/pw-lib .dev/docker` (e as pastas de trabalho das suítes) | 0 |

`df -h` no começo: 17 GB livres. No meio caiu a 6,6 GB (o worktree tinha ~6 GB; o resto
não é desta fase) e fechou em 13 GB depois da limpeza.

## Problemas descobertos

1. **Ambiente: a política do Chrome da 0.1.x instalada no host bloqueia (e torna perigosas)
   as suítes com a extensão.**
   - Comando: `make test-nm`.
   - Saída: `FALHA extensão de teste carregada no Chrome real (perfil temporário)`, depois
     `TimeoutExpired` no `chrome-parent serve` (30 s).
   - O diagnóstico mostra a causa: `Extensions.loadUnpacked` com a `key` do Ward responde
     `extension ID "nhkehjnmlgkebhmelajfpmpblepaichf" is blocked by the administrator`.
   - Causa: `/etc/opt/chrome/policies/managed/ward.json`, instalado pelo usuário em
     10/11 (0.1.x). Ele tem `ExtensionSettings` com `force_installed` desse id (update
     `file:///usr/local/share/ward/extensao/update.xml`) e
     `NativeMessagingUserLevelHosts: false`.
   - Efeitos:
     - (a) nenhuma extensão de teste com o id do Ward carrega;
     - (b) o manifesto de native messaging no perfil de teste é ignorado;
     - (c) **todo Chrome de teste sem `--disable-extensions` recebe a extensão 0.1.x de
       verdade no perfil temporário**. O content script `envio.js` dela roda em toda página
       http(s), e num envio de formulário confiável ela fala com o host do sistema
       (`/usr/local/bin/ward-nm` → o `wardd` instalado, com o cofre real).
   - Por isso **não rodei `test-extension` nem `test-login`**: eles fazem login com clique
     real no WordPress 8081, e a extensão forçada poderia levar usuário e senha falsos ao
     daemon instalado.
   - O `test-nm` só abriu `about:blank`, e o service worker da 0.1.x não chama o host na
     partida (conferido no `servico.js` da linha de base).
   - O `ward playwright` e o Playwright do Python passam `--disable-extensions`, e o
     diagnóstico confirma que aí a extensão não entra. Por isso rodei `test-playwright` e
     `test-app-e2e`.
   - Não é defeito do produto: é o próprio Ward instalado agindo no Chrome como deve.
   - Para o orchestrator ou o usuário:
     - rodar essas suítes num contêiner com Chrome, ou com a política fora do lugar
       (`sudo`, é do usuário);
     - ou mudar as suítes para esconder `/etc/opt` sem perder `/run/user` (o bwrap não
       serve: o daemon e o `ward-nm` recusam `/run/user` de dono `nobody` no namespace);
     - e, nas suítes, recusar rodar quando `/etc/opt/chrome/policies/managed/` força o id
       do Ward (hoje só o `test-chrome-policy` percebe e pula).
2. **`test-app-e2e`: teste com expectativa antiga (D68).**
   - Comando: `make test-app-e2e PW_PYTHONPATH=…`.
   - Saída: `FALHOU - passo sem exceção (TimeoutError: Locator.click … "Permitir uma vez"
     … <button disabled …>)`, 9 de 10.
   - O log do daemon diz `executar: entrada padrão pipe` e `uso com a entrada padrão que a
     janela não mostra: aviso e confirmação na janela (D68)`.
   - Causa: `dev/tests/app_e2e.py › run_in_background` faz `Popen([WARD, "run", …])` sem
     `stdin`. O `ward run` herda a entrada do Python, que é o pipe por onde o `app-e2e.sh`
     passa a URL (`printf '%s\n' "$URL" | python3 …`). Com pipe, o D68 (Fase 54b da m1)
     prende os botões até a confirmação a mais. É o comportamento certo do produto
     (V6.8), e a evidência é `falha-1.png`.
   - Correção sugerida: `stdin=subprocess.DEVNULL` no `run_in_background`, como no
     `f65_ui.py`.
   - Os passos depois desse (uma vez, 1 hora com trava e destrave, sempre, Conectar com
     `--token-fd` e ataques ao token) não rodaram.
3. **Permissões negadas nesta sessão**:
   - o roteiro de capturas que sobe um `wardd` de teste próprio com a ponte, pelo mesmo
     caminho do `dev/tests/app-e2e.sh` (comando exato na tabela acima);
   - `make test-revealed-selection` com `WARD_DEV_SOCKET_DIR`. Sem a variável, a suíte usa a
     pasta própria padrão e passou.

   Sem o roteiro ficaram de fora: as capturas de todas as telas (V3.2), V3.3, V6.9, a
   marca "fora de ordem" no app e o V4.16 no app nativo. O roteiro está pronto em
   `scripts/f65-ui.sh` e `scripts/f65_ui.py`, para rodar com permissão do usuário.
4. **`/run/user/1000/ward-tray`**: o harness do `test-webkit-real` grava o ícone da bandeja
   ali (`mkdir`, `tray-icon-ward-0.png`). Parece a mesma pasta do app instalado, se ele
   estiver aberto. Não afeta o teste, mas é uma escrita fora de `.dev/`. Vale conferir se
   o app de teste deveria usar outra pasta.
5. **Marca "fora de ordem" (achado 4 da Fase 64), pela leitura do código**: a cópia de
   geração igual com nome mais velho não é marcada, porque o conteúdo é o de uma gravação
   real. Restaurá-la dá o mesmo que restaurar a original, então isso parece certo. Mas,
   como o algoritmo marca "geração menor que a de alguma versão com data anterior", essa
   cópia faz a **versão verdadeira** mais velha, que vem depois dela pela data, aparecer
   com "gravação mais antiga do que a data diz". O usuário vê o aviso na versão errada.
   Não é perda de segurança (é só aviso), mas vale decidir no plano se fica assim. Não
   conferido no app (Problema 3).
6. **Guarda do worktree**: `make gitleaks` e um `git add … && make gitleaks` foram barrados
   porque o texto do comando cita "git". A varredura de padrões da evidência rodou
   (`grep -F -f .dev/variations.txt`, nada achado), mas o gitleaks da fase não.
7. **`npm audit`**: 2 avisos *low* em `deepmerge` (por `@sveltejs/vite-plugin-svelte`,
   dependência de build), sem correção publicada. Abaixo do corte `moderate` do
   `app-front`. Fica para o V6.11.
8. **V6.5 (texto)**: o contrato pede "domínio não confere" para `ward login` **e**
   `ward playwright`. O `ward playwright` diz "a origem da página não bate com a da
   entrada", igual à 0.1.4. É expectativa do contrato, não defeito.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `docs/mission/evidence/M6/fase-65/fase65-*.log` | criados |
| `docs/mission/evidence/M6/fase-65/test-app-e2e/` (5 PNG e `qa.json` do `test-app-e2e`, movidos de `docs/mission/evidence/test-app-e2e/`) | criados |
| `docs/mission/evidence/M6/fase-65/scripts/f65-ui.sh`, `f65_ui.py`, `f65_caps.py` | criados |
| `docs/mission/handoffs/phase-65.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, porque a fase não mudou código. `make gitleaks` foi barrado pela guarda do worktree (Problema 6); a evidência passou no `grep -F -f .dev/variations.txt`
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado: pelas suítes. As que faltam estão em "Ficou de fora"
- [ ] (UI) Conferido contra a m1: só em parte (4 capturas; Problemas 1 e 3)
- [x] Ficou dentro do escopo da fase:
  - nenhum código corrigido;
  - serviços parados;
  - nenhum daemon de teste, Chrome, contêiner do Ward ou processo auxiliar no ar (só o `wardd` instalado, intocado);
  - `target/`, `app/src-tauri/target`, `app/node_modules`, `.dev/pw-lib` e as pastas de trabalho das suítes apagados
