# Fase 39c — Ponta a ponta do app, canais do V5.32, senha mestra no daemon e pendências do M7

## Implementado
- **Ponta a ponta do app (V5.10, V5.16, V5.24)**: `make teste-app-e2e PW_PYTHONPATH=…`
  (`dev/testes/app-e2e.sh` + `dev/testes/app_e2e.py`). Resultado: 28 de 28 checagens ok,
  varredura de logs limpa.
  - Sobe um `wardd` de teste só deste teste, com socket em `WARD_DEV_SOCKET_DIR` (padrão
    `/run/user/<uid>/ward-teste-e2e`). Recusa a pasta comum `ward-teste` e recusa subir
    se o daemon de teste do repositório já estiver no ar.
  - `WARD_DEV_APP` = a ponte (D63). A URL vem do `dev/ponte.sh` e chega ao Python pela
    entrada padrão, nunca em argv.
  - Guarda o cofre e o estado de teste antes e os devolve no `trap` (o "Permitir sempre"
    muda o nível).
  - No fim, para o daemon e roda `varredura-logs.py` em `.dev/logs`.
  - **V5.16**: cada pedido vem do `ward run acme-pg-aprovacao -- touch .dev/e2e-marca`,
    pela CLI de teste de verdade.
    - A janela mostra entrada, ambiente e comando.
    - Negar e "Fechar e negar": a CLI sai ≠ 0 e a marca não existe.
    - "Permitir uma vez" com PIN: executa, e o pedido seguinte abre a janela de novo.
    - "Permitir por 1 hora": o pedido seguinte roda sem janela. Depois de `ward travar`
      e destravar, a janela volta.
    - "Permitir sempre": executa, a entrada aparece "Livre" no app e o pedido seguinte
      roda sem janela.
  - **V5.24**: o "Conectar" em `acme-ssh-salto` abre o "terminal" do repositório com
    `ward ssh --token-fd` real contra o `ssh-salto` (2224) e conecta sem janela nem PIN.
    Os ataques, feitos pelo próprio terminal, que tem o token:
    - Varredura de `/proc/*/cmdline` e `environ`: 0 achados em ~1000 arquivos lidos.
    - Reuso do token: vale o nível (a janela abre), o QA nega e não conecta.
    - Token usado em `acme-ssh-interno-aprov`, que salta pelo `acme-ssh-salto`: vale o
      nível e é negado.
    - Token usado depois de 31 s: vale o nível e é negado.
  - **V5.10**: no Novo (login) vazio, o "Salvar" fica desabilitado. Com o nome, habilita;
    ao salvar, aparece "Campo obrigatório" no campo que falta.
  - Evidência em `docs/mission/evidence/fase-39c/`: `qa.json` e 7 capturas
    (`aprovacao-ward-run-real.png`, `conectar-*.png`, `formulario-novo-*.png`,
    `sempre-entrada-livre.png`).
- **Ponte com terminal de verdade (só `modo-teste`)**:
  - `TerminalDoRepositorio` em `ward-ponte-teste/src/simulados.rs` usa o mesmo
    `TerminalDoSistema::comando` do app, com `dev/testes/terminal-teste.py` no lugar do
    `x-terminal-emulator`. Só aceita arquivos regulares e executáveis dentro do
    repositório tirado de um `WARD_DEV_DIR` limpo; senão a ponte cai no
    `TerminalDeTeste`, que só registra.
  - `partida::repositorio`.
  - `impl Terminal for Box<T>` em `app/nucleo/src/terminal.rs`.
  - `DaemonSocket::padrao()` (só `modo-teste`) e `dev/ponte.sh` passam a seguir
    `WARD_DEV_SOCKET_DIR`.
  - `terminal-teste.py` responde ao `ESC[6n` do ash e grava o fim da saída do pty em
    `.dev/logs/terminal-teste.saida` (0600; a varredura de logs confere esse arquivo).
- **Canais pendentes do V5.32 (D67 no PRD)**: duas variáveis entram nas fixas de D57
  (`lancador.rs`).
  - `GST_PLUGIN_LOADING_WHITELIST` lista só `libgstcoreelements.so` (gstreamer) e
    `libgstapp.so` (gst-plugins-base), com prefixo de arquivo por arquitetura. Pelo
    pacote só, o libvisual ainda carregava, porque é do gst-plugins-base. Fecha frei0r,
    LADSPA, libvisual e LV2. Fecha também o nvcodec (libcuda →
    `cuda_injection_path_shm`, `~/.nv/ComputeCache`) e os plugins Intel
    (`~/.mfx_trace`, `/dev/shm/GFX_MEDIA_TRACE`).
  - `__GL_APPLICATION_PROFILE=0` fecha o `~/.nv/nvidia-application-profiles-rc`, mesmo
    com um `…-globals-rc` plantado em `enabled: true`. Sobra a leitura do arquivo
    globals, que vai para o RA8.
  - Medido no host com `make teste-canais-gst` (novo, `dev/testes/canais-gst.sh`):
    gst-inspect e EGL sob strace, iscas em `.dev/canais-gst/`. O controle sem as
    variáveis carrega as iscas; com elas, "canais da 29b fechados".
  - `teste-webkit-real` agora reprova (sai 1) se uma isca de plugin ou CUDA carregar, ou
    se algum processo abrir `~/.nv/…-rc`, `~/.mfx_trace`, `GFX_MEDIA_TRACE`,
    `cuda_injection_path_shm` ou `~/.nv/ComputeCache`. Avisa se o WebKit disser que falta
    elemento do GStreamer. `SEM_39C=1` roda o controle.
  - `dev/conteiner-plataforma/dentro.sh`: a lista esperada de variáveis inclui as duas.
- **`unsound = "all"`** no `deny.toml` da raiz.
- **RA9** no PRD: o risco do menu da bandeja no barramento da sessão está registrado,
  com a proposta de aceitar. A decisão vai para o usuário no M5.
- **Senha mestra no daemon (mínimo de 12 e NFC)**:
  - `ward-core`: `MIN_CARACTERES_SENHA_MESTRA = 12`, `SenhaMestra::digitada` (NFC com
    caminho rápido ASCII, saída em `Zeroizing` com capacidade reservada antes, sem
    realocar) e `SenhaMestra::serve_de_nova`.
  - `wardd`: o `senha_de` normaliza em destravar, criar, trocar e restaurar.
    - `criar_cofre` e `trocar_senha` recusam senha nova curta com erro de validação no
      campo (`senha`/`nova`), antes de qualquer escrita, sem gastar geração.
  - App: o `primeiro_uso` usa a mesma regra do ward-core.
- **Roteiro do usuário**: a seção 11a ganhou o `teste-webkit-real` com WebKit.

## Segurança
- **Invariantes tocadas**:
  - **S11 (D57, D64, D67; o V5.32 é do app)**: o ambiente do app fecha os canais de
    código de HOME do GStreamer e do driver NVIDIA. A lista é fixa no código e conferida
    pelo app; um valor forçado é recusado.
  - **S3 e D3 (token do Conectar)**: provado de ponta a ponta que o token não sai em
    argv nem no ambiente e que reuso, prazo vencido e outra entrada (até o salto) caem no
    nível.
  - **S10 e D2 (aprovação)**: negar e fechar não executam; "uma vez" não vale para o
    próximo; "1 hora" morre ao travar.
  - **S6 e S8 (senha mestra)**: o daemon é a segunda camada do mínimo de 12; a NFC evita
    que a mesma senha digitada de formas diferentes abra ou não o cofre. A normalização
    vem de biblioteca, e os buffers normalizados zeram ao sair.
  - **S14 (dependências)**: `unsound = "all"`; uma dependência nova, com motivo abaixo.
  - **S13 (logs)**: a varredura de `.dev/logs` no fim do e2e saiu limpa, incluindo a
    saída do pty do terminal de teste.
- **Testes de ataque escritos**:
  - `crates/ward-core/src/formato/testes.rs`:
    - `senha_digitada_composta_ou_decomposta_abre_o_mesmo_cofre` (a decomposta crua não
      abre);
    - `ataque_senha_nova_curta_disfarcada_e_recusada`;
    - `senha_digitada_ascii_e_bytes_invalidos_ficam_iguais`.
  - `crates/wardd/tests/configuracoes.rs`:
    - `ataque_senha_mestra_nova_curta_recusada_pelo_daemon`: curta e 11 acentos
      decompostos (22+ bytes); nada é gravado e a geração não muda;
    - `senha_mestra_composta_abre_decomposta_pelo_daemon`.
  - `crates/ward-plataforma/src/lancador.rs`:
    - `ataque_lista_do_gstreamer_so_com_plugins_do_sistema`;
    - ambiente forçado com valores ruins das duas variáveis, e as duas faltando.
  - `crates/ward-ponte-teste/src/simulados.rs` ›
    `ataque_terminal_do_repositorio_so_com_arquivos_do_repositorio`: link do atacante,
    `WARD_DEV_DIR` torto ou relativo, sem execução.
  - `dev/testes/app_e2e.py`: os 4 ataques ao token, mais negar e fechar sem execução.
  - `dev/testes/canais-gst.sh`: as iscas com controle.
- **Dependências novas**: `unicode-normalization` `=0.1.25` com
  `default-features = false`, que traz só `tinyvec` (e o `tinyvec_macros` dele).
  - Motivo: a NFC precisa das tabelas Unicode; escrever à mão seria pior.
  - É a crate de normalização do ecossistema (usada pelo `url`/`idna`), sem `unsafe` no
    caminho usado.
  - `cargo audit` e `cargo deny` limpos.

## Ficou de fora
- **Verificação no WebKitGTK real** (`make teste-webkit-real`, `make app-check`): este
  host não tem o `libwebkit2gtk-4.1-dev`. Está no roteiro do usuário, seção 11a. O
  `canais-gst` mede o mesmo GStreamer e o mesmo EGL fora do WebKit; falta confirmar
  que nenhum elemento que o WebKit usa ficou de fora da lista.
- **`make teste-docker-plataforma`**: o `dentro.sh` mudou (lista de variáveis) e o teste
  não foi rodado nesta fase, para não disputar Docker e disco com as fases paralelas.
  Rodar no fechamento do milestone.
- **Linhas no `CLAUDE.md`**: não editei o arquivo, porque a instrução veio de agente e
  não do usuário (mesma decisão da Fase 39). Linhas propostas para a tabela de comandos:
  - `teste-logs-app`: "Varredura de logs (V5.27): autoteste e `.dev/logs` (ou
    `LOGS=…`) contra `.dev/variacoes.txt` | `make teste-logs-app` | pronto".
  - `teste-canais-gst`: "D67/V5.32 sem WebKit: gst-inspect e EGL sob strace com as iscas
    de 29b em `.dev/canais-gst/`, controle sem as variáveis; precisa de `strace` e
    `gst-inspect-1.0` | `make teste-canais-gst` | pronto".
  - `teste-app-e2e`: "V5.10, V5.16, V5.24 de ponta a ponta pela ponte (`wardd` de teste
    próprio em `WARD_DEV_SOCKET_DIR`, padrão `/run/user/<uid>/ward-teste-e2e`), com o
    Chrome headless e a CLI de teste real; precisa de `make dev-cofre`,
    `dev-variacoes`, `app-front`, dos serviços de pé e de `PW_PYTHONPATH`; ~8 min |
    `make teste-app-e2e PW_PYTHONPATH=…` | pronto".

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-cofre`, `make dev-variacoes`, `make app-front` | 0 |
| `make teste-canais-gst` | 0 |
| `make teste-app-e2e PW_PYTHONPATH=<scratchpad>/pwlib` (última rodada) | 0 (28/28) |
| `cargo test -p ward-core` (testes de senha), `-p wardd --test configuracoes` | 0 |
| `cargo test -p ward-plataforma --all-features --lib` | 0 (87) |
| `cargo test -p ward-ponte-teste --features modo-teste --lib` | 0 |
| `make dev-cofre-conferir` (depois do e2e: cofre de volta ao original) | 0 |
| `make ferramentas` (gitleaks faltava no worktree) | 0 |
| `make fmt` | 0 |
| `make check` | 0 ("tudo limpo") |

## Problemas descobertos
- **Para o usuário (pendente)**: `make teste-webkit-real` com o WebKit (roteiro 11a),
  `make app-check` e `make teste-docker-plataforma` (o `dentro.sh` mudou).
- **RA9 (menu da bandeja no barramento da sessão)**: decisão do usuário no M5.
- **RA8 ampliado**: o driver NVIDIA ainda lê
  `~/.nv/nvidia-application-profile-globals-rc` com `__GL_APPLICATION_PROFILE=0`. Não
  carrega código, mas é leitura de arquivo do HOME. Registrado no RA8.
- **A ponte é de uso único por página**: tira o token da URL depois de ler, então
  recarregar a página dá 403 e "resposta inválida da ponte de teste". O QA reabre a URL
  original. Não é falha de segurança; vale saber para o validador.
- **Conflitos prováveis com a 39b**:
  - `app/src/telas/Formulario.svelte`: `novoVazio`, `camposIniciais` e o `desabilitado`
    do Salvar;
  - `crates/wardd/src/nucleo.rs`: `senha_de`, `senha_nova`, `criar_cofre`,
    `trocar_senha`;
  - os `Cargo.lock` (dependência nova).
- **`wardd` de outro agente rodando**:
  `/tmp/claude-1000/…/scratchpad/target/debug/wardd` (pid 759979, não-dumpable) estava no
  ar durante a fase. Não é meu e não mexi nele. O daemon do checkout principal
  (`ward-teste`) também não foi usado.
- A pasta `/run/user/1000/ward-teste-e2e/` fica com um `wardd.sock` morto depois do
  teste; o `daemon.sh` a reaproveita na rodada seguinte.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `deny.toml` | alterado |
| `Cargo.lock`, `app/src-tauri/Cargo.lock` | alterado |
| `crates/ward-core/Cargo.toml` | alterado |
| `crates/ward-core/src/limites.rs` | alterado |
| `crates/ward-core/src/formato.rs` | alterado |
| `crates/ward-core/src/formato/testes.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/configuracoes.rs` | alterado |
| `crates/ward-plataforma/src/lancador.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/src/partida.rs` | alterado |
| `crates/ward-ponte-teste/src/simulados.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/primeiro_uso.rs` | alterado |
| `app/nucleo/src/terminal.rs` | alterado |
| `app/src/telas/formulario.js` | alterado |
| `app/src/telas/Formulario.svelte` | alterado |
| `Makefile` (`teste-app-e2e`, `teste-canais-gst`, `SEM_39C` no `teste-webkit-real`) | alterado |
| `dev/ponte.sh` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado |
| `dev/testes/webkit-real.sh` | alterado |
| `dev/testes/canais-gst.sh` | criado |
| `dev/testes/app-e2e.sh` | criado |
| `dev/testes/app_e2e.py` | criado |
| `dev/testes/terminal-teste.py` | criado |
| `docs/mission/prd.md` (D57, D67, RA8, RA9) | alterado |
| `docs/mission/roteiro-usuario.md` (seção 11a) | alterado |
| `docs/mission/evidence/fase-39c/` (`qa.json` e 7 PNG) | criado |
| `docs/mission/handoffs/phase-39c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido no Chrome headless pela ponte (o Salvar desabilitado usa o estado `desabilitado` do `Botao` primário, já desenhado; nenhum visual novo)
- [x] Ficou dentro do escopo da fase
