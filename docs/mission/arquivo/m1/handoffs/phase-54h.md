# Fase 54h — `Modal` contra o design (UI)

## Implementado
- `app/src/lib/componentes/Modal.svelte`: o cabeçalho ganhou o `fechar` do componente
  `Modal` (`611`; frames `615` no padrão e `634` no perigo, `20414` na instância
  `20411`). É um `<button type="button" aria-label="Fechar">` de 32×32
  (`--controle-sm`), raio 6 (`--raio-sm`), fundo transparente, com o `Ícone/fechar` de
  20 px no tom padrão (`#9AA1AE`). O título passou a ocupar o resto da linha
  (`flex: 1`; 386 px como no design). Com o cabeçalho de 32 px, o corpo desce os 3 px
  que faltavam (y 173, igual ao frame `20085`).
- O "X" chama `dialogo.close()`. Esse é o caminho do Esc: o evento `close` chama o
  `onfechar` de cada diálogo, e é o mesmo `onfechar` que o Cancelar usa. Os três
  diálogos reais (`TrocarSenha`, `TrocarPin`, `EscolherPendrive`) já apagam os campos e
  limpam os erros nesse caminho.
- Medida no FigCraft, numa cópia do `Ward.figcraft` no scratchpad aberta pelo MCP
  headless. O original não foi aberto. Medi o frame `20085` › `20411` (cabeçalho
  `20412`, título `20413`, fechar `20414`, ícone `20415`, corpo `20417`, rodapé
  `20462`) e o set `Modal` `611` (`612`/`628`).
- Conferido no Chrome headless (Playwright do Python, 1280×800) pela ponte de teste, em
  Configurações:
  - "Trocar senha mestra" bate com o design: diálogo 400,96 480×608, cabeçalho
    425,121 430×32, título y 124 e 386 de largura, X 823,121 32×32 com raio 6px e fundo
    transparente, ícone 829,127 20×20 `rgb(154,161,174)`, corpo y 173, rodapé y 635 h 44.
  - O X, o Esc e o Cancelar fecham os dois diálogos de troca. Ao reabrir, os campos
    estão vazios.
  - O foco inicial continua no primeiro campo (`trocar-senha-atual` e
    `trocar-pin-atual`).
  - O Tab percorre só o diálogo e passa pelo X. Enter no X focado fecha.
  - "Escolher pendrive" (aberto pelo interruptor do backup externo) também fecha pelo
    X, pelo Esc e pelo Cancelar, e o interruptor continua desligado nos três casos.
- Evidência em `docs/mission/evidence/fase-54h/`:
  - `medidas.json`, `foco.json` e `pendrive.json`;
  - capturas `trocar-senha-mestra.png`, `cabecalho-trocar-senha.png`,
    `trocar-pin.png`, `escolher-pendrive.png` e `fechar-foco-teclado.png`;
  - `figcraft-20085.png`, a captura do frame no FigCraft.

## Segurança
- Invariantes tocadas: nenhuma SEG. O X segue o caminho que o Esc já usava (V5.25: o
  foco continua preso no `<dialog>` nativo e o Esc continua fechando). Fechar pelo X
  apaga os campos de senha e de PIN, como fazem o Cancelar e o Esc. Conferi isso pelos
  campos vazios ao reabrir.
- Nenhum segredo vai para estado global, armazenamento ou URL. O CSS só usa tokens.
- Testes de ataque: não se aplica, porque nenhuma SEG foi tocada. Os três JSON da
  evidência não têm nenhum padrão de `.dev/variacoes.txt` (`grep -c -F -f` = 0 em cada
  um).
- A URL da ponte, que tem o token, ficou só num arquivo 0600 do scratchpad e foi
  apagada no fim. Os scripts de medida também ficaram no scratchpad.
- Dependências novas: nenhuma.

## Ficou de fora
- `make check` e cargo: não rodei, por instrução, porque a 54c compila no mesmo
  checkout. Também não usei `make app-ponte`. Subi o daemon de teste com os binários já
  compilados em `target/debug/`, por
  `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54h WARD_DEV_APP=…/ward-ponte-teste dev/daemon.sh subir`
  e `dev/ponte.sh`. Depois parei o daemon com `dev/daemon.sh parar` e apaguei
  `/run/user/1000/ward-54h`.
- A variante perigo só aparece na Vitrine, que só abre na tela de espera do build de
  teste e não estava alcançável com o cofre destravado. Ela usa o mesmo cabeçalho, com
  o ícone de alerta antes do título, como o `628`. Não medi essa variante no Chrome.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make app-front` (build, `app-dist.py`, `npm audit` com os 2 avisos low já conhecidos do `deepmerge`) | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54h WARD_DEV_APP=$PWD/target/debug/ward-ponte-teste dev/daemon.sh subir` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54h dev/ponte.sh` (URL num arquivo 0600 do scratchpad) | 0 |
| `python3 -I -B <scratchpad>/pw54h/medir.py docs/mission/evidence/fase-54h < url` | 0 |
| `python3 -I -B <scratchpad>/pw54h/foco.py …` | 0 |
| `python3 -I -B <scratchpad>/pw54h/pendrive.py …` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54h dev/daemon.sh parar` | 0 |
| `grep -c -F -f .dev/variacoes.txt docs/mission/evidence/fase-54h/*.json` | 0 achados |
| `make check` | não rodado (instrução do orchestrator) |

## Problemas descobertos
- No "Escolher pendrive", o foco inicial agora vai para o X. Antes ia para o primeiro
  botão habilitado do rodapé, porque o diálogo não foca nenhum campo ao abrir. Esse
  comportamento é coerente e seguro, porque não deixa "Ligar" focado por engano. Se o
  design quiser o foco na lista, isso é uma fase própria.
- No design, os botões do rodapé do `Modal` (`612`/`628`, `620`/`624`) e do `Diálogo de
  troca` (`20463`/`20467`) estão com 40 px (`6:141`, `controle/md`) e 4 px de padding no
  topo do rodapé (`6:129`). O componente `Botão` `263` é 44 (`controle/padrao`, desde a
  4d), e o app usa 44 sem esse padding. O total bate (rodapé 44, diálogo 608), mas o
  botão do app fica em y 635 e o do design em y 639. A instância parece ter guardado o
  tamanho antigo (override "size"). Isso é do design, e não mexi.
- Em "Trocar PIN" (frame `20193` › `20471`), o design tem 558 de altura e o app tem
  554. A diferença está no corpo do `TrocarPin` (o cabeçalho e o início do corpo batem
  em relação ao diálogo, +25 e +77). É anterior a esta fase e fora do `Modal`; não
  investiguei.
- O plano usa o nome `TrocarSenha`, mas o "X" vale para todos os diálogos do `Modal`.
  Conferi os três que existem.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/src/lib/componentes/Modal.svelte` | alterado |
| `docs/mission/evidence/fase-54h/medidas.json` | criado |
| `docs/mission/evidence/fase-54h/foco.json` | criado |
| `docs/mission/evidence/fase-54h/pendrive.json` | criado |
| `docs/mission/evidence/fase-54h/trocar-senha-mestra.png` | criado |
| `docs/mission/evidence/fase-54h/cabecalho-trocar-senha.png` | criado |
| `docs/mission/evidence/fase-54h/trocar-pin.png` | criado |
| `docs/mission/evidence/fase-54h/escolher-pendrive.png` | criado |
| `docs/mission/evidence/fase-54h/fechar-foco-teclado.png` | criado |
| `docs/mission/evidence/fase-54h/figcraft-20085.png` | criado |
| `docs/mission/handoffs/phase-54h.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução: a 54c compila em paralelo; `make app-front` = 0)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada)
- [x] (UI/DESIGN) Conferido contra o FigCraft (frame `20085` e set `611`, cópia no scratchpad)
- [x] Ficou dentro do escopo da fase
