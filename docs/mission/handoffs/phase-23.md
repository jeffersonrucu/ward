# Fase 23 — Motor de aprovação, `abrir_app` (D57) e papel app pelo pid do lançador (D63)

## Implementado
- **Motor de aprovação** (`crates/wardd/src/aprovacao.rs`, novo):
  - pedido pendente com id; vai à conexão do papel app (D65 a) ou, sem app
    conectado, o motor abre o app pelo lançador. Sem como abrir: "aprovação
    indisponível" na hora (D21);
  - prazo de 2 min pelo relógio injetado (`RelogioMotor`) → `expirado` (S10);
    conexão do app que cai com o pedido entregue → `negado` ("fechar nega"); app que
    sai sem ter recebido o pedido → "aprovação indisponível"; CLI que cai → cancela;
  - cofre travado num uso do agente: `pedido_de_destrave` ao app e espera até 2 min,
    contada em separado da aprovação (D16) → `travado_tempo_esgotado`. Sem app que
    possa abrir, `travado` na hora (ver Problemas);
  - traits `AbridorApp`, `Assinante`, `Cliente`; `SemApp`/`SemCliente`. Não há
    aprovador no binário: os testes assinam um `Assinante` em processo e respondem por
    `Daemon::atender` com o papel app (D8).
- **Uso do agente** (`crates/wardd/src/uso.rs`, novo): `Daemon::autorizar_uso`
  (destrave → entrada, bloqueada = inexistente → nível combinado D19, o mais restritivo
  entre a entrada e o túnel do `banco`/salto do `servidor`; a outra bloqueada ou sumida
  nega sem perguntar; ambiente mais sensível no pedido) e `responder_aprovacao`:
  - "Negar" sem PIN; os três "Permitir" com PIN pela contagem do núcleo (S9). PIN
    errado → `pin_incorreto` e o pedido continua pendente; o 5.º erro trava e nega;
  - "uma vez": só aquele pedido; "1 hora" (D2): liberação por entrada no `Aberto`
    (some ao travar, ao mudar o nível, ao remover e ao renomear); "sempre": as entradas
    do pedido "com aprovação" viram "livre" no disco (gravação com desfazer; falhou →
    o pedido continua pendente).
  - `executar`/`ssh`/`login`/`playwright` autorizados ainda respondem `interno` (a
    execução é das Fases 26/27).
- **D51** (`crates/ward-core/src/exibicao.rs`, novo): `exibir_argumentos` (cada
  argumento entre aspas, `\"` e `\\`, sem ambiguidade), `exibir_texto` e `neutralizar`
  (idempotente sobre a saída das outras). Controle C0/C1/`DEL` → `\xNN` (`\n`, `\r`,
  `\t`), formatação Unicode (bidi, largura zero, BOM, tags, separadores, seletores,
  preenchedores) → `\u{XXXX}`, byte inválido → `\xNN`; corte com aviso e número de
  bytes, sem partir escape. O daemon monta `comando` e `cadeia` do
  `PedidoDeAprovacao` com elas.
- **Protocolo** (`ward-core`):
  - `abrir_app` (todos os papéis, sem campo; campo a mais recusa o pedido);
  - avisos ao app (D65 a): só a conexão do papel app recebe `pedido_de_aprovacao` e
    `pedido_de_destrave`, cada um num quadro inteiro, nunca no meio de uma resposta
    (uma trava de escrita por conexão); as respostas seguem a ordem dos pedidos (uma
    thread por conexão). Essa conexão fica sem prazo de leitura; ao fechar, o que ela
    recebeu e não respondeu é negado. Não há operação de assinatura: uma
    `receber_pedidos` que existiu numa versão intermediária saiu ao alinhar com D65;
  - D63: `estado` traz `papel` (`agente`/`app`/`extensao`), sem outro dado. O daemon
    recusa escrever a um papel o `estado` de outro; o cliente lê o que veio;
  - `Papel::de_nome`.
- **D63, papel app só ao pid do lançador:**
  - `Lancador` guarda o pidfd do app (`pidfd_open` logo depois do `spawn`, com o filho
    ainda sem colher; sem pidfd o app é morto e não abre) e expõe
    `e_o_app_lancado(pid)` (mesmo pid e pidfd vivo);
  - classificação num ponto só: `wardd::servidor::papel_da_classe` (usada por
    `Papeis::papel`), que exige `ClassePar::App` (grupo e pai, D47) **e** o pid do
    lançador. Sem exceção de teste (é da 23a).
- **Código de saída do app** (pendência da 29a): `Lancador` guarda o `ExitStatus` ao
  colher (`tomar_saida`, `pid`); a vigia do `wardd` (`lancamento.rs`, novo) registra
  `app (pid N) saiu com código C` ou `encerrado pelo sinal S`, só números.
- `main.rs`: com a instalação, o motor usa o `AbridorReal` (`/usr/local/bin/ward-app`)
  e o `Papeis` recebe o `AppLancado`; sem ela, `Motor::sem_app()`.
- `ward-plataforma`: `par_fechou` (poll sem ler) para o daemon ver a CLI que caiu;
  `sys::fechamento` generaliza o poll de `esperar_fechamento`.

## Segurança
- Invariantes tocadas:
  - **S10:** expira em 2 min, fechar nega, conexão que cai nega; a resposta só vem do
    papel app (tabela do protocolo, conferida também no `atender`).
  - **S9:** todo "Permitir" passa pela contagem de PIN do núcleo; o 5.º erro trava e
    nega; mudar o nível acaba com a liberação de 1 hora.
  - **S1:** nenhuma resposta nova leva segredo; `pedido_de_*` só para o app; o agente
    não assina nem responde; D63 fecha o `ward-app` setgid aberto por outro filho do
    daemon; bloqueada continua igual a inexistente.
  - **D51:** o agente não consegue mostrar outro comando (escape visível, forma sem
    ambiguidade, corte com aviso).
  - **D57:** `abrir_app` sem campo: nada do pedido chega ao ambiente do app.
  - **S13:** o log só tem ids, códigos e números (sem nome de entrada, comando ou
    cadeia).
- Testes de ataque escritos:
  - `crates/wardd/tests/aprovacao.rs`: `v3_14_sem_resposta_expira_em_2_min`,
    `v3_14_negar_e_fechar_negam`, `v3_14_uma_vez_so_aquele_pedido`,
    `v3_14_uma_hora_ate_59_59_e_travar_encerra`, `v3_14_sempre_deixa_livre_no_arquivo`,
    `ataque_permitir_sem_pin_valido`, `ataque_agente_responde_a_aprovacao` (V3.15/V5.20),
    `v3_15_sem_aprovador_no_binario`, `v3_16_prod_tem_os_mesmos_niveis_e_botoes`,
    `d19_nivel_combinado`, `d21_aprovacao_indisponivel`,
    `app_aberto_recebe_o_pedido_ao_assinar`, `d51_comando_e_cadeia_com_escape_visivel`,
    `cliente_que_cai_cancela`, `v3_18_espera_o_destrave_ate_2_min`, `d57_abrir_app`,
    `v3_36_so_o_pid_do_lancador_e_app`, `v3_38_estado_traz_o_papel`;
  - `crates/wardd/tests/socket.rs › v3_20_ciclo_pelo_socket_sem_segredo_no_log`: pelo
    binário, `responder_aprovacao` do agente → não autorizado, `receber_pedidos` →
    operação desconhecida;
    `abrir_app` e uso com aprovação → aprovação indisponível; `estado` com `papel`;
  - `crates/wardd/src/servidor.rs › assinante_escreve_quadro_inteiro_e_falha_fechado`;
  - `crates/wardd/src/lancamento.rs › saida_vai_ao_log_so_com_numeros`;
  - `crates/ward-core/src/exibicao.rs`: `ataque_controle_e_bidi_viram_escape_visivel`,
    `ataque_imitar_escape_ou_separador_nao_engana`, `corte_com_aviso_nunca_em_silencio`,
    `neutralizar_e_idempotente_sobre_a_saida`, mais 4;
  - `crates/ward-core/src/protocolo/testes.rs`: `v3_38_estado_traz_o_papel_e_so_ele`,
    tabela do V3.15 com `abrir_app`, fuzz ajustado;
  - `crates/ward-plataforma/src/lancador.rs`: `ataque_so_o_pid_do_lancador_e_o_app`
    (outro filho vivo do mesmo pai não é o app; morto e colhido, nem o pid dele),
    `saida_do_app_fica_para_o_log`; `socket.rs › par_fechou_sem_consumir_bytes`.
- Dependências novas: nenhuma.

## Ficou de fora
- **Exceção do `modo-teste`, `WARD_DEV_APP`, ponte e app conferindo o papel:** Fase 23a
  (D63). O daemon de teste continua sem app (todo cliente é agente; com aprovação →
  indisponível).
- **V3.36 no contêiner** (app de teste setgid aberto pelo lançador do `wardd` e
  reexecutado com o mesmo pid): o contêiner da 21b/29a usa o `ward-plataforma-conteiner`
  como "daemon", que chama `classificar` direto, sem o pidfd. Ficou só a parte do
  classificador, como a nota do plano pede. `make teste-docker-plataforma` não foi
  rodado (não toquei no contêiner; `classificar` não mudou).
- Execução de verdade depois da aprovação: Fases 26/27.
- Lado do app: thread leitora e fila dos avisos (23a, D65 a), janela (Fase 36).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-cofre` | 0 |
| `git merge master` (D61–D64; fast-forward, sem conflito) | 0 |
| `git merge master` depois do 1.º commit (D65, Fase 24): conflitos em `wardd/src/lib.rs`, `main.rs` e `nucleo.rs`, resolvidos juntando os dois lados (motor + trava; `atender_de` com o `registrar_uso` da 24) | 1, resolvido |
| `cargo test -p ward-core --locked` | 0 |
| `cargo test -p ward-plataforma --lib --locked` (lancador, socket) | 0 |
| `cargo test -p wardd --all-features --locked --test aprovacao` (1.ª: 2 falhas de teste, nome do nível e limite da cadeia; corrigidas) | 101, 0 |
| `cargo test -p wardd --all-features --locked --lib --test socket` | 0 |
| `make fmt` | 0 |
| `make check` (1.ª: clippy `type_complexity` num teste; 2.ª: teste do assinante instável porque outro teste em paralelo faz `fork` e segura o fd até o `exec`; corrigido com nova tentativa limitada) | 2, 2, 0 |
| `make check` depois do merge da Fase 24 (1.ª: `v3_15_sem_aprovador_no_binario` pegou o `env::var` do `trava.rs`; lista ajustada) | 2, 0 |

## Problemas descobertos
- **Transporte ao app (D65 a):** o daemon empurra os avisos na conexão do papel app
  logo que ela é aceita; o lado do app (thread leitora, fila de 16) é da 23a. O app de
  hoje (29a) não sabe separar aviso de resposta, mas ninguém recebe o papel app antes da
  23a no teste nem antes da instalação no release.
- **Conexão do papel app sem prazo de leitura:** era 60 s (Fase 22); a conexão do app
  agora espera aberta, para receber os avisos.
- **`v3_15_sem_aprovador_no_binario` aceita `env::var` no `trava.rs`** (Fase 24: só
  `WARD_DEV_DBUS_SISTEMA` sob `modo-teste`, barramento do logind, fora da aprovação).
  A proibição de `cfg(feature)` em `aprovacao.rs`, `uso.rs`, `servidor.rs` e `nucleo.rs`
  continua.
- **Para o merge:** acrescentei `papel` aos dois construtores de `Resposta::Estado` fora
  do daemon (`crates/ward/src/testes.rs` da Fase 25 e `app/nucleo/src/daemon_falso.rs`),
  sem outra mudança neles.
- **Destrave sem app:** com o cofre travado e sem como abrir o app, o uso responde
  `travado` na hora, em vez de segurar 2 min esperando um `ward destravar` da CLI. Escolhi
  responder logo porque ninguém mostraria nada ao usuário. O planner confirma.
- **Uso combinado com a outra entrada bloqueada:** responde `negado`, sem perguntar (a
  própria bloqueada continua igual a inexistente). "Sempre" num combinado deixa livres
  as duas que estavam com aprovação; "1 hora" libera as duas.
- **Corridas aceitas:** "sempre" gravado e o pedido expirando no mesmo instante deixa a
  entrada livre com o pedido negado (o usuário aprovou com PIN). Pedidos simultâneos da
  mesma entrada abrem um pedido cada; o "1 hora" de um não resolve os outros.
- **`abrir_app` do agente** reabre o app cada vez que o usuário fecha (uma instância por
  vez limita, mas é incômodo). Sem limite de frequência.
- **`pidfd` tirado depois do `spawn`**, não entre o `fork` e o `execve` (o `std` não
  expõe `create_pidfd` estável). É equivalente: o filho não colhido não perde o pid.
- **`pidfd_send_signal(0)` num zumbi** ainda dá "vivo"; não importa (zumbi não conecta),
  e ao colher o lançador solta o pidfd.
- **D51 com limite pequeno:** abaixo de ~128 bytes o aviso de corte não cabe inteiro; o
  daemon usa 64 KiB (comando) e `MAX_TEXTO` (cada nome da cadeia).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/exibicao.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-plataforma/src/lancador.rs` | alterado |
| `crates/ward-plataforma/src/socket.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/wardd/src/aprovacao.rs` | criado |
| `crates/wardd/src/uso.rs` | criado |
| `crates/wardd/src/lancamento.rs` | criado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado (campos `pub(crate)`, motor, liberações, ops novas) |
| `crates/wardd/src/servidor.rs` | alterado (`papel_da_classe`, avisos na conexão do app, cliente) |
| `crates/wardd/tests/aprovacao.rs` | criado |
| `crates/wardd/tests/nucleo.rs` | alterado (campo `papel`) |
| `crates/wardd/tests/socket.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado (só o campo `papel` no `estado` falso) |
| `app/nucleo/src/daemon_falso.rs` | alterado (só o campo `papel` no `estado` falso) |
| `docs/mission/handoffs/phase-23.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase
