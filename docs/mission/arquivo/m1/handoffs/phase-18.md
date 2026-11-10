# Fase 18 — TOTP (RFC 6238) e gerador de senha

## Implementado
- **`ward-core::totp`** (público, `crates/ward-core/src/totp.rs`):
  - `Totp::de_semente(&Segredo)` (6 dígitos, 30 s) e `Totp::com_digitos(&Segredo, 6..=8)`.
    A semente é base32 maiúsculo, de 16 a 128 caracteres, com `=` só no fim (mesmas
    regras de `semente_totp_valida` da Fase 15). Os bits que sobram no fim são descartados,
    como fazem o `oathtool` e os autenticadores.
  - `codigo_em(segundos_unix) -> CodigoTotp` e `codigo_agora()`. `CodigoTotp` tem
    `codigo()` (zeros à esquerda) e `restante()` (1 a 30 s, para a contagem do app).
  - HOTP (RFC 4226 §5.3): HMAC-SHA1 dos crates `hmac`/`sha1` sobre o contador em
    big-endian. O truncamento dinâmico e os dígitos são cola do algoritmo, e não
    primitiva.
  - A semente decodificada vive num `Zeroizing<Vec<u8>>`, sem `Clone`. O `Debug` mostra
    `semente: "••••"`. A saída do HMAC é zerada, e o estado do HMAC zera pela feature
    `zeroize`. `ErroTotp` (`SementeInvalida`, `DigitosInvalidos`, `RelogioInvalido`) só
    tem texto fixo.
  - O código não é tratado como segredo (R8: o agente recebe o código), então
    `CodigoTotp` tem `Display`/`Debug`/`Clone` normais.
- **`ward-core::gerador`** (público, `crates/ward-core/src/gerador.rs`):
  - `gerar(&OpcoesGerador) -> Result<Segredo, ErroGerador>`. `OpcoesGerador` tem
    `tamanho` (4–128, padrão 24, que bate com o design) e os conjuntos `minusculas`,
    `maiusculas`, `digitos`, `simbolos` (padrão: todos).
  - A fonte é só `cripto::aleatorio` (`getrandom`). Se o RNG falhar, o gerador falha, sem
    fonte reserva.
  - Cada caractere sai uniforme por rejeição: o byte ≥ `256 − 256 % n` é descartado,
    então não há viés de módulo.
  - "Pelo menos um de cada conjunto marcado" também sai por rejeição da senha inteira.
    A distribuição fica uniforme entre as senhas válidas, sem posição reservada para cada
    conjunto.
  - Há dois limites para uma fonte quebrada: 1 000 rejeições seguidas de byte e 10 000
    tentativas de senha. Em qualquer um deles a saída é `AleatorioIndisponivel`, e não um
    laço sem fim.
  - Os bytes aleatórios ficam num `Zeroizing<[u8; 256]>`, e cada byte é zerado ao ser
    usado. As tentativas descartadas são zeradas no próprio buffer (capacidade fixa, sem
    realocação). A senha sai como `Segredo` sem cópia (`mem::take`).
  - Símbolos: `!#$%&()*+,-./:;<=>?@[]^_{|}~`. Ficam de fora espaço, `"`, `'`, crase e
    `\` (ver "Problemas").
- `lib.rs`: só `pub mod gerador;` e `pub mod totp;`, com um comentário. O
  `pub mod entrega;` da Fase 16 entrou em paralelo e foi mantido.

## Segurança
- Invariantes tocadas:
  - **S6:** o HMAC-SHA1 vem só do RustCrypto (`hmac` 0.13, `sha1` 0.11), e o aleatório só
    de `getrandom`. Nada de HMAC, hash ou RNG feito à mão. O base32 é decodificação e
    não cripto. Foi feito na std para não trazer dependência (S14).
  - **R8:** o módulo devolve só o código. A semente não sai por getter, `Debug` ou erro.
  - **S8 / V2.28:** semente e senha gerada ficam em tipos que zeram (`Zeroizing`,
    `Segredo`) e não têm `Clone`. O buffer do RNG e a saída do HMAC também zeram.
  - **S13 / V2.27:** `Debug`/`Display` sem semente nem senha, e erros só com texto fixo.
  - **S14:** duas dependências novas, sem features padrão.
- Testes de ataque e funcionais escritos:
  - `crates/ward-core/src/totp/testes.rs`:
    - `v2_25_vetores_rfc6238_sha1`: RFC 6238 apêndice B, SHA1 em 8 dígitos, com
      T=59 → `94287082` e os outros 5 tempos;
    - `v2_25_vetores_rfc4226_hotp`: RFC 4226 apêndice D, contadores 0–9;
    - `v2_25_semente_do_contrato_codigos_conhecidos`: `JBSWY3DPEHPK3PXP` em 10 tempos
      (0, bordas de período, 2026, 2100, 20e9). Os valores foram conferidos fora do Ward
      com o HMAC da biblioteca padrão do Python;
    - `v2_25_bate_com_oathtool`: compara com o `oathtool --totp -b --now=@T` nos mesmos
      10 tempos e no instante atual. Sem `oathtool`, avisa e passa, a não ser com
      `WARD_EXIGIR_OATHTOOL=1`;
    - `ataque_semente_malformada_e_recusada`: semente vazia, com 15 caracteres, em
      minúsculas, com `1`/`8`, com `=` no meio, com dado depois do `=`, com espaço, `\n`,
      nulo, caractere não ASCII, só enchimento, URI `otpauth://`, com 129 caracteres e em
      bytes não UTF-8;
    - `ataque_digitos_fora_do_intervalo`;
    - `ataque_tempo_extremo_nao_entra_em_panico` (`u64::MAX`);
    - `ataque_debug_e_erros_sem_semente`: o `Debug` não traz a semente em base32, em hex
      nem os bytes, e o erro não repete a semente quase válida;
    - `v2_28_totp_sem_clone_e_semente_zera`: em tempo de compilação, quebra se `Totp`
      ganhar `Clone`;
    - `v2_25_ward_totp_da_entrega_vem_deste_modulo` (pedido do orchestrator depois que a
      Fase 16 fechou): um `Recursos` de teste com relógio fixo implementa
      `codigo_totp` por `Totp::com_digitos(...).codigo_em(T)`. O `entrega::montar` de um
      `login` põe em `WARD_TOTP` os vetores do RFC 6238 (8 dígitos: T=59 → `94287082`,
      1111111109, 2000000000) e os 10 códigos conhecidos de `JBSWY3DPEHPK3PXP`. Nenhuma
      variável do ambiente contém a semente (R8). O teste fica no arquivo de testes do
      `totp`, sem tocar nos arquivos da Fase 16;
    - `restante_e_codigo_agora`, `enchimento_no_fim_e_aceito`, `base32_vetores_rfc4648`.
  - `crates/ward-core/src/gerador/testes.rs`:
    - `v2_26_dez_mil_senhas_por_combinacao`: as 15 combinações × 10 000 senhas, com os
      tamanhos 4–128 em rodízio. Nenhum caractere fora dos conjuntos, pelo menos um de
      cada conjunto marcado, e sem símbolos sai só alfanumérico;
    - `v2_26_tamanho_e_conjuntos_invalidos`: tamanhos 0, 1, 3, 129, 1000 e `usize::MAX`,
      e nenhum conjunto marcado;
    - `v2_26_toda_aleatoriedade_vem_da_fonte`: a mesma fonte dá a mesma senha, o RNG do
      sistema não repete, e `gerar` está ligado a `cripto::aleatorio`;
    - `ataque_rng_falha_nao_gera_senha_fraca`: o RNG falha logo de início ou no meio, e
      não sai senha;
    - `ataque_rng_preso_nao_trava`: com a fonte sempre `0xff` ou sempre `0`, o gerador
      dá erro e não fica em laço;
    - `ataque_vies_de_modulo`: com 4 000 senhas de 128 caracteres, cada caractere fica a
      menos de 8 % da média (o desvio-padrão é ~1,3 %);
    - `ataque_debug_da_senha_e_erros_sem_valor`, `v2_28_senha_gerada_zera`,
      `conjuntos_sao_disjuntos_e_ascii_visivel`.
  - Mutações conferidas à mão, todas pegas e revertidas:
    - sem a rejeição (`limite = 256`): falha `ataque_vies_de_modulo`;
    - `Debug` do `Totp` mostrando a chave: falha `ataque_debug_e_erros_sem_semente`;
    - máscara de 31 bits trocada por 32: falham os 3 testes de vetores.
- Dependências novas (só em `ward-core`, sem features padrão, com a feature `zeroize`):
  - `hmac 0.13.0`: HMAC do RustCrypto, exigido para o TOTP (RFC 6238/4226) sem cripto à
    mão;
  - `sha1 0.11.0`: SHA-1 do RustCrypto. O RFC 6238 e todo autenticador usam SHA1, e a
    entrada só guarda a semente, sem o algoritmo;
  - as duas usam `digest 0.11` e `cpufeatures`/`cfg-if`, que já estavam no grafo (o
    `Cargo.lock` ganhou só `hmac` e `sha1`). `cargo audit` e `cargo deny` limpos.

## Ficou de fora
- **SHA256/SHA512 e período ou dígitos vindos de `otpauth://`:** a entrada (Fase 15) só
  guarda a semente, e o V2.25 pede SHA1. Para isso precisaria do `sha2` e de campos novos
  no modelo. Fica para decisão do planner (ver "Problemas").
- **Integração de produção** (o `Recursos` do daemon chamando `Totp::de_semente(...)
  .codigo_agora()`, e o código no app e na extensão): é das Fases 22, 33 e 36. A ligação
  com o `entrega` já está coberta por teste (acima).
- **Normalizar a semente** (minúsculas, espaços, `otpauth://`): fica com o app, como
  combinado na Fase 15. O núcleo recusa o que não estiver normalizado.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --lib -- totp gerador` (21 testes, ~1,5 s) | 0 |
| mutações (rejeição, `Debug` da semente, máscara de 31 bits) e reversão | 101 → 0 |
| `make check` (1ª vez: fmt dos meus arquivos) | 2 |
| `make check` (2ª vez: dois lints meus, mais o `unused_imports` de `entrada.rs` enquanto a Fase 16 estava em andamento) | 2 |
| `make audit` / `make deny` / `make gitleaks` | 0 / 0 / 0 |
| `make test` (falhou `gravacao::v2_9_sigkill…` com `NotFound` no spawn, ver "Problemas") | 2 |
| `cargo test -p ward-core --lib -- gravacao` (sozinho) | 0 |
| `make check` (fmt, clippy, 104 testes, audit, deny, gitleaks: "tudo limpo") | 0 |
| `cargo test -p ward-core --lib -- totp` (13 testes, com o de integração com `entrega`) | 0 |
| `make check` (final, 105 testes: "tudo limpo") | 0 |

## Problemas descobertos
- **O `oathtool` não está instalado nesta máquina.** O `v2_25_bate_com_oathtool` passou
  pulando a comparação. Os valores fixos da semente de D15 foram conferidos de forma
  independente com o `hmac`/`hashlib` do Python, e os vetores do RFC batem. O validador
  deve instalar o `oathtool` e rodar com `WARD_EXIGIR_OATHTOOL=1`.
- **Teste de gravação instável quando há builds em paralelo:** o
  `gravacao::testes::v2_9_sigkill_em_200_pontos_aleatorios` (Fase 14) faz spawn do
  próprio executável de teste. Se outro worker recompila o `ward-core` durante a rodada,
  o binário some e o spawn dá `NotFound`. Sozinho e no `make check` final, passa. Não
  mexi nele.
- **Conjunto de símbolos (decisão minha, pelo lado seguro):** ficam de fora espaço, `"`,
  `'`, crase e `\`, porque quebram aspas de shell, `.env` e arquivos de configuração. O
  escopo só diz "conjuntos configuráveis". O planner confirma, e o app (Fase 33) mostra
  o mesmo conjunto.
- **Só SHA1, 6 dígitos e 30 s:** uma conta que exija SHA256 (alguns provedores via
  `otpauth://…&algorithm=SHA256`) daria códigos errados sem aviso. Se o escopo quiser
  isso, o modelo de entrada precisa de algoritmo, dígitos e período, e o `ward-core` do
  `sha2`.
- **Semente mínima de 80 bits** (16 caracteres): é o que a Fase 15 aceita e o que a
  fixture usa. O RFC 4226 recomenda 128 bits ou mais. Mantive igual à validação para não
  divergir.
- Durante a fase, o `unused_imports` em `crates/ward-core/src/entrada.rs:35` (os nomes
  reservados que a Fase 15 deixou para a 16) quebrou o clippy até a Fase 16 passar a
  usá-los. Não toquei no arquivo.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/totp.rs` | criado |
| `crates/ward-core/src/totp/testes.rs` | criado |
| `crates/ward-core/src/gerador.rs` | criado |
| `crates/ward-core/src/gerador/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod gerador;`, `pub mod totp;` e um comentário) |
| `crates/ward-core/Cargo.toml` | alterado (`hmac`, `sha1`) |
| `Cargo.lock` | alterado (só as entradas `hmac` e `sha1` e a lista de dependências do `ward-core`) |
| `docs/mission/handoffs/phase-18.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0 na rodada final
- [x] Só cofre de teste e credenciais falsas usados (semente falsa de D15 e vetor do RFC;
  nada em `.dev/` foi tocado)
- [x] Teste de ataque para cada (SEG) tocado (V2.27/V2.28 nos tipos novos, semente
  malformada, RNG falho ou preso, viés de módulo, tempo extremo)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
