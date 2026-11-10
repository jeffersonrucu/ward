# Fase 21 — `ward-plataforma`: socket, `SO_PEERCRED`, identificação e endurecimento

## Implementado
- `socket` (S5, V3.1, V3.2):
  - `preparar_diretorio(caminho)` cria o diretório com 0700. Se ele já existe com outra
    permissão (0755, 0750, 0701…), é de outro dono, é link simbólico ou é arquivo,
    recusa sem consertar nada. Toda pasta acima tem de ser do root ou do usuário, sem
    escrita para grupo e outros e sem link simbólico (`/tmp` recusa). O diretório
    fica aberto (`O_DIRECTORY|O_NOFOLLOW`) e é conferido de novo por dev/inode antes
    e depois do `bind`. Se for trocado por outro com o mesmo nome, dá `Trocado`.
  - `DiretorioSocket::escutar(nome)` cria o socket com 0600, conferido por `lstat`
    depois do `bind`. Um socket velho do usuário, sem ninguém escutando, é trocado.
    Com outro daemon escutando dá `JaEmUso`. Arquivo, link ou nome com `/`/`..` no
    lugar do socket são recusados, e nada que não seja socket nosso é apagado. No
    `Drop`, a `Escuta` apaga o socket se ele ainda for o mesmo inode.
  - `Escuta::aceitar()` e `conferir_conexao(&stream)`, que serve para qualquer laço,
    inclusive assíncrono, pelo `AsFd`. Com uid do par diferente do euid, a conexão
    fecha sem resposta e volta `ErroAceitar::UidRecusado { uid, pid }` para o log.
    Sem pid (outro namespace) ou sem pidfd, também fecha (`SemIdentidade`).
  - `diretorio_runtime()` = `/run/user/<euid>`, pelo uid e não por `XDG_RUNTIME_DIR`.
    Constantes `NOME_DIRETORIO` (`ward`), `NOME_DIRETORIO_TESTE` (`ward-teste`, D7) e
    `NOME_SOCKET` (`wardd.sock`).
- `processo` (D5, S1, S5, V3.5):
  - `ProcessoPar::do_socket` junta o `SO_PEERCRED` e o pidfd do `SO_PEERPIDFD`, e
    confere que o pid do pidfd (`/proc/self/fdinfo`) é o mesmo do `SO_PEERCRED`.
  - Contra reaproveitamento de pid, o daemon abre `/proc/<pid>` e só depois confere
    pelo pidfd que o processo original ainda vive. Tudo é lido por
    `openat`/`readlinkat` relativo a esse diretório, com limite de tamanho (status
    64 KiB, environ 1 MiB, maps 8 MiB).
  - `identificar(binario)` exige, nesta ordem:
    - `TracerPid` 0;
    - `readlink exe` igual ao caminho instalado;
    - binário e pastas acima do root, sem escrita de grupo/outros, sem link;
    - o arquivo em execução (`exe` aberto com `O_PATH`) é o mesmo dev/inode do
      instalado;
    - nenhuma variável com prefixo `LD_` no `environ`;
    - toda biblioteca mapeada como executável no `maps` é do root e é o mesmo inode
      (pega `memfd`, biblioteca apagada ou de `/tmp`, e `LD_PRELOAD` que apagou a
      própria variável);
    - `TracerPid` e pidfd vivo de novo no fim.
  - `identificar_host_extensao(host, chrome)` faz o mesmo para o host e confere o
    pai. O pidfd do pai é aberto depois de ler o `PPid`, e o `PPid` é relido com o
    filho vivo. Do pai conferem-se exe, dono root, depurador e `LD_*`, sem o `maps`,
    porque o Chrome mapeia o Widevine da pasta do usuário.
  - `classificar(&Instalacao)` devolve `ClassePar::{App, HostExtensao, Comum { app,
    extensao }}`. Qualquer falha cai em `Comum`.
  - `reconferir_sem_depurador()` serve para antes de cada operação sensível. Usa
    `status`, que continua legível mesmo com o cliente não-dumpable.
  - `Recusa` é `Copy` e vai para o log sem caminho nem valor. Uma variável `LD_*` de
    nome inventado aparece só como `LD_*`.
  - `cadeia_de_processos(pid)` / `ProcessoPar::cadeia()` montam a cadeia do mais
    antigo para o mais novo, com até 32 elementos. O nome vem do `comm`, sem
    caractere de controle nem de direção de texto e com até 64 caracteres. É só para
    exibir.
  - `conferir_servidor(&stream, &wardd)` é a conferência do lado do cliente (S5),
    mas não funciona contra o daemon endurecido (ver Problemas).
- `endurecimento` (S8, V3.6, V3.7):
  - `endurecer_processo()` aplica `PR_SET_DUMPABLE=0` e `RLIMIT_CORE` 0/0, e confere
    os dois depois de aplicar.
  - `conferir_ambiente_proprio()` recusa qualquer `LD_*`, só pelo nome, e
    `TracerPid` ≠ 0.
  - `iniciar_endurecido()` roda os dois na ordem certa (dumpable primeiro). A
    mensagem do `ErroInicio` traz o nome da variável e nunca o valor.
- `memoria` (D14):
  - `BufferTravado::novo(len)` cria uma região `mmap` só sua, em páginas inteiras,
    com `mlock`, `MADV_DONTDUMP` e `MADV_WIPEONFORK`. No `Drop`, zera a região
    inteira com `zeroize` e então faz `munlock`/`munmap`. O `Debug` mostra só `len`.
    O limite é 1 MiB, e um erro de `mlock`/`madvise` recusa em vez de seguir sem
    trava.
  - `travar_regiao(&[u8])` trava memória que já existe. Não há função de destravar,
    porque o `mlock` é por página e destravar soltaria segredos vizinhos.
- `unsafe`: só em `src/sys.rs` (wrappers da `libc`) e `src/memoria.rs` (fatias da
  região mapeada, `Send`/`Sync`), cada item com `#[allow(unsafe_code)]` e
  `// SAFETY:`. Nos testes de integração do crate há `unsafe` só para `ptrace` e
  `signal`, também com `// SAFETY:`.

## Segurança
- Invariantes tocadas:
  - S5: diretório 0700 e socket 0600, os dois do usuário, e uid do par por
    `SO_PEERCRED`. Uid diferente fecha sem responder.
  - S1/D5: os papéis app e extensão só saem da identificação completa, e qualquer
    falha cai em agente.
  - S8: não-dumpable, core 0, recusa `LD_*` e depurador, buffer travado zerado ao
    sair.
  - S13: nenhuma mensagem de erro leva valor de variável nem conteúdo do `/proc`.
- Testes de ataque escritos:
  - `tests/socket.rs`:
    - `ataque_diretorio_existente_0755_nao_sobe_nem_conserta`;
    - `ataque_diretorio_de_outro_dono_nao_sobe`;
    - `ataque_link_simbolico_no_lugar_do_diretorio`;
    - `ataque_arquivo_no_lugar_do_diretorio`;
    - `ataque_pasta_acima_gravavel_por_grupo`;
    - `ataque_arquivo_ou_link_no_lugar_do_socket`;
    - `segundo_daemon_no_mesmo_socket_recusa_e_socket_velho_e_trocado`;
    - `diretorio_trocado_depois_de_preparar_recusa_escutar`;
    - caminho feliz: `diretorio_novo_fica_0700_e_socket_0600_do_usuario` (V3.1).
  - `src/socket.rs` › `uid_diferente_recusa`: a decisão por uid (V3.2, lógica; ver
    "Ficou de fora") e `pai_gravavel_por_outros_recusa` (`/tmp`).
  - `tests/identificacao.rs`:
    - `ataque_binario_copiado_para_fora_da_instalacao` (V3.5);
    - `ataque_variaveis_ld_no_ambiente` (`LD_PRELOAD`, `LD_AUDIT`,
      `LD_LIBRARY_PATH`; V3.5);
    - `ataque_cliente_sob_strace` (TracerPid; V3.5);
    - `ataque_biblioteca_de_usuario_sem_variavel`;
    - `ataque_processo_que_sai_antes_da_conferencia` (pidfd);
    - `host_da_extensao_exige_o_pai_permitido`;
    - `host_da_extensao_sem_pai_permitido_quando_o_pai_e_o_teste`;
    - `cliente_confere_o_servidor_antes_da_senha` (servidor falso);
    - `servidor_nao_dumpable_nao_pode_ser_conferido_pelo_proc`, que registra o
      conflito descrito em Problemas.
  - `tests/endurecimento.rs`:
    - `ataque_ler_memoria_ambiente_e_ptrace_do_processo_endurecido` (V3.6). Cobre
      `/proc/<pid>/mem`, `environ` e `maps` com EACCES, `PTRACE_SEIZE` feito pelo
      próprio pai (que o Yama deixaria) com EPERM, `strace -p` falhando e `limits`
      com core 0/0, além de um controle sem endurecer em que o `environ` é legível;
    - `ataque_sigsegv_nao_deixa_core` (V3.6): morre por SIGSEGV, `core_dumped()`
      falso e nenhum arquivo na pasta;
    - `ataque_subir_com_ld_recusa` (V3.7): sai ≠ 0, com o nome na mensagem e sem o
      valor;
    - `ataque_subir_sob_strace_recusa` (V3.7);
    - caminho feliz: `sobe_limpo_sem_ld_e_sem_depurador`.
  - `src/processo.rs`: `ambiente_com_qualquer_ld_recusa_sem_valor`,
    `mapas_recusam_biblioteca_fora_do_root`, `mapas_recusam_inode_diferente_do_caminho`,
    `instalacao_recusa_arquivo_de_usuario_e_link`, `caminho_permitido_so_absoluto_e_canonico`,
    `nome_da_cadeia_higienizado`, `recusa_nao_vaza_detalhe`.
  - `src/memoria.rs`: `debug_nao_mostra_conteudo`, `zerar_apaga_a_regiao_inteira`,
    `buffer_travado_le_escreve_e_trava` (VmLck) e `tamanho_fora_do_limite_recusa`.
- Dependências novas: nenhuma crate entra na árvore (`Cargo.lock` só ganha a lista de
  dependências do `ward-plataforma`).
  - `libc` 0.2.191, já presente pelo `ward-core`, para `getsockopt`
    (`SO_PEERCRED`/`SO_PEERPIDFD`), `pidfd_open`/`pidfd_send_signal`,
    `openat`/`readlinkat`, `prctl`, `setrlimit`/`getrlimit` e
    `mmap`/`mlock`/`madvise`/`munmap`. Nada disso existe estável na biblioteca
    padrão (`UnixStream::peer_cred` continua instável).
  - `zeroize` 1.9.1, já presente, para zerar o `BufferTravado`.
  - O `SO_PEERPIDFD` (77, do asm-generic) é definido no crate porque a `libc` 0.2.191
    não o exporta para glibc. Fica restrito a x86_64/aarch64.

## Ficou de fora
- A ligação com o `wardd` é da Fase 22. O `main` do `wardd` deve chamar
  `iniciar_endurecido()` antes de qualquer outra coisa e sair ≠ 0 com a mensagem
  (V3.7). Depois vêm `preparar_diretorio(diretorio_runtime().join(NOME_DIRETORIO))`,
  `escutar(NOME_SOCKET)` e, a cada conexão, `conferir_conexao` (log da recusa) e
  `classificar` → papel.
- O mapeamento de `ClassePar` para o `Papel` do protocolo fica com as Fases 20 e 22:
  `App` vira app, `HostExtensao` vira extensão e `Comum` vira agente. Não criei tipo
  de protocolo. O `Instalacao` com os caminhos reais (`/usr/local/…`) é do M9; o
  daemon de teste passa os seus.
- V3.2 ponta a ponta (`docker run --user 4242`) depende do daemon. Aqui só a decisão
  por uid está testada (unidade), porque não há como criar um processo de outro uid
  sem root. Com o diretório 0700, o uid 4242 nem chega a conectar (EACCES), então a
  checagem do `SO_PEERCRED` é a segunda camada. Para exercitá-la de verdade, o
  validador precisa abrir o diretório no contêiner.
- V3.6 com `gdb -p` e `coredumpctl`: coberto pelo mesmo `ptrace` (EPERM) e pelo
  `core_dumped()` falso. O `gdb` não está no teste.
- O `mlock` da chave de dados do `ward-core` (`cripto::Chave`, um `Box` no heap) fica
  para a Fase 22, por `travar_regiao`, ou para uma mudança no `ward-core` que use um
  armazenamento travado. Ver Problemas.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `cargo test -p ward-plataforma` (21 unidade + 26 integração) | 0 |
| `cargo clippy -p ward-plataforma --all-targets --all-features -- -D warnings` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **S5 × S8: conflito no escopo. Decide o planner; não escolhi um lado.** D5 diz que
  "o mesmo teste vale para a CLI e o app conferirem o `wardd` antes de mandar a
  senha mestra". Só que o `wardd` endurecido (S8, `PR_SET_DUMPABLE=0`) tem `exe`,
  `environ` e `maps` ilegíveis para outro processo do mesmo uid (EACCES, conferido
  em `servidor_nao_dumpable_nao_pode_ser_conferido_pelo_proc`). Contra o daemon
  real, `conferir_servidor` dá `SemAcesso`, falhando fechada. Se a CLI exigir isso,
  nunca manda a senha; se ignorar, S5 fica sem a conferência. Opções:
  - rótulo de LSM por `SO_PEERSEC`, com um perfil AppArmor por binário instalado,
    que o kernel atribui no exec, que não depende do dumpable e que o usuário não
    forja (exige perfil instalado como root no M9);
  - aceitar só "par do mesmo uid, `/proc/<pid>` não-dumpable e `status` com
    `Name: wardd`", o que é fraco e forjável;
  - aceitar o risco no escopo.
- **Mesma tensão para app e host.** Se o app ou o `ward-nm` ficarem não-dumpable
  antes da identificação, o daemon não lê o `/proc` deles e eles caem em agente.
  Ligação com a Fase 20: o cliente conecta, espera o daemon responder com o papel e
  só então se endurece. Na janela entre a identificação e o endurecimento, um
  processo-pai malicioso (o Yama `ptrace_scope=1` deixa o pai depurar o filho) pode
  se prender, injetar código e se soltar antes do pedido sensível.
  `reconferir_sem_depurador` só pega o depurador enquanto ele está preso. O rótulo
  de LSM acima resolveria também este caso.
- **Chrome não-dumpable.** Se o processo do navegador do Chrome for não-dumpable, o
  `exe` dele fica ilegível e o host da extensão nunca é reconhecido. Também não
  conferi se o Chrome lança o host como filho direto ou por um intermediário. É
  preciso confirmar no M6 com o Chrome real.
- **`LD_*` fecha mais que o D5.** Recuso qualquer variável com prefixo `LD_`, e não
  só as três do D5, seguindo o "e similares" de S8. Quem tiver `LD_LIBRARY_PATH` no
  ambiente da sessão vai ver o app como agente e o daemon não vai subir. A falha é
  fechada, mas pode surpreender.
- **O `maps` vai além do D5.** Exijo que toda biblioteca executável do app e do host
  seja do root. É defesa a mais, contra `LD_PRELOAD` que apaga a variável. O
  WebKitGTK do app Tauri pode carregar algo de fora do root, como um módulo GTK do
  usuário, e então o app cai em agente. Confirmar no M5.
- **O SIGSEGV não mata o Rust.** O `std` do Rust instala um tratador de SIGSEGV que
  ignora o sinal mandado por `kill`, e o processo continua vivo. Para o `kill -SEGV`
  da V3.6, o `wardd` real não morre: não deixa core, mas também não cai. O teste
  devolve a ação padrão ao filho para conferir que não há core. A Fase 22 pode fazer
  o mesmo, se quiser que o daemon morra.
- **Sem conferência de que `/proc` é procfs** (`fstatfs`). Um processo do mesmo uid
  não remonta `/proc` no namespace do daemon, então deixei de fora.
- **Exige Linux 6.5 ou mais novo** por causa do `SO_PEERPIDFD`. Em kernel antigo,
  toda conexão é recusada (`SemIdentidade`). Este Pop!_OS está no 7.2.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/Cargo.toml` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | criado |
| `crates/ward-plataforma/src/memoria.rs` | criado |
| `crates/ward-plataforma/src/endurecimento.rs` | criado |
| `crates/ward-plataforma/src/processo.rs` | criado |
| `crates/ward-plataforma/src/socket.rs` | criado |
| `crates/ward-plataforma/tests/comum/mod.rs` | criado |
| `crates/ward-plataforma/tests/socket.rs` | criado |
| `crates/ward-plataforma/tests/identificacao.rs` | criado |
| `crates/ward-plataforma/tests/endurecimento.rs` | criado |
| `Cargo.lock` | alterado |
| `docs/mission/handoffs/phase-21.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados. Nenhum cofre foi aberto; os
  diretórios de teste ficam em `/run/user/<uid>/ward-plataforma-teste-*` e são
  apagados
- [x] Teste de ataque para cada (SEG) tocado (V3.1, V3.5, V3.6, V3.7; V3.2 só na
  lógica, ver "Ficou de fora")
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase
