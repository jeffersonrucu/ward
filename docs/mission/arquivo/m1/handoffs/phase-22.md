# Fase 22 — `wardd`: socket, estado travado/destravado, PIN, nível, gerações

## Implementado
- `crates/wardd` virou lib + binário (`#![forbid(unsafe_code)]` nos dois).
  - `main.rs`: `iniciar_endurecido()` vem antes de tudo (exit ≠ 0 em erro). Depois:
    1. config;
    2. escuta: `LISTEN_FDS` (D46) ou, só no `modo-teste`, socket próprio num diretório 0700 (`preparar_diretorio`);
    3. `Instalacao::do_sistema` (sem os grupos, ninguém é app/extensão e todo cliente é agente);
    4. `servir`.
  - `config.rs`:
    - release: `$HOME/.local/share/ward/{cofre.ward,estado}`;
    - `modo-teste`: exige `WARD_DEV_DIR` (`cofre-teste.ward`, `cofre-teste.estado`) e o socket fica em `WARD_DEV_SOCKET_DIR` ou `$XDG_RUNTIME_DIR/ward-teste`;
    - os caminhos são conferidos nos bytes: absolutos, sem `.`, `..` nem `//`.
  - `arquivos.rs`:
    - leitura do cofre com `O_NOFOLLOW`: arquivo regular do euid, sem escrita de g/o, com limite de tamanho;
    - identidade (dev, ino, mtime, mtime_nsec, tamanho), conferida de novo depois de ler;
    - arquivo de estado estrito (`ward-estado 1` / `vista N` / `queimada N`, 0600, até 256 bytes, sem zero à esquerda, queimada ≥ vista), gravado de forma atômica (temporário `O_EXCL` 0600, fsync, rename, fsync do diretório).
  - `nucleo.rs` (estado e operações):
    - destravar com espera crescente global (D20);
    - travar zera o cofre aberto e as entradas;
    - S9: PIN conferido pelo hash em tempo constante (`Configuracao::conferir_pin`); 5 erros seguidos travam, contando entre conexões;
    - D50: a lista do agente sai sem bloqueadas e sem nível;
    - V3.17: bloqueada responde igual a inexistente;
    - entrada nova nasce "com aprovação" e editar mantém o nível;
    - `detalhe`, `salvar_entrada`, `remover_entrada`, `tempo_trava`, `mudar_nivel`, `trocar_pin`, `trocar_senha`, `criar_cofre` e `conferir_pin`, com desfazer na memória quando a gravação falha.
    - Gravação (D39/D53):
      1. confere a identidade (diferente → não grava, trava, `cofre_mudou`);
      2. pré-confere o conjunto (`serializar`);
      3. grava `queimada` = max(queimada, geração+1) no estado (se falhar, o cofre não é gravado);
      4. `gravar_entradas`;
      5. com sucesso, `vista` = geração.
      - Falha no fsync do diretório: atualiza a identidade (o rename aconteceu).
    - Reabrir:
      - lê (vista, queimada) e abre com `abrir_entradas_lida`;
      - sem estado e com cofre: `retrocesso`;
      - arquivo trocado com o daemon travado: `cofre_mudou` uma vez, e a tentativa seguinte decide só pela geração.
    - `trocar_senha`:
      - queima geração+1 antes;
      - `Formato` vira `senha_incorreta` (D52);
      - qualquer erro depois de selar trava.
    - Mutex envenenado trava o cofre.
  - `servidor.rs`:
    - limite de quadro por papel;
    - prazos de leitura (60 s) e escrita (10 s);
    - até 64 conexões; cheio, sai a parada mais antiga (`shutdown`); sem nenhuma parada, a nova recebe `ocupado`;
    - `reconferir_sem_depurador` antes de cada pedido de quem não é agente;
    - conexão recusada (`UidRecusado`/`SemIdentidade`/E-S) fecha sem mandar byte e vai ao log só com uid/pid.
  - `log.rs`: `aviso!` sempre; `depurar!` só com `RUST_LOG` debug/trace. Só textos fixos, códigos e números.
  - `examples/cliente-teste.rs` (`wardd-cliente-teste`): cliente mínimo, sem segredo, para o contêiner (que não tem Python).
- Mudanças no `ward-core`, necessárias para o daemon:
  - **Protocolo, D50:** `ItemLista.nivel` virou `Option<NivelAcesso>`. É `Some` só no papel app e vem de `ItemLista::de_entrada(e, papel)`. A codificação recusa nível numa lista do agente (`NaoPermitidaAoPapel`), e a leitura exige ou proíbe o campo conforme o papel.
  - **`corpo::abrir_entradas_lida`:** igual a `abrir_entradas_desde`, que agora delega a ela, mas devolve também a geração lida do corpo. Com isso a `vista` vai para a geração que está no disco, e não para uma geração queimada que nunca chegou lá.
- `Makefile` e `dev/`:
  - `make dev-daemon` / `make dev-daemon-parar` (`dev/daemon.sh`):
    - compila com `modo-teste` e sobe por `env -i` só com HOME (do passwd), `PATH=/usr/bin:/bin`, `XDG_RUNTIME_DIR`, `LANG` e `WARD_DEV_DIR` (V3.30);
    - log em `.dev/logs/wardd.log`, pid em `.dev/wardd.pid`;
    - sem estado, cria `vista 0 / queimada 0` (só no cofre de teste);
    - `parar` só mata o pid se `comm` for `wardd` e o `cmdline[0]` for o `target/debug/wardd` daqui. O `/proc/<pid>/exe` não serve, porque o daemon se marca não-dumpable.
  - `make dev-cofre` apaga o `.estado` velho depois de recriar o cofre.
  - `make teste-docker-uid` (`dev/testes/docker-uid.sh` + `dev/conteiner-uid/dentro.sh`): V3.2 num `ubuntu:24.04` (mesma digest da 21b), `--network none`.
- `CLAUDE.md`: só as linhas de `dev-daemon`, `dev-daemon-parar` e `teste-docker-uid`.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1:** a lista do agente sai sem segredo e sem nível. A bloqueada não aparece e responde igual a uma inexistente.
  - **S3:** papel pelo socket (egid, D47) e uid pelo `SO_PEERCRED`. Uid de fora fecha sem nenhum byte.
  - **S7/S13:** gerações (vista/queimada) e identidade do arquivo. Não grava por cima de arquivo trocado.
  - **S8:**
    - travar solta a chave e as entradas, que zeram (`Zeroizing`/`Segredo`);
    - senha e PIN só em tipos que zeram;
    - o log não tem valor (nem com `RUST_LOG=trace`).
  - **S9:**
    - PIN pelo hash Argon2id, comparado em tempo constante pela biblioteca;
    - 5 erros travam;
    - nível muda só com PIN, só pelo app;
    - entrada nova nasce com aprovação.
  - **V3.25/V3.30:** o processo recusa `TMPDIR`/`LD_*`, e o `make dev-daemon` sobe limpo.
  - Nenhum filho é criado nesta fase, então `LISTEN_*` não chega a ninguém.
- **Testes de ataque** (todos falham do lado do atacante)
  - `crates/wardd/tests/nucleo.rs`:
    - `v3_10_travar_e_tudo_responde_travado`;
    - `v3_9_senha_errada_espera_crescente_sem_contagem`: relógio falso, esperas 0,1,2,4,8,16,30,… e respostas iguais byte a byte;
    - `v3_11_mudar_nivel_exige_pin_e_papel_app`: sem PIN, PIN errado e agente com PIN certo; o arquivo não muda;
    - `v3_12_cinco_erros_de_pin_travam`: 4+acerto zera; `trocar_pin` conta;
    - `v3_17_v3_26_lista_do_agente_e_bloqueada_igual_inexistente`: `executar`/`ssh`/`login`/`playwright`;
    - `v3_27_entrada_nova_nasce_com_aprovacao`: nível no rascunho é recusado; remover e recriar volta com aprovação; túnel quebrado é recusado;
    - `v3_22_rollback_e_arquivo_trocado`: `cp` da cópia antiga, estado apagado, `mv` e reescrita com o daemon destravado e travado; estado 0600 sem segredo;
    - `v3_28_geracao_queimada_no_estado`: diretório 0300 (o fsync do diretório falha depois do rename), reinício entre A e B, cópia de A dá retrocesso, estado que não grava não deixa gravar o cofre, `trocar_senha` que falha depois de selar trava;
    - `d52_trocar_senha_com_senha_errada`;
    - `primeiro_uso_e_pin_so_como_hash`: agente não cria cofre; PIN e senha não estão no cofre nem no estado; troca de PIN.
  - `crates/wardd/tests/memoria.rs › v3_8_travar_zera_a_chave_e_os_segredos_antes_de_liberar`:
    - Alocador global observador (`unsafe` só no teste, D31). A chave de dados é tirada do arquivo por Argon2id + XChaCha20-Poly1305.
    - Nenhum bloco liberado durante destravar, usar e travar tem a chave, a senha, o PIN ou os segredos.
    - O controle confirma que o detector acha a chave num bloco sem zerar.
  - `crates/wardd/tests/socket.rs`:
    - `v3_30_ambiente_com_tmpdir_ou_ld_recusa_subir`: `TMPDIR`, `LD_LIBRARY_PATH`, `LD_PRELOAD` e falta de `WARD_DEV_DIR` fazem o daemon sair ≠ 0 sem criar o socket;
    - `v3_20_ciclo_pelo_socket_sem_segredo_no_log`: binário real com `RUST_LOG=trace`. Cobre senha errada e certa, lista sem bloqueada nem nível, bloqueada = inexistente, agente sem `mudar_nivel`, 70 conexões paradas (a mais antiga sai, a nova é atendida), quadro de 4 GiB e JSON inválido; no fim, nem o log nem a resposta têm senha, PIN ou segredo, em claro ou em base64.
  - `crates/wardd/src/config.rs › caminho_relativo_ou_com_ponto_ponto_recusado` e `modo_teste_exige_dev_dir`.
  - `crates/wardd/src/arquivos.rs › estado_estrito`.
  - `crates/ward-core/src/protocolo/testes.rs › d50_lista_do_agente_sem_nivel_e_do_app_com_nivel`.
  - `make teste-docker-uid` (V3.2), rodado uma vez com 7/7 ok:
    - uid 4242 recebe EACCES;
    - uid 4242 com `CAP_DAC_OVERRIDE` ambiente e o root são fechados com `bytes: 0`, e o log mostra `conexão recusada (UidRecusado): uid 4242` e `uid 0`;
    - uid 1000 é atendido.
- **Dependências do `wardd`** (todas já estavam no workspace e no `Cargo.lock`; nenhuma crate nova no lockfile):
  - `libc`: só as constantes `O_NOFOLLOW`/`O_CLOEXEC` do `custom_flags`. Nenhuma chamada.
  - `zeroize`: `Zeroizing` para a senha e os buffers do daemon.
  - dev-deps `argon2` e `chacha20poly1305`: só no teste V3.8, para desembrulhar a chave de dados do arquivo de teste e procurá-la na memória.

## Ficou de fora
- `revelar`, `copiar`, `totp`, aprovação, `emitir_token`, `sugerir`, `preencher` e `salvar_login` respondem `interno`, e `executar`/`ssh`/`login`/`playwright` de entrada existente também. As fases seguintes do M3/M4 implementam isso (aqui só há a parte de nível/bloqueio). `entrada_padrao`/`janela` respondem `pedido_invalido`.
- Trava automática por tempo e por suspensão: ficam para a fase da trava. Aqui só o `tempo_trava` é gravado.
- `make teste-home-real`: não rodei nenhum cliente ssh/curl nesta fase, então não foi necessário.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-servicos`, `make dev-cofre` | 0 |
| `cargo test -p wardd --all-features --test nucleo` | 0 |
| `cargo test -p wardd --all-features --test memoria` (repetido 3× depois de `touch`) | 0 |
| `cargo test -p wardd --all-features --test socket` | 0 |
| `TMPDIR=/tmp LD_LIBRARY_PATH=/nao-existe make dev-daemon` | 0 |
| `wardd-cliente-teste … estado` / `lista` (travado) | 0 (`resposta: estado`, `resposta: erro travado`) |
| destravar o `.dev/cofre-teste.ward` pelo socket (senha de teste pelo stdin) | 0 (20 itens = 21 da fixture − 1 bloqueada, nenhum com nível; estado foi a vista 1) |
| `make dev-daemon-parar` (duas vezes: para e depois "não está no ar") | 0 |
| `make teste-docker-uid` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **Contador do D20 e dos erros de PIN só em memória:** um reinício do daemon zera a espera. Quem reinicia é o usuário (systemd), não o agente, mas fica registrado. Persistir no estado seria fácil; não fiz por estar fora do texto do escopo.
- **A espera do D20 é global:** um agente que manda senhas erradas atrasa o destravar do usuário em até 30 s. É o que o D20 pede ("global entre conexões"), mas é um DoS de conveniência. O planner decide se o app ganha caminho próprio.
- **Sem `mlock` da chave de dados:** o `CofreAberto` do `ward-core` não expõe o buffer, e fazer isso exige mudar o núcleo e um módulo de plataforma com `unsafe`. Fica para quem tratar de swap/core dump (o `iniciar_endurecido` já desliga o core dump).
- **O daemon de teste não dá o papel app:** sem os grupos `ward-app`/`ward-nm`, todo cliente é agente. A ponte de teste (D7) e as telas que precisam de `criar_cofre`/`mudar_nivel` não conseguem esses pedidos contra o `make dev-daemon`. A fase da ponte ou a 29a precisa definir como o modo de teste reconhece o app sem setgid e sem abrir brecha no release.
- **`criar_cofre` sobrescreve um estado velho** quando não há cofre (o estado começa em vista 0, queimada 1). Sem cofre, não há o que proteger contra retrocesso, mas um cofre apagado e recriado perde a memória da geração. Registro sem mudar.
- **Janela de TOCTOU curta:** entre o rename do `gravar_entradas` e o `stat` que pega a identidade nova, um processo do mesmo uid poderia trocar o arquivo, e a troca seria aceita como sendo do Ward. Mesmo uid já é fora do modelo (S3), mas a identidade só fecha isso de verdade se o `ward-core` devolver o fd do temporário.
- **Falso positivo no detector de memória (resolvido no teste):** a primeira versão copiava as marcas para a pilha a cada `dealloc`. Esse resto voltava ao heap pelos bytes de enchimento de um `enum` movido (o `Vec<Entrada>` realocado) e dava 6 achados do segredo, que mudavam com qualquer mudança de código. As marcas agora ficam num `RefCell` lido por referência, e a chave do teste fica em `Box`. Conclusão para o produto: resto de pilha pode ir parar no heap pelo enchimento. Hoje o daemon não deixou segredo nenhum na pilha que o teste visse, mas código futuro que copie segredo para array na pilha pode criar o mesmo vazamento.
- **Duas mudanças no `ward-core`:** o protocolo (D50, `ItemLista.nivel` opcional) e `abrir_entradas_lida`. As duas são necessárias para o daemon e têm teste; o validador do M2 deve olhar.
- **O handler de SIGSEGV do runtime do Rust** continua instalado (ele só imprime "stack overflow"). Não tem valor de segredo, mas é registrado para a auditoria de sinais.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/lib.rs` | criado |
| `crates/wardd/src/arquivos.rs` | criado |
| `crates/wardd/src/config.rs` | criado |
| `crates/wardd/src/log.rs` | criado |
| `crates/wardd/src/nucleo.rs` | criado |
| `crates/wardd/src/servidor.rs` | criado |
| `crates/wardd/examples/cliente-teste.rs` | criado |
| `crates/wardd/tests/comum/mod.rs` | criado |
| `crates/wardd/tests/nucleo.rs` | criado |
| `crates/wardd/tests/memoria.rs` | criado |
| `crates/wardd/tests/socket.rs` | criado |
| `crates/ward-core/src/corpo.rs` | alterado |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `Cargo.lock` | alterado (só as dependências do `wardd`) |
| `Makefile` | alterado |
| `dev/daemon.sh` | criado |
| `dev/testes/docker-uid.sh` | criado |
| `dev/conteiner-uid/dentro.sh` | criado |
| `CLAUDE.md` | alterado (só as linhas de `dev-daemon`, `dev-daemon-parar` e `teste-docker-uid`) |
| `docs/mission/handoffs/phase-22.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica: fase sem UI)
- [x] Ficou dentro do escopo da fase
