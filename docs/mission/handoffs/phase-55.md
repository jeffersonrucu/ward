# Fase 55 — Subcomandos da CLI (DT10)

## Implementado
- Renomes de subcomando na CLI `ward` (`crates/ward/src/args.rs`), sem alias (DT10, PRD regra 12):
  `lista` → `list` (com `--json`), `travar` → `lock`, `destravar` → `unlock`, `ajuda` → `help`
  (`-h`/`--help`/`-V`/`--version` iguais). Os quatro nomes antigos caem no braço
  `UnknownCommand`: saída 2, "comando desconhecido", antes de conectar.
- `ward --help` (`HELP`): só os três nomes trocados; o espaço depois do nome foi ajustado para a
  descrição ficar na mesma coluna (24). Conferido por script: a ajuda nova é igual a
  `dev/fixtures/legacy/help-0.1.4.txt` com as três trocas (mesmo texto pt-BR). `ward help` não
  entrou na ajuda (a 0.1.4 também não listava `ajuda`), para o V5.2 ficar só nos nomes.
- `UnexpectedArgument` passa a citar `list`/`lock`/`unlock`.
- Mensagens que citam subcomando (`crates/ward/src/commands.rs`): `MSG_RUN_LOCKED`, a dica de
  `ResponseError::Locked` (`/usr/local/bin/ward unlock`) e "`ward unlock` precisa de um terminal".
  O daemon (`wardd`) e o `app/core`/`app/src-tauri` não têm mensagem com subcomando da CLI
  (conferido por grep); só o texto do nível bloqueado no `app/src` cita `ward lista` (ver abaixo).
- App: "ela some do ward lista" → "ela some do ward list" em `app/src/lib/components/levels.js`
  (2) e `app/src/screens/credentials.js` (1), por ser texto que cita subcomando (nota do plano).
- Testes Rust (`crates/ward/src/args.rs`, `crates/ward/src/tests.rs`) com os nomes novos, mais
  testes de ataque novos (abaixo).
- Scripts que chamam a CLI: `dev/tests/cli.py` (todas as chamadas, as etiquetas e a dica
  `/usr/local/bin/ward unlock`, mais a seção nova `1c` para V5.1/V5.2), `dev/tests/app_e2e.py`,
  `dev/tests/extension.py`, `dev/tests/playwright.py`, `dev/tests/ssh_home.py`,
  `dev/container-platform/inside.sh` (V4.2 com `unlock list lock`) e
  `dev/container-install/inside.sh` (V9.3 com `ward unlock`).
- `CLAUDE.md`: a linha do `make test-cli` com `ward unlock` e `list --json`, e o que a seção nova
  confere.

### Literais alterados (PRD regra 5)
| Arquivo | Antigo → novo | Motivo |
| --- | --- | --- |
| `crates/ward/src/args.rs` | match `"ajuda"`/`"travar"`/`"destravar"`/`"lista"` → `"help"`/`"lock"`/`"unlock"`/`"list"`; `UnexpectedArgument("travar"/"destravar"/"lista")` → `"lock"/"unlock"/"list"` | nome de subcomando (DT10) |
| `crates/ward/src/args.rs` | `HELP`: `ward lista [--json]   `, `ward destravar        `, `ward travar           ` → `ward list [--json]    `, `ward unlock           `, `ward lock             ` | nome de subcomando; espaço mantém a coluna |
| `crates/ward/src/commands.rs` | `/usr/local/bin/ward destravar` (2×) e `` `ward destravar` `` → `unlock` | mensagem que cita subcomando |
| `app/src/lib/components/levels.js`, `app/src/screens/credentials.js` | "some do ward lista." → "some do ward list." | texto que cita subcomando |
| `crates/ward/src/tests.rs`, `dev/tests/*.py`, `dev/container-*/inside.sh` | argv e etiquetas com os nomes novos | uso do subcomando renomeado |
| `CLAUDE.md` | `ward destravar` → `ward unlock`, `lista --json` → `list --json` | doc operacional da suíte |

`allow.txt`/`frozen.txt`: nenhuma entrada nova; o `frozen.txt` não tinha entradas da Fase 55.

## Segurança
- Invariantes tocadas: S1/S3 (nenhum argumento ecoado; a senha digitada por engano depois de
  `unlock` ou do nome antigo não volta), S5/V4.2 (a CLI continua recusando servidor falso antes
  da senha e parando antes da senha com servidor que não fala v2: o `client.rs` e a ordem
  conectar → conferir → endurecer → ler senha não mudaram; o diff em `commands.rs` é só de
  literais). DT10: sem alias; nome antigo nunca conecta.
- Testes de ataque escritos (não rodados, regra 6):
  - `crates/ward/src/args.rs` › `tests::old_names_are_unknown_commands` (os quatro nomes antigos,
    com argumento e em outra caixa, dão `UnknownCommand`; a ajuda não cita nome antigo e cita os
    novos).
  - `crates/ward/src/args.rs` › `tests::attack_argument_not_echoed_in_message` (agora com `unlock`
    e com o nome antigo + senha).
  - `crates/ward/src/tests.rs` › `attack_old_subcommands_unknown_without_talking_to_the_daemon`
    (servidor ouvindo no socket de teste: cada nome antigo sai 2 com "comando desconhecido",
    nenhum evento — nem conectar, nem endurecer, nem ler senha —, zero conexões e zero bytes,
    a senha não aparece).
  - `crates/ward/src/tests.rs` › `v4_2_attack_*` e `daemon_missing` com os nomes novos.
  - `dev/tests/cli.py` › seção `1c` (m2:V5.1/V5.2): nomes antigos saem 2 sem conexão num socket
    próprio em `/run/user/<uid>/ward-cli-old-<pid>`, sem eco; `help`/`-h`/`--help` saem 0;
    `ward --help` = ajuda da 0.1.4 com só os nomes trocados; `--version` sai 0.
  - `dev/container-install/inside.sh`: o V9.3 com `ward unlock` agora também exige que a saída
    não seja "comando desconhecido" (sem a troca, o V9.3 passaria só porque o nome antigo
    sai 2, sem testar a recusa do servidor falso). O mesmo risco existia no V4.2 do
    `dev/container-platform/inside.sh`, que já tem conferência positiva da mensagem.
- Dependências novas: nenhuma.

## Ficou de fora
- Skill e deny (Fase 58): `skill/ward/SKILL.md` (linhas com `ward lista`, `ward travar`,
  `/usr/local/bin/ward destravar`), `skill/README.md`, `skill/settings-deny.json` (negar `ward unlock`
  em todas as formas e manter as antigas) e `dev/tests/skill.py` (`ALLOWED`, `UNLOCK`, exemplos
  `ward lista`/`ward travar`, lista de ataques com as duas formas). Até lá o `make test-skill` falha:
  ele confere que todo `ward <sub>` da skill está no `ward --help`.
- Documentação (Fase 59): `docs/migracao.md` (4 citações) junto com `dev/tests/migration-guide.py`
  (espera `/usr/local/bin/ward destravar` e `ward lista`; o `make test-migration-guide` precisa das
  duas mudanças juntas), `docs/instalacao.md` (5), `docs/escopo.md` (6), `README.md` (1),
  `.claude/agents/planner.md` (2) e `.claude/agents/user-testing-validator.md` (1).
- `ward --version` ainda diz 0.1.4 (DT19, Fase 60).
- `wardd-test-client` (`crates/wardd/examples/test-client.rs`) mantém os argumentos próprios
  `estado|lista|destravar` (documentados como nomes do cliente, não da CLI `ward`); são chamados por
  `dev/container-platform/inside.sh`, `dev/tests/units.sh` e `dev/tests/deb.sh`. Se for para traduzir,
  é uma fase com todos os chamadores (56 ou 60).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make compile` (log `.dev/logs/fase55-compile.log`) | 0 |
| `cargo fmt -p ward --check` (depois de `cargo fmt -p ward`) | 0 |
| `cargo clippy --locked -p ward --all-targets --features test-mode -- -D warnings` (log `.dev/logs/fase55-clippy.log`) | 0 |
| `make english-check PATHS="crates/ward dev/tests/cli.py dev/tests/app_e2e.py dev/tests/extension.py dev/tests/playwright.py dev/tests/ssh_home.py dev/container-install/inside.sh dev/container-platform/inside.sh app/src/lib/components/levels.js app/src/screens/credentials.js"` (log `.dev/logs/fase55-english-paths.log`) | 0 |
| `make english-check` (repositório inteiro; log `.dev/logs/fase55-english.log`) | 0 |

Nenhum teste rodou (PRD regra 6); nenhum daemon, cofre real ou daemon instalado foi tocado.

## Problemas descobertos
- Divergência PRD regra 2 × nota do plano: a regra 2 diz que nenhum texto de tela muda, e a nota da
  Fase 55 manda trocar as mensagens do app que citam subcomando. Segui o plano só no texto do nível
  bloqueado ("some do ward list"), que cita o subcomando. Efeitos: (1) o
  `docs/mission/design-nodes.md` (linha 854, `Nível=bloqueado`) e o `Ward.figcraft` continuam com
  "ward lista"; (2) a comparação de capturas da Fase 65 com `docs/mission/arquivo/m1/evidence/final/`
  vai mostrar essa diferença no diálogo de nível e no formulário. O orchestrator decide se o
  design-nodes acompanha (Fase 59) ou se o texto volta.
- Risco de merge: `app/src/lib/components/levels.js` e `app/src/screens/credentials.js` (Fase 54) e
  `dev/container-install/inside.sh` (Fase 57) têm hunks de uma a três linhas desta fase.
- O `ward --help` não cita `ward help` (como a 0.1.4 não citava `ajuda`); se o contrato quiser o
  subcomando listado, o V5.2 passa a ter uma linha a mais além dos nomes.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/commands.rs` | alterado |
| `crates/ward/src/tests.rs` | alterado |
| `app/src/lib/components/levels.js` | alterado |
| `app/src/screens/credentials.js` | alterado |
| `dev/tests/cli.py` | alterado |
| `dev/tests/app_e2e.py` | alterado |
| `dev/tests/extension.py` | alterado |
| `dev/tests/playwright.py` | alterado |
| `dev/tests/ssh_home.py` | alterado |
| `dev/container-install/inside.sh` | alterado |
| `dev/container-platform/inside.sh` | alterado |
| `CLAUDE.md` | alterado |
| `docs/mission/handoffs/phase-55.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não: regra 6 (só
  `make compile` e `english-check`); fmt e clippy do crate `ward` rodados à parte, limpos
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (escritos; rodam no Milestone 6)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — n/a (só o texto do nível; ver Problemas)
- [x] Ficou dentro do escopo da fase
