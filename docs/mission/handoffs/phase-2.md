# Fase 2 — Checador de pt-BR e `make compile` (Missão 2, Milestone 1)

## Implementado
- **`dev/english_check.py`** (DT17). Usa só a biblioteca padrão e roda com `#!/usr/bin/python3 -IB`.
  - **Arquivos lidos:** os do git (`ls-files --cached --others --exclude-standard`).
    - Fora do conteúdo e do nome: `docs/`, `.claude/`, `README.md`, `CLAUDE.md` e `skill/**/*.md`.
    - Só o nome é conferido: lockfiles, binários (NUL, UTF-8 inválido ou extensão de binário, `.jsonl` incluído), `dev/english-check/` e `dev/fixtures/legacy/`.
  - **Lexer próprio por linguagem.** Cada um separa comentário, identificador e literal:
    - Rust; C;
    - JS/MJS (com template `${}` e regex);
    - Svelte, HTML, XML e SVG:
      - `<script>` vai para o JS e `<style>` para o CSS;
      - `{expr}` vai para o JS;
      - os valores de `class`/`id` são identificadores, os de `style` vão para o CSS, e o texto e os outros atributos são literais;
    - CSS;
    - Python (docstring conta como comentário; as expressões de f-string contam como identificador);
    - shell:
      - aspas, `$x`, `${x…}`, `$(…)`, `$((…))` e heredoc;
      - o nome de atribuição, `local`/`export`/`read`/`for`/`function` e `nome()` são identificadores;
      - a palavra solta não é conferida;
    - Makefile:
      - nomes de variável, alvo e pré-requisito sem `/`, e referências `$(…)`;
      - a receita passa pelo lexer de shell;
    - Dockerfile (pelo shell, com `ARG`/`ENV`);
    - TOML (chave e palavra solta);
    - YAML (chave, âncora e alias);
    - JSON (chave é identificador, valor é literal);
    - INI (units do systemd, `.desktop`, `.conf`, `.env`, `.npmrc`, `.gitignore`, `.txt`).
  - **Regras** (as do cabeçalho de `pt-words.txt`):
    - Letra fora do ASCII acusa.
    - `[stems]` e `[words]` acusam em comentário, identificador e nome.
    - `[function]` acusa como segmento de identificador.
    - Em comentário, conta por frase: acusa com 2 `[function]`, ou com 1 `[function]` e 1 `[ambiguous]`.
    - Comentários de linhas seguidas formam um bloco.
    - O prefixo `on` de prop do Svelte sai antes da busca.
    - Literais nunca acusam.
  - **Exceções:** `allow.txt` (`<glob> <palavra|identificador> <motivo>`) e `frozen.txt` (`<glob> <palavra|identificador> <fase> <motivo>`).
    - Uma exceção sem uso falha com "exceção sem uso".
    - Com `--paths`, isso vale só para as exceções cujo glob casa com algum arquivo ou nome conferido.
    - Uma linha mal formada também falha (ex.: `frozen.txt` sem fase numérica).
  - **Modos:**
    - Sem argumento: o repositório inteiro.
    - `--paths P…`: só esses arquivos ou pastas. O nome é conferido da pasta dada para baixo. Caminho fora do repositório sai com 2.
    - `--literals [--with-location]`: uma string JSON por linha, ordenadas e sem repetição. Serve para a Fase 3 e o V6.3.
    - `--self-test`.
  - **Saída:** `arquivo:linha: <comment|identifier|name> '<token>': <motivo>`.
  - **Códigos de saída:** 0 quando está limpo, 1 com achado ou problema nas exceções, 2 com erro de uso ou erro interno.
- **Autoteste** (`--self-test`, V1.2 e V1.3; 91 casos ok). Trabalha numa cópia dos arquivos do repositório em `.dev/english-check-selftest.*` (mkdtemp 0700), apagada no fim.
  - **Por linguagem** (Rust, JS, Svelte, CSS, shell, Python, Makefile, unit do systemd, TOML, YAML, Dockerfile e C):
    - um arquivo novo com um identificador pt-BR e um comentário pt-BR, cada um na sua linha;
    - as mesmas linhas acrescentadas a um arquivo existente da cópia;
    - em cada um, exige o achado na linha exata.
  - **Outros plantios que têm de acusar:**
    - letra fora do ASCII;
    - pasta e arquivo com nome pt-BR.
  - **Controles negativos, que têm de dar zero achado:**
    - `"senha incorreta"` como literal em Rust, Python, JS, shell (com heredoc), Svelte (texto e `title`) e YAML;
    - comentários em inglês com "a" e "de" soltos.
  - **Links simbólicos (V1.4):** um link para arquivo fora da cópia e um link para pasta fora da cópia não são lidos.
  - **Exceções:**
    - a `allow` usada silencia o achado;
    - a `allow` e a `frozen` sem uso falham, com `--paths` e no modo inteiro;
    - `frozen` sem fase falha.
- **`dev/syntax_check.py`** (só biblioteca padrão; usado pelo `make compile`). Nada do código conferido é executado.
  - `bash -n` dos `*.sh` e dos arquivos com shebang de bash ou sh.
  - Python pelo `compile()` embutido. É o mesmo parser do `py_compile`, mas sem gravar `__pycache__` no repositório.
  - `node --check` do JS fora do `app/src`: a extensão, `dev/testes/bidi.mjs` e `app/vite.config.js`. O Svelte já é conferido pelo Vite.
- **Makefile**, com comentários novos em inglês:
  - **`compile`** (V1.5), depende de `origens`:
    - `cargo check --workspace --all-targets --all-features --locked`;
    - `npm ci --ignore-scripts`, só se faltar `app/node_modules`;
    - `npm run build` (Vite de produção);
    - `cargo check --all-targets --all-features --locked` do `app/src-tauri`, se `pkg-config --exists webkit2gtk-4.1`; sem isso, avisa e pula;
    - `dev/syntax_check.py`.
    - Não roda teste nenhum.
  - **`english-check`**:
    - roda `$(PYTHON) dev/english_check.py`, mais `--paths $(PATHS)` se `PATHS` vier preenchido;
    - só gera relatório e está fora do `make check` até a Fase 60.
  - Os dois entraram no `.PHONY`.
- **`dev/english-check/allow.txt`**:
  - um cabeçalho com o formato e a regra das palavras que também são inglesas (`responder`, `mascara`, `motor`, `pasta`…);
  - duas entradas de dados do usuário:
    - `dev/fixtures/*/gama-generico generico`: o nome da entrada;
    - `dev/fixtures/*/gama-generico/app.env falso`: o conteúdo do anexo de teste, V4.19.
- **`dev/english-check/frozen.txt`**: só o cabeçalho, sem entradas.
- `CLAUDE.md`: duas linhas novas na tabela de comandos (`make compile` e `make english-check`).
- `docs/glossario.md`: `dev/syntax_check.py` entrou na linha "novos (Fases 2 e 3)".
- **Renomes, literais e entradas novas:**
  - Renomes: nenhum.
  - Literais alterados: nenhum. A fase só criou código e não alterou nenhum literal existente.
  - Entradas novas de `allow.txt`: as 2 acima. De `frozen.txt`: nenhuma.

## Segurança
- **Invariantes tocadas:** S14, pelo V1.4.
  - Nenhuma dependência nova: só a biblioteca padrão do Python, e nada mudou em `Cargo.lock` ou `package-lock.json`.
  - Os dois scripts rodam com `python3 -I -B`.
  - O git roda com `GIT_CONFIG_GLOBAL=/dev/null` e `GIT_CONFIG_NOSYSTEM=1`. `core.excludesFile`, `core.attributesFile`, `core.fsmonitor` e `core.hooksPath` ficam fixos pela linha de comando, e o `PATH` é `/usr/bin:/bin`.
  - **Leitura:** `O_NOFOLLOW|O_NONBLOCK`, só arquivo comum (FIFO e dispositivo nunca), e só quando a pasta resolvida fica dentro do repositório. Link é conferido só pelo nome.
  - **Escrita:** só em `.dev/`, pelo autoteste, que recusa `.dev` se for link.
  - O `--paths` recusa caminho fora do repositório.
  - O checador não executa nada além do `git ls-files`.
  - O `syntax_check.py` só faz checagem de sintaxe (`bash -n`, `node --check`, `compile()`), com ambiente mínimo.
- **Testes de ataque** (dentro do autoteste, em `dev/english_check.py` › `self_test`):
  - "link para fora não seguido": link para arquivo e link para pasta fora da cópia, com conteúdo pt-BR. Nenhum achado.
  - "allow sem uso", "frozen sem uso", "frozen sem uso (tudo)" e "frozen sem fase": uma exceção que mascara o que não devia, ou que ficou velha, faz o checador falhar.
- **Dependências novas:** nenhuma.
- O `make compile` passa pela pré-checagem `origens` antes do cargo e usa `--locked`. O `npm ci` só roda com `--ignore-scripts`, e só se faltar `node_modules`.

## Ficou de fora
- **O `english-check` no `make check`:** é da Fase 60, pelo plano. O alvo também não roda o autoteste; ele roda à parte, com `--self-test`.
- **Palavras soltas no shell** (argumentos e nomes de comando) não são conferidas: no shell não dá para separar mensagem de nome sem executar.
  - O nome de função é pego na definição (`nome()`).
  - A variável é pega na atribuição e no uso (`$x`).
  - Os argumentos `--sem-mps` e afins dos scripts não são acusados. A troca deles segue o glossário nas Fases 5, 6 e 45.
- **Expressões dentro de f-string do Python** só têm os identificadores extraídos por regex. Strings aninhadas na expressão ficam como identificador.
- **`responder`/`mascara`:** hoje todos os usos são pt-BR (`area_gtk.rs`, `JanelaAprovacao.svelte`, `CampoSecreto.svelte`…). Por isso nenhuma entrada foi criada: ela falharia por falta de uso. A fase que escrever o uso inglês legítimo acrescenta a entrada, e o cabeçalho do `allow.txt` diz isso.
- **`make check`** não foi rodado: pela regra 6 do PRD, as fases dos M1–M5 só rodam `make compile` e o `english-check`. O autoteste do checador foi rodado.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I -B dev/english_check.py --self-test` (91/91) | 0 |
| `make compile` (origens, cargo check do workspace, Vite, cargo check do `app/src-tauri` com WebKitGTK presente, sintaxe: 48 shell, 35 Python, 7 JS) | 0 |
| `make english-check PATHS="dev/english_check.py dev/syntax_check.py dev/english-check"` | 0 |
| `make english-check PATHS="dev/fixtures/anexos/gama-generico"` (as duas entradas do `allow.txt` usadas) | 0 |
| `python3 -I -B dev/english_check.py` (repositório inteiro: 179 321 achados em 419 arquivos, 0 problemas nas exceções; relatório esperado antes das traduções) | 1 |
| `make english-check PATHS="../etc"` (recusa fora do repositório) | 2 |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos
1. **Linha de base do checador:** 179 321 achados (82 884 em comentário, 96 096 em identificador e 351 em nome).
   - Amostras conferidas à mão em Rust, Svelte, CSS, shell, Makefile, Python e TOML: todos são pt-BR de verdade.
   - Nenhum token de identificador tem espaço, ou seja, nenhum literal foi lido como identificador.
   - Das palavras acusadas, as que também estão no dicionário inglês (`remover`, `anterior`, `motor`, `eco`, `clique`…) aparecem aqui só no sentido pt-BR.
2. **Nomes de arquivo fora do plano de renomes:** o checador acusa nomes de arquivo que o plano não cita uma a um. Exemplos:
   - `app/design/icones/*.svg`;
   - `app/src-tauri/icons/bandeja-*-32.png`;
   - `dev/fixtures/anexos/`.
   - Cada um precisa de fase dona (Fase 5 ou 35) ou de entrada no `allow.txt` com motivo. Os ícones da bandeja são carregados por caminho no Tauri, então mudar o nome deles mexe no código.
3. **Comentários novos do `Makefile` citam `origens`:** é o nome atual do alvo, que muda na Fase 6. Nos comentários e nas dependências, ele aparece como achado nas linhas novas, e isso é esperado. A fase não criou `frozen` para ele, porque não é nome de fio.
4. **`CLAUDE.md` fora do checador:** o DT17 não cita o `CLAUDE.md`. Ficou fora (conteúdo e nome) por ser documentação em pt-BR, como o `README.md`. Se o PRD quiser o contrário, é uma linha em `DOC_FILES`.
5. **Estado do repositório:** igual ao da Fase 1. `docs/escopo.md` já estava modificado, e havia renomes indexados de `docs/mission/evidence/M0/…`. A fase não mexeu nisso.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/english_check.py` | criado |
| `dev/syntax_check.py` | criado |
| `dev/english-check/allow.txt` | criado |
| `dev/english-check/frozen.txt` | criado |
| `Makefile` | alterado (alvos `compile` e `english-check`, `.PHONY`) |
| `CLAUDE.md` | alterado (duas linhas na tabela de comandos) |
| `docs/glossario.md` | alterado (`dev/syntax_check.py` na lista de novos) |
| `docs/mission/handoffs/phase-2.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. No lugar dele, rodaram `make compile` e `make english-check` nos caminhos da fase, os dois limpos, e o autoteste.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto).
- [x] Teste de ataque para cada (SEG) tocado: V1.4, com link para fora e exceção sem uso.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
