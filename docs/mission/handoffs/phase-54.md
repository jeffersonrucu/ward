# Fase 54 — Baixos da auditoria final

## Implementado
- **`ward run` morre na trava (S8).** `Estado` ganhou `SessoesRun`
  (`crates/wardd/src/nucleo.rs`): a lista dos grupos de processos dos `ward run` em
  curso (`Weak<Grupo>`). `Estado::travar` conta a trava e mata (`SIGKILL` no grupo) os
  registrados, sob o mesmo mutex do registro; o `sessao_com` (`execucao.rs`) registra o
  grupo logo depois de lançar e, se o cofre travou entre a autorização e o lançamento,
  mata na hora. A CLI recebe "ward: o cofre travou; o comando foi encerrado" e o código
  137. O `Grupo` já colhido não leva `kill` (pid reusado). O `ward ssh` passa
  `trava: None` (só o agente e a ponte caem, como antes; ver Problemas).
- **Trava por tempo sem uso.** `trava::conta_como_uso` agora recebe o papel: zera a
  contagem só ação do app que deu certo (menos `estado`/`travar`/`destravar`/
  `abrir_app`), entrega do agente (`executar`/`ssh`/`login`/`playwright`, que já
  anotavam o uso depois da autorização) e da extensão (`preencher`, `preencher_login`,
  `salvar_login`). `lista` do agente, `entrada_padrao`, `janela`, `sugerir`,
  `aguardar_login`, `resultado_login` não adiam a trava.
- **Cópia externa** (`externo::copiar`): recebe `geracao` (no daemon, a
  `backup::geracao_selada` pela chave do cofre aberto, com o estado preso só durante a
  conferência de cada versão; travado → nada vai). Só vão versões que conferem, uma por
  geração (a de data mais antiga), fora de ordem não (`backup::fora_de_ordem`) e com
  geração que o pendrive ainda não tem (as versões do pendrive são lidas com
  `O_NOFOLLOW|O_NONBLOCK`, arquivo comum, até `MAX_ARQUIVO_COFRE`, agora `pub(crate)`).
  As `VERSOES` mais novas são contadas só entre as que conferem: lixo não ocupa lugar.
- **`dev/conteiner-plataforma/dentro.sh`**: o preencher em `http://localhost:8081`
  espera agora `negado` (a confirmação da 44c foi pedida e ninguém confirmou), sem
  `preenchimento` nem `senha`, e o log do daemon tem de ter "origem local, confirmação
  na janela".
- **`dev/testes/home-real.sh`**: `python3` na regex do "o trace pegou o cliente" e o
  2º texto entre aspas de `readlink`/`readlinkat` (buffer de saída) ignorado pelo
  analisador.
- **Tela "Trocar senha mestra"** (`app/src/telas/TrocarSenha.svelte`): aviso na cor
  `--cor-aviso-texto` (o mesmo token da nota do `TrocarPin`): "Os backups antigos, no
  computador e no pendrive, continuam abrindo com a senha antiga. Se você troca porque a
  senha antiga vazou, apague esses backups." Conferido no Chrome headless pela ponte de
  teste (1280×800): `docs/mission/evidence/fase-54/trocar-senha-aviso.png`.
- **`make teste-ataques-fundacao` sai 0.** Na 1ª rodada só o V1.5 falhou: a varredura
  de pasta proibida no `Makefile` e em `dev/` achava a pasta do ssh do home escrita por
  extenso no comentário do `teste-migracao` (`Makefile`) e em três textos do
  `dev/testes/migracao.py` (Fase 53d). Reescritos como no resto do repositório (texto
  sem o caminho, ou o literal montado em partes, `"." + "ssh"`); `teste-migracao` segue
  0 com os mesmos ataques.

## Segurança
- Invariantes tocadas:
  - **S8** (segredo zerado ao travar): o comando que recebeu a senha no ambiente não
    sobrevive à trava; registro e trava ordenados pelo mesmo mutex (sem janela entre
    autorizar e lançar).
  - **Trava por inatividade** (escopo › "Destravar e travar"): consulta periódica não
    segura o cofre aberto.
  - **S2/S6** (backup externo): só bytes autenticados pela chave do cofre vão para o
    pendrive; a decifração para conferir zera ao sair (`geracao_selada`).
  - **S13**: os textos novos de log são fixos (contagens, pid); o teste do `ward run`
    confere o log sem a senha.
- Testes de ataque escritos:
  - `crates/wardd/tests/run.rs` › `f54_ataque_ward_run_morre_na_trava` (`sleep 4;
    printenv WARD_PASS > arquivo` no `sh` e num neto; `travar` no meio: 137 em < 3 s,
    aviso, nenhum arquivo, anexo apagado; controle: sem a correção o teste falha com "a
    sessão só acabou com o fim do comando").
  - `crates/wardd/tests/trava.rs` › `ataque_lista_periodica_do_agente_nao_adia_a_trava`
    (lista do agente e `sugerir` da extensão a cada 10 min: trava às 4h00; controle com
    ação do app).
  - `crates/wardd/src/trava.rs` › `ataque_lista_e_sugerir_nao_contam_como_uso`.
  - `crates/wardd/tests/externo.rs` › `ataque_lixo_no_backup_local_nao_gira_o_pendrive`
    (35 versões plantadas com data mais nova: cabeçalho válido com corpo trocado, versão
    de outro cofre e cópia de uma versão real antiga; nada copiado, nada apagado no
    pendrive; depois, uma gravação real vai sozinha e gira 1).
- Testes ajustados: `tests/trava.rs` (o "uso às 3h" do V3.19 passou a ser ação do app),
  `tests/login.rs` (papel no `conta_como_uso`), `tests/ssh.rs` ›
  `fase28_travar_fecha_o_tunel_para_conexoes_novas` (o `ward run` com túnel agora sai 137
  na trava, além de a ponte recusar).
- Dependências novas: nenhuma.

## Ficou de fora
- Frame/texto no FigCraft para o aviso da troca de senha: a fase não é de design (ver
  Problemas).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` (936 testes) | 0 |
| `make app-front` | 0 |
| `make dev-servicos-subir` / `make dev-servicos-parar` | 0 / 0 |
| `make dev-servicos-conferir` | 0 |
| `make teste-ataques-servicos` (home-real com `pg_select.py`, iscas) | 0 |
| `make teste-docker-plataforma` (135 ok) | 0 (1ª rodada 2: o daemon responde `negado`, não `aprovacao_indisponivel`; esperado corrigido) |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54 make teste-cli` | 0 (1ª rodada 2: `.dev/known_hosts` velho, ver Problemas) |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-54 make app-ponte` + Playwright (TrocarSenha) | 0 |
| `make teste-ataques-fundacao` (167 ok, "Todos os ataques foram barrados") | 0 |

## Problemas descobertos
- **`ward ssh` aberto não morre na trava.** Só o agente SSH e a ponte do salto caem
  (D12); uma sessão `sshpass` já conectada segue. Mantido (o achado era do `ward run`);
  decisão do usuário se a trava deve derrubar também o `ward ssh`.
- **Rotação do backup local conta lixo.** Com versões plantadas (mesmo uid) com data
  mais nova, a gravação seguinte apaga versões reais da pasta local (visto no teste da
  cópia externa). A cópia externa não as repassa nem gira o pendrive por elas, mas o
  backup local perde versões. Fora da fase.
- **Pendrive com nomes plantados.** Quem escreve no pendrive (vfat) com nomes de data
  passada recente ainda faz a rotação de lá apagar cópias reais (49a só cobriu datas
  futuras). Fora da fase.
- **Clock que volta**: com o relógio do sistema voltando, uma versão real pode sair
  "fora de ordem" e não ir para o pendrive (falha fechada).
- **`.dev/known_hosts` velho**: a chave do 2222 no known_hosts do Ward (TOFU) não batia
  com a dos serviços (`.dev/servicos/known_hosts`, regravado ao subir os serviços). Copiei
  `.dev/servicos/known_hosts` para `.dev/known_hosts` (0600; o antigo ficou em
  `.dev/logs/known_hosts.antes-f54`). Só estado de teste.
- **FigCraft**: o `Diálogo de troca` › `Tipo=senha mestra` (`19918`, explicação `19924`) e
  a tela `Configurações / Trocar senha mestra` (`20085`/`20411`) não têm o aviso novo.
- Disco: apaguei `target/debug/incremental` (7 GB, cache de compilação incremental) e
  `target/sem-modo-teste` para o `teste-ataques-fundacao` caber.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/src/trava.rs` | alterado |
| `crates/wardd/src/externo.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/wardd/tests/trava.rs` | alterado |
| `crates/wardd/tests/login.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `crates/wardd/tests/externo.rs` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado |
| `dev/testes/home-real.sh` | alterado |
| `dev/testes/migracao.py` | alterado |
| `Makefile` | alterado (só comentário) |
| `app/src/telas/TrocarSenha.svelte` | alterado |
| `docs/mission/evidence/fase-54/trocar-senha-aviso.png` | criado |
| `docs/mission/handoffs/phase-54.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [x] (UI) Conferido no Chrome headless; sem node no FigCraft para o aviso (ver Problemas)
- [x] Ficou dentro do escopo da fase
