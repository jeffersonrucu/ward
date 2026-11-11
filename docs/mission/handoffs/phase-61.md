# Fase 61 — Bateria A (estática e unitária)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 91cc5ec`
  (master atual, "fase 60 concluída").
- **Gitleaks do histórico (decisão do usuário de 2026-11-11: ignorar por fingerprint).**
  Criei `.gitleaksignore` na raiz só com os 12 fingerprints exatos
  (`commit:arquivo:regra:linha`) dos achados que só existem no histórico, os mesmos da
  tabela do handoff da 54a (commits `1ae5610`, `0c09209`, `c0d849e`, `81960fb`,
  `541720e`, `1b2820b`, todos `generic-api-key`). Nada genérico, e o `.gitleaks.toml`
  ficou como estava. Commit separado: `29ae8cf chore(security): ignora por fingerprint
  os 12 achados do histórico (fase 61, m2)`.
  - Antes: o passo `gitleaks git --config .gitleaks.toml` do `make gitleaks` dava
    `leaks found: 12`.
  - Depois: as 6 varreduras do `make gitleaks` (árvore, índice e histórico, normal e
    extra) dão `no leaks found` (`evidence/M6/fase-61/leaks-after.log`).
  - Ataque: plantei numa cópia (`git clone` em `.dev/`, apagada depois) um valor falso
    novo, `api_key = "<24 caracteres aleatórios gerados na hora>"`, já com o
    `.gitleaksignore` copiado. Com o arquivo na árvore, o `gitleaks dir` acusa
    (`leaks-planted.log`). Depois de um commit que apaga o arquivo, o `gitleaks git`
    acusa o commit plantado (`72e2373…:crates/planted_fake.rs:generic-api-key:2`) e só
    ele: os 12 continuam ignorados (`leaks-planted-history.log`). O valor sai
    `REDACTED` nos logs.
- **Bateria A**: rodei todos os alvos da linha da fase e mais o `make test-system-socket`.
  Não corrigi código. Os resultados estão na tabela de comandos e as falhas em
  "Problemas".

### Resultado por asserção

| Asserção | Como | Resultado |
| --- | --- | --- |
| V1.2, V1.3 (checador) | `english_check.py --self-test` (93/93) e `english-check` no `make check` (0 achados, 0 problemas nas exceções) | ok |
| V1.2/V1.3/V1.5 da fundação (m1) | `make test-attacks-foundation` | **falha**: 4 "ataques passaram", todos por consequência do `make check` vermelho (Problema 1) |
| V1.7, V6.11 (gitleaks) | `make gitleaks` com o `.gitleaksignore` | ok (6 varreduras limpas; um valor plantado continua sendo acusado) |
| V2.1, V3.1 (pt-BR) | `english-check` do repositório inteiro, dentro do `make check` | ok |
| V2.2 (fio congelado) | clone em `.dev/` no `01a1e23` (fim do M3, pai direto do commit da Fase 48 `b919e11`), `cargo test -p ward-core --lib wire_v1` | ok, 10/10. `git log` dos testes de fio: as mudanças de `b1c96ff` a `01a1e23` são só renomes da API (36+/36−). Nenhuma fixture de `dev/fixtures/legacy/` mudou até `01a1e23`; depois disso só entrou `install-0.1.4.txt` (arquivo novo) |
| V4.1, V4.2, V4.8, V4.9, V4.15 | `cargo test` do `ward-core` (body/tests, legacy_v4, wire_v2, protocol/tests) e `wardd --test migration` | ok (esses alvos passaram) |
| V5.3 (deny da skill) | `make test-skill` | ok |
| V5.5 (sem test-mode) | `make test-no-test-mode` | ok |
| V5.11 (docs) | `make test-migration-guide` | ok |
| V3.6/V3.7, app | `make app-check` (app-front, app-dist, bidi, npm audit, fmt, clippy, 23 testes, audit, deny, release, V5.6) | ok (com observação do npm audit, Problema 6) |
| V6.1 (`make check` inteiro) | `make check` | **falha** no passo 4/7 (`cargo test`); os passos 0–3 passam e `frozen.txt` não existe. Rodei à parte os passos 5–7: audit 0, deny 0, gitleaks 0 |
| V6.11 (lockfiles) | lockfiles comparados com a `v0.1.4` (`Cargo.lock`, `app/src-tauri/Cargo.lock`, `app/package-lock.json`) | ok: nenhuma dependência de terceiros nova ou com outra versão, só os crates do Ward renomeados e na 0.2.0. O `.gitleaks.toml` só ganhou, desde a 0.1.4, as duas formas base64 de valores de D15 (decisão da Fase 51) |
| dev-vault e variações | `make dev-vault`, `make dev-variations` (290 padrões), `make test-dev-vault` | ok |
| Socket do sistema (54a) | `make test-system-socket` | ok: 59 binários, nenhuma conexão no socket do sistema falso, o controle conecta. Os 22 que não passam no contêiner falham pelo que falta lá dentro (strace, dbus, ssh, Chrome) e pelos 4 testes do Problema 1 |

## Segurança

- **Invariantes tocadas:** S15 (gitleaks). O `.gitleaksignore` só cala os 12 achados
  exatos, que são valores falsos de teste fora de D15 e já trocados na árvore. Um valor
  novo, no mesmo arquivo ou em outro, gera outro fingerprint e é acusado, como mostra o
  ataque plantado. A allowlist do `.gitleaks.toml` não mudou. S1 e S5: o
  `test-system-socket` confirma que nenhum teste fala com o socket do sistema.
- **Testes de ataque**: valor falso novo plantado numa cópia com o `.gitleaksignore`,
  acusado tanto na árvore quanto no histórico (`evidence/M6/fase-61/leaks-planted*.log`).
  Os demais ataques são os das suítes rodadas.
- **Evidência**: `grep -F -f .dev/variations.txt docs/mission/evidence/M6/fase-61/*`
  não acha nada. Os logs completos ficam em `.dev/logs/fase61-*.log`, fora do git. O log
  do `cargo test` cheio tem, na mensagem de pânico do `dates.rs`, o base64 de um valor
  falso de D15. O resumo que foi para a evidência não tem esse valor.
- **Dependências novas**: nenhuma.

## Ficou de fora

- Correções (por regra da fase): cada falha vai para "Problemas", e o orchestrator cria
  as fases de correção.
- A 2ª rodada do `make check` e do `make test-attacks-foundation`, que depende dele, fica
  para depois das correções.
- A linha do `make test-system-socket` na tabela do `CLAUDE.md` continua faltando
  (registrado na 54a). Não editei o `CLAUDE.md`.

## Comandos executados

Logs completos em `.dev/logs/fase61-*.log`; resumos em `docs/mission/evidence/M6/fase-61/`.
As rajadas rodaram em `systemd-run --user --scope -p TasksMax=2000` (3000 no
`test-system-socket`).

| Comando | Exit code |
| --- | --- |
| `make gitleaks` antes do `.gitleaksignore` | 2 (12 achados no histórico) |
| `make gitleaks` depois (`leaks-after.log`) | 0 |
| `make -C <cópia com valor plantado> gitleaks`, árvore (`leaks-planted.log`) | 2 (1 achado: o plantado) |
| idem, só no histórico (`leaks-planted-history.log`) | 2 (1 achado: o commit plantado) |
| `make check` (`make-check.log`) | 2: passos 0–3 ok, 4/7 `cargo test` falha em `ward-app-core --lib` |
| `cargo test --workspace --all-features --locked --no-fail-fast` (`cargo-test-no-fail-fast.log`) | 101: 1042 passaram, 4 falharam, 4 ignorados, 53 alvos |
| `make audit` | 0 |
| `make deny` | 0 |
| `python3 -I dev/english_check.py --self-test` | 0 (93/93) |
| `make dev-services` + `make dev-variations` | 0 |
| `make test-migration-guide` | 0 |
| `cargo test -p ward-core --lib --locked wire_v1` no clone em `01a1e23` (`wire-v1-at-01a1e23.log`) | 0 (10/10) |
| `make dev-vault` | 0 |
| `make test-dev-vault` | 0 |
| `make test-attacks-foundation` | 2 (4 "ataques passaram", Problema 1) |
| `make test-skill` | 0 |
| `make test-no-test-mode` | 0 |
| `make app-check` | 0 |
| `make test-system-socket` | 0 |

## Problemas descobertos

1. **`make check` falha no `cargo test`** (V6.1). Com `--no-fail-fast` são 4 testes,
   todos com expectativa velha, em pt-BR ou v1, que não acompanhou as Fases 50–56. O
   produto não tem defeito aqui.
   - `app/core/src/key.rs:286` `key::tests::reads_the_key_without_returning_the_content`:
     espera `{"resultado":"carregada","tamanho":87}`, e o serde agora dá
     `{"result":"loaded","size":87}`. Causa provável: a Fase 54 (`b843391`) traduziu o
     serde e não mudou o literal do teste.
   - `crates/wardd/tests/dates.rs:204` `attack_v3_41_date_from_the_client_refused`: o
     `assert!(forged.contains("_em\":1"))` procura o sufixo pt-BR de `criado_em` e
     `modificado_em`, e os campos agora são `created_at` e `modified_at`. A guarda do
     próprio teste para antes do ataque, então a recusa de data vinda do cliente não foi
     conferida por este teste nesta rodada. Correção: trocar a guarda por `"_at\":1"`.
   - `crates/wardd/tests/core.rs:114` `v3_9_wrong_password_growing_wait_without_count`:
     `assert!(!s.chars().any(|c| c.is_ascii_digit() && c != '1'))` foi escrito para o
     quadro v1. O quadro v2 tem `"v":2`, e o teste acusa o `2`. A espera crescente
     passou (os `unlock_wait` vêm antes). Correção: excluir o campo de versão ou aceitar
     o `2`.
   - `crates/wardd/tests/run.rs:1140` `f54b_attack_piped_script_on_approval`: espera
     `resposta=pedido_invalido` e
     `pedido operacao=executar … decisao=uma_vez`, e escreve `uma_vez <PIN>` no arquivo de
     modo. O aprovador de teste já fala v2 (`operacao=run … decisao=once`,
     `resposta=invalid_request`). É o achado da Fase 56 que o plano previa para a 62, e
     aparece já aqui porque o `make check` roda esse teste.
2. **`make test-attacks-foundation` falha (4)**, só por consequência do Problema 1: as
   checagens V1.2 ("make check não pegou leak.txt", "sem o arquivo, make check continua
   falhando"), V1.3 ("não falhou em cargo audit pelo advisory") e "o ambiente do usuário
   mudou o make check" esperam um `make check` que chegue aos passos de audit e gitleaks,
   e ele para no `cargo test`. As outras 174 checagens passaram. Rodar de novo depois da
   correção do 1.
3. **`.gitleaksignore` e reescrita futura**: os fingerprints levam o hash do commit. Se o
   histórico for reescrito (rebase, filter-repo), os achados voltam com outros hashes, e
   o arquivo tem de ser refeito.
4. **Guarda do worktree**: comando Bash que cite a palavra `gitleaks` junto com
   redirecionamento ou outros comandos é recusado (o guarda lê "git"). `make gitleaks`
   sozinho passa. A Fase 60 já tinha registrado algo parecido.
5. **Cuidado com `cd` nas chamadas**: o diretório volta ao worktree a cada chamada.
   Um `cd <cópia> && …` seguido de `git commit` em outra chamada caiu no worktree
   (desfeito com `git reset --soft HEAD~1` antes do commit certo; nada se perdeu). Nas
   cópias, usei `git -C <cópia>`.
6. **`npm audit` do app**: o `--audit-level=moderate` passa, mas mostra 2 avisos de
   severidade baixa: `deepmerge >=4.2.0` (GHSA-gcqx-w48m-w2h4, prototype poisoning), via
   `@sveltejs/vite-plugin-svelte`, só de build, sem correção publicada. O
   `package-lock.json` é igual ao da 0.1.4, então não é regressão. O V6.11 pede
   "`npm audit` limpo": o validador decide se baixa severidade sem correção conta.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `.gitleaksignore` | criado (commit `29ae8cf`) |
| `docs/mission/evidence/M6/fase-61/leaks-after.log`, `leaks-planted.log`, `leaks-planted-history.log` | criados (commit `29ae8cf`) |
| `docs/mission/evidence/M6/fase-61/{make-check,cargo-test-no-fail-fast,audit,deny,english-self-test,test-migration-guide,wire-v1-at-01a1e23,dev-vault,test-dev-vault,test-attacks-foundation,test-skill,test-no-test-mode,app-check,test-system-socket}.log` | criados |
| `docs/mission/handoffs/phase-61.md` | criado |

## Procedimentos
- [x] `make check` rodado (falha no `cargo test`, Problema 1; audit, deny e gitleaks rodados à parte: 0)
- [x] Só cofre de teste e credenciais falsas usados (`.dev/test-vault.ward`; nenhum daemon de teste ou instalado tocado; valor plantado aleatório e só numa cópia apagada)
- [x] Teste de ataque para cada (SEG) tocado (S15: valor plantado continua acusado)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (nenhum código corrigido)
