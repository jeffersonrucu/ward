# Fase 16 — Entrega: tipo → variáveis de ambiente (D10)

## Implementado
- **`ward-core::entrega`** (público; sem dependência nova, sem `unsafe`, sem E/S):
  - `montar(&Entrada, &mut dyn Recursos) -> Result<Entrega, ErroEntrega>`: monta a
    lista exata de D10, na ordem: `WARD_ENTRADA`/`WARD_GRUPO`/`WARD_AMBIENTE`, depois as
    variáveis do tipo, os extras (`WARD_<CAMPO>`) e os anexos (`WARD_ARQUIVO_<NOME>`),
    com o nome de `entrada::nome_variavel` (Fase 15).
  - Trait **`Recursos`**, que quem chama implementa (o daemon na Fase 26; um falso nos
    testes). Assim o núcleo continua sem Linux:
    - `codigo_totp(semente)`: código atual; a semente nunca vira variável;
    - `arquivo(nome, conteudo)`: grava o anexo ou a chave (`nome = "chave"`) em
      `<rt>tmp/` e devolve o caminho;
    - `tunel(servidor, host, porta)`: abre o túnel do `banco` e devolve a porta local.
      Com túnel, host/porta viram `127.0.0.1:<local>` em todas as variáveis e URLs.
  - Os 9 tipos:
    - `login`: `WARD_TOTP` só com semente;
    - `banco`: os 5 motores;
    - `servidor` e `ftp/sftp`: `WARD_PASS` **ou** `WARD_ARQUIVO_CHAVE` +
      `WARD_PASSPHRASE`;
    - `api`: `WARD_URL` opcional, `WARD_TOKEN` e a variável escolhida;
    - `aws`: session token e role opcionais;
    - `smtp`;
    - `genérico`: só extras e anexos;
    - `nota segura`: `ErroEntrega::TipoSemEntrega`, cujo `Display` é exatamente
      "tipo sem entrega".
  - URLs do banco:
    - postgres: `postgresql://u:s@h:p[/base]`;
    - mysql: `mysql://…[/base]`;
    - redis: `redis://…[/n]`, só quando a base é número;
    - mongo: `mongodb://…[/base]`;
    - sqlserver: `sqlserver://u:s@h:p[?database=base]`.

    Usuário, senha e base vão com percent-encoding (RFC 3986, hex maiúsculo, tudo fora de
    `A-Za-z0-9-._~` é codificado). Host IPv6 ganha colchetes. `SQLCMDSERVER` sai como
    `host,porta`.
  - `Entrega` / `Variavel`: `nome()`, `expor()`, `e_segredo()` (marca senha, token, URL
    com senha e passphrase; o TOTP e o caminho de arquivo não, conforme D11), `valor(nome)`
    e `nomes()`.
- `entrada.rs`: só re-exporta `variaveis_do_tipo`, `VARIAVEIS_COMUNS` e
  `anexos_do_tipo`, com `#[cfg(test)]` e `pub(crate)`, para o teste de coerência.
- `lib.rs`: só a linha `pub mod entrega;` e um comentário.

## Segurança
- Invariantes tocadas:
  - **S8 / V2.28:** todo valor fica num `Zeroizing<String>`. As URLs com senha são
    alocadas uma vez só, no tamanho exato (`juntar` calcula o tamanho antes), então não
    sobra cópia parcial do segredo depois de uma realocação. `Entrega` e `Variavel` têm
    `ZeroizeOnDrop` e não têm `Clone`.
  - **S13 / V2.27:** o `Debug` de `Entrega`/`Variavel` mostra só `NOME=••••`, e nenhum
    valor aparece, nem os não secretos. `ErroEntrega`/`ErroRecurso` só têm
    `&'static str`.
  - **S1/S3 (preparação para o M4):**
    - percent-encoding em usuário, senha **e base**. Sem ele, uma base
      `app?host=evil.invalid` mandaria o libpq (e a senha) para outro host, e uma senha
      com `@`/`/`/`#` mudaria o destino;
    - extra nunca vira variável do sistema: `LD_PRELOAD` vira `WARD_LD_PRELOAD`, e
      `PGPASSWORD`/`DATABASE_URL` como extra não sobrescrevem as do tipo;
    - conferência final da lista: nome em `[A-Z_][A-Z0-9_]*`, sem `LD_*`, sem repetido,
      sem nulo em valor;
    - o que vem de `Recursos` também é conferido: TOTP com 6 a 8 dígitos, caminho não
      vazio e sem nulo, porta do túnel diferente de 0.
  - **S2 (preparação):** `nota segura` recusa antes de chamar qualquer recurso, então não
    cria arquivo. A chave privada vai só para arquivo e nunca para o ambiente. Se a
    montagem falhar depois de criar um arquivo, quem chama apaga (está documentado no
    módulo; o `ward run` da Fase 26 limpa em toda saída).
- Testes de ataque (`crates/ward-core/src/entrega/testes.rs`):
  - `ataque_senha_usuario_ou_base_mudam_o_destino_do_url`: os 5 motores, com
    senha/usuário/base cheios de `@ : / ? # & =`;
  - `ataque_extra_vira_variavel_do_sistema`: `LD_PRELOAD`, `PATH`, `PGPASSWORD`,
    `DATABASE_URL` e o anexo `LD_AUDIT`;
  - `ataque_recurso_devolve_lixo`: TOTP com `\n`, letras, nulo, curto ou longo, caminho
    vazio ou com nulo, porta 0, e recurso que falha, com o erro sem segredo;
  - `ataque_debug_e_erros_sem_segredo`;
  - `defesa_conferir_recusa_lista_invalida`;
  - `entrega_zera_ao_sair_e_nao_clona`: em tempo de compilação;
  - `url_alocada_uma_vez_no_tamanho_exato`.
- Funcionais:
  - `v2_14_entrega_esperada_do_contrato`: as 11 linhas da tabela, comparando nomes e
    valores exatos, nada a mais e nada a menos; inclui o túnel pelo servidor certo, a
    chave e o `app.env` com o conteúdo certo nos arquivos, e "tipo sem entrega" com zero
    chamadas;
  - `v2_14_comuns_vem_primeiro_e_na_ordem`;
  - `v2_15_todos_os_motores`: os 5 motores, com `falso:DB/01` → `falso%3ADB%2F01`;
  - `v2_16_extra_e_anexo_normalizados`: `WARD_API_KEY`,
    `WARD_ARQUIVO_CHAVE_ACESSO_PEM` e a recusa de repetidos ao salvar;
  - `variaveis_do_tipo_batem_com_as_reservadas`: toda `WARD_*` do tipo está na lista
    reservada da validação. É o elo com a Fase 15: um extra nunca colide;
  - `marca_de_segredo_bate_com_o_valor`;
  - `opcionais_ausentes_ficam_de_fora`;
  - `redis_com_base_numerica_vai_no_url`;
  - `host_ipv6_vai_entre_colchetes_no_url`;
  - `percent_encoding_de_todos_os_bytes`;
  - `extras_entram_em_todo_tipo_que_tem_entrega`.
- Mutações conferidas e revertidas, ambas pegas:
  - base sem encoding: `ataque_senha_usuario_ou_base…` falha;
  - checagem de `LD_` desligada: `defesa_conferir…` falha.
- Dependências novas: nenhuma. O percent-encoding é feito à mão, porque não é
  criptografia, e tem teste de todos os bytes especiais e do UTF-8.

## Ficou de fora
- **`WARD_TOTP` igual ao `oathtool` (V2.14):** o código vem de `Recursos::codigo_totp`.
  Aqui o teste usa um código falso e confere que a semente certa foi passada. Não liguei
  o `crate::totp` da Fase 18, que roda em paralelo e cuja API ainda pode mudar, e o
  `oathtool` não está instalado. Quem fecha é a Fase 26, quando o daemon implementar
  `Recursos` com `totp::Totp`, ou o orchestrator, com um teste depois que a 18 fechar.
- Criar e apagar os arquivos temporários e abrir o túnel de verdade: Fases 26 e 29 (o
  núcleo não faz E/S).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked entrega` (18 testes) | 0 |
| `cargo clippy -p ward-core --all-targets --locked -- -D warnings` | 0 |
| mutações (base sem encoding; sem checagem `LD_`) e reversão | 101 → 0 |
| `make check` | 0 ("tudo limpo") |

## Problemas descobertos
- **Decisões minhas, fora do texto de D10, para o orchestrator confirmar:**
  - `SQLCMDSERVER` sai como `host,porta`, a sintaxe do `sqlcmd`;
  - a base do sqlserver vai no URL como `?database=`;
  - no redis, a base só entra no URL se for número. D10 não define `REDIS_USER`, então o
    usuário só vai no URL;
  - `WARD_TOTP` e `WARD_ARQUIVO_*` não são marcados como segredo (D11: o código não é
    mascarado; o caminho não é segredo).
- **A validação aceita `base` com qualquer caractere de linha** (`?`, `#`, `/`). A
  entrega neutraliza isso com percent-encoding, mas variáveis como `PGDATABASE` recebem o
  texto cru. Isso é inofensivo para os clientes, que tratam o valor como nome. Talvez
  valha restringir o formato da base na validação (Fase 15). Não mexi nela.
- `variaveis_do_tipo` está correta só porque os tipos sem variável `WARD_*` própria
  (banco, aws, smtp) usam nomes sem `WARD_`. O teste de coerência quebra se alguém
  acrescentar uma `WARD_*` num tipo sem atualizar a lista.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/entrega.rs` | criado |
| `crates/ward-core/src/entrega/testes.rs` | criado |
| `crates/ward-core/src/entrada.rs` | alterado (só a re-exportação `#[cfg(test)]` de 3 nomes) |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod entrega;`) |
| `docs/mission/handoffs/phase-16.md` | criado |

`Cargo.toml` e `Cargo.lock` não foram alterados, nem os arquivos da Fase 18 (`totp*`,
`gerador*`).

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (valores de D15; nada em `.dev/` tocado)
- [x] Teste de ataque para cada (SEG) tocado (S8/V2.28, S13/V2.27, mais URL, variável do sistema e recurso malicioso)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
