# Fase 1 — Inventário e glossário pt-BR → inglês (Missão 2, Milestone 1)

## Implementado
- `docs/glossario.md` (pt-BR, 1822 linhas), com 15 seções:
  1. Como usar.
  2. Termos (tabela termo → inglês, com sentidos separados, mais as palavras funcionais e os verbos comuns).
  3. Crates, pacotes, binários e feature.
  4. Pastas e arquivos (Rust, `app/src`, extensão, instalação, `dev/`, evidência).
  5. Caminhos em `.dev/`.
  6. Makefile: alvos, variáveis e argumentos dos scripts.
  7. CLI e chaves do `--json`.
  8. Variáveis de ambiente (DT11, DT12 e as de script).
  9. Marcas de `modo-teste` e caminhos instalados (DT15).
  10. Protocolo v2: envelope, operações, tipos de resposta, papéis e constantes, todos os códigos de erro, todos os campos de pedido e resposta, e os valores.
  11. Nomes gravados: corpo v5 (DT4) e estado `ward-state 2` (DT6).
  12. Native messaging e mensagens internas da extensão.
  13. Tauri: comandos, parâmetros, janelas, capabilities, avisos, campos `serde` e códigos que vão à webview.
  14. CSS: os 228 tokens de `tokens.css` e as 16 classes globais, sem nome repetido.
  15. Tipos públicos do Rust.
- Inventário feito por script: identificadores de Rust, JS, Svelte, CSS, Python, shell, Makefile, TOML e YAML, mais nomes de arquivo e pasta. A cobertura foi conferida: toda palavra pt-BR que aparece 3 vezes ou mais nos identificadores está na seção 2, exceto abreviações e dados (`el`, `res`, `pos`, `cha`, `gama`, `nano`…).
- `dev/english-check/pt-words.txt` (5709 linhas), com 151 radicais, 5415 palavras, 83 palavras funcionais e 31 ambíguas. O cabeçalho em inglês descreve a semântica que o checador da Fase 2 deve seguir. Os radicais foram conferidos contra os dicionários inglês americano e britânico e contra cerca de 29 mil arquivos Rust/JS do registry do cargo e do `node_modules`.
- PRD DT6: acrescentadas as chaves do estado v2 (`vista`→`seen`, `queimada`→`burned`, `falhas_agente`→`agent_failures`).
- Nenhum nome gravado fora de DT4 foi achado no corpo. O `corpo.rs` só grava em texto o tipo, o nível, o ambiente, o motor, a segurança SMTP e o protocolo FTP; o resto é binário. Por isso a tabela DT4 do PRD não mudou.
- Renomes feitos: nenhum. Literais mudados: nenhum. Entradas novas em `allow`/`frozen`: nenhuma, porque esses arquivos são da Fase 2.

## Segurança
- Invariantes tocadas: nenhuma. A fase só escreveu documentação e a lista de palavras, sem mudar código.
- O glossário protege estes pontos para as fases seguintes:
  - Marcas de `modo-teste` (seção 9): a recusa vale para a forma nova **e** para a antiga (DT12, V3.37/V9.2).
  - `WARD_PORTA` vira `WARD_LISTEN_PORT`, e não `WARD_PORT`, para não colidir com a variável entregue pelo `ward run`.
  - O teste que procura `WARD_APROV` passa a procurar também a forma nova.
  - A chave `.dev/extensao/chave.pem` só muda de lugar por `mv`, com modo 0600.
  - O caminho do cofre real não muda.
- Testes de ataque: nenhum, porque a fase não toca nenhum (SEG).
- Dependências novas: nenhuma.
- Nada de segredo: o glossário cita só nomes. Os valores falsos do cofre de teste aparecem só como padrão (`falso-…`).

## Ficou de fora
- Nomes de função e método não estão listados um a um: saem da seção 2 na ordem do inglês, e a seção 15 tem os poucos fixados.
- Palavras raras (que aparecem 1 ou 2 vezes) nos nomes de teste traduzem-se pelo sentido.
- `make check` não rodou. A regra do plano é que os Milestones 1–5 não rodam suítes, e a Fase 1 não compila nada.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| scripts de inventário e cobertura no scratchpad (`inv*.py`, `envocab.py`, `ptw.py`, `gen_ptwords.py`, `cover.py`, `css.py`) | 0 |
| `git grep` de conferência (flags, variáveis, tipos) | 0 |
| conferência das colunas das tabelas do glossário (0 erros) | 0 |
| `make check` | não rodado (regra do plano: M1–M5 sem suítes; fase sem compilação) |

## Problemas descobertos
1. **`ward lista`:** a tabela de texto imprime `TipoEntrada::nome()`, que é o nome gravado. Com o corpo v5, ela mostraria `database` em vez de `banco`. A Fase 51 precisa passar a imprimir `rotulo()`/`label()`. Ficou registrado na seção 7.
2. **`responder` e `mascara`:** são palavras inglesas e portuguesas ao mesmo tempo. Ficaram em `[words]` (palavra exata), e não em radicais. O checador vai acusar `responder` também quando o código for inglês legítimo, e esses casos vão para o `allow.txt` da Fase 2.
3. **Nomes de thread** (`ward-app-bandeja`, `wardd-*`…): são literais e aparecem em `ps`/journal. O glossário traduz, mas eles só mudam na fase do Tauri (54) ou do daemon.
4. **Colisões de nome evitadas:**
   - `Autent` (`ssh.rs`) vira `SshAuth`, porque `Autenticacao` já vira `Auth`.
   - `Copia` vira `BackupCopy`, para não confundir com o trait `Copy`.
   - `Forma` (`execucao.rs`) vira `Shape`, porque `Formulario` já vira `Form`.
   - `Configuracao` vira `VaultConfig`, porque `Config` já existe no `wardd`.
5. **Estado do repositório antes da fase:**
   - `docs/escopo.md` já estava modificado no worktree.
   - Havia renomes indexados de `docs/mission/evidence/M0/…` para `docs/mission/arquivo/m1/…`.
   - `docs/mission/{prd,plan,validation-contract}.md` não estão versionados (a versão antiga aparece como apagada no índice).
   - Nada disso foi mexido.
6. **Glossário × escopo:** o `docs/escopo.md` cita nomes pt-BR (alvos, caminhos) que mudam nesta missão. A atualização dele é das fases que mudam cada nome, e não foi feita aqui.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/glossario.md` | criado |
| `dev/english-check/pt-words.txt` | criado |
| `docs/mission/prd.md` | alterado (DT6: chaves do estado v2) |
| `docs/mission/handoffs/phase-1.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não: regra do plano para M1–M5; fase sem compilação)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto)
- [x] Teste de ataque para cada (SEG) tocado (nenhum tocado)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
