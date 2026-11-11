# Fase 47a — V1.5 pula só a linha de base de `dev/fixtures/legacy/` (Missão 2, trilha S)

## Implementado

Parte de `master` f4c73db (o worktree estava em b4d5e1b; `git reset --hard f4c73db` com a
árvore limpa). Só `dev/tests/attacks-foundation.sh` mudou.

- A varredura de pasta proibida do `v1_5` saiu para `forbidden_dirs_scan <raiz>`
  (padrões em `FORBIDDEN_DIR_PATTERNS`, os mesmos seis, montados em pedaços). Ela continua
  varrendo o `Makefile` e todo arquivo de `dev/` (`find -type f`), com a mesma regex
  `grep -nEi`, e pula **só** um arquivo que esteja listado diretamente no
  `dev/fixtures/legacy/SHA256SUMS` (caminho exato `<raiz>/dev/fixtures/legacy/<nome>`) e
  cujo sha256 ainda bata com a linha. Nome do `SHA256SUMS` só vale se for relativo, sem
  `..`/`.`/vazio e com caracteres `[A-Za-z0-9._-/]`; `SHA256SUMS` ou a pasta como link
  desligam a exclusão. Escolhi isso em vez de podar a pasta inteira (`-prune`): uma poda
  deixaria `dev/fixtures/legacy/` como esconderijo para qualquer arquivo novo ou editado.
- `v1_5_baseline_controls` (chamado no fim do `v1_5`), numa cópia dos arquivos rastreados
  (`copy_workspace`, sem build) em `$TMP`:
  - controle: `strings-0.1.4.txt` da cópia tem os padrões (a exclusão tem efeito);
  - controle: a cópia sem mudança fica limpa;
  - o mesmo padrão (caminho do cofre real) plantado em cada lugar abaixo, numa cópia nova,
    tem de ser acusado: `dev/tests/`, `dev/fixtures/` (vizinho), `dev/fixtures/legacy-x/`
    (prefixo), `dev/tests/fixtures/legacy/` (pasta aninhada com o mesmo nome), arquivo
    novo em `legacy/`, arquivo em subpasta de `legacy/` fora da lista, arquivo da linha
    de base com uma linha a mais (sha256 não bate), `Makefile`;
  - linha `../../tests/<plantado>` acrescentada ao `SHA256SUMS` não tira o arquivo
    plantado da varredura.

## Segurança

- **Invariantes**: S15 (V1.5: `Makefile` e `dev/` não citam pasta do usuário). A exclusão
  é mais estreita do que "a pasta": só os 8 arquivos da Fase 3, intactos. A integridade
  do `SHA256SUMS` em si (arquivo da linha de base editado junto com a linha do hash) é do
  V1.6, não desta varredura; está escrito no comentário do teste.
- **Testes de ataque escritos**: `dev/tests/attacks-foundation.sh` › `v1_5_baseline_controls`
  (9 plantios acusados + 2 controles).
- **Dependências novas**: nenhuma (bash, `find`, `sha256sum`, `grep`).

## Ficou de fora

- `make test-attacks-foundation` inteiro: não rodei (roda `make check` em cópias, PRD
  regra 6; fica para o M6). Rodei só o `v1_5` e os controles novos por um harness
  (`<scratchpad>/fase47a/harness-fase47a.sh`) que extrai `ok`/`failed`/`copy_workspace`/
  `v1_5`/`forbidden_dirs_scan`/`v1_5_baseline_controls` do script e roda com `ROOT` = o
  worktree e cópias em `.dev/fase47a-tmp` (apagada no fim). Sem cofre, sem serviços,
  sem home real.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard f4c73db` | 0 |
| harness do `v1_5` (log `<scratchpad>/fase47a/fase47a-v1_5.log`): 15 `ok`, 0 falha; antes desta fase o mesmo `v1_5` acusava 3 linhas em `dev/fixtures/legacy/` (handoff 47) | 0 |
| `make english-check PATHS="dev/tests/attacks-foundation.sh"` (log `fase47a-english-check.log`) | 0 |
| `make compile` (log `<scratchpad>/fase47a/fase47a-make-compile.log`; 48 shell, 35 Python, 7 JS, 0 erro) | 0 |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- Nenhum novo. Nota: o `CLAUDE.md` ainda cita os nomes antigos (`teste-ataques-fundacao`
  etc.); é da fase de documentação.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/tests/attacks-foundation.sh` | alterado |
| `docs/mission/handoffs/phase-47a.md` | criado |

## Procedimentos
- [ ] `make check` rodado — não: PRD regra 6 (`make compile` e `english-check`, ambos 0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum usado)
- [x] Teste de ataque para cada (SEG) tocado (V1.5/S15: plantios fora da exclusão)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
