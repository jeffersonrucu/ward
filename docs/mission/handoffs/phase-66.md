# Fase 66 — Artefatos 0.2.0 e roteiro do usuário

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b` (0.1.5, só no `origin/master`); com a árvore
  limpa, `git reset --hard 3b3dc3b` (master atual, "fase 65c concluída").
- **Recusa de pacote de teste velho** (achado da 65c): `dev/check-test-bundle.sh` (novo)
  sai 1 se `app/dist-teste/index.html` falta, é link ou é mais velho que qualquer fonte
  (`app/src`, `app/index.html`, `app/vite.config.js`, `app/package.json`,
  `app/package-lock.json`), com a mesma regra do `app-e2e.sh` da 65c; rótulo da
  mensagem validado (`^[a-z0-9-]{1,40}$`, senão sai 2). Chamado por `make app-bridge`
  (no lugar do `test -f`, antes de qualquer build ou daemon) e por
  `dev/tests/revealed-selection.sh` (logo depois da lista de arquivos). O `app-e2e.sh`
  ficou como a 65c deixou.
- **Ajustes do contrato** (`docs/mission/validation-contract.md`, sem mudar produto):
  V4.7, V4.12, V4.17 e V6.5; lista abaixo, para o usuário.
- **`docs/instalacao.md`**: "60 caminhos" → "61 linhas (o ./ e 60 caminhos)", contado
  no `.deb` montado (`dpkg-deb -c | wc -l` = 61, sendo `./` a 1ª linha); a 0.1.x diante
  do cofre migrado diz "o conteúdo do cofre é inválido" (ou recusa como retrocesso),
  não "versão do corpo desconhecida" (o que a Fase 64 viu no binário 0.1.4).
- **README**: já cita `sudo apt install ./ward_0.2.0_amd64.deb` (Fase 60), e esse é o
  nome que o `make deb` gerou. Sem mudança.
- **Artefatos 0.2.0** (como o usuário, sem `sudo`): `make release` (0), `make extension`
  (0, id `nhkehjnmlgkebhmelajfpmpblepaichf`), `make extension-crx` (0, "versão 0.2.0",
  id igual ao `EXTENSION_ID` de `crates/ward-nm/src/lib.rs:80`), `make deb
  EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSION_CRX=…/target/extension/ward.crx`
  (0; o próprio `deb.sh` conferiu lista = staging, root/root, sem bit especial).
- **Conferência do `.deb`**: `Package: ward`, `Version: 0.2.0`, `Architecture: amd64`,
  `Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0`; 61 linhas no
  `dpkg-deb -c`, todas `root/root`, nenhum `s`/`S` nos modos; binários 0755 (o 2755 vem
  do `dpkg-statoverride`); os sha256 de `ward`, `wardd`, `ward-app`, `ward-nm` e
  `ward.crx` no staging (`target/deb/package`) são os mesmos dos artefatos de release.
  Sem `test-mode`: `app-release.sh` (do `make release`) passou no `ward-app`, e
  `strings` dos quatro binários não tem `WARD_DEV_`, `ward-test-bridge`, `wardd-test`
  nem `ward-teste` (0 ocorrências). `ward --version` = `ward 0.2.0`; `tauri.conf.json`,
  `Cargo.toml` (workspace e app) e o manifesto da extensão dizem 0.2.0 (`wardd`,
  `ward-nm` e `ward-app` não imprimem a versão).
- **Cópia para o usuário**: `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/`
  com `ward_0.2.0_amd64.deb`, `ward.crx`, `bin/{ward,wardd,ward-nm,ward-app}` e
  `SHA256SUMS`; `sha256sum -c` da cópia: todos SUCESSO.

  | Artefato | sha256 |
  | --- | --- |
  | `ward_0.2.0_amd64.deb` | `b16b8c9f409d39f6a6311ce99d131d19688efe8498fb34b17448ec7fa29de357` |
  | `ward.crx` | `c4187c1403bc0c2d16f3f81e6026385614636b0a75a12d165c76cb1fc029c5f9` |
  | `bin/ward` | `4982efe94c7eb2e6e4c1be0a2b53271689e543b331816ac31429561efa641aa4` |
  | `bin/wardd` | `926d75b76ee834aed6afbaef81830d6fb7c2cb27bfc91fae91feeebd4eb09c7c` |
  | `bin/ward-nm` | `dd2238daf41e324c8007c4c280837ae801543799ce49cad9de798554ebc56a03` |
  | `bin/ward-app` | `1e43093222935fae42dcd8c4a63f305ed90ccd054be08c49f2ad47a6269d457a` |

- **Roteiro do usuário** `docs/mission/roteiro-usuario.md`: aviso da 0.1.5 (Problema 1),
  artefatos, 1 copiar a pasta do cofre (como em `docs/instalacao.md`) e nomes
  reservados, 2 conferir sha256/versão, 3 instalar por cima (3a `.deb` copiado para
  `/root/ward` e `sudo dpkg -i`; 3b `sudo make install` com `BIN_*` e `EXTENSION_CRX`
  apontando para `target/release-0.2.0/`; 3c conferir), 4 primeiro destrave =
  migração por `/usr/local/bin/ward unlock` ou pelo app, 5 conferir entradas, 6 skill e
  deny no `~/.claude` a partir de `skill/` (o usuário faz), 7 janela nativa, bandeja
  ("Travar agora"), aprovação (4 botões, PIN), código TOTP novo no detalhe (65b) e
  extensão 0.2.0, 8 apagar a cópia depois, 9 voltar pela cópia.

### Mudanças no contrato (para o usuário)

1. **V4.7**: com o daemon rodando, o resultado depende do estado dele: destravado, a
   gravação seguinte dá `vault_changed`; travado, o destrave relê o arquivo e dá
   `rollback`. Antes o contrato só dizia "rodando dá `vault_changed`" (achado da 64).
2. **V4.12**: o `wardd` 0.1.4 recusa o cofre v5 com `cofre_invalido` ("o conteúdo do
   cofre é inválido", log "corpo inválido") com o estado `ward-estado 1`, e com
   `retrocesso` com o estado `ward-state 2`; antes o contrato pedia "versão do corpo
   desconhecida". A recusa sem gravar nada continua exigida (achado da 64).
3. **V4.17**: a CLI 0.1.4 pode mostrar "ward: resposta inválida do daemon" (não lê o
   quadro v2 `unknown_version` da DT8), além do erro de versão; `executar` virou `run`
   (o subcomando da CLI 0.1.4; `executar` é o nome da operação v1); o daemon tem de
   registrar `unknown_version`. Nada executa e o cofre continua travado, como antes
   (achado da 64).
4. **V6.5**: `ward playwright` recusa com "a origem da página não bate com a da
   entrada" (texto da 0.1.4, em `strings-0.1.4.txt`); "domínio não confere" fica para
   `ward login` e a extensão. A recusa e o `posts.log` sem mudança continuam exigidos
   (achado da 65).

## Segurança

- Invariantes tocadas: nenhuma lógica de produto mudou. S14/S15 (artefatos de release
  sem `test-mode`, sem segredo) conferidos acima. A recusa do pacote velho é só de
  testes (evita falso verde/timeout com interface da 0.1.x); nada afrouxado.
- Testes de ataque: `dev/check-test-bundle.sh` rodado à mão — pacote atual sai 0;
  `touch app/src/App.svelte` sai 1 com a mensagem; `make app-bridge` com o pacote velho
  para em `Makefile:322` antes do build e do daemon; sem `app/dist-teste` sai 1; rótulo
  `x;y` sai 2. Nenhum (SEG) do contrato tocado em código.
- A chave `.dev/extension/key.pem` foi copiada 0600 para o `.dev/` do worktree (apagado
  junto no fim); nenhum segredo em docs, handoff ou log.
- Dependências novas: nenhuma.

## Ficou de fora

- `make check` inteiro: pela regra 6 do PRD a fase fecha com `make compile` e
  `english-check`; a bateria é dos validadores do M6.
- `make test-deb` com os artefatos novos (V6.13 pede "passa no `make test-deb`"): exige
  Docker e a linha de base; é do validador. O `deb.sh` já fez as conferências do pacote.
- Instalar, publicar a release, `~/.claude`: do usuário.
- `make extension-crx` abre o Chrome sem a guarda da 65a (achado da 65a, aceito porque
  só empacota com perfil e HOME temporários em `target/`). Mantido.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 3b3dc3b` | 0 |
| `df -h /` (antes do build) | 0 (21 GB livres) |
| `make release` | 0 |
| `make extension` | 0 |
| `make extension-crx` | 0 |
| `make deb EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSION_CRX=…/ward.crx` | 0 |
| `dpkg-deb -f/-c` (conferência) | 0 |
| `dev/check-test-bundle.sh` (atual / velho / sem pacote / rótulo inválido) | 0 / 1 / 1 / 2 |
| `make app-bridge` com pacote velho | 2 (recusa esperada, antes do build) |
| `sha256sum -c SHA256SUMS` na cópia do checkout principal | 0 |
| `make compile` | 0 |
| `make english-check` (repositório inteiro) | 0 (0 achados) |

## Problemas descobertos

1. **(Crítico para a publicação) A 0.2.0 não tem a correção da 0.1.5.** A máquina tem
   `ward 0.1.5` instalado (`dpkg-query -W ward`). O commit `b4d5e1b` ("aberto pelo menu
   com o app na bandeja, mostra a janela; 0.1.5") está só no `origin/master`; o master
   local saiu de `5114a25` (0.1.4) e não o tem (`merge-base master origin/master` =
   `5114a25`; nenhum `show_window`/`mostrar_janela` no código). Ele muda protocolo
   (resposta nova `mostrar_janela`, só app), daemon (`aprovacao.rs`), núcleo do app e
   `main.rs`. Instalar estes artefatos por cima da 0.1.5 traz de volta o defeito
   (abrir pelo menu com o app na bandeja não mostra a janela) e a linha de base dos
   testes (0.1.4) não é a versão instalada. Sugestão: fase de correção que porta
   `b4d5e1b` para os nomes em inglês (resposta v2 `show_window`, testes de papel:
   só o app recebe, sem dado), depois refazer os artefatos. O roteiro avisa o usuário
   no topo para não instalar antes dessa decisão. Fora do escopo da fase: não portei.
2. `docs/mission/prd.md:223` ainda diz que o 0.1.4 recusa o v5 com "versão do corpo
   desconhecida"; o binário diz "o conteúdo do cofre é inválido" (Fase 64). Corrigi o
   contrato (V4.12) e `docs/instalacao.md`; o PRD é do planner.
3. `wardd`, `ward-nm` e `ward-app` não têm `--version`: o V6.13 ("o app diz 0.2.0")
   só se confere pelo `Cargo.toml`/`tauri.conf.json`, pelo `Version` do `.deb` e pela
   janela "Sobre"/metainfo, se houver.
4. `target/release-0.2.0/` saiu 0755 (o `install -d -m 0700` não valeu para o pai já
   criado pelo `-d` com `bin/`); `bin/` é 0700. Sem segredo nos artefatos; só registro.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/check-test-bundle.sh` | criado |
| `dev/tests/revealed-selection.sh` | alterado |
| `Makefile` | alterado (`app-bridge`, comentário de `test-revealed-selection`) |
| `docs/mission/validation-contract.md` | alterado (V4.7, V4.12, V4.17, V6.5) |
| `docs/instalacao.md` | alterado ("61 linhas", mensagem da 0.1.x) |
| `docs/mission/roteiro-usuario.md` | criado |
| `docs/mission/handoffs/phase-66.md` | criado |
| `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/` (fora do git) | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não: regra 6 do PRD (`make compile` e `english-check` verdes)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG em código; recusa do pacote velho testada à mão)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
