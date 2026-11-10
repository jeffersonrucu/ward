# Fase 41 — `migration-guide.py`, `attacks-foundation.sh` e `skill.py` (Missão 2, trilha S)

## Implementado

Troca 1:1 pelo `docs/glossario.md`, sem mudar comportamento. Parte de `master` a51cd7e
(o worktree estava em b4d5e1b; `git reset --hard a51cd7e` com a árvore limpa).
Identificadores e comentários em inglês; as mensagens de saída (`ok …`, `FALHA …`,
`test-skill: …`, `==> autoteste` …) continuam em pt-BR, iguais.

- **`dev/tests/skill.py`**: docstring e comentários em inglês. Renomes (só tokens `NAME`,
  por `tokenize`): `RAIZ` → `ROOT`, `VARIACOES` → `VARIATIONS`, `PERMITIDOS` → `ALLOWED`,
  `PENDENTES` → `PENDING`, `COFRE` → `VAULT`, `CAMINHOS_DENY` → `DENY_PATHS`,
  `ATAQUES_BASH` → `BASH_ATTACKS`, `ATAQUES_BASH_INTEIRO` → `WHOLE_BASH_ATTACKS`,
  `LEGITIMOS` → `LEGITIMATE`, `DESTRAVAR` → `UNLOCK`, `GATILHOS` → `TRIGGERS`,
  `OBRIGATORIOS_SKILL` → `REQUIRED_IN_SKILL`, `PROIBIDOS` → `FORBIDDEN`, `Falha` →
  `Failure`; funções `subcomandos_da_ajuda` → `help_subcommands`, `ler_frontmatter` →
  `read_frontmatter`, `trechos_de_codigo` → `code_excerpts`, `conferir_comandos` →
  `verify_commands`, `ler_padroes` → `read_patterns`, `conferir_segredos` →
  `verify_secrets`, `sem_chave_repetida` → `no_repeated_key`, `conferir_deny` →
  `verify_deny`, `padrao_regex` → `pattern_regex`, `subcomandos` → `subcommands`,
  `negado` → `denied`, `comandos_da_skill` → `skill_commands`, `conferir_bash` →
  `verify_bash`, `conferir` → `verify`, `ataque` → `attack`, `acrescentar` → `append`,
  `trocar` → `replace`, `tirar_deny` → `remove_deny`, `acrescentar_deny` →
  `append_deny`, `autoteste` → `self_test`; locais pela seção 2 (`arvore` → `tree`,
  `avisos` → `notices`, `trocas` → `swaps`, `todos=` → `every=` …).
- **`dev/tests/migration-guide.py`**: docstring e comentários em inglês. Renomes:
  `RAIZ` → `ROOT`, `GUIA` → `GUIDE`, `VARIACOES` → `VARIATIONS`, `PASSOS` → `STEPS`,
  `MONTAGEM` → `SETUP`, `DESTINO` → `RECIPIENT`, `ENVIO` → `SENDING`, `REGRA` → `RULE`,
  `EXCECOES_ENVIO` → `SEND_EXCEPTIONS`, `FRASES_DESTINO` → `RECIPIENT_SENTENCES`,
  `COMANDOS` → `COMMANDS`, `SEPARADORES` → `SEPARATORS`, `PROIBIDO` → `FORBIDDEN`,
  `APAGAR` → `DELETE`, `FIND_DESTINOS` → `FIND_DESTINATIONS`, `Falha` → `Failure`;
  funções `secoes` → `sections`, `tem_conferir_e_voltar` → `has_verify_and_go_back`,
  `separar_blocos` → `split_blocks`, `frases` → `sentences`, `comandos_do_bloco` →
  `block_commands`, `conferir_bloco` → `verify_block`, `regras_do_comando` →
  `command_rules`, `e_da_memoria` → `is_in_memory`, `e_chave` → `is_key`,
  `conferir_tokens` → `verify_tokens`, `regras_de_copia` → `copy_rules`,
  `regras_do_find` → `find_rules`, `antes_de` → `before_heading`, `conferir` → `verify`,
  `ler_variacoes` → `read_variations`, `ataques` → `attacks`, `autoteste` → `self_test`
  (e as internas `trocar` → `swap`, `antes_p2` → `before_p2`, `bloco` → `block`); locais
  pela seção 2 (`corpo` → `body`, `erros` → `errors`, `leva` → `carries` …).
  - O uso impresso numa chamada errada vinha da última linha da docstring
    (`__doc__…[-1]`). Com a docstring em inglês, virou a constante `USAGE` com o mesmo
    texto pt-BR, byte a byte (conferido contra a docstring do `HEAD`).
- **`dev/tests/attacks-foundation.sh`**: comentários em inglês e renomes: `RAIZ` →
  `ROOT`, `pasta_privada` → `private_dir`, `CACHE_COPIAS` → `COPIES_CACHE`, `VAZAMENTO`
  → `LEAK`, `limpar` → `cleanup`, `falhas`/`falhou` → `failures`/`failed`,
  `copiar_workspace` → `copy_workspace`, `adicionar_crate_ataque` → `add_attack_crate`,
  `allowlist_ancorada` → `anchored_allowlist`, `dev_forcado_no_indice` →
  `dev_forced_into_index`, `extra_sem_allowlist_de_caminho` →
  `extra_without_path_allowlist`, `origem_recusada` → `refused_origin`,
  `origens_do_cargo` → `cargo_origins`, `ferramentas_fixas` → `fixed_tools`, `MARCAS` →
  `MARKS`, `preparar_iscas_cargo` → `prepare_cargo_decoys`, `barrado_antes` →
  `blocked_before`, `config_na_copia` → `config_in_copy`, `origens_lista_permitida` →
  `origins_allowlist`, `config_do_cargo_home` → `cargo_home_config`, `no_home` →
  `in_home`, `ambiente_do_make` → `make_environment`, `git_sem_config_do_usuario` →
  `git_without_user_config`, `origens_da_raiz_durante_a_rodada` →
  `root_origins_during_run`, `rodar` → `run`, e os locais (`saida` → `output`, `segredo`
  → `secret`, `chave` → `key`, `alvo` → `target`/`triple`, `padroes` → `patterns` …).
  - **Textos procurados em saídas de outros arquivos ficaram iguais** e conferidos contra
    o master: `==> 0/6 origens`, `==> 1/6`, `==> 4/6 cargo audit`, `==> make check: tudo
    limpo` (echo do `Makefile`); `^origens: `, `origens: pacote <nome>: fonte fora do
    crates.io`, `.cargo/config.toml: .cargo/config\* no repositório é proibido`,
    `^origens: .*config`, `toolchain ativo` (`dev/origins.py`); `não é o gitleaks`
    (`Makefile`, alvo `gitleaks-binary`); `RUSTSEC-2020-0071`, `source-not-allowed`
    (cargo-audit/deny). A linha `"crates/ward-test-bridge",` que o `sed` procura no
    `Cargo.toml` raiz também está igual.
- **Glossário**: notas de sentido nas linhas `destino` (recipient), `guardar` (o
  substantivo guarda → guard), `montar` (montagem da skill antiga → setup; "montado por
  partes" → built in pieces), `trocar` (swap/replace nos autotestes) e `voltar` ("Voltar
  atrás" → go back).

### Literais alterados (PRD regra 5; todos são nome de arquivo, pasta, crate ou texto gerado nas cópias temporárias)

- `attacks-foundation.sh`:
  - arquivo plantado na raiz: `vazamento.txt` → `leak.txt` (o `grep -q` que o procura na
    saída do `make check` e as mensagens `ok`/`FALHA` que o citam mudaram junto);
  - `mktemp … ward-ataques-fundacao.XXXXXX` → `ward-attacks-foundation.XXXXXX`;
  - crate de teste `crates/ataque` (`name = "ataque"`, `sed` no `Cargo.toml`,
    `rm -rf crates/ataque`) → `crates/attack`; crate servido por git `falso-git` →
    `fake-git` (o `grep` da mensagem do `origins.py` e do `cargo-deny` acompanham);
    `ward-falso` do `[patch]` → `ward-fake`; crate `fora` → `outside`; `falso-ok` →
    `fake-ok` (e a descrição "para um fake-ok (v13-alias)");
  - chave forçada no índice `.dev/chave` → `.dev/key` (e o `grep -q '.dev/key'`);
  - pastas e arquivos dentro de `$TMP` (`iscas/` → `decoys/`, `programa.sh` →
    `program.sh`, `marcas` → `marks`, `no-home.sh/.out` → `in-home.sh/.out`, `minimo` →
    `minimal`, `ancora`, `indice`, `pontilhada`, `config-antigo`, `registro`, `pai`/
    `filho`, `invalido`, `ambiente`, `fora`, `limpa`, `ferramentas`, `vazou`,
    `fora-cargo`, `pai2`, `tc-canal`, `tc-legado`, `amb` → `env-make`, `home-falso`,
    `gitconfig-mau`, `gitconfig-ignora`, `git-isolado`, `v1_2-limpo.log`,
    `chave-extra`), os arquivos plantados nas cópias (`burla.txt` → `bypass.txt`,
    `docs/icone.svg`, `foto.png`, `fonte.woff2`, `pacote.txt.gz` → `icon.svg`,
    `photo.png`, `font.woff2`, `package.txt.gz`), `/nao/existe` → `/does/not/exist`,
    `outro.toml` → `other.toml`, `qualquer.txt`/`.dev/qualquer` → `anything.txt`/
    `.dev/anything`;
  - comentários `-C` das chaves geradas (`ward-teste-vazamento` → `ward-test-leak` …),
    `git -c user.name=teste …` → `test`, e os textos dos `lib.rs` gerados (`//! …`,
    `porta_dos_fundos` → `back_door`, `echo programa` → `echo program`).
  - Nenhum desses nomes é lido fora do script (grep no repositório).
- `skill.py`: arquivos plantados nas cópias do autoteste `ward/referencia.md` →
  `ward/reference.md` e o link `ward/variacoes.md` → `ward/variations.md`.
- `migration-guide.py`: nenhum (o texto do uso é o mesmo, só mudou de lugar).

### Exceções

Nenhuma entrada nova em `allow.txt` nem em `frozen.txt`. Os comentários que citavam
subcomandos congelados (`lista`, `travar`, `destravar`), os scripts `cofre-*` da skill
antiga e títulos do guia (`Passo`, `Conferir`, `Voltar atrás`, `Carregar de arquivo`)
foram reescritos sem citar a palavra (pelo nome da constante ou pelo sentido); os
literais com esses nomes ficam (fio/texto congelado até as Fases 55 e 58).

## Segurança

- **Invariantes**: nenhuma lógica mudou. `skill.py` e `migration-guide.py` foram
  conferidos por AST: o original com o mesmo mapa de renomes aplicado e o arquivo novo
  têm a mesma árvore (docstrings à parte), com diferença só nas três constantes de
  caminho do `skill.py` acima e, no `migration-guide.py`, a troca de `__doc__…[-1]` por
  `USAGE` (mesmo texto). O `attacks-foundation.sh` foi conferido com o mapa ao contrário
  (nomes novos → antigos, sem comentários) contra o original: sobram só os literais
  listados acima.
- **S14/S15 (V1.2, V1.3, V1.5)**: os ataques são os mesmos, na mesma ordem, com as mesmas
  expectativas. O V1.5 continua montando os padrões por partes; os comentários novos
  não citam pasta proibida.
- **Testes de ataque**: nenhum novo (a fase só renomeia). Escritos, não rodados (PRD
  regra 6): `make test-attacks-foundation`, `make test-skill` e
  `make test-migration-guide` rodam no M6 (Fase 61).
- **Dependências novas**: nenhuma.

## Ficou de fora

- `CASA_PASSWD` (2 achados do `english-check` no `attacks-foundation.sh`): é definido em
  `dev/env.sh` (Fase 46) e usado também em `dev-vault.sh`, `ssh-home.sh` (Fase 44) e
  `home-decoys.sh` (Fase 46). Trocar aqui mexeria em arquivos de outras fases; fica para
  a Fase 46 renomear no `env.sh` e em todos os usos (sugestão: `PASSWD_HOME`, como a
  variável `CASA` do Makefile na Fase 6).
- Textos procurados em saídas do `Makefile` e do `dev/origins.py` (acima): ficam até as
  Fases 45 (`origins.py`) e 47 (Makefile); quem os mudar tem de ajustar os `grep` deste
  script na mesma fase.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard a51cd7e` | 0 |
| `make compile` (log em `<scratchpad>/fase41/fase41-make-compile.log`) | 0 (origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python e 7 JS) |
| `make english-check PATHS="dev/tests/migration-guide.py dev/tests/skill.py"` | 0 (0 achados) |
| `make english-check PATHS=dev/tests/attacks-foundation.sh` | 1 (2 achados, `CASA_PASSWD`, Fase 46) |
| `python3 -I -B dev/english_check.py --self-test` | 0 (91/91) |
| conferência por AST (`skill.py`, `migration-guide.py`) e por mapa reverso (`attacks-foundation.sh`) | 0 |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

- Para as fases que traduzirem `Makefile` (47) e `dev/origins.py` (45): o
  `attacks-foundation.sh` procura `==> 0/6 origens`, `==> 1/6`, `==> 4/6 cargo audit`,
  `==> make check: tudo limpo`, `não é o gitleaks`, `^origens: `, `origens: pacote …: fonte
  fora do crates.io`, `.cargo/config* no repositório é proibido` e `toolchain ativo`. Se
  algum desses textos mudar, o ataque correspondente passa a "falhar" à toa.
- `CASA_PASSWD` (acima): dono é a Fase 46.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/tests/attacks-foundation.sh` | alterado |
| `dev/tests/migration-guide.py` | alterado |
| `dev/tests/skill.py` | alterado |
| `docs/glossario.md` | alterado |
| `docs/mission/handoffs/phase-41.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. Rodaram `make compile`, o
  `english-check` nos arquivos da fase e o autoteste do checador.
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre aberto, nenhum daemon).
- [x] Teste de ataque para cada (SEG) tocado: os ataques existentes seguem iguais; nenhum
  novo, porque a fase só renomeia.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
