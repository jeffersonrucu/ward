# Fase 20 — Protocolo em `ward-core`

## Implementado
- **`ward_core::protocolo`** (módulo novo, sem `unsafe`, sem dependência de Linux). Os detalhes estão no doc do topo de `protocolo.rs`.
  - **Quadro** (`protocolo::quadro`): 4 bytes de tamanho (u32 big-endian) seguidos do corpo JSON.
    - `limite_do_quadro(papel)`: 1 MiB para agente e extensão (`MAX_QUADRO_COMUM`, V3.4) e 8 MiB só para o app (`MAX_QUADRO_APP`, D5/D29). O papel vem da conexão, nunca do quadro.
    - O tamanho é conferido antes de ler o corpo. Tamanho 0 é recusado. O "negativo" (`0xFFFF_FFFF`) é recusado como grande demais.
    - O corpo é lido em blocos de 64 KiB que só são alocados quando os bytes chegam. Assim, um cliente que declara 1 MiB e fica calado não faz o daemon reservar memória. No fim os blocos viram um buffer `Zeroizing` de capacidade exata, e os blocos zeram.
    - `escrever_quadro` recusa corpo vazio ou acima do limite sem escrever nada.
    - `LIMITE_RESPOSTA` = `MAX_QUADRO_RESPOSTA` (2 × corpo + 1 KiB por entrada). É o que a CLI e o app leem do daemon.
  - **JSON estrito próprio** (`json.rs`, leitor e escritor):
    - o topo é sempre objeto;
    - números só inteiros sem sinal (negativo, fração, expoente, zero à esquerda e overflow são recusados);
    - escapes completos, com par substituto; substituto solto é recusado; controle cru é recusado;
    - chave repetida é recusada (em `Campos::de`; no topo, depois do papel);
    - limites: profundidade 8, 20 000 valores por pedido, 32 chaves por objeto.
    - Todo texto lido fica num `Zeroizing<String>` com a capacidade calculada antes, sem realocar. O escritor conta numa passada e escreve na outra, num `Zeroizing<Vec<u8>>` de capacidade exata.
  - **Mensagens versionadas**:
    - pedido: `{"v":1,"op":…,"dados":{…}}`;
    - resposta: `{"v":1,"tipo":…,"dados":{…}}`;
    - `v` diferente de 1 dá `versao_desconhecida`.
    - A leitura é estrita: um campo a mais no topo ou nos dados (`"revelar": true`, `"geracao"`, `"nivel"` no rascunho) recusa o pedido inteiro.
    - **Todo segredo vai em base64** (`base64ct`, tempo constante, recusa forma não canônica): senha mestra, PIN, segredos e extras do rascunho, anexos, token, `Resposta::Segredo` e preenchimento. Argv, cwd e ambiente do `ward run` também vão em base64, porque podem não ser UTF-8.
  - **Papéis e tabelas** (`Papel` = agente | app | extensão; `Operacao` e `TipoResposta`, montadas por macro, com `papeis()`):
    - **26 operações**:
      - de todos: `estado`;
      - de agente e app: `destravar`, `travar`, `lista`;
      - só do agente: `executar`, `ssh`, `login`, `playwright`, `entrada_padrao`, `janela`;
      - só do app: `criar_cofre`, `detalhe`, `revelar`, `copiar`, `totp`, `salvar_entrada`, `remover_entrada`, `mudar_nivel`, `responder_aprovacao`, `emitir_token`, `trocar_pin`, `trocar_senha`, `tempo_trava`;
      - só da extensão: `sugerir`, `preencher`, `salvar_login`.
    - **15 tipos de resposta**. Quatro levam credencial (`segredo`, `token`, `codigo_totp`, `preenchimento`) e são só de app ou extensão.
    - Uma asserção `const` garante que nenhum tipo com credencial vai ao agente e que `responder_aprovacao`, `mudar_nivel`, `revelar`, `copiar`, `emitir_token` e `preencher` não vêm dele.
  - **API**:
    - `decodificar_pedido(papel, bytes)`: a ordem é JSON → versão → operação conhecida → **papel** → resto do topo e campos. Uma operação de outro papel responde `nao_autorizado` com qualquer conteúdo de `dados`. O erro devolvido já é a `ErroResposta` a mandar.
    - `codificar_resposta(papel, &Resposta)` recusa o tipo que não é do papel (`NaoPermitidaAoPapel`).
    - `decodificar_resposta(papel, bytes)` recusa o tipo de outro papel: a CLI nunca aceita resposta com credencial.
    - `codificar_pedido(papel, &Pedido)` recusa operação de outro papel e o que o daemon recusaria por formato.
    - `erro_do_quadro`.
  - **Conferências de formato no pedido**:
    - PIN de 4 a 6 dígitos ASCII;
    - "negar" sem PIN, e os três "permitir" com PIN;
    - argv não vazio, sem NUL;
    - nome de variável sem `=` nem NUL;
    - minutos de trava na faixa de `limites`;
    - alvo do revelar é um campo secreto conhecido ou um extra;
    - `salvar_login` só aceita `login`;
    - rascunho sem nível (S9), sem campo de outro tipo, e com anexos somando até 256 KiB, contados durante a leitura (D29).
  - **Respostas sem segredo por construção**:
    - `ItemLista::de_entrada` traz nome, tipo, grupo, ambiente, projeto, tags, nível e URL/host;
    - `DetalheEntrada::de_entrada` traz os campos de texto, só os **nomes** dos segredos preenchidos, os extras secretos sem valor e o tamanho dos anexos.
    - A leitura recusa um detalhe forjado: extra secreto com valor, ou campo secreto em `campos`.
  - **Erros** (`ErroResposta`): 24 códigos simples mais `validacao`, cada um com mensagem fixa em português, sem número nem valor.
    - Na leitura, a mensagem do quadro é ignorada e volta a fixa do código.
    - D39: `retrocesso` ("o cofre no disco é mais antigo que o último aberto", igual a `ErroAbrir::Retrocesso`), `cofre_mudou` ("o arquivo do cofre mudou fora do Ward") e `geracao_esgotada` (igual a `ErroGravacao::GeracaoEsgotada`).
    - `From<&ErroAbrir>`, `From<&ErroGravacao>` e `de_validacao(&ErrosValidacao)`.
    - `entrada_inexistente` é um código só, para a bloqueada responder igual à que não existe (V3.17).
- **`limites`** (D32) ganhou `MAX_ENTRADA`, `MAX_TEXTOS_ENTRADA`, `MAX_QUADRO_COMUM`, `MAX_QUADRO_APP`, `MAX_QUADRO_RESPOSTA`, `MAX_ARGUMENTO` (128 KiB, o `MAX_ARG_STRLEN`), `MAX_ARGUMENTOS`, `MAX_VARIAVEIS_AMBIENTE` e `MAX_TEXTO_EXIBIDO`. Duas asserções de compilação conferem que a maior entrada em base64, mais os textos escapados, cabe em `MAX_QUADRO_APP`, e que o quadro comum é menor que 2 MiB.
- `lib.rs`: só a linha `pub mod protocolo;`.

## Segurança
- **Invariantes tocadas e como a fase as respeita:**
  - **S1 / V3.3**:
    - nenhum tipo de resposta com credencial é permitido ao agente, conferido na compilação, no teste da tabela e na escrita e leitura;
    - lista e detalhe não têm segredo por construção;
    - operação de app ou de extensão pedida pelo agente responde `nao_autorizado` antes de olhar os campos.
  - **S9 / S10 / V3.15**: `mudar_nivel` e `responder_aprovacao` são só do app, com PIN no formato certo. Os três "permitir" exigem PIN e "negar" não aceita PIN. O rascunho não tem nível. O módulo não tem feature, variável de ambiente nem argumento que mude a tabela (D8).
  - **S8**: todo quadro, texto lido, base64 decodificado e segredo vive em `Zeroizing` ou `Segredo`, com a capacidade exata (sem cópia por realocação). O alocador do V2.33 confirma.
  - **S13**: os erros não carregam posição, trecho do quadro, contagem (D20) nem geração (D39). O `Debug` de `Valor`, `Pedido` e `Resposta` não mostra segredo.
  - **D39**: nenhuma mensagem tem campo de geração. `"geracao"` vindo do cliente, no topo, nos dados ou no rascunho, é recusado.
  - **V3.4**: quadro de 2 MiB, tamanho negativo, tamanho 0, truncado, JSON inválido e operação desconhecida viram erro, sem pânico e sem ler o corpo recusado. As 10 000 conexões caladas são da Fase 22; o leitor só aloca o que chega.
- **Testes de ataque escritos:**
  - `crates/ward-core/src/protocolo/testes.rs`:
    - `v3_15_tabela_operacao_papel`: a tabela inteira de operações e respostas, como o auditor a lê.
    - `v3_15_sem_porta_dos_fundos_no_codigo`: sem `cfg(feature`, `cfg(debug_assertions`, `std::env`, `env!`, `option_env!` nem `modo-teste` no protocolo.
    - `v3_3_operacoes_de_app_e_extensao_do_agente_sao_nao_autorizadas`: cada par operação × papel não permitido recebe 6 quadros (válido, sem dados, `"revelar":true,"campo":"senha"` no topo e nos dados, dados malformados, `"geracao"`), e todos respondem `NaoAutorizado`.
    - `v3_3_campos_inventados_recusam_o_pedido`, `d39_geracao_nunca_vem_do_cliente_nem_vai_para_ele` e `s1_resposta_com_credencial_nunca_vai_ao_agente`.
    - `v3_3_lista_e_detalhe_do_cofre_de_teste_sem_segredo`: as 21 entradas de D15, sem nenhum segredo cru, em base64, escapado ou por linha (mais de 40 agulhas).
    - `debug_sem_segredo`, `leitura_da_resposta_e_estrita` (detalhe e TOTP forjados) e `erros_sem_numero_e_mensagem_fixa_na_leitura` (mensagem injetada pelo "servidor" é ignorada).
    - `v3_4_quadros_recusados_antes_de_ler_o_corpo`: 2 MiB, `MAX+1`, `-1`, `-2` e 0, com um leitor que prova que só os 4 bytes do cabeçalho foram lidos.
    - `v3_4_quadro_truncado_e_fechado` e `v3_4_json_invalido_e_operacao_desconhecida_respondem_erro`.
    - `envelope_recusado`, `json_estrito` (31 recusas), `json_limites_de_profundidade_nos_e_chaves` (100 000 níveis não estouram a pilha) e `base64_estrito`.
    - `pin_e_decisao_da_aprovacao`, `executar_recusa_nul_igual_e_vazio`, `revelar_exige_um_alvo_conhecido` e `s9_rascunho_sem_nivel_e_sem_campo_de_outro_tipo`.
    - `d29_anexos_ate_256_kib_somados_pelo_app`: no limite passa e valida; com 1 byte a mais o cliente não codifica e o quadro forjado é recusado; pelo agente é `NaoAutorizado`.
    - `maior_entrada_cabe_no_quadro_do_app`: nota de 1 MiB de `"`, 64 extras de 64 KiB e 256 KiB de anexos cabem em 8 MiB e validam; o mesmo quadro é recusado pelo limite do agente e da extensão.
    - `d39_erros_do_destravar_e_das_gravacoes` e `entrada_inexistente_tem_um_quadro_so`.
    - `fuzz_decodificador`: 100 000 mutações (SplitMix com semente fixa, 4 threads, tokens hostis) de um quadro válido de cada operação e de cada tipo, por papel aleatório. Nada entra em pânico; o que é aceito respeita o papel, nunca é credencial para o agente e, escrito de novo, é estável.
    - `fuzz_quadro_e_json_aleatorios`: 20 000 entradas aleatórias.
  - `crates/ward-core/tests/corpo_memoria.rs` › `s8_protocolo_nenhum_bloco_liberado_tem_segredo`:
    - percorre destravar e salvar (codificar → `escrever_quadro` → `ler_quadro` → decodificar → `Rascunho::validar`), `Resposta::Segredo` ao app e as recusas (papel errado, anexo acima de D29);
    - o resultado é zero marca em bloco liberado; a marca extra é o base64 da senha mestra, como vai no quadro;
    - um controle confere que o detector vê o quadro;
    - testei uma mutação à mão (uma cópia `String` do texto lido): o teste falhou como esperado; depois desfiz a mutação.
- **Dependência nova:** `base64ct` 1.8.3 (`default-features = false`), do RustCrypto. Motivo: segredos no quadro em base64 de tempo constante e estrito, decodificados num buffer que zera. Já estava no `Cargo.lock` como dependência de `password-hash`/`phc`; o lock só ganhou a linha `"base64ct"` no `ward-core`. `cargo audit` e `cargo deny` estão limpos. O JSON é próprio porque o `serde_json` não zera os buffers internos de escape nem as `String` que monta (S8). Não é cripto, então a regra "nada feito à mão" não se aplica.

## Ficou de fora
- **Fluxo daemon → extensão do `ward login`** (o daemon pede à extensão que preencha a aba do agente e recebe o resultado): fica para as Fases 40/41. Elas devem acrescentar a operação e a resposta à tabela, com teste do papel. Hoje a extensão tem `sugerir`, `preencher` e `salvar_login`.
- **Backup e restauração** (Fase 45, D17): sem operação ainda. Restaurar será só do app.
- **Sub-protocolo de sessão do `run`/`ssh`** (fim da entrada padrão, sinal, pty): defini o mínimo (`entrada_padrao` com vazio = fim, `janela`, `saida`, `fim`). As Fases 26/27 podem refinar mantendo as operações só do agente.
- **Regra de negócio**: conferir o PIN contra o hash, filtrar as bloqueadas da `lista` do agente, existência da entrada, origem do D9, prazo do token e espera crescente (D20) ficam no daemon (Fases 22, 23, 27, 40). O protocolo dá os tipos e os erros.
- **Conexões simultâneas, tempo limite de leitura e `Ocupado`**: Fase 22 (o código `ocupado` já existe).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 600 make ferramentas` | 0 |
| `cargo clippy -p ward-core -- -D warnings` (durante) | 0 |
| `timeout 1200 cargo test -p ward-core --lib protocolo` (1.ª vez) | 101 (duplicata de chave antes do papel; corrigido) |
| `timeout 1200 cargo test -p ward-core --lib protocolo` | 0 |
| `timeout 1200 cargo test -p ward-core --test corpo_memoria s8_protocolo` | 0 |
| idem com mutação de controle (cópia `String` no leitor) | 101 (esperado; desfeito) |
| `cargo fmt --all` | 0 |
| `timeout 2400 make check` (1.ª vez) | 2 (clippy: `duplicate_mod`, `format!` aninhado, `as_bytes`; corrigido) |
| `timeout 3000 make check` | 0 |

## Problemas descobertos
- **Lista de operações e respostas criada aqui.** O escopo e o PRD citam as operações (D3, D4, D5, D16, D21), mas sem nomes nem campos. Escolhi os nomes da tabela acima. Três decisões minhas, que o planner deve confirmar:
  - `remover_entrada` não pede PIN; o escopo só exige PIN para nível e aprovação;
  - `tempo_trava` não pede PIN;
  - a `lista` do agente leva o nível de cada entrada, porque ajuda o Claude a saber se vai haver aprovação.
- **Dados do cliente que o app mostra**: `PedidoDeAprovacao.comando` e `.cadeia` são texto montado pelo daemon a partir de dados do agente. O transporte é seguro, porque o JSON escapa tudo. Mas a janela de aprovação (Fases 23/36) precisa neutralizar bidi e caracteres de controle ao exibir, senão o agente forja o comando mostrado. Nada disso é validado aqui, porque `comando` precisa ser o comando exato.
- **Mapeamento de `ErroGravacao::Formato`**: em `gravar_entradas`, `Formato` é falha ao selar, e o `From` devolve `falha_ao_gravar`. Em `trocar_senha`, `Formato` também significa senha atual errada. A Fase 22 deve responder `senha_incorreta` nesse caso (está no doc do `From`).
- **Resíduo de 3 bytes na pilha**: no `Base64::decode`, o `base64ct` usa um `tmp_out` de 3 bytes na pilha que não zera, no último bloco. É pouco e fica dentro da biblioteca auditada. Registro só para o auditor.
- **`corpo_memoria.rs` ganhou o teste S8 do protocolo.** É o único binário com alocador de teste (D31). Pus o teste nele em vez de criar outro arquivo com `unsafe`. O nome do arquivo ficou menos preciso.
- **`MAX_QUADRO_COMUM` = 1 MiB** limita o `ward run` a argv + ambiente de cerca de 750 KiB depois do base64. O Linux aceita até cerca de 2 MiB de `ARG_MAX`. Um ambiente gigante seria recusado com `quadro_invalido`; a Fase 26 deve dar uma mensagem clara na CLI.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | criado |
| `crates/ward-core/src/protocolo/json.rs` | criado |
| `crates/ward-core/src/protocolo/quadro.rs` | criado |
| `crates/ward-core/src/protocolo/campos.rs` | criado |
| `crates/ward-core/src/protocolo/pedido.rs` | criado |
| `crates/ward-core/src/protocolo/resposta.rs` | criado |
| `crates/ward-core/src/protocolo/erros.rs` | criado |
| `crates/ward-core/src/protocolo/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod protocolo;`) |
| `crates/ward-core/src/limites.rs` | alterado (limites do protocolo) |
| `crates/ward-core/Cargo.toml` | alterado (`base64ct`) |
| `Cargo.lock` | alterado (só a linha `"base64ct"` do `ward-core`) |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado (teste S8 do protocolo) |
| `docs/mission/handoffs/phase-20.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase (`wardd`, CLI e plataforma intocados)
