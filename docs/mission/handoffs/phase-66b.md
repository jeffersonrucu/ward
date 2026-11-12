# Fase 66b — Ressalvas dos validadores do M6 e artefatos 0.2.0 finais

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`; com a árvore limpa, `git reset --hard 2b74fd3`
  (master, "QA do M6 (aprovado com ressalva); ressalva na fase 66b").
- **user-testing-validator (ressalva)**: `dev/tests/app_e2e.py` ganhou
  `unlock_button_ready` (botão de envio do formulário com o rótulo exato "Destravar",
  habilitado, e campo editável). `unlock()` espera por ele antes de digitar a senha (sai com
  erro claro se não voltar em 40 s), e `unlock_screens` checa "o botão 'Destravar' volta
  depois da espera". Durante a espera de D20 só o botão desabilita e o Enter é
  descartado; o campo continua habilitado, por isso a espera antiga não servia.
  `make test-app-e2e` passou 2 vezes seguidas: 152/152 nas duas.
- **security-auditor, médio (roteiro 3b)**:
  - `docs/mission/roteiro-usuario.md` 3b não roda mais `sudo make install` do checkout.
    Copia para `/root/ward-src` (0700, root) o instalador `ward-install-0.2.0.tar`, os
    binários e o `.crx`, confere os sha256 como root, extrai com `--no-same-owner`, pede
    para ler o `install.sh` e roda `sudo make -C /root/ward-src install` com `BIN_*` e
    `EXTENSION_CRX` dentro de `/root/ward-src`. O uninstall também sai de lá.
  - O roteiro diz que esta máquina tem a 0.1.5 pelo `.deb` (o caminho é o 3a). O passo
    9.1 (voltar para a 0.1.x) também passou a usar a cópia em `/root`.
  - `docs/instalacao.md` traz o mesmo aviso: 2b (o `git archive` do clone gera o
    instalador, copiado para `/root/ward-src` e rodado de lá), a lista das duas formas,
    o item 0.3 ("Código de confiança") e o uninstall (4b e 6), com
    `make -C /root/ward-src uninstall`.
  - `README.md`: "a partir do código" agora gera o `.deb` e aponta o 2a/2b do roteiro, sem
    `sudo make install` do checkout.
- **security-auditor, baixo (`show_window` sem limite)**: `crates/wardd/src/approval.rs`
  ganhou `SHOW_WINDOW_INTERVAL` (1 s) e `State::last_show`, contado pelo relógio do motor.
  Um pedido a menos de 1 s da última entrega junta-se a ela: quem chamou recebe `ok` e o
  app não recebe nada. Assinante novo zera o intervalo; sem assinante, nada conta.
  - A entrega continua com o lock do motor preso. O assinante mora no estado, como em
    todos os avisos, para manter a ordem; tirá-lo do lock mexeria na entrega das
    aprovações.
  - A escrita tem o `WRITE_DEADLINE` de 10 s do socket, e agora é no máximo uma por
    segundo. Está registrado no comentário de `show_window`.
- **security-auditor, baixo (leitor v2)**: escolhi documentar a decisão com teste, sem
  apertar o leitor:
  - `data` ausente vale `{}` e os nomes podem ter escape JSON. As duas formas já valiam na
    0.1.x, e a extensão (`popup.js` `open_app`, `login.js` `await_login`), o `ward-nm`
    dos testes e as suítes de contêiner/nm mandam `{"v":2,"op":"status"}` sem `data`.
  - Nada compara bytes do quadro: o nome é decodificado, procurado na tabela exata e só
    então o papel é conferido. O `ward-nm` recodifica na forma canônica (sempre com
    `data`) antes do daemon.
  - Exigir `data` mudaria a extensão e várias suítes que não rodei (nm, extension, login,
    docker-platform). A decisão está no doc do módulo `crates/ward-core/src/protocol.rs`.
- **Metainfo**: `install/desktop/local.ward.app.metainfo.xml` com
  `<release version="0.2.0" date="2026-11-11"/>` e `<release version="0.1.5"
  date="2026-11-10"/>` (`appstreamcli validate --no-net`: ok, 2 infos que já existiam).
  O `.deb` novo leva esse arquivo.
- **Evidências do M6 sem o home**: os 11 logs de `docs/mission/evidence/M6/` (fase-61,
  fase-61a, fase-62) tiveram o caminho trocado: worktree e checkout viraram `<repo>`, o
  resto do home virou `<home>`. O nome do usuário no `stat` de `fase62-v66.log` virou
  `<user>`. Não sobrou nenhum `jeffersonoliveira` em texto sob `M6/` (PNGs não conferidos).
- **Artefatos 0.2.0 refeitos** (como o usuário, sem `sudo`): `make release`, `make
  extension`, `make extension-crx` (versão 0.2.0, id `nhkehjnmlgkebhmelajfpmpblepaichf`)
  e `make deb EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSION_CRX=…/ward.crx`.
  - Instalador novo `ward-install-0.2.0.tar`: `git archive --format=tar a01b39e1…
    Makefile install docs/design/logo`, reprodutível. São 56 entradas, todas root/root,
    sem link. Os 12 `src:` do `tree.txt` estão nele, e `install/` e o `Makefile` são
    iguais aos do commit.
  - Conferência do `.deb`: `Package: ward`, `Version: 0.2.0`, `Architecture: amd64`,
    `Depends: sshpass, openssh-client, systemd, libwebkit2gtk-4.1-0`; 61 linhas no
    `dpkg-deb -c`, todas root/root, nenhum `s`/`S`, e o metainfo dentro com 0.2.0 e 0.1.5.
  - Os sha256 de `ward`, `wardd`, `ward-nm`, `ward-app` e `ward.crx` no staging são os dos
    artefatos.
  - Sem `test-mode`: `strings` dos 4 binários sem `WARD_DEV_`, `ward-test-bridge`,
    `wardd-test` nem `ward-teste` (0 em cada; o `app-release.sh` passou), e com
    `show_window` (1 em cada). `ward --version` = `ward 0.2.0`.
- **Cópia para o usuário**: `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/`
  foi substituída. `SHA256SUMS` novo, com o tar; `sha256sum -c` deu SUCESSO nos 7. A pasta
  passou a 0700 (Problema 4 da 66).

  | Artefato | sha256 |
  | --- | --- |
  | `ward_0.2.0_amd64.deb` | `1b7adb945c29a83baeee3a86f6267ae49be825eea713cfb9bd90903781161667` |
  | `ward.crx` | `ad931ba0739de4b72635f37a20dc0aefe71be818f6e94e20f807327c17b8106b` |
  | `ward-install-0.2.0.tar` | `a64fbc686db4ec4f0379d053136c628be0b15e4ad7cf3cb617abdabf4a227209` |
  | `bin/ward` | `994f8d29eaabe5e1eae5b6bdabd925eae9f2736621759c735cb6e4bc886bd70b` |
  | `bin/wardd` | `0f85fe58941c7f89e007405f8e2164bc1e3104373ed4c30d4fa1172f27e419e5` |
  | `bin/ward-nm` | `c073f81c88cae15bff34b3491772e9f5b1048543822b2b2c78ebef324cd76e97` |
  | `bin/ward-app` | `ceaff36dc21f1b5bbcf80b21d737eeea3cbd51042deef085232131cd5c8445df` |

  `ward` e `ward-nm` têm o mesmo sha256 da 66a (nada neles mudou). `wardd` mudou (o
  limite), e `ward-app` e o `.crx` também (rebuild; o `.crx` não é reprodutível, Problema
  1 da 66a). A tabela do roteiro foi atualizada.
- Evidência em `docs/mission/evidence/M6/fase-66b/`: `test-app-e2e/` da 2ª rodada (58
  arquivos, varridos contra `.dev/variations.txt`: nada) e os logs `fase66b-*.log`, com
  os caminhos trocados por `<repo>`/`<home>`.

## Segurança

- **Invariantes tocadas**:
  - S1/S9: a matriz de papéis do `show_window` não mudou; o limite só reduz entregas.
  - D57: o `open_app` continua sem campo e o chamador recebe só `ok`.
  - V3.3/V3.4: o leitor v2 não mudou; a decisão está fixada por teste.
  - S14/S15: artefatos sem `test-mode` e sem segredo.
  - Instalação (D18/D60): o roteiro não roda mais código do usuário como root.
- **Testes de ataque**:
  - `crates/wardd/tests/approval.rs › attack_open_app_burst_joins_show_window`: rajada de
    1 500 `open_app` (agente e extensão) dá 1 `show_window`; 999 ms depois, nada; 1 s
    depois, mais 1 (para 50 pedidos); assinante novo recebe na hora e o antigo não;
    sem assinante, nada.
  - `attack_show_window_only_to_the_subscribed_app` foi ajustado (o relógio avança 1 s
    entre os pedidos).
  - `crates/ward-core/src/protocol/tests.rs ›
    decision_missing_data_and_escaped_names_are_the_canonical_request`:
    - `data` ausente e nomes ou chaves com escape (`list`, `li…`,
      `"v"`, `"op"`, `"data"`) recodificam para os mesmos bytes da forma
      canônica;
    - operação do app com escape (`reveal`, `copy`, `set_level`) dá
      `unauthorized` para agente e extensão;
    - outra grafia (`List`, `liſt`, espaço, NUL) dá operação desconhecida ou
      pedido inválido;
    - `data` ausente com campo obrigatório (`run`, `unlock`, `reveal`, `answer_approval`,
      `fill`) dá `invalid_request`;
    - segunda `op` com escape, `data` duplicada com escape e campo extra com escape dão
      `invalid_request`.
  - `crates/ward-nm/src/tests.rs ›
    attack_escaped_names_and_missing_data_reach_the_daemon_canonical`: `open_app` sem
    `data`, com escape no nome ou nas chaves, chega ao daemon só como
    `{"v":2,"op":"open_app","data":{}}`. `reveal`, `list` e `run` com escape dão
    `unauthorized` e nada chega ao daemon.
  - `dev/tests/app_e2e.py`: nova checagem "o botão 'Destravar' volta depois da espera".
  - Roteiro 3b: é documentação, sem teste automático. O conteúdo do tar foi conferido
    contra o `tree.txt` e o commit; o `make install` com `DESTDIR` foi negado (abaixo).
- **Dependências novas**: nenhuma.
- A chave `.dev/extension/key.pem` foi copiada 0600 para o `.dev/` do worktree, apagado
  no fim. Nenhum segredo em docs, handoff ou log.

## Ficou de fora

- Apertar o leitor v2 (exigir `data`, recusar escape): ficou na decisão documentada com
  teste (acima). Se o orchestrator preferir apertar, é preciso mudar `extension/popup.js`
  e `extension/login.js` e as suítes `nm`, `extension`, `login`, `docker-platform` e
  `ward-nm/tests/binary.rs`.
- Tirar a entrega do `show_window` do lock do motor: não fiz. A entrega fica com o
  intervalo de 1 s e o `WRITE_DEADLINE` (justificativa acima).
- `make test-install` com o `make install` 0.2.0 (V9.x) não rodei: a fase pediu
  `test-deb`, que passou.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 2b74fd3251454e69ed269af8611d3a2d380ebe71` | 0 |
| `make dev-services`, `make dev-vault`, `make dev-variations` | 0 |
| `cargo test -p ward-core decision_missing` / `-p ward-nm --lib` / `-p wardd --test approval show_window` | 0 / 0 / 0 |
| `make fmt` | 0 |
| `make check` (1ª) | 2 (english-check: palavras pt-BR num comentário novo do `app_e2e.py`; corrigido) |
| `make check` | 0 ("tudo limpo") |
| `make dev-services-up`, `make dev-services-verify` | 0, 0 |
| `PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null` (1ª) | 0 (152/152) |
| `PW_PYTHONPATH=$PWD/.dev/pw-lib/site make test-app-e2e < /dev/null` (2ª, seguida) | 0 (152/152) |
| `make dev-services-down` | 0 |
| `make test-deb` | 0 ("test-deb: tudo ok") |
| `make compile` | 0 |
| `make english-check` | 0 (0 achados) |
| `df -h /` antes do release | 0 (9,6 GB livres depois de apagar `target/no-test-mode`) |
| `make release`, `make extension`, `make extension-crx` | 0 |
| `make deb EXTENSION_ID=nhkehjnmlgkebhmelajfpmpblepaichf EXTENSION_CRX=…/ward.crx` | 0 |
| `git archive … a01b39e … Makefile install docs/design/logo` | 0 |
| `dpkg-deb -f/-c`, `strings`, `sha256sum` (conferência) | 0 |
| `sha256sum -c SHA256SUMS` em `target/release-0.2.0/` | 0 (7 SUCESSO) |
| `appstreamcli validate --no-net` do metainfo | 0 |
| `make check` final (com handoff, evidências e roteiro no índice) | 0 ("tudo limpo") |

## Problemas descobertos

1. **Comando negado**: `make -C target/install-tar/src install DESTDIR=… BIN_*=…` (o
   instalador extraído do tar, só copiando para uma pasta em `target/`). Não tentei pelo
   `install.sh` direto, para não contornar a regra "nunca `make install`". A conferência
   do tar foi só por conteúdo:
   - os 12 `src:` do `tree.txt` presentes;
   - `diff -r` de `install/` e do `Makefile` contra o commit: iguais;
   - nenhum link.

   O `make deb` usou o mesmo `install.sh` com `DESTDIR` e passou.
2. A entrega do `show_window` (e de todos os avisos) continua sob o lock do motor, limitada
   pelo `WRITE_DEADLINE` de 10 s. Com o intervalo de 1 s, um agente não segura mais o
   motor em rajada. Um app que não lê o socket ainda segura o motor até 10 s por aviso;
   isso já valia para as aprovações antes desta fase.
3. A âncora de confiança dos sha256 do roteiro (3a e 3b) é o próprio roteiro e o handoff,
   que estão no checkout do usuário. Um programa do mesmo uid poderia trocar os dois.
   Quem quiser uma âncora de fora compara com o commit publicado (o tar é reprodutível por
   `git archive`). Só registro; o roteiro já dizia que "o risco não some".
4. As capturas antigas (PNG) de `docs/mission/evidence/` podem mostrar o caminho do
   worktree (Problema da 66c); não as alterei.
5. O instalador tem um `src:<file>` no comentário do `tree.txt`. É só documentação; o
   `install.sh` lê só as linhas da árvore.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/approval.rs` | alterado (`SHOW_WINDOW_INTERVAL`, `last_show`) |
| `crates/wardd/tests/approval.rs` | alterado (teste novo, ajuste do relógio) |
| `crates/ward-core/src/protocol.rs` | alterado (decisão no doc do módulo) |
| `crates/ward-core/src/protocol/tests.rs` | alterado (teste novo) |
| `crates/ward-nm/src/tests.rs` | alterado (teste novo) |
| `dev/tests/app_e2e.py` | alterado (espera o botão "Destravar") |
| `install/desktop/local.ward.app.metainfo.xml` | alterado (0.2.0 e 0.1.5) |
| `docs/instalacao.md` | alterado (2b por `/root/ward-src`, uninstall) |
| `docs/mission/roteiro-usuario.md` | alterado (3b, 9.1, artefatos, sha256) |
| `README.md` | alterado (instalar do código pelo `.deb`) |
| `docs/mission/evidence/M6/fase-61/*.log`, `fase-61a/*.log`, `fase-62/fase62-v66.log`, `fase-62/scripts/cargo-target.sh` | alterados (`<repo>`/`<home>`/`<user>`) |
| `docs/mission/evidence/M6/fase-66b/` | criado (e2e e logs) |
| `docs/mission/handoffs/phase-66b.md` | criado |
| `/home/jeffersonoliveira/projetos/cofre/target/release-0.2.0/` (fora do git) | substituído (com `ward-install-0.2.0.tar`) |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica (sem tela nova)
- [x] Ficou dentro do escopo da fase
