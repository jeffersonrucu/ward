# Fase 15 — Modelo de entrada

## Implementado
- **`ward-core::entrada`** (público), sem dependência nova e sem `unsafe`:
  - **Enums com identificador estável e rótulo em português:** `TipoEntrada` (os 9 tipos:
    `login`, `banco`, `servidor`, `api`, `aws`, `smtp`, `ftp-sftp`, `nota-segura`,
    `generico`; rótulos `ftp/sftp`, `nota segura`, `genérico`), `Ambiente`
    (`dev|stage|prod`), `NivelAcesso` (`bloqueado|com-aprovacao|livre`, padrão
    `ComAprovacao`, `mais_restritivo` para D19), `Motor` (5 motores, com porta padrão),
    `SegurancaSmtp` (`nenhuma|starttls|tls`) e `Protocolo` (`ftp|sftp`). Os identificadores
    batem com os de `dev/fixtures/entradas.json`.
  - **Dados por tipo:** `Login`, `Banco` (túnel = nome de um `servidor`), `Servidor`
    (salto = nome de um `servidor`), `Api`, `Aws` (access key ID não é segredo, como no
    M0), `Smtp`, `FtpSftp`, `NotaSegura` e `Generico`. `Autenticacao` é
    `Senha(Segredo)` | `Chave { chave_privada, passphrase }`, então "senha e chave" não
    tem como existir numa entrada validada.
  - **`Segredo`** (`entrada/segredo.rs`): bytes que zeram ao sair (`Drop` → `zeroize`, que
    zera a alocação inteira) e marca `ZeroizeOnDrop`. Não tem `Clone`, `PartialEq`, `Hash`
    nem `Deref`. `Debug` = `Segredo(••••)` e `Display` = `••••`, sem tamanho. O acesso é
    explícito, por `expor()` e `expor_texto()`. Todo campo secreto, extra secreto e
    conteúdo de anexo é `Segredo`.
  - **`Entrada`:** campos privados, só nasce por `Rascunho::validar`. Getters,
    `segredos()` (V2.13: campos do tipo, extras secretos e todos os anexos, com a origem
    de cada um), `referencia()` (salto ou túnel), `definir_nivel` (só o daemon chama,
    depois do PIN), `substituir_por` (edição: mantém o nível e recusa mudar o tipo) e
    `conferir_no_cofre(outras)`. Esta última confere o nome único e se túnel e salto
    apontam para um `servidor` existente, sem autorreferência e sem cadeia. `Display`
    mostra só `nome (tipo, grupo, ambiente)`.
  - **Validação** (`entrada/validacao.rs`): `Rascunho` (o formulário: textos crus e
    segredos em `Segredo`, **sem campo de nível**) → `validar()`. Ela junta todos os
    erros e devolve `ErrosValidacao`. Cada `ErroValidacao` tem só `&'static str`: o
    campo, a posição na lista e o motivo em português. Nenhum valor digitado entra em
    erro, por construção. Regras:
    - nome da entrada em slug (`[a-z0-9][a-z0-9._-]*`, até 64);
    - `grupo` e `ambiente` obrigatórios; `ambiente` exato em `dev|stage|prod`;
    - obrigatórios por tipo:
      - login: url, usuário e senha;
      - banco: motor, host, usuário e senha;
      - servidor e ftp/sftp: host, usuário e senha **ou** chave;
      - api: token;
      - aws: access key ID, secret e região;
      - smtp: host, usuário, senha e segurança;
      - nota: texto;
      - genérico: pelo menos um extra;
    - porta vazia usa a padrão (motor, protocolo, segurança ou 22);
    - motor fora da lista é recusado;
    - passphrase só com chave;
    - `ftp` não aceita chave;
    - chave privada precisa ter cara de PEM/OpenSSH;
    - semente TOTP em base32 maiúsculo, de 16 a 128 caracteres;
    - URL `http(s)://` sem espaço;
    - limites de tamanho em todo campo e lista.
  - **`nome_variavel`** (D10): `api-key` → `API_KEY`, `chave acesso.pem` →
    `CHAVE_ACESSO_PEM`. Dois extras (ou dois anexos) que normalizam igual são recusados ao
    salvar (V2.16, parte da validação). Também é recusado o extra que vira uma variável
    que o tipo já entrega (`variaveis_do_tipo`, `VARIAVEIS_COMUNS`, prefixo
    `ARQUIVO_`) e o anexo `chave` no servidor e no ftp/sftp (`WARD_ARQUIVO_CHAVE`).
- `lib.rs`: só a linha `pub mod entrada;`. Sem conflito com o `pub mod gravacao` da
  Fase 14, que já estava lá.

## Segurança
- Invariantes tocadas:
  - **S8 / V2.28:** `Segredo` zera ao sair, e `Entrada`, `Dados`, `Autenticacao`,
    `CampoExtra` e `Anexo` têm a marca `ZeroizeOnDrop`. Ela vale porque todo valor secreto
    está num `Segredo`. Nenhum tipo do módulo tem `Clone`, nem os rascunhos. Os
    construtores tomam posse do `String`/`Vec` sem copiar. Se a validação falha, o
    rascunho cai e os segredos zeram. Um extra não secreto passa por `Segredo` até ser
    validado.
  - **S13 / V2.27:** `Debug`/`Display` de segredo imprimem `••••`. O `Debug` dos rascunhos
    mostra só o tipo. Os erros só têm textos fixos.
  - **S9:** o rascunho não tem nível, então entrada nova sempre começa "com aprovação".
    `substituir_por` mantém o nível de antes: editar não desbloqueia nem libera. Trocar o
    nível é `definir_nivel`, que o daemon só chama com PIN e papel app (Fase 22).
  - **S3/S1 (preparação para o M4):** host, usuário, salto e túnel não começam por `-`.
    Assim, um `-oProxyCommand=…` não vira opção do `ssh`.
  - **Entrada não confiável (app e extensão):** a validação recusa:
    - caractere de controle, separador de linha e caracteres de formatação Unicode
      (bidi override, largura zero, tags, BOM), que forjariam o texto da janela de
      aprovação e da lista;
    - nulo em segredo;
    - tamanhos acima dos limites;
    - nome de anexo com `/`, `\`, `.` ou `-` no começo;
    - variável do `api` em `LD_*`, `WARD_*`, `BASH_FUNC_*`, `PATH`, `BASH_ENV`,
      `SSHPASS`, `GIT_SSH_COMMAND`, `NODE_OPTIONS` etc.
- Testes de ataque escritos (`crates/ward-core/src/entrada/testes.rs`):
  - `v2_27_debug_e_display_sem_segredo`: `{:?}`, `{:#?}` e `{}` de rascunho, entrada,
    dados, extras, anexos e `segredos()` de um de cada tipo, sem nenhum dos 14 segredos
    de teste;
  - `v2_27_erros_nao_carregam_valor_do_usuario`: o segredo vai parar em nome, grupo,
    ambiente, motor, host, porta, base, usuário, túnel, tags, extras e anexo, mais um
    segredo com nulo. Os mais de 10 erros (`Display`, `Debug`, `{:#?}`) e o erro de
    conferência não o repetem;
  - `v2_28_tipos_de_segredo_zeram_e_nao_clonam`: em tempo de compilação, a ambiguidade
    de trait quebra o build se qualquer um dos 21 tipos (os do módulo, `SenhaMestra` e
    `CofreAberto`) ganhar `Clone`. Também confere `ZeroizeOnDrop` nos tipos do módulo e
    em `cripto::Chave`;
  - `ataque_host_e_usuario_viram_opcao_do_ssh`: `-oProxyCommand=…`, `-p` e espaço em
    host e usuário do servidor, sftp, banco e smtp, e `-J` como salto;
  - `ataque_texto_invisivel_forja_a_janela_de_aprovacao`: U+202E, U+200B, U+2066, U+2028,
    U+FEFF, ESC, CR, LF, U+E0041 e U+00AD em grupo, projeto, nome e valor de extra e
    nome de anexo;
  - `ataque_segredo_com_nulo_ou_binario`;
  - `ataque_variavel_perigosa_no_api`;
  - `ataque_extra_ou_anexo_sobrescreve_variavel_do_tipo`: um extra `PASS` no login
    viraria `WARD_PASS`, e um anexo `chave` no servidor viraria `WARD_ARQUIVO_CHAVE`;
  - `ataque_nome_de_anexo_sai_do_diretorio`;
  - `ataque_tamanhos_excessivos`: senha acima de 64 KiB, nota acima de 1 MiB, anexo
    acima de 16 MiB, 65 extras, 33 anexos, 17 tags, TOTP com 129 caracteres, grupo e URL
    longos;
  - `ataque_nivel_nao_muda_ao_salvar_nem_ao_editar`.
- Funcionais:
  - `v2_12_*` (6 testes): cada obrigatório de cada tipo, sem grupo e sem ambiente em
    todos os tipos, ambiente fora da lista (`PROD`, `" dev"`, `prod\n`…), servidor e
    sftp com as duas autenticações ou com nenhuma, motor fora da lista, formatos e
    junção de todos os erros;
  - `v2_13_campos_secretos_de_cada_tipo`;
  - `v2_16_normalizacao_e_repetidos`;
  - `cofre_de_teste_do_contrato_valida_e_confere`: as 20 entradas de D15 validam e
    passam no `conferir_no_cofre`;
  - `conferir_no_cofre_referencias`, `porta_vazia_usa_a_padrao`,
    `nivel_combinado_e_nomes`, `um_de_cada_tipo_valida`.
- Mutações conferidas à mão, todas pegas e revertidas:
  - `impl Clone for Segredo`: o build dos testes falha (E0283);
  - `Debug` do `Segredo` mostrando o valor: `v2_27_debug_e_display_sem_segredo` falha;
  - `host_valido` sem a checagem de `-`: `ataque_host_e_usuario…` falha;
  - `PASS` fora das variáveis do login: `ataque_extra_ou_anexo…` falha.
- Dependências novas: nenhuma. Só `zeroize`, que já existia.

## Ficou de fora
- **Serialização da entrada no corpo do cofre.** O plano não diz qual fase faz, e a
  Fase 19 (`make dev-cofre`) e a 22 (daemon) precisam dela. Fica para o orchestrator ou o
  planner. Sugiro relê-la sempre por `Rascunho::validar` (validar de novo o que vem do
  disco) e pôr `definir_nivel` depois.
- **Entrega D10 (variáveis de ambiente):** é da Fase 16. Aqui ficam só `nome_variavel` e
  as listas de nomes reservados (`variaveis_do_tipo`, `VARIAVEIS_COMUNS`,
  `anexos_do_tipo`, `pub(crate)`), e a Fase 16 deve manter as duas coisas iguais.
- **Semântica de edição de segredo** (campo secreto ausente quer dizer "manter o atual"?):
  depende do protocolo e do app (Fases 20, 22 e 33). Aqui `substituir_por` troca a
  entrada inteira, já validada.
- Decodificar base32 e a chave privada: é da Fase 18 (TOTP) e do `ssh`/agente (Fase 27).
  Aqui só se confere o formato.
- Mexer em renomear ou apagar uma entrada que é referenciada: é operação do daemon (Fase
  22). `referencia()` existe para isso.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked entrada` (24 testes, ~0,15 s; `CARGO_TARGET_DIR` no scratchpad para não disputar o lock com a Fase 14) | 0 |
| mutações (`Clone`, `Debug`, host `-`, `PASS` reservado) e reversão | 101 → 0 |
| `cargo clippy -p ward-core --all-targets -- -D warnings`, filtrado para `entrada` | 0 avisos em `entrada` |
| `make check` | 2 (para no passo 2/6 por dois avisos de clippy em `crates/ward-core/src/gravacao/testes.rs`, da Fase 14) |
| `make test` | 2 (61 ok; os 3 que falham são `gravacao::testes::v2_9_*` e `v2_10_v2_11_*`, da Fase 14 em andamento) |
| `make audit` / `make deny` / `make gitleaks` | 0 / 0 / 0 |
| `cargo fmt --check` (passo 1/6 do `make check`) | 0 |

## Problemas descobertos
- **O `make check` não fecha limpo por causa da Fase 14**, que rodou em paralelo: há dois
  erros de clippy (`for_unbounded_range` e `needless_range_loop`) e três testes de
  gravação falhando em `crates/ward-core/src/gravacao/testes.rs`. Não toquei nesses
  arquivos. Depois que a 14 fechar, o `make check` precisa rodar de novo.
- **V2.28 e o `CofreAberto` (formato, Fase 13):** ele zera a chave ao sair, porque o
  campo `chave_dados` é `Zeroizing`, e não tem `Clone`. Mas não tem a marca
  `ZeroizeOnDrop`, que o V2.28 pede "em tempo de compilação". Não mexi no `formato`, que
  está fora da minha área nesta rodada. O teste confere `ZeroizeOnDrop` em
  `cripto::Chave`, e confere que nem `CofreAberto` nem `SenhaMestra` têm `Clone`. Falta
  acrescentar `impl ZeroizeOnDrop for CofreAberto {}` e `for SenhaMestra {}` no
  `formato.rs`, numa fase que possa mexer nele. `cripto::Chave` (`Zeroizing<[u8; 32]>`)
  tem `Clone`, mas o clone também zera, então isso atende "não implementam `Clone` sem
  zerar".
- **Decisões minhas, fora do escopo, para o orchestrator confirmar** (fui pelo lado mais
  seguro):
  - Os obrigatórios por tipo: o design só diz "nome, grupo e ambiente". A URL do `api` é
    opcional, e o genérico exige pelo menos um extra.
  - O nome da entrada é slug minúsculo.
  - Salto e túnel têm **um nível só**. O servidor de salto não pode ter salto, o
    servidor do túnel não pode ter salto, e um servidor usado como salto ou túnel não
    pode ganhar salto. Assim um uso envolve no máximo as duas entradas de D19. Se o
    usuário quiser cadeia de saltos, D19 precisa dizer como fica o nível combinado de três
    entradas.
  - A lista de variáveis proibidas no `api`.
  - Os limites de tamanho: 64 KiB por segredo, 1 MiB por nota, 16 MiB por anexo, 64
    extras, 32 anexos e 16 tags.
  - A semente TOTP precisa chegar normalizada (maiúsculas, sem espaço). O app normaliza
    antes de mandar.
  - O `usuário` do login aceita espaço. Nos outros tipos, não aceita espaço nem `-` no
    começo.
- **Fixture `acme-ssh-interno` e `-aprov`:** o contrato diz "chave de
  `acme-ssh-chave-prod`". O modelo não tem referência de chave, então a Fase 19 copia a
  chave em cada entrada, e é assim que o teste das 20 entradas está montado.
- Durante a fase, alguém rodou `cargo fmt` no workspace e reformatou meus arquivos em
  andamento. Não causou dano. Daqui em diante formatei só a minha árvore, com
  `rustfmt --edition 2024 crates/ward-core/src/entrada.rs`.
- O V2.16 inteiro (o ambiente com `WARD_API_KEY` e `WARD_ARQUIVO_CHAVE_ACESSO_PEM`) só
  fecha na Fase 16. Aqui fica a parte de "recusados ao salvar".

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/entrada.rs` | criado |
| `crates/ward-core/src/entrada/segredo.rs` | criado |
| `crates/ward-core/src/entrada/validacao.rs` | criado |
| `crates/ward-core/src/entrada/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod entrada;`) |
| `docs/mission/handoffs/phase-15.md` | criado |

`Cargo.toml` e `Cargo.lock` não foram alterados.

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): rodado, mas sai 2
  por causa do `gravacao` da Fase 14. A parte desta fase está limpa (fmt, clippy em
  `entrada`, 24 testes, audit, deny, gitleaks).
- [x] Só cofre de teste e credenciais falsas usados (valores de D15; a chave privada
  falsa é montada em tempo de execução, e nada em `.dev/` foi tocado)
- [x] Teste de ataque para cada (SEG) tocado (V2.27, V2.28, mais injeção de opção,
  texto invisível, variável perigosa, nome de anexo, tamanhos e nível na edição)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
