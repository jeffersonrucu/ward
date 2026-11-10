# Fase 3 — Linha de base legada (DT18) (Missão 2, Milestone 1)

> **Decisão do usuário (opção a), já aplicada.** Na primeira geração, o gitleaks com
> `.gitleaks.toml` acusou `protocol-v1.jsonl:47` (`generic-api-key`): era o base64 de
> `falso_token_github_01` na resposta `token` de amostra. Seguindo o PRD DT18, a fase
> parou e perguntou. O usuário escolheu trocar a amostra do token por `135790`, o PIN de
> D15. O base64 dele (`MTM1Nzkw`) tem 8 caracteres e fica abaixo do mínimo da regra.
> A fixture foi trocada pela versão alternativa, que o código da tag gerou. O
> `SHA256SUMS` foi refeito. Os dois gitleaks sobre `dev/fixtures/legacy/` não acusam
> nada, e os 10 testes de fio passaram com essa fixture contra a v0.1.4. O allowlist
> não mudou.

## Implementado
- **Worktree da tag:** `git worktree add --detach .dev/baseline/src v0.1.4`.
  - Antes, copiei `.dev/servicos/chaves/` (a chave falsa do deploy) do checkout principal para a worktree. Assim, o `acme-ssh-chave-prod` do cofre legado usa a mesma chave dos serviços de teste de pé.
  - Removida no fim com `git worktree remove --force` e `prune`, junto com o `target/` dela. `git worktree list` mostra só o checkout principal.
- **`dev/fixtures/legacy/`** (no git; só valores falsos, todos gerados pelo código da v0.1.4):
  - `vault-0.1.4.ward`: o cofre de teste completo.
    - Saiu do `make dev-cofre` da tag: 22 entradas, corpo v4.
    - Depois, duas gravações reais pelo `wardd` de teste 0.1.4 (veja "Como as gravações foram feitas"). Geração 3, trava em 240 min, backup externo desligado.
  - `state-0.1.4`: `ward-estado 1` / `vista 3` / `queimada 3` / `falhas_agente 0`, gravado pelo daemon 0.1.4.
  - `backup-0.1.4/`: as duas versões que o backup local do daemon gravou.
    - `ward-20261110-213053.ward` (geração 2) e `ward-20261110-213054.ward` (geração 3).
    - A segunda é idêntica byte a byte ao `vault-0.1.4.ward`.
  - `protocol-v1.jsonl`: 91 linhas no formato `{"kind":"request|response|error","role":"<papel v1>","frame":<quadro exato da 0.1.4>}`.
    - 36 pedidos, um por operação, na ordem de `Operacao::TODAS`.
    - 23 respostas, que cobrem todos os 20 tipos de resposta (inclui `Validacao` e três `pedido_de_aprovacao`).
    - 32 erros, um por código de `ErroResposta::SIMPLES`.
    - Montado a partir dos tipos (`um_pedido_de_cada`/`dados_validos`/`uma_resposta_de_cada` dos testes do protocolo da tag). Os segredos foram trocados por valores de D15:
      - `falso-clientex-01` → `falso-WP-senha-01`;
      - `falso-nova-02` → `falso-SSH-senha-01`;
      - PIN novo `2468` → `135790`;
      - `segredo` → `falso-WP-senha-01`;
      - `token` → `135790` (decisão do usuário; veja o topo).
    - Os códigos TOTP de amostra (`123456`, `654321`) ficaram.
  - `list-v1.json`: saída do `ward lista --json` da CLI de teste 0.1.4, contra o daemon destravado. 21 entradas (a bloqueada não aparece), com as chaves `nome, tipo, grupo, ambiente, projeto, endereco`.
  - `help-0.1.4.txt`: `ward --help` do binário de release 0.1.4. É idêntico ao da CLI de teste.
  - `strings-0.1.4.txt`: 4 294 literais, saídos de `english_check.py --literals` (sem localização, ordenados, sem repetição).
    - O checador foi copiado temporariamente para a worktree, e só os arquivos versionados da tag entraram.
    - Entraram 178 arquivos:
      - `crates/{ward,wardd,ward-nm,ward-core,ward-plataforma}/src`;
      - `app/nucleo/src`;
      - `app/src-tauri/{src,capabilities,tauri.conf.json}`;
      - `app/index.html`;
      - `app/src`;
      - `extensao/`.
    - Ficaram de fora os arquivos só de teste: qualquer caminho com "test" (`*/testes.rs`, `ssh_testes.rs`, `cofre_teste.rs`, `ward-plataforma/src/teste.rs`, `lib/teste/Vitrine.svelte`, `lib/ponte/ponte-teste.js`).
    - Os módulos `#[cfg(test)]` dentro de arquivos de produto entraram: o lexer não os separa. Para o V6.3, a comparação usa o mesmo conjunto de caminhos com os nomes novos.
  - `SHA256SUMS`, na própria pasta: `sha256sum -c SHA256SUMS` dá ok.
- **`crates/ward-core/src/wire_v1.rs`** (novo; `#[cfg(all(test, unix))] mod wire_v1;` em `lib.rs`): os testes de fio do V2.2, todos em inglês e guiados pelos dados.
  - `v1_requests_read_and_written_byte_for_byte`: cada pedido da fixture é lido pelo código atual com o papel da linha e escrito de novo, e os bytes têm de ser iguais. A ordem das operações tem de ser `Operacao::TODAS`.
  - `v1_responses_read_and_written_byte_for_byte`: o mesmo para as respostas. Todos os tipos de `TipoResposta::TODAS` aparecem, 23 linhas.
  - `v1_error_codes_read_and_written_byte_for_byte`: o mesmo para os erros, iguais a `ErroResposta::SIMPLES`.
  - `v1_fixture_is_version_1` e `v1_fixture_has_only_known_kinds`.
  - `vault_014_body_v4_read_and_written_byte_for_byte`:
    - abre a fixture com a senha de teste (`formato::abrir`) e confere `VERSAO_CORPO == 4` e o `u16` do corpo igual a 4;
    - `desserializar` seguido de `serializar` dá os mesmos bytes;
    - geração 3;
    - os nomes, tipos e níveis gravados (v4) são iguais aos da tabela `ENTRIES_V4`;
    - 240 min de trava.
  - `backup_014_two_consecutive_generations`: a versão mais nova é o cofre. A mais velha tem geração 2 e, escrita na geração 3, dá o corpo do cofre.
  - Ataques: `attack_renamed_wire_name_is_refused`, `attack_vault_014_byte_flipped_does_not_open` e `vault_and_backups_014_have_no_value_in_clear` (veja Segurança).
  - **Rodados uma vez contra o código da tag**, na worktree: 10/10 ok, com a fixture atual incluindo a fixture final, com o token `135790`. Isso confere as fixtures (V1.6, V2.2). No checkout principal, os testes só compilam (`make compile`), pela regra 6. O código do `ward-core` lá é o mesmo da tag.
- **`.dev/baseline/`** (fora do git, 0700/0600, com `SHA256SUMS` próprio):
  - `bin-release/`: `ward`, `wardd` e `ward-nm` de release 0.1.4, por pacote e sem `modo-teste` (`strings` sem `WARD_DEV_*`), mais o `partida-conteiner` de release.
  - `bin-test-mode/`: `ward`, `wardd` e `ward-nm` com `modo-teste` (debug, como os do `make dev-daemon`; o `wardd` tem as marcas), mais `wardd-app-teste` e `wardd-aprovador-teste`.
  - `deb-0.1.4/ward_0.1.4_amd64.deb`:
    - montado pelo `make deb` da tag com os binários de release acima e `BIN_APP` = `partida-conteiner` (como faz o `make teste-deb`; o app Tauri não foi compilado);
    - CRX3 falso `ward-fake.crx` (`crx_falso.py --versao 0.1.4`), com o id em `extension-id.txt` (`nnnmelocmlolknddjimbajgabpmikdab`);
    - `Version: 0.1.4`, `Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0`.
  - `extension-0.1.4/`: a extensão montada pelo `make extensao` da tag (id `nhkehjnmlgkebhmelajfpmpblepaichf`).
  - `generator-protocol-v1.rs` e `linha_base.rs`: o gerador da `protocol-v1.jsonl` e o app auxiliar das gravações, com o modo de uso no cabeçalho, para refazer a linha de base se preciso.
- **Como as gravações foram feitas:** só na worktree, com o cofre dela, sem tocar no daemon de teste do checkout principal nem no `wardd` real.
  - O `wardd` de teste 0.1.4 subiu por `dev/daemon.sh` com:
    - `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-baseline`, pasta própria, apagada no fim;
    - `WARD_DEV_APP=<worktree>/target/debug/examples/linha_base`.
  - `dev/abrir-app.py` abriu o app auxiliar com o papel app. Ele:
    - destravou com a senha de teste;
    - pediu `tempo_trava 240` duas vezes, com 1,5 s entre elas. É o mesmo valor, então a configuração não muda, mas cada pedido grava, o que dá as gerações 2 e 3 e duas versões de backup.
  - Depois, `ward lista --json` (agente), `ward travar` e `dev/daemon.sh parar`.
- **`.gitignore`:** duas exceções exatas ao `*.ward`: `!/dev/fixtures/legacy/vault-0.1.4.ward` e `!/dev/fixtures/legacy/backup-0.1.4/*.ward`. Sem elas, as fixtures cifradas não entrariam no git.
- **`docs/glossario.md`:** as linhas "novos (Fases 2 e 3)" e "novos (Fase 3)" agora listam os arquivos da fixture, o `wire_v1.rs` e a organização de `.dev/baseline/`.
- **Renomes, literais e exceções:**
  - Renomes: nenhum.
  - Literais alterados em código existente: nenhum.
  - Literais novos: só os do `wire_v1.rs`, que é teste.
  - Entradas novas de `allow.txt`/`frozen.txt`: nenhuma.

## Segurança
- **Invariantes tocadas:**
  - **S15 (V1.7).**
    - Só valores falsos: os segredos dentro das fixtures são valores de D15. A lista foi conferida decodificando todo base64 da `protocol-v1.jsonl`.
    - O cofre e os backups estão cifrados.
    - A chave do deploy dentro do cofre é a chave falsa de `.dev/servicos/chaves/` (gerada em `.dev/`, nunca no git) e só aparece cifrada.
    - Os dois gitleaks sobre as fixtures não acusam nada, depois da troca do token pela decisão do topo.
  - **S2.** Nenhum valor em claro no cofre e nos backups da fixture (teste abaixo).
  - **S6.** Um byte alterado não abre (teste abaixo).
  - **RM1/S1 (V2.2).** Os testes de fio prendem os nomes de operação, campo, erro, papel e o corpo v4.
- **Testes de ataque escritos** (`crates/ward-core/src/wire_v1.rs`):
  - `attack_renamed_wire_name_is_refused`: cada pedido da fixture, com o nome da operação trocado, recebe `OperacaoDesconhecida`, e com o campo `entrada` renomeado para `entry`, `PedidoInvalido`. É isso que garante que um rename no fio quebra os testes em vez de passar calado.
  - `attack_vault_014_byte_flipped_does_not_open`: um bit trocado na magia, no cabeçalho, no meio ou no último byte não abre, e a senha errada também não (S6).
  - `vault_and_backups_014_have_no_value_in_clear`: nenhum valor falso de D15, nem "falso" nem "OPENSSH", aparece em claro no cofre nem nos dois backups (S2, V1.7).
- **Dependências novas:** nenhuma. Os testes usam só o `ward-core` e a std. O `include_bytes!` lê as fixtures em tempo de compilação.
- **Nunca tocado:** cofre real, `wardd` real (`/usr/local/bin/wardd`, pid do usuário), daemon de teste do checkout principal (socket `ward-teste`), `~/.ssh`, perfil do Chrome. Nenhum Docker foi usado.

## Ficou de fora
- **O app Tauri 0.1.4 de release:** o `.deb` da linha de base leva o `partida-conteiner`, como o do `make teste-deb`. A atualização em contêiner (Fase 57) usa esse app no lugar do `ward-app`. Compilar o Tauri custaria vários GB de disco, que já está com 15 GB livres. Se a Fase 57 precisar do `ward-app` de verdade, ele pode ser feito do mesmo jeito, numa worktree da tag.
- **Teste automático da `list-v1.json`, do `help-0.1.4.txt` e do `state-0.1.4`:** ficam para as fases donas (51, 55 e 49). Os três estão no `SHA256SUMS`.
- **`make check` e o `make gitleaks` inteiro:** não rodaram, pela regra 6. O gitleaks rodou só sobre as fixtures novas e sobre o `wire_v1.rs`, como pede o plano; depois da troca do token, nenhum achado.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git worktree add --detach .dev/baseline/src v0.1.4` | 0 |
| (worktree) `make dev-cofre` | 0 |
| (worktree) `cargo build --locked -p wardd/ward/ward-nm --features modo-teste …` | 0 |
| (worktree) `dev/daemon.sh subir` (socket próprio, `WARD_DEV_APP=linha_base`), `dev/abrir-app.py`, `ward lista --json`, `ward travar`, `dev/daemon.sh parar` | 0 |
| (worktree) `make dev-cofre-conferir` (22 entradas, geração 3) | 0 |
| (worktree) `cargo test -p ward-core --lib -- --ignored gerar_fio_v1` (gerador) | 0 |
| (worktree) `cargo test -p ward-core --lib wire_v1` (10/10, fixture atual e alternativa) | 0 |
| (worktree) `cargo clippy -p ward-core --all-targets -- -D warnings`; `cargo fmt -p ward-core --check` | 0 |
| (worktree) `cargo build --release --locked -p ward/wardd/ward-nm`, `-p ward-app-nucleo --example partida-conteiner` | 0 |
| (worktree) `npm ci --ignore-scripts --offline`, `make extensao` | 0 |
| (worktree) `make deb … BIN_APP=partida-conteiner` | 0 |
| (worktree) `english_check.py --literals --paths <178 arquivos de produto>` | 0 |
| `sha256sum -c SHA256SUMS` (fixtures) | 0 |
| `gitleaks dir --config .gitleaks.toml dev/fixtures/legacy` (1ª geração) | 1 (linha 47; levou à decisão do topo) |
| a fixture alternativa aplicada, o `SHA256SUMS` refeito e `sha256sum -c` | 0 |
| `gitleaks dir --config .gitleaks.toml dev/fixtures/legacy` (depois da troca) | 0 |
| `gitleaks dir --config dev/gitleaks-extra.toml --max-archive-depth 3 dev/fixtures/legacy` | 0 |
| `gitleaks dir` (os dois configs) em `crates/ward-core/src/wire_v1.rs` | 0 |
| `make compile` (depois da troca) | 0 |
| `git worktree remove --force .dev/baseline/src` e `git worktree prune` | 0 |
| `make compile` | 0 |
| `make english-check PATHS="crates/ward-core/src/wire_v1.rs crates/ward-core/src/lib.rs dev/fixtures/legacy .gitignore"` | 1 (veja Problemas 2) |
| `make check` | não rodado (regra 6 do PRD) |

## Problemas descobertos
1. **Gitleaks na `protocol-v1.jsonl:47` (resolvido):** a causa era a regra `generic-api-key` sobre o base64 de um valor de D15 ao lado da chave `"token"`. Ficou resolvido pela decisão do topo, sem mexer no allowlist. A alternativa em `.dev/baseline/alternatives/` foi apagada, e o `generator-protocol-v1.rs` agora usa `TOKEN_SAMPLE = "135790"`.
2. **`english-check` dos caminhos da fase não fica limpo.**
   - Os 187 achados são de três fontes, e nenhum é texto escrito por esta fase:
     - no `wire_v1.rs`, só identificadores da API atual do `ward-core` (`protocolo`, `decodificar_pedido`, `ErroResposta`, `SenhaMestra`, `corpo::serializar`…);
     - no `lib.rs`, linhas que já existiam;
     - no `.gitignore`, comentários que já existiam.
   - Os comentários e os identificadores próprios do arquivo são em inglês, e as linhas novas do `lib.rs` e do `.gitignore` não têm achado.
   - Somem quando as Fases 10–18 renomearem a API.
3. **V2.2 e o `git log` dos testes de fio.**
   - O contrato diz que as Fases 7–47 não alteram esses testes. Mas o `wire_v1.rs` usa a API do `ward-core`, e as Fases 10–18 vão ter de renomear os usos nele.
   - Para reduzir isso, os testes ficaram guiados pelos dados: leem a fixture e escrevem de novo, com o mínimo de API. Sugiro que o validador do V2.2 aceite, nesse arquivo, diffs que sejam só de nomes (como a regra 7), e não aceite mudança de fixture nem de expectativa.
   - O arquivo fica na raiz de `src/` com nome em inglês, então nenhuma fase precisa movê-lo.
4. **"Corpo serializado do cofre de teste igual ao de `vault-0.1.4.ward`".** Foi interpretado como ler e escrever de novo, byte a byte, mais os nomes, tipos e níveis gravados. Gerar o corpo do zero não dá os mesmos bytes: a chave ed25519 do deploy, o sal do hash do PIN e as datas mudam a cada `make dev-cofre`.
5. **`strings-0.1.4.txt` tem literais de teste.**
   - Vêm dos módulos `#[cfg(test)]` dentro de arquivos de produto (ex.: `falso-outro-39b`, `AAAAC3Nz…falso`, cabeçalhos `-----BEGIN `). São valores falsos que já estão no código-fonte versionado. O gitleaks não acusa nada nesse arquivo.
   - Se o V1.7 for lido como "nada além de D15, nem em literal de teste", esse arquivo teria de ser filtrado, e o V6.3 compararia com o mesmo filtro.
6. **Versão do manifesto da extensão 0.1.4:** é `0.1.0` (não subiu na 0.1.x). A Fase 60 (DT19) põe `0.2.0`.
7. **Disco:** o `target/` do checkout principal tem 20 GB (16 GB em `target/debug`) e o disco está com 15 GB livres. Não apaguei nada do checkout principal. A worktree e o `target/` dela foram removidos.
8. **Estado do repositório:** igual ao das Fases 1 e 2 (`docs/escopo.md` modificado e renomes indexados de `docs/mission/evidence/M0/…`). Não mexi nisso.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/fixtures/legacy/vault-0.1.4.ward` | criado |
| `dev/fixtures/legacy/state-0.1.4` | criado |
| `dev/fixtures/legacy/backup-0.1.4/ward-20261110-213053.ward` | criado |
| `dev/fixtures/legacy/backup-0.1.4/ward-20261110-213054.ward` | criado |
| `dev/fixtures/legacy/protocol-v1.jsonl` | criado |
| `dev/fixtures/legacy/list-v1.json` | criado |
| `dev/fixtures/legacy/help-0.1.4.txt` | criado |
| `dev/fixtures/legacy/strings-0.1.4.txt` | criado |
| `dev/fixtures/legacy/SHA256SUMS` | criado |
| `crates/ward-core/src/wire_v1.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (declaração `#[cfg(all(test, unix))] mod wire_v1;`) |
| `.gitignore` | alterado (duas exceções exatas para os `.ward` da fixture) |
| `docs/glossario.md` | alterado (linhas "novos" das Fases 2/3 e 3) |
| `docs/mission/handoffs/phase-3.md` | criado |
| `.dev/baseline/**` (fora do git) | criado |

## Procedimentos
- [ ] `make check` rodado: não, pela regra 6 do PRD. No lugar dele, rodaram `make compile` (ok), o `english-check` da fase (Problemas 2), o gitleaks das fixtures (limpo, depois da decisão) e os testes de fio contra o código da tag (10/10).
- [x] Só cofre de teste e credenciais falsas usados.
- [x] Teste de ataque para cada (SEG) tocado: V1.7 e V2.2, com fio renomeado, byte alterado e valor em claro.
- [x] (UI/DESIGN) não se aplica.
- [x] Ficou dentro do escopo da fase.
