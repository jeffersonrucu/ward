# Fase 23a — Exceção do modo-teste para o app de teste, ponte pelo daemon real e transporte de D65 a

## Implementado
- **Exceção do `modo-teste` (D63).**
  - O daemon de teste lê `WARD_DEV_APP` (`crates/wardd/src/config.rs`) e o `abrir_app` abre esse binário pelo lançador (`AbridorReal::de_teste`).
  - O processo aberto recebe o papel app sem o egid `ward-app`. Para isso precisa ser o pid vivo do lançador (pidfd), ter `PPid` igual ao daemon e não estar sob `ptrace` (`ProcessoPar::classificar_app_de_teste`, `AppDeTeste`, `Papeis::app_de_teste`).
  - Qualquer outro par segue o caminho normal: sem o grupo, fica agente.
  - Sem a feature, `AppDeTeste` tem um campo `Infallible` e não pode ser construído.
- **`BinarioDeTeste`** (`crates/ward-plataforma/src/lancador.rs`, só com `modo-teste`).
  - O caminho precisa ser absoluto, simples e ficar dentro da raiz do repositório (o pai de `WARD_DEV_DIR`), sem ser a própria raiz.
  - Nenhum componente pode ser link. O alvo precisa ser arquivo regular executável, do uid, sem o+w no arquivo nem nos pais.
  - O nome é restrito a `[A-Za-z0-9._-]`.
  - Tudo é conferido de novo a cada lançamento.
  - Se o binário se chama `ward-app`, recebe o ambiente de D57. Os demais recebem um ambiente mínimo fixo: HOME, PATH, XDG_RUNTIME_DIR, LANG, WARD_DEV_DIR e, se definido, WARD_DEV_SOCKET_DIR.
  - Saída em `.dev/logs/<binário>.log`: `O_NOFOLLOW`, append, modo 0600, com fstat conferido.
- **`make app-ponte` pelo daemon real.**
  - Sobe o daemon de teste com `WARD_DEV_APP` apontando para a ponte (`dev/daemon.sh`, com marca em `.dev/wardd.app`).
  - Se o daemon já estiver no ar com outro app, pede `make dev-daemon-parar`.
  - Pede `abrir_app` (`dev/abrir-app.py`, pedido sem campo nenhum) e imprime a URL que `dev/ponte.sh` lê do log da ponte.
  - Smoke manual feito:
    - a URL saiu e `estado_do_cofre` respondeu `travado` pela ponte;
    - a ponte executada à mão respondeu "o daemon não deu o papel app" e saiu com 1;
    - com `make dev-daemon-parar`, a ponte registrou "a conexão com o daemon de teste caiu" e saiu.
- **`make dev-daemon`** compila a ponte e passa `WARD_DEV_APP` (padrão: a ponte; dá para trocar com `make dev-daemon WARD_DEV_APP=…`).
- **Papel na primeira resposta.**
  - O app (`app/nucleo/src/partida.rs` › `conferir_papel`, novo `ErroPartida::Papel`) e a ponte (`crates/ward-ponte-teste/src/partida.rs` › `conectar_com_papel_app`) pedem `estado` antes de qualquer outra coisa. Se o papel não for app, saem sem usar a conexão.
  - Com o pai errado, o app pede `abrir_app` (a instância verdadeira) e sai. Esse comportamento já existia e agora passa pela mesma conexão.
- **Transporte do `DaemonSocket` (D65 a)** em `app/nucleo/src/daemon.rs`.
  - Uma conexão só, uma thread leitora, e um pedido por vez sob mutex.
  - Avisos (`pedido_de_aprovacao` e `pedido_de_destrave`) vão para `FilaAvisos`, limitada a 16. Excesso é descartado com log sem conteúdo.
  - Fecham a conexão: resposta sem pedido pendente, quadro acima do limite, quadro que não decodifica e tipo do papel agente.
  - Prazo de resposta de 130 s; se estourar, a conexão fecha e a resposta atrasada nunca vira resposta de outro pedido.
  - Pedido grande demais não sai e a conexão segue.
  - Buffers em `Zeroizing`.
  - `Drop` faz shutdown.
  - Reconexão só antes da primeira (D47).
  - Novo `vigiar_fechamento`, usado pela ponte.
- **Release sem `WARD_DEV_APP`.** O novo `make teste-sem-modo-teste` (`dev/testes/sem-modo-teste.sh`) faz três coisas:
  - roda `cargo tree -e normal,build` e confere que nenhum membro liga `wardd/modo-teste` ou `ward-plataforma/modo-teste`, com autoteste do caso forçado;
  - roda os testes de unidade do `wardd` e do `ward-plataforma` **sem** a feature;
  - roda `strings` no `wardd` de release e na rlib do `ward-plataforma`, sem `WARD_DEV_APP`, `WARD_DEV_DIR` ou `WARD_DEV_SOCKET_DIR`, nem os nomes do app ou da ponte de teste. No `wardd` também não pode haver `BinarioDeTeste`, `classificar_app_de_teste` ou `lancar_para_teste`.

  Além disso, `dev/testes/app-release.sh` procura `WARD_DEV_APP` no binário do app.
- **V3.34, lado do app, no contêiner da plataforma** (pedido do orchestrator depois do merge da 22b).
  - `docker run --ulimit memlock=-1:-1`.
  - O "daemon" do contêiner informa `vmlck` (maior que 0, `mlockall` de D62).
  - O app lançado relata `vmlck=0,kB`.
  - O servidor do contêiner agora responde o quadro `estado` ao app de teste, que confere o papel.
- `git merge master` feito (Fase 22b), sem conflitos.

## Segurança
- **Invariantes tocadas e como a fase as respeita:**
  - **D63 / S6 (papel por processo, não por pedido).**
    - A exceção só existe com `modo-teste`.
    - Só vale para o pid exato que o lançador do próprio daemon abriu, com `PPid` igual ao daemon e sem tracer.
    - Uma cópia do binário, ou o mesmo binário executado à mão, fica agente.
    - O `abrir_app` não aceita campo: binário, argv e ambiente vêm só da configuração do daemon.
    - `WARD_DEV_APP` fora do repositório, link, arquivo do sistema (`/usr/bin/true`) ou o+w faz o daemon se recusar a subir.
  - **S1 / S3 (segredo fora de argv e de arquivo).**
    - Nada de segredo no ambiente ou no argv do app de teste.
    - O log do app é 0600 em `.dev/logs/`.
    - O teste confere que o log do daemon não tem `falso-`.
  - **D65 a.**
    - O app não mostra nada vindo de quadro forjado ou de papel agente, e a conexão fecha.
    - A fila de avisos não cresce além de 16.
    - Resposta atrasada não é entregue a outro pedido.
  - **S8 / V3.34.** O app filho não herda o `mlockall` (conferido no contêiner).
- **Testes de ataque escritos:**
  - `crates/wardd/tests/app_teste.rs` › `v3_37_so_o_app_aberto_pelo_daemon_de_teste_e_app`. Cobre:
    - `WARD_DEV_APP=/usr/bin/true` é recusado;
    - cliente feito à mão fica agente;
    - `abrir_app` com campos (caminho, argumentos ou ambiente) dá erro e não abre nada;
    - binário executado à mão e cópia ficam agente, e `revelar` responde `nao_autorizado`;
    - nada falso aparece no log.
  - `crates/wardd/src/servidor.rs`:
    - `d63_sem_o_grupo_o_pid_do_lancador_nao_da_papel_app`;
    - `d63_par_filho_sem_egid_fica_agente` (sem a feature, roda em `make teste-sem-modo-teste`).
  - `crates/ward-plataforma/tests/identificacao.rs` › `app_de_teste_sem_egid_so_com_o_pai_daemon`.
  - `crates/ward-plataforma/src/lancador.rs`:
    - `ataque_binario_de_teste_fora_das_regras_e_recusado`;
    - `ponte_de_teste_recebe_o_ambiente_minimo_e_o_log_e_0600`;
    - `app_de_teste_recebe_o_ambiente_de_d57`.
  - `app/nucleo/src/daemon.rs`:
    - `v5_33_avisos_no_meio_vao_para_a_fila_e_a_resposta_para_quem_pediu`;
    - `v5_33_mil_avisos_a_fila_nao_passa_de_16_e_o_app_segue_respondendo`;
    - `v5_33_resposta_sem_pedido_fecha_a_conexao`;
    - `v5_33_quadro_acima_do_limite_fecha_a_conexao`;
    - `v5_33_tipo_do_papel_agente_ou_forjado_fecha_sem_mostrar_nada`;
    - `sem_resposta_no_prazo_fecha_e_a_atrasada_nao_vira_outra`;
    - `pedido_grande_demais_nao_sai_e_a_conexao_continua`.
  - `app/nucleo/src/partida.rs`:
    - `ataque_sem_papel_app_na_primeira_resposta_sai_sem_usar_a_conexao`;
    - `com_outro_pai_pede_a_instancia_verdadeira_e_sai`.
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_ponte_sem_papel_app_sai_na_primeira_resposta` (agente, extensão e app).
  - `dev/testes/sem-modo-teste.sh` (V3.37 no release).
  - `dev/conteiner-plataforma/dentro.sh`: o app tem `VmLck` 0 e o daemon tem `VmLck` maior que 0.
- **Dependências novas:** nenhuma. Há um binário novo, `wardd-app-teste` (`required-features = ["modo-teste"]`), que só existe para o teste de V3.37.

## Ficou de fora
- **O `main.rs` do Tauri não mudou.** A conferência do papel está em `partida::iniciar_com`, que ele já chama. A fila de avisos (`DaemonSocket::avisos`) ainda não tem consumidor na interface; isso é da fase da janela de aprovação.
- **`make teste-sem-modo-teste` não entra no `make check`.** O `make check` usa `--all-features` em `target/`, e esse alvo compila de novo em `target/sem-modo-teste` (release e testes). Cabe ao orchestrator decidir se ele entra no check ou no validador.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` | 0 |
| `cargo test -p wardd --features modo-teste` (todas as suítes) | 0 |
| `cargo test -p ward-app-nucleo` (com e sem `modo-teste`) | 0 |
| `cargo test -p ward-ponte-teste --all-features` | 0 |
| `make teste-sem-modo-teste` | 0 |
| `make teste-docker-plataforma` (70 ok, inclui V3.34 lado do app) | 0 |
| `make app-ponte` (smoke manual; `make dev-daemon-parar` no fim) | 0 |
| `make fmt` | 0 |
| `make check` (1ª vez: falhou só o `v3_33_kill_9_entre_as_senhas_erradas` da 22b, por tempo, ver Problemas) | 2 |
| `cargo test -p wardd --test socket v3_33` (3 vezes, isolado) | 0 |
| `make check` (2ª vez) | 0 |
| `make app-front` | 0 |
| `make app-check` (com o `PKG_CONFIG_PATH` do scratchpad, R32) | 0 |

## Problemas descobertos
- **Copiar pela ponte sem Wayland.** A ação de copiar da ponte depende do Wayland, e o ambiente mínimo do `BinarioDeTeste` não tem `WAYLAND_DISPLAY`. Pela ponte aberta via daemon, copiar falha. Não incluí a variável porque vir da sessão contraria o ambiente fixo. Se for preciso, o caminho é a regra de D57 (`ward-app`).
- **`ErroApp::SemProtocolo` foi removido.** Agora o transporte existe; o `main.rs` do Tauri não usava essa variante.
- **O ambiente mínimo leva `WARD_DEV_SOCKET_DIR` quando definido**, porque os testes com socket próprio precisam dele. O valor vem da configuração do daemon, não do pedido.
- **O `dist` da ponte sem argumentos sai de `WARD_DEV_DIR`** (`<repo>/app/dist-teste`). Sem `WARD_DEV_DIR` absoluto terminado em `.dev`, a ponte se recusa a subir.
- **Escrita de grupo é aceita.** A regra de escrita do `BinarioDeTeste` só recusa o+w, não g+w, porque o umask do usuário é 002 e os binários de `target/` saem g+w. Fica registrado como resíduo do modo-teste.
- **Token no log.** A URL com o token vai para `.dev/logs/ward-ponte-teste.log` (0600), legível por qualquer processo do uid. É resíduo de D63, aceitável só com o cofre de teste.
- **`abrir_app` só com o pai errado.** O app pede `abrir_app` apenas quando o pai está errado. Com o pai certo e papel diferente de app, sai sem pedir, para evitar laço.
- **Número de sockets no contêiner.** O teste de CLOEXEC passou a aceitar qualquer número de sockets (`socket_cloexec=true/`), porque com o transporte de D65 a esse número varia: são cópias da leitora e da vigia. Todos continuam conferidos com CLOEXEC.

- **Teste da 22b sensível a tempo.** O `crates/wardd/tests/socket.rs` › `v3_33_kill_9_entre_as_senhas_erradas` exige que a primeira espera fique abaixo de 900 ms, e com a máquina carregada (o `make check` inteiro) deu 907 ms. Isolado, passou 3 vezes. O código é da Fase 22b, então não mexi; sugiro uma margem maior.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `CLAUDE.md` | alterado |
| `Makefile` | alterado |
| `app/nucleo/examples/partida-conteiner.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/partida.rs` | alterado |
| `crates/ward-plataforma/examples/conteiner.rs` | alterado |
| `crates/ward-plataforma/src/lancador.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/processo.rs` | alterado |
| `crates/ward-plataforma/tests/identificacao.rs` | alterado |
| `crates/ward-ponte-teste/src/lib.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/src/partida.rs` | criado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/app-teste/main.rs` | criado |
| `crates/wardd/src/config.rs` | alterado |
| `crates/wardd/src/lancamento.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/servidor.rs` | alterado |
| `crates/wardd/tests/app_teste.rs` | criado |
| `dev/abrir-app.py` | criado |
| `dev/conteiner-plataforma/dentro.sh` | alterado |
| `dev/daemon.sh` | alterado |
| `dev/ponte.sh` | criado |
| `dev/testes/app-release.sh` | alterado |
| `dev/testes/docker-plataforma.sh` | alterado |
| `dev/testes/sem-modo-teste.sh` | criado |
| `docs/mission/handoffs/phase-23a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica (fase sem tela)
- [x] Ficou dentro do escopo da fase (o V3.34 do lado do app foi pedido pelo orchestrator)
