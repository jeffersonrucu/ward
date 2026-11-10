# Fase 54c — Rotação dos backups contra versões plantadas; `ward ssh` morre na trava

## Implementado
- **Rotação só pelo que confere** (`crates/ward-core/src/backup.rs`). A nova
  `para_apagar_conferidas(lista: &[(Versao, Option<u64>)], teto, protegidas)` substitui a
  `para_apagar` (removida). Contam só as versões com data até o teto (Fase 49a) e geração
  selada conhecida. Ficam as 30 **gerações** mais altas, cada uma contada uma vez. Saem as
  versões (original e cópias) das gerações abaixo delas, menos as protegidas. `None`
  (lixo, outro cofre, versão de antes de uma troca de senha) nunca conta e nunca sai.
  A ordem segue a geração, não a data: um relógio que voltou não tira a versão nova.
  - `PastaBackup::guardar(arquivo, instante, cofre)`: a rotação lê cada versão até o
    teto e confere a geração (`geracao_selada`, o conteúdo decifrado zera ao sair).
    `guardar_com_pontos` recebe a função de geração (os testes do V7.3 passam uma
    tabela). Chamadas em `gravacao.rs` (`gravar_entradas_com_backup`,
    `trocar_senha_com_backup` com o cofre novo) e `guardar_atual` ajustadas.
  - `PastaBackup::com_leitor(LerVersao)`: o núcleo não tem `libc`. O daemon
    (`arquivos::PastaAberta::pasta`) passa `ler_versao_backup`, com `O_NOFOLLOW |
    O_NONBLOCK`, arquivo comum do uid, até `MAX_ARQUIVO_COFRE`. O leitor padrão do
    núcleo (só std: `lstat` + `open` + mesmo dev/ino) fica para testes e ferramentas.
- **Pendrive** (`crates/wardd/src/externo.rs`): `rodar` usa a mesma regra, com a
  geração de cada versão de lá (`geracoes_em`, agora `Vec<(Versao, Option<u64>)>`, leitura
  `O_NOFOLLOW | O_NONBLOCK` em `ler_versao`). Nomes plantados com data passada recente
  não apagam cópias reais. Sem cofre aberto nada é copiado nem apagado.
- **Relógio que volta**: o comportamento está documentado em `externo.rs`, `backup.rs`,
  `docs/escopo.md` › Backup e no PRD D17. No pendrive, a versão "fora de ordem" não vai
  (falha fechada). No computador ela fica e não apaga nada naquela gravação.
- **`ward ssh` morre na trava** (decisão do usuário, 2026-11-10):
  - `preparar_ssh` (`ssh.rs`) registra a sessão em `SessoesRun` (`trava:
    Some(MorreNaTrava)`). A trava mata o grupo, e `sessao_com` manda o aviso "ward: o
    cofre travou; o comando foi encerrado". A CLI sai com 137.
  - O `sshpass` cria o `ssh` com `setsid()`, fora do grupo, e matar só o grupo deixava
    esse `ssh` vivo (confirmado no teste de controle). O novo
    `Grupo::matar_com_sessoes_filhas` (`ward-plataforma/src/filho.rs`) faz quatro
    passos, todos com o mutex `colhido` preso:
    1. `SIGSTOP` no grupo. Parado, o líder não colhe os filhos, então o pid deles não é
       reusado.
    2. Procura em `/proc` os filhos diretos do líder que têm sessão própria (pid = grupo
       = sessão).
    3. `SIGKILL` no grupo de cada um.
    4. `SIGKILL` no grupo do líder.
  - `SessoesRun::travar` e `registrar` usam essa função (vale também para o `ward run`).
  - `sys::sinal_grupo`, com `matar_grupo`/`parar_grupo` em volta dela, concentra o
    único `unsafe` (`kill`), com `// SAFETY:`.
- Docs: `docs/escopo.md` › Backup (rotação e relógio) e › S8 (a trava encerra o `ward
  run`/`ward ssh`). Nada mexido em "Janela de aprovação". PRD D12 e D17. Contrato:
  V4.26 ajustado e V7.12 novo (sem marcar).

## Segurança
- Invariantes tocadas:
  - **S8**: o que recebeu segredo não sobrevive à trava. Isso vale para a sessão SSH e
    para o `ssh` com sessão própria que o `sshpass` cria. Nenhum processo de fora leva
    o sinal: só conta quem tem o líder como pai.
  - **S1**: o agente de D12 continua caindo na trava.
  - **S6/S7** (backup): só bytes autenticados pela chave do cofre decidem o que sai, e a
    rotação nunca apaga o que não confere. A leitura não segue link e não fica presa
    num FIFO (`O_NONBLOCK`). O conteúdo decifrado zera ao sair.
  - **S13**: os logs novos têm só texto fixo e contagem.
- Testes de ataque escritos:
  - `crates/ward-core/src/backup/testes.rs` ›
    `f54c_ataque_lixo_com_data_mais_nova_nao_apaga_versoes_reais`: 36 plantados (corpo
    trocado, outro cofre, cópias da real mais antiga e da mais nova). Nenhuma real sai,
    o lixo nunca sai e as cópias saem só com a geração delas. Pela regra antiga, as 5
    reais sairiam na gravação seguinte.
  - `… › f54c_versoes_de_antes_da_troca_de_senha_nao_contam_nem_saem`
  - `… › f54c_relogio_que_volta_nao_apaga_versao_real`
  - `… › f54c_rotacao_so_conta_e_so_apaga_o_que_confere`: a regra sozinha (lixo,
    cópias, relógio).
  - `… › rotacao_conta_so_ate_o_teto`: reescrito para a regra nova.
  - `crates/wardd/tests/externo.rs` ›
    `f54c_ataque_nomes_plantados_no_pendrive_nao_apagam_copias_reais`: 35 nomes de data
    passada recente no pendrive. Nada sai.
  - `crates/wardd/tests/ssh.rs` › `f54c_ataque_ward_ssh_morre_na_trava`: senha (via
    `sshpass` falso com `setsid`) e chave. Termina em 137 em menos de 5 s, com o aviso,
    sem a saída de depois e sem nenhum processo vivo. Controle: com `g.matar()` no
    lugar, falha com "ssh … sobreviveu à trava".
  - `crates/ward-plataforma/src/filho.rs` ›
    `ataque_filho_com_sessao_propria_morre_com_matar_com_sessoes_filhas`: no controle,
    `matar` não alcança o filho. Com a função nova os dois morrem, e um processo de
    fora não leva nada. Também `campos_do_stat_com_parenteses_no_nome`.
  - `dev/testes/cli.py` (V4.26 no `teste-cli`): `ward ssh acme-ssh-chave-prod 'sleep 21'`
    e `acme-ssh-senha-stage 'sleep 22'` contra os SSH de teste. A trava dá 137 em 0,1 s,
    com o aviso, e nenhum `/usr/bin/ssh`/`sshpass` da sessão sobra.
- Testes ajustados:
  - `d12_travar_derruba_o_agente`: agora espera 137.
  - `pendrive_fica_com_30_versoes`: 35 gerações reais em vez de lixo, e o lixo fica.
  - `ataque_lixo_no_backup_local_nao_gira_o_pendrive`: `apagadas` vai a 0, porque o
    enchimento não confere.
  - `v7_3_sigkill_em_100_pontos_aleatorios`: uma tabela de 5000 versões com gerações
    distintas no lugar de 3 repetidas, para a rotação seguir girando pela geração. Saiu
    com 27 mortes no meio da gravação e cerca de 2800 versões usadas.
- Dependências novas: nenhuma.

## Ficou de fora
- O app (`TrocarSenha.svelte`) e a seção "Janela de aprovação" do escopo: são de outros
  workers.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test --workspace --all-features --locked --test externo --test ssh` | 0 |
| idem com `g.matar()` no lugar (controle) | 101 (esperado: "ssh … sobreviveu à trava") |
| `make check` (954 testes) | 0 (1ª rodada 2: clippy `needless_range_loop` no teste, corrigido) |
| `make dev-servicos-subir` / `make dev-servicos-parar` | 0 / 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54c make dev-daemon WARD_DEV_APP=` / `make dev-daemon-parar` | 0 / 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54c make teste-cli` | 0 (0 falhas) |
| `git checkout -- docs/mission/evidence/fase-4[123]` | 0 |

## Problemas descobertos
- **Mudança de comportamento, a confirmar com o usuário**: as versões de antes de uma
  troca de senha mestra (outra chave) agora nunca saem pela rotação, nem no computador
  nem no pendrive. Antes elas iam saindo pela data. Isso combina com o aviso da tela
  "Trocar senha" (apagar à mão se a senha vazou), mas elas ficam na pasta para sempre se
  o usuário não apagar. O mesmo vale para lixo plantado (pode encher o disco, coisa que
  o mesmo uid já consegue de outro jeito).
- **Cópias de uma geração mantida não saem**: quem planta mil cópias da versão atual com
  nomes novos as deixa na pasta até a geração delas cair abaixo das 30. Não ocupam lugar
  das reais.
- **`arquivos::ler_cofre` sem `O_NONBLOCK`** (fora da fase): a lista do restaurar
  (`nucleo.rs:1651`, `1768`) e a leitura das versões locais na cópia externa
  (`externo::copiar`) abrem as versões com `O_NOFOLLOW` sem `O_NONBLOCK`. Um FIFO posto
  no lugar de uma versão entre o `readdir` e o `open` prende a thread (DoS do mesmo uid).
  A rotação nova usa leitura com `O_NONBLOCK`.
- **CLI que cai no `ward ssh` por senha** (fora da fase): fora da trava, `grupo.matar()`
  ainda mata só o grupo do `sshpass`. O `ssh` com sessão própria depende do `SIGHUP` do
  pty do `sshpass` para morrer (o OpenSSH sai com ele). Dá para trocar por
  `matar_com_sessoes_filhas` no fim da sessão também.
- `.dev/known_hosts` foi recopiado de `.dev/servicos/known_hosts` antes do `teste-cli`,
  como na Fase 54. É só estado de teste.
- Disco: 2,8 GB livres ao fim. Nada foi apagado.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/backup.rs` | alterado |
| `crates/ward-core/src/backup/testes.rs` | alterado |
| `crates/ward-core/src/gravacao.rs` | alterado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado (só comentários) |
| `crates/wardd/src/externo.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/tests/externo.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `crates/wardd/tests/ssh_falso.py` | alterado (modo `parado`) |
| `dev/testes/cli.py` | alterado |
| `docs/escopo.md` | alterado (Backup e S8) |
| `docs/mission/prd.md` | alterado (D12, D17) |
| `docs/mission/validation-contract.md` | alterado (V4.26, V7.12) |
| `docs/mission/handoffs/phase-54c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
