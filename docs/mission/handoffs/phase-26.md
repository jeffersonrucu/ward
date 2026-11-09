# Fase 26 — `ward run`

## Implementado
- **CLI** (`crates/ward`):
  - `ward run <entrada> -- <comando> [args…]`. O argv vai em bytes, sem shell, e o `--` que vier depois é do comando. Sem entrada, sem `--` ou sem comando, o uso sai 2.
  - **D52:** o pedido é medido antes de conectar.
    - Acima de 750 KiB, sai 125 com a mensagem fixa do limite.
    - Campo acima de 128 KiB, ou argumentos/variáveis demais, também sai 125, com mensagem fixa.
    - Nenhum valor é citado.
  - Ambiente sem `LD_*` e cwd em bytes.
  - Antes do `executar`, um `estado`: com o cofre travado, avisa no stderr que espera o destrave por até 2 min (V4.21).
  - Na sessão:
    - a entrada padrão vai em quadros `entrada_padrao` de até 64 KiB, e um vazio marca o fim;
    - `saida` vai para stdout/stderr e `fim` dá o código de saída;
    - erro do daemon sai 125 com a mensagem (como `env` e `docker run`);
    - não há prazo de leitura nem de escrita.
  - Ajuda atualizada.
- **Plataforma** (`ward-plataforma::filho`, `sys`): fork/execve próprio no lugar do `std::process::Command`, para o segredo do ambiente não passar por buffer que não zera (S8). No filho:
  - `setpgid` e `PR_SET_PDEATHSIG=SIGKILL`; se o pai mudou, sai;
  - sinais no padrão e máscara vazia;
  - `dup2` dos pipes, `chdir` e `close_range(CLOEXEC)` de todos os fds do daemon;
  - busca no `PATH` do próprio filho (o padrão é `/bin:/usr/bin`), sem fallback para shell.

  A falha antes do exec volta por um pipe de aviso: 127 para comando não encontrado, 126 para sem permissão, com mensagem fixa. O `Grupo`:
  - espera com `waitid(WNOWAIT)`;
  - mata o grupo inteiro (`kill(-pgid)`) quando o líder sai, e também no `Drop`;
  - depois colhe o processo.

  Código de saída: o do processo, ou 128+sinal.
- **Daemon** (`wardd::execucao`, novo):
  - Ordem do `executar`:
    1. pré-checagem da nota segura visível (`tipo_sem_entrega`, sem pedir aprovação e sem revelar se uma bloqueada existe);
    2. `autorizar_uso`, depois `registrar_uso`;
    3. sob a trava, a entrada é encontrada de novo (pode ter mudado durante a aprovação) e a entrega (`ward_core::entrega::montar`) e a máscara são montadas;
    4. a trava é solta e o ambiente é montado: o da CLI sem `LD_*`, a primeira de cada duplicata, os nomes da entrega por cima e o ajuste de locale;
    5. lançamento.
  - **Locale (handoff 22a, RA3):** se o `LC_CTYPE` efetivo (`LC_ALL` > `LC_CTYPE` > `LANG`) não é UTF-8, o filho recebe `C.UTF-8` só na variável que manda. Assim o bash usa aspas, que a máscara cobre, e não `$'…'`.
  - **Anexos e chave (S2):** cada um vai para `<rt>tmp/s<pid>-<n>/<nome>`.
    - Pasta 0700; arquivo `create_new`, 0600, `O_NOFOLLOW|O_CLOEXEC`.
    - Ao sair (qualquer saída), cada arquivo é zerado (só se for o mesmo inode, regular e com `nlink==1`) e a pasta é apagada.
    - A partida do `wardd` esvazia `<rt>tmp/` (V4.15) e loga só a contagem.
    - `<rt>` é `/run/user/<uid>/ward/` no release (pelo uid efetivo) e o diretório do socket no daemon de teste.
  - **Sessão:**
    - threads de stdout/stderr com `poll` e passo de 100 ms, mascarando em fluxo e mandando `saida` de até 64 KiB;
    - stdin por fila limitada (8), com contrapressão até a CLI;
    - leitor da CLI: `EntradaPadrao` vai para a fila e `Janela` é ignorado. Qualquer outro quadro, ou o EOF da CLI, mata o grupo (D13).
    - No fim, nesta ordem: espera o líder, mata o grupo, colhe, apaga os temporários, dá 2 s para o resto da saída, manda `fim` e fecha a conexão. O log só leva números.
  - Novos erros de protocolo: `tipo_sem_entrega` ("tipo sem entrega") e `entrega_falhou` ("a entrega da credencial falhou; nada foi executado").
  - `servidor.rs`: um gancho de 7 linhas manda o `executar` do papel agente para `execucao::sessao`. Se um processo foi lançado, a conexão acaba com a sessão. Se o pedido foi recusado antes (`erro`), a conexão segue, como nas outras operações; o `v3_20` da Fase 22 manda vários `executar` recusados na mesma conexão. Nada mais mudou ali.
  - `config.rs`/`main.rs`: campo `rt`, `com_temporarios(rt)` e limpeza na partida.
  - `aprovacao.rs`: `Motor::esperando_sem_app()`, ligado no `main`. Com o cofre travado e o app indisponível, o pedido espera o `ward destravar` por até 2 min em vez de responder `travado` na hora (V4.21; ver Problemas). O `Daemon::novo` dos testes continua com o comportamento antigo.
- **Contêiner** (`make teste-units`, V4.38):
  - grupos `ward-app`/`ward-nm` e cópias setgid 2755 do cliente mínimo, com o `ward` de release instalado;
  - `ward run acme-pg-dev -- /usr/local/bin/ward-app … estado` (e `lista`, e o mesmo com o `ward-nm`), com o ambiente de D57, fica com papel agente;
  - o journal tem "papel app recusado (pid N) … (D63)", e nenhum valor `falso-` aparece nele;
  - o cliente mínimo agora imprime `resposta: estado papel=<papel>`.

## Segurança
- **Invariantes tocadas:**
  - **S1:** a CLI nunca recebe segredo. A sessão só manda `saida` (já mascarada), `fim` e `erro`. A `Resposta` do papel agente com credencial é recusada na codificação (`Cli::enviar`).
  - **S2:** temporários só em `<rt>tmp/` (0700/0600), zerados e apagados em toda saída e esvaziados na partida. O `<rt>` aberto ou que é link é recusado.
  - **S3:** o segredo só vai no ambiente, nunca em argv.
  - **S4:** máscara em fluxo nos dois canais, com corte entre pedaços e a pausa de D34.
  - **S8:** fork/exec com buffers `Zeroizing` de tamanho exato.
  - **S9/D8:** o uso passa por `autorizar_uso` (níveis, aprovação, D19).
  - **S13:** o log não leva valor.
  - **D13/D60:** o ambiente vem só da CLI, sem `LD_*` e sem nada do daemon (`LISTEN_*`, `WARD_DEV_*`, `RUST_LOG`).
  - **D63:** o filho do daemon com o grupo do app fica agente (V4.38).
- **Testes de ataque:**
  - `crates/wardd/tests/run.rs`:
    - `v4_entrega_mascara_codigo_e_recusas` cobre: `LD_PRELOAD`/`LD_LIBRARY_PATH` tirados; variável do daemon não vaza; a `WARD_PASS` da CLI é sobrescrita; a senha em claro, em base64 e em pedaços com pausa, no stdout e no stderr; nota, aprovação sem app e bloqueada sem executar nada (marca ausente); quadro fora da sessão mata o comando; o log em `trace` sem segredo nem base64.
    - `v4_15_cli_que_cai_e_daemon_morto`: a CLI cai e o grupo e o neto morrem em até 1 s, e o anexo some; o filho com SIGKILL dá 137 e o anexo some; o `wardd` com SIGKILL leva o filho junto (pdeathsig) e a partida seguinte limpa `<rt>tmp/` (0700).
    - `ra1_registro_environ_do_filho_legivel_pelo_mesmo_uid`: prende o estado do RA1 (o `environ` do filho é legível pelo mesmo uid; o do `wardd` não é).
    - `d52_pedido_grande_demais_recusado_pelo_daemon`: segunda camada, sem executar nada.
    - `v4_21_travado_espera_o_destravar_da_cli`.
  - `crates/wardd/src/execucao.rs` (unitários): `ataque_rt_aberto_ou_link_recusado`, `temporarios_0600_zerados_e_apagados` (um link no lugar do arquivo não é seguido nem zerado), `partida_limpa_o_que_ficou`, `ld_e_proibida`, `locale_utf8_fica_e_c_vira_c_utf8`.
  - `crates/ward-plataforma/src/filho.rs` (11 testes):
    - o ambiente não é herdado;
    - os fds do daemon não chegam ao filho;
    - os sinais ficam no padrão;
    - matar o grupo leva os netos;
    - pdeathsig;
    - busca no `PATH` sem shell;
    - `Debug` sem valor.
  - `crates/ward/src/testes.rs`: `d52_run_grande_demais_recusado_antes_de_conectar` (nem conecta) e `run_pedido_saida_e_codigo` (o pedido sai sem `LD_*`).
  - `crates/ward/src/args.rs` › `run`: a mensagem de uso não repete argumento.
  - `dev/testes/cli.py` (`make teste-cli`, contra o `make dev-daemon`):
    - V4.3–V4.8 e V4.10 por sha256 contra "Entrega esperada", com o TOTP calculado em Python;
    - V4.11, V4.17 (as 8 formas), V4.19, V4.36;
    - anexo 0600 em `<rt>tmp/` e apagado;
    - D60, `LD_*`, entrada padrão;
    - nenhum padrão de `.dev/variacoes.txt` em nenhuma saída;
    - o log do daemon conferido à parte com `grep -F -f .dev/variacoes.txt` (0).
  - `dev/conteiner-units/dentro.sh`: V4.38.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **V4.9 (sftp) e V4.12 (psql):**
  - O `make teste-home-real` reprovou neste worktree: os serviços de pé são de outro worktree (outras chaves de host e de login), e o git do worktree abre `.git/worktrees/…` no home.
  - Por isso nenhum cliente ssh/sftp/curl foi rodado.
  - Não há `psql` no host.
  - Os dois ficam para o validador, com os serviços deste checkout. Para o V4.9, o cliente tem de entrar no `home-real.sh` (D56/V4.37 é da Fase 27).
- **V4.13 (RA1):** sem mitigação genérica. O `execve` volta o `dumpable` a 1, o namespace de usuário não muda o `PTRACE_MODE_READ` e o Yama só cobre o `ATTACH`. Reprova e vai ao usuário; o teste `ra1_…` prende o estado.
- **Túnel do banco (`acme-pg-tunel`, V4.29):** é da Fase 28. Hoje sai `entrega_falhou`, sem executar.
- **V4.14** (laço em `/proc/*/cmdline`) e **V4.16** (`find ~ /tmp`) são do auditor do M4. Pelo desenho, o segredo nunca vai em argv nem em arquivo fora de `<rt>`.
- **`revelar` do V4.38:** o cliente mínimo não manda `revelar`. Conferi o papel agente (no `estado`) e a linha do journal, que basta, porque `revelar` é só do app. Fica para o validador se ele quiser a resposta literal "não autorizado".
- **V4.21 "sem destravar sai ≠0 em 120 s ± 2 s":** não foi cronometrado aqui (são 2 min). O motor é o da Fase 23 (prazo de D20).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-servicos`, `make dev-cofre` | 0 |
| `cargo test -p ward-plataforma --lib filho` | 0 |
| `cargo test -p wardd --all-features --test run` | 0 |
| `cargo test -p ward --all-features` | 0 |
| `make dev-daemon` + `make teste-cli` (0 falha) + `make dev-daemon-parar` | 0 |
| `make teste-units` (com V4.38) | 0 |
| `make teste-home-real` | 2 (ambiente do worktree; ver Ficou de fora) |
| `make fmt` | 0 |
| `make check` | 2 nas duas primeiras rodadas (`v3_33` sensível a carga, load ~20; ver Problemas); 0 na última |

## Problemas descobertos
- **RA1 / V4.13:** o `/proc/<pid>/environ` do filho e o anexo 0600 são legíveis por qualquer processo do mesmo uid enquanto o comando roda. Não há mitigação genérica; a decisão é do usuário.
- **Ambiente da CLI muda o que o comando aprovado faz:** `BASH_ENV`, `PATH`, `NODE_OPTIONS`, `PYTHONSTARTUP`, `GIT_SSH_COMMAND` e outros vêm da CLI (D13 manda copiar, menos `LD_*`). Um agente pode pedir aprovação de `psql …` com `PATH` ou `BASH_ENV` próprios, e a janela de aprovação não mostra nem o ambiente nem o cwd. O escopo não pede filtrar nada além de `LD_*`. Registro para o usuário, sem escolher lado.
- **Comportamento novo para V4.21:** com o cofre travado e sem app, o daemon de produção agora espera até 2 min pelo `ward destravar`. A Fase 23 respondia `travado` na hora. Mudei pelo V4.21 e pelo D16; o `Daemon::novo` dos testes antigos (v3_10) segue respondendo `travado`.
- **O grupo inteiro morre no fim:** processo em segundo plano do comando (`cmd &`) morre quando o líder sai. Isso é intencional (anexo e segredo não sobrevivem ao comando), mas difere de um shell comum.
- **Neto com `setsid`:** sai do grupo e não é morto. As threads de leitura dele param por `poll` com prazo de 2 s, e a sessão termina mesmo assim. O neto segue vivo com o segredo no ambiente, como em RA1.
- **Travar durante um `ward run`** não mata o comando em andamento. O escopo só exige isso para o agente do `ward ssh` (D12, Fase 27).
- **`<rt>` no release:** é `/run/user/<uid>/ward/`, que só existe com a sessão do logind. No contêiner de units (sem logind), o `wardd` loga "pasta de temporários do ward run indisponível (permission denied)" e um `ward run` com anexo ou chave sairia `entrega_falhou`. Na máquina real, com sessão, existe. Vale decidir se o `wardd` cria `/run/user/<uid>` (não pode: é do root) ou se a unit ganha `RuntimeDirectory=`.
- **`v3_33_kill_9_entre_as_senhas_erradas` (Fase 22b) é sensível a carga.** Numa rodada do `make check`, a primeira espera deu 1,005 s contra o limite de 900 ms. Com load average ~20 (testes de outros worktrees a 976% de CPU), chegou a 1,1–1,4 s também rodado sozinho, e às vezes passa. O caminho do `destravar` não passa por código desta fase; o tempo é do Argon2 sob carga. Não é desta fase: só registro.
- **Worktree:** o `make teste-home-real` acusa o git do worktree (abre `…/.git/worktrees/<nome>/HEAD`, fora da lista permitida) e não loga nos serviços de outro checkout. É do ambiente, não desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/filho.rs` | criado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-core/src/protocolo/erros.rs` | alterado |
| `crates/wardd/src/execucao.rs` | criado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado (gancho de 7 linhas) |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/aprovacao.rs` | alterado |
| `crates/wardd/src/config.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/tests/run.rs` | criado |
| `crates/wardd/examples/cliente-teste.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/cliente.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado |
| `dev/testes/cli.py` | alterado |
| `dev/testes/units.sh` | alterado |
| `dev/conteiner-units/dentro.sh` | alterado |
| `Makefile` | alterado (`teste-units` compila e passa o `ward`) |
| `CLAUDE.md` | alterado (linhas de `teste-units` e `teste-cli`) |
| `docs/mission/handoffs/phase-26.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V4.13 só como registro do RA1)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
