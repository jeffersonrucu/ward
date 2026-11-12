# Fase 61a — Correção da Bateria A

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard
  9233ca4` (master atual, "fase 61 concluída; fase 61a de correção").
- **Os 4 testes do Problema 1 da Fase 61**, com a expectativa trocada para o v2 e sem
  afrouxar o que provam:
  - `app/core/src/key.rs` › `reads_the_key_without_returning_the_content`: o literal
    agora é `{"result":"loaded","size":<n>}`, o que o serde produz desde a Fase 54.
  - `crates/wardd/tests/dates.rs` › `attack_v3_41_date_from_the_client_refused`: a
    guarda `"_em\":1"` virou `"created_at":1` ou `"modified_at":1`. Ganhou mais duas
    guardas: o rascunho forjado tem de ser **diferente** do original (um `replacen` que
    não ache o trecho mandaria o original e não provaria nada) e tem de continuar JSON
    válido (a recusa tem de vir do campo de data, não de JSON quebrado). Agora as 4
    variantes (edição com `created_at`, edição com `modified_at`, entrada nova com cada
    um) chegam ao daemon e recebem `InvalidRequest`. O cofre fica com os mesmos bytes, as
    datas não mudam e a entrada nova não existe.
  - `crates/wardd/tests/core.rs` › `v3_9_wrong_password_growing_wait_without_count`: a
    regra "nenhum dígito além de `1`" era do quadro v1. Agora o quadro é lido como objeto
    JSON, o campo `v` tem de existir e valer exatamente `2`, e é tirado. O resto do quadro
    não pode ter nenhum dígito, e o quadro inteiro tem exatamente 1 dígito. É mais
    estrito que antes: o v1 aceitava qualquer quantidade de `1`.
  - `crates/wardd/tests/run.rs` › `f54b_attack_piped_script_on_approval`: espera
    `resposta=invalid_request` e `pedido operacao=run … decisao=once`, os nomes v2 que o
    aprovador de teste imprime (`Operation::name`, `Decision::name`,
    `ResponseError::code`). As palavras do arquivo de modo (`uma_vez`,
    `uma_vez_ciente`) não mudaram, porque o `test-approver/main.rs` ainda lê essas.
    Assim o achado da Fase 56, que o plano guardava para a 62, já sai corrigido aqui.
- **Dois testes intermitentes** que deixavam o `make check` vermelho de vez em quando
  (achados nesta fase, fora dos 4; nenhum defeito no produto):
  - `app/core/src/terminal.rs` › `attack_connect_token_does_not_show_up_in_proc` falhou
    1 vez em 3 rodadas do `make check` com `ExitStatus(512)`. É o `dash` saindo com 2
    num `exec` com erro que não é ENOENT/EACCES, ou seja ETXTBSY. O teste gravava os
    scripts falsos com `std::fs::write` num processo com muitas threads, e um `fork` de
    outro teste segurava o fd de escrita até o `exec` dele. Correção: o script vai para
    um `.tmp` e quem cria o arquivo executado é o `/bin/cp`, então nenhum fd deste
    processo escreve no inode que roda. O ataque não mudou: o leitor de `/proc` em laço,
    os fds reabertos e o token inteiro pelo canal.
  - `crates/ward-platform/src/process.rs` ›
    `attack_process_with_u202e_in_name_reaches_chain_whole` derrubou o `make check`
    aninhado do `test-attacks-foundation` (o nome lido foi `process::tests:`, o da
    thread que fez o `fork`). No `posix_spawn` (`CLONE_VFORK`), o pai volta quando o
    filho larga a memória antiga, no `exec_mmap`, e isso acontece um instante antes de o
    kernel gravar o `comm` novo no mesmo `exec`. Correção: espera de até 2 s até o nome
    deixar de ser o da thread que fez o `fork`. As asserções finais são as mesmas (pid
    certo, nome exato `cl\u{202E}edua`, sem `?`). No produto isso não pesa: o cliente do
    socket já terminou o `exec` antes de conectar.

## Segurança

- **Invariantes tocadas**: nenhuma regra de produto mudou. Os testes voltaram a
  conferir de verdade:
  - V3.41 (data vinda do cliente recusada): o ataque roda nas 4 variantes. Antes a
    guarda parava o teste antes de mandar o pedido.
  - V3.9 / D20 (o quadro de senha errada não revela contagem): mais estrito, só o `2`
    da versão pode aparecer.
  - D68 / f54b (script por pipe na aprovação): recusa sem a caixa, roda com a caixa e
    roda no terminal, conferido pelo log v2 do aprovador.
  - Fase 27a (token do Conectar fora de `/proc`) e D51 (U+202E inteiro na cadeia): só
    perderam a intermitência.
- **Testes de ataque**: os mesmos citados acima, agora verdes no `make check`. O
  `test-attacks-foundation` ficou com 178 `ok` e 0 `FALHA`.
- **Evidência**: `grep -F -f .dev/variations.txt docs/mission/evidence/M6/fase-61a/*`
  não acha nada (usei a lista do checkout principal, só para leitura; o worktree não tem
  `.dev/variations.txt`).
- **Dependências novas**: nenhuma. O `serde_json`, que os testes de integração do
  `wardd` passaram a usar, já é dependência normal do crate.

## Ficou de fora

- Nada do pedido. Os outros nomes pt-BR do aprovador e do app de teste (`papel=`,
  `pedido operacao=`, palavras do arquivo de modo) são literais de teste já aceitos pelo
  `english-check`. Não mexi neles.

## Comandos executados

Logs completos em `.dev/logs/fase61a-*.log` (fora do git). Os da última rodada, sem cor,
estão em `docs/mission/evidence/M6/fase-61a/`. Tudo rodou em `systemd-run --user --scope
-p TasksMax=2000`. `df -h` antes do build: 20 GB livres.

| Comando | Exit code |
| --- | --- |
| `make check` (1ª): tudo verde até o 6/7; o 7/7 falhou por falta do gitleaks no worktree | 2 |
| cópia do `.dev/tools/bin/gitleaks` do checkout principal + `make gitleaks-binary` (sha256 conferido) | 0 |
| `make check` (2ª): `terminal.rs` intermitente (ETXTBSY) | 2 |
| `cargo test -p ward-app-core --lib terminal::` ×15 antes da correção | 0 (15/15) |
| `cargo test -p ward-app-core --lib` ×6 depois da correção | 0 |
| `make check` (3ª) | 0 |
| `make test-attacks-foundation` (1ª): V1.3 caiu pelo `process.rs` intermitente no `make check` aninhado | 2 |
| `cargo test -p ward-platform --lib` ×5 depois da correção | 0 |
| `make check` (final, `make-check.log`): 53 alvos de teste ok, audit, deny, 6× gitleaks "no leaks found", "tudo limpo" | 0 |
| `make test-attacks-foundation` (final, `test-attacks-foundation.log`): 178 ok, "Todos os ataques foram barrados." | 0 |

## Problemas descobertos

1. **Testes que gravam e executam binário num processo com threads**: o
   `process.rs` já tinha uma repetição para ETXTBSY (Fase 55) e o `terminal.rs` não
   tinha. Um teste novo nesse formato deve gravar o executável por um processo filho
   (`cp`) ou repetir no ETXTBSY.
2. **Worktree sem gitleaks**: o `.dev/tools/` não vai para um worktree novo. Copiei o
   binário do checkout principal e o `make` conferiu o sha256 antes de usar. Outra opção
   é `make tools`, que precisa de rede.
3. O `test-attacks-foundation` roda um `make check` inteiro lá dentro, então qualquer
   teste intermitente aparece lá como "ataque passou" (V1.3). Vale lembrar disso ao ler
   uma falha dele.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/core/src/key.rs` | alterado |
| `app/core/src/terminal.rs` | alterado |
| `crates/wardd/tests/dates.rs` | alterado |
| `crates/wardd/tests/core.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/ward-platform/src/process.rs` | alterado (só o teste) |
| `docs/mission/evidence/M6/fase-61a/make-check.log` | criado |
| `docs/mission/evidence/M6/fase-61a/test-attacks-foundation.log` | criado |
| `docs/mission/handoffs/phase-61a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (nenhum daemon instalado nem cofre real tocado)
- [x] Teste de ataque para cada (SEG) tocado (V3.41 agora roda; V3.9 mais estrito)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (os 2 testes intermitentes estão registrados acima: sem eles o `make check` e o `test-attacks-foundation` não ficam verdes de forma confiável)
