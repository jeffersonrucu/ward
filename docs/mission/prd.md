# PRD — Ward

Missão 2: código em inglês (planner, 2026-11-10). A missão 1 (o produto, releases
0.1.0–0.1.4) está concluída e arquivada em `docs/mission/arquivo/m1/` (PRD, plano,
contrato, roteiro, handoffs e evidências). Fonte: `docs/escopo.md`, em especial a seção
"Missão 2 — Código em inglês", com as respostas do usuário de 2026-11-10.

## Objetivo

Traduzir para o inglês todo o código do Ward (identificadores, nomes de arquivo e de
pasta, comentários, nomes de crate e de feature, subcomandos da CLI, alvos e variáveis
do Makefile, nomes gravados no cofre e campos do protocolo) **sem mudar o
comportamento nem enfraquecer a segurança**, e publicar daemon, CLI, app, extensão,
`.deb` e `.crx` juntos na **0.2.0**. Os textos que o usuário lê continuam em pt-BR, e
nenhuma tela muda. O cofre da 0.1.x é migrado pelo daemon na primeira abertura, sem
perda e sem segredo em claro no disco.

Quando acabar, um checador automático no `make check` garante que não sobrou
identificador nem comentário em pt-BR, e a bateria inteira de testes da missão 1 passa
com os nomes novos.

## Regras (com referência ao escopo)

1. **Escopo da tradução** ("Missão 2 — Código em inglês"): ficam em inglês os
   identificadores (funções, métodos, tipos, variáveis, constantes, módulos, crates,
   arquivos e pastas) e todos os comentários, em qualquer linguagem do repositório
   (Rust, JS/TS, Svelte, CSS, shell, Python, Makefile, systemd, TOML, YAML, Dockerfile,
   C). Ficam em pt-BR os textos exibidos ao usuário (CLI, app, extensão, notificações,
   descrições do `.desktop`/metainfo), `docs/`, o `README.md`, a skill (`skill/**/*.md`)
   e os arquivos de `.claude/`. Esses acompanham os nomes novos.
2. **Interface intacta** ("App desktop": interface toda em português; "Missão 2" › sem
   Milestone 0): nenhum texto, layout, token visual ou comportamento de tela muda. O
   `Ward.figcraft` e `docs/mission/design-nodes.md` ficam como estão (só os caminhos de
   arquivo citados no `design-nodes.md` acompanham os nomes novos, na Fase 59).
3. **Troca 1:1** ("Missão 2" › testes só no fim): cada fase renomeia um nome e ajusta
   todos os usos dele na mesma fase, em todo o repositório. Usa o glossário
   (`docs/glossario.md`, Fase 1) para que o mesmo termo vire sempre a mesma palavra.
   Nome novo fora do glossário entra no glossário na mesma fase.
4. **Fio congelado até o Milestone 4** (DT3): nos Milestones 1–3 nenhum byte que
   cruze uma fronteira muda. Ficam como estão os nomes gravados no cofre, os nomes de
   operação, campo, erro e papel do protocolo, as mensagens de native messaging, os
   comandos e eventos do Tauri, os campos `serde` que vão à webview, as variáveis de
   ambiente, os subcomandos e a saída da CLI, e todo texto exibido. Um campo `serde`
   renomeado no Rust leva `#[serde(rename = "<nome antigo>")]` até a Fase 54. Um nome
   que aparece em log ou em mensagem por `Debug`/`Display` (ex.: `UidRecusado`, que o
   `make test-docker-uid` procura no log) tem os usos em scripts e testes ajustados na
   mesma fase. Texto exibido ao usuário nunca sai de `Debug` de identificador: se sair,
   a fase o troca por literal pt-BR igual ao de antes.
5. **Literais**: string literal não é traduzida, salvo quando é nome (caminho, alvo,
   variável de ambiente, nome de fio na fase que o muda). Cada fase lista no handoff
   todo literal alterado (`git diff -U0` filtrado por aspas) com o motivo.
6. **Testes só no fim** ("Missão 2" › testes só no fim; pedido do usuário de
   2026-11-10): as fases dos Milestones 1–5 não rodam suíte nenhuma. Cada fase termina
   com **uma** compilação rápida, `make compile` (Fase 2: `cargo check` do workspace com
   todos os alvos e features, `cargo check` do `app/src-tauri` se houver
   `libwebkit2gtk-4.1-dev`, build de produção do Vite, `node --check` da extensão,
   `bash -n` dos scripts e `py_compile` do Python), e com o checador limpo nos caminhos
   que ela traduziu (`make english-check PATHS=…`). Os testes (inclusive os novos) são
   escritos nas fases, mas só rodam no Milestone 6, que tem a bateria inteira e as fases
   de correção.
7. **Segurança primeiro** ("Objetivo"; "Segurança"): nenhuma invariante S1–S15 pode
   regredir. Rename que toque em código de segurança (conferência de papel, máscara,
   cripto, gravação atômica, endurecimento, aprovação, PIN, domínio) não muda lógica:
   só nomes e comentários. Na dúvida entre rename agressivo e segurança, deixe o nome
   e registre no handoff.
8. **Cripto e dependências** ("Segurança" › S6, S14): nenhuma dependência nova em Rust
   nem em npm. O checador é Python da biblioteca padrão. A migração do formato usa as
   mesmas funções de cripto e de gravação que já existem.
9. **Nunca tocar** (`CLAUDE.md` › Regras): cofre real (`~/.local/share/ward/`),
   KeePassXC, `~/.ssh`, `~/.aws`, perfil real do Chrome, `~/.claude`. A migração do
   cofre real e a instalação da 0.2.0 são do usuário (Fase 66 só prepara o roteiro).
10. **Chave da extensão** ("Arquitetura" › extensão por política): o id da extensão
    não muda. A chave privada `.dev/extensao/chave.pem` muda de lugar só por `mv`
    (nunca copiada nem gerada de novo), e o id calculado dela continua igual ao do
    `ward-nm` (Fase 5).
11. **Cofre real no disco** (resposta 4 do usuário): os nomes não mudam
    (`~/.local/share/ward/cofre.ward`, `estado`, versões `ward-*.ward`, pasta
    `ward-backup` do pendrive), e a magia do formato (`WARDCOFR`) também não (DT6). Só
    o conteúdo migra.
12. **Sem alias** (respostas 3 e do enunciado): subcomandos, alvos e variáveis antigos
    deixam de existir. As regras de deny da skill negam também as formas antigas
    (DT10).
13. **Paralelismo** (`MEMORY.md`: workers em worktrees, commit local por fase, sem
    push): o plano marca as trilhas que podem rodar juntas. As trocas estruturais
    (Milestone 1) e as de fio (Milestone 4) são sequenciais.

## Modelo de ameaça e invariantes (S1–S15, copiadas do escopo, com o milestone que cobre cada uma)

O modelo de ameaça é o de "Segurança" › "Modelo de ameaça" e "Fora do modelo de
ameaça", sem mudança. Esta missão acrescenta três riscos próprios:

- **RM1 — rename que muda comportamento**: um `sed` que pega um literal de fio, um
  `match` que perde um braço, uma conferência que passa a comparar com o nome errado.
  Defesa: fio congelado até o M4 (regra 4), literais listados no handoff (regra 5),
  testes de fio contra a linha de base (Fase 3) e a bateria inteira no M6.
- **RM2 — migração do cofre**: perder dados, gravar segredo em claro, deixar o cofre
  meio gravado, aceitar um nome desconhecido, abrir caminho para retrocesso. Defesa:
  DT4–DT7 e as asserções V4.*.
- **RM3 — versões misturadas na atualização**: CLI 0.1.x com daemon 0.2.0 (ou o
  contrário), extensão velha com `ward-nm` novo, artefato antigo de `modo-teste`
  aceito pelo instalador, caminho antigo da política do Chrome que fica. Defesa:
  DT8, DT9, DT12 e as asserções V5.* e V6.*.

Todas as invariantes são conferidas de novo, com os nomes novos, no Milestone 6. A
coluna "Milestone" diz onde a missão mexe no código que as sustenta.

| | Invariante (texto do escopo, "Segurança" › "Invariantes") | Milestone |
|---|---|---|
| S1 | Nenhuma operação do socket devolve segredo à CLI. Revelar e copiar só pelo app; preencher só pela extensão no domínio certo. | M4 (protocolo v2), M6 |
| S2 | Segredo nunca vai para disco em claro. Anexo temporário só em `/run/user/<uid>/ward/`, 0600, apagado ao fim (inclusive se o comando falhar ou for morto). | M4 (migração), M6 |
| S3 | Segredo nunca vai em argv (visível em `/proc/<pid>/cmdline`); só no ambiente do processo filho, em arquivo temporário ou em fd. | M4 (variáveis), M5, M6 |
| S4 | A máscara cobre os valores secretos e as variações declaradas quando o valor inteiro está num mesmo pedaço da saída (D34 revisto, 2026-11-09: nada é segurado nem atrasado, para a máscara não virar oráculo de prefixo). O daemon lê de uma vez tudo o que já está no pipe ou no pty, e o pipe de saída tem 256 KiB: um `write` único de até esse tamanho é mascarado inteiro. Quando o valor chega cortado entre leituras separadas no tempo (escrita lenta, byte a byte, ou um `write` maior que o pipe), o pedaço que o completa sai marcado e o começo que já saiu fica em claro; risco aceito pelo usuário. | M2 (máscara), M6 |
| S5 | Socket 0600 do usuário em pasta do root (`/run/ward/<uid>/`, criada pelo systemd do sistema; o daemon de teste usa um diretório 0700 do usuário), conferência do uid do cliente por `SO_PEERCRED`. A CLI e o app conferem que o servidor é o `wardd` instalado (pasta do root e par com uid 0 no `SO_PEERCRED`) antes de enviar a senha mestra. | M2 (plataforma), M4 (versão), M6 |
| S6 | Criptografia só com bibliotecas auditadas: Argon2id (parâmetros registrados no cabeçalho, mínimo ~256 MiB / 3 passes), XChaCha20-Poly1305 com o cabeçalho como dado autenticado, chave de dados aleatória embrulhada pela chave derivada. Um byte alterado no arquivo faz a abertura falhar. Nada de cripto feita à mão. | M2 (formato), M4 (corpo v5), M6 |
| S7 | Gravação atômica (temp + fsync + rename). Uma gravação interrompida nunca corrompe a versão anterior. | M2, M4 (migração), M6 |
| S8 | Daemon com `PR_SET_DUMPABLE=0`, sem core dump, segredos zerados da memória ao travar e após o uso (`zeroize`). Daemon recusa iniciar com `LD_PRELOAD` e similares. Travar também encerra o que já recebeu segredo: o `ward run` em curso (Fase 54) e a sessão `ward ssh` aberta (Fase 54c; decisão do usuário, 2026-11-10), com `SIGKILL` no grupo do comando e no do `ssh` que o `sshpass` cria com sessão própria; a CLI sai 137 com "o cofre travou; o comando foi encerrado". Descendente que sai do grupo também morre (Fase 55, D69): o daemon é `PR_SET_CHILD_SUBREAPER` (sem ele não sobe), e o fim de uma sessão (fim do comando, CLI que cai, trava) para com `SIGSTOP` e depois mata com `SIGKILL`, por `pidfd` conferido pelo horário de início, a árvore inteira do líder, os órfãos da sessão dele e os **órfãos sem dono** (filho do daemon fora do registro, numa sessão que não é de líder vivo nem a do daemon: quem fez `setsid`), com os descendentes deles. A trava varre os órfãos mesmo sem sessão em curso. A CLI só diz "encerrado" se nada sobrou. Efeito aceito: o órfão sem dono não diz de qual sessão veio, então morre no fim de qualquer sessão; o `chrome_crashpad_handler` do Chrome do `ward playwright` (`fork` duplo com `setsid`) morre quando qualquer `ward run` termina (o Chrome segue, só perde o relatório de crash), e um descendente do app com `setsid` que fique órfão (o app fechou) também. | M1 (feature `test-mode`), M2, M5 (marca), M6 |
| S9 | Mudar nível de acesso exige PIN. 5 erros de PIN travam o cofre. | M2, M4 (nomes de nível), M6 |
| S10 | Aprovação expira em 2 min e fechar nega. | M2, M6 |
| S11 | Tauri com capabilities mínimas (só os comandos que o app usa), CSP estrita, nenhum conteúdo remoto, nenhum segredo na webview fora do "revelar". | M3, M4 (comandos Tauri), M6 |
| S12 | Área de transferência limpa em 30 s. | M2, M6 |
| S13 | Logs de depuração nunca contêm segredo. | M2, M4 (migração), M6 |
| S14 | Dependências mínimas; `cargo audit` e `cargo deny` limpos; lockfiles no git; frontend sem dependência de runtime além do necessário. | M1 (checador sem dependência), M6 |
| S15 | O repositório nunca contém segredo real (varredura por gitleaks ou equivalente). | M1 (fixtures legadas, chave da extensão), M6 |

## Fora do escopo

- Mudar qualquer comportamento, tela, texto exibido ou regra de segurança; feature
  nova; dependência nova.
- Traduzir `docs/`, o README, a skill, os arquivos de `.claude/` ou o histórico do git
  (commits antigos continuam em pt-BR). O arquivo da missão 1
  (`docs/mission/arquivo/m1/`) não é editado.
- Renomear o cofre real, a pasta `~/.local/share/ward/`, a pasta `ward-backup`, os
  nomes das versões do backup, a magia `WARDCOFR`, os grupos `ward-app`/`ward-nm`, o
  socket `/run/ward/<uid>/wardd.sock`, as units `wardd@`, os binários (`ward`, `wardd`,
  `ward-app`, `ward-nm`), o host `br.ward.nm`, o id do app (`local.ward.app`) e o id e a
  chave da extensão.
- Alias para os nomes antigos (subcomandos, alvos, variáveis, operações).
- Voltar da 0.2.0 para a 0.1.x com o cofre migrado (a 0.1.x recusa o corpo v5; o
  caminho de volta é restaurar à mão uma versão v4 do backup, descrito em
  `docs/instalacao.md`, DT7).
- Publicar a release no GitHub e instalar no computador do usuário: são do usuário
  (Fase 66 deixa os artefatos e o roteiro).
- Design no FigCraft (sem Milestone 0, resposta 1 do usuário).

## Fluxos

Os fluxos do produto são os da missão 1 (`docs/mission/arquivo/m1/prd.md` › Fluxos),
com os nomes novos. O que muda para cada um:

1. **Usuário no app**: nada visível. Na primeira vez que destrava a 0.2.0, o daemon
   migra o cofre (corpo v4 → v5) e grava uma geração nova; a tela é a mesma. Restaurar
   uma versão antiga do backup (v4) funciona e grava no formato novo.
2. **Usuário no terminal**: `ward list`, `ward unlock` (pelo caminho completo
   `/usr/local/bin/ward unlock`, como os guias mandam), `ward lock`, `ward run`,
   `ward ssh`, `ward login`, `ward playwright`, `ward --help`. `ward lista`,
   `ward destravar` e `ward travar` dão "comando desconhecido" (saída 2, mensagem em
   pt-BR). As mensagens e a tabela continuam em pt-BR, com os rótulos de tipo e nível
   em pt-BR ("banco", "com aprovação").
3. **Claude via skill**: `ward list --json` (chaves e valores em inglês: `type`
   `database`, `level` `approval`…) → escolhe a entrada → `ward run`/`ward ssh`/
   `ward login`/`ward playwright` → janela de aprovação igual à de antes. O filho do
   `ward run` recebe `WARD_ENTRY`, `WARD_GROUP`, `WARD_ENVIRONMENT`, `WARD_PROTOCOL` e
   `WARD_FILE_<NAME>`/`WARD_FILE_KEY`, nunca as formas antigas. A skill e o deny
   acompanham.
4. **Extensão**: igual para o usuário. Ela fala o protocolo v2 com o `ward-nm`; uma
   extensão 0.1.x que sobrar até o Chrome atualizar recebe "versão desconhecida" e
   nenhum dado.
5. **Atualização 0.1.4 → 0.2.0** (novo): `sudo make install` ou `dpkg -i` do `.deb`
   novo. A instalação para o `wardd@<uid>` antigo (o cofre trava), troca os binários e
   apaga `/usr/local/share/ward/extensao/` e `/usr/local/lib/ward/grupos-criados`
   (levando a marca para `created-groups`). Depois reescreve a política do Chrome com o
   caminho novo e religa o socket. O cofre real não é tocado até o usuário destravar.

## Decisões provisórias

- **DT1 — Arquivo da missão 1**: `prd.md`, `plan.md`, `validation-contract.md`,
  `roteiro-usuario.md`, `handoffs/` e `evidence/` da missão 1 foram para
  `docs/mission/arquivo/m1/` (`git mv`, feito pelo planner). `design-nodes.md` fica em
  `docs/mission/`. Os scripts que gravavam evidência em
  `docs/mission/evidence/fase-NN/` passam a gravar em
  `docs/mission/evidence/<suíte>/` (ex.: `evidence/test-extension/`), Fase 5.
- **DT2 — Referências a IDs nos comentários**: `Dnn`, `RAnn`, `Vn.n`, `Mn` e "Fase nn"
  sem prefixo, nos comentários, são da missão 1 (arquivo `m1`). Na tradução, "Fase"
  vira "Phase" e o ID fica. Referência a esta missão leva o prefixo `m2:`
  (`m2:V4.3`, `m2:DT5`). O `CLAUDE.md` registra a convenção (Fase 6).
- **DT3 — Fio congelado** (regra 4) até o Milestone 4, que muda o fio de uma vez:
  corpo v5, protocolo v2, variáveis entregues, comandos do Tauri.
- **DT4 — Corpo v5**: o cabeçalho do arquivo não muda (magia `WARDCOFR`, versão do
  formato 1, parâmetros do Argon2id, AAD). Muda só o corpo cifrado:
  `VERSAO_CORPO` 4 → 5, com os nomes em inglês. A leitura aceita v4 (nomes pt-BR, por
  uma tabela fechada de conversão) e v5; a gravação só gera v5. Nome desconhecido em
  qualquer versão recusa a abertura sem gravar nada. Os nomes do v5:

  | Grupo | v4 (0.1.x) | v5 (0.2.0) |
  |---|---|---|
  | tipo | `login`, `banco`, `servidor`, `api`, `aws`, `smtp`, `ftp-sftp`, `nota-segura`, `generico` | `login`, `database`, `server`, `api`, `aws`, `smtp`, `ftp-sftp`, `secure-note`, `generic` |
  | nível | `bloqueado`, `com-aprovacao`, `livre` | `blocked`, `approval`, `free` |
  | segurança SMTP | `nenhuma`, `starttls`, `tls` | `none`, `starttls`, `tls` |
  | motor, ambiente, protocolo ftp | já em inglês | iguais |

  O separador continua o hífen dos nomes estáveis de hoje (`ftp-sftp`). Por isso
  `secure-note`, e não `secure_note` como no exemplo da resposta 2 do usuário. A Fase
  1 confere no código se há outro nome gravado (configuração, backup externo, extras)
  e o põe nesta tabela.
- **DT5 — Migração na primeira abertura**: ao destravar um cofre v4, o daemon monta o
  corpo v5 em memória e grava pelo caminho de gravação de sempre (temporário com
  `O_EXCL`, fsync, rename, fsync da pasta; S7), com geração + 1, estado atualizado e
  versão nova no backup local. As versões v4 do backup ficam, e a rotação lê v4 e v5
  (ambas seladas pela mesma chave). O destrave só responde "ok" depois de gravar. Se a
  gravação falhar, o cofre continua v4 e intacto, o destrave responde erro e nada é
  apagado. A migração acontece uma vez: o cofre v5 aberto de novo não é regravado. Com
  o cofre migrado, o PIN, os níveis, as datas, os anexos, os extras, as sementes TOTP e
  a configuração (trava, backup externo, hash do PIN) são os mesmos, byte a byte nos
  valores.
- **DT6 — Arquivo de estado**: o nome no disco (`estado`) fica. O conteúdo passa de
  `ward-estado 1` para `ward-state 2`, com as chaves em inglês (`vista`→`seen`,
  `queimada`→`burned`, `falhas_agente`→`agent_failures`; glossário, seção 11). A leitura aceita os
  dois, a gravação só gera o 2, e as gerações e as senhas erradas do agente são lidas
  das duas formas com o mesmo significado (a conferência de retrocesso de D39/D53 não
  muda). A magia `WARDCOFR` fica: é a assinatura do arquivo do cofre real e dos
  backups.
- **DT7 — Volta para a 0.1.x**: não é suportada com o cofre migrado. A 0.1.x recusa o
  corpo v5 ("versão do corpo desconhecida") sem gravar nada. `docs/instalacao.md`
  ensina a voltar restaurando à mão uma versão v4 do backup. A Fase 64 confere com o
  `wardd` 0.1.4 da linha de base.
- **DT8 — Protocolo v2**: `VERSAO` 1 → 2. Operações, respostas, campos, códigos de erro
  e papéis em inglês, `snake_case`. O daemon v2 responde a um quadro v1 com o erro
  `unknown_version` (mensagem pt-BR "versão do protocolo incompatível: atualize o
  Ward") e fecha, sem dado nenhum. Um quadro v2 com campo desconhecido (inclusive um
  nome pt-BR antigo, como `entrada_padrao` no lugar de `stdin`) é recusado
  (`invalid_request`), nunca ignorado nem trocado por um padrão. A CLI e o app conferem
  a versão na primeira resposta (`status`) e param, sem mandar a senha mestra, se o
  servidor não for v2. Nomes (a Fase 1 completa os campos e os erros no glossário):

  | v1 | v2 | v1 | v2 |
  |---|---|---|---|
  | `estado` | `status` | `detalhe` | `detail` |
  | `destravar` | `unlock` | `revelar` | `reveal` |
  | `travar` | `lock` | `copiar` | `copy` |
  | `lista` | `list` | `totp` | `totp` |
  | `executar` | `run` | `salvar_entrada` | `save_entry` |
  | `ssh`, `login`, `playwright` | iguais | `remover_entrada` | `remove_entry` |
  | `entrada_padrao` | `stdin` | `mudar_nivel` | `set_level` |
  | `janela` | `resize` | `responder_aprovacao` | `answer_approval` |
  | `abrir_app` | `open_app` | `emitir_token` | `issue_token` |
  | `criar_cofre` | `create_vault` | `trocar_pin` | `change_pin` |
  | `trocar_senha` | `change_password` | `tempo_trava` | `lock_timeout` |
  | `backups` | `backups` | `restaurar_backup` | `restore_backup` |
  | `configuracao` | `config` | `backup_externo` | `external_backup` |
  | `definir_backup_externo` | `set_external_backup` | `pendrives` | `usb_drives` |
  | `sugerir` | `suggest` | `preencher` | `fill` |
  | `salvar_login` | `save_login` | `aguardar_login` | `await_login` |
  | `preencher_login` | `fill_login` | `resultado_login` | `login_result` |
  | resposta `ok` | `ok` | `erro` | `error` |
  | `saida` | `output` | `fim` | `exit` |
  | `sessao_playwright` | `playwright_session` | `segredo` | `secret` |
  | `pedido_de_aprovacao` | `approval_request` | `pedido_de_destrave` | `unlock_request` |
  | `codigo_totp` | `totp_code` | `sugestoes` | `suggestions` |
  | `preenchimento` | `fill` | `pedido_de_login` | `login_request` |
  | papel `agente` | `agent` | `extensao` | `extension` |
  | erro `nao_autorizado` | `unauthorized` | `versao_desconhecida` | `unknown_version` |

  A matriz de permissão por papel (`TODOS`, `SO_AGENTE`, `SO_APP`, `SO_EXTENSAO`…) não
  muda: só os nomes.
- **DT9 — Native messaging e extensão**: as mensagens extensão ↔ `ward-nm` ↔ daemon usam
  o protocolo v2. Uma extensão 0.1.x recebe `unknown_version` e nenhum dado. A versão
  do manifesto vai para 0.2.0, para o Chrome atualizar pela política.
- **DT10 — CLI e deny**: subcomandos `list [--json]`, `unlock`, `lock`, `run`, `ssh`,
  `login`, `playwright`, `help` (mais `-h`/`--help`, `-V`/`--version`); `ajuda`,
  `lista`, `destravar` e `travar` deixam de existir. O `settings-deny.json` da skill nega
  `ward unlock` em todas as formas que hoje nega `ward destravar`, e continua negando as
  formas antigas (`destravar`), para a 0.1.x que ainda estiver instalada enquanto o
  usuário troca.
- **DT11 — Variáveis entregues** (resposta 3): `WARD_ENTRADA`→`WARD_ENTRY`,
  `WARD_GRUPO`→`WARD_GROUP`, `WARD_AMBIENTE`→`WARD_ENVIRONMENT`,
  `WARD_PROTOCOLO`→`WARD_PROTOCOL`, `WARD_ARQUIVO_<NOME>`→`WARD_FILE_<NAME>`,
  `WARD_ARQUIVO_CHAVE`→`WARD_FILE_KEY`. As demais (`WARD_URL`, `WARD_USER`, `WARD_PASS`,
  `WARD_TOTP`, `WARD_TOKEN`, `WARD_HOST`, `WARD_PORT`, `WARD_PASSPHRASE`, `PG*`,
  `MYSQL_*`, `REDIS*`, `MONGODB_URI`, `DATABASE_URL`, `AWS_*`, `SMTP_*`) não mudam. O
  `<NAME>` de anexo e o `WARD_<CAMPO>` dos extras vêm do nome que o usuário deu: são
  dados e não mudam (ex.: `WARD_REGIAO`).
- **DT12 — Variáveis internas e de teste**: `WARD_DEV_EXTENSAO`→`WARD_DEV_EXTENSION`,
  `WARD_DEV_DBUS_SISTEMA`→`WARD_DEV_SYSTEM_DBUS`, `WARD_GERENTE_PROGRAMA`,
  `WARD_CAMINHO`, `WARD_PAI_CHROME_SOCK`, `WARD_EXIGIR_OATHTOOL` e outras `WARD_*` em
  pt-BR viram inglês pelo glossário (`WARD_DEV_DIR`, `WARD_DEV_APP`,
  `WARD_DEV_SOCKET_DIR` ficam). A feature `modo-teste` vira `test-mode` e a marca de
  modo de teste nos binários ganha nome novo. A conferência do V3.37/V9.2 da missão 1
  (`make test-no-test-mode`, `instalar.sh`) passa a recusar a marca **nova e a antiga**,
  e o `strings` do release não pode ter nenhuma das variáveis de teste, nova nem antiga.
- **DT13 — Renomes estruturais** (Fases 4–6). Crates: `ward-plataforma`→`ward-platform`,
  `ward-ponte-teste`→`ward-test-bridge`, `app/nucleo` (`ward-app-nucleo`)→`app/core`
  (`ward-app-core`). Pastas: `instalacao/`→`install/`, `extensao/`→`extension/`,
  `dev/testes/`→`dev/tests/`, `dev/servicos/`→`dev/services/`,
  `dev/conteiner-*`→`dev/container-*`, `dev/fixtures/entradas.json`→`entries.json`. As
  pastas internas do `app/src` (`telas`, `lib/componentes`, `lib/estilo`, `lib/ponte`,
  `lib/teste`) mudam na trilha web (Fases 35–38). Arquivos de `.dev/`:
  `cofre-teste.ward`→`test-vault.ward`, `cofre-teste.estado`→`test-vault.state`,
  `variacoes.txt`→`variations.txt`, `servicos/`→`services/`,
  `extensao/chave.pem`→`extension/key.pem`, `logs/ward-ponte-teste.log`→
  `logs/ward-test-bridge.log`, e o resto pelo glossário. A Fase 5 move os arquivos de
  `.dev/` com `mv` (o cofre de teste também pode ser recriado; a chave da extensão
  nunca).
- **DT14 — Makefile**: alvos e variáveis pelo glossário, sem alias. Proposta (a Fase 1
  fecha):

  | Antigo | Novo | Antigo | Novo |
  |---|---|---|---|
  | `ferramentas` | `tools` | `origens` | `origins` |
  | `teste-ataques-fundacao` | `test-attacks-foundation` | `dev-servicos` | `dev-services` |
  | `dev-servicos-subir` | `dev-services-up` | `dev-servicos-parar` | `dev-services-down` |
  | `dev-servicos-estado` | `dev-services-status` | `dev-servicos-conferir` | `dev-services-verify` |
  | `dev-servicos-trocar-chave-host` | `dev-services-rotate-host-key` | `teste-ataques-servicos` | `test-attacks-services` |
  | `teste-iscas-home` | `test-home-decoys` | `teste-home-real` | `test-real-home` |
  | `dev-cofre` | `dev-vault` | `dev-cofre-conferir` | `dev-vault-verify` |
  | `dev-variacoes` | `dev-variations` | `teste-dev-cofre` | `test-dev-vault` |
  | `app-ponte` | `app-bridge` | `teste-logs-app` | `test-app-logs` |
  | `teste-area-transferencia` | `test-clipboard` | `teste-webkit-real` | `test-webkit-real` |
  | `teste-app-e2e` | `test-app-e2e` | `teste-selecao-revelado` | `test-revealed-selection` |
  | `teste-canais-gst` | `test-gst-channels` | `teste-docker-plataforma` | `test-docker-platform` |
  | `teste-docker-uid` | `test-docker-uid` | `teste-units` | `test-units` |
  | `teste-install` | `test-install` | `teste-politica-chrome` | `test-chrome-policy` |
  | `extensao-crx` | `extension-crx` | `teste-deb` | `test-deb` |
  | `teste-cli` | `test-cli` | `teste-ssh-home` | `test-ssh-home` |
  | `teste-playwright` | `test-playwright` | `teste-sem-modo-teste` | `test-no-test-mode` |
  | `teste-socket-ataques` | `test-socket-attacks` | `teste-skill` | `test-skill` |
  | `teste-migracao` | `test-migration-guide` | `teste-nm` | `test-nm` |
  | `extensao` | `extension` | `teste-extensao` | `test-extension` |
  | `teste-login` | `test-login` | `dev-daemon-parar` | `dev-daemon-stop` |
  | var. `COFRE` | `VAULT` | `VARIACOES` | `VARIATIONS` |
  | `SERVICO` | `SERVICE` | `CONEXOES` | `CONNECTIONS` |
  | `EXTENSAO_ID` | `EXTENSION_ID` | `EXTENSAO_CRX` | `EXTENSION_CRX` |
  | `CHAVE_EXTENSAO` | `EXTENSION_KEY` | `PONTE_DE_TESTE` | `TEST_BRIDGE` |
  | `SEM_MPS` | `NO_MPS` | `SEM_PLAYWRIGHT_PYTHON` | `NO_PLAYWRIGHT_PYTHON` |
  | `GITLEAKS_VERSAO` etc. | `GITLEAKS_VERSION` etc. | `ID_EXTENSAO` (const) | `EXTENSION_ID` |

  Ficam: `build`, `fmt`, `check`, `test`, `audit`, `deny`, `gitleaks`, `app-front`,
  `app-check`, `release`, `install`, `uninstall`, `deb`, `dev-daemon`, `NODE_DIR`,
  `PREFIX`, `DESTDIR`, `WARD_UID`, `BIN_*`, `DEB_DIR`, `PW_PYTHONPATH`. Novos:
  `compile` (regra 6) e `english-check`.
- **DT15 — Caminhos instalados** (resposta 5): `/usr/local/share/ward/extensao/`→
  `/usr/local/share/ward/extension/` e `/usr/local/lib/ward/grupos-criados`→
  `/usr/local/lib/ward/created-groups`. O `instalar.sh` (agora `install/install.sh`)
  apaga os caminhos da 0.1.x numa reinstalação e passa a marca para o nome novo com o
  mesmo conteúdo. O uninstall lê a marca nova ou, sem ela, a antiga (só para saber
  quais grupos a instalação criou; nunca apaga grupo que não esteja na marca). O `.deb`
  0.2.0 não lista os caminhos antigos: o `dpkg` os tira na atualização, e o
  `postinst` confere. A política do Chrome aponta para o `update.xml` do caminho novo.
- **DT16 — Atualização do serviço**: a instalação (`make install` e `postinst`) para o
  `wardd@<uid>.service` em execução antes de trocar o binário, para nenhum daemon 0.1.x
  ficar no ar com clientes 0.2.0. O daemon parado trava o cofre (S8), e o socket o sobe
  de novo na próxima conexão.
- **DT17 — Checador de pt-BR** (`dev/english_check.py`, Fase 2): só a biblioteca padrão
  do Python. Lê os arquivos do git fora de `docs/`, `README.md`, `skill/**/*.md`,
  `.claude/`, lockfiles e binários. Separa, por linguagem, comentários, identificadores
  e literais. Acusa nos comentários e identificadores, e nos nomes de arquivo e pasta:
  (a) letra fora do ASCII; (b) palavra da lista `dev/english-check/pt-words.txt`
  (radicais do domínio, como cofre, senha, entrada, segredo, travar e nível, mais
  palavras funcionais, como "de", "que", "não" e "para", contadas por frase, para não
  pegar "de" em inglês). Não acusa literais. Tem duas listas: `allow.txt` (exceção
  permanente: caminho + palavra + motivo) e `frozen.txt` (nome de fio congelado até a
  fase que o muda; a Fase 60 exige essa lista vazia e apagada). Exceção que não casa
  com nada faz o checador falhar. Tem modo `--paths` (fases) e `--literals` (extrai os
  literais para comparar com a linha de base), e um autoteste que planta uma violação
  por linguagem numa cópia e exige falha. Entra no `make check` como gate na Fase 60.
- **DT18 — Linha de base** (Fase 3): antes de qualquer rename, a partir da tag `v0.1.4`
  (`git worktree` em `.dev/baseline/src`), ficam guardados:
  - no git, em `dev/fixtures/legacy/`, só com valores falsos de D15:
    - `vault-0.1.4.ward`: o cofre de teste completo, v4, senha de teste;
    - `state-0.1.4`;
    - `backup-0.1.4/`: duas versões, gerações seguidas;
    - `protocol-v1.jsonl`: cada pedido e cada resposta da v1, serializados a partir dos
      tipos;
    - `list-v1.json`: a saída do `ward list --json`;
    - `strings-0.1.4.txt`: os literais do produto (CLI, daemon, `ward-nm`, app,
      extensão), extraídos pelo checador;
    - `help-0.1.4.txt`: o `ward --help`;
    - `SHA256SUMS`;
  - fora do git, em `.dev/baseline/`: os binários 0.1.4 de release e de `modo-teste`
    (`ward`, `wardd`, `ward-nm`) e um `.deb` 0.1.4 com CRX3 falso, para os testes de
    atualização e de versão misturada.

  Se o gitleaks acusar algum arquivo de `dev/fixtures/legacy/`, a fase para e pergunta:
  não se acrescenta allowlist genérica (`CLAUDE.md`).
- **DT19 — Versão 0.2.0** em todos os artefatos (workspace Cargo, `app/package.json`,
  `tauri.conf.json`, `app/src-tauri/Cargo.toml`, manifesto da extensão, `.deb`), Fase 60.
- **DT20 — Mensagens das ferramentas de dev**: a saída dos scripts de `dev/` e do
  Makefile é literal e pode ficar em pt-BR (regra 5). Só os nomes de alvo, variável,
  caminho e subcomando citados nela acompanham.

## Critérios de aceite

- [ ] `make english-check` sem nenhum achado no repositório inteiro, com o autoteste
  passando, `frozen.txt` vazio e removido, e o checador dentro do `make check` (V1.*,
  V6.1).
- [ ] Nenhum nome antigo da lista de DT8–DT15 sobra fora de `docs/mission/arquivo/`, de
  `dev/fixtures/legacy/`, da tabela de conversão v4/v1 e das regras de deny que negam
  as formas antigas (V6.2).
- [ ] Os literais do produto são os da linha de base, exceto os nomes que esta missão
  troca, cada um listado (V6.3).
- [ ] O cofre 0.1.4 abre na 0.2.0, migra para v5 sem perder dado e sem segredo no disco,
  e uma gravação morta no meio deixa a v4 intacta (V4.*).
- [ ] Protocolo v2 em todos os clientes; v1 e campo antigo são recusados sem dado (V4.*,
  V5.*).
- [ ] Todas as suítes da missão 1 passam com os nomes novos: `make check`, `app-check`,
  `test-no-test-mode`, `test-socket-attacks`, `test-cli`, `test-ssh-home`,
  `test-attacks-foundation`, `test-attacks-services`, `test-home-decoys`,
  `test-real-home`, `test-dev-vault`, `test-docker-platform`, `test-docker-uid`,
  `test-units`, `test-install`, `test-deb`, `test-chrome-policy`, `test-nm`,
  `test-extension`, `test-login`, `test-playwright`, `test-webkit-real`, `test-skill`,
  `test-migration-guide` e as do app (`test-app-e2e`, `test-clipboard`,
  `test-revealed-selection`, `test-app-logs`, `test-gst-channels`) (V6.*).
- [ ] Cada invariante S1–S15 tem o ataque da missão 1 repetido e falhando, mais os
  ataques novos de RM1–RM3 (contrato, marcados (SEG)).
- [ ] A interface é a mesma da missão 1: mesmas telas, textos e estados, conferidos
  contra `docs/mission/arquivo/m1/evidence/final/` (V6.*, (UI)).
- [ ] Atualização 0.1.4 → 0.2.0 por `make install` e por `.deb` sem caminho antigo,
  com o cofre intacto (V5.*).
- [ ] Artefatos 0.2.0 (`make release`, `make extension-crx` com a chave de sempre,
  `make deb`) prontos e o roteiro do usuário escrito; publicar e instalar ficam com o
  usuário.
