# Fase 47 — Restaurar backup (UI, V7.7)

## Implementado
- **Diálogo "Restaurar backup"** (`app/src/telas/RestaurarBackup.svelte`), aberto pelo
  botão "Restaurar" de `Configurações › Backup local`. Segue os frames `16349`
  (`Restaurar backup / Versões`) e `16512` (`/ Confirmar`), conferidos node a node numa
  cópia do `Ward.figcraft` (ids em `docs/mission/design-nodes.md` › "Fase 47").
  - **Versões:** lista de rádio com as versões do backup local, da mais nova para a mais
    antiga: data e hora no fuso local ("08/11/2026 às 07:06"), "Cópia local" e "quando"
    ("há 3 min", "hoje", "ontem", "há 3 dias"). Cinco itens à vista, como no frame; o
    resto rola. A legenda diz a contagem ("30 versões, da mais nova para a mais antiga…").
    "Continuar" só habilita com uma versão escolhida. Estados sem versão ("Ainda não há
    versões…") e pasta indisponível (mensagem do daemon).
  - **Confirmar:** o item escolhido, "Senha mestra desta versão" (`CampoSenha`), **"PIN
    atual"** (`CampoPin`), o `Destaque` aviso ("o cofre atual é guardado como backup…
    valem a senha mestra e o PIN desta versão… a versão volta como uma gravação nova do
    cofre", D17/D39) e a legenda "Se a senha não abrir esta versão, nada muda no cofre
    atual". Voltar à esquerda e "Restaurar esta versão" à direita.
  - **Depois de restaurar:** o diálogo fecha, o toast diz "Versão de … restaurada. O cofre
    tem N entradas." e a tela relê as configurações (as da versão). As entradas da versão
    aparecem em Credenciais.
  - **Erros do daemon mapeados:** `senha-incorreta` → erro no campo da senha;
    `pin-incorreto` → erro no PIN e consulta do estado (o 5.º trava → destravar);
    `travado` e `cofre-mudou` → tela de destravar; `backup-inexistente` → volta à lista
    atualizada com a mensagem; `backup-indisponivel` e o resto → mensagem fixa.
- **Componente `ItemDeVersao.svelte`** (set `Item de versão` 11264, padrão/selecionado):
  `<input type="radio">` real com o desenho por cima, ou só o desenho (`estatico`).
- **`app/src/telas/restaurar.js`:** data local, "quando", legenda, aviso e toast.
- **Núcleo do app (`app/nucleo/src/restaurar.rs`, novo):**
  - `VersaoVista { nome, instante }`: o instante (segundos UTC) sai do próprio nome, com
    aritmética inteira; nome fora do formato faz a resposta inteira ser recusada.
  - `Restauracao { versao, senha: SenhaMestra, pin: Pin }` com `deny_unknown_fields`
    (senha vazia, PIN fora do formato ou número, campo a mais → recusado na leitura, sem
    eco); `Restaurado { restaurado, entradas }`.
  - `versao_pedida`: formato estrito de `ward_core::backup::nome_valido`.
- **`cliente.rs`:** `backups` e `restaurar_backup` (quadro largado logo depois da troca);
  `backup_inexistente`, `backup_indisponivel` e `cofre_mudou` viraram `ErroApp`
  próprios (`erro.rs`).
- **`servico.rs`:** `listar_backups`; `restaurar_backup` confere o formato, **pede a lista
  ao daemon e só manda o `restaurar_backup` se a versão estiver nela**, larga senha e PIN,
  relê a contagem de entradas e descarta o formulário aberto (era do cofre de antes).
- **Comandos Tauri `listar_backups` e `restaurar_backup { dados }`:** em `comandos.rs`
  (`COMANDOS_PRINCIPAL`, `COMANDOS`), `comandos.js`, capability `principal` e
  `generate_handler!`. O `restaurar_backup` atualiza o tempo da bandeja com a
  configuração da versão.
- **Ponte de teste:** os dois comandos (campo a mais/tipo errado recusados) e os modos
  `--daemon-falso sem-backups` e `backups-indisponiveis`.
- **`daemon_falso`:** `backups` e `restaurar_backup` com a ordem do `wardd` (formato →
  PIN atual, que conta para os 5 → versão na pasta → senha da versão em tempo constante
  pelo `ctutils`); o atual vira a versão mais nova; o cofre fica com as entradas, a senha
  e o PIN da versão, sem pedidos de aprovação pendentes. `backups_de_teste` monta 30
  versões a partir de agora (há 2 min, 5 h, ontem, 3 e 6 dias, depois a cada 9 h), cada
  uma com o cofre de teste menos `min(i, 6)` entradas e a senha mestra de teste (D15).

## Segurança
- **Invariantes tocadas**
  - **S1/S11:** nenhuma resposta ao app traz segredo, senha, PIN, caminho ou geração: a
    lista só tem nome e instante; o restaurar, `restaurado` e a contagem. Senha e PIN
    ficam só no estado local do diálogo, saem dele antes da resposta (e ao voltar, fechar
    ou destruir) e o objeto do pedido é zerado; nada em storage, URL ou log (conferido no
    Chrome contra `.dev/variacoes.txt`, com os valores dos inputs incluídos).
  - **S9:** o restaurar exige o PIN atual (o daemon conta e trava no 5.º), além da senha
    da versão.
  - **D5/V7.8:** os dois comandos só na janela `principal` com origem local; as
    operações do daemon já eram só do papel app (Fase 45).
  - **Entrada não confiável:** a versão vinda da interface passa por duas barreiras no
    Rust antes do `restaurar_backup`: formato estrito e presença na lista atual do daemon.
  - **D39:** o aviso diz que a versão volta como gravação nova; a geração é do daemon.
- **Testes de ataque** (todos falham do lado do atacante)
  - `app/nucleo/src/restaurar.rs`:
    - `ataque_versao_forjada_fora_do_formato_e_recusada` (`..`, caminho absoluto,
      `backups/…`, `-01`, `-100`, mês 13, hora 24, maiúscula, `\n`, espaço, dígito largo,
      U+202E, `cofre.ward`);
    - `ataque_restauracao_com_campo_a_mais_ou_pin_ruim_e_recusada` (`geracao`/`caminho` a
      mais, PIN `12ab`/`123`/número, senha vazia, campos faltando; `Debug` sem valor);
    - `pelo_servico::ataque_pin_errado_nao_restaura_e_o_quinto_trava`;
    - `pelo_servico::ataque_senha_da_versao_errada_nao_muda_nada`;
    - `pelo_servico::ataque_versao_inexistente_ou_forjada_nao_chega_ao_daemon` (fora do
      formato: zero quadros; fora da lista: nenhum quadro `restaurar_backup`; mandado
      direto: `backup_inexistente` e nada muda);
    - `pelo_servico::ataque_cofre_travado_no_meio_nao_restaura`;
    - `pelo_servico::v7_8_papel_agente_recebe_nao_autorizado` (o "daemon" decodifica com
      o papel agente: `backups` e `restaurar_backup` dão `NaoAutorizado`);
    - `pelo_servico::v7_7_escolhe_versao_confirma_e_as_entradas_da_versao_aparecem` e
      `pasta_indisponivel_e_sem_versoes`.
  - `app/src-tauri/src/main.rs` ›
    `ataque_restaurar_backup_so_na_janela_certa_e_versao_conferida_no_rust`: janela
    `outra`, janela `aprovacao` e origem remota negadas pela ACL; versão forjada →
    `pedido-invalido` antes do daemon; PIN/senha fora do formato, campo a mais e faltando
    recusados sem eco.
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_restaurar_backup_pela_ponte`:
    nenhuma resposta com segredo, PIN, `/home`, `.local`, `backups/` ou `geracao`; forjados
    recusados; inexistente, PIN errado e senha errada não mudam as entradas; o certo
    troca as entradas pelas da versão.
  - **QA no Chrome** (Playwright Python headless, ponte `--daemon-falso`): PIN errado,
    senha da versão errada, versão forjada por rota do Playwright (`../cofre.ward` →
    "Pedido inválido.", `ward-19990101-000000.ward` → volta à lista com "Essa versão não
    está mais…"), cofre travado pela ponte entre escolher e confirmar (vai ao destravar,
    nada restaurado). Em cada estado: nenhum padrão de `.dev/variacoes.txt` no DOM, nos
    valores dos inputs, em `localStorage`/`sessionStorage`, na URL ou no cookie; nenhum
    padrão nos logs da ponte; nenhuma origem remota.
- **Dependências novas:** nenhuma (Rust ou npm). O Playwright do QA é o do scratchpad.

## Ficou de fora
- **Tamanho ("2,4 MB") e origem ("Pendrive WARD-BACKUP") de cada versão:** o protocolo da
  Fase 45 só traz o nome do arquivo. O item mostra sempre "Cópia local". Não criei
  operação nova (ver Problemas).
- **"Mostrando 5 de 30 versões":** a lista mostra todas (até 64 do protocolo) com rolagem
  e cinco à vista; a legenda diz o total.
- **"Último: hoje às 14:32" na linha "Backup local"** de `Configurações` (frame `15242`):
  não mexi para manter pequena a mudança em `Configuracoes.svelte` (a Fase 49 mexe no
  mesmo arquivo).
- **Ponta a ponta com `wardd` real:** o QA usou o daemon falso. O `wardd` real tem os
  testes da Fase 45 (V7.11 etc.); o restaurar pelo app com o daemon de teste fica para
  o validador do M7 (`make app-ponte` + este diálogo).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make gitleaks-binario` (gitleaks copiado do checkout principal) | 0 |
| `make dev-cofre` e `make dev-variacoes` | 0 |
| `cargo test -p ward-app-nucleo --all-features restaurar` (1ª: `Entrada` sem `Clone` no falso; refeito) | 101, depois 0 |
| `cargo test -p ward-ponte-teste --all-features --test ataques restaurar` | 0 |
| `make app-front` | 0 |
| `qa47.py` modo `cheio` (1ª: seletor ambíguo do botão no script; corrigido) | 1, depois 0 |
| `qa47.py` modos `sem-backups` e `backups-indisponiveis` | 0 |
| `make fmt` e `cargo fmt` em `app/src-tauri` | 0 |
| `make check` (1ª: clippy `manual_contains` no `servico.rs`; corrigido) | 2, depois 0 |
| `make app-check` (R32: `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do scratchpad) | 0 |

## Problemas descobertos
- **Design × daemon (PIN):** o frame `16512` só pede a senha da versão (e a ajuda diz "O
  PIN não serve aqui."), mas o `wardd` da Fase 45 exige também o PIN atual. Acrescentei o
  `Campo PIN` "PIN atual" entre a senha e o aviso, e a ajuda da senha virou "É a senha
  que valia em <dia>." O FigCraft precisa do campo (fase de design).
- **Design × protocolo (tamanho e origem):** `16349` mostra tamanho e "Pendrive
  WARD-BACKUP"; o protocolo só traz nomes do backup local. Ou o FigCraft perde esses
  dados, ou o planner cria campo de tamanho na resposta `backups` (sem caminho).
- **Restaurar só com o cofre destravado** (pendência da 45, não decidida): com o cofre
  corrompido ou a senha atual esquecida não há restaurar pelo app. A tela segue o
  daemon.
- **Aviso de D39 no texto:** acrescentei "A versão volta como uma gravação nova do cofre."
  ao `Destaque` do frame (o pedido da fase pede o aviso da geração nova).
- **Mensagem da versão fora do formato:** aparece "Pedido inválido." (só uma interface
  comprometida chega lá).
- **Pedidos de aprovação pendentes** continuam sem ser cancelados pelo `wardd` ao
  restaurar (pendência da 45); o falso limpa os dele.
- **O `daemon_falso` tira da lista a versão "guardada" quando ela é restaurada**
  (`Entrada` não tem `Clone`); o `wardd` deixa o arquivo. Só afeta o QA.
- **Fuso:** "ontem"/"hoje" e a data usam o fuso do WebKit; o nome do arquivo é UTC.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `app/nucleo/src/restaurar.rs` | criado |
| `app/nucleo/src/lib.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/nucleo/src/erro.rs` | alterado |
| `app/nucleo/src/comandos.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `app/src-tauri/capabilities/principal.json` | alterado |
| `crates/ward-ponte-teste/src/http.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `app/src/lib/componentes/ItemDeVersao.svelte` | criado |
| `app/src/lib/ponte/comandos.js` | alterado |
| `app/src/telas/RestaurarBackup.svelte` | criado |
| `app/src/telas/restaurar.js` | criado |
| `app/src/telas/Configuracoes.svelte` | alterado (botão habilitado + o diálogo) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-47/*` (10 PNG do app, 2 PNG dos frames) | criado |
| `docs/mission/handoffs/phase-47.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (daemon falso; nenhum backup real)
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) Conferido contra o FigCraft (cópia do `Ward.figcraft`; nada salvo)
- [x] Ficou dentro do escopo da fase
