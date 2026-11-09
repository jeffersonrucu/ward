# Fase 22a — Fixture com caracteres especiais, Go com `ensure_ascii` e reauditoria da 19e (D58, D59, D61)

## Implementado
- **D59, 22.ª entrada do cofre de teste.** `delta-api-simbolos` (api, `delta`, `dev`, livre):
  - URL `https://api.teste.invalid`;
  - token `falso-Ab3$k&<9>Zq!'/ç-01`, variável `SIMBOLOS_TOKEN`;
  - extra secreto `SEGREDO_ASCII=falso-Ab3$k&<9>Zq!'`.

  Entrou em `dev/fixtures/entradas.json` e no espelho do contrato usado pelos testes do corpo (`corpo/cofre_teste.rs`, inclusive `SEGREDOS`). As contagens 21 → 22 mudaram em `corpo/testes.rs`, `protocolo/testes.rs`, `tests/corpo_memoria.rs` e `examples/dev-cofre/testes.rs`. O `make dev-cofre` aceita as 22 entradas: nenhuma passou do orçamento de D29.
- **D58, Go com `ensure_ascii`** (`mascara.rs`): nova forma de texto `JsonEscapado::GoAscii`, que é o `ensure_ascii` com `&`, `<` e `>` também em `\u00XX`.
  - Só entra quando o valor tem `&<>` e algum caractere fora do ASCII. Com U+2028/U+2029 sem `&<>`, a forma é igual à do `ensure_ascii` e não é repetida.
  - Vale no segredo inteiro, no valor de linha e por linha de texto, porque é a mesma `formas_de_texto`.
- **D61, reauditoria da 19e** (`mascara.rs`, passada por linha):
  - **JSON na linha**, em toda linha de texto e no segredo de uma linha só que, aparado, começa por `{` ou `[`. Vale para extra, campo e valor de entrega, todos pelo novo `Gerador::segredo_de_campo`.
    - A busca anda de string em string. Cada par `"chave"` + espaços/tabs + `:` + espaços/tabs + string vira valor.
    - A string é decodificada (`\"`, `\\`, `\/`, `\b\f\n\r\t`, `\uXXXX` com par substituto). Escape inválido ou substituto sozinho descarta o par.
    - Com 6+ caracteres, entra como valor de linha (segredo inteiro).
    - Não repete o valor que o parser de `chave: valor` já gerou.
    - O decodificado fica num terceiro buffer `Zeroizing` (`tmp3`), zerado ao sair.
  - **Comentário no fim** (YAML e dotenv):
    - valor sem aspas: o trecho até o primeiro espaço ou tab seguido de `#`, aparado;
    - valor entre aspas seguido de espaços/tabs e `#`: o miolo.
  - **Chave YAML composta** (`valor_de_chave_composta`): a chave vai até o primeiro `:` seguido de espaço, tab ou fim. Não pode ter `#` nem aspas e não começa por `-`, `{` ou `[`. Só gera candidato quando o valor difere do parser antigo, que continua valendo.
  - **`''` do YAML:** no valor entre aspas simples, também com comentário, `''` vira `'`.
  - **Campos de tab de linha que não é UTF-8:** cada campo de 6+ bytes entra só pelo literal, aparado e cru, e pelo percent (as visões do autômato). Não ganha forma de texto nem base64. O campo igual à linha ou à linha aparada não se repete, então o binário 0–255 do V2.36 não fica mais caro.
- **Oráculo** (`dev/variacoes.py`): as mesmas regras, escritas a partir do PRD e sem chamar a máscara.
  - `json_go_ascii`, com a condição literal de D58.
  - `valores_json`: string JSON por regex, decodificada pelo `json.loads(strict=False)`; valor que não vira UTF-8 é descartado.
  - `comentario_e_aspas` e `CHAVE_YAML_COMPOSTA`.
  - `campos_de_tab_binarios`, que dão só literal e URL.
  - JSON do segredo de uma linha que começa por `{`/`[`.
  - Docstring atualizada.
- **Teste cruzado e V3.29 na lista real** (`dev/testes/dev-cofre.sh`):
  - 22 entradas e `delta-api-simbolos` na tabela;
  - nova `simbolos_d59` com 14 formas calculadas fora do oráculo e conferidas na lista. As do shell vêm do bash real em `C.UTF-8` (`${X@Q}`, `declare -p`, `printf %q` do `SEGREDO_ASCII`). As de JSON vêm da biblioteca padrão do Python (literal, base64, percent, Go, PHP e Go + `ensure_ascii`), com a forma do contrato conferida por igualdade;
  - o cruzado agora passa 497 padrões de 22 entradas.
- **Orçamento (V2.36/V2.43 refeitos):** `limites/testes.rs` › `json_de_varias_linhas` ganhou 4 tipos de linha no pior caso:
  - par JSON no meio da linha, com objeto aninhado, `ç` e `&ç`;
  - YAML com comentário;
  - chave composta com `''` e comentário;
  - dotenv com comentário.

  O custo medido continua ≥ o real, a máscara fica ≤ 8 MiB e o par de túnel (D19) cabe.
- **`Makefile`:** só os textos de ajuda de `dev-cofre` (22 entradas, D59), `dev-variacoes` (tabela única de D54, D58, D61) e `teste-dev-cofre`.
- **`.gitleaks.toml`:** os dois valores de D59 na allowlist, cada um casado inteiro (`^…$`).

## Segurança
- **Invariantes tocadas:** S4, pela máscara (D58, D61). A tabela única segue igual na máscara e no oráculo, e o orçamento de D29 continua o mesmo: nenhum limite foi afrouxado e as 22 entradas de D15 cabem.
- **Testes de ataque** (todos com valores falsos montados no teste). Antes, os mesmos casos passavam pelo repro.
  - `crates/ward-core/src/mascara/testes.rs` › `v3_29_delta_api_simbolos_e_go_com_ensure_ascii`:
    - a forma do contrato;
    - o JSON html-safe + `ensure_ascii` e o `ensure_ascii` do python real;
    - `${V@Q}` e `declare -p` do bash real;
    - PHP;
    - `printf %q` do `SEGREDO_ASCII` (bash real);
    - os padrões do oráculo da entrada, um por um;
    - o custo cabe em D29.
  - `› v3_29_go_com_ensure_ascii_por_linha_e_linha_binaria`:
    - o D58 linha a linha de um segredo de várias linhas, pelo python real, e no valor de linha;
    - a forma só aparece quando muda;
    - a linha binária de anexo fica com o literal (com e sem `\r`), sem forma de texto, e sai `••••`.
  - `› v3_31_json_numa_linha`:
    - anexo `client.json` minificado, extra `{"user":…,"token":…}`, objeto aninhado e duas chaves na linha, valor com `ç\/`, todos pela saída real do `json.loads`;
    - escape inválido e substituto sozinho são descartados;
    - array sem chave só registrado (RA3);
    - o oráculo gera os mesmos padrões.
  - `› v3_32_yaml_dotenv_e_tab_de_linha_binaria`:
    - os 5 valores YAML como o `yq` imprime;
    - o dotenv pelo `source` real;
    - o anexo Latin-1 pelo `cut -f2`/`-f3` real;
    - o campo binário sem base64;
    - `iconv` só registrado (RA3);
    - custos dentro de D29;
    - o oráculo cobre.
  - `› d61_candidatos_so_quando_mudam`: nada se repete, e URL, comentário e flow do YAML não viram chave composta.
  - `dev/testes/variacoes_teste.py` › `D58D61.*` (10 testes): o oráculo com as formas novas, o "só quando muda", RA3 (array, flow, valor depois do `=` num campo binário) e o campo binário sem base64.
  - `dev/testes/dev-cofre.sh` › `simbolos_d59` e `cruzado` (V3.29 e V2.57 com 22 entradas).
  - **Controle por mutação:**
    - sem `json_da_linha` e sem `comentario_e_aspas` na passada por linha, `v3_31` e `v3_32` reprovam;
    - sem a forma `GoAscii`, os dois `v3_29` reprovam.
  - **Repro do auditor** (`contorno19e.rs`), copiado para `scratchpad/ataque-22a` e apontado para este worktree, com o original intocado:
    - todo caso de D61 sai `ok`: YAML com comentário, chave com espaço e com barra, dotenv, `''`, JSON de uma linha, aninhado, duas chaves, minificado, `cut -f2` e `cut -f3` Latin-1;
    - continuam vazando só os `repr()` do Python, o caso principal e os 11 do fuzz, que D61 manda para RA3.
- **Dependências novas:** nenhuma.

## Ficou de fora
- `repr()` do Python, `util.inspect` do Node, `iconv`, array JSON sem chave e flow do YAML: RA3, por D61 (só registrados nos testes).
- Linha do `make dev-cofre` no `CLAUDE.md` (ainda diz "21 entradas"): o arquivo não estava na lista da fase (ver Problemas).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 1500 make ferramentas` (3 vezes) | 2 (download do gitleaks: GitHub respondeu 504; ver Problemas) |
| `timeout 600 make dev-servicos` | 0 |
| `timeout 1500 make dev-cofre` | 0 |
| `cargo test --locked -p ward-core --lib -- mascara limites` (antes das fixtures) | 0 |
| `cargo test --locked -p ward-core` (com 22 entradas) | 0 |
| `python3 -I -B dev/testes/variacoes_teste.py` (1.ª vez) | 1 (duas expectativas erradas no teste novo: `x.apps` tem 6 caracteres; `-x:` já casava no parser antigo; corrigidas) |
| `python3 -I -B dev/testes/variacoes_teste.py` | 0 (43 testes) |
| `cargo test … -- v3_29 v3_31 v3_32 d61` (1.ª vez) | 101 (asserção errada do `d61_…`, comparava com linha de tab; corrigida) |
| `cargo test … -- v3_29 v3_31 v3_32 d61` | 0 |
| `cargo test … -- limites` (pior caso com D61) | 0 |
| mutação (sem JSON e comentário) + `cargo test … v3_31 v3_32` | 101 (esperado) |
| mutação (sem `GoAscii`) + `cargo test … v3_29` | 101 (esperado) |
| `cargo run --bin contorno19e` (cópia do repro) | 0 (só `repr()` vaza, RA3) |
| `timeout 300 make gitleaks-binario` (binário copiado) | 0 |
| `timeout 3500 make check` | 0 |
| `timeout 3000 make teste-dev-cofre` | 0 |

## Problemas descobertos
- **`make ferramentas` falhou 3 vezes:** o download do gitleaks no GitHub devolveu 504. Copiei `.dev/ferramentas/bin/gitleaks` do checkout principal. O `make gitleaks-binario` conferiu o sha256 fixado no `Makefile` antes de qualquer uso, e o `make check` confere de novo.
- **`CLAUDE.md` desatualizado:** a linha do `make dev-cofre` ainda diz "21 entradas … (D15 + `acme-totp-dev` de D28)". Agora são 22, com `delta-api-simbolos` de D59. O arquivo não estava na lista da fase; fica para o planner.
- **Condição de D58:** o PRD diz "`&`, `<`, `>`, U+2028 ou U+2029 **e** fora do ASCII". Como U+2028/U+2029 já são fora do ASCII e o `ensure_ascii` já os escreve em `\u`, a forma combinada só difere quando há `&<>`. A máscara gera a forma só nesse caso. O oráculo segue o texto literal e, nos outros casos, dá a mesma string do `ensure_ascii`, então o conjunto é o mesmo. O planner pode ajustar o texto de D58.
- **Chave YAML composta e parser antigo:** os dois convivem. O antigo aceita chave que começa por `-` (`-x: v`), e D61 recusa esse começo na regra nova. Mantive o antigo para não perder cobertura; o candidato novo só entra quando o valor difere.
- **JSON com controle cru:** a string com tab ou controle sem escape é aceita, como no `json.loads(strict=False)`. D61 só manda descartar escape inválido.
- **Locale do `ward run` (Fase 26):** em locale C, o bash escreve `${X@Q}` e `declare -p` de valor fora do ASCII como `$'…\303\247…'` (RA3). Com `C.UTF-8`, usa as aspas que a máscara cobre. O teste fixa `LC_ALL=C.UTF-8`. Se o ambiente do filho no `ward run` não tiver locale UTF-8, o `${X@Q}` do token de D59 cai em RA3. Fica para a Fase 26 decidir o `LANG`/`LC_*` do filho.
- **`iconv`, por acaso:** o caso do repro passou a sair mascarado, porque o campo ASCII de uma linha Latin-1 sobrevive ao `iconv`. Com o acento dentro do campo, continua em RA3.
- **Ordem no `visitar`:** no segredo de campo, os padrões por linha agora vêm antes do Basic (`segredo_de_campo` e depois `basic`). O conjunto e o custo não mudam.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/mascara.rs` | alterado (D58, D61, `Gerador::novo`/`segredo_de_campo`) |
| `crates/ward-core/src/mascara/testes.rs` | alterado (V3.29, V3.31, V3.32, D61) |
| `crates/ward-core/src/limites/testes.rs` | alterado (pior caso do extra JSON com D61) |
| `crates/ward-core/src/corpo/cofre_teste.rs` | alterado (`delta-api-simbolos`, 22 entradas) |
| `crates/ward-core/src/corpo/testes.rs` | alterado (contagem 22) |
| `crates/ward-core/src/protocolo/testes.rs` | alterado (contagem 22) |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado (contagem 22) |
| `crates/ward-core/examples/dev-cofre/testes.rs` | alterado (contagem 22) |
| `dev/fixtures/entradas.json` | alterado (D59) |
| `dev/variacoes.py` | alterado (D58, D61) |
| `dev/testes/variacoes_teste.py` | alterado (`D58D61`) |
| `dev/testes/dev-cofre.sh` | alterado (22 entradas, `simbolos_d59`) |
| `.gitleaks.toml` | alterado (2 valores de D59) |
| `Makefile` | alterado (só textos de ajuda) |
| `docs/mission/handoffs/phase-22a.md` | criado |
| `.dev/cofre-teste.ward`, `.dev/variacoes.txt`, `.dev/ferramentas/bin/gitleaks` | gerados/copiados (fora do git) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V3.29, V3.31, V3.32; V2.29, V2.30, V2.36, V2.43, V2.57 refeitos com 22 entradas)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase (`crates/wardd`, `crates/ward`, `app/`, `crates/ward-plataforma` e `CLAUDE.md` intocados)
