# Fase 19d — Lista do `make dev-variacoes` com D36 e D37

## Implementado
- `dev/variacoes.py`, D36: a passada por linha de todo segredo de várias linhas (chave privada, extra secreto com quebra de linha, anexos) agora gera:
  - cada linha aparada com 6+ caracteres;
  - o valor de `CHAVE=valor`/`export CHAVE=valor`, como antes;
  - o valor de `"chave": "valor"` (JSON, com ou sem a vírgula do fim);
  - o valor de `chave: valor` (YAML, também `- chave: valor`; precisa de espaço ou tab depois dos dois pontos, então URL e `host:porta` ficam de fora).

  Cada valor entra com e sem aspas, só com 6+ caracteres. O mínimo agora é contado em caracteres quando o texto é UTF-8 válido e em bytes quando não é, como diz D34/D36 (antes era só em bytes). Cada linha e cada valor recebem as mesmas codificações de antes.
- `dev/variacoes.py`, D37: a nova função `escapadas()` entra em `todas()`. Ela gera as formas JSON do Go (`\u0026`, `\u003c`, `\u003e`, `\u2028`, `\u2029` em cima do escape JSON comum), `ensure_ascii` do Python (`\uXXXX` minúsculo, com par substituto) e aspas duplas do shell (barra antes de `$`, `` ` ``, `"` e `\`; só para valores sem quebra de linha). Cada forma entra só quando difere da literal. Por isso vale para cada segredo, para o inteiro de várias linhas (o JSON fica numa linha só), para cada linha e para cada valor de linha. Hex em maiúsculas fica fora (RA3).
- O `ensure_ascii`, que já existia dentro de `json_escape`, saiu dali e foi para `escapadas()`. A lista final não muda por isso.
- Atualizei a docstring do `variacoes.py`. Ela diz que o arquivo é um oráculo independente e que não importa nem chama o `ward-core`.
- Novo `dev/testes/variacoes_teste.py`, com `unittest` e só a biblioteca padrão. São 24 testes com os valores falsos da V2.52 e os casos de antes da V2.29: URL, base64, Basic, `DATABASE_URL`, miolo de base64, escape JSON/PHP, padrão curto e recusa de saída fora de `.dev/`. Para não depender do código do oráculo, as formas esperadas estão escritas à mão, com `~` no lugar da barra invertida (função `esc`).
- `dev/testes/dev-cofre.sh`: nova etapa `variacoes_d36_d37`, rodada por `make teste-dev-cofre`, que faz três coisas:
  - roda o `variacoes_teste.py` e confere que a saída dele não tem segredo;
  - calcula, de forma independente e com a biblioteca padrão, as formas de D37 dos literais das fixtures e das linhas do anexo, e procura cada uma na lista real;
  - confere que o valor de linha do anexo está na lista.
- `CLAUDE.md`: atualizei a descrição de `make dev-variacoes` (D11, D36, D37, oráculo independente, teste) e a de `make teste-dev-cofre` (V2.52).

## Segurança
- **S4** (oráculo de "contém segredo"): a lista agora cobre as linhas soltas e as formas escapadas que a máscara da 19c passou a mascarar. Um validador que procura com `grep -F -f` passa a achar vazamento de `head -n 4` da chave, de `jq -r .private_key`, de `declare -p` e do JSON do Go e do Python.
- **Independência**: o `variacoes.py` não importa nem chama `ward-core`, `cargo` ou `subprocess`. Um teste (`Independencia.test_nao_chama_o_ward_core`) confere isso na fonte. Escrevi as regras a partir do PRD e de D36/D37. Li o `mascara.rs` só para confirmar a semântica de `valor_de_dois_pontos` (chave do YAML, vírgula do fim). Não copiei código dele.
- **Saída sem segredo**: as mensagens de falha dos testes dizem o caso, nunca o valor nem o conjunto. O `dev-cofre.sh` passa o log do teste pelo `sem_segredo`.
- **Testes de ataque** (o vazamento por forma escapada ou linha solta seria pego):
  - `dev/testes/variacoes_teste.py`:
    - `D37FormasEscapadas.test_json_do_go`, `test_aspas_duplas_do_shell`, `test_ensure_ascii_minusculo`, `test_ensure_ascii_com_par_substituto`, `test_u2028_e_u2029`, `test_formas_tambem_por_linha`, `test_inteiro_de_varias_linhas_em_json`;
    - `test_so_quando_difere_da_literal` (`falso-simples-01` sem padrão novo) e `test_hex_maiusculo_fica_de_fora`;
    - `D36PassadaPorLinha.test_linhas_aparadas` (CRLF e espaços), `test_json_de_varias_linhas`, `test_yaml`, `test_chave_igual` (`PORT=8080` não vira padrão), `test_url_e_host_porta_nao_viram_yaml` e `test_minimo_em_caracteres`;
    - `D11DeAntes.*` (V2.29) e `Independencia.*`.
  - Conferi com mutações num script de rascunho, fora do repositório. Cada uma reprova algum teste: `escapadas` vazia, Go sem `\u0026`, `ensure_ascii` em maiúsculas, shell sem `\$`, sem os valores de linha e mínimo em bytes.
  - `dev/testes/dev-cofre.sh` › `variacoes_d36_d37`.
- **Dependências novas**: nenhuma.

## Ficou de fora
- `Makefile`: o texto de ajuda de `dev-variacoes` ainda fala só de "variações de D11". A fase não podia tocar no `Makefile` (ver "Problemas").
- O valor JSON com escapes que o `jq -r` decodifica (por exemplo, a `private_key` com `\n`) não ganha forma decodificada. A máscara também não cobre isso (RA3), e o oráculo acompanha o que a máscara promete.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 1200 make ferramentas` | 0 |
| `python3 -I -B dev/testes/variacoes_teste.py` | 0 (24 testes) |
| `python3 -I -B <scratchpad>/mutacao.py` | 0 (6 mutações, todas reprovadas) |
| `timeout 3000 make teste-dev-cofre` | 0 |
| `timeout 3000 make check` (1.ª vez) | 2 (gitleaks: cabeçalho `BEGIN … PRIVATE KEY` falso no teste; trocado por `BEGIN CHAVE FALSA DE TESTE`) |
| `timeout 3000 make check` | 0 |
| `timeout 3000 make teste-dev-cofre` (depois da troca) | 0 |

## Problemas descobertos
- O texto de ajuda de `dev-variacoes` no `Makefile` (`## dev-variacoes: … as variações de D11`) ficou desatualizado. A mudança é só desse comentário, mas a fase não podia tocar no `Makefile`.
- Nenhum segredo de D15 tem `&`, `<`, `>`, `$`, aspas, barra invertida ou caractere fora do ASCII. Por isso, na lista real, a checagem de D37 do `dev-cofre.sh` não tem caso: ela imprime "nenhum segredo das fixtures tem caractere a escapar", e a cobertura de D37 vem do `variacoes_teste.py`. Para o M3 em diante, o planner pode querer uma entrada de teste com esses caracteres (por exemplo `Ab3$k&<9>Zq!`). Isso muda as fixtures e o V2.29, então fica fora desta fase.
- A causa provável dos escapes `\uXXXX` "decodificados" no PRD e no contrato, que o handoff da 19c apontou: a ferramenta de escrita de arquivo do agente transforma `\uXXXX` com uma barra em caractere. Isso aconteceu no meu primeiro rascunho do teste. Por isso o teste usa `~` no lugar da barra e `chr(0x…)`. Quem editar esses documentos deve conferir os escapes com `grep` depois de gravar.
- O mínimo de 6 do oráculo passou a contar caracteres, não bytes. Uma linha de 5 caracteres acentuados, com 6 bytes ou mais, saiu da lista. A máscara também não a mascara.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/variacoes.py` | alterado |
| `dev/testes/variacoes_teste.py` | criado |
| `dev/testes/dev-cofre.sh` | alterado |
| `CLAUDE.md` | alterado (descrição de `make dev-variacoes` e `make teste-dev-cofre`) |
| `docs/mission/handoffs/phase-19d.md` | criado |
| `.dev/variacoes.txt`, `.dev/cofre-teste.ward` | regenerados por `make teste-dev-cofre` (fora do git) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V2.52)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase (`ward-core`, `wardd`, `Makefile` e fixtures intocados)
