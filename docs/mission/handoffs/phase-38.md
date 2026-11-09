# Fase 38 — Configurações (UI)

## Implementado
- **Tela `Configurações / Geral`** (`app/src/telas/Configuracoes.svelte`), feita a partir
  do frame `15242` do FigCraft (ids em `docs/mission/design-nodes.md` › "Fase 38"). Ela
  substitui o "Esta tela ainda não existe." em `App.svelte`.
  - **Acesso:** "Senha mestra › Trocar" abre `TrocarSenha.svelte`, que pede a senha
    atual, a nova e a confirmação. A nova segue a regra do primeiro uso: no mínimo 12
    caracteres, com o mesmo medidor de força. "PIN de aprovação › Trocar" abre
    `TrocarPin.svelte`, que pede o PIN atual, o novo e a confirmação, com o mesmo
    `beforeinput` do primeiro uso. "Tempo de trava" é um `Select` compacto (15 min,
    30 min, 1 h, 2 h e 4 h, o padrão). Um valor gravado fora da lista também aparece.
    Ao mudar, o tempo é gravado pelo daemon.
  - **Backup:** "Backup local" e "Kit de emergência" aparecem com os botões
    desabilitados. Restaurar é da Fase 47 e o kit é do M7. "Backup externo em pendrive"
    mostra o estado lido do daemon (desligado) com o interruptor desabilitado. A nota
    "Backups sempre cifrados" é igual à do design.
  - Os toasts são "Tempo de trava salvo.", "PIN trocado." e "Senha mestra trocada.". O
    erro `travado` leva à tela de destravar. PIN errado consulta o estado, porque o 5º
    erro trava. Uma falha de gravação também confere o estado, porque o daemon trava em
    D53.
- **Componentes:** `LinhaDeOpcao` ganhou o snippet `controle`, para as variantes
  `Controle=botão` (10380) e `Controle=select` (10825). `Select` ganhou `compacto`
  (`largura/select-compacto`, rótulo só para leitor de tela) e `onmudar`. Os dois
  diálogos montam `Modal` + `CampoSenha`/`CampoPin` e reusam `primeiro-uso.js`
  (`erroDaSenha`, `pinValido`, `forcaDaSenha`, `MENSAGENS`). O `PainelDeMarca` não foi
  usado, porque a tela fica dentro da casca e não tem painel de marca no design.
- **Protocolo (`ward-core`):** operação nova `configuracao`, só para o app, sem campo.
  A resposta nova `configuracao` traz `{ minutos_trava, backup_externo }`, nunca o hash
  do PIN nem o UUID. A leitura recusa minutos fora de 1–1440 e campo a mais. Asserções
  de compilação garantem que `configuracao`, `trocar_pin`, `trocar_senha` e
  `tempo_trava` não aceitam o papel agente nem o extensão.
- **Daemon (`wardd`):** `Pedido::Configuracao` lê a configuração do `CofreAberto` (com o
  cofre travado, responde `travado`). `trocar_pin` (PIN atual pelo `conferir_pin`, com
  contagem e trava no 5º erro), `trocar_senha` (D52/D53) e `tempo_trava` já existiam
  (Fase 22) e gravam geração nova (D39). Não mexi neles.
- **Núcleo do app (`app/nucleo`):**
  - `configuracoes.rs` (novo): `TrocaPin { atual, novo, confirmacao }` e
    `TrocaSenha { atual, nova, confirmacao }`, os dois com `deny_unknown_fields` e
    `Digitado`, que zera e tem `Debug` mascarado. As regras são as de
    `primeiro_uso.rs`: `pin_valido`, `erros_da_senha` e a comparação em tempo
    constante, que passou a `pub(crate)`. A senha atual só precisa ter de 1 a
    `MAX_SEGREDO` bytes. Também tem `minutos_validos` (1–1440, D45), `Configuracoes` e
    `ResultadoTroca`.
  - `cliente.rs`: `configuracao`, `trocar_pin`, `trocar_senha` e `tempo_trava`.
  - `servico.rs`: `configuracoes`, `trocar_pin`, `trocar_senha_mestra` e
    `definir_tempo_trava`. Com erro de campo, nada vai ao daemon.
  - Comandos novos `configuracoes`, `trocar_pin { dados }`,
    `trocar_senha_mestra { dados }` e `definir_tempo_trava { minutos }`. Entraram em
    `comandos.rs`, `comandos.js`, na capability (`allow-*`), no `generate_handler!` e
    no despacho da ponte. Na ponte, os `dados` saem do mapa por `remove`, e campo a mais
    é recusado.
  - `daemon_falso`: atende `configuracao`, `tempo_trava`, `trocar_pin` (mesma regra e
    mesma contagem do `mudar_nivel`) e `trocar_senha` (compara em tempo constante; a
    nova passa a destravar).

## Segurança
- **Invariantes tocadas:**
  - **S9:** trocar o PIN exige o PIN atual, conferido no daemon. A contagem de erros é
    a mesma do `mudar_nivel`, e o 5º erro seguido trava. O PIN novo vira só o hash
    Argon2id (D30).
  - **S1/D5:** as operações de configuração são só do papel app. Isso é conferido na
    tabela, numa asserção de compilação e por teste.
  - **S11:** nenhuma resposta traz PIN, senha ou hash. Os valores digitados ficam no
    estado local do diálogo e saem dele antes da resposta, ao fechar (inclusive por Esc)
    e no `onDestroy`. Nada vai para `localStorage`, `sessionStorage`, URL ou estado
    global. O QA conferiu o HTML, os dois armazenamentos e a URL depois de cada passo.
  - **S7/D39:** toda mudança grava geração nova (testado no `wardd`).
  - **ACL:** os 4 comandos só passam na janela `principal` com a origem local.
- **Testes de ataque:**
  - `crates/wardd/tests/configuracoes.rs`:
    - `ataque_agente_e_extensao_nao_leem_nem_mudam_a_configuracao`
    - `ataque_configuracao_com_o_cofre_travado`
    - `ataque_resposta_configuracao_sem_hash_nem_pin`
    - `ataque_adivinhar_pin_pela_troca_trava_no_quinto_erro`
    - `v5_22_trocar_pin_pede_o_pin_atual`
    - `v5_22_trocar_senha_pede_a_atual_e_mantem_a_configuracao`
    - `v5_22_padrao_4h_desligado_e_tempo_alteravel_gravado_no_cofre` (geração +1; o
      valor sobrevive a travar e destravar; fora de 1–1440 é recusado na leitura)
  - `app/nucleo/src/configuracoes.rs`:
    - `ataque_pin_atual_ou_novo_fora_do_formato_e_recusado`
    - `ataque_senha_nova_curta_ou_diferente_e_atual_vazia_recusadas`
    - `ataque_erros_e_debug_nao_mostram_valor`
    - `ataque_campo_a_mais_ou_tipo_errado_e_recusado`
    - `ataque_minutos_fora_da_faixa`
    - `pelo_servico::ataque_pin_ou_senha_fora_da_regra_nao_chegam_ao_daemon` (espião:
      zero quadros)
    - `pelo_servico::ataque_tempo_fora_da_faixa_nem_chega_ao_daemon`
    - `pelo_servico::ataque_adivinhar_o_pin_pela_troca_trava_no_quinto_erro`
    - `pelo_servico::v5_22_*` (3 testes; o quadro do `trocar_pin` não leva a
      confirmação)
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_configuracoes_pela_ponte`
  - `app/src-tauri/src/main.rs` ›
    `ataque_configuracoes_so_na_janela_certa_e_conferidas_no_rust`
  - `ward-core` › `v3_15_tabela_operacao_papel` e a tabela de respostas, atualizadas
    com `configuracao`.
- **QA no Chrome** (`docs/mission/evidence/fase-38/qa.json`, 31 checagens, todas
  `true`; ponte com `--daemon-falso cheio`, Chrome headless, 1280×800, tema claro do
  sistema):
  - V5.22: padrão 4 horas; backup externo desligado e sem como mexer; tempo trocado
    para 30 min continua depois de sair e voltar à tela; trocar PIN pede o PIN atual
    (sem ele, nada vai ao daemon; errado mostra "PIN incorreto. Nada foi alterado." e
    esvazia os campos; certo troca); trocar senha pede a atual (sem ela, nada vai; nova
    curta é recusada; errada mostra "Senha mestra incorreta. Nada foi alterado."; certa
    troca).
  - Também: Esc fecha e apaga o digitado; nenhum valor no HTML, no armazenamento, na URL
    ou nas respostas; só pedidos locais; console sem erro.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Ligar o backup externo e o frame `16178`:** Fase 46. A tela já mostra "Ligado." se
  a configuração disser que está ligado, mas o interruptor fica desabilitado.
- **"Restaurar" (Fase 47) e "Imprimir kit" (M7):** os botões aparecem desabilitados. O
  "Último: hoje às 14:32" do design saiu da descrição do backup local: a operação
  `backups` existe, mas mostrar a data é da tela de restaurar.
- **Ponta a ponta com o `wardd` real pela tela:** os testes do `wardd` cobrem as
  operações. O QA na tela usou o daemon falso.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p wardd --test configuracoes` | 0 |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `cargo test -p ward-ponte-teste --all-features --test ataques configuracoes` | 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso cheio` + Playwright (`qa38.py`, scratchpad): a 1ª rodada só falhou no `console_sem_erro` (o 404 de `/favicon.ico`); o filtro passou a olhar a URL do 404, e a 2ª rodada deu tudo `true` | 1, depois 0 |
| `make ferramentas` | 0 |
| `make check`: a 1ª rodada falhou em `v3_15_tabela_operacao_papel`, porque faltava `configuracao` na tabela de respostas do teste. Corrigi, e a 2ª rodada saiu limpa | 2, depois 0 |
| `make app-check` com o `PKG_CONFIG_PATH` do scratchpad (R32): a 1ª rodada falhou em `ataque_comandos_proprios_so_na_janela_e_origem_certas` e `ataque_destravar_so_na_janela_certa_e_senha_sem_eco`, porque o `wardd` de teste de outro worktree estava destravado no socket fixo do uid (pendência da 35); o teste novo da fase passou. Com aquele daemon parado (não fui eu que o parei), a 2ª rodada saiu limpa | 2, depois 0 |
| Depois do `git merge master`: `make check` | 0 |
| Depois do merge: `make app-front` | 0 |
| Depois do merge: `make app-check` (R32) | 0 |
| Depois do merge: ponte `--daemon-falso cheio` + `qa38.py`, com todas as checagens `true` | 0 |

## Problemas descobertos
- **Design × contrato no PIN:** a descrição do frame `15242` diz "Trocar pede a senha
  mestra", mas V5.22 e o `wardd` (`trocar_pin { atual, novo }`) pedem o PIN atual. Segui
  o contrato e o daemon, e o texto da tela ficou "Trocar pede o PIN atual". O texto do
  FigCraft precisa ser ajustado numa fase de design, ou o planner decide o contrário.
- **"(máximo 4 h)" × D45:** o design limita a trava a 4 h, e D45 aceita de 1 a 1440 min.
  A tela oferece até 4 h, e o daemon aceita até 24 h, por exemplo de outro cliente. Um
  valor fora da lista aparece no select. Decidir se o máximo é 4 h também no
  `ward_core::limites`.
- **Sem frames para os diálogos:** "Trocar senha mestra" e "Trocar PIN" não têm frame no
  FigCraft. Montei os dois com `Modal`, `CampoSenha`/`CampoPin` e `Botão`. Vale
  desenhá-los numa fase de design.
- **Operação nova `configuracao` no protocolo:** a Fase 46, em paralelo, também mexe na
  configuração. No merge pode haver conflito nas tabelas `Operacao`/`TipoResposta`, em
  `testes.rs` e no `nucleo.rs`. A resposta tem só `minutos_trava` e `backup_externo`; se
  a 46 precisar do UUID, que seja num campo à parte, para app só.
- **Pendências da 30 que continuam abertas:** o mínimo de 12 caracteres da senha nova
  só é imposto no app; o `wardd` aceita senha de 1 byte no `trocar_senha`. Também não
  há NFC. Segui o primeiro uso, sem decisão nova.
- **Senha atual errada sem espera de D20:** `trocar_senha` no `wardd` não aplica a
  espera crescente de D20 à senha atual. É só papel app, mas um app comprometido
  poderia testar senhas mais rápido por aqui que pelo `destravar`. Decidir se a espera
  do papel app vale também na troca.
- **Teste do Tauri sensível ao socket fixo (pendência da 35, de novo):** com um `wardd`
  de teste de outro worktree no ar, dois testes de ACL do `app/src-tauri` falham. Com
  worktrees em paralelo, isso volta a acontecer; vale isolar o socket do teste.
- **Cópias no JS e no IPC:** as mesmas das Fases 30 e 31. O lado Rust zera a partir do
  `Digitado`.

## Merge com o master (Fases 46, 36, 27, 22d)
- **Protocolo:** `configuracao` ficou junto de `backup_externo`, `definir_backup_externo`
  e `pendrives` da 46, nessa ordem, nas tabelas `Operacao`/`TipoResposta`, em
  `pedido.rs`/`resposta.rs` e nas tabelas do V3.15 em `testes.rs`. Nenhum `ErroResposta`
  novo na 38, então o `SIMPLES` continua com o tamanho que o master deixou (29).
- **`backup_externo` × `configuracao`:** as duas respostas leem a mesma fonte no `wardd`
  (`a.cofre.configuracao().backup().ligado()`), então não divergem. A tela continua
  lendo pelo `configuracoes` (uma ida ao daemon só, sem UUID). O app ainda não tem
  comando para a operação `backup_externo` da 46, e criar um seria escopo da tela de
  ligar o backup externo. Se essa tela usar `backup_externo`, o campo `backup_externo`
  da resposta `configuracao` pode sair.
- **App:** os quatro comandos da 38 entraram em `COMANDOS_PRINCIPAL` (só a capability
  `principal`). `COMANDOS` agora é principal + aprovação, espelhado em `comandos.js` e no
  `generate_handler!`. No `daemon_falso`, os campos `minutos_trava`/`senha` convivem com
  `aprovacoes` da 36. `cheio()`/`sem_cofre()` seguem com `..vazio()`, e o
  `conferir_pin` é o mesmo para trocar PIN, mudar nível e responder aprovação.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/configuracoes.rs` | criado |
| `app/nucleo/src/configuracoes.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/primeiro_uso.rs` | alterado (`igual` → `pub(crate)`) |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/lib/componentes/LinhaDeOpcao.svelte` | alterado |
| `app/src/lib/componentes/Select.svelte` | alterado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/telas/Configuracoes.svelte` | criado |
| `app/src/telas/TrocarSenha.svelte` | criado |
| `app/src/telas/TrocarPin.svelte` | criado |
| `app/src/telas/configuracoes.js` | criado |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-38/*` (8 PNG + `qa.json`) | criado |
| `docs/mission/handoffs/phase-38.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia do `Ward.figcraft`; nada salvo)
- [x] Ficou dentro do escopo da fase
