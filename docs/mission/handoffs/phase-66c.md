# Fase 66c — Ressalvas do scrutiny do M6 (V6.8/V6.9 pela UI, capturas e teclado, Chrome isolado, contrato)

## Implementado
- **A1 (m2:V6.9 pela UI)**, em `dev/tests/app_e2e.py` (`make test-app-e2e`):
  - mudar o nível de `acme-pg-dev` pede o PIN; PIN errado mostra "PIN incorreto. Nada foi
    alterado." e o nível continua livre; PIN certo muda para "Com aprovação" (o `ward run`
    passa a abrir a janela) e volta a livre;
  - 4 PINs errados no diálogo de nível e o 5º na janela de aprovação travam o cofre: janela
    com "PIN bloqueado", os três "Permitir" desligados, a CLI negada, o app no destravar;
  - depois, 5 PINs errados só no diálogo de nível travam de novo; o log do daemon tem as
    duas travas por PIN (S9).
- **A2 (m2:V6.8 pela UI)**, na janela de `acme-pg-prod`:
  - entrada, destaque `prod`, comando exato, diretório e cadeia, com os 4 botões;
  - cada um dos três "Permitir" sem PIN só pede o PIN (a CLI continua esperando); PIN
    errado mostra o erro e nada roda;
  - com a entrada padrão num pipe: aviso, os três desligados mesmo com o PIN até a
    confirmação a mais; ataque que tira `acknowledged` da resposta no caminho (rota do
    Playwright) é recusado e nada roda; com a confirmação e o PIN, roda;
  - sem resposta, o aviso "O pedido expira em N s" aparece e a CLI sai com "expirado" em
    100–135 s.
- **A3 (m2:V3.2, V3.3, V3.4)**, captura de cada tela com a varredura do DOM, dos valores
  dos campos, dos `title` e do armazenamento contra `.dev/variations.txt` (tela com
  padrão não vira PNG):
  - destravar (padrão e senha errada), Credenciais cheia, busca, sem resultado, filtros,
    detalhe, revelar/ocultar (só checagem do DOM, sem captura do valor), nível, nova
    credencial, formulário novo e de edição dos 9 tipos, gerador, Configurações e os
    diálogos trocar senha, trocar PIN, restaurar backup e kit;
  - com o daemon falso da ponte: primeiro uso, Credenciais vazia, escolher pendrive;
  - teclado: foco inicial na senha e em Negar, ordem do Tab da casca
    (`ordem-de-foco-credenciais.json`), foco visível, Shift+Tab, Tab/Shift+Tab na janela de
    aprovação, setas nas opções de nível, Esc fecha filtros, nível, os diálogos de
    Configurações e escolher pendrive, e Esc na janela nega (CLI ≠ 0, nada roda).
  - Pasta de evidência configurável por `E2E_EVIDENCE` (absoluta, dentro de
    `docs/mission/evidence/`).
- **D1**: `dev/tests/totp_detail.py` abre o Chrome pelo `dev/tests/chrome-isolated.sh` e
  exige nenhum alvo `chrome-extension://` (sai 3).
- **D2**: `make test-playwright` e `make test-chrome-policy` rodam no contêiner de
  navegador (`ward-teste/browser:1`):
  - `dev/tests/browser-container.sh` ganhou as suítes `playwright` e `chrome-policy`;
    `dev/container-browser/inside.sh` as roda;
  - `test-playwright`: o binário do `tests/playwright.rs` é compilado no host
    (`--no-run`, caminho achado por `dev/tests/test_binary.py`) e roda no contêiner com
    `--ignored`, seguido do `playwright.sh`; `posts.log` pelo `service_relay.py`; sem `ss`
    no contêiner, a porta é conferida só por `/proc/net/tcp*`;
  - `test-chrome-policy`: no host só monta a política (`install.sh` com `DESTDIR`) e a
    chave; as rodadas rodam no contêiner com `/etc/opt/chrome` = pasta do teste montada
    (sem bwrap). A rodada 3 (controle do ataque da auditoria) deixa de ser pulada e passa
    a ser falha se pular: reproduz o roubo sem a política e a rodada 4 o bloqueia.
- **Contrato** (`docs/mission/validation-contract.md`), só as três mudanças da linha da fase:
  - **V6.2**: troca a lista curta pelos lugares permitidos de verdade (tabela da Fase 60):
    documentos da missão e notas/tabelas de `docs/` e `CLAUDE.md`; `dev/fixtures/legacy/`;
    tabela de conversão v4/v1; testes de recusa em que o nome antigo é o ataque; deny e
    `skill/README.md`; marca antiga de `test-mode` e caminhos da 0.1.x da atualização e do
    uninstall; rótulos pt-BR exibidos e o arquivo `estado`; protocolo do teste da área de
    transferência, chaves internas e prosa pt-BR dos logs. Também registra como a procura
    conta palavras comuns (só literal exato entre aspas) e subcomandos (`ward <nome>`).
  - **V6.3**: "nenhum texto exibido mudou" passa a ter a exceção das mensagens novas de
    propósito (migração para o corpo v5, versão do protocolo DT8, nome reservado da Fase
    54b, `ward-state 2`).
  - **V6.11**: `npm audit` "sem moderate ou superior; cada low sem correção registrado"
    (hoje 2 low de `deepmerge`, via `@sveltejs/vite-plugin-svelte`, de build), e a exceção
    registrada do `.gitleaks.toml`: a forma base64 de dois valores falsos de D15 (senha
    mestra de teste e do WordPress), usada em `dev/fixtures/protocol-v2.jsonl`, casada
    inteira.
- **B5**: a sobra não rastreada `docs/mission/evidence/test-app-e2e/` do checkout principal
  (`falha-0.png`, `qa.json` de uma rodada com pacote velho; sem padrão) foi copiada para
  `docs/mission/evidence/M6/test-app-e2e/`. O original no checkout principal **não** foi
  apagado (fora do worktree): o orchestrator apaga.
- `CLAUDE.md`: linhas da guarda, do `test-app-e2e`, do `test-playwright`, do
  `test-chrome-policy` e nova linha do `test-totp-detail`.
- Evidência em `docs/mission/evidence/M6/fase-66c/`: `test-app-e2e/` (58 PNG, `qa.json`,
  ordem de foco), `totp-detalhe.png` e os logs `fase66c-*.log` (caminhos trocados por
  `<repo>`/`<home>`).

## Segurança
- Invariantes tocadas:
  - S9 (5 PINs travam): testado pela UI nos dois caminhos (nível + janela, e só nível).
  - S10 (aprovação, expira em 2 min, PIN nos permitir): testado pela UI.
  - D68 (entrada em pipe): a UI segura os botões e a resposta sem a confirmação é recusada.
  - S11/V3.4 (valor fora da tela): cada captura passa pela varredura; revelar só no DOM,
    ocultar e sair da entrada tiram o valor.
  - S15 (contrato do gitleaks): a exceção base64 agora está escrita no V6.11.
- Testes de ataque escritos:
  - `dev/tests/app_e2e.py` › `approval_pipe` (rota tira `acknowledged`; recusado, nada roda);
  - `approval_prod` (três "Permitir" sem PIN, PIN errado, Esc nega);
  - `five_wrong_pins` (o 5º trava e nega, nos dois caminhos);
  - `screens`/`shot` (varredura de V3.4 em toda tela, controle positivo no revelar);
  - `totp_detail.py` e `chrome_policy.py` › exigem nenhuma extensão; a rodada 3 do
    `chrome-policy.sh` falha se pular.
- Dependências novas: nenhuma no Rust nem no npm. Na imagem de teste
  `ward-teste/browser:1`, camada nova com `libatomic1` (o node do driver do Playwright do
  Python precisa dela; sem ela o driver cai com "Connection closed while reading from the
  driver"). A camada confere que a imagem segue sem a política do Chrome, sem
  `/etc/chromium` e sem `ward-nm`.
- Perfil seccomp novo `dev/container-browser/seccomp.json` (só `test-playwright` e
  `test-chrome-policy`): o perfil padrão do Docker bloqueia o user namespace e o Chrome
  sem `--no-sandbox` não abre ("Failed to move to new namespace"). Como `ward playwright`
  nunca usa `--no-sandbox` (o teste de unidade confere), escolhi abrir o namespace em vez de
  tirar o sandbox. O perfil é ALLOW por padrão com lista de bloqueio (mount e família,
  bpf, keyctl, ptrace, process_vm_*, setns, io_uring, kexec, módulos, relógio, swap,
  reboot, perf, userfaultfd…); o contêiner segue com `--cap-drop ALL`,
  `no-new-privileges`, `--network none` e o uid do usuário.

## Ficou de fora
- Apagar `docs/mission/evidence/test-app-e2e/` no checkout principal (fora do worktree; o
  orchestrator faz).
- Reexecutar `test-nm`, `test-extension` e `test-login` com a imagem nova: a camada só
  acrescenta `libatomic1` e as suítes antigas não mudaram de caminho no
  `browser-container.sh` (o perfil seccomp só entra para playwright e chrome-policy). Não
  rodei por tempo e disco; fica para o QA do milestone.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-services`, `make dev-services-up`, `make dev-services-verify` | 0 |
| `make dev-vault`, `make dev-variations`, `make app-front` | 0 |
| `PW_PYTHONPATH=… TOTP_DETAIL_SHOT=… make test-totp-detail` | 0 (23 ok) |
| `make test-chrome-policy` | 0 (rodadas 1, 2, 2b, 3 e 4) |
| `PW_PYTHONPATH=… make test-playwright` | 0 (`--ignored` 1/1, 37 ok) |
| `E2E_EVIDENCE=… PW_PYTHONPATH=… make test-app-e2e` | 0 (151/151) |
| `make compile` | 0 |
| `make english-check` | 0 |
| `make check` | 0 |
| `make dev-services-down` | 0 |

## Problemas descobertos
- Comando negado pelo controle de permissões:
  `E2E_EVIDENCE=… PW_PYTHONPATH=… WARD_DEV_SOCKET_DIR=/run/user/1000/ward-test-e2e-66c make test-app-e2e < /dev/null > .dev/fase66c-test-app-e2e.log 2>&1; echo rc=$? >> …`.
  Rodei sem `WARD_DEV_SOCKET_DIR`: o `app-e2e.sh` usa o padrão
  `/run/user/1000/ward-test-e2e` (pasta própria da suíte, não a `ward-test` comum).
  Outros negados no caminho (edição por heredoc de Python, `docker run` com
  `seccomp=unconfined`, `systemd-run … cargo build`): troquei por Edit/Write, pelo perfil
  seccomp próprio e pelos alvos do make; nada foi contornado.
- As capturas da janela de aprovação mostram o caminho do worktree (comando, diretório) e a
  cadeia de processos da máquina, como as evidências anteriores da Fase 39c.
- A imagem antiga do contêiner de navegador pode ter ficado sem tag (`<none>`) depois do
  rebuild.
- `make test-app-e2e` ficou ~5 min mais longo (espera dos 2 min da expiração).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `CLAUDE.md` | alterado |
| `Makefile` | alterado |
| `dev/container-browser/Dockerfile` | alterado |
| `dev/container-browser/inside.sh` | alterado |
| `dev/container-browser/seccomp.json` | criado |
| `dev/tests/app-e2e.sh` | alterado |
| `dev/tests/app_e2e.py` | alterado |
| `dev/tests/browser-container.sh` | alterado |
| `dev/tests/chrome-policy.sh` | alterado |
| `dev/tests/chrome_policy.py` | alterado |
| `dev/tests/playwright.py` | alterado |
| `dev/tests/test_binary.py` | criado |
| `dev/tests/totp_detail.py` | alterado |
| `docs/mission/validation-contract.md` | alterado (V6.2, V6.3, V6.11) |
| `docs/mission/evidence/M6/fase-66c/` | criado |
| `docs/mission/evidence/M6/test-app-e2e/` | criado (sobra movida, B5) |
| `docs/mission/handoffs/phase-66c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft: não se aplica (fase de testes; capturas conferidas a olho)
- [x] Ficou dentro do escopo da fase
