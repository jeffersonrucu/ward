# Fase 28d — Cópia do workspace no `teste-ataques-fundacao`

## Implementado
Só `dev/testes/ataques-fundacao.sh` mudou.

- **`copiar_workspace`**: copia os arquivos que o git rastreia (`git ls-files -z` mais `tar --null --no-recursion`, com o conteúdo da árvore de trabalho). Antes copiava só `crates/` e alguns arquivos soltos.
  - Assim vão juntos `app/` (membro `app/nucleo`) e `dev/fixtures/entradas.json`, que o `app/nucleo` e a `ward-ponte-teste` leem por `include_str!`. Só copiar `app/` não bastava: o clippy da cópia ainda quebrava nesse arquivo.
  - `.dev/`, `target/` e `node_modules/` ficam de fora, porque o git os ignora.
  - Confere se Cargo.toml, Cargo.lock, Makefile, `dev/origens.py`, `app/nucleo/Cargo.toml` e `dev/fixtures/entradas.json` chegaram, e sai se faltar algum.
  - Roda `git init -q` vazio em cada cópia (ver o item seguinte).
- **`$TMP` em `.dev/`** (`mktemp -d "$RAIZ/.dev/ataques-fundacao.XXXXXX"`, `.dev` 0700), não mais em `/tmp`.
  - Motivo: o `cargo test` da cópia roda `crates/wardd/tests/app_teste.rs` (V3.37). Ele sobe o daemon de teste com `WARD_DEV_APP`, e o `BinarioDeTeste::conferir` (`crates/ward-plataforma/src/lancador.rs`) recusa de propósito um app abaixo de pasta gravável por outros. O `/tmp` (1777) é uma delas, então numa cópia em `/tmp` esse teste falha sempre ("esperando o socket do daemon") e o V1.3 dava falha falsa.
  - Os dois gitleaks da árvore pulam `^\.dev/`. O `git init` vazio impede que o `git`/`gitleaks git` de uma cópia sem repositório próprio ache o repositório de cima. O Makefile tira `GIT_CEILING_DIRECTORIES` do ambiente, por isso não dá para usar a variável.
- **bwrap do `config_do_cargo_home`**: ganhou `--bind "$TMP" "$TMP"`. O home é coberto por tmpfs e o `$TMP` agora está no home; sem o bind, os scripts somem e o resultado é exit 127. O bind expõe só a pasta temporária do teste.
- **`rodar <função>`**: depois de cada ataque, apaga os `target/` das cópias (`rm -rf "$TMP"/*/target`). Antes, os builds de várias cópias se somavam no disco até o fim.

## Segurança
- Invariantes tocadas: S14 e S15 (V1.2, V1.3 e V1.5), só no harness de teste. Nenhum código de produção mudou e nenhuma regra ficou mais frouxa.
  - O V1.5 continua varrendo `Makefile` e `dev/` com os mesmos padrões.
  - A recusa de pasta gravável do `BinarioDeTeste` ficou como está: a cópia é que saiu do `/tmp`.
- O material gerado (chave ed25519, segredo aleatório) fica agora em `.dev/ataques-fundacao.*` (0700, ignorado pelo git e pelos gitleaks da árvore) e é apagado pelo `trap` ao sair. Conferi depois de cada interrupção: nada sobrou.
- Testes de ataque: nenhum novo. Os existentes passaram a rodar de fato nas cópias.
- Dependências novas: nenhuma.

## Ficou de fora
- **`make teste-ataques-fundacao` inteiro até "Todos os ataques foram barrados"**: não terminou, por falta de disco. Parei, como pedido.
  - Na última rodada passaram **160 `ok`, sem nenhuma `FALHA`**, de `v1_2` até o fim de `config_do_cargo_home`: V1.2 (os três), allowlist, índice, V1.3 (advisory no passo 4/6 do `make check` e no deny, git, lockfile), V1.5 (os quatro), varredura extra, origens, ferramentas fixas, lista de permitidas com o controle do rustc-wrapper, CARGO_HOME e rustup override.
  - Faltaram `ambiente_do_make` (um `make check` inteiro numa cópia) e `git_sem_config_do_usuario`, que é barato.
  - Durante o `ambiente_do_make`, o disco chegou a 600 MB livres. A cópia tinha 6,2 GB, ainda compilando, e o `target/` do worktree (do V1.2) tinha 3,1 GB. Parei a rodada; o `trap` limpou a cópia. Também apaguei o `target/` deste worktree, que era só meu. Ficaram 9,9 GB livres.
  - O pico da rodada inteira fica perto de 10 GB: `target/` da raiz mais uma cópia compilando. Precisa de uns 12 GB livres e de nenhum outro worktree compilando ao mesmo tempo. O `target/` do checkout principal tem 15 GB, e o worktree `agent-a5cd8a2a19328a141` tem 3,7 GB. Não toquei em nenhum dos dois.
- **`make check`**: não rodado, pelo mesmo motivo (o V1.2 já roda o `make check` da raiz duas vezes, e as duas deram certo na rodada acima: sai 2 com o vazamento e volta a 0 sem ele).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `df -h /` (antes) | 0 (14 GB livres) |
| `make teste-ataques-fundacao` (1ª, só `app/` copiado) | interrompido; V1.3 com falha falsa (`include_str!` de `dev/fixtures/`) |
| `make teste-ataques-fundacao` (2ª, `git ls-files`, `$TMP` em `/tmp`) | interrompido; V1.3 com falha falsa (`app_teste` recusado sob `/tmp`) |
| `make teste-ataques-fundacao` (3ª, `$TMP` em `.dev/`) | interrompido; CARGO_HOME com 127 (bwrap sem o `$TMP`) |
| `make teste-ataques-fundacao` (4ª, final) | interrompido no `ambiente_do_make` por disco (600 MB livres); 160 ok, 0 FALHA até ali |
| `bash -n dev/testes/ataques-fundacao.sh` | 0 |
| `make check` | não rodado (disco) |

## Problemas descobertos
- **Disco**: o `teste-ataques-fundacao` precisa de dois builds completos do workspace ao mesmo tempo (raiz e cópia). Com worktrees paralelos, não cabe na partição de 104 GB. Sugestão para o orchestrator: rodar a validação final sozinho, depois de apagar os `target/` dos worktrees que já terminaram.
- A 1ª e a 2ª rodadas foram editadas sob o bash em execução; parei as duas e recomecei limpo. Nenhum resultado delas foi usado.
- Uma cópia em `/tmp` sempre vai falhar no `app_teste` (V3.37), e isso é correto. Qualquer harness futuro que copie o workspace para `/tmp` e rode `cargo test` cai no mesmo problema.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/testes/ataques-fundacao.sh` | alterado |
| `docs/mission/handoffs/phase-28d.md` | criado |
| `.dev/ferramentas/bin/gitleaks` (fora do git; cópia do checkout principal, sha256 conferido pelo `make gitleaks-binario`) | criado |

## Procedimentos
- [ ] `make check` rodado (não: disco)
- [x] Só cofre de teste e credenciais falsas usados (só material gerado na hora)
- [x] Teste de ataque para cada (SEG) tocado (os existentes; 160 ok até o `ambiente_do_make`)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
