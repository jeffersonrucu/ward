# Fase 19a — Limites e validação do núcleo (D29, D31–D34)

## Implementado
- **`ward_core::limites`** (novo, `crates/ward-core/src/limites.rs`). Todos os limites são constantes `pub` (D32):
  - texto: nome, linha, URL, host, tag, nome de extra e de anexo, access key, região e base (63, e 99999 no redis);
  - segredos: 64 KiB por segredo e por valor de extra, 1 MiB na nota, TOTP de 16 a 128;
  - listas: 16 tags, 64 extras, 32 anexos;
  - D29: `MAX_ANEXOS_SOMADOS` = 256 KiB, `LIMITE_PADROES` = 8 MiB, `ORCAMENTO_MASCARA` = `LIMITE_PADROES / 2`, `MINIMO_CONFIAVEL` = 6;
  - corpo: `MAX_CORPO` = 64 MiB e `MAX_ENTRADAS` = 10 000.

  Um `const _` confere na compilação a coerência entre os limites (todo texto ≤ `MAX_TEXTO`, orçamento × 2 ≤ limite, `MAX_CORPO` cabe em u32).
- **Uma fonte só.** `entrada/validacao.rs`, `corpo.rs` e `mascara.rs` não definem mais nenhum desses números.
  - `corpo::MAX_CORPO`/`MAX_ENTRADAS` e `mascara::LIMITE_PADROES`/`MINIMO_CONFIAVEL`/`ORCAMENTO_MASCARA` continuam acessíveis pelo nome antigo, como `pub use` (não são definições novas).
  - O `corpo.rs` só guarda `MAX_DADOS = 7`, que é estrutural (quantos campos um tipo tem).
- **Anexos (D29):**
  - na validação, cada anexo vai até 256 KiB (`longo demais`) e a soma também (nova mensagem: "os anexos somam mais de 256 KiB"). O anexo vazio continua recusado;
  - na 1ª passada do corpo, um `ler_anexos` novo confere a soma por entrada antes de alocar.
- **Orçamento da máscara (D29):**
  - `mascara::custo_da_entrada(&Entrada) -> Result<usize, ErroMascara>` mede a entrada e a entrega dela.
    - A entrega é montada com recursos de mentira que não são segredo: código TOTP `000000`, caminho fixo e porta de túnel 65535. Essa porta é a mais longa, então o `DATABASE_URL` medido nunca fica menor que o real.
    - A soma usa o mesmo `visitar`/`Gerador` de `construir`, pela `somar` nova, que os dois compartilham.
    - Para no primeiro byte acima do limite.
    - A `nota segura` custa 0, porque não tem entrega.
  - No fim de `Rascunho::validar`, logo também na leitura do corpo, uma entrada acima de 4 MiB é recusada com campo `entrada` e a mensagem de D29, "entrada grande demais para a máscara do `ward run`". A constante é `pub` como `entrada::GRANDE_PARA_MASCARA`, para o app.
  - Variante nova `ErroMascara::SemEntrega` (defesa em profundidade, com mensagem própria).
- **Base do `banco` (D33):**
  - fora do redis: `[A-Za-z0-9_-]{1,63}`;
  - no redis: número de 0 a 99999, só dígitos e sem zero à esquerda;
  - mensagens em português, sem o valor.

  A entrega não mudou: o percent-encoding fica como defesa extra.
- **`CHAVE=valor` (D34):** em `Gerador::anexo`, o valor (com e sem aspas, cada um por si) só vira padrão próprio com 6 caracteres ou mais. A linha inteira continua mascarada.
- **Alocador de teste (D31)**, em `tests/corpo_memoria.rs`:
  - `alloc` usa `System.alloc_zeroed`;
  - o `realloc` próprio saiu, e a implementação padrão do `GlobalAlloc` faz bloco novo por `alloc` (zerado e contado), copia e libera o antigo por `dealloc` (procurado);
  - assim a procura nunca lê byte não inicializado;
  - os comentários `// SAFETY:` foram atualizados, e o doc do módulo registra a exceção de D31.
- **Testes ajustados ao V2.30 novo:**
  - `um_de_cada_cheio()` (sem parâmetro): cada entrada leva dois anexos com bytes de 0 a 255 que somam exatamente 256 KiB;
  - a `api` com token de 64 KiB virou uma 10ª entrada (`zz-cheio-api-token`), só com o anexo pequeno. Ver "Problemas";
  - o teste de disco virou `v2_30_ida_e_volta_pelo_disco_com_anexos_no_limite`;
  - o teste do anexo vazio virou `v2_30_anexo_vazio_recusado_ao_salvar_e_no_corpo`;
  - V2.32: "codificador recusa acima de 64 MiB" agora usa 13 `nota segura` cheias (~5,3 MiB cada), porque com 256 KiB de anexos uma entrada não chega perto do limite.
- **Testes de outras fases que dependiam das regras antigas:**
  - `entrega/testes.rs`: o V2.15 do redis ficou sem base, e o ataque de URL agora confere que a base hostil é recusada, testando usuário e senha hostis com base válida;
  - `entrada/testes.rs`: dois testes trocavam o motor para redis mantendo a base `app_teste`;
  - `mascara/testes.rs`: o teste "falha fechado acima de 8 MiB" agora junta várias entradas, cada uma dentro do orçamento.

## Segurança
- **Invariantes tocadas:**
  - **S4 / V2.36:** nenhuma entrada aceita fica sem `ward run`.
    - Cada entrada com entrega custa até 4 MiB pela mesma função do `Gerador`, e o custo medido é limite superior do real (o teste confere `real <= custo`).
    - O par de D19 (banco + servidor de túnel, servidor + salto) cabe nos 8 MiB.
    - A máscara continua falhando fechado acima de 8 MiB.
  - **S4 / V2.38:** só o valor curto de `CHAVE=valor` deixa de ser padrão, como RA3 aceita. Segredo de campo de qualquer tamanho segue mascarado.
  - **S8, S13 / V2.32:** a soma dos anexos é conferida na 1ª passada, sem alocar. Os erros não levam valor.
  - **S8 / V2.33:** o custo monta a entrega e os padrões só em tipos que zeram (`Zeroizing`, `reservar`). O teste do alocador segue com zero achados no caminho todo, inclusive na validação.
  - **D31 / V2.40:** `unsafe` fora do `ward-plataforma` só aparece em `tests/corpo_memoria.rs`. Os outros crates só citam `forbid(unsafe_code)`.
  - **S6/S13 (D33):** a base com `?`, `#`, `/`, `.`, espaço ou `%` não chega à entrega.
- **Testes de ataque escritos:**
  - `limites/testes.rs`:
    - `v2_35_validacao_e_corpo_no_mesmo_limite_de_anexos`;
    - `v2_35_orcamento_e_o_mesmo_na_validacao_e_na_mascara`;
    - `v2_36_maior_entrada_de_cada_tipo_monta_a_mascara` (busca binária da maior entrada aceita, com segredos que exercitam escape JSON e percent-encoding);
    - `v2_36_par_banco_e_servidor_de_tunel_monta_a_mascara`;
    - `v2_36_custo_recusa_acima_do_orcamento_e_nota_fica_fora`.
  - `entrada/testes.rs`:
    - `v2_36_anexos_somados_e_orcamento_ao_salvar`;
    - `v2_37_base_do_banco`;
    - `mensagens_citam_os_limites_atuais`: se um limite mudar, a mensagem que o cita reprova.
  - `entrega/testes.rs`:
    - `v2_37_base_valida_sai_igual` (`PGDATABASE=acme_db-1` e `REDIS_URL` com `:6379/3`);
    - `ataque_senha_usuario_ou_base_mudam_o_destino_do_url` (ajustado).
  - `mascara/testes.rs`:
    - `v2_38_valor_curto_de_chave_valor_nao_vira_padrao`;
    - `v2_38_mutacao_sem_minimo_mascararia_o_valor_curto` (controle);
    - `ataque_anexo_grande_demais_falha_fechado` (ajustado).
  - `corpo/testes.rs`:
    - `v2_36_corpo_forjado_com_entrada_grande_demais`: anexo de 256 KiB + 1, dois de 200 KiB e `api` com 64 extras de 64 KiB, mais o controle no limite.
  - `tests/corpo_memoria.rs`:
    - forjados novos: anexos que somam 256 KiB + 1, e um anexo de 256 KiB + 1 com os bytes de verdade no buffer, os dois recusados com zero alocações;
    - `v2_32_codificador_recusa_acima_do_limite_antes_de_alocar` (refeito).
- **Mutações feitas numa cópia do workspace** (`scratchpad/mut`), todas pegas:
  - **V2.35, "cópia de trabalho":** `MAX_ANEXOS_SOMADOS` = 128 KiB. Os testes de fronteira presos à constante passaram no limite novo, inclusive validação, corpo, orçamento e todos os do `corpo_memoria`. Só reprovaram os dois que citam o número literal: a mensagem "256 KiB" e o caso de 200 KiB.
  - Sem o mínimo de D34: V2.38 reprovou.
  - Orçamento desligado na validação: 6 testes de V2.35 e V2.36 reprovaram.
  - Soma de anexos desligada no corpo: o V2.32 reprovou (alocou).
  - `base_valida` sempre verdadeira: V2.37, o ataque de URL e as mensagens reprovaram.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Corpo v2 com a seção de configuração (D30):** é da Fase 19b.
- **O aviso do app ao escolher o arquivo e a recusa do orçamento mostrada no campo (D29):** são da Fase 34, que usa `limites::MAX_ANEXOS_SOMADOS` e `entrada::GRANDE_PARA_MASCARA`.
- **O dimensionamento do protocolo:** é da Fase 20.
- **O exemplo `examples/dev-cofre` da Fase 19:** é da 19, então não mexi nele. Ver "Problemas".

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked` (`CARGO_TARGET_DIR` no scratchpad), 1ª rodada | 101 (5 testes: base `app_teste` em redis, `um_de_cada_cheio` acima do orçamento) |
| `cargo test -p ward-core --locked` depois das correções | 0 |
| 5 mutações na cópia do workspace | 101 cada (pegas) |
| `make check`, 1ª e 2ª vez | 2 (fmt dos meus testes; depois `clippy::bool_assert_comparison` num teste meu) |
| `make check` | 0 (160 + 4 + 16 testes, audit, deny, gitleaks limpos) |

## Problemas descobertos
- **V2.30 × D25 × D29:** "uma de cada tipo com … anexos que somam 256 KiB … dentro do orçamento" não fecha para a `api` com o token no limite de D25 (64 KiB).
  - O token gera ~1 MiB de padrões na entrada e outro ~1 MiB no `WARD_TOKEN` da entrega. Somado aos ~2,4 MiB de um anexo de 256 KiB com bytes de 0 a 255, passa de 4 MiB.
  - Separei em duas entradas `api`, uma com o anexo cheio e outra com o token cheio.
  - O mesmo vale em geral: entrada com segredos grandes e anexos cheios pode ser recusada pelo orçamento. É o que D29 quer, mas o app (Fase 34) vai mostrar essa recusa mais do que se espera.
- **`examples/dev-cofre/main.rs` (Fase 19)** ainda define `const MAX_ANEXO: u64 = 16 * 1024 * 1024`. Ele passa nos testes, porque as fixtures são pequenas e a validação do núcleo recusa o resto. Mas é um limite repetido fora de `limites` (D32) e diverge de D29. A Fase 19b deve trocar para `ward_core::limites::MAX_ANEXOS_SOMADOS`.
- **Mensagens com número no texto:** "256 KiB", "até 63", "0 a 99999" e "16 a 128" ficam em `&'static str`, porque `ErroValidacao` só aceita texto fixo. O teste `mensagens_citam_os_limites_atuais` reprova se a constante mudar e a mensagem não. O `Display` de `ErroCorpo::GrandeDemais` também cita "64 MiB e 10 000" e não tem esse teste.
- **Redis sem zero à esquerda** (`03` e `00` recusados) é escolha minha, para ter uma forma só de cada número. D33 só diz "número de 0 a 99999".
- **`nota segura` fora do orçamento** também deixa sem limite de máscara os extras e anexos dela: até 64 × 64 KiB + 256 KiB. É inofensivo porque ela não tem `ward run`. Se um dia a nota ganhar entrega, o orçamento passa a valer sozinho, porque `custo_da_entrada` mede pela entrega.
- **Custo de CPU:** a validação agora gera os padrões da máscara (até 4 MiB por entrada) para medir. A leitura de um cofre de 64 MiB cheio de entradas grandes fica proporcionalmente mais lenta (segundos, no pior caso, em debug). Corpo forjado continua recusado na 1ª passada, em menos de 100 ms.
- O V2.36 diz "a maior entrada aceita de cada tipo". O teste fixa campos grandes (segredos de 4 KiB com caracteres de escape, anexos no limite, 16 tags, textos no máximo) e busca o maior volume de extras aceito. O máximo absoluto vale por construção: a validação mede com a mesma função que monta a máscara.
- `entrada.rs` ganhou só o `GRANDE_PARA_MASCARA` no `pub use`. Fora da lista do plano, mas necessário para o app e os testes.
- O `scratchpad/fora` que a Fase 19 apagou não era desta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/limites.rs` | criado |
| `crates/ward-core/src/limites/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (`pub mod limites;`) |
| `crates/ward-core/src/entrada.rs` | alterado (só o `pub use` de `GRANDE_PARA_MASCARA`) |
| `crates/ward-core/src/entrada/validacao.rs` | alterado |
| `crates/ward-core/src/entrada/testes.rs` | alterado |
| `crates/ward-core/src/corpo.rs` | alterado |
| `crates/ward-core/src/corpo/testes.rs` | alterado |
| `crates/ward-core/src/corpo/cofre_teste.rs` | alterado (`um_de_cada_cheio`) |
| `crates/ward-core/src/mascara.rs` | alterado |
| `crates/ward-core/src/mascara/testes.rs` | alterado |
| `crates/ward-core/src/entrega/testes.rs` | alterado (só testes) |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado |
| `docs/mission/handoffs/phase-19a.md` | criado |

`Cargo.toml`, `Cargo.lock`, `dev/`, `Makefile` e `examples/` não foram tocados.

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (nada em `.dev/` tocado; valores `falso-*`)
- [x] Teste de ataque para cada (SEG) tocado (V2.30, V2.32, V2.36, V2.37, V2.38, V2.40)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
