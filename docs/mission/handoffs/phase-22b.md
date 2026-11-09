# Fase 22b — Problemas do handoff 22 (D62): espera do D20 por papel e no estado, `mlockall`, geração do `criar_cofre` e identidade pelo fd

## Implementado
- **Espera do D20 por papel e guardada no estado** (`crates/wardd/src/nucleo.rs`, `arquivos.rs`):
  - dois contadores no `Estado`: `falhas_agente` (papel agente, inclui o `ward destravar`; extensão contaria como agente, mas o protocolo já não deixa ela destravar) e `falhas_app` (só em memória);
  - um mutex de vez por papel (`vez_agente`, `vez_app`) segura a espera e a tentativa: a espera do agente não segura o app. O app ainda espera o Argon2 de uma tentativa do agente em andamento (mutex do estado), não a espera do D20;
  - a falha do agente vai para o arquivo de estado (`gravar_estado_completo`: temporário, fsync, rename, fsync do diretório) **antes** de a resposta de erro sair; se não der para gravar, vai ao log e fica em memória;
  - formato do estado: linha nova `falhas_agente N` depois de `queimada`, com o mesmo cabeçalho `ward-estado 1`. O leitor estrito aceita o formato antigo (três linhas) como 0, e toda gravação regrava no novo. A linha é estrita como as outras (sem zero à esquerda, até `u32::MAX`, na ordem);
  - ao subir, `Daemon::novo` lê `falhas_agente` do estado; a primeira tentativa do agente espera o que falta de `espera_destrave(n)`, contado da partida (relógio ilegível: a espera inteira); as seguintes, a espera inteira;
  - o destravar certo de qualquer papel e o `criar_cofre` zeram os dois, no disco também (o destravar regrava o estado se o contador da memória ou o do disco não for 0). Travar (inclusive a trava automática da Fase 24) não mexe neles.
- **`mlockall`** (`crates/ward-plataforma/src/endurecimento.rs`, `sys.rs`):
  - `iniciar_endurecido` agora devolve `Memoria` (`Travada` ou `SemTravar(motivo)`). Por último, lê `RLIMIT_MEMLOCK`:
    - infinito: `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)`;
    - finito, erro na leitura ou erro no `mlockall`: no release, `ErroInicio::LimiteMemlock`/`TravarMemoria` (o `wardd` sai ≠ 0 com a mensagem e sem socket); no `modo-teste`, `SemTravar`, e o `wardd` avisa no log e segue.
  - Com limite finito, o `mlockall` nunca é chamado: o `MCL_FUTURE` faria o Argon2 falhar.
  - A regra fica em `decidir_memoria` (pura), para o teste cobrir os dois builds.
  - `sys::rlimit_memlock` e `sys::travar_toda_memoria` levam `// SAFETY:`.
- **`criar_cofre` com estado velho** (`nucleo.rs`, `ward-core/src/formato.rs`):
  - `CofreAberto::depois_da_geracao(q)` é novo no `ward-core` (só sobe a geração, nunca desce).
  - Sem cofre e com estado de `queimada` q, o cofre nasce na q+1, o estado vai a q+1/q+1 e o log avisa.
  - Sem estado, continua 0/1 → 1/1. Estado ilegível e sem cofre: recusa com `falha_ao_gravar` (ver Problemas). `queimada = u64::MAX`: `geracao_esgotada`.
- **Identidade pelo fd** (`ward-core/src/gravacao.rs`):
  - `IdentidadeArquivo` (dev, ino, mtime, mtime_nsec, tamanho), feita por `From<&fs::Metadata>`. O `wardd::arquivos::Identidade` passou a ser esse tipo.
  - `gravar_entradas` devolve `Result<IdentidadeArquivo, _>`, lida por `fstat` do fd do temporário depois do fsync e antes do rename. `gravacao::trocar_senha` devolve `(CofreAberto, IdentidadeArquivo)`.
  - `ErroGravacao::Io` ganhou `identidade: Option<IdentidadeArquivo>`, `Some` só em `Etapa::FsyncDiretorio` (o rename aconteceu).
  - O daemon usa só essa identidade (`confirmar`, falha do fsync do diretório). Não faz mais nenhum `stat` do caminho depois do rename; o `arquivos::identidade` ficou só nas conferências de antes de gravar.
- **Gancho de teste** `Daemon::com_gancho_depois_de_gravar` (`#[doc(hidden)]`, sem `cfg(feature)`, porque o V3.15 proíbe isso no `nucleo.rs`):
  - chamado logo depois de cada `gravar_entradas`/`trocar_senha`, antes de o daemon olhar o resultado;
  - o binário nunca liga, e nenhum pedido do socket chega a ele.
- **Teste instável ETXTBSY** (`crates/ward-plataforma/tests/comum/mod.rs`):
  - `iniciar` usa `subir_com_nova_tentativa`, que repete o `spawn` só em `ETXTBSY` (até 100 × 20 ms; outro erro falha na hora);
  - o `ataque_binario_copiado_para_fora_da_instalacao` copia por `copiar_binario` (copia e faz `sync_all`). A causa é o fd de escrita da cópia herdado por um `fork` de outra thread antes do `execve`.
- **Contêiner das units** (`dev/testes/units.sh`, `dev/conteiner-units/dentro.sh`): V3.34 no `wardd` de release.
  - O `units.sh` leva o `.dev/cofre-teste.ward` (0600). O `dentro.sh` o instala no home do uid 1000 com estado 0/0.
  - Destrava pelo stdin com a senha de teste e roda `lista`.
  - Confere `VmLck > 0` e `VmLck ≥ VmRSS`.
  - Roda o mesmo binário com `prlimit --memlock=8 MiB`: tem de sair ≠ 0 com "LimitMEMLOCK=infinity" e sem chegar a "pronto".
  - O `wardd-cliente-teste` ganhou `destravar`: senha do stdin (capacidade fixa, sem realocar) num `Segredo`, nunca em argv.
- `CLAUDE.md`: só a linha do `make teste-units`.

## Segurança
- **Invariantes tocadas**
  - **S9 / D20:**
    - o agente não ganha tentativas matando o `wardd`: o contador vai ao disco com fsync antes da resposta e volta ao subir;
    - erro do agente não atrasa o app;
    - a resposta continua sem a contagem;
    - o estado só tem números.
  - **S8 / D14:**
    - release só sobe com a memória inteira travada, inclusive a do Argon2;
    - falha fechada sem `LimitMEMLOCK=infinity`;
    - filhos (`fork` + `execve`) não herdam o travamento.
  - **S7 / D39 / D53:**
    - o cofre recriado sobre um estado velho nasce acima da `queimada`, e a cópia antiga é retrocesso;
    - a identidade é a do fd que o Ward escreveu, então uma troca logo depois do rename vira "o arquivo do cofre mudou fora do Ward", trava e não grava.
  - **S13:** os logs novos têm só números e textos fixos (contagem do agente na partida, geração do estado velho, motivo do memlock).
  - **S3:** a senha do cliente de teste do contêiner vem do stdin.
- **Testes de ataque** (todos falham do lado do atacante)
  - `crates/wardd/tests/d62.rs`:
    - `v3_33_espera_do_agente_sobrevive_ao_reinicio`: 10 erradas, com o contador no disco a cada resposta; daemon descartado e novo → 30 s; contado da partida (20 s → 10 s; 45 s → 0, depois 30 s); o certo zera no disco;
    - `ataque_agente_no_meio_da_espera_nao_atrasa_o_app`: agente preso na espera de 4 s numa thread, e o app destrava com espera 0; depois os dois zeram, e o erro de um não atrasa o outro;
    - `v3_33_formato_antigo_e_estado_sem_segredo`: três linhas → 0, regravado com `falhas_agente 1`; o app não vai ao disco; sem senha, PIN, senhas erradas nem segredos (em claro e em base64), só números;
    - `v3_35_criar_cofre_com_estado_velho_e_copia_antiga`: 7/7 → cofre na 8, estado 8/8; cópia de um cofre anterior na geração 5 → `retrocesso` ("o cofre no disco é mais antigo que o último aberto"), também num daemon novo;
    - `v3_35_criar_cofre_sem_estado_e_com_estado_invalido`;
    - `v3_35_troca_logo_depois_do_rename_e_recusada`: o gancho troca o arquivo (outro inode, mesmos bytes) logo depois do rename, em `tempo_trava` e em `trocar_senha`; a gravação seguinte responde `cofre_mudou`, trava e o arquivo trocado fica intacto. Com o `stat` antigo depois do rename, este teste passaria a gravar.
  - `crates/wardd/tests/socket.rs › v3_33_kill_9_entre_as_senhas_erradas` (binário real): três ciclos de subir → senha errada → `kill -9`. O contador está no disco a cada resposta, e as esperas medidas são ~0 s, ≥0,5 s e ≥1,5 s. O log não tem senha, e com o limite comum tem o aviso "memória sem travar" (V3.34, `modo-teste`).
  - `crates/wardd/src/arquivos.rs › estado_estrito` (linhas ruins da `falhas_agente`) e `d62_formato_antigo_le_zero_e_novo_tem_a_linha`.
  - `crates/ward-plataforma/src/endurecimento.rs › d62_memoria_release_falha_fechado_e_modo_teste_avisa`: 8 MiB e erro do `mlockall` → erro no release e aviso no `modo-teste`; com limite finito, o `mlockall` não é chamado.
  - `crates/ward-plataforma/tests/endurecimento.rs`:
    - `v3_34_memlock_de_8_mib`: `prlimit` de 8 MiB no processo real; a mensagem traz o limite e a diretiva;
    - `v3_34_filho_nao_herda_o_mlockall`: o processo travado tem `VmLck > 0` e o `cat` que ele executa tem `VmLck: 0 kB`;
    - `sobe_limpo_sem_ld_e_sem_depurador` ajustado aos dois builds.
  - `crates/ward-core/src/gravacao/testes.rs`:
    - `d62_identidade_do_fd_e_troca_logo_depois_do_rename`;
    - `d62_cofre_novo_depois_da_geracao_queimada`;
    - `v2_9_falha_de_es_injetada_em_cada_ponto` e o teste de queima em cada ponto: depois do rename a identidade do erro é a do arquivo no lugar, antes é `None`;
    - `v2_53_…` com a identidade no erro do fsync do diretório.
  - `make teste-units` (contêiner, `wardd` sem `modo-teste`), 25 ok:
    - destrava o cofre de teste;
    - `VmLck 211268 kB ≥ VmRSS 7052 kB`;
    - 8 MiB → sai 1, com a mensagem e sem "pronto";
    - os 17 pontos da Fase 24 continuam ok.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **V3.34, "o app lançado tem `VmLck: 0 kB`" no contêiner:** o contêiner das units não tem app. O da plataforma (21b/29a) é da 23a em paralelo, e o pedido era tocar o mínimo no lançador. A não-herança está coberta no host por `v3_34_filho_nao_herda_o_mlockall` (mesmas flags, `fork` + `execve`). O validador pode conferir no contêiner da 23a, que lança o app de teste.
- **Erros de PIN:** continuam em memória, como D62 decide.
- `servidor.rs` e lançador: não foram tocados.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-cofre` | 0 |
| `cargo test -p ward-core --all-features --locked` | 0 |
| `cargo test -p ward-plataforma --all-features --locked --test endurecimento --test identificacao --lib` | 0 |
| `cargo test -p wardd --all-features --locked` (inclui `d62`, `socket`) | 0 |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 |
| `make teste-units` | 0 |
| `make fmt` | 0 |
| `make check` (duas vezes, a 2.ª depois do ajuste do contador no disco) | 0 |

## Problemas descobertos
- **`criar_cofre` com estado ilegível recusa** (`falha_ao_gravar`) em vez de começar do zero: falha fechada, mas o usuário fica sem criar cofre até apagar o arquivo de estado, e a mensagem do app não diz isso. Apagar o estado (em vez de corromper) já leva ao começo do zero, e isso é a família de RA4. O planner decide se a recusa fica, se ganha erro próprio ou se vira começo do zero.
- **O app ainda espera o Argon2 de uma tentativa do agente em andamento** (o mutex do estado fica preso durante a abertura, ~1 s). A espera do D20 do agente não o atinge. Separar o Argon2 do mutex do estado é mudança maior no núcleo.
- **Gancho de teste no `Daemon` sem `cfg(feature)`:** `com_gancho_depois_de_gravar` existe também no release, só pela API da lib. O binário não liga, e nada do socket chega a ele. Foi assim porque o `v3_15_sem_aprovador_no_binario` proíbe `cfg(feature)` no `nucleo.rs`. A auditoria pode preferir outra forma.
- **Contador do agente no estado:** quem reescreve o arquivo de estado zera o contador. É o resíduo de D62, da família de RA4, sem trabalho.
- **`ErroGravacao::Io` mudou de forma** (campo `identidade`). Quem constrói o erro precisa do campo novo (hoje só os testes do `ward-core`).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/gravacao.rs` | alterado |
| `crates/ward-core/src/gravacao/testes.rs` | alterado |
| `crates/ward-core/src/formato.rs` | alterado (`depois_da_geracao`) |
| `crates/ward-core/src/corpo/testes.rs` | alterado (retorno da `trocar_senha`) |
| `crates/ward-plataforma/src/endurecimento.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado (`rlimit_memlock`, `travar_toda_memoria`) |
| `crates/ward-plataforma/src/lib.rs` | alterado (`Memoria`) |
| `crates/ward-plataforma/tests/comum/mod.rs` | alterado (ETXTBSY) |
| `crates/ward-plataforma/tests/identificacao.rs` | alterado (ETXTBSY) |
| `crates/ward-plataforma/tests/endurecimento.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado (aviso da memória) |
| `crates/wardd/examples/cliente-teste.rs` | alterado (`destravar` pelo stdin) |
| `crates/wardd/tests/d62.rs` | criado |
| `crates/wardd/tests/socket.rs` | alterado (`kill -9`) |
| `dev/testes/units.sh` | alterado |
| `dev/conteiner-units/dentro.sh` | alterado |
| `CLAUDE.md` | alterado (linha do `make teste-units`) |
| `docs/mission/handoffs/phase-22b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase
