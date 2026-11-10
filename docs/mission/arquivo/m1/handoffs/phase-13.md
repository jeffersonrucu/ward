# Fase 13 — Formato e cripto em `ward-core`

## Implementado
- **`ward-core::cripto`** (privado ao crate). É só cola: tamanhos fixos, buffers que
  zeram ao sair e um erro único, sem cripto feita à mão.
  - `argon2id(senha, sal, segredo, dados_assoc, custo, saida)`: Argon2id v0x13 do crate
    `argon2`. O formato chama com `segredo` e `dados_assoc` vazios
    (`derivar_kek`). O vetor do RFC 9106 passa por essa mesma função. A memória de
    trabalho (256 MiB) é alocada aqui num `Zeroizing<Vec<Block>>` e entra por
    `hash_password_into_with_memory`, porque o `argon2` 0.6 não zera a memória que ele
    mesmo aloca (ver "Problemas").
  - `selar`/`abrir`: XChaCha20-Poly1305 do `chacha20poly1305` (`encrypt_inout_detached`/
    `decrypt_inout_detached`). Em `selar`, o buffer de saída já nasce do tamanho final,
    então nenhuma realocação deixa cópia em claro. Em `abrir`, o texto só existe num
    `Zeroizing<Vec<u8>>` e só volta se a tag bater. A comparação da tag é a do próprio
    crate.
  - `aleatorio`/`chave_aleatoria`: `getrandom::fill`. Se o sistema não entregar bytes,
    falha em vez de seguir.
- **`ward-core::formato`** (público): arquivo com cabeçalho de 160 bytes, versionado.

  | offset | tam | campo |
  | --- | --- | --- |
  | 0 | 8 | magia `WARDCOFR` |
  | 8 | 2 | versão (u16 LE) = 1 |
  | 10 | 1 | KDF = 1 (Argon2id 0x13) |
  | 11 | 1 | AEAD = 1 (XChaCha20-Poly1305) |
  | 12 | 4+4+4 | memória KiB, passes, paralelismo (u32 LE) |
  | 24 | 32 | sal |
  | 56 | 24 | nonce da chave |
  | 80 | 48 | chave de dados embrulhada (32 + tag) |
  | 128 | 24 | nonce dos dados |
  | 152 | 8 | tamanho do corpo com a tag (u64 LE) |
  | 160 | … | corpo cifrado ‖ tag |

  - A KEK é Argon2id(senha, sal, parâmetros do cabeçalho). A chave de dados é aleatória
    e vai embrulhada pela KEK, com os bytes 0..80 como AAD. O corpo é cifrado pela chave
    de dados, com o cabeçalho inteiro (0..160) como AAD. Todo byte do arquivo fica
    autenticado, e o tamanho exato do arquivo também (campo de tamanho no AAD).
  - Parâmetros: `PADRAO` = 256 MiB / 3 passes / 4 faixas, `MINIMO` = 256 MiB / 3 / 1 e
    `MAXIMO` = 2 GiB / 16 / 16. Abaixo do mínimo ou acima do máximo, o arquivo é
    recusado **antes** de derivar: um cabeçalho rebaixado não deixa o Argon2 barato, e
    um inflado não faz o daemon alocar memória sem fim (backup trocado no pendrive, por
    exemplo).
  - Senha errada, arquivo adulterado, truncado, acrescido, lixo ou magia errada dão o
    mesmo `ErroFormato::SenhaOuArquivo` ("senha mestra errada ou arquivo do cofre
    adulterado"). À parte ficam só `VersaoNaoSuportada`, `ParametrosAbaixoDoMinimo` e
    `ParametrosAcimaDoMaximo`, que dependem só do cabeçalho público. Nenhuma variante
    carrega dado do cofre.
  - API pública:
    - `SenhaMestra` (zera ao sair, sem `Clone`, `Debug` = `SenhaMestra(••••)`);
    - `CofreAberto::novo`/`novo_com` e `CofreAberto::selar` (regrava com nonce de dados
      novo, sem rodar o Argon2 de novo; é o caminho do daemon);
    - `abrir(arquivo, senha) -> (CofreAberto, Zeroizing<Vec<u8>>)`;
    - `ler_cabecalho` (estrutura e parâmetros, sem autenticar; documentado assim);
    - `trocar_senha(arquivo, atual, nova)`.
  - `CofreAberto` não tem `Clone`, e o `Debug` mostra `chave_dados: "••••"`.
- **Troca de senha mestra** (V2.8): exige a senha atual. O arquivo novo sai com sal,
  chave de dados e nonces novos, e não só com a chave reembrulhada. Assim quem tem a
  senha antiga e uma cópia antiga (backup) não consegue abrir o arquivo novo. Os
  parâmetros ficam os do cofre ou os `PADRAO`, o que for maior.
- `Cargo.toml` do workspace: `opt-level = 3` no perfil dev/test só para `argon2`,
  `blake2`, `chacha20` e `poly1305`. Sem isso, cada Argon2id de 256 MiB leva dezenas de
  segundos no `cargo test`. Com isso, os 25 testes (centenas de derivações reais) rodam
  em cerca de 30 s.

## Segurança
- Invariantes tocadas:
  - **S6**: Argon2id com os parâmetros no cabeçalho, mínimo 256 MiB / 3 passes;
    XChaCha20-Poly1305 com o cabeçalho como AAD; chave de dados aleatória embrulhada;
    um byte alterado faz a abertura falhar; nenhuma primitiva própria.
  - **S2** (parte do formato): o conteúdo não aparece em claro no arquivo, e os buffers
    em claro e as chaves zeram ao sair (`Zeroizing`, `ZeroizeOnDrop` do
    `XChaCha20Poly1305` com a feature `zeroize`, memória do Argon2).
  - **S13**: `Debug` sem segredo, e erros sem dado.
  - **S14**: só as quatro dependências abaixo, sem features padrão.
- Testes de ataque escritos:
  - `crates/ward-core/src/formato/testes.rs`:
    - `v2_1_senha_errada_mesmo_erro_de_adulterado`: com `falso-mestra-ward-02`, o erro é
      o mesmo do corpo adulterado, e com a mesma mensagem;
    - `v2_2_bit_trocado_em_cada_byte`: um arquivo por posição, com o bit `i % 8`
      trocado e parâmetros reais, em 4 threads. Nenhum abre e nenhum entra em pânico
      (`catch_unwind`); o original abre;
    - `v2_3_truncado_e_acrescido`: todos os tamanhos de 0 a n−1, +1 byte (0x00, 0x01,
      0xff), e +1 byte com o campo de tamanho "consertado" (a tag pega);
    - `v2_4_cabecalho_registra_parametros`;
    - `v2_4_rebaixamento_para_8_mib_falha`: é recusado pelo mínimo. E mesmo com um
      mínimo de teste que aceite 8 MiB, a autenticação pega. Também rebaixa só os
      passes;
    - `v2_4_bem_formado_abaixo_do_minimo_recusado`: um cofre autêntico de 8 MiB é
      recusado. O controle prova que ele é bem formado, e `novo_com` não cria cofre
      fraco;
    - `parametros_acima_do_maximo_recusados_sem_derivar`: memória, passes ou paralelismo
      enormes são recusados em menos de 100 ms, sem alocar; paralelismo 0 também;
    - `cabecalho_e_aad_do_corpo`: com a chave de dados certa, qualquer um dos 160 bytes
      do cabeçalho alterado impede o corpo de abrir;
    - `recorte_entre_cofres_e_gravacoes_falha`: cabeçalho de um cofre com corpo de
      outro, corpo de outra gravação do mesmo cofre e chave embrulhada trocada;
    - `v2_5_composicao_do_formato`: o arquivo refeito só com as funções de `cripto`;
    - `v2_6_dois_cofres_iguais_sao_diferentes`: sal, nonces, embrulho, cifrado e a
      própria chave de dados diferem;
    - `v2_8_troca_de_senha` e `troca_de_senha_exige_a_atual`;
    - `nada_em_claro_no_arquivo`, `debug_nao_mostra_segredo`, `lixo_nao_abre`,
      `selar_de_novo_reabre_e_troca_o_nonce`, `ida_e_volta`, `conteudo_vazio_abre`.
  - `crates/ward-core/src/cripto.rs`:
    - `v2_5_argon2id_rfc9106` (RFC 9106 §5.3);
    - `v2_5_xchacha20poly1305_draft_selar`/`_abrir` (`draft-irtf-cfrg-xchacha-03`
      A.3.1), conferidos contra os vetores dos testes dos próprios crates;
    - `abrir_recusa_qualquer_bit_trocado` (cifrado, tag, AAD, nonce, chave, truncado);
    - `derivar_kek_e_o_argon2id_sem_extras`, `aleatorio_nao_repete`.
  - Mutação conferida à mão: com o AAD do corpo sem nonce e tamanho, falham
    `cabecalho_e_aad_do_corpo` e `v2_5_composicao_do_formato`. Revertido.
- Dependências novas (só em `ward-core`, todas sem features padrão):
  - `argon2 0.6.0` (`alloc`, `zeroize`): Argon2id do RustCrypto, exigido por S6. Sem
    `password-hash`/`getrandom`, porque não usamos string PHC.
  - `chacha20poly1305 0.11.0` (`alloc`, `zeroize`): XChaCha20-Poly1305 do RustCrypto,
    exigido por S6. A feature `zeroize` zera a chave no drop.
  - `getrandom 0.4.3`: aleatório do sistema para sal, nonces e chave de dados. É mais
    direto que `rand`, e o `rand_core`/`OsRng` não fazem falta aqui.
  - `zeroize 1.9.1` (`alloc`): buffers e chaves que zeram ao sair.
  - Transitivas: `blake2`, `digest`, `base64ct`, `cpufeatures`, `chacha20`, `cipher`,
    `poly1305`, `universal-hash`, `aead`, `inout`, `crypto-common`, `block-buffer`,
    `hybrid-array`, `typenum`, `ctutils`, `cmov`, `cfg-if`, `libc`. Todas do
    RustCrypto ou de base. `cargo audit` e `cargo deny` limpos.

## Ficou de fora
- Gravação no disco (temp + fsync + rename): é da Fase 14. Aqui o formato só trabalha
  em memória (`&[u8]` → `Vec<u8>`).
- O conteúdo do corpo (entradas, tipos, serialização) é da Fase 15. O formato cifra
  bytes opacos.
- `mlock` da chave de dados (D14): é da Fase 21 (`ward-plataforma`). O `ward-core` não
  depende de Linux.
- Política de senha mestra (tamanho mínimo, força): o formato recusa só senha vazia. A
  regra de UI/daemon fica para as fases do daemon e do app.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core` (25 testes, ~30 s) | 0 |
| mutação do AAD (2 testes falham, como esperado) e reversão | 101 → 0 |
| `make check` | 0 |
| `make teste-ataques-fundacao` ("Todos os ataques foram barrados") | 0 |

## Problemas descobertos
- **V2.7 cita `subtle`, mas o RustCrypto atual não usa mais `subtle`.** O
  `chacha20poly1305 0.11`/`poly1305 0.9`/`digest 0.11` comparam em tempo constante pelo
  `ctutils` (com o `cmov`), que veio no lugar dele. Por isso `cargo tree -p ward-core`
  mostra `ctutils`/`cmov` e nenhum `subtle`. Nenhuma comparação de tag é feita no
  código do Ward. O orchestrator decide se V2.7 ganha uma nota trocando `subtle` por
  `ctutils`.
- **O `argon2` 0.6 não zera a memória de trabalho** (`Blocks::drop` só desaloca, mesmo
  com a feature `zeroize`). Contornei alocando a memória no `cripto::argon2id`
  (`Zeroizing<Vec<Block>>`) e usando `hash_password_into_with_memory`. Vale anotar para
  a auditoria: quem chamar `hash_password_into` direto (o PIN da Fase 22, por exemplo)
  deixa a memória do Argon2 sem zerar. O certo é passar pela mesma função.
- **D14 vs. memória do Argon2**: o PRD diz que a memória do Argon2 não é travada por
  `mlock`. Aqui ela é zerada ao sair, mas pode ir para a swap durante a derivação. O
  risco residual é o mesmo de D14.
- O máximo (2 GiB / 16 / 16) é escolha minha, para barrar negação de serviço com
  cabeçalho inflado. Não está no escopo. Cofre gravado com parâmetros acima disso por
  uma versão futura vai pedir versão nova do formato ou do código.
- Tempo de abertura com os parâmetros padrão: cerca de 0,5 a 1 s por derivação com
  `opt-level = 3`, sem a feature `parallel` (as 4 faixas rodam em sequência). Para o
  destravar está bom. Se o daemon quiser paralelo, ligar `parallel` puxa o `rayon`
  (dependência a mais, S14).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/Cargo.toml` | alterado (4 dependências) |
| `crates/ward-core/src/lib.rs` | alterado (módulos `cripto`, `formato`) |
| `crates/ward-core/src/cripto.rs` | criado |
| `crates/ward-core/src/formato.rs` | criado |
| `crates/ward-core/src/formato/testes.rs` | criado |
| `Cargo.toml` | alterado (`opt-level = 3` das crates de cripto no perfil dev) |
| `Cargo.lock` | alterado |
| `docs/mission/handoffs/phase-13.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (senhas `falso-mestra-ward-0x`,
  conteúdo falso de D15; nada em `.dev/` foi tocado)
- [x] Teste de ataque para cada (SEG) tocado (V2.1–V2.7, mais rebaixamento, inflado,
  recorte e AAD)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
