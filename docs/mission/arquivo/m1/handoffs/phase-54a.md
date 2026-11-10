# Fase 54a — RA8 aceito, docs endurecidos

## Implementado
- `docs/escopo.md`: RA8 entra em "Fora do modelo de ameaça" como "Sessão gráfica ou shell do
  usuário adulterados por um processo do mesmo uid". O texto cobre o proxy no `wayland-<n>`/`bus`,
  o `ward` falso no `PATH` (o caso da auditoria final), alias e função no rc do shell, e tema
  GTK/dconf/fontes/perfil NVIDIA. Lista as reduções: D57, S5 (barra só o servidor falso), a skill
  e o deny que não destravam, e os guias pelo caminho completo. Diz com clareza o que o caminho
  completo não fecha. A pendência "a decidir antes do M4" virou "resolvida".
- `docs/mission/prd.md`: RA8 "fechado em 2026-11-09: risco aceito pelo usuário", com o parágrafo
  da decisão e da consequência. O resumo "Fora do modelo" (seção de ameaças) e o fluxo 2
  ("Usuário no terminal") citam RA8 e `/usr/local/bin/ward destravar`.
- `docs/instalacao.md`: o passo 3 usa `/usr/local/bin/ward destravar`, e uma nota "Senha mestra
  só pelo caminho completo" explica o porquê e o limite.
- `docs/migracao.md`: um item novo em "Antes de começar" manda destravar pelo app ou por
  `/usr/local/bin/ward destravar`, explica o porquê e o limite, e diz que os outros comandos `ward`
  do guia não pedem a senha mestra. O guia não tinha `ward destravar` antes.
- `skill/ward/SKILL.md`: a mensagem de "travado" para o usuário fala em destravar pelo app ou com
  /usr/local/bin/ward destravar (caminho completo), em prosa e sem crases, porque `destravar`
  fica fora de V8.1. Ela também diz para nunca sugerir que o usuário digite a senha mestra num
  comando rodado pelo Claude ou com `!`.
- `skill/README.md`: seção nova "Destravar: só você, pelo caminho completo".
- `docs/mission/roteiro-usuario.md`: nota de que a senha mestra só vai na janela do app, de que a
  CLI de teste vai sempre por `target/debug/ward` e de que, instalado, o destrave é por
  `/usr/local/bin/ward destravar`. O roteiro não tinha comando que digitasse a senha mestra.
- `dev/testes/migracao.py` (item 7, Fase 54a): o guia tem de citar `/usr/local/bin/ward destravar`
  e não pode ter `ward destravar` sem esse prefixo. Três ataques novos.
- `dev/testes/skill.py` (item 8, Fase 54a): o SKILL.md tem de citar `/usr/local/bin/ward destravar`
  e não pode ter `ward destravar` sem o prefixo; o `skill/README.md` tem de citar
  `` `/usr/local/bin/ward destravar` ``. Quatro ataques novos.

## Segurança
- Invariantes tocadas: S3/S5 (a senha mestra só no TTY e para o `wardd` instalado) e o resíduo
  RA8. É só documentação: nenhum código do Ward muda. O caminho completo fecha o ataque da
  auditoria (`ward` falso antes no `PATH`). Testei no bash e no zsh, por script na scratchpad:
  ele não fecha uma função com o nome `/usr/local/bin/ward` (vale nos dois shells) nem um alias
  do zsh com esse nome. O bash recusa esse alias. O próprio `command` pode ser trocado por uma
  função. Os docs dizem isso e o escopo deixa o shell adulterado como risco aceito.
- Testes de ataque escritos:
  - `dev/testes/migracao.py` › ataques `54a: destravar pelo PATH`, `54a: destravar por outro ward`,
    `54a: destravar pelo PATH em outro passo`
  - `dev/testes/skill.py` › ataques `skill manda destravar pelo PATH`, `skill manda destravar por
    outro ward`, `skill cita destravar pelo PATH além do caminho completo`, `README sem o caminho
    completo`
- Dependências novas: nenhuma.

## Ficou de fora
- A mensagem da própria CLI (veja Problemas): é código do `ward`, fora desta fase de docs.
- Um `make teste-skill` completo, porque compila o `ward` em `target/sem-modo-teste` e o disco
  tem ~2,9 GB livres, com outro worker compilando. Rodei o `dev/testes/skill.py` direto contra o
  `target/debug/ward` do checkout principal. O script só usa o `ward --help` desse binário.
- `make check`: não rodou, pelo disco e porque nenhum arquivo Rust mudou. Rodei à parte o
  gitleaks das duas varreduras de árvore (o binário do checkout principal, com o sha256
  conferido contra o `GITLEAKS_BIN_SHA256` do Makefile), sem achados. Não rodei as varreduras de
  git/índice/histórico porque nada foi commitado.
- O comentário do Makefile e a linha do CLAUDE.md de `teste-migracao`/`teste-skill` não citam a
  Fase 54a. Deixei assim para não conflitar com a Fase 54 no checkout principal.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I -B dev/testes/migracao.py <cópia 0600 de .dev/variacoes.txt>` (= `make teste-migracao`) | 0 |
| `python3 -I -B dev/testes/skill.py /home/…/cofre/target/debug/ward` (com `.dev/variacoes.txt` copiado 0600 no worktree, apagado no fim) | 0 |
| `gitleaks dir --config dev/gitleaks-arvore.toml …` | 0 |
| `gitleaks dir --config dev/gitleaks-extra-arvore.toml … --max-archive-depth 3` | 0 |
| `grep -c -F -f .dev/variacoes.txt` nas linhas acrescentadas | 0 ocorrências |
| `make check` | não rodou (disco; só docs e Python de teste mudaram) |

## Problemas descobertos
- **A CLI manda usar o `ward` do `PATH`.** `crates/ward/src/comandos.rs:143` responde
  `travado: destrave com ` + "`ward destravar`" + ` ou pelo app`. A mensagem aparece no terminal
  do usuário e também no do Claude. Deveria dizer `/usr/local/bin/ward destravar` (ou "pelo
  app"). O `teste-cli` (V4.22) e as evidências do M3/M4 conferem o texto exato, então mudar
  isso afeta os dois. Fica para o orchestrator decidir (é código do `ward`).
- O caminho completo não fecha uma função do bash ou do zsh, nem um alias do zsh, com o nome
  `/usr/local/bin/ward`. Testei e registrei no escopo e no PRD como parte do resíduo aceito. Se
  o usuário quiser fechar mais, a opção é destravar só pelo app, mas o proxy Wayland também é
  RA8. Volta ao usuário.
- O `teste-skill` real (com o `ward` sem `modo-teste`) precisa rodar quando houver disco.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/escopo.md` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/instalacao.md` | alterado |
| `docs/migracao.md` | alterado |
| `docs/mission/roteiro-usuario.md` | alterado |
| `skill/ward/SKILL.md` | alterado |
| `skill/README.md` | alterado |
| `dev/testes/migracao.py` | alterado |
| `dev/testes/skill.py` | alterado |
| `docs/mission/handoffs/phase-54a.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não rodou (disco); o
  gitleaks das árvores rodou à parte, limpo; nenhum arquivo Rust mudou
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
