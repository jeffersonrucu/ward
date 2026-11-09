# Fase 19c — Correções da auditoria do M2 (D36–D45)

## Implementado
- **D36 (máscara por linha):** a passada por linha (`Gerador::linhas`) vale para os anexos e também para todo segredo de campo ou extra com `\n`. Isso cobre a chave privada em campo e o JSON de várias linhas. Cada linha com 6 caracteres ou mais vira padrão sem o `\r` do fim e também aparada. Os valores de `CHAVE=valor`, `"chave": "valor"` e `chave: valor` (novo `valor_de_dois_pontos`) viram padrão com e sem aspas, cada um com o mesmo mínimo de 6.
- **D37 (formas escapadas):** `Gerador::segredo` gera mais três formas, cada uma só quando difere da literal:
  - JSON do Go (`&`, `<`, `>`, ` `, ` `);
  - JSON `ensure_ascii` do Python (`\uXXXX` minúsculo, com par substituto);
  - aspas duplas do shell (`\` antes de `$`, `` ` ``, `"` e `\`).
  Os buffers da pilha são zerados no fim.
- **D38 (texto de exibição):** a lista negra de invisíveis virou lista branca: U+0020–U+007E mais U+00C0–U+017F, sem × e ÷. A mensagem é "caractere não permitido". Segredos não passam por ela.
- **D39 (anti-rollback):**
  - O corpo passou a v3, com `geracao: u64` (≥ 1) logo depois da versão. Corpo v2 e geração 0 são recusados com o erro genérico.
  - `CofreAberto` guarda a geração: `geracao()` é pública; `proxima_geracao`, `gravou` e `de_partes` são `pub(crate)`.
  - `gravar_entradas(destino, &mut cofre, entradas)` grava a geração n+1 e só sobe o cofre depois de gravar com sucesso. Em `u64::MAX` devolve `GeracaoEsgotada` sem tocar no disco.
  - `corpo::abrir_entradas_desde(arquivo, senha, geracao_minima)` devolve `ErroAbrir::Retrocesso` quando a geração do arquivo é menor que a mínima.
  - `gravacao::trocar_senha(destino, atual, nova, geracao_minima)` devolve `ErroGravacao::Retrocesso`. `formato::trocar_senha` agora só devolve as chaves e o corpo, sem selar.
- **D40 (chave no heap):**
  - `cripto::Chave` virou um newtype `Box<Zeroizing<[u8; 32]>>` (`ZeroizeOnDrop`, `Deref`). Uma asserção `const` prende o tamanho dela ao de `usize`.
  - O `buf` de `ler_phc_pin` é `Zeroizing`.
  - Em `conferir_pin`, a saída recalculada fica num `Zeroizing` e é comparada por `ctutils::CtEq`.
- **D41 (URL):** parser estrito.
  - Esquema `https` em minúsculas; `http` só para `localhost` e `127.0.0.1`.
  - Host em minúsculas: domínio com rótulos `[a-z0-9-]` de 1 a 63; IPv4 sem zero à esquerda; IPv6 entre colchetes via `std`.
  - Porta de 1 a 65535, sem zero à esquerda.
  - Sem usuário na URL, sem `\`, e todo `%` com dois dígitos hex.
- **D42 (variável do `api`):** precisa começar por letra maiúscula e terminar em `_TOKEN`, `_KEY`, `_SECRET`, `_PASSWORD`, `_PAT` ou `_AUTH`. A lista negra continua por cima e agora também recusa nomes com `ASKPASS`.
- **D43 (usuário de `servidor`/`ftp-sftp`):**
  - `[A-Za-z0-9._@-]`, sem `-` no começo, até `MAX_USUARIO`. O `@` entrou por decisão do usuário, repassada pelo orchestrator.
  - O usuário de `banco` e de `smtp` passou a aceitar só ASCII visível, sem `-` no começo.
  - Atualizei D43 no PRD, a V2.49 e a V4.35 no contrato, e a nota da Fase 27 no plano: `-l <usuário>`, o host depois de `--`, e um teste de ataque com usuário que contém `@`.
- **D44:** `gravar_cofre`, `gravar` e `CofreAberto::selar` passaram a `pub(crate)`. Os doctests `compile_fail` cobrem `gravar_cofre` (E0603) e `selar` (E0624).
- **D45:** a mensagem de `ErroCorpo::GrandeDemais` agora é montada a partir de `MAX_CORPO`/`MIB` e `MAX_ENTRADAS`. `MAX_USUARIO` foi criado em `limites`.
- **Exemplo `dev-cofre`:** grava na geração 1, confere a geração lida e imprime a linha de configuração com `geração N`. Ajustei `dev/testes/dev-cofre.sh` e `CLAUDE.md` para o corpo v3.
- **Orçamento de D29 recontado:** as 21 entradas de D15 continuam aceitas. O exemplo `dev-cofre` abriu, validou e conferiu todas (`v2_29_grava_reabre_e_confere`, `make teste-dev-cofre` com exit 0). Não foi preciso parar.

## Segurança
- **Invariantes tocadas:**
  - S4: máscara, D36/D37 e o orçamento recontado.
  - S7/S9: geração e anti-rollback; a gravação pública só a partir de entradas.
  - S8: chave no heap e comparação em tempo constante.
  - S3/S13: URL, variável e usuário; texto de exibição.
  - S2: `selar` e `gravar_cofre` fora da API pública.
- **Testes de ataque** (os repros do auditor viraram testes):
  - `mascara.rs` (repro):
    - `mascara/testes.rs` › `v2_41_chave_em_campo_mascarada_por_linha`, `v2_41_json_de_varias_linhas_e_yaml` e `v2_41_controle_sem_a_passada_por_linha_vazaria` (controle: os padrões antigos deixariam o miolo da chave passar);
    - `mascara/testes.rs` › `v2_42_formas_escapadas` e `formas_escapadas_so_quando_mudam`.
  - `validacao.rs` (repro):
    - `entrada/testes.rs` › `v2_45_texto_de_exibicao_e_lista_branca`, `v2_47_url_estrita`, `v2_48_variavel_do_api_com_sufixo` e `v2_49_usuario_do_ssh` (com `@` aceito; `deploy@x;id`, `deploy@'$(id)'`, `deploy@%h` e `-deploy@x` recusados);
    - `entrega/testes.rs` › `v2_48_variavel_aceita_sai_com_o_token`.
  - Corpo forjado: `corpo/testes.rs` › `ataque_corpo_forjado_com_texto_variavel_e_usuario` (D38/D42/D43 na leitura, incluindo o usuário do salto e o do ftp).
  - `gravar.rs` (repro de rollback):
    - `corpo/testes.rs` › `v2_44_geracao_sobe_e_retrocesso_e_recusado`, `v2_44_geracao_esgotada_nao_toca_no_disco` e `v2_44_geracao_zero_e_corpo_v2_recusados`;
    - `v2_31_campos_forjados` agora recusa as versões 2 e 4.
  - Orçamento:
    - `limites/testes.rs` › `v2_36_maior_entrada_de_cada_tipo_monta_a_mascara` (V2.43: chave de 64 KiB em várias linhas e extra JSON de várias linhas; custo medido ≥ real) e `v2_43_entrada_que_so_passa_do_orcamento_por_d36_e_recusada`;
    - o par de D19 continua em `v2_36_par_banco_e_servidor_de_tunel_monta_a_mascara`.
  - Leitura do código:
    - `tests/fontes.rs` › `v2_46_chave_no_heap_e_sem_hash_password` (asserção de tamanho, `buf` em `Zeroizing`, `ct_eq`, nenhum `hash_password(`/`verify_password(`, nenhuma chave de 32 bytes por valor fora do `Box`);
    - `tests/fontes.rs` › `v2_50_unicas_gravacoes_publicas` (só `gravacao.rs` escreve arquivo; os `pub fn` dele são `gravar_entradas`, `trocar_senha` e `limpar_temporarios`).
  - V2.51: `corpo/testes.rs` › `v2_51_mensagem_de_grande_demais_acompanha_os_limites`. Trava, KDF, UUID, PHC e `redis` já tinham cobertura das fases 19a e 19b.
  - Doctests `compile_fail` (`formato.rs`, `gravacao.rs`):
    - `configuracao_mut().geracao`, `definir_geracao`, atribuição a `geracao` e `selar` de fora (V2.44 e V2.50);
    - `gravar_cofre` de fora (E0603).
  - O V2.33 (`tests/corpo_memoria.rs`) segue com zero achados. Ele passou a gravar por `gravar_entradas` e ganhou os casos forjados "versão 2" e "geração 0".
- **Dependência nova:** `ctutils` 0.4.8 (`default-features = false`). Motivo: D40 pede comparação em tempo constante por biblioteca auditada. Ela já estava no lock como dependência do `argon2` 0.6, então o lock só ganhou a linha de dependência do `ward-core`. `cargo audit` e `cargo deny` estão limpos.

## Ficou de fora
- `dev/variacoes.py` (a lista de variações do `make dev-variacoes`) não gera as formas de D36/D37. Ele está fora do escopo pedido. Registrei em "Problemas".
- Os repros rodam contra o cofre em `.dev/`. Não os executei; reescrevi cada caso como teste com entradas montadas em memória e uma chave falsa de várias linhas. Os casos de RA3 do repro (hex, UTF-16, HTML) são risco aceito e não ganharam teste.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 2400 cargo test -p ward-core --locked --no-fail-fast` | 0 |
| `cargo fmt --all` | 0 |
| `timeout 3000 make check` (1.ª vez) | 2 (clippy em teste; corrigido) |
| `timeout 3000 make check` (2.ª vez) | 2 (gitleaks: chave YAML `client-key-data` no teste; renomeada) |
| `timeout 3000 make check` | 0 |
| `timeout 2400 make teste-dev-cofre` | 0 |

## Problemas descobertos
- No `prd.md` e no `validation-contract.md` (V2.42), os escapes `\uXXXX` aparecem já decodificados. Por exemplo, "`{"p": "senha-ação-01"}` (`json.dumps`)" deveria ser `senha-ação-01`, e `falso-😀-01` deveria ser `falso-😀-01`. Os testes usam as formas escapadas, que eram a intenção. O planner deve corrigir o texto.
- `dev/variacoes.py` não acompanha D36 e D37. A lista do `make dev-variacoes` (usada como agulha nos testes de vazamento) não tem as linhas soltas nem as formas Go, `ensure_ascii` e shell.
- `trocar_senha` ganhou o parâmetro `geracao_minima`, e `formato::trocar_senha` deixou de selar. O daemon (M3) deve passar a maior geração que já viu, persistida no estado dele. RA4 está fora do modelo, por decisão do usuário.
- `cripto::Chave` é um newtype, não um alias de `Box<Zeroizing<…>>` como diz o texto de D40. O `zeroize` não implementa `ZeroizeOnDrop` para `Box<T>`. O tamanho continua o de um ponteiro, e a asserção prova isso.
- `gravar_cofre` e `gravar` ficaram sem uso fora dos testes (`#[cfg_attr(not(test), allow(dead_code))]`). O backup da Fase 45 deve usá-los ou removê-los.
- `MAX_USUARIO` é igual a `MAX_LINHA` (2 KiB). D43 só diz "o limite de usuário de `limites`". Se o planner quiser um limite menor (por exemplo 256), é uma mudança de uma linha.
- Com a chave privada no limite de 64 KiB (~1,6 MiB de padrões), o `servidor` e o `ftp/sftp` não cabem no orçamento junto com os anexos no limite de 256 KiB. Já era assim antes de D36, que só soma cerca de 64 KiB pela passada por linha. A validação recusa com a mensagem de D29. Por isso, a "maior entrada aceita" desses tipos no teste leva metade dos anexos. Nenhuma entrada de D15 chega perto disso.
- No `ward ssh` (Fase 27/28) o usuário pode ter `@`. A Fase 28 não deve montar o `ProxyJump` como `usuário@host` na linha de comando. Escrevi isso em D43 no PRD.
- O usuário de `banco` e de `smtp` passou a aceitar só ASCII visível e a recusar `-` no começo. Isso segue D43, que fala de "usuário estrito", mas o texto de D43 cita só `servidor` e `ftp/sftp`. Fica registrado para o planner confirmar.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/Cargo.toml` | alterado |
| `Cargo.lock` | alterado (só a linha `ctutils` do `ward-core`) |
| `crates/ward-core/src/cripto.rs` | alterado |
| `crates/ward-core/src/formato.rs` | alterado |
| `crates/ward-core/src/formato/testes.rs` | alterado |
| `crates/ward-core/src/limites.rs` | alterado |
| `crates/ward-core/src/limites/testes.rs` | alterado |
| `crates/ward-core/src/corpo.rs` | alterado |
| `crates/ward-core/src/corpo/testes.rs` | alterado |
| `crates/ward-core/src/gravacao.rs` | alterado |
| `crates/ward-core/src/mascara.rs` | alterado |
| `crates/ward-core/src/mascara/testes.rs` | alterado |
| `crates/ward-core/src/entrada/validacao.rs` | alterado |
| `crates/ward-core/src/entrada/testes.rs` | alterado |
| `crates/ward-core/src/entrega/testes.rs` | alterado |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado |
| `crates/ward-core/tests/fontes.rs` | criado |
| `crates/ward-core/examples/dev-cofre/main.rs` | alterado |
| `crates/ward-core/examples/dev-cofre/testes.rs` | alterado |
| `dev/testes/dev-cofre.sh` | alterado |
| `CLAUDE.md` | alterado (descrição do `make dev-cofre`: corpo v3, geração 1) |
| `docs/mission/prd.md` | alterado (D43 com `@`) |
| `docs/mission/validation-contract.md` | alterado (V2.49, V4.35) |
| `docs/mission/plan.md` | alterado (nota da Fase 27 sobre `@`) |
| `.dev/cofre-teste.ward` | regenerado pelo `make teste-dev-cofre` (corpo v3, geração 1) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase (wardd, protocolo e CLI intocados)
