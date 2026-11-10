# Fase 33 — Formulário parte 1 (UI)

## Implementado
- **Tipos da fase:** login, api, aws, smtp, nota segura e genérico, com Novo e Edição, a
  partir dos frames do FigCraft (ids em `docs/mission/design-nodes.md` › "Fase 33").
- **Nenhuma operação nova no protocolo.** Usei as que já existem em
  `ward_core::protocolo`: `revelar`, `totp`, `salvar_entrada`, `detalhe` e `copiar`.
- **`app/nucleo/src/cliente.rs`:**
  - `revelar(entrada, alvo)` devolve um `Segredo`;
  - `totp(entrada)` devolve `CodigoTotp`: o código precisa ter 6 dígitos e o `restante`
    ficar entre 1 e 30, senão é `RespostaInvalida`;
  - `salvar(anterior, rascunho)` devolve `Salvamento::{Salvo, Problemas}`.
- **`app/nucleo/src/daemon_falso.rs`** (só `modo-teste`):
  - responde `Revelar` e `Totp` (o `Totp` pelo `ward_core::totp`);
  - responde `SalvarEntrada` replicando o wardd: valida, confere no cofre, entrada nova
    entra como `ComAprovacao`, entrada editada mantém o nível.
- **`app/nucleo/src/formulario.rs` (novo):**
  - `Formulario` (`deny_unknown_fields`). Cada segredo chega como `vazio`, `manter`,
    `gerada` ou `{novo}`. `TextoSecreto` guarda o valor em `Zeroizing`, com `Debug`
    mascarado e `Deserialize` que toma posse da `String`.
  - `montar` confere os campos contra uma lista fixa por tipo. `manter` só vale na edição
    e só para um segredo (ou extra secreto) que a entrada editada tem.
  - A semente TOTP é normalizada: sem espaço, em maiúsculas, e lida de `otpauth://`.
    Fora de SHA1/6/30 a resposta é "parâmetros de TOTP não suportados" (D35).
  - Antes do pedido, `validar_local` roda a validação do `ward-core` numa cópia, para
    todos os erros aparecerem juntos. Os problemas que o daemon devolve passam por
    `erros_do_daemon`: o campo sai de uma tabela fixa e o motivo vira "inválido" se for
    longo ou tiver controle ou formatação Unicode.
- **`app/nucleo/src/servico.rs`:** métodos novos (`ao_sair` também descarta o
  formulário):
  - `revelar_campo` decide entre campo e extra secreto pelo detalhe; campo de texto dá
    `CampoInexistente`;
  - `codigo_totp` e `copiar_codigo_totp` (este pelo `Copiador`, D4);
  - `usar_senha_gerada(campo)`: só `senha`; move a senha do gerador para um mapa no
    Rust e devolve só o tamanho;
  - `descartar_formulario`;
  - `salvar_credencial(anterior, formulario)`: na edição, o tipo tem de ser o mesmo e a
    entrada não pode ter anexos (`ErroApp::AnexosNaoMantidos`). Devolve
    `{ salvo, erros }`, sem segredo.
- **`credenciais.rs`:** o detalhe agora traz `projeto` e `tags`, para a edição.
- **Comandos novos**, com a ACL de cada um:
  - `revelar_campo`, `codigo_totp`, `copiar_codigo_totp`, `usar_senha_gerada`,
    `descartar_formulario` e `salvar_credencial`;
  - registrados em `comandos.rs`, `capabilities/principal.json` (`allow-…`),
    `ponte/comandos.js`, no `generate_handler!` do Tauri e no despacho da ponte de teste.
- **Interface:**
  - componentes novos, um para cada componente da Design System: `TituloDeSecao`,
    `CampoExtra` (editável), `CodigoTotp`, `Interruptor`, `PopoverGerador`,
    `CartaoDeTipo` e `ComboboxGrupo` (ARIA combobox com "Criar grupo “x”");
  - `CampoSecreto` ganhou o modo editável (digitação em `type=password`, com olho local),
    `ontrocar` (clique na máscara para trocar o valor salvo), `mostrarRevelar`,
    `mostrarCopiar` e `erro`;
  - telas `Formulario.svelte` + `formulario.js` (campos por tipo, NFC, host da URL em
    punycode, montagem do pedido, erros por campo) e `EscolherTipo.svelte`;
  - `Credenciais.svelte`: "Nova credencial" abre Escolher tipo e depois o formulário;
    "Editar" abre a edição (fica desabilitado para os tipos da Fase 34 e para entradas
    com anexos). Ao salvar, recarrega a lista, abre a entrada e mostra o Toast
    "Credencial salva.";
  - o revelar do detalhe de Credenciais (D65 b) usa `revelar_campo`. O valor some em
    30 s, ao ocultar, ao trocar de entrada ou ao sair.
- **Gerador no formulário (D65 f):** tamanho de 8 a 64, força pelos limiares da 37 e
  símbolos de `ward_core::gerador::SIMBOLOS`, vindos do Rust. "Usar esta senha" guarda a
  senha no Rust; a tela mostra só a máscara com o tamanho e manda `"gerada"`.

## Segurança
- **Invariantes tocadas:**
  - **S11 / V5.3:** nenhuma resposta do formulário (salvar, TOTP, usar senha, detalhe)
    traz segredo do cofre. O valor só chega no `revelar_campo`, por clique, e sai em
    30 s, ao ocultar, ao trocar de entrada ou ao sair. Segredo salvo vai como "manter" e
    é posto no rascunho pelo Rust. A senha gerada nunca passa pelo JS, a não ser no
    revelar do popover.
  - **S1:** toda entrada dos comandos é tratada como não confiável.
    - `deny_unknown_fields` no formulário (um campo `nivel` forjado é recusado).
    - Limites de tamanho, tipo dentro da lista da fase, chaves de campo dentro da lista
      do tipo.
    - `manter` não puxa segredo de outra entrada: numa entrada nova é sempre recusado, e
      na edição só vale para o que a entrada editada tem.
    - A resposta do daemon também é não confiável: o problema é limpo.
  - **D4 / V5.5:** copiar (campo e código TOTP) passa pelo `Copiador`, com limpeza em
    30 s.
  - **D35 / V5.13:** `otpauth://` fora de SHA1/6/30 é recusado, a mensagem aparece no
    campo e nada é gravado.
  - **V5.14:** a senha do gerador não chega à interface ao preencher o campo.
  - **ACL:** os seis comandos só passam na janela `principal`, com a origem local.
- **Testes de ataque:**
  - `app/nucleo/src/formulario.rs`:
    - `ataque_otpauth_fora_de_sha1_6_30_e_recusado`
    - `ataque_problema_forjado_pelo_daemon_nao_vira_texto_livre`
    - `ataque_debug_do_texto_secreto_nao_mostra_o_valor`
    - `ataque_formulario_com_chave_desconhecida_e_recusado`
  - `app/nucleo/src/servico.rs`:
    - `ataque_formulario_forjado_pela_interface` (manter em entrada nova, extra de outra
      entrada, tipo trocado, campo de outro tipo, tipos da 34, anexos, nome anterior
      inválido)
    - `ataque_revelar_so_de_segredo_e_respostas_sem_segredo`
    - `ataque_otpauth_nao_suportado_nao_e_salvo`
    - `gerador_do_formulario_preenche_a_senha_sem_passar_pela_interface`
    - `salvar_sem_obrigatorio_mostra_erro_no_campo`
    - `salvar_novo_e_editar_mantendo_segredo`
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_formulario_pela_ponte`
  - `app/src-tauri/src/main.rs` › `ataque_formulario_so_na_janela_certa`
  - QA no Chrome (`docs/mission/evidence/fase-33/qa.json`):
    - nenhum segredo da fixture no DOM em 12 momentos (antes do revelar, depois de
      ocultar, nas edições, depois de gerar, depois de salvar);
    - `localStorage` e `sessionStorage` vazios;
    - URL sem token;
    - só pedidos locais.
- **Dependências novas:** nenhuma no repositório. O Playwright do QA roda fora do
  repositório, no scratchpad.

## Ficou de fora
- **Anexos:** "Adicionar arquivo" fica desabilitado. O protocolo não tem operação para
  ler o conteúdo de um anexo, então editar uma entrada com anexos apagaria os anexos
  (o `salvar_entrada` substitui a entrada inteira). Por isso essa edição é recusada.
- **Mudar nível** fica desabilitado (Fase 35). **Duplicar** e **Excluir** continuam sem
  ação.
- **Tipos banco, servidor e ftp/sftp** ficam desabilitados no Escolher tipo (Fase 34).
- **"Alterada em …"** no rodapé da edição fica vazio (datas na Fase 22d).
- No daemon real, `revelar` chega na 22c e o transporte do app na 23a. Hoje só o daemon
  falso responde.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (antes) | 0 |
| `cargo test -p ward-app-nucleo -p ward-ponte-teste --all-features` | 0 |
| `make app-front` | 0 |
| `make app-check` com `PKG_CONFIG_PATH` do scratchpad (R32); a 1ª rodada falhou no `cargo fmt --check`, corrigido | 2, depois 0 |
| ponte `--daemon-falso cheio` + Playwright (`qa33.py`, Chrome headless, 1280×800): todas as checagens `true` na 1ª rodada | 0 |
| `make check`: a 1ª rodada falhou em 2 lints do clippy em `formulario.rs`, corrigidos | 2, depois 0 |
| `make app-front` (final) | 0 |

## Problemas descobertos
- **Salvar no Novo vazio:** o design o desenha desabilitado, mas o V5.10 pede o erro no
  campo ao salvar. Deixei o botão clicável.
- **Estados de segredo que o design não desenha:**
  - Trocar um valor salvo: clicar na máscara abre a digitação. "Trocar" sem digitar
    nada salva o campo vazio; num campo obrigatório isso dá erro.
  - Senha vinda do gerador: só a máscara, sem revelar nem copiar até salvar.
- **Placeholder da URL:** o design usa "https://", mas o `dev/testes/app-dist.py` recusa
  qualquer `https://` no pacote. Em vez de afrouxar a checagem, troquei o texto por
  "Endereço com http ou https".
- **Escolher tipo:** o clique no cartão abre o formulário direto. O estado intermediário
  `Tipo escolhido` (18055) não aparece.
- **`Campo nota segura`** virou um `textarea` na tela (Novo) e o `Campo secreto`
  editável (Edição), não um componente próprio.
- **Rótulos do revelar:** agora são `Revelar <rótulo>` / `Ocultar <rótulo>` (antes
  `Revelar` / `Ocultar`). O roteiro de QA da Fase 37, fora do repositório, usava o
  seletor antigo.
- **Erros por campo:** ficam até o próximo salvar. O select de Ambiente mostra o erro
  embaixo, sem `aria-invalid`.
- **Console do QA:** dois 404, que parecem ser do favicon (não investiguei).
- **Valor digitado:** fica no estado da tela enquanto o formulário está aberto
  (inevitável) e sai ao salvar, cancelar ou sair da tela.
- **Rebase:** reapliquei as mudanças sobre o master atual (Fases 22a, 23, 24). As
  contagens do cofre de teste não estão fixadas nos testes novos.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/formulario.rs` | criado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/credenciais.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/gerador.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/lib/componentes/CampoSecreto.svelte` | alterado |
| `app/src/lib/componentes/CampoExtra.svelte` | criado |
| `app/src/lib/componentes/CartaoDeTipo.svelte` | criado |
| `app/src/lib/componentes/CodigoTotp.svelte` | criado |
| `app/src/lib/componentes/ComboboxGrupo.svelte` | criado |
| `app/src/lib/componentes/Interruptor.svelte` | criado |
| `app/src/lib/componentes/PopoverGerador.svelte` | criado |
| `app/src/lib/componentes/TituloDeSecao.svelte` | criado |
| `app/src/telas/Credenciais.svelte` | alterado |
| `app/src/telas/EscolherTipo.svelte` | criado |
| `app/src/telas/Formulario.svelte` | criado |
| `app/src/telas/formulario.js` | criado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-33/` (22 PNG + `qa.json`) | criado |
| `docs/mission/handoffs/phase-33.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; o original não foi tocado)
- [x] Ficou dentro do escopo da fase
