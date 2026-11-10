# Fase 37 — Gerador de senha avulso (UI)

## Implementado
- **Escolha do caminho (pedido do orchestrator):** o protocolo do daemon não tem
  operação de gerar senha (só erros e operações do cofre em `ward_core::protocolo`). Por
  isso a senha é gerada no `app/nucleo`, em Rust, por `ward_core::gerador::gerar` (RNG do
  sistema, sem viés, símbolos de D35). Nunca no JavaScript. O "copiar" usa o mesmo
  `Copiador` da Fase 29 (D4, S12).
- **`app/nucleo/src/gerador.rs` (novo):**
  - `Gerador` guarda a senha atual num `Segredo` (Zeroizing, sem `Clone`/`Display`).
  - `Opcoes` vem da interface e é validado: tamanho 8–64 (o que o design mostra, dentro
    dos 4–128 do núcleo), pelo menos um conjunto. `u64` enorme e tamanho fora da faixa
    dão `PedidoInvalido`.
  - `Gerada` (a resposta do "gerar") tem só `tamanho`, `bits`, `forca` e `simbolos`
    (`ward_core::gerador::SIMBOLOS`, uma fonte só). Nenhum caractere da senha.
  - Força pela entropia das opções (`tamanho × log2(alfabeto)`, de 10 em 10 para baixo):
    `< 60` fraca, `< 80` razoável, `< 128` forte, senão muito forte. Os 24 caracteres
    com os quatro conjuntos dão "muito forte · cerca de 150 bits", como no design.
  - `Revelada` (Zeroizing, `Debug` mascarado) só para o "revelar".
  - Ao gerar, a senha anterior zera antes de chamar o RNG. Se o RNG falhar, não sobra
    senha velha.
- **`servico.rs`:** `gerar_senha`, `copiar_senha_gerada` (resposta `()`, via
  `Copiador`), `revelar_senha_gerada` e `descartar_senha_gerada`. `ao_sair` também zera a
  senha avulsa. Um lock envenenado descarta a senha.
- **`erro.rs`:** `AleatorioIndisponivel` e `SemSenhaGerada`, com mensagem fixa.
- **Comandos novos** em `comandos::COMANDOS`, `capabilities/principal.json`
  (`allow-gerar-senha`, `allow-copiar-senha-gerada`, `allow-revelar-senha-gerada`,
  `allow-descartar-senha-gerada`), `app/src/lib/ponte/comandos.js`, no
  `generate_handler!` do Tauri e no despacho da ponte de teste.
- **Interface:**
  - `app/src/telas/GeradorSenha.svelte`, a tela `Gerador de senha / Avulso` (13141):
    - tamanho por `<input type=range>` (8–64, padrão 24);
    - quatro interruptores; o último ligado fica desabilitado;
    - máscara com um ponto por caractere;
    - revelar por 30 s (some ao esconder, gerar de novo ou sair);
    - copiar, gerar de novo e força com quatro segmentos;
    - nota "Não fica no cofre" e rodapé;
    - pedidos de gerar em série, para a senha guardada no Rust ser sempre a das opções
      na tela;
    - ao sair da tela, `descartar_senha_gerada`.
  - Componentes novos, só com tokens:
    - `NavegacaoLateral.svelte` (782/3037);
    - `LinhaDeOpcao.svelte`, com `checkbox role=switch` (10379/10368, `Interruptor` 6838);
    - `ControleDeslizante.svelte` (12604). O preenchido vai por
      `style.setProperty('--progresso')` (CSSOM, que a CSP permite), sem atributo `style`.
  - `CampoSecreto.svelte`: prop `mascara` (padrão igual ao de antes). O valor revelado
    aparece sem ligaduras (ver Problemas).
  - `App.svelte`: com o cofre destravado, mostra a casca (navegação + tela). Fora disso,
    o esqueleto da Fase 29.
  - `ponte-teste.js` (só no build de teste): a URL de abertura aceita
    `#token=…&tela=gerador`, para o QA no Chrome antes das telas de destravar. A tela
    sai da URL junto com o token. No build de produção, o ramo some (`app-dist.py` ok).
- **`docs/mission/design-nodes.md`:** seção da Fase 37.

## Segurança
- **Invariantes tocadas:**
  - **S11:**
    - a senha nasce e fica no Rust;
    - "gerar" e "copiar" não devolvem o valor; só "revelar", no clique, o leva à
      webview, e a tela o tira em 30 s, ao esconder, ao gerar de novo ou ao sair;
    - nada em armazenamento do navegador, URL ou estado global (QA: `localStorage`,
      `sessionStorage` e cookie vazios; nenhum pedido fora da origem);
    - capability só com os 4 comandos novos, só na janela `principal` e com origem
      local.
  - **S12/V5.5:** o copiar do gerador passa pelo mesmo `Copiador` (limpeza em 30 s só
    se a área ainda tiver o valor).
  - **S6:** RNG só pelo `ward_core::gerador` (`getrandom`); nada de aleatório no JS.
  - **S8/S13:** `Segredo` e `Revelada` zeram ao sair, sem `Debug` do valor. Erros com
    mensagem fixa.
  - **D35:** a tela mostra o conjunto que vem de `ward_core::gerador::SIMBOLOS`, sem
    espaço, aspas, crase e `\`.
- **Testes de ataque escritos:**
  - `app/nucleo/src/gerador.rs`:
    - `ataque_opcoes_fora_do_formato_sao_recusadas` (0, 4, 7, 65, 128, 129, 2³², `u64::MAX`,
      nenhum conjunto; nada fica guardado);
    - `ataque_gerar_nao_devolve_o_valor` (nem no `Debug`, nem um trecho de 6
      caracteres);
    - `simbolos_sao_os_de_d35`, `forca_do_design`, `senha_respeita_tamanho_e_conjuntos`,
      `descartar_zera_e_gerar_de_novo_troca`, `limites_da_tela_dentro_do_nucleo`.
  - `app/nucleo/src/servico.rs`:
    - `ataque_gerador_copia_sem_devolver_o_valor` (resposta `()`, área com o valor,
      descartar e `ao_sair` zeram);
    - `ataque_limpeza_do_gerador_nao_apaga_texto_do_usuario` (V5.5 com área falsa).
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_gerador_avulso_pela_ponte`: a
    resposta do gerar é exatamente `{tamanho, bits, forca, simbolos}`. Recusa tamanho 7,
    65, 128, -1, 24.5, `"24"`, booleano como texto, campo faltando e nenhum conjunto.
  - `app/src-tauri/src/main.rs` › `ataque_gerador_so_na_janela_certa_e_sem_valor_no_gerar`:
    - pela IPC do Tauri (runtime falso e capability real), as chaves da resposta são só
      as quatro e não contêm o valor;
    - recusa tamanho fora e tipo errado;
    - os quatro comandos são negados em outra janela e com origem remota.
  - `app/nucleo/tests/configuracao_tauri.rs` › `limites_do_gerador_na_interface_sao_os_do_rust`,
    mais os testes de lista/capability da Fase 29, que agora cobrem os 4 comandos.
  - QA no Chrome headless (Playwright em Python, fora do repositório), contra a ponte,
    em 1280×800:
    - símbolos mostrados iguais aos de D35;
    - máscara de 24 pontos, e o valor fora do DOM antes do revelar;
    - revelado com 24 caracteres do alfabeto, e fora do DOM depois de esconder;
    - gerar de novo troca a senha;
    - sem símbolos, a senha é alfanumérica;
    - pelo teclado: 12 → "razoável · cerca de 70 bits", Home 8, End 64;
    - o último conjunto não desliga;
    - armazenamento vazio, nenhum pedido fora da origem, token e tela fora da URL.
- **Dependências novas:** nenhuma (Rust e npm).

## Ficou de fora
- **V5.5 de ponta a ponta com `wl-paste` real:** não cliquei em "copiar" na ponte, para
  não sobrescrever a área de transferência da sessão do usuário. A lógica está coberta
  com área falsa (os mesmos testes da Fase 29, mais os dois do gerador). Fica para o QA
  do milestone.
- **"Travar agora" na navegação:** o botão está desenhado, mas não há comando de
  travar. Fica para as Fases 31 e 39.
- **Itens "Credenciais" e "Configurações":** trocam a tela, que mostra "Esta tela ainda
  não existe." até as Fases 32 e 38.
- **Gerador no formulário (`Gerador de senha / No formulário`, popover 11671):** é da
  Fase 33.
- **App real (Tauri) chegando à tela:** a casca só aparece com o cofre destravado, e o
  `estado_do_cofre` ainda responde `sem-protocolo` (Fases 20–22). No Chrome, a tela
  abre pela URL de teste.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `cargo test -p ward-app-nucleo` (1ª: 2 falhas esperadas, capability e `comandos.js` ainda sem os comandos novos) | 101, 0 |
| `cargo test -p ward-ponte-teste --features modo-teste --test ataques` | 0 |
| `cargo test --all-features --locked` em `app/src-tauri` (`PKG_CONFIG_PATH` do scratchpad, R32) | 0 |
| `make app-front` | 0 |
| `make app-ponte` + Playwright (Python, Chrome headless): a 1ª rodada achou o último conjunto desligável e a ligadura `<=>`, ambos corrigidos; a 2ª parou no clique forçado do QA | 0, 1, 0 |
| `make check` | 0 |
| `make app-check` (`PKG_CONFIG_PATH` do scratchpad, R32) | 0 (avisos do `cargo deny` do app iguais aos da Fase 29: duplicados e `advisory-not-detected`) |

## Problemas descobertos
- **Ligaduras escondem caracteres (corrigido, vale para a Fase 33).** A Inter junta `<=>`
  numa seta (⇔), e a JetBrains Mono do valor revelado tem ligaduras de código (`<=`,
  `->`, `!=`). O usuário leria uma senha diferente da copiada. Desliguei `liga` e `calt`
  no valor revelado do `Campo secreto` e na lista de símbolos. Outras telas que mostram
  valor ou símbolo devem fazer o mesmo.
- **Texto do design diverge de D35/V5.21:** a descrição de "Símbolos" no FigCraft é
  `! @ # $ % & * ( ) - _ = + ?`, só parte do conjunto. Segui o contrato e mostro o
  conjunto inteiro de D35, que vem do Rust. A descrição no design precisa ser atualizada
  numa fase de design. Não escolhi lado no arquivo.
- **Faixa de tamanho:** o design diz "De 8 a 64", e o núcleo aceita 4–128 (handoff 18). A
  tela e o comando do app aceitam 8–64. O gerador do formulário (Fase 33) deve decidir a
  própria faixa. Sugiro registrar a de 8–64 no PRD.
- **Limiares de força são escolha minha:** o design só mostra "muito forte". Usei
  fraca < 60 bits, razoável < 80, forte < 128, muito forte ≥ 128, com as cores de perigo,
  aviso e sucesso. Precisa de aceite, e a Fase 33 deve usar os mesmos.
- **O valor revelado sai do tipo que zera:** a serialização da IPC (serde) e a string do
  JavaScript são cópias que não zeram. É o custo do "revelar" que S11 permite, e a Fase
  33 terá o mesmo.
- **Parâmetro de tela na URL de teste:** `&tela=<nome>` só no build de teste, para o QA
  abrir telas antes de destravar existir. As Fases 30–32 podem trocar isso pelo fluxo
  real.
- **Conflitos prováveis com a Fase 32 (em paralelo):** `App.svelte`, a lista de comandos
  (`comandos.rs`, `comandos.js`, capability, `generate_handler!`, despacho da ponte) e
  talvez uma navegação lateral própria. Na hora do merge, manter os comandos dos dois
  lados e um só `NavegacaoLateral.svelte`.
- **404 no console da ponte:** um recurso que o Chrome pede sozinho (provavelmente
  `/favicon.ico`). Não aparece nas respostas da página. Já existia na Fase 29.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/gerador.rs` | criado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/tests/configuracao_tauri.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `app/src/App.svelte` | alterado |
| `app/src/telas/GeradorSenha.svelte` | criado |
| `app/src/lib/componentes/NavegacaoLateral.svelte` | criado |
| `app/src/lib/componentes/LinhaDeOpcao.svelte` | criado |
| `app/src/lib/componentes/ControleDeslizante.svelte` | criado |
| `app/src/lib/componentes/CampoSecreto.svelte` | alterado (`mascara`, sem ligaduras) |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/lib/ponte/ponte-teste.js` | alterado (`&tela=` só no teste) |
| `docs/mission/design-nodes.md` | alterado (seção da Fase 37) |
| `docs/mission/evidence/fase-37/*.png` (5) | criado |
| `docs/mission/handoffs/phase-37.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks), mais `make app-front` e `make app-check`
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, área de transferência do usuário não tocada)
- [x] Teste de ataque para cada (SEG) tocado (S11 e V5.21; V5.5 na lógica)
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia; original intacto, `cmp` igual) em 1280×800
- [x] Ficou dentro do escopo da fase
