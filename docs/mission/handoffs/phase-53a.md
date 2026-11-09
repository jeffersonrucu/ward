# Fase 53a — App: botão "Carregar de arquivo" no campo "Chave privada"

## Implementado
- **Comando novo `carregar_chave { campo }`** (`app/nucleo/src/comandos.rs`, capability
  `principal`, `comandos.js`, Tauri `main.rs`, ponte `http.rs`). A interface não manda
  caminho. Só `campo = "chave_privada"` é aceito (`CAMPOS_CARREGAVEIS`); qualquer outro
  é `pedido-invalido` antes de abrir o seletor.
- **`app/nucleo/src/chave.rs`** (novo): `ler_chave` abre o arquivo escolhido com
  `O_NOFOLLOW | O_NONBLOCK | O_NOCTTY` e confere:
  - link simbólico (`ELOOP`): aviso "o arquivo é um link simbólico…";
  - o que não é arquivo comum (pasta, FIFO, `/dev/zero`): aviso;
  - mais que 64 KiB (`MAX_SEGREDO`), pelo `fstat` e de novo pelo que foi lido: aviso;
  - arquivo vazio: aviso;
  - forma de chave privada: só ASCII imprimível, `-----BEGIN <rótulo>-----` com rótulo
    OpenSSH/PEM de chave privada, miolo base64 (cabeçalhos `Nome: valor` do PEM antigo
    cifrado só antes do base64) e `-----END` do mesmo rótulo, sem nada depois. Chave
    pública, `.ppk`, certificado, duas chaves e texto qualquer são recusados.
  Os avisos são textos fixos, sem caminho nem erro do sistema. A resposta
  (`Carregamento`) é `carregada { tamanho }`, `cancelado` ou `aviso { motivo }`.
- **Serviço** (`servico.rs`): a chave lida vai para `carregadas` (`Segredo`, zera ao
  sair), por campo. O salvar com `"carregada"` (`ValorSecreto::Carregada`,
  `Fontes::carregada`) põe a chave no rascunho, que vai ao daemon pelo `salvar_entrada`.
  `descartar_formulario` (cancelar, salvar, sair, travar) apaga. `"carregada"` sem chave
  carregada, ou num campo que não é de chave, é pedido inválido.
- **Seletor**: o trait `Seletor` ganhou `escolher_chave` (o padrão é `escolher`). O GTK
  abre o mesmo diálogo com o título "Carregar chave privada" e o botão "Carregar"
  (`dialogo_de_arquivo`). A ponte usa o `SeletorDePasta` de sempre (`--anexos <pasta>`).
- **Tela** (`Formulario.svelte`, `formulario.js`): o botão **Carregar de arquivo**
  (`Botao` secundário, ícone `clipe`) fica abaixo da caixa da chave privada (`servidor` e
  `ftp/sftp` com autenticação por chave). Com a chave carregada, o campo vira o `Campo
  secreto` mascarado, com "Chave carregada do arquivo (N bytes). Ela fica oculta e vai
  para o cofre ao salvar." e "Trocar valor". Sem revelar e sem copiar. O aviso aparece
  na linha de ajuda do campo. O salvar manda `"carregada"`. O nome **Carregar de
  arquivo** é o que `docs/migracao.md` e `dev/testes/migracao.py` usam (nada mudou lá).

## Segurança
- **Invariantes tocadas**
  - **S2:** a chave vai do arquivo para a memória do app (zera ao sair) e dali ao
    daemon. Nada vai para a área de transferência nem para o disco. O app não grava
    nada; só lê.
  - **S11:** o conteúdo nunca volta à webview. A resposta tem só o tamanho e o motivo
    fixo. O QA confere DOM, `localStorage`, `sessionStorage`, URL e todas as respostas
    HTTP da ponte. A chave só aparece no "revelar" explícito do detalhe, depois de salva,
    e some ao ocultar.
  - **S12:** sem área de transferência no caminho da chave (o achado alto da auditoria
    do M10).
  - **S13:** os avisos não trazem caminho nem `io::Error`.
  - **Entrada não confiável:** campo fora da lista, argumento a mais (`caminho`) ou de
    outro tipo são recusados antes do seletor. O arquivo escolhido também não é
    confiável: link, FIFO, dispositivo, tamanho e forma são conferidos.
- **Testes de ataque**
  - `app/nucleo/src/chave.rs`:
    - `ataque_link_fifo_grande_vazio_e_texto_sao_recusados`: link para a chave, link
      para `/etc/hostname`, pasta, FIFO sem escritor (não prende), `/dev/zero`, 64 KiB
      + 1, vazio, texto, `.pub`, inexistente e sem permissão; o limite exato passa;
    - `ataque_o_que_nao_e_chave_privada_e_recusado`: 16 formas;
    - `aceita_openssh_e_pem` e `le_a_chave_sem_devolver_o_conteudo`.
  - `app/nucleo/src/servico.rs`:
    - `carregar_chave_vai_ao_daemon_sem_passar_pela_interface`: o revelar do daemon
      devolve exatamente o arquivo, e salvar descarta a chave;
    - `ataque_carregar_chave_campo_forjado_aviso_e_descartar`: campos forjados, aviso
      sem conteúdo nem caminho, link, aviso que não apaga a carregada, descartar,
      travar (`ao_sair`), cancelado e `"carregada"` na passphrase.
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_carregar_chave_pela_ponte`:
    corpos forjados (`{}`, `senha`, `passphrase`, número, `caminho` a mais), resposta
    sem miolo, nome do arquivo nem `/`, salvar com `"carregada"`, detalhe sem a chave,
    `"carregada"` depois de salvo e depois de um aviso recusados.
  - `app/src-tauri/src/main.rs` › `ataque_formulario_so_na_janela_certa`: passa a incluir
    `carregar_chave` (outra janela e origem remota recusadas pela ACL). **Não rodado**
    (ver Problemas).
- **Dependências novas:** nenhum crate novo. O `ward-app-nucleo` passou a depender de
  `libc` 0.2.191, a mesma do `ward-plataforma`/`wardd`, só pelas constantes
  `O_NOFOLLOW`, `O_NONBLOCK`, `O_NOCTTY` e `ELOOP` (valores dependem da arquitetura;
  não hardcodar). Os dois `Cargo.lock` ganharam só a linha da dependência.

## Ficou de fora
- **`make app-check`** (Tauri): o host não tem `libwebkit2gtk-4.1-dev` nem o
  `sysdeps` antigo do scratchpad. Pelo R32, a fase parou aí: `cargo fmt --check` do
  `app/src-tauri` passou; clippy, testes de ACL, audit/deny e release não rodaram.
- **QA com o `wardd` de teste real**: a ponte aberta pelo daemon (`WARD_DEV_APP`) não
  recebe `--anexos`, então não há seletor. O QA rodou na ponte com `--daemon-falso
  cheio`. O caminho até o daemon é o mesmo `salvar_entrada` de qualquer segredo do
  formulário.
- **Diálogo GTK de verdade**: sem janela nativa no QA. O `SeletorGtk` só ganhou título
  e botão.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-app-nucleo -p ward-ponte-teste` (177 + 7) | 0 |
| `cargo test -p ward-ponte-teste --features modo-teste` (21 ataques) | 0 |
| `make app-front` | 0 |
| `cargo fmt --all --check` e `cd app/src-tauri && cargo fmt --check` | 0 |
| `cd app/src-tauri && cargo clippy --all-targets --all-features --locked` | 101 (falta `libwebkit2gtk-4.1-dev`, R32) |
| QA Playwright pela ponte (`<scratchpad>/qa53a/qa.py`, Chrome headless 1280×800) | 0 (13/13) |
| `make check` (1ª rodada) | 2: `wardd` › `v3_33_kill_9_entre_as_senhas_erradas` por tempo, com load 23 (código não tocado; o Argon2 levou 1,85 s, e o limite é 1,5 s) |
| `make check` (2ª rodada) | 2: faltava o gitleaks no worktree |
| `make ferramentas` | 0 |
| `make gitleaks` | 1: 2 achados `generic-api-key` no miolo base64 da chave falsa dos testes de `chave.rs`; miolo trocado por `QUFB…` de baixa entropia |
| `make check` (3ª rodada, final) | 0 |

## Problemas descobertos
- **Teste por tempo.** `crates/wardd/tests/socket.rs` › `v3_33_kill_9_entre_as_senhas_erradas`
  falha com a máquina carregada (outros agentes em paralelo): o limite de 1,5 s para o
  primeiro Argon2 é apertado. Passou na rodada seguinte.
- **Design.** O FigCraft não tem o botão "Carregar de arquivo" nem o estado "chave
  carregada" nos frames `Formulário / servidor / Novo` (`9900`), `/ Edição` (`8981`) e
  `ftp/sftp` (`14743`). A tela usa só componentes e tokens existentes (`Botao`
  secundário com `clipe`, `Campo secreto`). Uma fase de DESIGN precisa desenhar o botão
  e os estados (aviso e carregada) e atualizar `design-nodes.md`.
- **R32.** O `make app-check` precisa do `libwebkit2gtk-4.1-dev` (sudo do usuário) ou de
  um `PKG_CONFIG_PATH` com os `.pc`. Rodar antes do merge, pelo orchestrator ou pelo
  usuário.
- **Arquivo em disco.** O botão aceita arquivo em qualquer pasta, inclusive no disco
  (`~/.ssh`). Ler não cria cópia, e o guia usa a pasta em memória. Recusar o que não está
  em tmpfs impediria carregar direto do `~/.ssh`; deixei aberto.
- **Lista de recentes do GTK.** O `GtkFileChooserDialog` pode gravar o caminho escolhido
  em `~/.local/share/recently-used.xbel`. É o caminho, não o conteúdo, e o "Adicionar
  arquivo" da Fase 34 faz o mesmo. Não conferido.
- **Trocar valor** depois de carregar volta à caixa de texto, mas a chave carregada fica
  no Rust até salvar, cancelar ou sair. Ela só é usada se o salvar mandar
  `"carregada"`, e a tela manda `{novo}` ou `"vazio"`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/chave.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/Cargo.toml` | alterado (libc) |
| `app/nucleo/src/anexos.rs` | alterado (`Seletor::escolher_chave`) |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/formulario.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/Cargo.lock` | alterado (1 linha) |
| `Cargo.lock` | alterado (1 linha) |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/telas/Formulario.svelte` | alterado |
| `app/src/telas/formulario.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/evidence/fase-53a/` (`qa.json`, 4 PNG) | criado |
| `docs/mission/handoffs/phase-53a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (daemon falso; chave falsa aleatória no scratchpad, apagada)
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: o botão não existe no design (ver Problemas); conferido no Chrome 1280×800
- [x] Ficou dentro do escopo da fase
