# Fase 44 — `ward playwright`

## Implementado
- `ward playwright <entrada>` (CLI): confere o estado (travado → mensagem de sempre), pede
  `Pedido::Playwright` com prazo próprio de 330 s (`Cliente::pedir_com_prazo`) e imprime só
  o caminho devolvido (absoluto, sem caractere de controle). Uso errado: `uso: ward
  playwright <entrada>`.
- `wardd` (`crates/wardd/src/playwright.rs`, `crates/wardd/src/cdp.rs`):
  - tipo conferido antes da aprovação (`TipoSemEntrega`), depois `autorizar_uso` (níveis,
    aprovação, bloqueada = inexistente) e `registrar_uso`;
  - URL, usuário, senha (`Zeroizing`), código TOTP, máscara e o contador de travas
    copiados sob o lock;
  - Chrome instalado (`/opt/google/chrome/chrome`) em headless com
    `--remote-debugging-pipe`, sem porta, argv só de flags fixas mais
    `--user-data-dir=<rt>/pw-<16 aleatórios>/` (0700) e `about:blank`. Perfil
    temporário dentro de `/run/user/<uid>/ward/` com preferências que desligam o
    gerenciador de senhas e o autofill. Ambiente montado do zero (PATH, HOME e XDG_* no
    perfil, TMPDIR, LANG, DBUS desligado). Filho só com fds 0–2 em `/dev/null`, 3
    (comandos) e 4 (respostas), num grupo de processos próprio;
  - CDP pelo pipe: contexto off-the-record (`Target.createBrowserContext`), sessão
    `flatten`, `Fetch` em todo documento do quadro principal: outra origem recebe
    `failRequest` (barra o redirecionamento 8084 → 8082 e o envio do formulário para
    fora). Origem conferida pela árvore de quadros (URL e `securityOrigin`) antes de
    preencher e no fim (D9);
  - preenchimento num mundo isolado (`Page.createIsolatedWorld` mais
    `Runtime.callFunctionOn` com a senha como argumento, nunca no código), com getters do
    protótipo (sem DOM clobbering). Exige exatamente um campo de senha visível, `POST`,
    `action` e `formaction` da mesma origem e nenhum `target`. Envio por `requestSubmit`.
    Se depois do login ainda há campo de senha visível, o resultado é `LoginRecusado`;
  - storage state no formato do Playwright (cookies do contexto mais `localStorage` da
    origem). Se a máscara acha a senha (qualquer variação de D54) no estado, devolve
    `SessaoComSegredo` e nada é gravado;
  - Chrome morto (grupo inteiro) e perfil apagado **antes** de responder;
  - gravação sob o lock, só se o cofre continua aberto e o contador de travas não mudou:
    `<rt>/playwright/<entrada>.json`, temporário `create_new` 0600 com `O_NOFOLLOW` e
    depois `rename`, pasta 0700. Nome validado (`[a-z0-9._-]`, sem `.` inicial);
  - travar apaga `<rt>/playwright/*` (`Estado::travar`). A partida do daemon
    (`playwright::limpar` no `main`) apaga `playwright/*` e os perfis `pw-*` que um daemon
    morto deixou;
  - erros novos: `LoginRecusado`, `NavegadorFalhou`, `SessaoComSegredo`.
- `ward-core`: `origem_da_url` (D9: `esquema://host[:porta]` sobre a `url_valida`
  estrita, sem a porta padrão), com o teste `d9_origem_da_url`.
- `ward-plataforma`: `Comando::lancar_com_depuracao` → `(Grupo, PipeDepuracao)`.
  `Stdio::Depuracao` no `sys.rs` põe o pipe nos fds 3/4 do filho (fontes do `dup2`
  sempre ≥ 5, `close_range` a partir do primeiro fd livre). O `unsafe` continua só no
  `sys.rs`, com `// SAFETY:` e teste.
- `make teste-playwright`: os ataques com o Chrome de verdade (`cargo test --ignored`) e
  o ponta a ponta contra o WordPress e o phish (`dev/testes/playwright.sh` e `.py`, com
  `wardd` próprio em `WARD_DEV_SOCKET_DIR`). Linha nova na tabela do `CLAUDE.md`.

## Segurança
- Invariantes tocadas:
  - S1/S2/S3: a senha não aparece na resposta, no argv, no ambiente, no log, no storage
    state nem no perfil. Ela só entra no mundo isolado como argumento do CDP. Os buffers
    de leitura e escrita do CDP são `Zeroizing` pré-alocados que nunca realocam, e o
    `postData` dos pedidos pausados é pulado com `IgnoredAny`, sem cópia.
  - S13 e D6: o storage state fica em `/run/user/<uid>/ward/playwright/`, com 0600, e é
    apagado ao travar e na partida. O perfil é temporário e apagado antes da resposta.
  - D9: a origem é conferida antes de preencher, no envio e no fim; redirecionamento ou
    envio para fora é barrado no navegador.
  - V3.17: bloqueada = inexistente.
  - D60: ambiente do Chrome montado do zero.
- Testes de ataque:
  - `crates/wardd/tests/playwright.rs`, com o Chrome falso `tests/chrome_falso.py`
    falando CDP nos fds 3/4:
    - `v6_12_sessao_gravada_so_o_caminho_volta`: resposta só com o caminho, arquivo
      0600, pasta 0700, sem a senha. Confere o argv (pipe, sem porta, sem segredo), o
      ambiente e os fds 0–4 (0–2 em `/dev/null`). A senha só chega ao preenchimento
      (conferida por sha256), o perfil some e travar apaga a sessão.
    - `v6_15_redirecionamento_para_outra_origem_recusado`: o 8082 é barrado e a senha
      nunca vai ao Chrome.
    - `d9_origem_da_pagina_e_do_envio`: página que mente a origem, POST para outra
      origem barrado e formulário que envia para fora.
    - `s1_sessao_com_a_senha_nao_e_gravada`: cookie e `localStorage` com a senha em
      base64.
    - `navegador_que_cai_ou_manda_lixo`: Chrome que morre, e 5 MiB sem `\0`.
    - `d6_travar_durante_a_sessao_nada_gravado`.
    - `recusas_sem_abrir_o_navegador`: bloqueada, inexistente, tipo errado, com
      aprovação sem app e papel app.
    - `d6_partida_limpa_sessoes_e_perfis`.
    - `chrome_real_login_e_ataques` (`#[ignore]`, roda no `make teste-playwright`): o
      Chrome de verdade contra sites locais. Casos: `action` para fora, `formaction`,
      `action` mascarado por `<input name="action">`, redirecionamento 302, formulário
      `GET`, `target=_blank`, dois campos de senha, login recusado, senha no
      `localStorage` e iframe de outra origem. Em todos: o phishing sem `POST` nem
      senha, a senha fora de toda URL, nada gravado e o perfil apagado.
  - `crates/wardd/src/playwright.rs`:
    - `argv_so_fixo_com_pipe_e_sem_porta`
    - `estado_no_formato_do_playwright`
    - `ataque_nome_fora_da_pasta_recusado`
  - `crates/wardd/src/cdp.rs`:
    - `escritor_escapa_e_nao_realoca`
    - `pausa_le_so_url_tipo_e_quadro`
  - `crates/ward-plataforma/src/filho.rs` › `pipe_de_depuracao_so_no_filho_em_3_e_4`: o
    pipe não vaza para outro filho.
  - `crates/ward-core/src/entrada/testes.rs` › `d9_origem_da_url`: subdomínio, porta,
    esquema, userinfo, `0x7f000001`.
  - `dev/testes/playwright.py` (`make teste-playwright`), com o Chrome e os serviços
    reais:
    - V6.12: `acme-wp-prod`, e o Playwright do Python abre `/wp-admin/` com o storage
      state;
    - V6.13: storage state e perfil varridos com `.dev/variacoes.txt`; travar apaga;
    - V6.14: vigia em `/proc` durante a sessão. Nenhum socket TCP em LISTEN do Chrome
      (`/proc/net/tcp*` e `ss`), argv sem segredo e os inodes do pipe só no processo
      principal do Chrome;
    - V6.15: `delta-redir` recusado e `posts.log` do phish sem mudança;
    - V6.16 (parte do daemon): log do `wardd` com `RUST_LOG=trace` sem padrão de
      `.dev/variacoes.txt`;
    - também gama-bloqueada, inexistente e tipo errado.
- Dependências novas: `serde` (derive) e `serde_json` no `wardd`. Já estavam no
  `Cargo.lock` pelo `ward-core`, então o lockfile só ganha 2 linhas de dependência. Motivo:
  o CDP é JSON com estrutura variável e eventos grandes, e um parser feito à mão seria
  mais arriscado. Só uso com structs tipadas, então campos desconhecidos (o `postData`
  com a senha) são pulados sem cópia. `cargo audit` e `cargo deny` estão limpos.

## Ficou de fora
- O lado `ward login` de V6.15 é da Fase 43, e a parte do `ward-nm` e do console da
  extensão em V6.16 é das Fases 40/41.
- `make teste-home-real` não roda em worktree (o bind do repositório dentro do home
  coberto não casa). O Chrome também não está no `dev/testes/home-real.sh`; ele só roda
  com HOME e XDG no perfil temporário.
- TOTP: o código só é preenchido se o formulário tem campo
  `autocomplete="one-time-code"`. Login em duas etapas (TOTP numa segunda página) não é
  tratado e cai em `LoginRecusado`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-servicos` | 0 |
| `make dev-cofre` | 0 |
| `make dev-variacoes` | 0 |
| `cargo test -p wardd --all-features --test playwright` | 0 (8 testes) |
| `cargo test -p wardd --all-features --test playwright -- --ignored` | 0 |
| `make fmt` | 0 |
| `make check` | 0 |
| `make teste-playwright PW_PYTHONPATH=<site-packages com playwright 1.64.1>` | 0 |
| `make dev-daemon` + `make teste-cli` + `make dev-daemon-parar` (depois de o daemon do outro checkout sair) | 0 (94 ok, 0 falhas) |

## Problemas descobertos
- `make teste-cli`: durante a fase o socket fixo `/run/user/1000/ward-teste/wardd.sock`
  ficou ocupado pelo `wardd` de teste de outro checkout. Não derrubei nem travei esse
  daemon. Tentei isolar com bwrap (tmpfs em `ward-teste`), mas no namespace de usuário o
  `/run/user` aparece como `nobody` e o `wardd` recusa a pasta, como deve. Esperei o
  daemon sair e rodei depois: 0 falhas. Sugestão: o `cli.py` e o `dev/daemon.sh` aceitarem
  `WARD_DEV_SOCKET_DIR`, para os workers em paralelo não disputarem o socket.
- RA1 (mesmo uid): um processo do mesmo usuário pode abrir `/proc/<chrome>/fd/3` (o
  Yama só restringe o `ptrace` ATTACH, não abrir o fd) e falar CDP com o Chrome, além de
  ler o perfil temporário. É o mesmo limite de RA1 do escopo. A janela é a duração da
  sessão (segundos), e o contexto é off-the-record.
- A mensagem de V6.15 no contrato é "domínio não confere". O código usa a mensagem do
  `OrigemDiferente` que já existe ("a origem da página não bate com a da entrada"). Não
  escolhi um lado.
- Erros novos (`LoginRecusado`, `NavegadorFalhou`, `SessaoComSegredo`, com `SIMPLES`
  agora com 32) podem conflitar no merge com as Fases 40/43, se elas também acrescentarem
  variantes.
- `origem_da_url` (ward-core) é útil para as Fases 40/43 (D9): vale reaproveitar e não
  duplicar.
- O `serde_json::to_vec_pretty` do storage state realoca ao montar. São só dados de
  cookie do site, nunca a senha (a máscara confere), mas não fica em `Zeroizing`.
- Limite de ~107 bytes do caminho de socket Unix: o Chrome põe o `SingletonSocket` no
  TMPDIR, então os subdiretórios do perfil têm nomes de 1 letra. Um `<rt>` muito longo
  faz o Chrome abortar (`NavegadorFalhou`).
- Não há `pip` nem `venv` no sistema. O Playwright do Python usado no V6.12 ficou fora do
  repositório (`PW_PYTHONPATH`). Sem ele, `SEM_PLAYWRIGHT_PYTHON=1` pula só essa
  conferência.
- `.dev/servicos/` do worktree foi recopiado do checkout principal, como na Fase 28,
  porque os serviços de pé usam as chaves de lá.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/playwright.rs` | criado |
| `crates/wardd/src/cdp.rs` | criado |
| `crates/wardd/tests/playwright.rs` | criado |
| `crates/wardd/tests/chrome_falso.py` | criado |
| `dev/testes/playwright.sh` | criado |
| `dev/testes/playwright.py` | criado |
| `docs/mission/handoffs/phase-44.md` | criado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/Cargo.toml` | alterado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/ward/src/args.rs` | alterado |
| `crates/ward/src/cliente.rs` | alterado |
| `crates/ward/src/comandos.rs` | alterado |
| `crates/ward-core/src/protocolo/erros.rs` | alterado |
| `crates/ward-core/src/entrada.rs` | alterado |
| `crates/ward-core/src/entrada/validacao.rs` | alterado |
| `crates/ward-core/src/entrada/testes.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `Cargo.lock` | alterado |
| `Makefile` | alterado |
| `CLAUDE.md` | alterado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
