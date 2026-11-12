# Fase 54e — Tamanho dos textos da troca de senha (UI)

## Implementado
- `app/src/telas/TrocarSenha.svelte`: `.explicacao` e `.aviso` passaram de
  `--tamanho-sm`/`--altura-linha-sm` (13/18) para `--tamanho-base`/`--altura-linha-base`
  (14/20), o estilo `8:12` (Inter 14/20) que o FigCraft usa nos dois textos. Nada mais
  mudou (`.falha` e as legendas continuam como estavam).
- Medida no FigCraft (cópia do `Ward.figcraft` no scratchpad, aberta pelo MCP headless;
  o original não foi aberto nem alterado): frame `20085` › `20411` › `corpo` `20417`,
  explicação `20418` (14/20, 430×40, `#9AA1AE`) e aviso `22963` (14/20, 430×60,
  `#F5B544`, 16 px abaixo da explicação).
- Conferido no Chrome headless (Playwright do Python, 1280×800) pela ponte de teste, em
  Configurações › "Trocar senha mestra": explicação 14px/20px, 430×40,
  `rgb(154,161,174)`; aviso 14px/20px, 430×60, `rgb(245,181,68)`; espaço entre os dois 16;
  x 425 igual ao design; quebras de linha iguais às do frame (2 e 3 linhas). Medidas em
  `docs/mission/evidence/fase-54e/medidas.json`, captura em
  `docs/mission/evidence/fase-54e/trocar-senha-mestra.png`.

## Segurança
- Invariantes tocadas: nenhuma. Mudança só de CSS (dois tokens já existentes). Nenhum
  segredo entra em estado, armazenamento ou URL; nada de cor ou medida solta.
- Testes de ataque: não se aplica (nenhuma SEG tocada). `medidas.json` sem padrão de
  `.dev/variacoes.txt` (`grep -c -F -f` = 0). A URL da ponte (com token) ficou só num
  arquivo 0600 do scratchpad, apagado no fim; o script de medida também ficou no
  scratchpad.
- Dependências novas: nenhuma.

## Ficou de fora
- `make check` e cargo: não rodados, por instrução (a 54c compila no mesmo checkout).
  Para a ponte, não usei `make app-ponte` (ele chama `cargo build`): subi o daemon de
  teste com os binários já compilados em `target/debug/` por
  `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54e WARD_DEV_APP=…/ward-ponte-teste dev/daemon.sh subir`
  e `dev/ponte.sh`; parei com `dev/daemon.sh parar` e apaguei a pasta do socket. Nada de
  Rust foi compilado. Os binários podem já ter mudanças em andamento da 54c; para esta
  tela (só CSS do front) isso não interfere.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make app-front` (build, `app-dist.py`, `npm audit` com 2 avisos low já conhecidos do `deepmerge`) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54e WARD_DEV_APP=$PWD/target/debug/ward-ponte-teste dev/daemon.sh subir` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54e dev/ponte.sh` (URL num arquivo 0600 do scratchpad) | 0 |
| `python3 -I -B <scratchpad>/pw/medir.py docs/mission/evidence/fase-54e < url` (Playwright, Chrome headless) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54e dev/daemon.sh parar` | 0 |
| `grep -c -F -f .dev/variacoes.txt docs/mission/evidence/fase-54e/medidas.json` | 1 (0 achados) |
| `make check` | não rodado (instrução do orchestrator) |

## Problemas descobertos
- O plano diz "`--tamanho-md` com altura de linha 20", mas no `tokens.css`
  `--tamanho-md` é 16px/24px. O design (`8:12`) é 14/20 = `--tamanho-base`/
  `--altura-linha-base`; segui o FigCraft (regra da fase UI). Corrigir o texto do plano.
- Fora do escopo (não mexi): no app o corpo do diálogo começa em y 170 e no design em
  y 173 (3 px; cabeçalho do `Modal`, comum a todos os diálogos), e o design tem o "X" de
  fechar no cabeçalho, que o `Modal.svelte` não mostra. Divergências anteriores a esta
  fase, do componente `Modal`, não da `TrocarSenha`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/src/telas/TrocarSenha.svelte` | alterado |
| `docs/mission/evidence/fase-54e/medidas.json` | criado |
| `docs/mission/evidence/fase-54e/trocar-senha-mestra.png` | criado |
| `docs/mission/handoffs/phase-54e.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução: a 54c compila em paralelo; `make app-front` = 0)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada)
- [x] (UI/DESIGN) Conferido contra o FigCraft (frame `20085`, cópia no scratchpad)
- [x] Ficou dentro do escopo da fase
