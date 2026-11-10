# Fase 54b — Entrada padrão na aprovação

## Implementado
- **Protocolo** (`ward-core`): novo `TipoEntradaPadrao` (`terminal`, `nula`, `pipe`,
  `arquivo`, `outra`; `oculta()` = não terminal nem nula). Campo **obrigatório**
  `tipo_entrada` em `executar` e `ssh`. `PedidoDeAprovacao.entrada_oculta` (lógico
  obrigatório, só em executar/ssh e nunca numa confirmação de origem).
  `ResponderAprovacao.entrada_confirmada` (ausente = falso, só `true` aceito como
  valor lógico; só com decisão que pede PIN).
- **CLI** (`crates/ward/src/comandos.rs`): `tipo_da_entrada_padrao()` faz `fstat` de
  uma cópia do fd 0 e classifica: fd fechado ou `/dev/null` (char 1:3) = nula, tty =
  terminal, FIFO/socket = pipe, arquivo regular = arquivo, resto (`/dev/zero`) =
  outra. `ward run` e `ward ssh` mandam o tipo; com `nula` a CLI não lê nada da
  entrada. Mensagem de travado: `` `/usr/local/bin/ward destravar` `` (da 54a).
- **Daemon** (`wardd`):
  - `uso.rs`: o pedido "com aprovação" com entrada oculta vai à janela com
    `entrada_oculta`; um "Permitir" sem `entrada_confirmada` (ou com ela num pedido
    sem entrada oculta) é `pedido_invalido`, antes do PIN (não conta falha) e o pedido
    segue pendente. A liberação de 1 hora não cobre entrada oculta. Entrada livre:
    sem mudança.
  - `execucao.rs`/`ssh.rs`: `nula` é cumprida. Sem pty, a entrada do filho fecha no
    lançamento; um `entrada_padrao` com dados mata o grupo (137), sem o dado chegar.
    Log `depurar!` só com o nome do tipo (para a suíte da CLI).
  - `aprovacao.rs`: `e_entrada_oculta(id)`.
  - `aprovador-teste`: modo `uma_vez_ciente <PIN>`; o log ganha ` entrada_oculta=1`
    só quando verdadeiro.
- **App**:
  - `app/nucleo` (`servico.rs`, `cliente.rs`, `aprovacao.rs`, `daemon_falso.rs`,
    `comandos.rs`): `responder_aprovacao(id, decisao, pin, ciente)`. O serviço recusa
    `ciente` sem PIN e, com PIN, `ciente` diferente de `entrada_oculta`. O daemon falso
    confere igual ao `wardd` e tem o modo simulado `aprovacao-entrada`.
  - `app/src-tauri/src/main.rs`: o comando aceita `ciente: Option<bool>`.
  - `ward-ponte-teste`: `ciente` lógico só com PIN; o modo `aprovacao-entrada` está no USO.
  - `JanelaAprovacao.svelte` e `aprovacao.js`: com `entrada_oculta` a janela mostra
    `Destaque` aviso ("O comando recebe dados pela entrada padrão, que esta janela não
    mostra. …") e `Interruptor` `aprovacao-entrada` ("Sei o que vai pela entrada padrão
    e permito mesmo assim"). Os três "Permitir" ficam desabilitados até ele ser ligado.
    A caixa desliga a cada pedido novo, e a resposta leva `ciente`. Conferido no
    Chrome headless pela ponte (`--daemon-falso aprovacao-entrada`, `&tela=aprovacao`):
    "Permitir uma vez" fica desabilitado com o PIN digitado e a caixa desligada,
    habilita com ela ligada e responde. Screenshots em
    `docs/mission/evidence/fase-54b/`.
- **Skill**:
  - `skill/settings-deny.json` ganha `Bash(*ward*run*<*)`, `Bash(*<*ward*run*)` e
    `Bash(*|*ward*run*)`. O `<<<` já existia.
  - `skill/ward/SKILL.md`: "Nada pela entrada padrão do `ward run`", com o script como
    arquivo no argv; nota no sftp.
  - `skill/README.md`: a regra nova e o limite do modelo por subcomando.
  - `dev/testes/skill.py`: 11 ataques novos em `ATAQUES_BASH`, cobrindo `<`, `0<`,
    `<` antes do `ward`, here-doc, `<<<` e `<(…)`. A lista nova `ATAQUES_BASH_INTEIRO`
    tem 5 casos de pipe, exigidos só no comando inteiro. O autoteste tira cada regra
    nova. Ataque `od` sem `<`, para a regra de `od` seguir provada sozinha.
- **Docs**: o PRD ganha D68 (decisão e mecanismo) e RA11 (proposta: "terminal" não é
  conferível). No contrato, V4.22 passa a exigir a dica `/usr/local/bin/ward destravar`,
  e entram V4.39 e V4.40 (SEG, desmarcadas para o validador).
- **Suítes Python**: `dev/testes/cli.py` confere a dica do V4.22 e o tipo que a CLI real
  manda (`/dev/null`, pipe, arquivo, `/dev/zero`, pty), pelo log `trace` do daemon de
  teste. `dev/testes/socket_matriz.py` manda `tipo_entrada`.

## Segurança
- **Invariantes tocadas**:
  - S9: quem decide é o daemon. A caixa da janela é só a interface, e o `wardd` recusa
    a resposta sem `entrada_confirmada`.
  - S1: papéis sem mudança. `entrada_confirmada` só vale no `responder_aprovacao` do
    papel app.
  - S4 e S6: o log tem só o nome do tipo, nunca o conteúdo da entrada. Os testes
    conferem que o script e os segredos não aparecem no log.
- **Testes de ataque escritos**:
  - `crates/wardd/tests/run.rs`:
    - `f54b_ataque_script_por_pipe_na_aprovacao` roda de ponta a ponta com o
      `wardd-aprovador-teste` aberto pelo daemon. O script do `python3` por pipe e os
      comandos para o `sh` por pipe e por arquivo dão `pedido_invalido`, nada roda e
      nada sai. Com `uma_vez_ciente` roda. Com terminal, "Permitir uma vez" passa como
      antes.
    - `f54b_ataque_entrada_nula_com_dado_mata_o_comando`.
  - `crates/wardd/tests/aprovacao.rs`:
    - `f54b_ataque_pipe_na_aprovacao_sem_a_caixa_nao_passa`
    - `f54b_terminal_e_nula_passam_como_antes`
    - `f54b_liberacao_de_1_hora_nao_cobre_pipe`
    - `f54b_ssh_com_pipe_avisa`
  - `crates/ward-core/src/protocolo/testes.rs`:
    - `ataque_tipo_da_entrada_padrao_fora_do_formato`
    - asserções novas em `ataque_pedido_de_aprovacao_com_origem_fora_da_forma`
  - `app/nucleo/src/aprovacao.rs`: `ataque_entrada_oculta_so_com_a_caixa` (interface
    forjada; o daemon falso também recusa).
  - `crates/ward/src/testes.rs`:
    - `f54b_tipo_da_entrada_padrao_pelo_fstat`
    - `f54b_run_e_ssh_mandam_o_tipo_e_nula_nao_le`
  - `dev/testes/skill.py`: 16 ataques de deny e 3 do autoteste.
- **Dependências novas**: nenhuma.

## Ficou de fora
- **Frame no FigCraft** para a janela com o aviso e o interruptor. A Fase 54d edita o
  arquivo em paralelo e a ordem foi não mexer nele. A tela usa só componentes que já
  existem (`Destaque` aviso, `Interruptor`), e o texto está marcado no código como "sem
  frame no FigCraft ainda".
- **`make app-check`** (build de release do Tauri) não rodou: o disco estava com cerca
  de 4,5 GB livres, e só existe o target debug do app. No `app/src-tauri` em debug,
  `clippy -D warnings`, `cargo test` (21 passaram) e `fmt --check` passaram. O núcleo
  (`ward-app-nucleo`, 186 testes) e o `make app-front` também passaram.
- **`psql` real**: não há `psql` no host. O ataque por pipe usa `sh` como leitor de
  comandos pela entrada (a decisão não olha o argv). O `teste-cli` roda sem a ponte,
  então o "com aprovação" responde "aprovação indisponível" de qualquer forma.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p wardd --all-features --test run f54b` | 0 |
| `cargo test -p wardd --all-features --test aprovacao` | 0 |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `cargo test -p ward` / `-p ward-core` | 0 |
| `make app-front` | 0 |
| QA Playwright Python headless (`ward-ponte-teste --daemon-falso aprovacao-entrada`, Chrome `/opt/google/chrome`) | 0 |
| `make check` | 0 |
| `make teste-skill` | 0 |
| `make dev-servicos-subir`; `make dev-daemon WARD_DEV_APP=` (socket `/run/user/1000/ward-54b`) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54b make teste-cli` (0 falhas; V4.22 e os 5 tipos da 54b ok) | 0 |
| `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo fmt --check` em `app/src-tauri` (debug) | 0 |
| `make dev-daemon-parar`, `make dev-servicos-parar` | 0 |
| `git checkout -- docs/mission/evidence/fase-4[123]` | 0 |

## Problemas descobertos
- **RA11 (proposta, aguarda o usuário)**: `tipo_entrada: terminal` é declarado pelo
  cliente do mesmo uid. Escapam do aviso um cliente de socket que mente e um agente que
  cria um pty (`script -c`, `pty.spawn`) e escreve o script nele. A fase fecha o caso da
  auditoria e o erro sem má intenção, mas não o agente que mira o Ward. Alternativa:
  avisar em toda entrada que não seja `nula`, tty incluído.
- **Deny por subcomando**: se o Claude Code olhar cada subcomando, `printf … | ward
  run …` vira dois comandos inocentes. Nenhuma regra por texto pega isso; quem barra é
  o daemon. Está documentado em `skill/README.md`.
- **`docs/escopo.md` › "Janela de aprovação"** não fala da entrada padrão. D68 está no
  PRD, e o escopo não foi editado (é fonte de verdade do usuário). Falta uma linha ali.
- **Design**: falta o frame `Aprovação / entrada padrão` (aviso e interruptor) na
  Layout do FigCraft. Fica para uma fase de DESIGN depois da 54d.
- `docs/mission/handoffs/phase-54.md` já estava modificado no checkout antes desta fase
  e não foi tocado por ela.
- Disco: `target/debug/incremental` foi apagado (permitido). Restam cerca de 4 GB livres.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado |
| `crates/wardd/src/aprovacao.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/aprovador-teste/main.rs` | alterado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/wardd/tests/nucleo.rs` | alterado |
| `crates/wardd/tests/socket.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `crates/wardd/tests/trava.rs` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `app/nucleo/src/aprovacao.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/telas/JanelaAprovacao.svelte` | alterado |
| `app/src/telas/aprovacao.js` | alterado |
| `skill/settings-deny.json` | alterado |
| `skill/ward/SKILL.md` | alterado |
| `skill/README.md` | alterado |
| `dev/testes/skill.py` | alterado |
| `dev/testes/cli.py` | alterado |
| `dev/testes/socket_matriz.py` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/validation-contract.md` | alterado |
| `docs/mission/evidence/fase-54b/aprovacao-entrada-desmarcada.png` | criado |
| `docs/mission/evidence/fase-54b/aprovacao-entrada-marcada.png` | criado |
| `docs/mission/handoffs/phase-54b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não há frame para o estado novo (ver
  Problemas). Conferido no Chrome com os componentes da Design System.
- [x] Ficou dentro do escopo da fase
