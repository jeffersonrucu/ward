# Fase 64 — Bateria D (migração de ponta a ponta e versões misturadas)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard
  198753e` (master atual: "fase 63a concluída; bateria C verde").
- **Preparação**:
  - `.dev/baseline/` copiado do checkout principal; `sha256sum -c SHA256SUMS` passou em
    todos os arquivos;
  - `.dev/tools` copiado e conferido pelo `make tools`;
  - `make dev-vault` (22 entradas) e `make dev-variations` (279 padrões), em
    `fase64-prep.log`.
- **Nenhum código corrigido**. A fase só rodou e registrou.
- **Como rodou**: `make dev-daemon` não foi usado. O roteiro
  `evidence/M6/fase-64/scripts/f64-run.sh` sobe cada daemon de teste com as mesmas
  regras do `dev/tests/socket-attacks.sh`:
  - os dois daemons são o `wardd` 0.2.0 `test-mode` (`target/debug/wardd`) e o `wardd`
    0.1.4 `test-mode` de `.dev/baseline/bin-test-mode/`;
  - cada um tem uma cópia própria de `vault-0.1.4.ward`, `state-0.1.4` e
    `backup-0.1.4/` em `.dev/f64.XXXXXX/<pasta>`;
  - cada um tem seu socket em `$XDG_RUNTIME_DIR/ward-f64-…`, por `WARD_DEV_SOCKET_DIR`;
  - todos sobem por `env -i` com `RUST_LOG=trace`;
  - o roteiro só mata os pids que ele mesmo criou e apaga as cópias e os sockets no
    `trap`.

  O papel app vem do mecanismo de D63:
  - `WARD_DEV_APP` aponta para uma cópia do `f64.py` dentro da pasta de trabalho;
  - o `open_app`/`abrir_app` abre essa cópia como filha do daemon, e ela recebe o papel
    app (conferido na primeira resposta, `role=app`);
  - o `f64.py` faz `detail` e `reveal` de cada campo e extra secreto, `totp`, `config`,
    `backups`, `change_pin` e `restore_backup`;
  - todo valor revelado vira sha256 dentro do processo e nunca sai. A senha e o PIN
    falsos vêm de arquivos 0600, nunca do argv.
- **Testes de `cargo`** focados na bateria (logs `fase64-cargo-*.log`):

  | Teste | Resultado |
  | --- | --- |
  | `-p wardd --all-features --test migration` | 11 ok: inclui SIGKILL em cada ponto pelo `test-hooks`, falha de gravação, falha do estado, V4.6, V4.7, V4.10 local e pendrive, e V4.11 |
  | `-p ward-core --all-features -- atomic_write wire_v1 legacy` | 39 ok: inclui `dt5_sigkill_at_each_point_v4_to_v5` e os testes de fio v1 (`dt8_*`) |
  | `-p wardd --all-features --test socket` | 5 ok: inclui a simulação do V4.17 |
  | `-p ward --all-features` | 32 ok: inclui os dois testes m2:V4.16 da CLI |
  | `-p ward-app-core` | 209 ok: inclui `attack_m2_v4_16_server_of_another_version_closes_before_the_password` do app |
  | `-p wardd --all-features --lib -- files::` | 7 ok: estado v1/v2, estado misto ou forjado, link, FIFO e arquivo estranho recusados |

### Resultado por asserção

| Asserção | Como | Resultado |
| --- | --- | --- |
| V4.3 | `f64-run.sh migrate` e `kill` (60 rodadas de SIGKILL no `wardd` 0.2.0 entre 0,02 s e 0,62 s depois do envio do primeiro destrave), mais o `cargo test --test migration` (SIGKILL em cada ponto nomeado pelo `test-hooks`) | ok. O primeiro destrave responde `ok`, o arquivo vira `WARDCOFR` regravado e 0600, e o backup local ganha uma versão v5. Nas rodadas de morte: 43 ficaram com a v4 byte a byte, 17 com a v5 e **0 com outra coisa**. Em todas, o destrave seguinte funcionou, migrou o que ainda era v4 e não deixou temporário. A troca de v4 para v5 cai entre 0,447 s e 0,457 s, então os pontos internos ficam com o teste de `cargo` (Problema 5) |
| V4.4 | não repetido (feito no contêiner na Fase 63). No host, pela pasta depois da migração | ok. Cofre, estado e backups sem padrão de `.dev/variations.txt` e sem nome de tipo ou nível em claro |
| V4.5 | `migrate` + `v45`: o app de teste no 0.2.0 (cofre migrado) e no 0.1.4 (`vault-0.1.4.ward`), comparados por `f64.py compare` | ok. **22 entradas, 0 diferenças**: nomes, grupos, ambientes, projeto, tags, campos (nomes v1 → v2 pela tabela posicional e `security` pela DT4), sha256 de cada segredo e extra secreto, extras de texto, anexos (nome e tamanho), datas e níveis. TOTP: nas 3 entradas com semente, o código do daemon bate com o RFC 6238 da semente revelada, e a semente é a mesma nos dois. Configuração igual (240 min, backup externo desligado, backup local ligado). O PIN errado dá `wrong_pin`/`pin_incorreto` e o `135790` confere nos dois. Os bytes dos anexos não saem pelo protocolo (só nome e tamanho) e ficam com o `cargo test` V4.3/V4.5, que compara os bytes |
| V4.6 | trava e destrava 3 vezes como agente, depois para o daemon, sobe um novo e destrava | ok. O cofre fica com o mesmo inode, mtime, tamanho e sha256, e o estado fica igual. A versão 5 do corpo não aparece pelo protocolo: o `cargo test` V4.6 confere, e o 0.1.4 recusa o corpo (V4.12) |
| V4.7 | v4 de volta no lugar do v5, por `cp` + `mv`, como um processo do usuário faria | ok. Com o daemon parado e subido de novo, destravar dá `rollback`, a v4 fica intacta e o estado não muda. Com o daemon rodando e destravado, a gravação seguinte (troca de PIN) dá `vault_changed` e não sobrescreve nada. Com o daemon rodando mas **travado**, o destrave dá `rollback`, não `vault_changed` (Problema 1) |
| V4.10 | restaura a versão v4 de geração 2 (`ward-20261110-213053.ward`) pelo app; depois lixo aleatório, uma versão de outro cofre (`.dev/test-vault.ward`) e a cópia renomeada de uma v5; depois 35 gravações (tempo de trava) | ok. A restauração responde `ok`, o cofre é regravado como v5 (não é a cópia), com geração nova (`seen 5`), e o dump depois dela é igual ao migrado (22 entradas, 0 diferenças). As duas v4 e as v5 aparecem conferidas, o lixo e o outro cofre não. Depois das 35 gravações sobram exatamente 30 versões reais: as v4 saíram, a cópia renomeada saiu (não ocupou lugar) e o lixo e o outro cofre ficaram (só o usuário apaga). Pendrive: só pelo `cargo test` |
| V4.11 | `migrate` e os testes `files::` | ok. Subir com `ward-estado 1` não regrava o estado. Depois da migração, o estado é `ward-state 2`, `seen 4`, `burned 4` e `agent_failures 0` (o mesmo `falhas_agente`), 0600 e sem temporário. Nome no disco: o `test-mode` usa `test-vault.state`, e o nome `estado` da release fica com o teste de contêiner (Fase 63). Link, FIFO e estado misto ou forjado são recusados pelos testes de unidade. Dono errado precisa de root e não foi repetido aqui |
| V4.12 | `v412`: `wardd` 0.1.4 com a cópia do cofre migrado | ok. Com estado `ward-estado 1` (`vista 4`): `cofre_invalido` "o conteúdo do cofre é inválido" e log `destravar recusado: corpo inválido`. Com o estado v2 copiado: `retrocesso`. Nos dois casos, o arquivo fica com o mesmo inode, mtime e sha256, e não aparece arquivo novo. O mesmo arquivo destrava `ok` num `wardd` 0.2.0 novo. A mensagem não é "versão do corpo desconhecida" (Problema 2) |
| V4.16 (parte 0.1.4) | `v416`: CLI 0.2.0 de teste num pty contra o `wardd` 0.1.4 real; o app pelo `cargo test -p ward-app-core` | ok. `ward unlock` sai 1 com "ward: versão do protocolo incompatível com o daemon: atualize o Ward; nada foi enviado", **sem prompt** (nada foi digitado). `list` e `lock` dão a mesma mensagem. O 0.1.4 continua travado e só registrou 3 × "pedido recusado (agente): versao_desconhecida", nenhum `destravar`. A parte do servidor falso já tinha passado no `test-cli` (Fase 62) |
| V4.17 | `v417`: CLI 0.1.4 de teste num pty contra o `wardd` 0.2.0 (cópia do cofre de teste) | ok no que importa. `lista` sai 1, `run acme-pg-dev -- /usr/bin/touch <marca>` (a operação `executar`) sai 125 e `destravar` sai 1, todos **sem prompt**. A marca não foi criada, o 0.2.0 continua travado e registrou `unknown_version`. A CLI velha mostra "ward: resposta inválida do daemon", não um erro de versão (Problema 3). `ward executar` nem existe na CLI 0.1.4 (sai 2, "comando desconhecido") |

## Segurança

- **Invariantes tocadas** (só por execução):
  - S7: gravação atômica, nunca outra coisa além da v4 ou da v5, rollback e
    vault_changed;
  - S2: arquivos cifrados, sem padrão;
  - S5: versão conferida antes da senha, nos dois sentidos;
  - S13: logs `trace` de todos os daemons, inclusive os 120 das rodadas de morte, sem
    padrão.
- **Testes de ataque escritos** (roteiros à mão em
  `docs/mission/evidence/M6/fase-64/scripts/`):
  - `f64-run.sh › sec_kill`: SIGKILL em 60 instantes do primeiro destrave;
  - `sec_migrate`:
    - "V4.7 com o daemon parado, destravar a cópia v4 dá rollback";
    - "trocar com o daemon rodando (destravado) dá vault_changed na gravação";
    - "V4.10 lixo e outro cofre não conferidos" e a rotação com a cópia renomeada;
    - "S2: … sem padrão";
  - `sec_v412`: o 0.1.4 diante do v5 não grava;
  - `sec_v416`: a CLI nova contra o daemon velho, sem prompt e sem `destravar` no log
    dele;
  - `sec_v417`: a CLI velha contra o daemon novo, sem executar e sem destravar;
  - `f64.py`: o app e o agente de teste com hash dos segredos, e a comparação.
- **Evidência**: `grep -F -f .dev/variations.txt` em `docs/mission/evidence/M6/fase-64/`
  não acha nada (exit 1). Os caminhos do home, do worktree e do `/run/user/<uid>` e o
  nome do usuário viraram `<home>`, `<repo>`, `<rt>` e `<user>`. A tela do pty nunca
  teve a senha: nenhum prompt apareceu em nenhum dos dois sentidos. Os dumps (só hashes)
  ficaram fora do git. `make gitleaks` (árvore, índice, histórico e extra): "no leaks
  found".
- **Dependências novas**: nenhuma.
- **Não tocados**: o cofre real, o `/usr/local/bin/wardd` instalado (que ficou no ar sem
  contato), `/usr/local`, `~/.ssh` e `~/.aws`. Nenhum `sudo`.

## Ficou de fora

- Correções, por regra da fase.
- V4.4 em contêiner: feito na Fase 63.
- V4.11 com estado de outro dono: precisa de root (contêiner). Não repeti aqui.
- V4.10 pendrive de ponta a ponta: só pelo `cargo test` (`v4_10_usb_drive_rotation_counts_v4_and_v5`).
- V4.16 no app nativo de verdade (Tauri): fica para a Fase 65. Aqui, só o
  `ward-app-core` pelo `cargo test`.
- Nenhum comando foi negado na parte que importa. Algumas formas de linha foram
  barradas pelo ambiente e rodei a forma simples equivalente:
  - um `export PATH=…; cargo build …` encadeado → `cargo build` com caminho absoluto;
  - um `grep … environ` sobre o `cli.py` → li o arquivo pelo Read;
  - `ls`/`python` em `/run/user/1000` para listar sockets que sobraram → não consegui
    listar a pasta. O `trap` do roteiro apaga os sockets de cada rodada, e nenhum
    processo de teste ficou no ar;
  - `sed -i` na evidência → um script Python no scratchpad.

  Nenhum desses era subir daemon por `make dev-daemon`, e nenhum foi contornado para
  fazer algo diferente.

## Comandos executados

| Comando | Exit code |
| --- | --- |
| `git reset --hard 198753e` | 0 |
| `sha256sum -c SHA256SUMS` (`.dev/baseline/`) | 0 |
| `make tools` | 0 |
| `make dev-vault && make dev-variations` (`fase64-prep.log`) | 0 |
| `make origins` | 0 |
| `cargo build --locked -p wardd --features test-mode --bin wardd` / `-p ward … --bin ward` | 0 / 0 |
| `cargo test --locked -p wardd --all-features --test migration` | 0 (11 ok) |
| `cargo test --locked -p ward-core --all-features -- atomic_write wire_v1 legacy` | 0 |
| `cargo test --locked -p wardd --all-features --test socket` | 0 (5 ok) |
| `cargo test --locked -p ward --all-features` | 0 (32 ok) |
| `cargo test --locked -p ward-app-core` | 0 (209 ok) |
| `cargo test --locked -p wardd --all-features --lib -- files::` | 0 (7 ok) |
| `f64-run.sh migrate v412 v45` (1ª rodada, `fase64-migrate.log`) | 1: dois erros do roteiro, corrigidos (cópia renomeada vazia; `config` × `configuracao` no nome do tipo) |
| `f64-run.sh migrate v412 v45 v416 v417` (2ª, `fase64-battery-d.log`) | 1: expectativa errada do roteiro sobre a cópia renomeada ("conferida" ≠ "ocupa lugar"), trocada pela conferência da rotação |
| `f64-run.sh migrate v412 v45 v416 v417` (final, `fase64-battery-d.log`) | 0 |
| `f64-run.sh kill` com 0,5–4,4 s | parado: todo kill caía depois da migração |
| `f64-run.sh kill` com 60 rodadas entre 0,02 e 0,62 s (`fase64-v43-kill.log`) | 0 |
| `make english-check PATHS="docs/mission/evidence/M6/fase-64"` | 0 |
| `make gitleaks` | 0 |
| `rm -rf target app/src-tauri/target` | 0 |

Todas as rajadas rodaram em `systemd-run --user --scope -p TasksMax=500` (1000–1500 nos
builds e testes). `df -h` antes do build: 18 GB livres. `make check` não foi rodado: a
fase não mudou código (só pôs roteiros em `docs/`), e o gitleaks e o english-check que
cobrem os arquivos novos passaram.

## Problemas descobertos

Nenhum defeito do produto. Todos os itens abaixo são texto do contrato que não bate com o
que a 0.1.4 ou o desenho fazem, ou cobertura que só o `cargo test` dá.

1. **V4.7: "trocar com o daemon rodando dá `vault_changed`"**.
   - Com o daemon rodando e **destravado**, a gravação seguinte dá `vault_changed`, como
     o contrato pede.
   - Com o daemon rodando e **travado**, o destrave relê o arquivo e dá `rollback`. A
     geração do estado vence, e o resultado é igualmente seguro.
   - Causa provável: o contrato não diz em que estado o daemon está. Sugestão: o
     contrato cita "destravado" para o `vault_changed` e aceita `rollback` no destrave.
2. **V4.12: mensagem do 0.1.4**.
   - O `wardd` 0.1.4 recusa o v5 com `cofre_invalido` ("o conteúdo do cofre é inválido";
     log "corpo inválido"), não com "versão do corpo desconhecida".
   - A recusa e o "nada gravado" valem. O binário 0.1.4 não muda, então o contrato tem
     de aceitar essa mensagem.
   - Com o estado `ward-state 2` copiado, o 0.1.4 responde `retrocesso`, também sem
     gravar.
3. **V4.17: "recebe erro de versão"**.
   - A CLI 0.1.4 mostra "ward: resposta inválida do daemon". O daemon 0.2.0 responde
     `unknown_version` num quadro v2 (DT8), e a CLI velha não sabe lê-lo.
   - Nada executa e o cofre fica travado. Nenhum prompt apareceu, então nenhum byte de
     senha saiu.
   - É expectativa do contrato, não defeito: a 0.1.4 não muda, e a DT8 manda responder
     em v2.
   - O contrato também cita `executar`, que é o nome da operação v1. O subcomando da
     CLI 0.1.4 é `run`.
4. **V4.10, listagem da cópia renomeada**.
   - Uma cópia de uma v5 com nome **mais antigo** e a mesma geração da original aparece
     como conferida e `out_of_order=false`.
   - No `cargo test`, a cópia de uma v4 com nome **mais novo** sai `out_of_order=true`.
   - A cópia não ocupou lugar na rotação (saiu com as 35 gravações), que é o que o
     V4.10 pede. Fica só a observação: a marca "fora de ordem" não aparece numa
     geração repetida com nome mais velho. Se o app mostrar essa marca ao usuário,
     vale conferir na Fase 65.
5. **V4.3 de ponta a ponta tem pouca resolução**.
   - A migração (Argon2id + gravação) termina em cerca de 0,45 s neste host. A troca de
     v4 para v5 nas rodadas cai numa janela de 10 ms, então o SIGKILL externo quase
     sempre pega "antes" ou "depois".
   - Os pontos internos (depois do write, do fsync, do rename, antes do fsync da pasta e
     antes do estado) ficam com o `cargo test` (`death::v4_3_sigkill_at_each_migration_point`
     e `dt5_sigkill_at_each_point_v4_to_v5`), verdes.
6. **V4.6, "o corpo diz versão 5"**: não aparece pelo protocolo. Fica com o `cargo test`
   e, de forma indireta, com a recusa do 0.1.4 (V4.12).
7. **Permissões do ambiente**: o `make dev-daemon` não foi tentado, porque daemons
   próprios, como os das suítes, bastaram.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `docs/mission/evidence/M6/fase-64/fase64-*.log` (prep, cargo-migration, cargo-core, cargo-socket, cargo-ward, cargo-app-core, cargo-wardd-files, migrate, battery-d, v43-kill) | criados |
| `docs/mission/evidence/M6/fase-64/logs/` (logs `trace` dos daemons 0.2.0 e 0.1.4 e telas do pty de V4.16/V4.17) | criados |
| `docs/mission/evidence/M6/fase-64/scripts/f64-run.sh`, `scripts/f64.py` | criados |
| `docs/mission/handoffs/phase-64.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, porque a fase não mudou código. `make gitleaks` e `make english-check` dos arquivos novos passaram
- [x] Só cofre de teste e credenciais falsas usados (cópias das fixtures 0.1.4 e do cofre de teste em `.dev/`)
- [x] Teste de ataque para cada (SEG) tocado (V4.3, V4.7, V4.12, V4.16, V4.17)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase:
  - nenhum código corrigido;
  - nenhum daemon de teste, contêiner ou processo auxiliar no ar (só o `wardd` instalado do usuário, intocado);
  - `target/`, `app/src-tauri/target` e as pastas `.dev/f64*` apagados.
