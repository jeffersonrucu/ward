# Fase 65b — Código TOTP no detalhe (UI/DESIGN, pedido do usuário)

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 481f321`
  (master atual, "docs(mission): fase 65b").
- **Detalhe da entrada com TOTP** (`app/src/screens/Credentials.svelte`): o campo
  "Semente TOTP" (revelar/copiar a semente) saiu. No lugar dele entra o componente
  `TotpCode` com o rótulo "Código TOTP": o código atual em dois grupos de 3 dígitos
  ("785 537"), a contagem até o próximo ("22 s") e o botão Copiar.
  - O código vem do comando Tauri `totp_code`, que já existia: `Service::totp_code` →
    op `totp` do protocolo v2, e o daemon calcula. O pedido sai uma vez, a contagem
    corre no front e, quando chega a 0, sai um pedido novo.
  - O Copiar chama `copy_totp_code`, que já existia: o código vai para a área de
    transferência pelo `Copier` do Rust. A limpeza e as regras de cópia (D4) são as
    mesmas do copiar de campo, e a resposta não traz valor.
  - O código sai da tela quando se troca de entrada, ao abrir "Nova credencial" ou
    Editar, ao travar e ao sair da tela. Um pedido atrasado não pinta o código em
    outra entrada (contador `totpRequest`).
  - Se o pedido falhar, o detalhe mostra o erro no toast.
- **Defesa em profundidade no front** (`app/src/screens/credentials.js`):
  - `detailSecrets` desenha todos os segredos menos `totp_seed`.
  - `hasTotp` diz se a entrada tem semente.
  - `detailField` faz `reveal()`/`copy()` do detalhe recusarem `totp_seed` e campo
    vazio ou que não seja string.
  - `detail.secrets` continua com o id `totp_seed` (só id e rótulo, nunca o valor)
    porque o formulário de edição usa esse id para saber que há semente salva.
- **Formulário de edição**: sem mudança de comportamento. A semente continua lá (com
  revelar e trocar), ao lado do "Código atual". A única mudança visível é o código em
  dois grupos de 3, como o componente do FigCraft.
- **`TotpCode.svelte`** ganhou três props: `label` (padrão "Código atual"), `emptyText` e
  `emptyHelp`. O detalhe usa "Calculando o código…" enquanto o primeiro pedido não
  volta.
- **Sem mudança de protocolo, do daemon ou de capability do Tauri**: `allow-totp-code` e
  `allow-copy-totp-code` já estavam em `capabilities/main.json`, e a ponte de teste já
  aceitava os dois comandos.
- **Design (FigCraft headless, `/home/jeffersonoliveira/documentos/Ward.figcraft`)**:
  - O set `Código TOTP` `6909` ganhou a propriedade de texto `Rótulo` (padrão "Código
    atual"), ligada aos textos `6895` e `6911`.
  - Tela nova `Credenciais / Detalhe com TOTP` `23159` (x 6800, y 2000), cópia da
    `Credenciais / Bloqueada`, com `acme-totp-dev` selecionada:
    - URL e Usuário lado a lado, como a grade de duas colunas do app;
    - Senha;
    - `Código TOTP` `23390` com "482 913" e "18 s";
    - cartão `Nível=livre` `23405`.
  - Arquivo salvo e reaberto (a tela está no arquivo reaberto).
  - Ids em `docs/mission/design-nodes.md` › "Fase 65b".
- **PNG** (pedido da memória do usuário), em `docs/mission/evidence/fase-65b/`:
  - `design-credenciais-detalhe-com-totp.png`: frame `23159` do FigCraft;
  - `design-codigo-totp-componente.png`: set `6909`;
  - `app-credenciais-detalhe-com-totp.png`: app, build de teste pela ponte de teste com
    o daemon falso do `ward-app-core`, Chrome headless 1280×800. Bate com o frame.
    Diferenças: o código é outro (é o atual); o rodapé não tem datas, porque o daemon
    falso não tem.

## Segurança

- **Invariantes tocadas**:
  - **S11** (o valor só entra na tela no "revelar"): a semente nunca entra no detalhe.
    Nada a pede e nada a revela, e o detalhe recebe só o id.
  - **D4**: a cópia do código é feita no Rust e a resposta não carrega valor.
  - O código TOTP fica só no estado local do componente: nada em estado global,
    armazenamento ou URL.
  - Nenhuma entrada nova não confiável: os comandos já validam o nome da entrada no
    core (`validate_entry_name`).
- **Testes de ataque escritos**:
  - `dev/tests/totp_detail.py`, pelo `make test-totp-detail` (ponte de teste +
    `--fake-daemon full`, Chrome headless pelo Playwright do Python, extensões
    desligadas, perfil temporário):
    - `acme-totp-dev` e `acme-wp-dev` mostram o código em 6 dígitos, a contagem anda e
      o Copiar chama `copy_totp_code` daquela entrada;
    - `acme-pg-dev` não mostra código nem pede `totp_code`;
    - **ataque**: todos os botões do detalhe são clicados e nenhuma chamada nomeia
      `totp_seed`; a semente não aparece em nenhuma resposta nem na página (HTML, texto
      e valores dos campos), em nenhuma forma (como salva, minúsculas, em grupos de 4,
      `secret=`, bytes decodificados);
    - **controle**: no formulário de edição a semente continua revelável, e o mesmo
      detector a acha no pedido, na resposta e na página. De volta ao detalhe, a página
      fica sem ela.
    - As cópias são respondidas pelo próprio teste, então a área de transferência da
      sessão não é tocada.
    - **Mutação**: com o `Credentials.svelte` do master, o teste dá 7 FAIL. Ele acusa
      `reveal_field`/`copy_field` com `totp_seed`, a semente na resposta e na página, e
      o rótulo da semente no detalhe.
  - `dev/tests/credentials_totp.mjs` (node, dentro do `make test-totp-detail`): 8 casos
    de `detailSecrets`/`hasTotp`/`detailField`. Inclui o ataque com `totp_seed`, vazio,
    `null`, número, objeto, array e objeto com `toString` que devolve `totp_seed`.
  - `app/core/src/credentials.rs` › `attack_totp_detail_without_the_seed`: o detalhe de
    `acme-totp-dev`/`acme-wp-dev` tem `totp_seed` só como id e nenhuma forma da semente
    no JSON; a resposta `totp` traz 6 dígitos e 1–30 s.
- **Dependências novas**: nenhuma.

## Ficou de fora

- **App Tauri nativo (WebKitGTK)**: não rodei o `make app-check`, que inclui o build de
  release do Tauri. A mudança é só no front e o `make compile` fez o `cargo check` do
  `app/src-tauri`. A conferência foi no Chrome pela ponte de teste, como manda a fase
  UI.
- **Ponte de teste com o `wardd` de teste real (`make app-bridge`)**: não precisei dela.
  Usei o daemon falso do `ward-app-core`, que calcula o TOTP pela mesma `ward_core::totp`
  e não precisa de socket, cofre de teste nem daemon. Nenhum pedido de permissão foi
  negado.

## Comandos executados

Logs `fase65b-*.log` no scratchpad da sessão. Não ficam no repositório: têm caminhos
locais.

| Comando | Exit code |
| --- | --- |
| `make app-front` | 0. O `npm audit` acusa os mesmos 2 avisos *low* de `deepmerge`, só em dev, como na Fase 65 |
| `make english-check` | 0 |
| `make compile` | 0 |
| `cargo test --locked -p ward-app-core` | 0 (202 + 8 testes, inclusive o novo) |
| `make test-totp-detail PW_PYTHONPATH=<repo>/.dev/pw-lib/site` | 0 (22 ok, 0 falha) |
| mutação: `totp_detail.py` contra o `Credentials.svelte` do master | 1 (7 FAIL, o esperado) |
| `make check` (1ª) | 2: o english-check acusou pt-BR na docstring do `totp_detail.py`; corrigido |
| `make check` (2ª) | 2 só no passo 7/7: faltava o gitleaks no `.dev/tools` do worktree. Os passos 0/7–6/7 (origens, english-check, fmt, clippy, test, audit, deny) passaram |
| `cp -a <checkout>/.dev/tools .dev/` + `make tools` (sha256 conferido) | 0 |
| `make gitleaks` (passo 7/7: árvore, índice, histórico e a varredura extra) | 0 (sem vazamento) |

## Problemas descobertos

- **Disco**: no começo da fase havia 4,2 GB livres, abaixo dos ~13 GB pedidos. Os
  `target/` de outros worktrees ocupavam espaço e não eram meus, então não mexi neles.
  Rodei build por build, olhando o `df`. Depois o espaço voltou a 23 GB, porque outra
  sessão limpou.
- **Pílula da contagem**: no app ela tem fundo `badge-neutral-bg`; no componente `6909`
  do FigCraft o "18 s" aparece sem fundo visível. A diferença já existia (Fase 33) e
  ficou como está.
- O `TotpCode` mostra o código em dois grupos de 3 também no formulário (antes eram 6
  dígitos juntos). Agora segue o componente do FigCraft. O valor copiado continua sem
  espaço, porque sai do Rust.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `app/src/screens/Credentials.svelte` | alterado |
| `app/src/screens/credentials.js` | alterado |
| `app/src/lib/components/TotpCode.svelte` | alterado |
| `app/core/src/credentials.rs` | alterado (teste) |
| `dev/tests/totp_detail.py` | criado |
| `dev/tests/credentials_totp.mjs` | criado |
| `Makefile` | alterado (alvo `test-totp-detail` no fim, com `.PHONY` próprio para não mexer na lista grande que a 65a também toca) |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 65b") |
| `docs/mission/evidence/fase-65b/*.png` | criados (3) |
| `docs/mission/handoffs/phase-65b.md` | criado |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório) |

## Procedimentos

- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (daemon falso; semente de teste de D15)
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
