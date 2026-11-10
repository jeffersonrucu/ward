# Fase 39e — Consertar o recorte do `dev/testes/webkit-real.sh`

## Implementado
- `dev/testes/webkit-real.sh`: o recorte do `main.rs` do harness agora tira só o
  bloco da `partida`. Antes ele apagava tudo entre `// Antes do GTK` e
  `let app = com_comandos(`, e com isso sumiam `fila`, `na_aprovacao`, `estado` e
  `canais` (E0425). Agora são três trocas, cada uma com `assert` de ocorrência única:
  1. o `let mut partida = partida::iniciar()…});` sai (fim achado pelo primeiro
     `\n    });\n` depois do início);
  2. `partida.tomar_daemon().ok_or(ErroApp::ConexaoPerdida)` vira
     `Err::<DaemonSocket, ErroApp>(ErroApp::ConexaoPerdida)`;
  3. o `partida.vigiar(move || {…});` vira `drop(na_vigia);` (o `assert` exige que o
     bloco cortado cite `na_vigia`).
  Depois sai o `use ward_app_nucleo::partida;`. Por fim, um `assert` recusa qualquer
  `partida.`/`partida::`/`partida(` que tenha sobrado fora de comentário. Variável nova no
  `main`, antes ou depois da partida, passa sem mexer no harness. Se a partida mudar de
  forma, o script para com "main.rs do app mudou: …" em vez de compilar outra coisa.
- Antes de rodar, conferi a troca numa cópia do `main.rs` no scratchpad. O diff contra o
  original mostra só as 3 trocas e o `use`.
- Ao contrário da cópia do scrutiny (`wk.sh`), o `RAIZ` continua saindo do caminho do
  script e não fica fixo no checkout.

## Segurança
- V5.32 (D48, D57, D64), sem mudar o que é medido: o harness continua sem a partida e
  só em `.dev/webkit-real/`. Nada que pule a partida entra no repositório, só o script
  que monta a cópia.
- Teste do ataque: `make teste-webkit-real SEM_39C=1` sai 2 com "FALHOU" nas iscas
  frei0r, ladspa e libvisual. O harness consertado ainda enxerga o ataque que a 39c
  fecha, e o `make teste-webkit-real` normal dá "V5.32 ok".
- Dependências novas: nenhuma.

## Ficou de fora
- **Linhas do `CLAUDE.md`** (`teste-logs-app`, `teste-canais-gst`, `teste-app-e2e`):
  não editei. O pedido veio por agente e não pelo usuário, e mudar o `CLAUDE.md` exige
  autorização do usuário (mesma decisão das Fases 39 e 39c). O texto proposto está no
  handoff da 39c, linhas 131–141. Basta o usuário ou o orchestrator, com o ok do
  usuário, colar na tabela de comandos.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make teste-webkit-real` | 0 ("webkit-real: V5.32 ok", 1 min 45 s) |
| `make teste-webkit-real SEM_MPS=1` | 0 (ver Problemas) |
| `make teste-webkit-real SEM_39C=1` | 2 (esperado: iscas da 29b carregam sem as variáveis da 39c) |
| `make check` | 0 ("tudo limpo") |

## Problemas descobertos
- `SEM_MPS=1` não mostra o ataque nesta máquina: deu "V5.32 ok", e o listener em
  `/tmp/nvidia-mps/control` não recebeu conexão nem a isca `cuda` carregou. Parece que
  o caminho do CUDA não é exercitado neste host/GPU, então o modo "mostrar o ataque de
  D64" não prova nada aqui. Isso vem de antes da 39e; não corrigi.
- O comentário do Makefile diz "Sai 1 se V5.32 falhar (inclusive os canais da 29b…)",
  mas com `SEM_39C=1` o script sai 2. É só documentação; não mexi.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/testes/webkit-real.sh` | alterado |
| `docs/mission/handoffs/phase-39e.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V5.32: `SEM_39C=1` falha como deve)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase

Limpeza: apaguei `.dev/webkit-real/src-tauri/target` (1,2 GB, build do harness). O
restante de `.dev/webkit-real/` (fontes copiadas e `saida/strace.log` da última rodada,
`SEM_39C=1`) ficou.
