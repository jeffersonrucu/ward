# Fase 49a — Achados da auditoria do M7

## Implementado
- **Médio: datas futuras no nome dos backups** (`ward-core::backup`, `wardd::externo`)
  - Rotação nova em `backup::para_apagar(lista, teto, protegidas)`, usada no backup
    local e no pendrive. Só as versões com data **até o teto** contam para as 30 e podem
    ser apagadas. As de data posterior não contam e nunca são apagadas, então nenhuma
    versão passada sai para dar lugar a uma futura.
  - Teto no local: o fim do segundo da versão recém-gravada (`Versao::fim_do_segundo`,
    sufixo 99). Teto no pendrive: o fim do segundo de agora. O teto é o fim do segundo,
    e não a própria `nova`, porque a rotação pode liberar um nome de sufixo menor no
    mesmo segundo e a `nova` reusa esse nome (ver Problemas).
  - `PastaBackup::listar_ate(agora)`: ignora versões com data depois de agora +
    `MARGEM_FUTURO` (5 min). Ela alimenta a lista do restaurar, a conferência do
    `restaurar_backup` (versão fora da lista dá `backup_inexistente`) e o que vai para o
    pendrive.
  - **Geração selada conferida, marca no item.** O daemon lê a geração de cada versão
    por `backup::geracao_selada(cofre, arquivo)` (`CofreAberto::geracao_selada` →
    `corpo::geracao_do_corpo`): AEAD com a chave do cofre aberto, sem Argon2, com a
    estrutura do corpo conferida inteira. `backup::fora_de_ordem` marca a versão cuja
    geração é menor que a de alguma versão com data anterior (o sufixo desempata o
    mesmo segundo; sem geração não conta).
  - A resposta `backups` mudou de lista de nomes para
    `[{nome, conferida, fora_de_ordem}]` (`VersaoBackup`). **A geração não vai ao app**:
    o teste de invariante `d39_geracao_nunca_vem_do_cliente_nem_vai_para_ele` (nenhuma
    resposta leva a geração) ficou valendo. Por isso escolhi "marca" em vez de "mostrar
    o número" (ver Problemas). A leitura recusa `fora_de_ordem` sem `conferida`, campo
    a mais (inclusive `geracao`), tipo errado e o formato antigo.
  - Item do restaurar: "Cópia local". Fora de ordem: "Cópia local · gravação mais
    antiga do que a data diz", em `aviso/texto`. De outra chave: "Cópia local · de outra
    senha mestra".
- **Baixo: restaurar mantém o backup externo e o tempo de trava atuais.** Antes de
  selar, a versão recebe os minutos de trava e o `BackupExterno` do cofre atual.
  Entradas, senha e PIN continuam os da versão. O aviso da confirmação diz isso.
- **Baixo: pasta de backups insegura vira aviso no app.** `configuracao` ganhou
  `backup_local: bool`: a pasta abre pelo fd e o `listar` confere 0700 e o dono. Em
  Configurações aparece um `Destaque` aviso logo abaixo da linha "Backup local"
  (`#config-backup-local-aviso`). A resposta não traz caminho.
- **Baixo: `.estado.tmp-*` limpos na partida.**
  `arquivos::limpar_temporarios_estado`, chamado em `Daemon::novo`, só apaga
  `.<nome>.tmp-<pid>-<n>` com as seguintes condições:
  - pid e n só com dígitos;
  - pid diferente do próprio;
  - arquivo comum (`DirEntry::file_type` não segue link);
  - dono é o euid.
- **Baixo: pendrive identificado só pelo UUID.** Isso foi documentado em
  `docs/instalacao.md` › "7. Backup externo" e no topo de `externo.rs`: o que um UUID
  clonado permite, o que não permite e o que fazer. Não criei marcador: um arquivo no
  pendrive seria tão copiável quanto o UUID.
- **Baixo: pedidos de aprovação pendentes cancelados ao restaurar.**
  - `Motor::cancelar_pendentes()` nega todos, chamado com o estado do daemon preso,
    logo depois de o restaurar dar certo.
  - Em `responder_aprovacao`, com o estado preso, o pedido é conferido de novo (ainda
    pendente e com as mesmas entradas). Assim um "permitir sempre" que já tinha passado
    pela primeira leitura não muda a entrada de mesmo nome da versão restaurada.
- **Daemon falso e ponte:**
  - geração em cada versão falsa; o atual guardado no restaurar leva a geração dele;
  - modos `--daemon-falso backup-plantado` (cópia de geração 3 com nome de daqui a
    1 min) e `backup-local-inseguro`.

## Segurança
- **Invariantes tocadas**
  - **S7 (integridade/versões):** dados do usuário plantados na pasta (local ou
    pendrive vfat 0777) não fazem a rotação apagar versões reais. Uma versão futura
    plantada não aparece como a mais nova (depois da margem) ou aparece marcada pela
    geração autenticada (dentro da margem).
  - **S9:** restaurar continua exigindo o PIN atual e a senha da versão. Pedido de
    aprovação de antes do restaurar não vale para o cofre novo.
  - **S1/S11/D39:** os campos novos (`conferida`, `fora_de_ordem`, `backup_local`) são
    booleanos, sem segredo, caminho nem geração. Os testes da ponte, do serviço e do
    daemon conferem que "geracao" não aparece. A interface não guarda nada: no QA,
    nenhum padrão de `.dev/variacoes.txt` aparece no DOM, nos inputs, em storage nem na
    URL.
  - **S2/S6:** nenhum byte em claro sai do `geracao_selada`. O conteúdo decifrado é
    `Zeroizing` e só o número volta.
  - **S13:** os logs novos são textos fixos com contagem ("N temporário(s)…", "N
    pedido(s)… cancelado(s)"), sem caminho nem nome.
- **Testes de ataque** (todos falham do lado do atacante)
  - `crates/ward-core/src/backup/testes.rs`
    - `ataque_versoes_com_data_futura_nao_apagam_as_reais`: 30 cópias autênticas em
      2099 e 30 dentro da margem; nenhuma real sai, as de 2099 ficam fora da lista e a
      geração denuncia as de dentro. Depois de mais 40 gravações ficam as 30 reais mais
      novas.
    - `rotacao_conta_so_ate_o_teto`
    - `ataque_copia_antiga_com_nome_novo_sai_marcada` (`fora_de_ordem`)
    - `ataque_geracao_selada_nao_se_forja`: outra chave, byte trocado (cabeçalho e
      corpo) e lixo dão `None`.
  - `crates/ward-core/src/protocolo/testes.rs` ›
    `ataque_restaurar_com_caminho_no_nome_recusado`: recusa `geracao` a mais,
    `fora_de_ordem` sem `conferida`, marca em texto, `caminho` a mais, o formato antigo
    só com nomes e `configuracao` sem `backup_local`. `d39_…` segue verde com a resposta
    nova no `uma_resposta_de_cada`.
  - `crates/wardd/tests/backup.rs`
    - `ataque_versoes_com_data_futura_nao_apagam_as_reais_nem_restauram`: pelo daemon,
      com restaurar de versão de 2036 dando `backup_inexistente` e cofre e estado iguais
      byte a byte.
    - `restaurar_mantem_tempo_de_trava_e_backup_externo_atuais`: na memória e no disco.
    - `pasta_de_backups_insegura_aparece_na_configuracao`: com 0755, 0750, 0707 e com a
      pasta trocada por link; a resposta não tem caminho.
    - `temporarios_do_estado_somem_na_partida`: link, pasta, nomes parecidos e o
      estado de outro arquivo ficam.
    - `ataque_versao_trocada_por_link`: agora dá `backup_inexistente`.
  - `crates/wardd/tests/aprovacao.rs` › `ataque_49a_restaurar_cancela_pedido_pendente`:
    o uso recebe `negado`; o "sempre" com o PIN certo dá `expirado`; o nível fica "com
    aprovação" na memória e depois de travar e destravar.
  - `crates/wardd/tests/externo.rs` ›
    `ataque_versoes_com_data_futura_no_pendrive_nao_apagam_as_reais`: 30 arquivos de
    2099 no pendrive e uma versão local 10 anos à frente. Nada apagado, e a futura local
    não vai para o pendrive.
  - `app/nucleo/src/restaurar.rs`
    - `pelo_servico::ataque_copia_plantada_chega_marcada_pela_geracao`: só a plantada
      sai marcada, antes e depois de restaurar, e o JSON não tem "geracao".
    - `ataque_versao_forjada_fora_do_formato_e_recusada`: recusa "fora de ordem" sem
      "conferida".
  - `crates/ward-ponte-teste/tests/ataques.rs` › `ataque_restaurar_backup_pela_ponte`:
    as chaves do item são exatamente `conferida`, `fora_de_ordem`, `instante` e `nome`;
    a senha de teste e "geracao" entraram nas agulhas.
  - **QA no Chrome** (Playwright Python headless, ponte com daemon falso, 1280×800):
    - `backup-plantado`: o item do topo aparece "Cópia local · gravação mais antiga do
      que a data diz" em cor de aviso, e os outros, "Cópia local"; não há "geracao" no
      DOM; o confirmar traz o aviso "O tempo de trava e o backup externo continuam os de
      agora";
    - `backup-local-inseguro`: o aviso aparece; no `cheio`, não aparece;
    - nenhum padrão de `.dev/variacoes.txt` aparece.
    - Screenshots em `docs/mission/evidence/fase-49a/`.
- **Dependências novas:** nenhuma (Rust ou npm).

## Ficou de fora
- **FigCraft:** a fase não é de design. O item marcado e o `Destaque` aviso
  da linha "Backup local" foram montados com componentes existentes (`Item de versão`,
  `Destaque` aviso, `aviso/texto`), sem frame novo. Ver Problemas.
- **`make app-check`** (build de release do Tauri): não rodou. Esta máquina não tem `libwebkit2gtk-4.1-dev` nem o sysroot de R32. O `app/src-tauri` só lê `minutos_trava` de `Configuracoes` e repassa `VersaoVista` por serde (campos novos), então não muda de assinatura. Fica para o validador.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --lib backup::` | 0 |
| `cargo test -p wardd --all-features --test backup --test aprovacao --test configuracoes --test datas --test externo` (1ª: `ataque_versao_trocada_por_link` esperava `backup_indisponivel`; 2ª: teto da rotação na `nova` em vez do fim do segundo, corrigido) | 101, 101, depois 0 |
| `cargo test -p ward-app-nucleo --all-features` (1ª: agulha `geracao` do teste antigo) | 101, depois 0 |
| `cargo test -p ward-ponte-teste --all-features --test ataques` (1ª: chaves do item) | 101, depois 0 |
| `make check` (1ª: `d39_geracao_nunca_vem_do_cliente_nem_vai_para_ele` com a geração na resposta; trocada por marcas) | 2 |
| testes de `ward-core` (protocolo e backup), `ward-app-nucleo`, `ward-ponte-teste` e `wardd` (backup, aprovacao, externo, datas, configuracoes) depois da troca (1ª: asserção do teste do serviço e o texto exato da `configuracao`) | 101, depois 0 |
| `make app-front` e `qa49a.py` de novo | 0 |
| `make app-front` | 0 |
| `qa49a.py` (scratchpad; Chrome headless) | 0 |
| `make fmt` | 0 |
| `make check` (2ª, depois da troca) | 0 |

## Problemas descobertos
- **A ordem pelo nome não é a ordem das gravações, mesmo sem atacante.** A rotação
  libera nomes antigos. Com várias gravações no mesmo segundo (até 99), a nova pode
  reusar `-0` depois de `-25` já existir, e a lista mostra a mais nova abaixo. Isso já
  acontecia antes. Agora a marca de fora de ordem acusa isso no item e o teto da rotação é o
  segundo inteiro, mas o nome continua enganando. Pede só uma nota; na prática, só com
  gravações em rajada.
- **Relógio para trás mais do que a margem (5 min).** As versões gravadas antes do
  ajuste ficam fora da lista do restaurar e fora da rotação até o relógio alcançar a
  data delas. Não são apagadas. É o preço de não confiar na data do nome; registro
  para o planner.
- **"Mostrar geração" × D39.** O achado pedia "mostrar geração ou data selada no
  item". O corpo não tem data selada, e o teste `d39_…` proíbe geração em qualquer
  resposta. Fiquei com o invariante: o daemon compara e manda só a marca. Se o planner
  quiser o número no item, tem de mudar o invariante (D39) primeiro. Não decidi isso.
- **A geração exige a chave do cofre aberto.** Versões de antes de uma troca de senha
  aparecem como "de outra senha mestra" e não entram na conferência de ordem. Uma cópia
  antiga plantada de antes da troca não sai marcada, mas também não abre sem a senha
  antiga.
- **`backups` decifra até 64 versões com o estado preso.** É AEAD sem Argon2, mas um
  cofre grande (até 512 MiB) deixaria o daemon ocupado. Com o tamanho real (KB), não
  pesa.
- **Design:** o FigCraft não tem os detalhes novos do item, o estado com aviso do
  `Item de versão`, o `Destaque` aviso da linha "Backup local", nem a frase nova do
  aviso da confirmação. Uma fase de design deveria acrescentar esses estados.
- **Pendrive por UUID:** fica a documentação. Para fechar de verdade, falta montar o
  pendrive pelo udisks2 com marca própria ou cifrar com chave do cofre (fora do escopo
  atual).
- **`restaurar_backup` de versão com link:** a conferência pela lista vem antes da
  leitura com `O_NOFOLLOW`. Por isso o erro mudou de `backup_indisponivel` para
  `backup_inexistente`. A leitura sem seguir link continua contra a troca no meio.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/backup.rs` | alterado |
| `crates/ward-core/src/backup/testes.rs` | alterado |
| `crates/ward-core/src/corpo.rs` | alterado |
| `crates/ward-core/src/formato.rs` | alterado |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/wardd/src/aprovacao.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/src/externo.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/uso.rs` | alterado |
| `crates/wardd/tests/aprovacao.rs` | alterado |
| `crates/wardd/tests/backup.rs` | alterado |
| `crates/wardd/tests/configuracoes.rs` | alterado |
| `crates/wardd/tests/datas.rs` | alterado |
| `crates/wardd/tests/externo.rs` | alterado |
| `crates/ward-ponte-teste/src/main.rs` | alterado |
| `crates/ward-ponte-teste/tests/ataques.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/configuracoes.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/restaurar.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src/lib/componentes/ItemDeVersao.svelte` | alterado |
| `app/src/telas/Configuracoes.svelte` | alterado |
| `app/src/telas/RestaurarBackup.svelte` | alterado |
| `app/src/telas/configuracoes.js` | alterado |
| `app/src/telas/restaurar.js` | alterado |
| `docs/instalacao.md` | alterado (seção 7) |
| `docs/mission/evidence/fase-49a/*.png` (4) | criado |
| `docs/mission/handoffs/phase-49a.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (daemon falso e diretórios de teste em `target/`; nenhum daemon no socket fixo)
- [x] Teste de ataque para cada (SEG) tocado (V7.1, V7.3, V7.5, V7.7, V7.11 e os seis achados)
- [x] (UI) Conferido no Chrome headless; sem frame novo no FigCraft (ver Problemas)
- [x] Ficou dentro do escopo da fase
