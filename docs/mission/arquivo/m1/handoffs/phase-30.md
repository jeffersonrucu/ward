# Fase 30 — Primeiro uso (UI)

## Implementado
- **Telas** `Primeiro uso / Criar cofre`, `/ Senha mestra` e `/ PIN`
  (`app/src/telas/PrimeiroUso.svelte` + `primeiro-uso.js`), feitas a partir dos frames
  `3537`, `3619` e `3733` do FigCraft (ids em `docs/mission/design-nodes.md` › "Fase 30").
  - `App.svelte`: com o estado `sem-cofre`, abre o primeiro uso. Ao criar, vai para a
    casca destravada (Credenciais, vazia).
  - Passo 2: senha mestra com no mínimo 12 caracteres (a ajuda do design), confirmação,
    medidor de força (tamanho × log2 do alfabeto, limiares de D65 f) e o aviso de que a
    senha não tem recuperação.
  - Passo 3: PIN e confirmação com o `Campo PIN` já existente; nota info do PIN.
- **Componentes novos**, um para cada componente da Design System: `PainelDeMarca`
  (717), `IndicadorDePassos` (741) e `CampoSenha` (17524/17511). O `Destaque` ganhou
  `tipo` (`neutro`, `aviso` 18684, `info` 18679).
  - `CampoPin`: uma regra de CSS, para o foco continuar visível com os 6 dígitos cheios
    (V5.25).
- **Lado Rust** (`app/nucleo`):
  - `primeiro_uso.rs` (novo): `NovoCofre { senha, confirmacao, pin, confirmacao_pin }`
    com `deny_unknown_fields`; cada valor num `Digitado` (`Zeroizing<String>`, `Debug`
    mascarado, `Deserialize` que toma posse do texto). `erros()` confere senha (12 a
    `MAX_SEGREDO`), PIN (4 a 6 dígitos ASCII) e as duas confirmações (em tempo
    constante, `ctutils`). Erros por campo, com texto fixo.
  - `cliente::criar_cofre`: `Pedido::CriarCofre` com `Segredo` do núcleo. Só senha e PIN
    vão ao daemon. `cofre_ja_existe` vira `ErroApp::CofreJaExiste` e `falha_ao_gravar`
    vira `ErroApp::CofreNaoGravado`, com mensagem que diz o que conferir.
  - `Servico::criar_cofre`: confere tudo; com erro de campo, nada vai ao daemon. Responde
    `ResultadoCriar { criado, erros }`, sem valor.
  - Comando `criar_cofre { dados }` em `comandos.rs`, na capability
    (`allow-criar-cofre`), em `ponte/comandos.js`, no `generate_handler!` e na ponte de
    teste.
  - `daemon_falso`: `DaemonFalso::sem_cofre()`. Responde `cofre_existe: false` e
    `cofre_inexistente` ao resto. O `criar_cofre` guarda só o `HashPin` (D30) e deixa o
    cofre vazio e destravado, como o `wardd`. No cofre já existente, responde
    `cofre_ja_existe`. A ponte ganhou `--daemon-falso sem-cofre`.
- **D30:** quem grava o hash Argon2id do PIN na configuração do corpo é o `wardd`
  (`nucleo.rs` › `criar_cofre`, Fase 22). Esta fase não mexeu no daemon nem no
  `ward-core`.

### Merge com o master (34, 23a, 35)
- Conflitos resolvidos com os dois lados em `cliente.rs`, `daemon_falso.rs`,
  `servico.rs`, `main.rs` do Tauri, `App.svelte`, `CampoPin.svelte`, `comandos.js`,
  `http.rs`, `main.rs` e `ataques.rs` da ponte e `design-nodes.md`.
  - `cliente.rs`: `criar_cofre` fica junto de `papel`, `tem_papel_app` e `abrir_app`
    (23a), e os erros `CofreJaExiste` e `CofreNaoGravado` junto de `PinIncorreto` (35).
  - `daemon_falso`: o `sem_cofre` convive com `erros_pin`/`travado` (35). Depois do
    `criar_cofre`, o `mudar_nivel` confere o PIN no hash criado, não no PIN de teste.
  - `CampoPin`: ficaram a borda do dígito cheio (35) e o foco visível com 6 dígitos
    (30).
  - Ponte: `--daemon-falso sem-cofre` entra junto de `[]` (ponte aberta pelo daemon,
    23a) e de `--anexos` (34).
- **Ponta a ponta com o `wardd` real:** o `wardd` de teste do worktree subiu sem cofre e
  sem estado em `.dev/`, com a ponte como `WARD_DEV_APP` (mesmo `env -i` do
  `dev/daemon.sh`, que recusa subir sem cofre), e a ponte foi aberta pelo `dev/ponte.sh`.
  O QA inteiro (`qa30.py`) passou; o `criar_cofre` real gravou o cofre e o estado
  (`vista 1`, `queimada 1`), e os logs do daemon e da ponte não têm a senha nem o PIN.
  Depois parei o daemon e apaguei o cofre e o estado de teste do worktree.

## Segurança
- **Invariantes tocadas:**
  - **S9 / D30:** o PIN só vai ao daemon pelo `criar_cofre`, depois de conferido. O app
    nunca o grava, e o daemon (real e falso) guarda só o hash.
  - **S11:** nenhuma resposta do `criar_cofre` traz senha ou PIN. Os valores digitados
    ficam no estado local do componente e são apagados ao criar e ao sair da tela.
    Nada vai a `localStorage`, `sessionStorage`, URL ou estado global; o QA conferiu.
  - **S1:** a entrada da interface não é confiável. Campo a mais (`nivel`), tipo
    errado, PIN fora de 4 a 6 dígitos ou com letras e confirmação diferente são
    recusados no Rust, mesmo que a tela seja contornada.
  - **ACL:** `criar_cofre` só passa na janela `principal`, com a origem local.
- **Testes de ataque:**
  - `app/nucleo/src/primeiro_uso.rs`:
    - `ataque_pin_com_3_ou_7_digitos_ou_letras_e_recusado` (inclui dígitos de largura
      cheia, espaço, `\n`, NUL e `+`/`-`)
    - `ataque_confirmacao_diferente_e_recusada`
    - `ataque_senha_curta_ou_grande_demais_e_recusada`
    - `ataque_erros_e_debug_nao_mostram_valor`
    - `ataque_campo_a_mais_ou_tipo_errado_e_recusado`
    - `pelo_servico::ataque_pin_ou_confirmacao_invalidos_nao_chegam_ao_daemon` (espião
      no transporte: zero quadros)
    - `pelo_servico::ataque_criar_por_cima_de_cofre_existente_e_recusado`
    - `pelo_servico::daemon_guarda_so_o_hash_do_pin`
    - `pelo_servico::cria_o_cofre_e_fica_destravado` (o quadro não leva as
      confirmações)
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_primeiro_uso_pela_ponte`
  - `app/src-tauri/src/main.rs` ›
    `ataque_criar_cofre_so_na_janela_certa_e_conferido_no_rust`
- **QA no Chrome** (`docs/mission/evidence/fase-30/qa.json`, 36 checagens, todas
  `true`): V5.7 (PIN com 3 e 7 dígitos e com letras recusado; senha curta e
  confirmação diferente recusadas; Enter avança; o foco vai ao campo com erro); V5.25
  (ordem do Tab nos passos 2 e 3, foco visível em cada parada, inclusive no PIN cheio);
  V5.26 (com o sistema em tema claro o app continua escuro, sem alternância de tema e
  sem texto em inglês nos 3 passos e na Credenciais vazia). Também conferido: nenhum
  valor no HTML, armazenamento vazio, URL sem token e sem valor, só pedidos locais,
  console sem erro (fora o 404 do `/favicon.ico` que o Chrome pede à ponte).
- **Dependências novas:** nenhuma. O Playwright do QA roda fora do repositório, no
  scratchpad.

## Ficou de fora
- **Daemon real:** o transporte do app (Fase 23a) ainda não existe, então o
  `criar_cofre` ponta a ponta com o `wardd` não foi exercitado pela tela. O daemon já
  atende a operação (Fase 22/22b).
- **"Imprimir o kit de emergência"** só é citado no aviso; a tela é da Fase 8/M7.
- **Destravar** é da Fase 31. `PainelDeMarca` e `CampoSenha` ficam prontos para ela.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-app-nucleo --all-features primeiro_uso` | 0 |
| `cargo test -p ward-ponte-teste --all-features` | 0 |
| `make app-front` (3 vezes) | 0 |
| ponte `--daemon-falso sem-cofre` + Playwright (`qa30.py`, Chrome headless, 1280×800, tema claro do sistema): a 1ª rodada teve só `console_sem_erro` falso, pelo 404 do `/favicon.ico` da ponte; o filtro foi ajustado e a rodada final deu tudo `true` | 0 |
| `make app-check` com `PKG_CONFIG_PATH` do scratchpad (R32): a 1ª rodada falhou no `cargo fmt --check` do `app/src-tauri`; corrigi com `cargo fmt` | 2, depois 0 |
| `make ferramentas`: o download do gitleaks deu 504 do GitHub duas vezes; copiei o binário do `.dev/ferramentas` do repositório principal, e o `make gitleaks-binario` conferiu o sha256 | 2, 2 (sha256 depois: 0) |
| `make check`: a 1ª rodada falhou no `clippy::type_complexity` de um teste em `primeiro_uso.rs`, corrigido com um `type`. A 2ª, rodando ao lado do `make app-check`, falhou só no `wardd --test socket` › `v3_33_kill_9_entre_as_senhas_erradas`, por tempo (veja Problemas). A 3ª, sozinha, passou limpa | 2, 2, depois 0 |
| `make app-front` (final) | 0 |
| Depois do `git merge master` (34, 23a, 35): `make check` | 0 |
| Depois do merge: criar cofre pela ponte aberta pelo `wardd` real (cofre e estado em `.dev/` sem estado prévio, `qa30.py`), daemon parado e cofre de teste apagado no fim | 0 |
| Depois do merge: `make app-check` com `PKG_CONFIG_PATH` do scratchpad | 0 |
| Depois do merge: ponte `--daemon-falso sem-cofre` + `qa30.py` | 0 |
| Depois do merge: `make app-front` (o `npm audit` mostra 2 de severidade baixa, abaixo do limite do alvo) | 0 |

## Problemas descobertos
- **Mínimo de 12 caracteres:** o texto de ajuda do design pede 12, mas o escopo, o
  PRD e o protocolo não têm mínimo (o `wardd` aceita senha de 1 byte). O app impõe 12 no
  Rust e na tela. O `ward-core` e o daemon não impõem. Decidir se o mínimo entra em
  `ward_core::limites` e no `wardd` (e na troca de senha, V5.22).
- **Senha mestra sem normalização Unicode:** o app manda os bytes UTF-8 do texto como
  foram digitados, e o `ward destravar` lê os bytes do TTY. Se um método de entrada
  produzir "é" composto num lado e decomposto no outro, a senha não confere. Decidir se
  há NFC (no app, na CLI e no daemon) ou se fica como está.
- **Força da senha:** o medidor é uma estimativa por alfabeto. Ele não conhece
  dicionário nem repetição (dependência nova como o zxcvbn foi descartada) e usa os
  rótulos do gerador ("forte"). O design mostra "Força: boa".
- **Largura do `Painel de marca`:** os 560 px no FigCraft não estão ligados a uma
  variable. Usei `--largura-dialogo-largo` (mesmo valor). Uma fase de design pode criar
  `largura/painel-marca`.
- **Cópias do valor no JS e no IPC:** a senha e o PIN passam por strings do JS e pelo
  corpo JSON da IPC do Tauri (e pelo `Vec` do corpo HTTP na ponte de teste), e nada
  disso é zerado. O lado Rust zera a partir do `Digitado`. Esse limite já é aceito para o
  formulário (Fase 33) e vale também para o destravar (Fase 31).
- **`CampoPin` corta em silêncio:** letra e 7º dígito não entram no campo. O aviso
  "Só números…" e "no máximo 6 dígitos" vem de um `beforeinput` na tela, para não mudar
  o comportamento do componente que a Fase 35 também usa.

- **Teste com prazo apertado (fora da fase):** `crates/wardd/tests/socket.rs` ›
  `v3_33_kill_9_entre_as_senhas_erradas` exige a 1ª espera abaixo de 900 ms. Com a
  máquina carregada (o `make app-check` rodando junto), ela deu 2,28 s e o teste falhou.
  Sozinho, passa. Não mexi; vale folgar o limite ou medir só a espera do D20, sem o
  Argon2 e a subida.

- **`make app-ponte` não cobre o primeiro uso:** o `dev/daemon.sh` recusa subir sem
  `.dev/cofre-teste.ward` ("rode 'make dev-cofre'"), e o socket de teste
  (`/run/user/<uid>/ward-teste/`) é um só para todos os worktrees. Testei com um script
  fora do repositório. Vale um `dev/daemon.sh subir --sem-cofre` (só com `COFRE` em
  `.dev/`) numa fase de ferramentas.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/primeiro_uso.rs` | criado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/telas/PrimeiroUso.svelte` | criado |
| `app/src/telas/primeiro-uso.js` | criado |
| `app/src/lib/componentes/CampoSenha.svelte` | criado |
| `app/src/lib/componentes/IndicadorDePassos.svelte` | criado |
| `app/src/lib/componentes/PainelDeMarca.svelte` | criado |
| `app/src/lib/componentes/Destaque.svelte` | alterado |
| `app/src/lib/componentes/CampoPin.svelte` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-30/` (9 PNGs + `qa.json`) | criado |
| `docs/mission/handoffs/phase-30.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; o original tem o mesmo sha256)
- [x] Ficou dentro do escopo da fase
