# Fase 26a — Ambiente do `executar` por lista fixa (achado médio da auditoria do M3)

## Implementado
- **Ambiente do filho do `ward run` montado do zero no daemon** (`crates/wardd/src/execucao.rs`, `montar_ambiente`):
  - antes, o filho recebia o ambiente inteiro da CLI menos `LD_*`; agora nada da CLI passa fora da lista;
  - o comando sem `/` é procurado só no `PATH` fixo (o `Comando` do `ward-plataforma` resolve pelo `PATH` do ambiente do filho, que agora é sempre o fixo); com `/`, vale o caminho, relativo ao diretório de trabalho;
  - o diretório de trabalho continua o da CLI;
  - a entrega (D10) vem por cima, como antes. Se ela trouxer `PATH`, `HOME`, `USER` ou `LOGNAME`, o `executar` falha fechado com `entrega_falhou` (o D42 já impede; é a segunda camada);
  - o log (`trace`) só conta quantas variáveis da CLI ficaram de fora, sem nome nem valor.
- **Lista final e o porquê de cada variável:**

  | Variável | Origem | Por quê |
  | --- | --- | --- |
  | `PATH` | fixo `/usr/local/bin:/usr/bin:/bin` | Pedido da fase. O `PATH` da CLI escolheria outro binário para o mesmo nome aprovado (`psql`, `git`, `sh`). Comando fora dele vai com caminho absoluto |
  | `HOME` | passwd (`getpwuid_r`), nunca a CLI | O `HOME` da CLI faria git, psql, ssh, npm e outros lerem configuração de uma pasta do atacante (`.gitconfig` com `core.sshCommand`/`core.fsmonitor`, `.psqlrc` com `\!`), sem aparecer na janela. Com o do passwd, as ferramentas leem o que leriam numa sessão normal do usuário |
  | `USER`, `LOGNAME` | passwd | Mesmo motivo: o nome é do uid, não da CLI. Algumas ferramentas usam esse nome como padrão (ex.: o usuário padrão do `psql`) |
  | `LANG`, `LC_ALL`, `LC_ADDRESS`, `LC_COLLATE`, `LC_CTYPE`, `LC_IDENTIFICATION`, `LC_MEASUREMENT`, `LC_MESSAGES`, `LC_MONETARY`, `LC_NAME`, `LC_NUMERIC`, `LC_PAPER`, `LC_TELEPHONE`, `LC_TIME` | CLI, valor conferido por `ward_plataforma::lancador::locale_valido` (o de D57) | O locale muda mensagens e formatos e é preciso para o UTF-8 (handoff 22a, RA3). **Risco avaliado:** a glibc aceita caminho absoluto como nome de locale (`LC_ALL=/tmp/x` carrega os arquivos de lá) e, fora de setuid, não barra `..`. Por isso só passa `[A-Za-z0-9_.@-]{1,64}`, sem começar por `.` nem ter `..`. `LC_*` só com os 13 nomes da glibc (`LC_TERMINAL` e afins ficam de fora). `LOCPATH`, `LANGUAGE`, `NLSPATH` e `GCONV_PATH` não passam |
  | `TERM`, `COLORTERM` | CLI, `[A-Za-z0-9._+-]{1,64}` sem começar por `.` | Cor e largura de saída. **Risco avaliado:** o ncurses busca o terminfo pelo nome em `TERMINFO`, `TERMINFO_DIRS` (que não passam), `~/.terminfo` (do `HOME` do passwd) e nas pastas do sistema. Sem `/` no nome não há caminho, e terminfo é dado, não código. A saída do `ward run` é pipe, então quase nada usa o `TERM` |
  | `TZ` | CLI, nome de zona `:?[A-Za-z0-9_+-]+(/[A-Za-z0-9_+-]+)*`, até 64 | **Risco avaliado:** a glibc lê `TZ=/caminho` ou `:/caminho` como arquivo, e um nome relativo com `../` sai do `zoneinfo` (só o modo setuid barra). Sem `.` e sem `/` no começo, o nome fica dentro de `/usr/share/zoneinfo` (`TZDIR` não passa). Fica de fora a forma POSIX completa (`BRT3,M10.2.0`, `<-03>3`). Na falta do `TZ`, vale o `/etc/localtime`, que é o que o usuário já tem |
  | `NO_COLOR` | CLI, `[A-Za-z0-9]{1,64}` | Só um sinal para desligar cor |
  | Entrega (D10) | daemon | Como antes, por cima do que veio da CLI com o mesmo nome |

  Tudo o mais some, entre outros: `LD_*`, `BASH_ENV`, `ENV`, `BASH_FUNC_*`, `SHELLOPTS`, `PS4`, `IFS`, `NODE_OPTIONS`, `NODE_PATH`, `PYTHONPATH`, `PYTHONSTARTUP`, `PYTHONHOME`, `PERL5LIB`, `PERL5OPT`, `RUBYOPT`, `GIT_*`, `SSH_*` (inclusive o `SSH_AUTH_SOCK` do usuário), `PG*` não entregues, `PSQLRC`, `TMPDIR`, `XDG_*`, `PAGER`, `EDITOR`, `TERMINFO*`, `TZDIR` e `WARD_*` da CLI. Valor fora da forma faz a variável sair em silêncio (falha fechada, sem erro), e o ajuste de `LC_CTYPE=C.UTF-8` (22a) continua.
- **Diretório de trabalho na janela de aprovação:**
  - antes, o pedido não mostrava o cwd;
  - `PedidoDeAprovacao` (protocolo, `ward-core`) ganhou `diretorio: String`, obrigatório no JSON e com até `MAX_TEXTO_EXIBIDO`;
  - o daemon (`uso.rs`, `descrever`) preenche no `executar` com `exibir_argumentos(&[cwd])`, entre aspas e com escape visível (D51); nas outras operações vai vazio (o `ssh` roda na pasta pessoal do passwd, fixa);
  - o app (`app/nucleo/src/aprovacao.rs`) neutraliza de novo e manda `diretorio` no `PedidoExibido`;
  - a `JanelaAprovacao.svelte` mostra uma `CaixaDeCodigo` "Diretório de trabalho" entre "Comando" e "Quem pediu", só quando o pedido traz um;
  - a tabela operação × papel (V3.15) não muda, porque só a forma de uma resposta do papel app cresceu.
- **`ward-plataforma`:**
  - `sys::conta_do_uid` (nome e home pelo `getpwuid_r`; o `home_do_uid` passou a usá-la, e o `// SAFETY:` foi atualizado para `pw_name`);
  - `conta_do_usuario()` público;
  - `lancador::locale_valido` público.
- **`ward ssh` e salto/túnel:** já montavam o ambiente do zero (`ssh.rs`: `PATH=/usr/bin:/bin`, `LANG=C.UTF-8`, `TERM` fixo do pty, `SSHPASS`; `ponte.rs` igual), e o pedido `ssh` não carrega ambiente da CLI. Não precisou mudar nada; os testes de ambiente de `crates/wardd/tests/ssh.rs` continuam passando. O túnel do `banco` no `ward run` usa a ponte, com ambiente próprio, e o comando (`psql`) recebe o ambiente novo.
- **CLI:** a ajuda do `ward run` explica o diretório atual, o `PATH` fixo (comando fora dele vai com caminho absoluto) e a lista. A CLI continua mandando o ambiente dela menos `LD_*`; quem filtra é o daemon (V4.36 e o protocolo ficam iguais).
- **Doc:** a linha do `make teste-cli` no `CLAUDE.md`.

## Segurança
- **Invariantes tocadas:**
  - **S3/D13:** o filho recebe só o ambiente montado. O segredo continua só na entrega.
  - **S8/D60:** nada do ambiente do daemon chega ao filho, nem da CLI fora da lista.
  - **S10/D51:** a janela mostra o diretório exato, com escape visível, e o agente não esconde onde o comando roda.
  - **S13:** o log só leva a contagem.
- **Testes de ataque:**
  - `crates/wardd/tests/run.rs` › `f26a_ataque_ambiente_da_cli_nao_muda_o_comando`. Monta uma pasta do atacante na frente do `PATH`, com `true`, `sh`, `env`, `id` e `wardfalso-26a` falsos que deixam uma marca, e passa `BASH_ENV`/`ENV` para um rc que marca, `NODE_OPTIONS=--require`, `GIT_SSH_COMMAND`, `PYTHONSTARTUP`, `PYTHONPATH`, `PERL5LIB`, `PGSERVICEFILE`, `SSH_AUTH_SOCK`, `HOME`/`USER` do atacante, `LOCPATH`, `LANG=/caminho`, `TZ=../..`, `TERM=../x` e `TERMINFO`. O resultado:
    - `true`, `bash -c`, `sh -c` e `sh -i -c` rodam os do sistema e a marca nunca aparece;
    - `wardfalso-26a` dá 127;
    - o `env` do filho tem só `PATH` fixo, `HOME`/`USER`/`LOGNAME` do passwd, `COLORTERM`, `NO_COLOR`, `LC_CTYPE` e `WARD_*`;
    - valores válidos de `TZ`/`TERM`/`LANG` passam;
    - o `pwd` é o da CLI;
    - o log não cita o atacante.
  - `crates/wardd/tests/run.rs` › `v4_entrega_mascara_codigo_e_recusas` (ajustado): `DA_CLI` não chega mais ao filho e o `PATH` é o fixo.
  - `crates/wardd/src/execucao.rs` › `ataque_so_a_lista_passa_da_cli` (45 nomes perigosos recusados, válidos aceitos) e `ataque_valor_que_vira_caminho_sai` (locale, terminfo e fuso como caminho, `..`, vazio, longo, forma POSIX com `,`/`.`, `$(id)`).
  - `crates/wardd/tests/aprovacao.rs` › `f26a_diretorio_de_trabalho_vai_a_janela_com_escape_visivel`: um cwd com U+202E, ESC e espaço chega à janela entre aspas e com escape.
  - `app/nucleo/src/aprovacao.rs` › `ataque_diretorio_com_bidi_vira_escape_visivel`.
  - `dev/testes/cli.py` (`make teste-cli`), seção "Fase 26a": o mesmo ataque pela CLI real contra o daemon de teste, com pasta do atacante em `.dev/` (apagada no fim). Ficaram 7 checagens novas, todas ok.
- **Dependências novas:** nenhuma.
- **Risco residual (documentado, não corrigido):**
  - o diretório de trabalho é da CLI. Um comando como `git` roda na pasta que o agente escolheu e lê o `.git/config` de lá (`core.fsmonitor`, hooks). A janela mostra o diretório, mas não o que há nele; quem aprova precisa olhar o diretório;
  - com `HOME` do passwd, as configurações do usuário (`~/.gitconfig`, `~/.psqlrc`, `~/.pgpass`) valem. Um processo do mesmo uid pode escrever nelas, o que já está no modelo de ameaça (pacote malicioso) e não é novo.

## Ficou de fora
- **Roteiro da skill** ("comando fora do `PATH` fixo vai com caminho absoluto"): a skill `skill/ward/` ainda não existe (Fase 50, em paralelo). Está escrito na ajuda da CLI; ver Problemas.
- **Filtro também na CLI** (mandar só a lista ao daemon): não foi feito, para não mudar o V4.36 (a variável de 800 KiB tem de dar o erro de 750 KiB) nem o protocolo. O daemon é a fronteira de confiança e filtra de qualquer jeito.
- **Frame do FigCraft:** não atualizado. Esta não é fase de design; a caixa nova reusa o componente `Caixa de código` existente, sem medida nova.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make build` | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 |
| `cargo test --all-features -p wardd --test run --test aprovacao --test ssh` | 0 |
| `cargo test -p ward-core -p ward-app-nucleo -p ward-plataforma` | 0 |
| `make gitleaks-binario` (binário copiado do repositório principal) | 0 |
| `make dev-servicos`, `make dev-cofre`, `make dev-variacoes` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-teste-26a make dev-daemon` | 0 |
| `WARD_DEV_SOCKET_DIR=… make teste-cli` (serviços de pé do repositório principal) | 0 (134 ok, 0 falhas) |
| `make app-front` | 0 |
| `PKG_CONFIG_PATH=… PKG_CONFIG_SYSROOT_DIR=… make app-check` (R32) | 0 |
| QA Playwright Python headless, `ward-ponte-teste --daemon-falso aprovacao` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **Chaves dos serviços por worktree:** os serviços de teste no ar foram montados com o `.dev/servicos/` do repositório principal. Com chaves geradas no worktree, o `teste-cli` falha em V4.24/V4.25 (publickey). Resolvi copiando as chaves falsas de `/home/jeffersonoliveira/projetos/cofre/.dev/servicos/` e recriando o cofre de teste. Vale registrar no procedimento dos workers em paralelo.
- **PRD D13** ainda diz "a CLI manda… o ambiente dela sem `LD_*`" e não descreve a lista. **D60** diz "o `ward run` monta o do filho a partir do da CLI". Os dois precisam passar a apontar para a lista desta fase. Não editei o PRD.
- **Fase 50 (skill):** o roteiro deve dizer que o `ward run` usa `PATH=/usr/local/bin:/usr/bin:/bin`, que ferramenta do nvm, do `~/.cargo/bin` ou do `~/.local/bin` vai com caminho absoluto, e que variáveis do usuário (ex.: `PGHOST`, `AWS_PROFILE`) não chegam ao comando; o que for preciso vem da entrada ou de `env VAR=… comando` no argv (aparece na janela).
- **Design:** o frame `Aprovação / *` do FigCraft (16701…17035) não tem a caixa "Diretório de trabalho". Uma fase de design deve acrescentá-la para o validador de UI não acusar divergência.
- **Contrato:** V4.3–V4.8 continuam valendo, já que a entrega não mudou. Falta uma asserção (SEG) do contrato para esta fase (lista do ambiente e `PATH` fixo); sugiro acrescentar uma V4.x com o ataque de `make teste-cli`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado (teste) |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/lancador.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `app/nucleo/src/aprovacao.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado (teste) |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src/telas/JanelaAprovacao.svelte` | alterado |
| `app/src/telas/aprovacao.js` | alterado |
| `dev/testes/cli.py` | alterado |
| `CLAUDE.md` | alterado |
| `docs/mission/evidence/fase-26a/aprovacao-normal-diretorio.png` | criado |
| `docs/mission/evidence/fase-26a/teste-cli.log` | criado |
| `docs/mission/handoffs/phase-26a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI) Conferido no Chrome headless (Playwright Python) com a ponte de teste; o frame do FigCraft ainda não tem a caixa nova (ver Problemas)
- [x] Ficou dentro do escopo da fase
