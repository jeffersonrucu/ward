# Fase 24a — Correções do scrutiny do M3

## Implementado
- **V3.25 (reprovada no scrutiny)**: `nome_para_mensagem`
  (`crates/ward-plataforma/src/endurecimento.rs`) agora devolve só o prefixo do nome
  até o primeiro byte fora de `[A-Za-z0-9_]` (exclusive), com no máximo 64 caracteres;
  nada é trocado (`?`) nem acrescentado (`…`). Vale também para a mensagem do lançador
  (`ErroSessao::AMais`), que usa a mesma função (o teste dele passou de `"X??2J"` para
  `"X"`).
- **Vaga reservada ao app** (`crates/wardd/src/servidor.rs`): `Vagas::reservar` recebe o
  papel. O papel app tem vagas à parte (`MAX_CONEXOES_APP = 8`), fora da conta e do
  despejo das 64 de agente/extensão: uma enxurrada de conexões caladas de agente não
  derruba mais a conexão do app (assinante de avisos, que fica ociosa por desenho), e
  64 agentes ocupados não tiram a vaga do app. App cheio recusa com `ocupado` e não
  despeja ninguém. Só o processo aberto pelo lançador é app (D63), então ninguém mais
  enche essas vagas.
- **V3.3/V3.4 no repositório**: `docs/mission/evidence/M3/{ataque,matriz}.py` viraram
  `dev/testes/socket_ataque.py` e `dev/testes/socket_matriz.py`, com asserções (o
  original só imprimia), e o executor `dev/testes/socket-ataques.sh`, ligado no alvo
  novo `make teste-socket-ataques` (fora do `make check`; `CONEXOES=<n>` troca as
  10 000). O executor sobe um `wardd` de teste **só dele**: cópia do cofre de teste em
  `.dev/socket-ataques.XXXXXX/` e socket em `$XDG_RUNTIME_DIR/ward-socket-ataques-…`
  por `WARD_DEV_SOCKET_DIR`; só mata o pid que criou. Confere:
  - V3.4: quadro de 2 MiB, tamanho `0xFFFFFFFF`, negativo com corpo, tamanho 0, JSON
    inválido, `{}`, UTF-8 inválido, versão errada, operação desconhecida (cada um com
    erro, e os de tamanho fecham a conexão), quadro cortado (fecha), 10 000 conexões
    caladas com `connect` bloqueante (todas aceitas; no fim só ≤ 129 abertas; cliente
    normal atendido antes, durante e depois; daemon vivo);
  - V3.3: matriz operação (lida da tabela `Operacao` de `protocolo.rs`, para não ficar
    velha) × as 22 entradas das fixtures + uma inexistente × com e sem `"revelar":
    true`/`"campo": "senha"` no topo e nos dados × sem dados: nenhuma resposta com
    padrão de `.dev/variacoes.txt`, nenhuma de tipo com credencial, toda operação só de
    app/extensão responde `nao_autorizado`, a bloqueada fora da lista; mais `executar`
    válido (`env`) com cada entrada: saída mascarada sem padrão da lista e a bloqueada
    byte a byte igual à inexistente;
  - log do daemon (`RUST_LOG=trace`) sem padrão de `.dev/variacoes.txt`.
  A versão curta (70 conexões) segue no `cargo test` (`v3_20_ciclo_pelo_socket…`).
- **V3.6 no binário**: `crates/wardd/tests/socket.rs › v3_6_wardd_no_ar_nega_proc_ptrace_e_core`
  sobe o `wardd` de teste, destrava (segredo em memória) e confere: `/proc/<pid>/mem`,
  `environ` e `maps` negados a outro processo do uid (`/usr/bin/cat`) e ao próprio pai;
  `strace -p` falha; `gdb -p` falha por permissão (se houver gdb; aqui não há, o teste
  avisa); `/proc/<pid>/limits` com core `0 0`; `kill -SEGV` com o cofre destravado mata
  sem core (status sem core, nenhum `core*` no diretório de trabalho). Controle: o mesmo
  `cat` lê o `environ` de um processo comum.
- **Ganchos de teste fora do release**: `Daemon::com_gancho_depois_de_gravar` e
  `Daemon::com_programas_ssh` só existem com a feature nova `ganchos-teste` do `wardd`
  (`ganchos-teste = ["modo-teste"]`, então o artefato com ela é recusado como o de
  teste, V9.2, e o `make teste-sem-modo-teste` acusa quem a ligar por dependência
  normal). `tests/ssh.rs` inteiro e `tests/d62.rs › v3_35_…` exigem a feature (o
  `make test` usa `--all-features`). Conferido à mão: com só `modo-teste`, um teste que
  chama os dois dá `E0599 no method named com_gancho_depois_de_gravar`.
- **V3.15**: `tests/aprovacao.rs › v3_15_sem_aprovador_no_binario` percorre `src/`
  inteiro (subpastas, recusa link), aceita `ganchos-teste` só em `nucleo.rs` e só logo
  antes do `pub fn` de um dos dois ganchos, exige que os dois tenham a feature e que
  sejam exatamente esses dois; `env::var` em subpasta é recusado.
- **`teste-dev-cofre` com trava**: `dev/testes/dev-cofre.sh` pega um `flock` em
  `.dev/.teste-dev-cofre.trava` (0600, recusa link); a 2ª rodada avisa e espera a 1ª
  (até 15 min).
- `plan.md`: tirada a linha duplicada "`ward-plataforma`, ajustes da Fase 21:".
- `CLAUDE.md`: linha do `make teste-socket-ataques` e a trava do `teste-dev-cofre`.

## Segurança
- Invariantes tocadas:
  - S8 (V3.25, V3.6): mensagem de recusa só com bytes do nome em `[A-Za-z0-9_]`, sem
    valor; V3.6 provado no binário do daemon, com o cofre destravado.
  - S1 (V3.3): matriz pelo socket real, como agente; nenhum segredo e app/extensão
    `nao_autorizado`.
  - S5 (V3.4): o daemon recusa/fecha quadros ruins e aguenta 10 000 conexões caladas.
  - S10/S9 (aprovação): a conexão do app, que recebe os pedidos de aprovação, não é
    mais despejável por um agente que enche o socket (antes, uma enxurrada derrubava a
    janela, e o pedido pendente virava negado: negação de serviço da aprovação, sem
    vazamento). Nenhuma vaga nova para agente.
  - D8/V3.15: os ganchos de teste não existem no release; o V3.15 os aceita só no
    lugar exato.
- Testes de ataque escritos:
  - `crates/ward-plataforma/src/endurecimento.rs › ataque_nome_com_controle_nao_vai_cru_para_a_mensagem`
    (atualizado: `\x1b`, `\n`, `\xff`, `…` em UTF-8, 200 caracteres, exatamente 64,
    OSC de título, `\r\n` forjando linha)
  - `crates/ward-plataforma/tests/endurecimento.rs › ataque_v3_25_nome_forjado_sai_cortado`
    (processo real com `LD_FORJA\nwardd: destravado`, `LD_COR\x1b[31m…`, 200 caracteres)
  - `crates/ward-plataforma/src/lancador.rs › …` (o teste do nome com `\x1b[2J`, novo
    valor esperado)
  - `crates/wardd/src/servidor.rs › ataque_conexoes_de_agente_nao_despejam_o_app`
    (10 000 agentes calados com o app conectado; 64 agentes ocupados + agente e
    extensão novos recusados e o app ainda entra; app cheio não despeja ninguém)
  - `crates/wardd/tests/socket.rs › v3_6_wardd_no_ar_nega_proc_ptrace_e_core`
  - `crates/wardd/tests/aprovacao.rs › ataque_v3_15_gancho_sem_feature_ou_fora_do_lugar`
    (gancho sem feature, feature em outro item, em outro arquivo, feature desconhecida,
    `env::var` em subpasta, `modo-teste` em `uso.rs`: todos recusados)
  - `dev/testes/socket_ataque.py` e `dev/testes/socket_matriz.py` (`make teste-socket-ataques`)
  - `make teste-dev-cofre` rodado duas vezes ao mesmo tempo: a 2ª esperou a trava e as
    duas passaram (no scrutiny, a 2ª falhava com `ln: … Arquivo existe`).
- Dependências novas: nenhuma (feature nova `ganchos-teste` no `wardd`, sem crate nova;
  `Cargo.lock` igual). Os scripts usam só a biblioteca padrão do Python.

## Ficou de fora
- Teste pelo socket real da vaga do app (app de teste assinando e 10 000 agentes): o
  `wardd-app-teste` roda um roteiro fixo e sai; precisaria de um modo novo nele. O teste
  de unidade exercita o mesmo `Vagas::reservar` que o `servir` chama.
- Extensão (`ward-nm`) segue no grupo comum das 64 vagas e pode ser despejada quando
  ociosa (o host de native messaging abre uma conexão por mensagem; não foi pedido).
- `v3_33_kill_9_entre_as_senhas_erradas` sensível a tempo (pendência da 23a): fora
  desta fase.
- Conferência permanente de que os ganchos não compilam sem a feature (seria um
  `compile_fail` ou um crate temporário); fica a checagem estrutural do V3.15 e a
  conferência à mão registrada acima.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` | 0 |
| `make dev-variacoes` | 0 |
| `make teste-socket-ataques` (10 000 conexões) | 0 |
| `make teste-socket-ataques CONEXOES=200` (depois do `executar` na matriz) | 0 |
| `make teste-dev-cofre` ×2 ao mesmo tempo | 0 e 0 |
| `cargo clippy -p wardd --all-targets -- -D warnings` sem feature e só com `modo-teste` | 0 |
| `make check` | 0 |
| `make teste-sem-modo-teste` | 0 |
| `make teste-dev-cofre` (final) | 0 |
| `make teste-socket-ataques` (final, 10 000) | 0 |

## Problemas descobertos
- O primeiro `kill -SEGV` (mandado por `kill`, não por falha de verdade) não mata o
  `wardd`: cai no tratador de SIGSEGV do std do Rust (o de estouro de pilha), que volta
  o tratador ao padrão e retorna; o segundo mata. Sem core nos dois casos (dumpable 0).
  O teste manda de novo até morrer. Não é falha de segurança, mas o V3.6 do contrato
  fala em "`kill -SEGV`" no singular.
- Nesta máquina `fs.suid_dumpable = 2` e o `core_pattern` é um pipe (apport). O que
  impede o core é o `PR_SET_DUMPABLE=0` (o kernel nem chama o apport); o `RLIMIT_CORE=0`
  sozinho não bastaria com pipe. Vale registrar para a instalação (Milestone 9).
- `gdb` não está instalado aqui; o caso `gdb -p` do V3.6 só roda onde houver
  `/usr/bin/gdb` (o `strace -p` cobre o mesmo `ptrace`).
- Nome de variável que começa com byte fora de `[A-Za-z0-9_]` (só possível no
  lançador, que confere qualquer nome) sai vazio na mensagem
  (`variável  não é do ambiente do app`). Segue o contrato (só esses caracteres), mas a
  mensagem fica estranha; não troquei por marca para não reabrir o V3.25.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/endurecimento.rs` | alterado |
| `crates/ward-plataforma/src/lancador.rs` | alterado (valor esperado do teste) |
| `crates/ward-plataforma/tests/endurecimento.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado (feature `ganchos-teste`) |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/wardd/tests/d62.rs` | alterado |
| `crates/wardd/tests/socket.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `dev/testes/dev-cofre.sh` | alterado |
| `dev/testes/socket-ataques.sh` | criado |
| `dev/testes/socket_ataque.py` | criado |
| `dev/testes/socket_matriz.py` | criado |
| `Makefile` | alterado (`teste-socket-ataques`) |
| `CLAUDE.md` | alterado |
| `docs/mission/plan.md` | alterado (linha duplicada) |
| `docs/mission/handoffs/phase-24a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
