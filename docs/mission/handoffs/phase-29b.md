# Fase 29b — Pendências da 29a (D64)

## Implementado
- **`CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps` no ambiente de D57** (`crates/ward-plataforma/src/lancador.rs`).
  - Entra em `VARIAVEIS_FIXAS` pela constante nova `PASTA_MPS_INEXISTENTE`. Com isso, `ambiente_do_app` monta a variável e `conferir_ambiente_do_app` passa a exigi-la: se faltar ou vier com outro valor, a partida sai antes de conectar.
  - A documentação do módulo diz por quê.
  - O bloco 29a do `dev/conteiner-plataforma/dentro.sh` agora espera a lista de D57 com `CUDA_MPS_PIPE_DIRECTORY`.
- **RA7 / RUSTSEC-2024-0429 (`app/src-tauri/deny.toml`).** A exceção **fica**, porque o `cargo audit` em `app/src-tauri/` ainda acha a 0429 (glib 0.18.5, "unsound"). Pela regra de D64, só sai se os dois não a acharem.
  - O `advisory-not-detected` da 29a tinha outra causa: o `unsound` padrão do cargo-deny 0.20 (`"workspace"`) não olha dependências transitivas.
  - Pus `unsound = "all"` no `deny.toml` do app. Agora o cargo-deny também vê a 0429, a exceção deixa de ser cega e o aviso sumiu.
  - Conferido:
    - sem a linha da 0429, o `cargo deny check advisories` falha com `unsound = "all"`;
    - com a linha, passa;
    - `grep -rn 'Variant' app/` só acha o comentário do `deny.toml`.
  - A política só ficou mais estrita; nenhuma exceção nova. O `APP_AUDIT_IGNORAR` do Makefile continua com as duas.
- **Teste com o WebKitGTK real (V5.32): `make teste-webkit-real` (`dev/testes/webkit-real.sh`).**
  - **Por que um harness:** o `ward-app` de verdade não chega ao GTK sob `strace`, porque a partida recusa depurador e exige daemon e setgid instalados.
  - **Como é montado:** o script copia `app/src-tauri` para `.dev/webkit-real/` e troca só o bloco da partida do `main.rs` por um serviço em erro. Tauri, `tauri.conf.json`, capabilities e interface (`app/dist`) são os mesmos. Nenhum código que pule a partida entra no repositório. Se o `main.rs`, o `Cargo.toml` ou o `tauri.conf.json` mudarem de forma, o script falha de propósito.
  - **Execução:**
    - roda o app com `env -i` e exatamente as 12 variáveis de D57;
    - os valores fixos são lidos do `lancador.rs`, da constante da arquitetura da máquina;
    - pasta de trabalho `/`, como o lançador usa;
    - `strace -f`, HOME falso em `.dev/webkit-real/saida/casa`, 20 s na sessão Wayland do usuário.
  - **O que confere** (sai 1 se algo falhar):
    - o WebKitWebProcess subiu;
    - o ambiente tem só as variáveis de D57;
    - o GStreamer varreu de fato a pasta do sistema; sem isso, as iscas não provariam nada;
    - nada fez `open`/`connect` em `/tmp/nvidia-mps`;
    - o listener do atacante em `/tmp/nvidia-mps/control` (SOCK_SEQPACKET, o tipo do cliente MPS) não recebeu conexão;
    - o plugin-marca de `~/.local/share/gstreamer-1.0/plugins` não carregou.
  - **Iscas dos canais novos** (sai 2, separado de V5.32): `.so` em `~/.frei0r-1/lib`, `~/.ladspa`, `~/.lv2/<bundle>`, `~/.libvisual/actor` e um caminho em `/dev/shm/cuda_injection_path_shm`.
  - **Varredura:** imprime os `open`/`connect`/`bind`/`mkdir` em `/tmp`, `/var/tmp`, `/dev/shm`, `/run/user/<uid>` (fora do `wayland-<n>` e do `bus`), no HOME e em sockets abstratos.
  - **`SEM_MPS=1`:** roda sem a variável e mostra o ataque que D64 fecha.
  - **Limpeza:** se `/tmp/nvidia-mps` ou `/dev/shm/cuda_injection_path_shm` já existirem, o script não mexe e sai. Os que ele cria são removidos num `trap`.
  - A linha do alvo entrou no `CLAUDE.md`.

## Segurança
- **Invariantes tocadas:**
  - **S8 (D48, D57, D64).** O canal do MPS da NVIDIA saiu de `/tmp`. Medido no host (RTX 4050, driver 595.104.02, WebKitGTK do sistema):
    - **sem a variável:** o `gst-plugin-scanner` (filho do WebKitWebProcess, ao carregar o `libgstnvcodec` → `libcuda`) fez `connect("/tmp/nvidia-mps/control") = 0`, e o listener do "atacante" recebeu a conexão;
    - **com a variável:** `connect("/proc/ward-sem-mps/control") = -1 ENOENT`, nada em `/tmp/nvidia-mps` e o listener ficou com 0 conexões. O app seguiu normal até o `timeout`.
  - **S14 / RA7:** a exceção da 0429 agora é vista pelos dois auditores, e o `unsound` ficou mais estrito no app.
  - **S1:** o app de teste e o harness não recebem segredo. Nenhum cofre foi aberto, e o HOME falso fica em `.dev/` ou no scratchpad.
- **Testes de ataque escritos:**
  - `crates/ward-plataforma/src/lancador.rs`:
    - `ataque_ambiente_forcado_no_app_e_recusado`: com `CUDA_MPS_PIPE_DIRECTORY=/tmp/nvidia-mps`, `/run/user/1000/mps` ou vazio, recusa com `Diferente`; sem a variável, recusa com `Faltando("CUDA_MPS_PIPE_DIRECTORY")`;
    - `ambiente_montado_tem_so_as_variaveis_de_d57` confere os valores de `CUDA_MPS_PIPE_DIRECTORY` e `WEBKIT_A11Y_BUS_ADDRESS`.
  - `dev/testes/webkit-real.sh` (`make teste-webkit-real`), V5.32: listener em `/tmp/nvidia-mps/control`, plugin-marca do GStreamer e iscas dos canais novos. O controle negativo (`SEM_MPS=1`) falha como esperado.
  - `dev/conteiner-plataforma/dentro.sh` (V5.31): o ambiente do app lançado inclui a variável nova (70 ok).
- **Dependências novas:** nenhuma. O script usa `strace`, `gcc` e `python3` do sistema.

## Varredura de canais (D64), com o ambiente de D57 completo
Resultado de `make teste-webkit-real`. Os processos são o app, o WebKitNetworkProcess, o WebKitWebProcess e o `gst-plugin-scanner`.

| Caminho | Chamada | Processo | Observação |
| --- | --- | --- | --- |
| `/run/user/1000/wayland-1`, `/run/user/1000/bus` | connect | app, web | Permitidos por D57. |
| `/run/dbus/system_bus_socket` | connect | app, web | Do root; fora do escopo da varredura. |
| `/dev/null` (AT-SPI) | connect, ECONNREFUSED | web | É o efeito de `WEBKIT_A11Y_BUS_ADDRESS`. |
| `/proc/ward-sem-mps/control` | connect, ENOENT | scanner | É o efeito de D64. |
| `/run/user/1000/dconf/user` | `openat O_RDWR|O_CREAT` = ok | app | Arquivo de invalidação do dconf (mmap). Já está em RA8 (dconf). |
| `/run/user/1000/dconf/profile` | openat, ENOENT | app | O perfil do dconf viria daqui. Já está em RA8 (dconf). |
| `/tmp`, `/var/tmp` | `openat O_RDONLY` da pasta (13×) | scanner | Só sonda a pasta temporária; nenhum arquivo criado ou aberto dentro. |
| `/dev/shm/GFX_MEDIA_TRACE` | openat, ENOENT | scanner | **Novo.** Configuração de trace do driver de mídia Intel (iHD). |
| `/dev/shm/cuda_injection_path_shm` | `openat O_RDWR|O_NOFOLLOW` = **ok** quando existe | scanner (libcuda) | **Novo.** Mecanismo de injeção de biblioteca do CUDA (Nsight). A isca simples (um caminho de `.so`) não carregou; o formato esperado não foi investigado. |
| `@cuda-uvmfd-<netns>-<pid>` (socket abstrato) | bind | scanner (libcuda) | **Novo.** O namespace abstrato não tem permissão: qualquer processo da mesma netns conecta. |
| `~/.frei0r-1/lib/*.so` | opendir + **dlopen** | scanner | **Novo, carrega código.** |
| `~/.ladspa/*.so` | opendir + **dlopen** | scanner | **Novo, carrega código.** `LADSPA_PATH` não impede. |
| `~/.libvisual/actor/*.so` (também `input`, `morph`, `transform`) | **dlopen** | scanner | **Novo, carrega código.** |
| `~/.lv2/<bundle>/manifest.ttl` | lido (o `.so` não carregou na varredura) | scanner | Dado. |
| `~/.mfx_trace`, `~/.mfx_trace/etc/mfx_trace` | openat, ENOENT | scanner | **Novo.** Configuração de trace do Intel VPL/MediaSDK. |
| `~/.nv/nvidia-application-profiles-rc*`, `~/.nv/ComputeCache` | openat / mkdir | app, web, scanner | **Novo.** Perfis do driver NVIDIA (configuração) e cache JIT de kernels CUDA, no HOME. |
| `~/.cache/gstreamer-1.0/registry.x86_64.bin` | lido e regravado | web, scanner | Já visto na 29a: um registro plantado não carrega plugin de fora de `GST_PLUGIN_SYSTEM_PATH`. |
| `~/.cache/gtk-3.0/compose`, `~/.cache/fontconfig`, `~/.fontconfig`, `~/.config/dconf/user`, `~/.local/share/{mime,icons,glib-2.0/schemas}`, `~/.icons` | lidos ou criados | app, web | Dados do GTK. Já estão em RA8 (handoff 29a). |
| `~/.local/share/local.ward.app/` (WebKitCache, CacheStorage, storage, indexeddb, hsts, mediakeys, cookies) | criados | rede, web | Pasta de dados do WebKit para o app. Ver Problemas. |

## Ficou de fora
- **Corrigir os canais novos (frei0r, LADSPA, libvisual, CUDA, Intel, `~/.nv`).** O pedido manda devolver ao planner, sem decidir aqui. Há dados para a decisão em Problemas.
- **Rodar o teste num contêiner.** O canal do MPS só existe com o driver NVIDIA do host, e o teste precisa de uma sessão Wayland real. Por isso rodou no host, como na 29a, com HOME falso. Nada foi escrito fora de `.dev/`, do scratchpad, de `/tmp/nvidia-mps` e de `/dev/shm/cuda_injection_path_shm`. Os dois últimos foram criados só durante o teste e removidos em seguida (conferido com `ls`). O app de teste abriu uma janela por 20 s na sessão do usuário, várias vezes.
- **`HOME` falso em vez do HOME do passwd (D57).** Escolhi assim para não escrever no home real do usuário (`~/.local/share/local.ward.app`, caches). O comportamento do WebKit e do GStreamer depende do valor de `HOME`, não de ele ser o do passwd.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (1ª: `curl` 504 ao baixar o gitleaks; repetido) | 2, 0 |
| `cargo test -p ward-plataforma --all-features --lib lancador` | 0 |
| `cargo-audit audit --deny warnings --ignore RUSTSEC-2024-0370` em `app/src-tauri` (a 0429 ainda aparece) | 1 (esperado) |
| `cargo-deny check advisories` do app: com `unsound = "all"` e a 0429 ignorada / sem a linha da 0429 / raiz com `unsound = "all"` | 0 / 1 (esperado) / 0 |
| `make app-check` (com `PKG_CONFIG_PATH` e `PKG_CONFIG_SYSROOT_DIR` do scratchpad, R32; sem `advisory-not-detected`) | 0 |
| Harness no scratchpad: `rodar.sh` sem MPS (listener SOCK_STREAM e depois SEQPACKET), com MPS, com iscas, `FREI0R_PATH`/`LADSPA_PATH`/`LV2_PATH` e `GST_PLUGIN_LOADING_WHITELIST` | 124 (`timeout`; esperado) |
| `make teste-webkit-real` (1ª: o script pegou a constante do aarch64 e o GStreamer não varreu; corrigido, e agora o script exige a varredura) | 0 (inválido) |
| `make teste-webkit-real` | 2 (V5.32 ok, canais pendentes) |
| `make teste-webkit-real SEM_MPS=1` (controle negativo) | 2 do make (script 1: V5.32 falhou, esperado) |
| `make teste-docker-plataforma` (70 ok) | 0 |
| `make check` (1ª: sem o gitleaks, pelo 504 acima; fmt, clippy, testes, audit e deny já limpos) | 2 |
| `make check` | 0 |

## Problemas descobertos
- **Canal novo, carrega código: pastas de plugin do HOME varridas pelo `gst-plugin-scanner`.** Volta ao planner (D64).
  - **O que acontece:** com o ambiente de D57 completo, o `gst-plugin-scanner` (filho do WebKitWebProcess, que o GStreamer abre quando o registro falta ou está velho) carrega os plugins do sistema `libgstfrei0r`, `libgstladspa` e `libgstlibvisual`. Esses plugins abrem com `dlopen` as `.so` de `~/.frei0r-1/lib`, `~/.ladspa` e `~/.libvisual/actor`. As iscas carregaram nas três.
  - **Como um pacote malicioso dispara:** basta apagar `~/.cache/gstreamer-1.0/registry.x86_64.bin` para forçar a varredura na próxima abertura do app.
  - **O que roda e onde:** o código roda no `gst-plugin-scanner`, um processo do mesmo uid, filho do processo web.
  - **Até onde chega:** o Yama não deixa o filho fazer `ptrace` no pai. Mas o scanner devolve o registro ao processo web por um pipe, e o registro traz caminhos de plugin que o processo web carrega depois. Não testei se dá para fazer o processo web carregar uma `.so` por aí; é o próximo passo de quem decidir.
  - **Experimentos para o planner** (só no scratchpad, nada entrou em D57):
    - `FREI0R_PATH=/proc/ward-sem-plugin` fecha o frei0r;
    - `LADSPA_PATH` **não** fecha o `~/.ladspa`, e a libvisual não tem variável conhecida;
    - `GST_PLUGIN_LOADING_WHITELIST=ward-nenhum`: nenhuma isca carrega e o app abre, mas o WebKit avisa `GStreamer element appsink not found` (o app não usa mídia, mas não confirmei que a janela desenha igual).
  - **Outras saídas possíveis:**
    - `GST_REGISTRY_UPDATE=no` com um registro do root;
    - um `GST_PLUGIN_SYSTEM_PATH` com só os plugins de que o WebKit precisa (pasta do root, Fase 51);
    - registrar em RA8.
- **Canal novo: `/dev/shm/cuda_injection_path_shm`.** O libcuda, no scanner, abre o arquivo `O_RDWR` quando ele existe. O `/dev/shm` é gravável por qualquer uid. É o mecanismo de injeção do Nsight. A isca simples não carregou, mas o formato não foi investigado. O mesmo processo faz `bind` de um socket abstrato `@cuda-uvmfd-<netns>-<pid>`. Os dois vêm do `libgstnvcodec`; sem o scanner carregar o nvcodec, não aparecem. Volta ao planner.
- **Configuração de driver no HOME e no `/dev/shm`:** `~/.nv/nvidia-application-profiles-rc*` (perfis do driver NVIDIA), `~/.nv/ComputeCache` (cache JIT do CUDA, código de GPU), `~/.mfx_trace` e `/dev/shm/GFX_MEDIA_TRACE` (trace do Intel). São dados do mesmo uid lidos por processos do app, da família de RA8. Volta ao planner.
- **Pasta de dados do WebKit no HOME:** `~/.local/share/local.ward.app/` (WebKitCache, CacheStorage, storage, indexeddb, cookies, hsts). A interface não grava segredo em armazenamento (regra da UI), mas o cache HTTP do WebKit fica ali e é legível pelo uid. Vale a auditoria do M5 conferir que nenhum valor revelado vai para esse cache. Talvez o planner queira um `WebContext` efêmero.
- **Raiz com `unsound = "all"`:** o `deny.toml` da raiz passa limpo com `unsound = "all"` (conferido). Não mudei porque o arquivo não é desta fase. Sugiro alinhar.
- **cargo-deny `--config`** é opção global (`cargo-deny --config X check`), não do `check`. É só uma nota para quem repetir a conferência.
- **O app de teste abre uma janela na sessão do usuário** durante `make teste-webkit-real`. Não há compositor aninhado no host (sem weston/cage). Fica fora do `make check`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/lancador.rs` | alterado (`CUDA_MPS_PIPE_DIRECTORY`, `PASTA_MPS_INEXISTENTE`, testes) |
| `app/src-tauri/deny.toml` | alterado (`unsound = "all"`, motivo de manter a 0429) |
| `dev/testes/webkit-real.sh` | criado |
| `dev/conteiner-plataforma/dentro.sh` | alterado (lista de D57 com a variável nova) |
| `Makefile` | alterado (alvo `teste-webkit-real`) |
| `CLAUDE.md` | alterado (linha do `teste-webkit-real`) |
| `docs/mission/handoffs/phase-29b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks), mais `make app-check` e `make teste-docker-plataforma`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto; HOME falso; sem sudo; nada em `/etc` ou `/run` do host além da leitura de `/run/user/1000` e do uso normal do app na sessão)
- [x] Teste de ataque para cada (SEG) tocado (V5.31 pelo conferidor e pelo contêiner, V5.32 pelo `teste-webkit-real`, V5.30 pela conferência do deny/audit)
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase (canais novos só registrados, sem decisão)
