# Fase 19e — Achados da reauditoria do M2 (D53–D55)

## Implementado
- **D53, geração queimada ao selar** (`formato.rs`, `gravacao.rs`, `corpo.rs`):
  - `gravar_entradas` serializa, sela e só então chama `CofreAberto::queimar(n+1)` (antes `gravou`, `pub(crate)`). Isso acontece antes de criar o temporário, e o contador fica em n+1 se a escrita, o fsync, o rename ou o fsync do diretório falharem. Em `u64::MAX` a recusa vem antes de selar: não toca no disco e não queima. Um corpo que não serializa também não queima.
  - `corpo::abrir_entradas_desde(arquivo, senha, geracao_minima, geracao_queimada)`: com disco abaixo do mínimo dá `Retrocesso`, como antes. Com sucesso, o contador fica em max(disco, queimada).
  - `gravacao::trocar_senha(destino, atual, nova, geracao_minima, geracao_queimada)` sela com max(disco, queimada) + 1.
  - `CofreAberto::geracao()` devolve "a última lida ou selada". Continua sem setter público.
  - Atualizei os chamadores (testes do corpo). O exemplo `dev-cofre` usa só `abrir_entradas`, que não mudou. O formato do arquivo também não mudou, então o `.dev/cofre-teste.ward` não precisou de regravação, mas `make teste-dev-cofre` o regravou mesmo assim (geração 1).
- **D54, máscara** (`mascara.rs`):
  - `visitar`: todo valor secreto de entrega com `\n` passa também por `linhas`. Com isso, `ConstrutorMascara::new().entrega(&g).construir()` sozinho já cobre o token de `api` de várias linhas.
  - Nova `Gerador::formas_de_texto`, a mesma para o segredo inteiro e para cada linha. Cada forma só entra quando difere do literal:
    - JSON padrão, com e sem `\/`;
    - Go e `ensure_ascii`;
    - PHP (`JsonEscapado::Php`: `\/` + `\uXXXX`, só quando há `/` e caractere fora do ASCII);
    - aspas duplas;
    - aspas simples (`'` vira `'\''`, só quando há `'`);
    - `printf %q` (só ASCII visível e espaço; os especiais sempre, `#` no começo, `~` no começo ou depois de `=`/`:`, conferido contra o bash 5.2.21 real).
  - `linhas`: cada linha de 6+ (sem `\r` e aparada) ganha as formas de texto, sem base64 nem miolos. Os campos de 6+ de uma linha com tab viram valor de linha (segredo inteiro).
  - `valor_de_dois_pontos` agora aceita `chave : valor` (a chave do YAML é aparada antes do `:`). `chave:\tvalor`, `"k":"v"` e a chave JSON com qualquer caractere menos `"` já funcionavam; os testes confirmam.
- **D54, oráculo** (`dev/variacoes.py`): reescrevi a parte de variações a partir da tabela única de D54, sem ler o `mascara.rs`:
  - formas de texto por linha, sem base64 nem miolos;
  - PHP, aspas simples e `printf %q`;
  - campos de tab e o parser de linha novo;
  - anexo inteiro só com literal, sem a quebra final, URL e base64;
  - Basic só para segredo de campo;
  - formas com quebra de linha ficam fora da lista (`listavel`).

  Entrou também `da_entrada(e)` (variações por entrada, usada pelo teste cruzado). Atualizei a docstring. O oráculo continua sem importar nem chamar a máscara (o teste `Independencia` segue passando).
- **D54, teste cruzado** (V2.57):
  - `dev/testes/dev-cofre.sh` › `cruzado`: um Python carrega o `variacoes.py` e manda pelo pipe `entrada<TAB>tipo<TAB>hex` para o modo novo `dev-cofre --cruzado` (stdin). Esse modo abre o cofre de teste, monta a máscara de cada entrada com a entrega dela (recursos falsos) e exige `••••` exato para cada padrão. O `DATABASE_URL`, que depende do host e da porta de quem roda, só precisa ter `••••` e nenhum trecho de 6+ de segredo. Resultado: 473 padrões de 21 entradas. Não grava arquivo e não imprime padrão.
  - Teste Rust `v2_57_oraculo_e_mascara_com_valores_de_d54`, porque as fixtures não têm caractere especial: roda o oráculo pelo python com valores falsos que têm `$&<>'/ç`, tab e linhas de chave e valor, e confere que mais de 200 padrões saem `••••`.
- **D55, URL** (`entrada/validacao.rs`): domínio cujo último rótulo começa por dígito é recusado (inclui todo `0x…`). A regra vale ao salvar e na leitura do corpo, porque é o mesmo `url_valida`.
- **CLAUDE.md**: mexi só na linha do `make dev-variacoes` (tabela de D54, teste cruzado).

## Segurança
- **Invariantes tocadas:**
  - S7/S9 (D53): nenhum par de arquivos selados tem a mesma geração, e a cópia de uma gravação que "falhou" é recusada pelo mínimo da seguinte.
  - S4 (D54): formas de texto por linha, entrega de várias linhas, formas novas, orçamento pela mesma função.
  - S13 (D55): host lido como IPv4 pelo navegador.
- **Testes de ataque.** Cada repro do auditor virou teste, e os três repros (cópia em `scratchpad/ataque-19e`, apontando para este worktree, contra uma cópia do cofre de teste) mostram os ataques falhando.
  - `geracao.rs`:
    - `gravacao/testes.rs` › `v2_53_fsync_do_diretorio_falha_e_a_geracao_nao_repete`: o ataque 0300, A em n+1, B em n+2, A no lugar de B dá `Retrocesso` sem valor, as gerações n, n+1 e n+2 são distintas, reabrir A com (n, n+2) leva a n+3, e `trocar_senha` com queimada n+5 sobre n+3 leva a n+6;
    - `v2_44_falha_em_cada_ponto_queima_a_geracao` (injeção do V2.9 nos 6 pontos);
    - `d53_corpo_que_nao_serializa_nao_queima` e `d53_queimada_menor_que_o_disco_nao_desce`;
    - `corpo/testes.rs` › `v2_44_geracao_sobe_e_retrocesso_e_recusado` (reescrito: o rename que falha queima n+2, a troca sela n+3);
    - `v2_44_geracao_esgotada_nao_toca_no_disco`.
    - Controle: com a queima de volta para depois da gravação, `v2_53_…` e `v2_44_falha_…` reprovam.
  - `desvio.rs`, todos em `mascara/testes.rs`:
    - `v2_54_entrega_de_varias_linhas_por_linha`: `head -n1`/`tail -n1` do bash real, CR e aparada, só `.entrega()` e `da_entrada`, mais o controle de que o inteiro sozinho não cobria;
    - `v2_55_formas_de_texto_por_linha`: `json.dumps(splitlines())` e `json.dumps({l: linha1})` do python real, `declare -p` de `sed -n 3p` do bash real e o Go à mão. Confere também a forma decodificada e a saída limpa com `\u00e7`;
    - `v2_56_formas_novas_e_parser_de_linha`:
      - `printf %q`, `${X@Q}`, `set` e `set -x` reais;
      - PHP;
      - as 6 linhas do parser, inteiras e só o valor;
      - `cut -f1`/`-f2` reais;
      - os casos de RA3 (`%q` com controle, `expand`, base64 de linha) só registrados;
    - `v2_56_printf_q_igual_ao_bash`: 475 valores contra o bash real, mais o `@Q`;
    - `d54_formas_novas_so_quando_mudam_e_linha_sem_base64` e `formas_escapadas_so_quando_mudam` (atualizado);
    - `v2_57_oraculo_e_mascara_com_valores_de_d54`.
    - Controle: sem as aspas simples, `v2_56_formas_…` e `v2_57_…` reprovam.
    - O que sobra do `desvio.rs`: `declare -p` do valor inteiro com controle (`$'…'`, RA3) e Go + `ensure_ascii` juntos (fora de D54, ver "Problemas").
  - `url.rs`: `entrada/testes.rs` › `v2_58_ultimo_rotulo_com_digito` (os 5 de D55 mais `x.example.9z` e afins, todos os recusados do repro, e os aceitos da V2.58/V2.47); `corpo/testes.rs` › `v2_58_corpo_forjado_com_ultimo_rotulo_numerico` (login e api forjados).
  - Oráculo: `dev/testes/variacoes_teste.py` › `D54TabelaUnica.*`, com 9 testes:
    - as 8 formas de texto da linha;
    - sem base64 nem miolos por linha;
    - `%q` só em ASCII, PHP e aspas simples só quando mudam;
    - campos de tab e parser novo;
    - anexo inteiro sem JSON;
    - `%q`/`@Q` iguais ao bash real.
  - Teste cruzado: `dev/testes/dev-cofre.sh` › `cruzado`. Controle: com base64 por linha posto no oráculo, ele reprova e aponta `acme-ssh-chave-prod`.
  - Orçamento (V2.36/V2.43 refeitos): `limites/testes.rs` roda com a tabela nova. O extra JSON de teste ganhou linhas com `'`, `/ação`, tab e `chave : valor`. O custo medido continua ≥ o real, a máscara fica ≤ 8 MiB e as 21 entradas de D15 continuam aceitas (`make teste-dev-cofre`). Nenhuma entrada de D15 foi recusada, então não precisei parar.
- **Dependências novas:** nenhuma.

## Ficou de fora
- D56 (`sftp -l`): é da Fase 27, como manda o plano.
- O daemon (estado com `vista`/`queimada`) é da Fase 22. A API do núcleo está pronta.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 1500 make ferramentas` | 0 |
| `timeout 1500 make dev-cofre` | 0 |
| `cargo test -p ward-core --lib -- geracao queima v2_53 d53 troca_de_senha` | 0 |
| mutação (queima depois da gravação) + `cargo test … v2_53 v2_44_falha` | 101 (esperado: 2 reprovados) |
| `cargo run --bin url` / `desvio` / `geracao` (repros, cópia do cofre de teste) | 0 |
| `cargo test -p ward-core --lib -- limites mascara entrega` (1.ª vez, sem o filtro de UTF-8) | 101 (orçamento estourado; ver Problemas) |
| `cargo test -p ward-core --lib -- limites::testes` | 0 |
| mutação (sem aspas simples) + `cargo test … v2_57 v2_56` | 101 (esperado) |
| `python3 -I -B dev/testes/variacoes_teste.py` | 0 (33 testes) |
| mutação no oráculo (base64 por linha) + `make teste-dev-cofre` | 1 (esperado: V2.57 reprova) |
| `timeout 3500 make check` (1.ª vez) | 2 (clippy: `duplicate_mod`, `needless_character_iteration`, `manual_is_multiple_of`; corrigidos) |
| `timeout 3500 make check` | 0 |
| `timeout 3000 make teste-dev-cofre` | 0 |

## Problemas descobertos
- **Formas de texto e campos de tab só em linha que é texto (UTF-8 válido).** Na 1.ª versão, com as formas e os campos de tab em toda linha, um anexo binário de ~200 KiB passava dos 4 MiB de D29. O `cheia()` do V2.36 deixou de caber, para todo tipo. O custo de um binário subia de ~8× para ~33× o tamanho: cada campo entre tabs virava segredo inteiro, com base64 e miolos. Restringi as duas coisas a linhas de texto e não mexi no orçamento. A linha binária continua mascarada pelo literal, como antes. Justificativa: `json.dumps`, `printf %q` e `cut -f` não produzem essas formas a partir de bytes que não são texto. O oráculo segue a mesma regra. O planner deve confirmar em D54. Nenhuma entrada de D15 foi afetada.
- **Go + `ensure_ascii` juntos** (JSON "html-safe" do Python, `\u0026` e `\u00e7` no mesmo valor) não está em D54 e continua passando (repro `desvio.rs`, caso 4). Proponho RA3 ou uma decisão nova.
- **`declare -p` do valor inteiro de várias linhas** (`$'…\n…'`) deixa linhas com acento visíveis em octal. É o caso "controle → `$'…'`" de RA3; fica registrado.
- **Oráculo mais estreito no anexo inteiro e no Basic.** Antes, o `variacoes.py` gerava JSON, miolos e escapes do anexo inteiro, o base64 do segredo sem a quebra final e o Basic de extras. A máscara nunca gerou nada disso, e o teste cruzado (`••••` exato) reprovaria. Agora o oráculo segue a tabela, e as linhas do anexo continuam cobertas por linha.
- As fixtures não têm caractere especial, então na lista real o cruzado exercita pouco de D54. O teste Rust com valores falsos cobre isso. Para o M3, uma entrada de teste com `Ab3$k&<9>Zq!` e `'` ajudaria (muda as fixtures e o V2.29; fica para o planner).
- `v2_53_fsync_do_diretorio_falha_…` não reproduz como root, porque o root lê diretório 0300. O teste avisa e sai, e o caso fica coberto pela injeção de `v2_44_falha_em_cada_ponto_…`.
- `gravacao/testes.rs` inclui `corpo/cofre_teste.rs` de novo, com `#[allow(clippy::duplicate_mod)]` e motivo no comentário. Foi para não abrir a visibilidade do `corpo::testes`.
- O texto de ajuda do `dev-variacoes` no `Makefile` continua desatualizado (vem da 19d). A fase não podia tocar no `Makefile`.
- Para a Fase 22, conforme D53: um erro de `trocar_senha` depois de selar perde o `CofreAberto`, e o daemon tem de travar e reabrir com a `queimada` do estado. Gravar `queimada` antes de cada gravação é obrigação do daemon.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/formato.rs` | alterado (`queimar`, docs da geração) |
| `crates/ward-core/src/gravacao.rs` | alterado (queima ao selar, `trocar_senha` com queimada) |
| `crates/ward-core/src/gravacao/testes.rs` | alterado (V2.44 reescrita, V2.53, D53) |
| `crates/ward-core/src/corpo.rs` | alterado (`abrir_entradas_desde` com queimada) |
| `crates/ward-core/src/corpo/testes.rs` | alterado (V2.44, V2.58 forjado) |
| `crates/ward-core/src/entrada/validacao.rs` | alterado (D55) |
| `crates/ward-core/src/entrada/testes.rs` | alterado (V2.58) |
| `crates/ward-core/src/mascara.rs` | alterado (D54) |
| `crates/ward-core/src/mascara/testes.rs` | alterado (V2.54–V2.57, D54) |
| `crates/ward-core/src/limites/testes.rs` | alterado (extra de teste com as formas de D54) |
| `crates/ward-core/examples/dev-cofre/main.rs` | alterado (modo `--cruzado`) |
| `dev/variacoes.py` | alterado (tabela única de D54, `da_entrada`, docstring) |
| `dev/testes/variacoes_teste.py` | alterado (V2.57) |
| `dev/testes/dev-cofre.sh` | alterado (teste cruzado) |
| `CLAUDE.md` | alterado (só a linha do `make dev-variacoes`) |
| `docs/mission/handoffs/phase-19e.md` | criado |
| `.dev/cofre-teste.ward`, `.dev/variacoes.txt` | regenerados por `make teste-dev-cofre` (fora do git) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V2.44, V2.53–V2.58)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase (`ward-plataforma`, `wardd`, protocolo, CLI, `Makefile` e fixtures intocados)
