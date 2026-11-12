# Fase 50a — Achados da auditoria do M8

## Implementado
- **Médio, deny de Bash sem âncora** (`skill/settings-deny.json`, de 30 para 117
  regras): todas as regras `Bash` começam com `*`. As de `ward run` usam
  `*ward*run*<forma>*`, para pegar também `/usr/local/bin/ward run`, `cd … && ward run`,
  `ward  run` e `"ward" run`. As formas cobertas são `env` (inclui `printenv`,
  `/usr/bin/env`, `environ`, `getenv`, `process.env`), `export`, `declare`, `typeset`,
  `compgen`, `xtrace`, `sh -x`/`-xc`/`-cx`/`-ex`/`-xe`/`-vx`/`-xv`, `set` (com espaço,
  aspa ou `;` antes), `printf`, `echo…$`, `<<<`, `base64`, `base32`, `basenc`, `xxd`,
  `hexdump`, `uuencode`, `openssl…enc`, `od`/`rev`/`tr` (com espaço, `/`, aspa ou `|`
  antes), `cut `, `fold`, `tee`, `head -c`, `tail -c`, ` dd `, `strings`,
  `python…-c`, `node -e/-p/--eval/--print`, `perl -e/-E`, `ruby -e`, `php -r`, `>`,
  `.pgpass`, `.netrc`, `git config` e `cat`/`head`/`tail`/`less`/`more`/`cp`/`mv`/`ln`/`<`
  de `WARD_ARQUIVO_*`.
  Os caminhos do Ward também são negados com curinga e por variável:
  - `*.local/s*`, `*.local/?*`, `*.local/[*`, `*.local/*/w*`, `*.local/*/?*`,
    `*.local/*/[*`, `*.local/*ward*`, `*share/w*` e `*XDG_DATA_HOME*`;
  - `*.config/w*`, `?`, `[`, `*ward*` e `*XDG_CONFIG_HOME*`;
  - `*/run/user*` (qualquer comando com `/run/user`) e `*XDG_RUNTIME_DIR*`;
  - `*ward-backup*` e `*/run/media*`;
  - `*.ssh/id*`;
  - `*environ*`, `*ENVIRON*`, `*/proc/*`, `*/proc` (e com espaço, `;`, `&`, `|` ou aspa
    depois), `*proc/*/e*`, `*proc/*/?*` e `*proc/*/[*` (pega `/proc/N/env*` e
    `cd /proc/N && cat env*`);
  - `*ward*destravar*` e `*ward*--token*`.
- **Autoteste com cada forma** (`dev/testes/skill.py`):
  - `ATAQUES_BASH`: 89 comandos. Entram todas as formas da auditoria e as da seção
    "Nunca faça" da skill, além de curingas, variáveis e `cd` nos caminhos.
  - Um leitor de regras faz o que o Claude Code faz: `*` casa qualquer texto e o resto
    é literal. Cada ataque tem de ser negado olhando o **comando inteiro** e também
    **cada subcomando** (`;`, `&`, `|`, fim de linha fora de aspas), porque o M10
    ainda não conferiu qual dos dois o Claude Code usa.
  - `LEGITIMOS` e todas as linhas dos blocos ```sh do SKILL.md **não** podem ser
    negados.
  - A regra `Bash` não pode ter parêntese dentro, porque o leitor do Claude Code
    pode cortar a regra no `)`.
  - Ataques novos ao próprio teste: regra de env ancorada como antes, regra de env
    tirada, as 5 de `od` tiradas, a de `>` tirada, a de `/run/user` tirada, a de
    `/proc/*` tirada, `Bash(*ward run*)` (que negaria o uso normal), regra com
    parêntese, caminho do storage state escrito num comando da skill, exemplo que
    faz `echo $GITHUB_TOKEN`, skill sem `/run/media/**/ward-backup/**`. Todos são
    recusados.
- **Baixo, storage state do `ward playwright`**: o problema estava na skill, então a
  regra não mudou. O exemplo agora é
  `WARD_ESTADO="$(ward playwright acme-wp-prod)" /usr/bin/python3 tests/test_painel.py`.
  O teste lê `os.environ["WARD_ESTADO"]` / `process.env.WARD_ESTADO`, e a skill diz
  para nunca escrever o caminho num comando. Saiu também a linha
  `# imprime: /run/user/…`.
- **Baixo, `//run/media/**/ward-backup/**`**: entrou `Read`/`Edit` no deny, a regra
  `Bash(*/run/media*)` e o caminho na skill e no README. O teste exige o caminho entre
  crases na skill: antes, o caminho `/media/…` passava por ser um pedaço do
  `/run/media/…`.
- **Baixo, `curl` do `beta-github-api`**: agora usa
  `--variable %GITHUB_TOKEN --expand-header 'Authorization: Bearer {{GITHUB_TOKEN}}'`
  (curl ≥ 8.3), sem shell e sem o token no argv do `curl`. Conferido com o curl 8.5.0
  local contra um servidor em 127.0.0.1 com um valor falso: o cabeçalho chega
  expandido. A skill explica a forma.
- **Baixo, `make teste-skill`**: compila o `ward` com
  `--target-dir target/sem-modo-teste`, o mesmo do `teste-sem-modo-teste`, e não
  mexe mais no `target/debug`. O comentário do Makefile e a linha do `CLAUDE.md`
  foram atualizados.
- **Baixo, "livre" = o agente consegue ler o segredo (RA3)**:
  - `docs/escopo.md` › "Nível de acesso por entrada";
  - `docs/migracao.md` › Passo 7, item 3 (roteiro do M10);
  - `skill/README.md` › "Nível livre". O README ganhou também a lista do que as
    regras negam, os falsos positivos de propósito e, no passo 3 da instalação, uma
    conferência no Claude Code real de que a regra com `*` no início nega. Ela usa um
    nome de entrada que não existe, para nada vazar se a regra falhar.
- **Pedido do QA do M8 (orchestrator)**: a tabela de erros da skill ganhou a linha
  `a extensão do Ward no Chrome não respondeu (o Chrome está aberto?)` (`ward login`,
  código 1). O agente avisa o usuário para abrir o Chrome com a extensão e espera.
  O texto foi conferido em `crates/ward/src/comandos.rs` (`NavegadorFalhou`). O código
  1 veio do pedido do QA e não foi conferido de novo.
- A skill agora proíbe redirecionamento e pós-processamento (`cut`, `tr`, `od`…)
  dentro do `ward run`. Também manda não reescrever um comando negado pela
  permissão: o agente avisa o usuário e espera.

## Segurança
- Invariantes tocadas: V8.2 (SEG), S1/S4 via RA3 (regras `Bash` são cinto de
  segurança, não barreira; a skill e o README dizem isso) e S2 (anexo e pasta de
  execução: `Read`/`Edit` e `Bash` negam `/run/user`).
- Testes de ataque, todos em `dev/testes/skill.py`:
  - `conferir_bash` roda os 89 de `ATAQUES_BASH` nos dois modelos (comando inteiro e
    por subcomando);
  - `autoteste` traz os 11 ataques novos listados acima, mais o de
    `deny sem Read(//run/media/**/ward-backup/**)`, que já vinha do laço de
    `CAMINHOS_DENY`.
- Dependências novas: nenhuma.

## Ficou de fora
- Não dá para conferir aqui se o Claude Code real lê `*` no início da regra, `?`/`[`
  como literal e o `//` absoluto, nem se compara o comando inteiro ou cada
  subcomando: isso exigiria mexer no `~/.claude`, o que é proibido. A conferência
  fica para o M10 (passo 3 do `skill/README.md`).
- Curinga genérico em toda a pasta de config (`cat ~/.config/*/x`) não é negado:
  negar isso bloquearia o uso normal do `~/.config`. O código não usa
  `~/.config/ward` (só V8.2 o cita).
- `make check` completo: o disco tinha 7–12 GB livres e outras fases rodavam em
  paralelo. A fase não muda nenhum código Rust, então rodei só origens, fmt-check,
  audit, deny e gitleaks; clippy e `cargo test` (workspace inteiro num target novo)
  não rodaram.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I -B dev/testes/skill.py <ward sem modo-teste>` (durante o trabalho) | 0 |
| `make teste-skill` | 0 |
| `python3 -I -B dev/testes/migracao.py` (`teste-migracao`) | 0 |
| varredura V1.5 (padrões do `ataques-fundacao.sh` em `Makefile` e `dev/`) | sem achados |
| `make origens fmt-check audit deny gitleaks` | 0 |
| `make check` completo | não rodado (disco; nenhuma mudança em Rust) |

## Problemas descobertos
- As regras negam alguns comandos inocentes de propósito, e o README avisa:
  - `update … set` do `psql` dentro do `ward run`;
  - `2>&1`;
  - pipe para `cut`/`tr` depois do `ward run`, se o Claude Code olhar o comando
    inteiro;
  - `ls ~/.local/share/fonts`;
  - qualquer comando com `/run/user`, `/proc/` ou `environ`.
  Se ficar incômodo no uso real (M10), dá para afrouxar regra por regra, mas
  `make teste-skill` exige cada forma de ataque.
- O PRD D17 ainda diz que o deny cobre só `/media/**/ward-backup/**`. O deny agora
  cobre também `/run/media/**`. Fica para o planner atualizar o PRD; não mexi.
- `.dev/variacoes.txt` e o gitleaks do worktree são cópias do checkout principal
  (`.dev/` é ignorado), feitas para rodar os testes sem refazer o cofre de teste.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `skill/settings-deny.json` | alterado |
| `skill/ward/SKILL.md` | alterado |
| `skill/README.md` | alterado |
| `dev/testes/skill.py` | alterado |
| `Makefile` | alterado (`teste-skill` em `target/sem-modo-teste`) |
| `CLAUDE.md` | alterado (linha do `make teste-skill`) |
| `docs/escopo.md` | alterado (nível livre, RA3) |
| `docs/migracao.md` | alterado (Passo 7, item 3) |
| `docs/mission/handoffs/phase-50a.md` | criado |

## Procedimentos
- [ ] `make check` rodado: só em parte (origens, fmt, audit, deny e gitleaks); clippy e
  test não, por falta de disco e porque a fase não muda Rust
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V8.2)
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
