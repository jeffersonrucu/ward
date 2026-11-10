# Fase 28f — Cópias do teste de ataques fora do alcance do origens

## Implementado
Só `dev/testes/ataques-fundacao.sh` mudou. Das duas saídas do plano, escolhi a mais segura:
as cópias saem do repositório. O `dev/origens.py` não ganhou exceção para `.dev/`.

- **Cópias em `~/.cache/ward-ataques-fundacao.XXXXXX`**: o `~` é o home do passwd
  (`CASA_PASSWD`), não o `$HOME`. A pasta é criada pelo `mktemp -d`, com modo 0700. O `trap`
  continua apagando tudo ao sair.
  - Antes, a função `pasta_privada` confere o `~/.cache` e a pasta criada:
    - o caminho não pode ser link (`realpath -e` igual ao caminho);
    - cada pasta da cadeia até `/` tem de ser do usuário ou do root, sem escrita de grupo
      nem de outros.
  - Assim o `BinarioDeTeste` (V3.37) continua aceitando o app de teste da cópia.
  - O script também sai se a pasta cair dentro do repositório.
  - O `~/.cache` é criado com 0700 se não existir. Se for link, o script sai.
- **Nova checagem `origens_da_raiz_durante_a_rodada`** (teste de regressão da 28f): roda duas
  vezes, depois do `config_do_cargo_home` e no fim.
  - Conta os `.cargo/config*` plantados nas cópias que ainda estão no disco. Se não achar
    nenhum, falha, porque a checagem ficaria sem efeito.
  - Exige que o `make -s origens` da raiz saia 0.
- Os comentários que diziam "cópia em `.dev/`" foram atualizados: `copiar_workspace` e o bwrap do
  `config_do_cargo_home`. O `--bind "$TMP"` do bwrap continua valendo, porque o
  `~/.cache` também fica sob o home coberto por tmpfs.

## Segurança
- Invariantes tocadas: S14 e S15 (V1.2, V1.3 e V1.5), só no harness de teste. Nenhum código
  de produção mudou e o `origens.py` ficou igual.
  - Continua recusando `.cargo/config*` em qualquer pasta do repositório, `.dev/` incluída.
  - Por isso não segui a outra opção do plano ("ignorar `.dev/`"): ela deixaria passar
    um `.cargo/config` plantado em `.dev/<pasta>` onde algum harness rode o cargo (ex.: o
    `.dev/webkit-real/` da 29b).
- O material gerado (chave ed25519, segredo aleatório, iscas) fica numa pasta 0700 do
  usuário, com a cadeia conferida, e é apagado pelo `trap`. Depois da rodada, nenhum
  `~/.cache/ward-ataques-fundacao.*` sobrou.
- Testes de ataque:
  - `dev/testes/ataques-fundacao.sh` › `origens_da_raiz_durante_a_rodada`: com 21
    `.cargo/config*` plantados nas cópias, o origens da raiz fica limpo.
  - Os 160+ ataques existentes passaram.
- Dependências novas: nenhuma.

## Ficou de fora
- Limpeza de cópias órfãs de rodadas mortas por SIGKILL (o `trap` não roda). Não apaguei
  por padrão, porque outro worktree pode estar rodando o teste ao mesmo tempo.
  - Se sobrar alguma, é só `rm -rf ~/.cache/ward-ataques-fundacao.*` com nenhuma rodada
    ativa.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `df -h /` (antes) | 0 (13 GB livres) |
| `bash -n dev/testes/ataques-fundacao.sh` | 0 |
| `make -s gitleaks-binario` (gitleaks copiado do `.dev/` do checkout principal, sha256 conferido) | 0 |
| `make teste-ataques-fundacao` | 0 (167 `ok`, 0 `FALHA`, "Todos os ataques foram barrados.") |
| `make -s origens` no worktree a cada 45 s durante a rodada (26 vezes) | 0 em todas (até com 21 configs plantados no disco) |
| `make check` (depois da rodada) | 0 ("make check: tudo limpo") |

Na rodada, o V1.2 deu certo nas duas pontas. Com `vazamento.txt`, o `make check` da raiz
saiu 2 e citou o arquivo. Sem ele, voltou a 0.

## Problemas descobertos
- Disco: no pior momento da rodada ficaram ~9 GB livres. O `target/` deste worktree tem uns
  3 GB e pode ser apagado depois do merge.
- Não parei o daemon nem a ponte do checkout principal. Do checkout principal, só copiei o
  binário do gitleaks.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/testes/ataques-fundacao.sh` | alterado |
| `docs/mission/handoffs/phase-28f.md` | criado |
| `.dev/ferramentas/bin/gitleaks` (fora do git; cópia do checkout principal, sha256 conferido) | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (só material gerado na hora)
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft (não se aplica)
- [x] Ficou dentro do escopo da fase
