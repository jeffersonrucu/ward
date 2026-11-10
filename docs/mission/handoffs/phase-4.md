# Fase 4 — Crates e feature (DT13) (Missão 2, Milestone 1)

## Implementado
- **Pastas, por `git mv`** (o histórico segue o arquivo):
  - `crates/ward-plataforma` → `crates/ward-platform`;
  - `crates/ward-ponte-teste` → `crates/ward-test-bridge`;
  - `app/nucleo` → `app/core`. A pasta `target/` ignorada que estava dentro foi junto; tem 4 KiB.
  - `crates/ward-platform/examples/conteiner.rs` → `examples/container.rs`. É o exemplo que leva o nome do crate (glossário, seção 3).
- **Renomes**, pelo glossário, seção 3:

  | Antigo | Novo |
  |---|---|
  | pacote `ward-plataforma`, lib `ward_plataforma` | `ward-platform`, `ward_platform` |
  | pacote `ward-ponte-teste`, lib `ward_ponte_teste`, binário `ward-ponte-teste` | `ward-test-bridge`, `ward_test_bridge`, `ward-test-bridge` |
  | pacote `ward-app-nucleo`, lib `ward_app_nucleo` | `ward-app-core`, `ward_app_core` |
  | exemplo `ward-plataforma-conteiner` | `ward-platform-container` |
  | feature `modo-teste` (Cargo, `cfg`, `cfg!`, `--features`) | `test-mode` |
  | feature `ganchos-teste` do `wardd` | `test-hooks` (termo novo; entrou no glossário, seção 3) |

- **Usos ajustados no repositório inteiro:**
  - `[workspace]`: os membros e as `workspace.dependencies`.
  - Os 8 `Cargo.toml`: dependências, `dep:`, `<crate>/test-mode`, `required-features`, `[[bin]]`/`[[example]]`, e `path = "../core"` no `app/src-tauri`.
  - Os dois `Cargo.lock`, só nos nomes. Nenhuma versão mudou, e a ordem alfabética ficou a mesma. O `make compile` com `--locked` passou.
  - Todos os `use`/caminhos `ward_platform::`, `ward_app_core::` e `ward_test_bridge::`.
  - Todos os `#[cfg(feature = "test-mode")]`, `cfg!(feature = "test-mode")` e `#[cfg(feature = "test-hooks")]`.
  - O Makefile: `-p`, `--features`, `--bin` e `--example`, `PONTE_DE_TESTE := …/ward-test-bridge` e os comentários.
  - Os scripts de `dev/`: `--features`, `-p`, os caminhos de `target/debug/ward-test-bridge` e de `examples/ward-platform-container`, o `Cargo.toml` copiado pelo `webkit-real.sh` (`../core`) e o `sed` do `ataques-fundacao.sh` sobre o membro `crates/ward-test-bridge`.
  - Os comentários que citam esses nomes (Rust, JS, Svelte, shell, Python, C, `instalacao/`).
  - `CLAUDE.md` e `docs/instalacao.md`, no ponto em que citam a feature.
- **Log da ponte:** o lançador grava a saída em `.dev/logs/<nome do binário>.log`. Com o binário novo, o log passa a ser `.dev/logs/ward-test-bridge.log`, que é o nome do glossário. Por isso `dev/ponte.sh` (`LOG=`), o comentário do Makefile, o `CLAUDE.md` e o doc de `crates/ward-test-bridge/src/main.rs` mudaram junto.
- **`cargo fmt`:** os nomes mais curtos mudaram a quebra de linha em `crates/ward/src/cliente.rs`, `crates/ward-platform/tests/identificacao.rs`, `app/core/tests/configuracao_tauri.rs` e outros. Rodei `cargo fmt` (workspace e `app/src-tauri`), e o `fmt --check` dá 0. Ele só mexeu em arquivos que já tinham rename. O `wire_v1.rs` e o `lib.rs` da Fase 3 ficaram intactos.
- **Literais alterados** (PRD regra 5). Todos são nome de crate, binário, feature ou caminho:
  - `cfg(feature = "test-mode")` e `"test-hooks"` (os atributos e as strings dos testes V3.15 em `crates/wardd/tests/aprovacao.rs`).
  - `app/core/tests/configuracao_tauri.rs`: `"test-mode = "` e `.devtools(cfg!(feature = "test-mode"))`, que são lidos do `Cargo.toml` e do `main.rs` do Tauri.
  - `crates/ward-core/src/protocolo/testes.rs`: a lista de proibidos do V3.15 **ganhou** `"test-mode"` e `"test_mode"` e manteve `"modo-teste"` e `"modo_teste"`.
  - `crates/ward-platform/src/lancador.rs` (testes): o nome de script `"ward-test-bridge"`, os caminhos `bin/../bin/ward-test-bridge` e `pasta-link/ward-test-bridge` e `.dev/logs/ward-test-bridge.log`. Os três primeiros estão em `strings-0.1.4.txt`, por virem de um arquivo de produto, e entram na diferença do V6.3.
  - `crates/wardd/src/main.rs`: o log do daemon de teste agora diz `"daemon de TESTE (test-mode): …"`. Só existe com a feature, e nenhum script procura esse texto.
  - Pastas temporárias de teste: `ward-app-core-…`, `ward-platform-…`, `/tmp/ward-platform-falso-nao-criar`.
  - Os `description` dos `Cargo.toml` que citam a feature.
  - Scripts:
    - `dev/ponte.sh` `LOG`;
    - os caminhos de `target/debug/ward-test-bridge` (em `app-e2e.sh` e `selecao-revelado.sh`) e de `examples/ward-platform-container` (em `install.sh`);
    - o `USO` e a mensagem `"… foi compilado com test-mode"` do `docker-plataforma.sh` (DT20);
    - os rótulos `"V9.2: wardd com test-mode"`;
    - o arquivo `bin/wardd-test-mode` do contêiner de install (`dev/testes/install.sh` e `dev/conteiner-install/dentro.sh`, os dois juntos).
- **Exceções:** nenhuma entrada nova em `allow.txt` nem em `frozen.txt`.

## Segurança
- **Invariantes tocadas:**
  - **S8:** feature `test-mode` e V3.15. O rename é só de nome. Não mudou nenhum `cfg`, nenhum braço nem nenhuma conferência: cada `modo-teste` virou `test-mode` no mesmo lugar.
  - **V3.37 e V9.2:** a marca de `modo-teste` nos binários.
  - **S14:** nenhuma dependência nova e nenhuma versão mudada nos lockfiles.
- **Pontos em que o nome ficou de propósito** (marca e texto exibido; Fase 56):
  - `instalacao/instalar.sh`: a lista de marcas (`ward-ponte-teste wardd-app-teste ward-teste`) e a mensagem exibida `"$nome foi compilado com modo-teste (contém …)"`. É texto exibido, congelado pela regra 4. Os testes que conferem essa mensagem continuam esperando `"modo-teste"`: `dev/testes/install.sh` 278–281, `dev/testes/deb.sh` 147 e `dev/conteiner-install/dentro.sh` 145.
  - `dev/testes/app-release.sh` (`ward-ponte-teste ponte-teste …`), `dev/testes/sem-modo-teste.sh` (`TEXTOS=…`) e `dev/testes/install.sh:277` (a marca plantada).
  - Os literais da ponte: `USO` e os `eprintln!("ward-ponte-teste: …")` em `main.rs` e `simulados.rs`. São eles que mantêm a marca `ward-ponte-teste` dentro do binário da ponte, e o `dev/ponte.sh` procura o prefixo `^ward-ponte-teste:` no log.
  - Com isso, o binário renomeado continua sendo recusado pelo `instalar.sh` (`WARD_DEV_*` e a marca), e nenhuma conferência ficou mais fraca.
- **Testes de ataque escritos:**
  - `crates/wardd/tests/aprovacao.rs` › `ataque_v3_15_gancho_sem_feature_ou_fora_do_lugar`: dois casos novos. `#[cfg(feature = "modo-teste")]` em `config.rs` e `#[cfg(feature = "ganchos-teste")]` antes de um gancho em `nucleo.rs` têm de ser recusados como feature desconhecida. Um rename desfeito, ou um nome velho que volte, não passa calado pelo V3.15.
  - `crates/ward-core/src/protocolo/testes.rs` › `v3_15_sem_porta_dos_fundos_no_codigo`: proíbe no protocolo a forma nova (`test-mode`/`test_mode`) e a antiga.
  - `dev/testes/sem-modo-teste.sh`: a conferência do `cargo tree` acusa `test-mode` **ou** `modo-teste` nos pacotes `wardd` e `ward-platform`. O autoteste com `--features test-mode` foi mantido. A lista de pacotes passou a ser `ward-app-core` e `ward-test-bridge`, porque com os nomes velhos o `cargo tree -p` falharia.
  - Escritos e não rodados (regra 6).
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Outras linhas da seção 3 do glossário,** fora do texto da Fase 4 no plano:
  - os binários `wardd-app-teste` e `wardd-aprovador-teste` e as pastas `app-teste/` e `aprovador-teste/`;
  - os exemplos `wardd-cliente-teste`, `pai-chrome`, `chrome-falso`, `dev-cofre` e `partida-conteiner`;
  - os `[[test]]` `trava`, `app_teste` e `app_valor`.
  - Motivos:
    - `wardd-app-teste` e `wardd-aprovador-teste` são marcas procuradas no release (`sem-modo-teste.sh`, `instalar.sh`). Renomear sem a recusa dupla da Fase 56 enfraqueceria a conferência.
    - Os `[[test]]` precisam mudar junto com o arquivo do teste, que é das Fases 25–27.
    - Os exemplos aparecem em vários scripts das Fases 5, 43 e 45.
  - Veja "Problemas".
- **`target/sem-modo-teste`, o alvo `teste-sem-modo-teste` e `dev/testes/sem-modo-teste.sh`:** são nome de pasta, de alvo e de arquivo, das Fases 5 e 6.
- **Os identificadores `modo_teste`** (`endurecimento.rs`, a função `com_modo_teste` e outros): são identificadores das Fases 8, 9 e 45.
- **O `.dev/logs/ward-ponte-teste.log` antigo no disco:** não foi movido nem apagado (os `.dev/` são da Fase 5). O próximo `make app-ponte` grava o nome novo.
- **O binário velho `target/debug/ward-ponte-teste`:** continua no `target/`. Nenhum script aponta mais para ele.
- **`docs/escopo.md`:** não tem nenhum dos nomes renomeados, então ficou sem mudança.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git mv` (3 pastas e 1 exemplo) | 0 |
| `cargo fmt --all` + `--check` (workspace e `app/src-tauri`) | 0 |
| `make compile`: origens, `cargo check --workspace --all-targets --all-features --locked`, Vite, `cargo check` do `app/src-tauri` com WebKitGTK, sintaxe de 48 shell, 35 Python e 7 JS. Rodou de novo depois do fmt | 0 |
| `cargo metadata --offline --locked --no-deps` (V1.8): pacotes `ward-platform`, `ward-test-bridge` e `ward-app-core`; features `test-mode` (6 pacotes) e `test-hooks`; nenhuma `modo-teste` nem `ganchos-teste`; `crates/ward-plataforma`, `crates/ward-ponte-teste` e `app/nucleo` não existem | 0 |
| `make english-check PATHS="crates/ward-platform crates/ward-test-bridge app/core Cargo.toml app/src-tauri/Cargo.toml crates/{ward,wardd,ward-nm}/Cargo.toml"`: nenhum achado nos nomes desta fase (pastas, pacotes, features, `container.rs`). Os achados que sobram são do conteúdo e dos nomes de arquivo internos, das Fases 7–9, 29–34 e 47 (45 nomes de arquivo como `lancador.rs` e `partida.rs`; o resto em comentários e identificadores) | 2 (relatório, esperado) |
| grep dos nomes antigos fora de `docs/mission/{arquivo,handoffs}`, `dev/fixtures/legacy`, glossário e PRD/plano/contrato: só os pontos listados em "Segurança" como intencionais | — |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos
1. **Linhas da seção 3 do glossário sem fase dona no plano.** O título da seção diz "Fase 4", mas o plano da Fase 4 só cita os três crates e a feature. O orchestrator precisa dar fase a estas linhas:
   - `wardd-app-teste` e `wardd-aprovador-teste`, binários e pastas. Sugestão: Fase 56, junto com a marca, porque o nome do binário é marca procurada no release e dá nome ao log.
   - Os exemplos `wardd-cliente-teste`, `pai-chrome`, `chrome-falso`, `dev-cofre` e `partida-conteiner`. Sugestão: as fases dos crates, ou a 43 e a 45 para os scripts que os usam.
   - Os `[[test]]` `trava`, `app_teste` e `app_valor`, que vão junto com as Fases 25 e 27.
2. **Marca da ponte.** O binário agora é `ward-test-bridge`, mas os literais de dentro (`USO` e `eprintln!`) ainda dizem `ward-ponte-teste`. A Fase 56 decide se troca os literais. Se trocar, as recusas (`instalar.sh`, `app-release.sh`, `sem-modo-teste.sh`, o plantio do `install.sh` e o `grep` do `dev/ponte.sh`) precisam aceitar as duas formas.
3. **A mensagem do `instalar.sh` ainda diz "modo-teste".** É texto exibido, congelado. A Fase 56 a troca junto com os três testes que a conferem.
4. **Estado do repositório:** `.gitignore`, `docs/escopo.md` e `crates/ward-core/src/lib.rs` já estavam modificados antes desta fase (Fase 3 e usuário), e há renomes indexados de `docs/mission/evidence/M0/…`. Não mexi neles.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/**` → `crates/ward-platform/**` (inclui `examples/conteiner.rs` → `examples/container.rs`) | movido (`git mv`) e alterado |
| `crates/ward-ponte-teste/**` → `crates/ward-test-bridge/**` | movido (`git mv`) e alterado |
| `app/nucleo/**` → `app/core/**` | movido (`git mv`) e alterado |
| `Cargo.toml`, `Cargo.lock`, `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock` | alterado |
| `crates/ward/Cargo.toml`, `crates/wardd/Cargo.toml`, `crates/ward-nm/Cargo.toml` | alterado |
| `app/src-tauri/build.rs`, `app/src-tauri/src/{main,area_gtk}.rs` | alterado |
| `crates/ward/src/{args,cliente,comandos,testes}.rs`, `crates/ward-nm/src/main.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs`, `crates/ward-core/tests/corpo_memoria.rs` | alterado |
| `crates/wardd/src/{agente_ssh,arquivos,cdp,config,execucao,externo,lancamento,lib,main,nucleo,playwright,ponte,servidor,ssh,trava}.rs`, `crates/wardd/{app-teste,aprovador-teste}/main.rs` | alterado |
| `crates/wardd/tests/{app_teste,app_valor,aprovacao,d62,externo,login,nucleo,playwright,run,socket,ssh,trava}.rs` | alterado |
| `app/src/lib/componentes/PopoverGerador.svelte`, `app/src/lib/ponte/{comandos,index,ponte-teste}.js`, `app/src/telas/{GeradorSenha.svelte,aprovacao.js,configuracoes.js,primeiro-uso.js}` | alterado (só comentários) |
| `Makefile`, `CLAUDE.md` | alterado |
| `dev/daemon.sh`, `dev/ponte.sh` | alterado |
| `dev/testes/{app-e2e,app-release,ataques-fundacao,canais-gst,deb,docker-plataforma,docker-uid,extensao,install,login,nm,playwright,politica-chrome,selecao-revelado,sem-modo-teste,socket-ataques,ssh-home,units,webkit-real}.sh`, `dev/testes/cli.py` | alterado |
| `dev/conteiner-{deb,install,plataforma,units}/dentro.sh`, `dev/conteiner-plataforma/falso.c` | alterado |
| `instalacao/{arvore.txt,deb.sh,instalar.sh}` | alterado (só comentários; mensagem e marcas intactas) |
| `docs/instalacao.md`, `docs/glossario.md` (linha `ganchos-teste` → `test-hooks`) | alterado |
| `docs/mission/handoffs/phase-4.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. No lugar dele, rodaram `make compile` (0), `cargo fmt --check` (0) e o `english-check` nos caminhos da fase, sem achado nos nomes da fase.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon iniciado).
- [x] Teste de ataque para cada (SEG) tocado: V3.15 e V3.37, com os nomes antigos recusados. Escritos, não rodados (regra 6).
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase. As linhas da seção 3 sem dono estão em "Problemas".
