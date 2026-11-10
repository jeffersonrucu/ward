# Fase 34 — Formulário parte 2: banco, servidor (com Conectar), ftp/sftp e anexos (UI)

## Implementado
- **Tipos da fase:** `banco`, `servidor` e `ftp/sftp`, com Novo e Edição, a partir dos
  frames do FigCraft (ids em `docs/mission/design-nodes.md` › "Fase 34"). Os três ficam
  habilitados em `Nova credencial / Escolher tipo` e no "Editar" de Credenciais.
- **Nenhuma operação nova no protocolo.** Usei `salvar_entrada` (com `anexos`, que o
  `Rascunho` já tinha), `detalhe`, `revelar` e `emitir_token`.
- **`app/nucleo/src/formulario.rs`:**
  - `tipo_do_formulario` aceita os 9 tipos; `campos_do_tipo` e `montar` ganharam banco
    (`motor`, `host`, `porta`, `base`, `usuario`, `tunel`; `senha`), servidor (`host`,
    `porta`, `usuario`, `salto`; `senha`, `chave_privada`, `passphrase`) e ftp/sftp
    (`protocolo`, `host`, `porta`, `usuario`; os mesmos três segredos);
  - `Formulario.anexos`: lista de ids dos anexos escolhidos no Rust (o conteúdo nunca
    vem da interface);
  - erros do daemon mapeados para os campos novos (`motor`, `base`, `tunel`, `salto`,
    `protocolo`, `chave_privada`, `passphrase`, `autenticacao`, `anexos`). A recusa do
    orçamento da máscara (D29, campo `entrada`) aparece no campo dos anexos;
  - `copiar_rascunho` (validação local) cobre os três tipos e os anexos.
- **`app/nucleo/src/anexos.rs` (novo):** `Seletor` (quem escolhe o arquivo), `Anexos`
  (pendentes em `Segredo`, que zera ao sair) e `Escolha` (`anexado {id, nome, tamanho}`,
  `cancelado` ou `aviso {motivo}`). Lê só arquivo regular, com o tamanho conferido pelo
  `fstat` do arquivo aberto e de novo pelo que foi lido (`take(MAX+1)`). Números de
  `ward_core::limites` (D32): acima de `MAX_ANEXOS_SOMADOS` (256 KiB) ou soma acima
  disso → aviso, o arquivo não é guardado nem enviado; vazio → aviso; mais de
  `MAX_ANEXOS` → aviso.
- **`app/nucleo/src/terminal.rs` (novo):** `Token` (o `emitir_token` só é aceito com
  `[A-Za-z0-9_-]`, 16 a 64 bytes, sem começar por `-`; `Debug` mascarado), `Terminal` e
  `TerminalDoSistema`: `/usr/bin/x-terminal-emulator -e /usr/local/bin/ward ssh --token
  <t> <entrada>` (D3), ambiente montado do zero (`PATH` fixo e só `HOME`, `LANG`,
  `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`,
  `XDG_CURRENT_DESKTOP`, `XDG_SESSION_TYPE`), cwd `/`, stdio nulo, espera numa thread.
  O nome da entrada só vai ao argv no formato do `ward-core` (não vira opção).
- **`cliente::emitir_token`**; **daemon falso** responde `emitir_token` só para
  `servidor` (token aleatório de 32 pelo `ward_core::gerador`).
- **`servico.rs`:** `com_seletor`, `com_terminal` (padrão: `SemSeletor`/`SemTerminal`,
  que respondem erro fixo), `escolher_anexo`, `remover_anexo`, `conectar` (só
  `servidor`; o token vai ao terminal e nunca à resposta). O `salvar_credencial` põe no
  rascunho cópias dos anexos pedidos pelo id (id repetido ou desconhecido é pedido
  inválido); salvo, cancelado ou ao sair, os anexos escolhidos são zerados.
- **Comandos novos**, com ACL: `escolher_anexo`, `remover_anexo`, `conectar` em
  `comandos.rs`, `capabilities/principal.json`, `ponte/comandos.js`, `generate_handler!`
  e o despacho da ponte.
- **Tauri:** `SeletorGtk` (o `GtkFileChooserDialog` do GTK, no lado Rust; comando
  síncrono, na thread principal) e `TerminalDoSistema`.
- **Ponte de teste:** `--anexos <pasta>` (opcional, no fim) troca o seletor por
  `SeletorDePasta` (o único arquivo regular da pasta, sem seguir link; a interface não
  manda caminho). O terminal da ponte (`TerminalDeTeste`) não abre nada: escreve
  "conectar <entrada> (token de uso único com N caracteres)" na saída de erro.
- **Interface:**
  - `formulario.js`: definições dos três tipos (placeholders do design, porta padrão por
    motor/protocolo, texto de "Acesso" do banco pelo motor, D10), "Autenticação" só da
    tela (decide quais segredos vão; os da outra forma vão `vazio`), `tamanhoLegivel`,
    `variavelDoAnexo` e `anexos` no pedido;
  - `Formulario.svelte`: selects de motor, protocolo, autenticação, túnel e salto
    (servidores do cofre, menos a própria entrada); com `ftp` a autenticação só oferece
    senha; chave privada nova em `textarea` mascarado (`-webkit-text-security: disc`),
    porque o `<input>` perde as quebras de linha; seção Anexos com "Adicionar arquivo",
    lista (`Anexo`) e aviso; "Conectar" no cabeçalho da edição de `servidor`;
  - `Anexo.svelte` (componente `Anexo` 6849); `Credenciais.svelte` passa os servidores
    ao formulário.

## Segurança
- **Invariantes tocadas:**
  - **S11 / V5.12:** o conteúdo do anexo nunca passa pela webview: o arquivo é
    escolhido (GTK ou pasta da ponte) e lido no Rust; a interface recebe id, nome e
    tamanho. O token do "Conectar" também não volta à interface. A chave privada
    digitada fica só no estado do formulário (como qualquer valor digitado, Fase 33),
    mascarada na tela.
  - **S1:** tudo que vem da interface é não confiável: ids de anexo (repetido,
    desconhecido, de tipo errado), `conectar` com nome que viraria opção, formulário
    com os dois métodos de autenticação ou chave no ftp (recusados pela validação),
    `manter` da chave numa entrada que só tem senha. A resposta do daemon também: token
    fora do formato é recusado antes de ir a um argv.
  - **D3 / S3:** o token vai no argv do terminal, como D3 define (não é segredo de
    credencial, morre no primeiro uso). Nenhum segredo de credencial vai a argv.
  - **D29 / D32 / V5.12:** limites de `ward_core::limites`; aviso antes de salvar; vazio
    recusado; recusa do orçamento da máscara no campo dos anexos.
  - **D47:** o terminal herda o processo do app depois de largar o grupo `ward-app`
    (antes do GTK), então não ganha o papel app; o ambiente do terminal é montado do zero.
  - **ACL:** os três comandos só na janela `principal`, com a origem local.
- **Testes de ataque:**
  - `app/nucleo/src/anexos.rs`: `ataque_arquivo_grande_soma_e_vazio_nao_sao_guardados`,
    `ataque_ids_forjados_pela_interface`, `ataque_anexos_demais`,
    `anexa_lista_e_remove_sem_conteudo_na_resposta`
  - `app/nucleo/src/terminal.rs`: `ataque_token_forjado_pelo_daemon_e_recusado`,
    `ataque_entrada_que_viraria_opcao_nao_vai_ao_argv`,
    `comando_do_terminal_com_ambiente_do_zero`
  - `app/nucleo/src/servico.rs`: `ataque_anexo_nao_volta_a_interface_e_vai_no_rascunho`,
    `ataque_anexo_grande_vazio_e_id_forjado`, `ataque_autenticacao_forjada_pela_interface`,
    `ataque_conectar_so_servidor_e_sem_token_na_resposta`,
    `banco_servidor_e_ftp_salvam_e_editam`, `erros_dos_tipos_novos_no_campo`
  - `app/nucleo/src/formulario.rs`: `recusa_do_orcamento_da_mascara_vai_para_os_anexos`
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_anexos_e_conectar_pela_ponte`
  - `app/src-tauri/src/main.rs` › `ataque_formulario_so_na_janela_certa` (com os três
    comandos novos)
  - QA no Chrome (`docs/mission/evidence/fase-34/qa.json`, todas `true`): nenhum segredo
    da fixture nem da chave/anexos desta rodada no DOM em 9 momentos; 300 KiB avisa e
    não lista; vazio recusado; soma acima de 256 KiB avisa; remover; D33 (`app.teste?x`)
    recusado no campo Base; "Conectar" chega ao terminal da ponte com token de 32 e o
    token não aparece na tela; `localStorage`/`sessionStorage` vazios; URL sem token; só
    pedidos locais.
- **Dependências novas:** `gtk = "=0.18.2"` direta em `app/src-tauri` para o diálogo
  de arquivo no lado Rust (S11: o conteúdo não passa pela webview). É o mesmo crate que
  o Tauri/wry já trazem: o `Cargo.lock` só ganhou a linha `"gtk"` nas dependências do
  `ward-app`, nenhum crate novo. Sem plugin do Tauri e sem permissão nova de plugin.

## Ficou de fora
- **Editar entrada com anexos** continua recusado (`AnexosNaoMantidos`; "Editar"
  desabilitado). O protocolo não tem operação que devolva o conteúdo do anexo ao app, e
  o `salvar_entrada` substitui a entrada inteira: editar apagaria os anexos. Na edição
  de uma entrada sem anexos dá para anexar. Precisa de decisão do planner (ex.: "manter
  anexo pelo nome" no `salvar_entrada`).
- **V5.24 ponta a ponta:** o `wardd` ainda responde `interno` ao `emitir_token`
  (`nucleo.rs`, "token: 27/34") e o `ward ssh` não tem `--token` (Fase 27). O app já
  pede o token e abre o terminal; os ataques do V5.24 (reusar o token do `cmdline`,
  usar depois de 30 s, usar em outra entrada) são do daemon e da CLI e ficam para a
  fase que atender `emitir_token` e o `ward ssh --token`.
- **Terminal padrão:** `x-terminal-emulator -e` (política do Debian). Não conferi no
  COSMIC do usuário qual terminal a alternativa aponta, nem abri o terminal de verdade
  (o `ward ssh --token` ainda não existe). No modo de teste a ponte só registra.
- **Diálogo do GTK no app real:** compilado e com clippy limpo, mas não aberto numa
  janela (o QA é pela ponte). Confere na primeira rodada do app real.
- **Mudar nível** (Fase 35), **Duplicar**/**Excluir**, datas do rodapé (22d): sem
  mudança.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `cargo clippy --all-targets --all-features -p ward-app-nucleo -p ward-ponte-teste -- -D warnings` | 0 |
| `cargo test --all-features -p ward-app-nucleo -p ward-ponte-teste` (67 + 6 + 12) | 0 |
| `cargo fmt` / `clippy --all-targets -D warnings` / `test` em `app/src-tauri` (com `PKG_CONFIG_PATH` do scratchpad, R32); a 1ª rodada do clippy falhou por falta de `app/dist`, resolvida com `make app-front` | 101, depois 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso cheio --anexos <pasta>` + Playwright (`qa34.py`, Chrome headless, 1280×800); a 1ª rodada teve 2 checagens `false` por corrida no script (lia o `h2` antes do detalhe abrir), corrigido | 0 |
| `make check` | 0 |
| `make app-front` (final) | 0 |

## Problemas descobertos
- **Anexo com nome que começa por `.`** (ex.: `.env`): o `ward-core` recusa (`ANEXO_NOME`),
  e o nome é o do arquivo escolhido. O erro aparece no campo, mas não há como renomear
  na tela. O escopo cita `.env` como anexo típico: decisão do planner (renomear ao
  anexar, ou aceitar `.` no início).
- **Chave privada no Novo:** o design desenha um `Campo secreto` de uma linha; usei
  `textarea` mascarado porque o `<input>` tira as quebras de linha da chave colada.
- **Túnel e salto** listam todos os `servidor` do cofre (menos a própria entrada); a
  lista do app não diz quais têm salto, então "servidor do túnel não pode ter salto" só
  aparece como erro ao salvar.
- **Anexos no design aparecem em todos os tipos** (banco também), e o V5.12 cita só
  `servidor` e `genérico`. Deixei a seção em todos, como o design e a Fase 33.
- **"Conectar" sem estado no design:** depois do clique o ícone vira "check" por 2 s; o
  erro (daemon sem `emitir_token`) vai ao `Toast` do rodapé.
- **Console do QA:** um 404 (favicon, o mesmo da Fase 33).
- **Toast "Credencial salva."** continua visível ao abrir outro formulário logo depois
  de salvar (comportamento da Fase 33, vem de Credenciais).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/anexos.rs` | criado |
| `app/nucleo/src/terminal.rs` | criado |
| `app/nucleo/src/formulario.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/src-tauri/Cargo.toml` | alterado |
| `app/src-tauri/Cargo.lock` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/lib/componentes/Anexo.svelte` | criado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/telas/Credenciais.svelte` | alterado |
| `app/src/telas/Formulario.svelte` | alterado |
| `app/src/telas/formulario.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/lib.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/src/simulados.rs` | criado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-34/` (13 PNG + `qa.json`) | criado |
| `docs/mission/handoffs/phase-34.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; o original não foi tocado)
- [x] Ficou dentro do escopo da fase
