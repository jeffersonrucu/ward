# Fase 15a — Corpo do cofre e acertos do núcleo

## Implementado
- **`ward-core::corpo`** (público, novo; sem dependência nova, sem `unsafe`). Faz D26: as
  entradas viram bytes para o `formato` cifrar e voltam.
  - Formato (desenho no topo de `corpo.rs`): versão do corpo (u16 = 1), número de
    entradas (u32) e, por entrada: tipo, nível, nome, grupo, ambiente e projeto; as
    tags; os campos do tipo na ordem de `esquema(tipo)`; os extras (nome, marca de
    segredo u8 0|1, valor); e os anexos (nome, conteúdo). Todo campo leva o comprimento
    antes (u32 LE). Tipo, nível, ambiente, motor, segurança e protocolo vão pelos
    identificadores estáveis da Fase 15. A porta vai em 2 bytes, e campo opcional ausente
    vai como campo vazio.
  - **Canônica:** as entradas saem em ordem de nome, e a leitura exige essa ordem estrita.
    Isso também barra nome repetido. Resultado: todo corpo aceito, serializado de novo, dá
    os mesmos bytes. O fuzz confere isso em cada corpo aceito.
  - `serializar(&[Entrada]) -> Zeroizing<Vec<u8>>`:
    1. confere o cofre (nome repetido ou túnel/salto inválido dão `Inconsistente`, porque
       um cofre assim não reabriria);
    2. calcula o tamanho final numa passada que só conta;
    3. recusa acima de 64 MiB ou de 10 000 entradas com `GrandeDemais`, antes de alocar;
    4. reserva a capacidade exata e escreve. O escritor nunca realoca: se faltar espaço,
       marca erro em vez de crescer o buffer.
  - `desserializar(&[u8]) -> Vec<Entrada>` lê em duas passadas:
    1. A primeira percorre o buffer inteiro **sem alocar nada**. Confere o limite de
       64 MiB, a versão, cada contagem (10 000 entradas, 16 tags, 64 extras, 32 anexos),
       cada comprimento contra o limite de D25 e contra o que resta do buffer, a porta com
       2 bytes, a marca 0|1, o tipo e o nível na lista, a ordem dos nomes e se sobrou byte.
    2. A segunda monta cada `Rascunho`, passa por `Rascunho::validar`, aplica
       `definir_nivel(nível guardado)` e confere o cofre todo por `conferir_no_cofre`.

    Qualquer falha devolve `ErroCorpo::Invalido`, sem nenhum dado. Ou o cofre abre
    inteiro, ou não abre.
  - Conferência do cofre sem custo quadrático: cada entrada é conferida por
    `conferir_no_cofre` contra a entrada para onde aponta e as que apontam para ela, que
    são as únicas que esse método olha (o nome repetido já foi barrado pela ordem). Os
    testes de fuzz conferem cada corpo aceito também pela conferência completa, entrada
    contra todas.
  - `abrir_entradas(arquivo, senha) -> (CofreAberto, Vec<Entrada>)` (`ErroAbrir` =
    `Formato` | `Corpo`): é o caminho das Fases 19 e 22.
- **`gravacao` (D27):**
  - `gravar` virou `pub(crate)`.
  - Novas `gravar_entradas(destino, &CofreAberto, &[Entrada])`, que serializa e grava por
    `gravar_cofre`, e `trocar_senha(destino, atual, nova) -> CofreAberto`. Esta lê o
    arquivo com limite de tamanho, troca a senha por `formato::trocar_senha` (sal, chave e
    nonces novos) e grava pelo `gravar` interno.
  - `ErroGravacao::Corpo(ErroCorpo)` e `Etapa::Ler` são novos.
  - O doc do módulo traz um doctest `compile_fail,E0603` (chamar
    `ward_core::gravacao::gravar` de fora) e um `no_run` com o caminho público.
- **`formato` (V2.28):** `impl ZeroizeOnDrop for SenhaMestra {}` e
  `for CofreAberto {}`. A senha é `Zeroizing`; a chave de dados é `Zeroizing`; sal, nonce
  e embrulho são públicos no cabeçalho.
- `entrada/testes.rs`: o teste de tempo de compilação `v2_28_*` agora exige
  `ZeroizeOnDrop` de `SenhaMestra` e `CofreAberto`. Foram só duas linhas; o código de
  `entrada` não mudou.
- `lib.rs`: só a linha `pub mod corpo;`.

## Segurança
- Invariantes tocadas:
  - **S8 / V2.28, V2.33:**
    - o buffer do corpo é `Zeroizing`, com a capacidade final reservada antes;
    - na leitura, todo segredo é copiado do buffer direto para um `Segredo`, numa alocação
      do tamanho exato;
    - o que já foi montado quando o corpo é recusado zera ao sair;
    - `CofreAberto` e `SenhaMestra` têm a marca `ZeroizeOnDrop`.

    O alocador de teste confirma que nenhum bloco liberado ou realocado tem segredo, e
    nisso entram a senha mestra e o Argon2.
  - **S2 / S7 / V2.34:**
    - toda gravação pública parte de um `CofreAberto` e sela antes (`gravar_entradas`,
      `gravar_cofre`, `trocar_senha`);
    - gravar bytes soltos de fora do crate não compila;
    - o arquivo gravado não tem nenhuma marca em claro.
  - **S6 / S13 / V2.31, V2.32:** o corpo decifrado é tratado como entrada não confiável.
    Comprimento, contagem e limite são conferidos antes de alocar, cada entrada passa de
    novo pela validação da Fase 15, e há um erro só, sem valor.
  - **S9:** o nível volta como foi gravado; os três níveis de D15 têm teste.
- Testes de ataque escritos:
  - `crates/ward-core/src/corpo/testes.rs`:
    - `v2_30_ida_e_volta_do_cofre_de_teste`: as 21 entradas (D15 + `acme-totp-dev` de
      D28) com os níveis da tabela; comparação campo a campo; canônico, ordem de entrada
      indiferente; escritor independente do teste dá os mesmos bytes;
    - `v2_30_uma_de_cada_tipo_no_maximo_do_modelo`: projeto, 16 tags, 64 extras, anexo
      binário 0..255, URL de 2048, token de 64 KiB, nota de 1 MiB;
    - `v2_30_ida_e_volta_pelo_disco_com_anexos_de_16_mib`: 30 entradas (três com anexo de
      16 MiB, corpo de ~50 MiB) passam por `gravar_entradas` → arquivo → `abrir_entradas`;
      o arquivo não tem nenhuma marca; o cofre vazio também vai e volta; senha errada dá
      erro do formato;
    - `v2_30_cofre_vazio`, `formato_conhecido_de_uma_entrada` (vetor feito à mão, byte a
      byte);
    - `troca_de_senha_sela_e_grava_pelo_mesmo_caminho`: com a senha atual errada, recusa
      e o arquivo não muda; a senha antiga para de abrir;
    - `v2_31_truncado_em_cada_tamanho_e_byte_a_mais` (0..n−1, e +0x00/0x01/0xff);
    - `v2_31_campos_forjados`, 35 casos, todos com o mesmo erro, sem pânico e sem o valor
      no `Display`/`Debug`:
      - versão 0, 2 e 0xffff; contagem de entradas a mais e a menos;
      - tipo fora da lista ou trocado; nível fora da lista (`admin`, vazio, `Livre`,
        `livre `);
      - nome repetido, colado ou não; fora de ordem;
      - salto inexistente, para servidor com salto ou para si mesmo; servidor usado como
        salto que ganha salto; túnel para quem não é servidor;
      - `-oProxyCommand=…` no host e `-J` no usuário; U+202E no grupo; nulo num segredo;
        senha e chave juntas;
      - porta 0 e porta com 3 bytes; ambiente `PROD`; nome não UTF-8; marca de segredo 2;
      - 17 tags, 65 extras e 33 anexos; grupo e segredo acima do limite; anexo `chave` no
        servidor;
    - `v2_31_bit_trocado_em_cada_byte` e `v2_31_100_000_mutacoes_aleatorias` (SplitMix
      com semente fixa, 4 threads; as mutações trocam bit, trocam byte, gravam u32
      "interessante", apagam, duplicam, inserem e trocam bytes): o resultado é sempre
      recusa, ou entradas que passam na conferência completa e são canônicas;
    - `serializar_recusa_cofre_que_nao_reabriria`, `serializar_recusa_mais_de_10_000_entradas`
      (e 10 000 cabem), `anexo_vazio_nao_existe_no_modelo`,
      `leitura_no_limite_aceita_e_acima_recusa`, `erros_sem_dado_do_cofre`.
  - `crates/ward-core/tests/corpo_memoria.rs` (binário próprio, com `#[global_allocator]`,
    que conta pedidos e procura marcas só na thread do teste):
    - `v2_32_corpo_forjado_recusado_rapido_sem_alocar`: campo com `u32::MAX` (no primeiro
      campo e no grupo); anexo declarando 4 GiB, 16 MiB + 1 e 16 MiB sem os bytes; 2^32−1,
      10 001 e 2 entradas declaradas; 17 tags, 65 extras, 33 e 2^32−1 anexos; corpo acima
      de 64 MiB. Todos são recusados em menos de 100 ms, sem pânico e **com zero pedidos ao
      alocador**;
    - `v2_32_corpo_autentico_aloca_so_o_que_existe`;
    - `v2_32_codificador_recusa_acima_do_limite_antes_de_alocar`: 4 anexos de 16 MiB dão
      `GrandeDemais`, com o maior pedido abaixo de 1 KiB; 3 cabem;
    - `v2_33_nenhum_bloco_liberado_tem_segredo`. Os dois controles são acusados: um `Vec`
      comum com o corpo e um segredo que cresce por realocação. O caminho inteiro (montar,
      serializar, desserializar, `CofreAberto::novo`, selar, `abrir_entradas`,
      re-serializar e soltar tudo) acha 0 marcas. Um corpo recusado no meio da 2ª passada
      também acha 0.
  - `crates/ward-core/src/gravacao.rs`: doctest `compile_fail,E0603` (V2.34).
  - `crates/ward-core/src/entrada/testes.rs`: `v2_28_tipos_de_segredo_zeram_e_nao_clonam`
    agora cobre `SenhaMestra` e `CofreAberto`.
- Mutações conferidas à mão (todas revertidas; `diff` igual ao original):
  - buffer do corpo sem a reserva exata e escrevendo com realocação: `v2_33` falha;
  - sem a ordem estrita dos nomes (nas duas passadas): falham 4 testes (nome repetido,
    bit trocado, campos forjados, mutações);
  - sem `impl ZeroizeOnDrop for CofreAberto`: o teste `v2_28` não compila (E0277);
  - `gravar` de volta a `pub`: o doctest `compile_fail` falha.

  Duas mutações não derrubaram teste, porque o comportamento não muda: (a) tirar da
  conferência "quem aponta para esta" (a regra "usado como salto não ganha salto" também
  sai pela entrada que aponta: "o salto não pode ter salto"); (b) tirar a checagem de
  2 bytes da porta na 1ª passada (a 2ª passada recusa de novo).
- Dependências novas: nenhuma.

## Ficou de fora
- `make dev-cofre`: é da Fase 19. Ela usa `CofreAberto::novo` + `gravar_entradas`, e
  `cofre_teste.rs` (só para testes) mostra como montar as 21 entradas pela API pública.
- `limpar_temporarios` ao abrir e uso pelo daemon: Fase 22.
- Backup datado: Fase 45, que deve usar o `gravar` interno.
- Comparar segredos "por sha256" (V2.30): comparei os bytes direto. Isso é mais forte e não
  pede dependência nova (o `ward-core` não tem `sha2`), e nenhuma mensagem de falha imprime
  segredo.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked --lib corpo` (`CARGO_TARGET_DIR` no scratchpad; 16 testes, ~30 s) | 0 |
| `cargo test -p ward-core --locked --test corpo_memoria` (antes do ajuste do teste do corpo autêntico) | 101 |
| `cargo test -p ward-core --locked --test corpo_memoria` (4 testes, ~5 s) | 0 |
| `cargo test -p ward-core --locked --doc` | 0 |
| mutações (buffer, ordem dos nomes, `ZeroizeOnDrop`, `gravar` público) e reversão | 101 → 0 |
| `make check` (1ª vez: `cargo fmt --check` acusou só meus arquivos) | 2 |
| `rustfmt --edition 2024` nos meus arquivos e `make check` (148 testes de unidade, 4 de integração, doctests, audit, deny, gitleaks) | 0 |

## Problemas descobertos
- **V2.30 × D25, anexo vazio:** o V2.30 pede "um anexo vazio" na ida e volta. A
  validação da Fase 15 recusa anexo vazio (`anexo vazio`), e D25 não fala disso. Uma
  `Entrada` com anexo vazio não existe, então ela não tem como fazer ida e volta. Não
  escolhi lado: o teste `anexo_vazio_nao_existe_no_modelo` registra o comportamento atual
  (o anexo vazio é recusado na validação e no corpo forjado). O planner decide se o
  contrato muda ou se a validação passa a aceitar.
- **V2.30 × D26, 16 MiB:** "uma de cada tipo com … um [anexo] de 16 MiB" são 9 × 16 MiB =
  144 MiB, acima do limite de 64 MiB do corpo (D26). Pus o anexo de 16 MiB em três delas,
  num corpo de ~50 MiB. O contrato precisa dizer "uma das entradas", ou o limite do corpo
  precisa subir.
- **O corpo v1 não tem onde guardar o hash do PIN.** O V3.13 diz que o PIN "só existe
  como hash Argon2id dentro da parte cifrada", e D26 define o corpo como versão +
  entradas. A Fase 22 vai precisar de um campo de configuração do cofre (hash do PIN, e
  talvez a configuração do backup). Como versão desconhecida é recusada, o caminho é um
  corpo v2 com uma seção de metadados. Fica para o planner decidir antes da Fase 22,
  inclusive se isso entra já na v1.
- **`unsafe` no teste de integração:** o `#[global_allocator]` exige `unsafe impl
  GlobalAlloc`. Ficou em `crates/ward-core/tests/corpo_memoria.rs`, com
  `#![allow(unsafe_code)]` só nesse binário de teste e `// SAFETY:` em cada bloco. A regra
  diz "unsafe só em módulo de plataforma", mas o plano pede esse alocador num teste de
  integração do `ward-core`. Nada disso entra em binário de produção. A procura lê os
  blocos byte a byte com `read_volatile`, inclusive bytes que nunca foram escritos. No
  modelo do Rust, isso é leitura de memória não inicializada, aceita aqui por ser só
  teste. O auditor deve confirmar que esta exceção vale.
- **V2.32, "nenhum pedido acima do tamanho real do buffer":** vale à risca para todo
  corpo forjado (zero pedidos). Num corpo autêntico, o vetor de entradas pede
  `n × size_of::<Entrada>()` (288 bytes cada). Esse número pode passar do tamanho do
  corpo, que tem ~160 bytes por entrada. É proporcional ao que existe de verdade, não ao
  declarado, e é assim que o teste confere.
- **Limites de leitura duplicados:** `corpo.rs` repete os limites de D25 (64 KiB, 1 MiB,
  16 MiB, 2048, 16/64/32) em vez de usar as constantes privadas de `entrada/validacao.rs`.
  A Fase 16 estava mexendo em `entrada.rs` ao mesmo tempo, então não toquei nesse módulo.
  Os testes de ida e volta no máximo do modelo prendem os limites da leitura aos da
  validação. Numa fase futura, vale expor essas constantes como `pub(crate)` e usar uma
  fonte só.
- **As entradas voltam em ordem de nome:** essa é a forma canônica. O daemon (Fase 22)
  não deve depender de outra ordem.
- `gravacao::trocar_senha` não confere se o corpo desserializa: ela só sela de novo o
  conteúdo que já estava autenticado. `ErroGravacao` ganhou `Corpo`, e `Etapa` ganhou
  `Ler`; quem fizer `match` exaustivo nesses tipos fora do crate vai precisar tratar as
  variantes novas (hoje ninguém faz).
- Durante a fase, um `cargo fmt` de outra fase reformatou meus arquivos em andamento, sem
  dano. A 1ª rodada do `make check` também parou no fmt dos meus arquivos; formatei só os
  meus, com `rustfmt`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/corpo.rs` | criado |
| `crates/ward-core/src/corpo/testes.rs` | criado |
| `crates/ward-core/src/corpo/cofre_teste.rs` | criado (só testes; incluído por `#[path]` nos dois testes) |
| `crates/ward-core/tests/corpo_memoria.rs` | criado |
| `crates/ward-core/src/formato.rs` | alterado (`ZeroizeOnDrop` de `SenhaMestra` e `CofreAberto`) |
| `crates/ward-core/src/gravacao.rs` | alterado (`gravar` `pub(crate)`, `gravar_entradas`, `trocar_senha`, `ErroGravacao::Corpo`, `Etapa::Ler`, doctests) |
| `crates/ward-core/src/entrada/testes.rs` | alterado (2 linhas no teste `v2_28`) |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod corpo;`) |
| `docs/mission/handoffs/phase-15a.md` | criado |

`Cargo.toml` e `Cargo.lock` não foram alterados.

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (valores de D15/D28; a chave privada
  falsa é montada em tempo de execução; arquivos só em `target/<perfil>/ward-teste-corpo/`,
  apagados no fim; nada em `.dev/` foi tocado)
- [x] Teste de ataque para cada (SEG) tocado (V2.28, V2.30–V2.34)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (não mexi em `entrega`, máscara, TOTP nem gerador)
