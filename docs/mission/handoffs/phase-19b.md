# Fase 19b — Corpo v2 com seção de configuração (D30)

## Implementado
- **`ward_core::configuracao`** (módulo público novo):
  - `HashPin`: hash Argon2id do PIN em texto PHC. Zera ao sair (`Zeroizing<String>`), não tem `Clone` nem `Display`, e o `Debug` mostra `HashPin(••••)`.
    - `HashPin::criar(pin)` aceita de 4 a 6 dígitos ASCII. O sal tem 16 bytes aleatórios, e o custo é `KDF_PIN` = 64 MiB, 3 passes, 1 faixa.
    - `HashPin::conferir(pin) -> bool` é a função que o daemon da Fase 22 só chama.
    - `parametros()` devolve o custo, que não é segredo.
  - `Configuracao`: hash do PIN, `minutos_trava` (padrão 240, de 1 a 1440) e `BackupExterno` (desligado por padrão, com UUID opcional; ligado exige UUID). Não tem `Clone`.
    - Sem hash não há como montar uma: `Configuracao::nova(hash)` exige um.
    - Também tem `conferir_pin`, `trocar_hash_pin`, `definir_minutos_trava` e `definir_backup`.
  - `ErroConfiguracao`: mensagens em português, sem valor.
- **`cripto`** (cola, nada feito à mão):
  - `hash_pin`: calcula por `cripto::argon2id` (memória de trabalho zerada, nota da Fase 13) e escreve o texto PHC pelos tipos do `phc` num buffer fixo de pilha. O resultado vai para uma alocação do tamanho exato.
  - `ler_phc_pin`: lê e confere o texto sem alocar. Exige:
    - Argon2id v=19, só `m`, `t` e `p`;
    - custo entre `KDF_PIN_MINIMO` e `KDF_PIN_MAXIMO` (256 MiB, 16 passes, 4 faixas);
    - sal de 16 bytes e saída de 32;
    - texto canônico: escrito de novo, dá os mesmos bytes.
  - `conferir_pin`: recalcula por `cripto::argon2id` e compara pelo `PartialEq` do `phc::Output`, que é o `CtEq` do `ctutils` (tempo constante, pela biblioteca).
- **Corpo v2** (`corpo.rs`, `VERSAO_CORPO = 2`):
  - Layout novo: `versão ‖ hash do PIN (campo) ‖ minutos (u32) ‖ backup ligado (u8 0|1) ‖ UUID (campo) ‖ n ‖ entradas`.
  - A versão 1 virou desconhecida.
  - A 1ª passada confere a seção inteira sem alocar: texto PHC completo, faixa da trava, marca 0|1, UUID, e ligado sem UUID é recusado.
  - A 2ª passada copia o hash para o `HashPin`.
  - Qualquer falha dá o mesmo `ErroCorpo::Invalido`.
  - Nova assinatura: `serializar(&Configuracao, &[Entrada])` e `desserializar -> (Configuracao, Vec<Entrada>)`. O codificador recusa configuração fora da regra com `Inconsistente`.
- **`formato`** (o `CofreAberto` passa a levar a configuração):
  - `CofreAberto` = `ChavesCofre` (interno: chave de dados e embrulho) + `Configuracao`.
  - `CofreAberto::novo(senha, configuracao)` e `novo_com(senha, kdf, configuracao)`; também `configuracao()` e `configuracao_mut()`.
  - `formato::abrir` e `formato::trocar_senha` viraram `pub(crate)`. De fora, o caminho é `corpo::abrir_entradas` e `gravacao::trocar_senha`.
  - `gravar_entradas` sempre grava a configuração do `CofreAberto`, então nenhum caminho grava as entradas sem o hash do PIN.
- **`gravacao::trocar_senha`** agora desserializa o corpo para tirar a configuração. Corpo inválido dá `ErroGravacao::Corpo`, e nada é gravado. Antes, ele selava de novo sem conferir.
- **`limites`**: entram `MIN_DIGITOS_PIN`/`MAX_DIGITOS_PIN` (4/6), `MAX_HASH_PIN` (128), `MINUTOS_TRAVA_PADRAO` (240), `MIN_MINUTOS_TRAVA`/`MAX_MINUTOS_TRAVA` (1/1440) e `MAX_UUID_BACKUP` (36), com as asserções de coerência.
- **`make dev-cofre`** (exemplo `dev-cofre`):
  - grava `Configuracao::nova(HashPin::criar(pin da fixture))`;
  - a conferência (`--conferir` e o fim do `make dev-cofre`) exige que o PIN da fixture confira, que um PIN vizinho não confira, que o custo seja `KDF_PIN`, a trava 240 e o backup desligado sem UUID;
  - imprime uma linha de configuração sem hash nem PIN.
  - O `MAX_ANEXO` repetido virou `limites::MAX_ANEXOS_SOMADOS` (pedido do orchestrator).
- `dev/testes/dev-cofre.sh`: duas checagens novas (linha de configuração na conferência; `135790` e `$argon2id$` fora do `.ward`). `CLAUDE.md` › "Comandos": descrições do `dev-cofre` e do `dev-cofre-conferir`.

## Segurança
- **Invariantes tocadas:**
  - **S9 / V3.13 (preparação), V2.39:**
    - o PIN só existe como hash Argon2id PHC dentro do corpo cifrado;
    - cofre sem hash, com hash fora do formato, rebaixado ou inflado é recusado;
    - a comparação é em tempo constante, pela biblioteca.
  - **S2 / V2.39:** `135790` e o texto PHC não aparecem no `.ward` (`grep -a` no teste do shell e nos testes Rust) nem no corpo decifrado (teste Rust).
  - **S8 / V2.28, V2.33:**
    - `HashPin` e `Configuracao` são `ZeroizeOnDrop` sem `Clone`, e `ChavesCofre` também; o teste de compilação foi estendido;
    - o alocador de teste procura também o PIN e a saída B64 do hash (marca dinâmica, num buffer fixo por thread) em todo bloco liberado, e acha 0 no caminho inteiro;
    - os dois controles novos (PIN e hash num `Vec` comum) são vistos.
  - **S6, S13 / V2.31, V2.32:**
    - a seção é entrada não confiável: tudo é conferido na 1ª passada sem alocar, com erro único e sem valor;
    - um hash inflado (`m=4194304`) é recusado sem rodar o Argon2.
  - **S13 / V2.27:** os `Debug` de `HashPin`, `Configuracao` e `CofreAberto` não mostram hash, PIN nem senha.
- **Testes de ataque escritos:**
  - `crates/ward-core/src/corpo/testes.rs`:
    - `v2_39_corpo_v1_sem_secao_ou_sem_hash_recusado`;
    - `v2_39_secao_de_configuracao_forjada` (23 hashes forjados, todos em menos de 5 s):
      - PIN em claro;
      - argon2i, argon2d, scrypt, v=16, sem versão;
      - m=8, t=1, p=0, m=4 GiB, t=u32::MAX, p=255;
      - `keyid` a mais, parâmetro faltando, fora de ordem, zero à esquerda;
      - sal curto, saída curta, sem saída, sobra, espaço, não ASCII, acima de 128;
    - na mesma função, 10 casos dos outros campos da seção: trava 0/1441/u32::MAX, marca 2, ligado sem UUID, UUID `../../x`/`-oX`/com espaço/37/U+202E. Os controles no limite abrem;
    - `v2_39_hash_rebaixado_do_pin_certo_recusado`: o hash de custo baixo feito para o PIN certo é PHC válido para a biblioteca, mas o corpo é recusado e o `HashPin` não nasce dele;
    - `v2_39_pin_so_como_hash_no_corpo`, `v2_30_configuracao_volta_igual`, `serializar_recusa_configuracao_fora_da_regra`;
    - V2.30 refeito sobre o corpo v2: o cofre vazio é só a configuração, byte a byte; `formato_conhecido_de_uma_entrada` foi refeito à mão; no disco, a configuração volta igual e o PIN confere; a troca de senha mantém o hash;
    - V2.31 refeito: a versão 1 entra entre as desconhecidas; bit trocado e 100 000 mutações agora cobrem também a seção (o aceito tem configuração válida e é canônico).
  - `crates/ward-core/src/configuracao/testes.rs`:
    - `hash_do_pin_confere_o_pin_certo_e_recusa_os_outros`;
    - `pin_de_4_a_6_digitos`;
    - `hash_e_phc_argon2id_padrao`: o `argon2::PasswordVerifier` serve de oráculo independente;
    - `cada_hash_tem_sal_proprio`;
    - `hash_adulterado_nao_confere`;
    - `hash_inflado_recusado_rapido`;
    - `debug_e_erros_sem_segredo`;
    - `tempo_de_trava_na_faixa`;
    - `backup_externo`;
    - `trocar_o_pin`;
    - `configuracao_nova_tem_os_padroes`.
  - `crates/ward-core/tests/corpo_memoria.rs`:
    - V2.32 ganhou 7 forjados da seção, cada um seguido de entrada válida: hash com `u32::MAX`, sem hash, inflado, rebaixado, PIN em claro, v1 e sem seção. Todos zero alocações e menos de 100 ms;
    - V2.33 com as marcas do PIN e do hash.
  - `crates/ward-core/examples/dev-cofre/testes.rs`:
    - `v2_39_cofre_gerado_tem_a_configuracao`;
    - `v2_29_grava_reabre_e_confere` ampliado: PIN diferente do hash e trava fora da padrão são acusados, sem o PIN no erro.
  - `crates/ward-core/src/entrada/testes.rs`: `v2_28_*` passou a cobrir `HashPin`, `Configuracao` e `ChavesCofre`.
- **Mutações** (feitas no lugar e revertidas, com `cmp` igual ao backup):
  - sem o texto canônico: V2.39 cai;
  - sem a faixa da memória: V2.39 e `hash_inflado` caem;
  - sem a conferência do hash na 1ª passada: o V2.32 do alocador cai (alocou). Antes do ajuste ele não caía, porque os forjados tinham 0 entradas; foi corrigido;
  - versão 1 aceita: V2.39 e V2.31 caem;
  - cópia comum do hash no `de_phc`: V2.33 cai;
  - ligado sem UUID aceito na 1ª passada: não cai, porque a 2ª passada (`BackupExterno::novo`) recusa igual. São duas camadas, com o mesmo erro.
- **Dependências novas:** nenhuma crate nova no `Cargo.lock` (não mudou; `--locked` passa). Foi ligada a feature `password-hash` do `argon2` 0.6, que passa a compilar `password-hash` 0.6.1 e `phc` 0.6.1 (RustCrypto). As duas já estavam no lock, e `ctutils`/`cmov` já eram compiladas. Motivo: usar os tipos do texto PHC da própria biblioteca e a comparação em tempo constante do `phc::Output`, em vez de codificar PHC e comparar à mão. `cargo audit` e `cargo deny` limpos.

## Ficou de fora
- Conferir o PIN no daemon, contar 5 erros e travar: é da Fase 22, que só chama `Configuracao::conferir_pin`.
- Trava automática pelo `minutos_trava`: Fase 24.
- Backup externo de verdade: Fase 45.
- Telas de Configurações: Fase 34+.
- Migração do corpo v1: não existe cofre real (D30).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo build -p ward-core --locked` (`CARGO_TARGET_DIR` no scratchpad) | 0 |
| `cargo test -p ward-core --locked` (177 lib + 4 alocador + 17 exemplo + doctests) | 0 |
| 6 mutações e reversão (`cmp` igual) | 101 em 5; 0 na equivalente (ligado sem UUID) |
| `make check`, 1ª vez | 2 (fmt dos meus arquivos) |
| `rustfmt --edition 2024` nos meus arquivos + `make check` | 2 (`clippy::type_complexity` no retorno de `trocar_senha`) |
| `make check` | 0 |
| `make teste-dev-cofre` (29 linhas `ok`, 2 delas novas; rodado 2 vezes) | 0 |

## Problemas descobertos
- **Escolhas minhas fora do escopo** (o escopo só diz "PIN de 4 a 6 dígitos" e "tempo de trava"):
  - trava de 1 a 1440 minutos, sem "nunca travar";
  - UUID do pendrive com 1 a 36 caracteres `[0-9A-Fa-f-]`, sem `-` nas pontas, o que cobre ext4 e FAT/exFAT;
  - custo do hash do PIN de 64 MiB/3/1, com faixa aceita até 256 MiB/16/4;
  - sal exatamente de 16 bytes e saída de 32;
  - texto PHC canônico exigido.

  O planner confirma ou muda.
- **Força do hash:** um PIN de 6 dígitos sai do hash por força bruta em pouco tempo. A proteção real é a cifra do cofre (D30 põe o hash na parte cifrada). Por isso o `HashPin` é tratado como segredo (zera, sem `Debug`/`Display`/`Clone`), e a interface nunca deve exibi-lo.
- **Resíduo na pilha:** `phc::Output`, `Salt` e `PasswordHash` são tipos `Copy` sem zeroize. Em `conferir_pin` e `escrever_phc`, a saída do hash fica em cópias na pilha que não são zeradas. O heap está limpo (V2.33). O risco é o mesmo da memória do Argon2 em D14.
- **A feature `password-hash` expõe `PasswordHasher`/`PasswordVerifier` do `argon2`**, que chamam `hash_password_into` e não zeram a memória de trabalho (nota da Fase 13). O código de produção não usa nenhum dos dois; só o teste usa o `verify_password` como oráculo. O auditor deve conferir por `grep` que nenhum código fora de teste chama `hash_password`/`verify_password`.
- **API que mudou** (afeta as Fases 22+):
  - `CofreAberto::novo` agora pede a configuração;
  - `serializar` e `desserializar` mudaram de assinatura;
  - `formato::abrir` e `formato::trocar_senha` são `pub(crate)`;
  - `gravacao::trocar_senha` recusa corpo inválido.

  Hoje ninguém fora do `ward-core` usava essas funções.
- **`HashPin::conferir` com PIN fora do formato** devolve `false` sem rodar o Argon2. O tempo revela só que o formato é inválido, o que não é segredo. A espera crescente e os 5 erros ficam com a Fase 22.
- O handoff da 19a dizia que o `MAX_ANEXO` do exemplo era de 16 MiB. No código já estava em 256 KiB; o problema era só ser um número repetido. Agora sai de `limites`.
- O `Debug` de `CofreAberto` passou a mostrar a configuração: tempo de trava e UUID, que não são segredo; o hash aparece como `••••`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/configuracao.rs` | criado |
| `crates/ward-core/src/configuracao/testes.rs` | criado |
| `crates/ward-core/src/cripto.rs` | alterado (hash do PIN em PHC, leitura e conferência) |
| `crates/ward-core/src/corpo.rs` | alterado (corpo v2) |
| `crates/ward-core/src/formato.rs` | alterado (`CofreAberto` com configuração, `ChavesCofre`) |
| `crates/ward-core/src/gravacao.rs` | alterado (`gravar_entradas` com a configuração, `trocar_senha`, doctest) |
| `crates/ward-core/src/limites.rs` | alterado (limites da configuração) |
| `crates/ward-core/src/lib.rs` | alterado (`pub mod configuracao;`) |
| `crates/ward-core/Cargo.toml` | alterado (feature `password-hash` do `argon2`) |
| `crates/ward-core/src/corpo/testes.rs` | alterado |
| `crates/ward-core/src/corpo/cofre_teste.rs` | alterado (`PIN_TESTE`, `configuracao_de_teste`, `configuracao_nova`) |
| `crates/ward-core/src/formato/testes.rs` | alterado |
| `crates/ward-core/src/gravacao/testes.rs` | alterado |
| `crates/ward-core/src/limites/testes.rs` | alterado |
| `crates/ward-core/src/entrada/testes.rs` | alterado (só o `v2_28`) |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado (`unsafe fn casa` nova, com `// SAFETY:`, dentro da exceção de D31) |
| `crates/ward-core/examples/dev-cofre/main.rs` | alterado |
| `crates/ward-core/examples/dev-cofre/testes.rs` | alterado |
| `dev/testes/dev-cofre.sh` | alterado |
| `CLAUDE.md` | alterado ("Comandos": `dev-cofre`, `dev-cofre-conferir`) |
| `docs/mission/handoffs/phase-19b.md` | criado |
| `.dev/cofre-teste.ward` | regerado (fora do git) |

`Cargo.lock` não mudou.

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (senha e PIN de D15; nada fora de `.dev/` e do scratchpad)
- [x] Teste de ataque para cada (SEG) tocado (V2.27, V2.28, V2.30–V2.33, V2.39)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
