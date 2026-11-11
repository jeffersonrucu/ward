# Fase 58 — Skill e deny (DT10)

## Implementado
- `skill/ward/SKILL.md` com os nomes da 0.2.0:
  - `ward list [--json]` e `ward lock` (tabela de comandos, fluxo, erros, SSH do `~/.ssh`);
    o destrave do usuário por `/usr/local/bin/ward unlock` (RA8).
  - `ward list --json` com as chaves em inglês (`name`, `type`, `group`, `environment`,
    `project`, `address`) e os tipos gravados (`database`, `server`, `login`, `api`, `aws`,
    `smtp`, `ftp-sftp`, `secure-note`, `generic`) ao lado do rótulo pt-BR da tabela
    ("banco", "servidor", "ftp/sftp", "nota segura", "genérico").
  - Variáveis entregues (DT11): `WARD_FILE_KEY`, `WARD_FILE_<NOME>`, `WARD_PROTOCOL`,
    `WARD_ENTRY`, `WARD_GROUP`, `WARD_ENVIRONMENT`.
  - Exemplo do Playwright: `WARD_ESTADO` → `WARD_STATE` (glossário › "Internas e de teste";
    o nome só existe na skill e no `skill.py`, os dois desta fase).
- `skill/settings-deny.json`:
  - `Bash(*ward*destravar*)` virou quatro regras: `*unlock*` e `*ward*unl*` (0.2.0),
    `*destravar*` e `*ward*destr*` (0.1.x, mantidas). `*unlock*`/`*destravar*` pegam o
    binário com curinga (`/usr/local/bin/w?rd unlock`, `[w]ard`, `wa*`, `/usr/*/bin/w*d`) e
    as formas por `env`, `sh -c`, `script -qc`, `python3 -c`; `*ward*unl*`/`*ward*destr*`
    pegam o subcomando com curinga (`ward unl*`, `unlo?k`, `unl[o]ck`).
  - As nove regras de anexo (`cat`, `head`, `tail`, `less`, `more`, `cp`, `mv`, `ln`, `<`)
    agora também para `WARD_FILE*`; as de `WARD_ARQUIVO*` ficam para a 0.1.x.
- `skill/README.md`: o destrave por `/usr/local/bin/ward unlock`, as regras novas e as da
  0.1.x, os comandos inocentes que passam a ser negados (`grep unlock`, `terraform destroy`
  dentro do `ward run`) e uma seção "Atualizar da 0.1.x para a 0.2.0" (copiar a pasta
  `ward/` de novo e acrescentar as regras que faltam; a antiga pode ficar).
- `dev/tests/skill.py`:
  - `ALLOWED` = list/run/ssh/login/playwright/lock; `--help` básico com `list`/`lock`;
    `UNLOCK` = `/usr/local/bin/ward unlock`; a checagem de RA8 pega `ward unlock` e o nome
    antigo pelo `PATH`; `LEGITIMATE` com `ward list`, `list --json`, `lock`,
    `/usr/local/bin/ward lock`, `help`, `--help` e `WARD_STATE`.
  - `unlock_attacks(word, …)`: 26 formas por nome (inteiro, ao lado de outro subcomando,
    caminho completo, espaço duplo, aspas, `env`, `env -i`, `sh -c`, `bash -c`, `script -qc`,
    `python3 -c` com `subprocess.run` e `os.execv`, curinga no binário e no subcomando), para
    `unlock` e para o nome da 0.1.x, nos dois modelos (inteiro e por subcomando).
  - Ataques de anexo com `{k}` em `ATTACHMENT_VARIABLES` (`WARD_FILE_KEY` e o nome da
    0.1.x), agora também com `tail`, `less`, `more` e `mv`.
  - `FORBIDDEN` na skill: subcomando da 0.1.x mesmo fora de código, variável entregue da
    0.1.x (`WARD_ARQUIVO`, `WARD_ENTRADA`, `WARD_GRUPO`, `WARD_AMBIENTE`, `WARD_PROTOCOLO`,
    `WARD_ESTADO`) e chave antiga do JSON entre crases; `REQUIRED_IN_SKILL` com
    `` `ward list --json` ``, `` `database` ``, `` `WARD_FILE_KEY` ``, `` `WARD_ENTRY` ``.
  - Autoteste: 75 ataques plantados, todos recusados pelo motivo esperado (log).
- `CLAUDE.md`: a linha do `make test-skill` com list/lock e o que a Fase 58 confere.

### Literais alterados (PRD regra 5)
| Arquivo | Antigo → novo | Motivo |
| --- | --- | --- |
| `dev/tests/skill.py` | `ALLOWED`, conjunto do `--help`: `lista`/`travar` → `list`/`lock` | subcomando (DT10) |
| `dev/tests/skill.py` | `UNLOCK` → `/usr/local/bin/ward unlock`; regex de RA8 com `(unlock\|<antigo>)` | subcomando (DT10) |
| `dev/tests/skill.py` | `LEGITIMATE`: `ward lista[ --json]`, `ward travar` → `list`, `lock` (+ `help`, `--help`, caminho completo) | subcomando (DT10) |
| `dev/tests/skill.py` | `$WARD_ARQUIVO_CHAVE` nos ataques → `${k}` (as duas formas) | variável entregue (DT11) |
| `dev/tests/skill.py` | `WARD_ESTADO=` → `WARD_STATE=` (2×) | variável do exemplo (glossário) |
| `dev/tests/skill.py` | ataques do autoteste com `ward unlock`, os nomes antigos e `WARD_ENTRY`/`address` | conferência nova |
| `skill/settings-deny.json` | `Bash(*ward*destravar*)` → as quatro regras acima; nove `Bash(*ward*run*…WARD_FILE*)` | DT10, DT11 |

`allow.txt`/`frozen.txt`: nada novo; nenhuma entrada da Fase 58.

## Segurança
- Invariantes tocadas: m2:V5.3 (SEG; ameaça 1, agente com prompt injection) e as V8.1,
  V8.2, V8.4 da missão 1 (skill sem segredo, deny dos caminhos, SSH do `~/.ssh` direto).
  S1/S3: a senha mestra nunca passa pelo agente (a skill não destrava; o deny nega
  `ward unlock` e o nome antigo). RA8: só o caminho completo do `ward` instalado.
- Testes de ataque escritos e **rodados** (`make test-skill`, saída 0):
  - `dev/tests/skill.py` › `unlock_attacks("unlock", …)` e `unlock_attacks(<nome da 0.1.x>, …)`
    em `verify_bash` (cada forma negada inteira e por subcomando);
  - `dev/tests/skill.py` › `BASH_ATTACKS` de anexo × `ATTACHMENT_VARIABLES`;
  - `dev/tests/skill.py` › `self_test`: deny sem cada uma das quatro regras de unlock, só
    com o nome antigo, só com o novo, regra ancorada no início, sem cada regra de anexo
    (8 verbos × 2 nomes), skill com subcomando antigo (em código e no texto), variável
    antiga, chave antiga do JSON, sem `ward list --json`, destrave pelo `PATH`, por outro
    `ward`, pelo nome antigo, README sem o caminho completo novo.
- Nenhum padrão de `.dev/variations.txt` em `skill/` (conferido pelo `make test-skill`).
- Dependências novas: nenhuma.

## Ficou de fora
- Curinga no binário **e** no subcomando ao mesmo tempo (`w?rd unl*`) e o subcomando partido
  por aspas (`un''lock`) ou montado em string (`'unl' + 'ock'` no Python) não são negados.
  As regras comparam texto (o `skill/README.md` diz isso); negar `* un*` pegaria
  `uname -a` dos exemplos. A barreira é outra: a senha mestra não passa pelo agente e o
  `ward unlock` precisa de terminal.
- A instalação no `~/.claude` é do usuário (M10); nada foi escrito lá.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 00f31eb` (o worktree partia de `b4d5e1b`, mais velho que o master) | 0 |
| `make dev-variations` (log `.dev/logs/fase58-dev-variations.log`) | 0 |
| `make test-skill` (log `.dev/logs/fase58-test-skill.log`; 1ª rodada falhou num ataque meu mal feito, ver Problemas) | 0 |
| `make compile` (log `.dev/logs/fase58-compile.log`) | 0 |
| `make english-check` (repositório inteiro; log `.dev/logs/fase58-english.log`) | 0 |

`make check` não rodou (PRD regra 6). O `make test-skill` não fala com daemon.

## Problemas descobertos
- Na primeira rodada, o ataque "deny sem a regra `<*WARD_FILE`" passou: a regra geral de
  entrada padrão `*ward*run*<*` (Fase 54b) já cobre o `xxd < "$WARD_FILE_KEY"`, então a regra
  de `<` do anexo é redundante (para as duas formas). Tirei o `<` da lista de remoções do
  autoteste e deixei a regra (não custa nada).
- `*unlock*` e `*destravar*` sem o `ward` negam qualquer comando com a palavra (ex.: `grep
  unlock src/`, `cargo test unlock`). Escolhi segurança (pegar o binário com curinga);
  `*ward*destr*` nega também `terraform destroy` dentro do `ward run`. Os dois estão no
  `skill/README.md`. Se o usuário achar demais, a alternativa é só `*ward*unlock*` e
  `*ward*unl*`, sem o curinga no binário (e o contrato m2:V5.3 com "curingas" teria de mudar).
- O glossário (`docs/glossario.md`, "Internas e de teste (DT12; Fase 56)") põe
  `WARD_ESTADO` → `WARD_STATE` na Fase 56. Fiz aqui porque o nome só existe na skill e no
  `skill.py`; se a Fase 56 fizer o mesmo nesses arquivos, o merge pode dar conflito nas
  mesmas linhas (troca igual).
- `CLAUDE.md` (linha do `make test-skill`) ganhou um hunk; a Fase 55 já mexeu em outra linha.
- O deny instalado no `~/.claude` desta máquina já nega comandos Bash com o nome antigo do
  destrave: um `sed`/heredoc com o texto foi recusado durante a fase. Usei o Edit.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `skill/ward/SKILL.md` | alterado |
| `skill/README.md` | alterado |
| `skill/settings-deny.json` | alterado |
| `dev/tests/skill.py` | alterado |
| `CLAUDE.md` | alterado |
| `docs/mission/handoffs/phase-58.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não: regra 6 (só `make compile` e `english-check`); o
  `make test-skill` rodou e passou
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (m2:V5.3; rodado)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — n/a
- [x] Ficou dentro do escopo da fase
