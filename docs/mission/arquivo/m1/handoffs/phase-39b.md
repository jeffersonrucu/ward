# Fase 39b — Anexos na edição

## Implementado
- **Protocolo** (`ward-core`): `Pedido::SalvarEntrada` ganhou `manter_anexos: Vec<String>`,
  que leva os nomes dos anexos da entrada editada (`anterior`) que continuam nela. O
  campo é opcional no quadro e só vai quando a lista não está vazia, então o quadro de
  antes continua igual. A leitura recusa (`pedido_invalido`): campo sem `anterior`, nome
  vazio, repetido, com mais de `MAX_NOME_ANEXO` bytes, lista que não é de textos, e
  mantidos mais novos acima de `MAX_ANEXOS`. A escolha da 39b foi "manter por nome
  dentro do `salvar_entrada`", sem operação nova.
- `ward_core::protocolo::juntar_anexos_mantidos(rascunho, antiga, nomes)` copia o
  conteúdo de cada anexo mantido da entrada antiga, dentro do daemon, à frente dos
  anexos novos. Se um nome não existe na entrada editada (`../x`, maiúsculas,
  outra entrada) ou aparece repetido, o resultado é `pedido_invalido` e o rascunho fica
  como estava. Depois disso o rascunho passa pelo `validar()` inteiro: soma de 256 KiB
  (D29), nome repetido com um anexo novo e orçamento da máscara.
- **`wardd`** (`nucleo.rs` › `salvar_entrada`): procura a entrada `anterior` (se não
  existe, `entrada_inexistente`), junta os mantidos e valida. Anexo que não está na
  lista sai da entrada. O `daemon_falso` do app segue a mesma regra.
- **Núcleo do app**: `FormularioCredencial.anexos_mantidos` (`serde(default)`). O
  `salvar_credencial` confere os nomes no detalhe da entrada editada (nome que não
  existe, repetido ou numa entrada nova dá `PedidoInvalido`, e nada é gravado). O erro
  `AnexosNaoMantidos` da Fase 34 foi removido. Na edição, o
  `escolher_anexo(edicao: { entrada, mantidos })` conta os anexos mantidos na soma e
  na quantidade (`Base`, `Anexos::anexar_sobre`). Os nomes chegam da interface sem
  confiança e são conferidos no detalhe antes de o seletor abrir.
- **Nome com `.` no começo** (decisão pedida no plano): o `.env` entra como `env`
  (`nome_do_anexo` tira os pontos do começo, e um nome só de pontos avisa). O
  `ward-core` continua recusando anexo cujo nome comece por `.`. A tela mostra o nome
  guardado (`env · WARD_ARQUIVO_ENV`).
- **Tauri e ponte de teste**: o `escolher_anexo` aceita `edicao` opcional. Nenhuma
  capability nova (o comando já existia).
- **Interface**: "Editar" habilitado para entradas com anexos (saíram a condição e o
  tooltip). O formulário lista os anexos guardados (nome, tamanho e variável) com
  "Remover", que só tira da lista. A mudança vale ao salvar, e "Cancelar" não remove
  nada. `montarFormulario` manda `anexos_mantidos` com os nomes. O conteúdo de um anexo
  guardado nunca chega à interface.

## Segurança
- **S11 / V5.12**: o conteúdo de um anexo guardado não sai do daemon. A edição manda
  só nomes, e o daemon copia os bytes. Nenhuma resposta traz conteúdo (o detalhe
  continua só com nome e tamanho). O DOM, o `localStorage`, o `sessionStorage` e a URL
  ficaram sem conteúdo no QA.
- **S1**: o agente e a extensão continuam sem `salvar_entrada` (`nao_autorizado`), com
  ou sem `manter_anexos`.
- **D29**: a soma de 256 KiB e o limite de quantidade contam os mantidos, no app (aviso
  antes de salvar) e no daemon (validação inteira).
- **Isolamento entre entradas**: um nome mantido só alcança anexos da própria entrada
  `anterior`. Não dá para copiar o anexo de outra entrada para a sua, nem com caminho,
  nem numa entrada nova.
- Testes de ataque:
  - `crates/ward-core/src/protocolo/testes.rs` › `fase39b_ataque_manter_anexos_forjado`,
    `fase39b_ataque_juntar_anexos_mantidos`, `fase39b_manter_anexos_vai_e_volta`
  - `crates/wardd/tests/anexos_edicao.rs` › `v5_12_ataque_manter_anexos_forjado`
    (anexo de outra entrada, `../g2/b.env`, entrada inexistente, entrada nova
    "copiando", agente e extensão, 200 KiB mantidos + 60 KiB novos, nome repetido;
    o disco não muda), `v5_12_editar_mantem_adiciona_e_remove_anexo` (bytes iguais no
    disco, renomear leva os anexos, remover, resposta do `detalhe` sem o conteúdo em
    claro nem em base64)
  - `app/nucleo/src/servico.rs` ›
    `ataque_edicao_mantem_remove_e_adiciona_anexo_sem_o_conteudo_passar_pelo_app`
    (um espião registra todo quadro trocado com o daemon, e nenhum leva o conteúdo)
    e os casos forjados de `anexos_mantidos` no teste de formulário existente
  - `app/nucleo/src/anexos.rs` › `ataque_soma_com_os_anexos_mantidos`,
    `ataque_nome_com_ponto_no_comeco`
- Dependências novas: nenhuma.

## Ficou de fora
- Renomear um anexo guardado pela tela: não está pedido no V5.12.
- Ver o conteúdo de um anexo guardado: proibido por S11.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --lib fase39b` | 0 |
| `cargo test -p wardd --test anexos_edicao` | 0 |
| `cargo test --all-features -p ward-app-nucleo -p ward-ponte-teste` | 0 |
| `make app-front` | 0 (2 avisos baixos do `npm audit`, `deepmerge`, que já existiam) |
| `make fmt` | 0 |
| `make check` (1ª rodada) | 2: `v3_33_kill_9_entre_as_senhas_erradas` (tempo, máquina carregada; arquivo não tocado) |
| `make check` (2ª rodada) | 2: só o passo 6/6, porque faltava o binário do gitleaks no `.dev/` deste worktree; os passos 0–5 passaram |
| `make ferramentas` | 0 |
| `make check` (3ª rodada) | 2: de novo só `v3_33_kill_9_entre_as_senhas_erradas` (load average 41, com outros worktrees compilando; primeira espera 1,70 s, limite 1,5 s) |
| `make gitleaks` (passo 6/6 sozinho) | 0 (árvore, índice, histórico e varredura extra: nenhum vazamento) |
| `qa39b.py` (Playwright Python, Chrome headless, ponte `--daemon-falso cheio --anexos`) | 0 (18/18 checagens) |

## Problemas descobertos
- **Decisão do `.env`**: escolhi tirar os pontos do começo (`.env` vira `env`) em vez de
  afrouxar o `ward-core`, porque o nome vira arquivo em `<rt>/tmp/` e variável
  `WARD_ARQUIVO_<NOME>`. O planner deve confirmar e registrar isso em `docs/escopo.md`
  (o escopo não fala de nome com ponto no começo).
- **Teste com tempo instável**: `crates/wardd/tests/socket.rs` ›
  `v3_33_kill_9_entre_as_senhas_erradas` falhou uma vez com a máquina carregada (o
  primeiro Argon2 levou 1,85 s, e o limite é 1,5 s). Passou na 2ª rodada e falhou de novo na 3ª, com load average 41 (outros worktrees
  compilando ao mesmo tempo). O arquivo não foi tocado nesta fase; o limite merece mais
  margem ou um teste fora da carga.
- **Espaço em disco**: o disco chegou a 100% durante o build (vários worktrees com
  `target/`). Apaguei o `target/debug/incremental` deste worktree.
- O Playwright do Python não está instalado no sistema. Para o QA, baixei as wheels do
  PyPI (sha256 registrado na saída) para o scratchpad. Nada entrou no repositório.
- **`make app-check`** não rodou: falta o `libwebkit2gtk-4.1-dev` e não achei o
  `PKG_CONFIG_PATH` das fases anteriores neste worktree (R32). A mudança em
  `app/src-tauri/src/main.rs` (argumento `edicao` opcional) passou no `cargo fmt
  --check`, mas não foi compilada.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/anexos_edicao.rs` | criado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `app/nucleo/src/anexos.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/credenciais.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/formulario.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/telas/Credenciais.svelte` | alterado |
| `app/src/telas/Formulario.svelte` | alterado |
| `app/src/telas/formulario.js` | alterado |
| `docs/mission/evidence/fase-39b/` (`qa.json` e 7 PNG) | criado |
| `docs/mission/handoffs/phase-39b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): todos os passos passaram (0–5 na 2ª rodada, 6 sozinho); só o teste de tempo `v3_33` falha com a máquina carregada
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI) Conferido no Chrome headless contra a tela existente; nenhum componente novo, usa o `Anexo` (6849) que já existia
- [x] Ficou dentro do escopo da fase
