# Fase 32 — Credenciais (vazia, cheia, filtrada, busca) (UI)

## Implementado
- **Cliente do protocolo no app (`app/nucleo/src/cliente.rs`, novo).** `estado`, `lista`,
  `detalhe` e `copiar` usam só operações que já existem em `ward_core::protocolo`, no
  papel app (`codificar_pedido`/`decodificar_resposta`). Não há operação nova.
  - Resposta de outro tipo, detalhe de outra entrada ou quadro que não decodifica dá
    `RespostaInvalida`.
  - O `Erro` do daemon vira um `ErroApp` de mensagem fixa: `Travado`,
    `EntradaInexistente`, `CampoInexistente`, `NaoAutorizado` ou `Recusado`. O código
    interno do protocolo não chega à interface.
- **Trait `Daemon` (`daemon.rs`).** Passou a ser `trocar(&[u8]) -> Zeroizing<Vec<u8>>`,
  troca de um quadro. O `DaemonSocket` real ainda responde `SemProtocolo` (veja
  "Pontos de ligação" abaixo).
- **Daemon falso (`app/nucleo/src/daemon_falso.rs`, novo, só `cfg(test)` ou
  `modo-teste`).**
  - Atende pelo protocolo de verdade: decodifica o pedido no papel app, responde e
    codifica.
  - Usa as 21 entradas do `cofre_de_teste()` do `ward-core`, o mesmo arquivo das
    fixtures. Um teste confere nome, tipo, grupo, ambiente e nível contra
    `dev/fixtures/entradas.json`.
  - Tem dois modos: `cheio()` e `vazio()`.
  - D50 já está aplicado: `ItemLista::de_entrada(e, Papel::App)`.
- **Modelos da tela (`app/nucleo/src/credenciais.rs`, novo).**
  - `ItemCredencial` traz nome, tipo, grupo, ambiente, nível, resumo da 2ª linha
    (como "postgres · app@127.0.0.1:55432"), endereço e usuário.
  - `DetalheCredencial` traz:
    - os campos de texto com rótulo em português;
    - os segredos só pelo id e rótulo;
    - os extras, com o valor só quando não é segredo;
    - os anexos só com nome e tamanho.
  - Nenhum tipo tem campo para valor secreto.
- **Serviço e comandos.**
  - `listar_credenciais` e `detalhe_credencial { entrada }` entraram no `COMANDOS`, no
    `comandos.js`, na capability `principal`, no Tauri (`main.rs`) e na ponte de teste.
  - `copiar_campo` agora resolve pelo detalhe:
    - segredo vai por `copiar(Alvo::Campo)`;
    - extra secreto vai por `copiar(Alvo::Extra)`;
    - texto usa o valor que o daemon mandou no detalhe.
  - O valor sempre vai pelo `Copiador` (D4), e a resposta é `null`.
  - O nome da entrada passa por `validar_nome_entrada` antes de chegar ao daemon.
- **Ponte de teste.** `ward-ponte-teste --dist app/dist-teste --daemon-falso
  cheio|vazio` serve a interface com o daemon falso. Sem a opção, continua com o socket
  real.
- **Interface.**
  - `app/src/telas/Credenciais.svelte` e `app/src/telas/credenciais.js` (regras puras):
    - agrupamento por grupo, em ordem de nome, com cor 1–5;
    - busca por nome, host/URL e usuário, sem acento e sem caixa;
    - filtros de grupo, tipo e ambiente combinados com a busca;
    - resumo do topo ("21 entradas · 4 grupos", "2 de 21 entradas · filtros ativos",
      "Nenhuma entrada");
    - linha "Tipo banco · ambiente stage · busca pg" com "Limpar"; botão
      "Filtros · N";
    - contagem por grupo ("2 de 13 entradas").
  - Detalhe: cabeçalho, grade de campos com "Copiar", `Campo secreto` (só máscara),
    extras, cartão do nível (livre, com aprovação, bloqueado) e rodapé.
  - Estado vazio de `Credenciais / Vazia`: busca e filtro desabilitados, texto da lista
    e `Destaque`.
  - Teclado:
    - setas, Home e End movem o foco entre os itens visíveis (roving tabindex);
    - Enter ou clique abre o item;
    - Ctrl+K em qualquer lugar ou "/" fora de um campo foca a busca;
    - na busca, seta para baixo vai à lista, Enter abre o 1º item e Esc limpa.
  - Componentes novos: `CabecalhoDeGrupo` (901), `Destaque` (18673) e
    `DialogoFiltros` (5222, `<dialog>` nativo com foco preso e Esc).
  - Componentes alterados:
    - `ItemDeLista` compacta: duas linhas como 4002/4028/4045. A borda do selecionado
      é por dentro, então o item mantém 76 px.
    - `Campo`: `pilula`, `rotuloOculto`, `somenteLeitura`, `bind:entrada` e repasse
      de atributos.
    - `AcaoDeCampo`: repasse de atributos.
    - `tipos.js`: chaves iguais às do protocolo (`ftp-sftp`, `nota-segura`) e
      `tipoDe()` com fallback.
    - `BadgeTipo`.
- **Merge com a Fase 37.**
  - Um só `NavegacaoLateral.svelte`, o da 37.
  - O ícone do item ativo passou de tom `forte` para tom `marca`. O node `4073` usa
    #93A8FB = `--cor-icone-marca`, e o `forte` é branco.
  - A tela Credenciais roda dentro da casca do `App.svelte` da 37 (`tela ===
    'credenciais'`).
  - Os 8 comandos dos dois lados estão mantidos.
- **Conferência no Chrome.**
  - Playwright Python headless, 1280×800, ponte de teste com daemon falso.
  - Capturas em `docs/mission/evidence/fase-32/` (`app-credenciais-*.png`), lado a lado
    com o PNG dos frames (`design-credenciais-*.png`). O resultado do roteiro está em
    `qa.json`.
  - As medidas, cores e textos batem com os frames 4780, 4063, 4410 e 5259. As
    diferenças estão em "Problemas".

### Pontos de ligação com o daemon real (Fase 22/23)
1. **`app/nucleo/src/daemon.rs` › `DaemonSocket::trocar`.** Hoje confere a conexão e
   devolve `SemProtocolo`. Falta trocar por `escrever_quadro` + `ler_quadro` no socket
   já aberto (D47: uma conexão só, sem reconectar).
   - Os avisos que o daemon manda sem pedido (`pedido_de_aprovacao`,
     `pedido_de_destrave`) precisam ser separados da resposta. Pode ser uma thread
     leitora com fila, ou um laço que repassa avisos ao Tauri como evento.
   - Só isso muda. `cliente.rs`, `servico.rs` e a interface já falam o protocolo.
2. **Papel app.** O `make dev-daemon` dá papel agente a todo cliente, e aí a `Lista`
   chega sem nível (D50). O cliente recusa isso como `RespostaInvalida`, porque
   `decodificar_resposta(Papel::App, …)` exige o nível. Para testar a tela contra o
   daemon de teste, ele precisa reconhecer o app no modo de teste. Isso está registrado
   no handoff da 22.
3. **N+1 pedidos.** `listar_credenciais` faz 1 `lista` + 1 `detalhe` por entrada, porque
   o resumo da 2ª linha e a busca por usuário precisam de campos que a `ItemLista` não
   tem. Com o daemon real, isso dá 22 trocas pelo socket ao abrir a tela. Veja
   "Problemas".
4. **`estado_do_cofre`.** Usa `cliente::estado`. Com o socket real, a tela abre quando o
   daemon responde `travado: false`.

## Segurança
- **Invariantes tocadas.**
  - **S11 / D4 (V5.3):**
    - A lista e o detalhe que vão à interface são tipos sem campo de segredo.
    - O `Campo secreto` só desenha a máscara.
    - "Copiar" responde `null`, e o valor vai do Rust para a área de transferência.
    - A tela não guarda nada em `localStorage`/`sessionStorage`/IndexedDB/URL.
    - O detalhe só existe no estado do componente e some ao trocar de tela, porque o
      componente desmonta.
  - **S1 (resposta do daemon não confiável):**
    - Resposta de outro tipo é recusada.
    - Detalhe de outra entrada é recusado.
    - Detalhe forjado com `"valor"` dentro de um segredo é recusado pelo decodificador
      estrito (`deny_unknown_fields`).
  - **Entrada da interface não confiável:**
    - `entrada` passa por `validar_nome_entrada`, e um nome fora do formato nem chega
      ao daemon.
    - Erros têm mensagem fixa, sem eco do pedido.
    - Na ponte, `entrada` de outro tipo ou ausente dá `pedido-invalido`.
  - **Capability (V5.1):** entram só `allow-listar-credenciais` e
    `allow-detalhe-credencial`, os dois comandos que a tela usa.
  - **CSP:** não mudou. `app-dist.py` confere que não há inline nem URL remota.
- **Testes de ataque.**
  - `app/nucleo/src/cliente.rs` › `ataque_resposta_de_outro_tipo_ou_forjada_e_recusada`.
  - `app/nucleo/src/servico.rs`:
    - `ataque_lista_e_detalhes_de_todas_as_entradas_sem_segredo`: as 21 entradas e as
      formas literal, JSON-escapada, base64 e por linha de cada segredo;
    - `ataque_copiar_nao_devolve_o_valor`;
    - `ataque_erro_nao_carrega_o_pedido`;
    - `ataque_nome_de_entrada_fora_do_formato_nem_chega_ao_daemon`.
  - `app/nucleo/src/daemon_falso.rs` › `so_o_papel_app_e_atendido`.
  - `crates/ward-ponte-teste/tests/ataques.rs` ›
    `ataque_respostas_da_tela_credenciais_sem_segredo`: lista e detalhe das 21 pela
    ponte, mais nome com `\n`, HTML, número e ausente.
  - No Chrome, pelo roteiro de QA (`docs/mission/evidence/fase-32/qa.json`):
    - abre as 21 entradas;
    - confere `outerHTML`, os `value` dos inputs, a URL e as respostas da ponte contra
      os 19 segredos do cofre de teste: nenhum achado;
    - `localStorage`, `sessionStorage`, cookie e IndexedDB ficam vazios;
    - "Copiar" de senha e de host responde `{"ok":true,"valor":null}`.
- **Dependências novas.**
  - `serde` (derive) no `ward-app-nucleo`, a mesma versão que o workspace já usa via
    `ward-core`. Serve para serializar os modelos da tela para o Tauri e para a ponte.
    Não é crate nova no `Cargo.lock`.
  - Nenhuma no `package.json`.

## Ficou de fora
- **Botões sem ação, que pertencem a outras fases:** Editar, Duplicar, Excluir e Nova
  credencial (33), Mudar nível (35), Travar agora (31/38), revelar do `Campo secreto`.
- **"Revelar" com 30 s (parte de V5.3/V5.11).** O protocolo não tem operação que
  devolva o valor ao app. A operação existente (`copiar`) já serve só para a área de
  transferência, e criar operação nova estava fora do pedido.
- **Metadados "Criada em … · Alterada em …" do rodapé do detalhe.** Nem `DetalheEntrada`
  nem `ItemLista` têm datas, então o rodapé fica sem o texto.
- **Anexos no detalhe.** O modelo já traz nome e tamanho, mas o frame Cheia não desenha
  anexos e a tela não os mostra. Isso fica para a fase de anexos (V5.12).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (gitleaks: download deu 504 duas vezes; binário copiado do checkout principal com o sha256 conferido) | 2 → contornado |
| `cargo test -p ward-app-nucleo -p ward-ponte-teste --features ward-ponte-teste/modo-teste` (depois dos dois merges: 42 + 6 + 10) | 0 |
| `make app-front` | 0 |
| `PKG_CONFIG_PATH=<scratchpad>/sysdeps/... make app-check` (fmt, clippy, ACL, audit, deny, release + checagem de strings V5.6) | 0 |
| `make check` (depois do merge da 37 e do master atual) | 0 |
| roteiro Playwright (`qa32.py`) contra a ponte `--daemon-falso cheio` e `vazio` | 0 |

## Problemas descobertos
- **N+1 no `listar_credenciais`** (1 + 21 trocas). O resumo da 2ª linha (`usuario@host:porta
  · salto|chave|senha`, motor) e a busca por usuário precisam de campos que a `ItemLista`
  não tem. Vale avaliar se a `ItemLista` do papel app deve ganhar `usuario` e um resumo
  (protocolo, Fase 20/22). Não foi mudado aqui.
- **O frame mostra "20 entradas" e "12 entradas" no acme; o cofre de teste tem 21 e 13**
  (D28 acrescentou `acme-totp-dev`). A tela mostra a contagem real.
- **Não há frame para "busca/filtros sem resultado".** A tela mostra "Nenhuma entrada com
  essa busca ou esses filtros." na lista e um estado vazio com `Ícone/busca` no detalhe,
  no mesmo padrão de `Credenciais / Vazia`. Precisa de frame no FigCraft.
- **Bloqueado:** não há frame do cartão de nível bloqueado em Credenciais. O texto veio
  do diálogo 5935 + "Mudar o nível pede o PIN.", e o ícone é `olho-fechado`.
- **`NavegacaoLateral` da Fase 37** usava tom `forte` (branco) no ícone ativo; o design
  (4073) usa `--cor-icone-marca`. Corrigido no merge.
- **O `make dev-daemon` ainda não dá papel app** (veja "Pontos de ligação", item 2). A
  ponte com socket real não abre a tela Credenciais até isso existir.
- **Console do Chrome:** às vezes aparece um 404 de recurso. É o `favicon.ico`, que a
  ponte não serve. Não é da tela.
- **R32:** `libwebkit2gtk-4.1-dev` não está instalado. O `make app-check` usou o
  `PKG_CONFIG_PATH` do scratchpad, como a Fase 29.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/Cargo.toml` | alterado |
| `app/nucleo/src/cliente.rs` | criado |
| `app/nucleo/src/credenciais.rs` | criado |
| `app/nucleo/src/daemon_falso.rs` | criado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/Cargo.lock`, `Cargo.lock` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/telas/Credenciais.svelte` | criado |
| `app/src/telas/credenciais.js` | criado |
| `app/src/lib/componentes/CabecalhoDeGrupo.svelte` | criado |
| `app/src/lib/componentes/Destaque.svelte` | criado |
| `app/src/lib/componentes/DialogoFiltros.svelte` | criado |
| `app/src/lib/componentes/ItemDeLista.svelte` | alterado |
| `app/src/lib/componentes/Campo.svelte` | alterado |
| `app/src/lib/componentes/AcaoDeCampo.svelte` | alterado |
| `app/src/lib/componentes/BadgeTipo.svelte` | alterado |
| `app/src/lib/componentes/tipos.js` | alterado |
| `app/src/lib/componentes/NavegacaoLateral.svelte` | alterado (tom do ícone ativo) |
| `app/src/lib/ponte/comandos.js` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado (seção Fase 32) |
| `docs/mission/evidence/fase-32/*` (5 capturas do app, 4 PNG do design, `qa.json`) | criado |
| `docs/mission/handoffs/phase-32.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia headless; o original não foi alterado)
- [x] Ficou dentro do escopo da fase
