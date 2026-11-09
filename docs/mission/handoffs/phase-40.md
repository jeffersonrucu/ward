# Fase 40 — `ward-nm` (native messaging)

## Implementado
- **`ward_core::origem`** (novo; D9). `Origem::da_url` usa o mesmo parser estrito da URL do `login` (D41, D55) e devolve esquema, host e porta efetiva. A porta padrão (443/80) é igual à omitida, porque o Chrome tira a porta padrão da URL da aba. `mesma_origem(entrada, aba)` dá falso quando qualquer um dos lados não passa no parser: `chrome://`, `file://`, userinfo, maiúsculas, IP em hex, `http` fora de `localhost`/`127.0.0.1`. Para isso, `url_valida` e `separar_host_porta` passaram a `pub(crate)` e `entrada::validacao` a `pub(crate) mod`.
- **Daemon** (`crates/wardd/src/nucleo.rs`): `sugerir` e `preencher` saíram de `interno`.
  - Os dois aceitam só o papel extensão. Essa é a terceira camada; as duas primeiras são a tabela do protocolo e o `atender_de`. Com o cofre travado, respondem `travado`.
  - `sugerir` devolve as entradas `login` cuja URL tem a mesma origem da aba, só nome e usuário, em ordem de nome. Origem inválida dá lista vazia. A entrada bloqueada aparece, porque pelo escopo o nível regula o agente ("O nível vale para `run`, `ssh`, `login` e `playwright`").
  - `preencher` devolve usuário, senha e o código TOTP atual (relógio do daemon) só quando a origem bate. Origem diferente dá `origem_diferente`, sem valor. Entrada inexistente e entrada de outro tipo respondem igual (`entrada_inexistente`). O log leva só o nome da entrada (`preencher <nome>` ou `… recusado: origem diferente (D9)`), nunca a origem pedida nem os valores.
  - `salvar_login` continua `interno` (Fase 42).
- **`ward-nm`** (`crates/ward-nm`, agora lib + bin):
  - **Argumento.** Aceita exatamente um argumento, `chrome-extension://<32 letras a–p>/`, que é o que o Chrome real passa (conferido). Qualquer outra forma sai com 2 e não escreve nada no stdout.
  - **Endurecimento.** `endurecer_processo()` (dumpable 0, `RLIMIT_CORE` 0) roda antes de tudo.
  - **Mensagens.** Native messaging com tamanho na ordem nativa. Da extensão vêm no máximo 64 KiB (`MAX_MENSAGEM_ENTRADA`), e o corpo só é lido depois de conferir o tamanho. Para a extensão saem no máximo 1 MiB, o limite do Chrome. Mensagem vazia, grande demais ou cortada recebe `quadro_invalido` e o host sai ≠ 0. Mensagens e quadros ficam em `Zeroizing`.
  - **Pedido.** É lido por `decodificar_pedido(Papel::Extensao)`, a leitura estrita do daemon: JSON malformado, chave repetida e campo a mais são recusados, e operação de app ou de agente dá `nao_autorizado`. A lista explícita `OPERACOES` (estado, sugerir, preencher, salvar_login, abrir_app) fica presa à tabela por teste. O pedido é recodificado na forma canônica e vai ao daemon numa conexão nova por mensagem, conferida antes do primeiro byte (D46). No release só vale `/run/ward/<uid>/wardd.sock` com par uid 0. No `modo-teste` vale `WARD_DEV_SOCKET_DIR`/`ward-teste`, com o par não-dumpable, como na CLI.
  - **Resposta.** É lida por `decodificar_resposta(Papel::Extensao)` e conferida contra a operação (`resposta_cabe`: um `preenchimento` nunca responde um `sugerir`). Um `estado` com papel diferente de `extensao` vira `nao_autorizado`. Depois disso a resposta é recodificada. Falha do daemon vira `interno`.
  - **Stderr.** Só textos fixos.
- **Chrome real** (exemplo `crates/ward-plataforma/examples/pai-chrome.rs` e `make teste-nm`). Pendência de D47 e do handoff 21, conferida com o Chrome 156:
  - o Chrome executa o host como **filho direto** do processo do navegador (`/opt/google/chrome/chrome`), sem intermediário;
  - o navegador é **dumpable**: o `exe` dele se lê pelo `/proc`, tanto no headless de teste quanto no Chrome da sessão do usuário, olhado só por `readlink` sem tocar no perfil;
  - `ProcessoPar::conferir_pai` aceita esse pai e recusa o mesmo host aberto por outro processo (`PaiNaoPermitido`).

  O ramo "Chrome não-dumpable: só `PPid` + `Name`" de D47 **não foi implementado** (ver Segurança).
- **`make teste-nm`** (`dev/testes/nm.sh` + `nm.py`, fora do `make check`):
  - Sobe um `wardd` de teste só dele: cópia do cofre e socket próprio por `WARD_DEV_SOCKET_DIR`.
  - V6.1 com o `ward-nm` aberto à mão.
  - Mensagem de 70 000 bytes e JSON malformado.
  - Chrome real headless com perfil, HOME e manifestos de native messaging só em `.dev/nm.*`. A extensão de teste entra pelo `Extensions.loadUnpacked` no `--remote-debugging-pipe`, sem porta TCP.
  - Logs do daemon e do Chrome e o relatório da extensão conferidos contra `.dev/variacoes.txt`.
- **`make teste-docker-plataforma`** (Fase 40 no `dentro.sh`): o `ward-nm` do **release** instalado root:`ward-nm` 2755, o `chrome-falso` (exemplo novo do `ward-nm`) em `/opt/google/chrome/chrome` (root 0755) e o `wardd` de teste real com o socket do "systemd" por `LISTEN_FDS`. Fazem parte do alvo:
  - **Papel e origem.** Papel extensão. Sugerir e preencher em `localhost:8081`. Em `127.0.0.1:8081`, recusa (V6.2) e lista vazia. `revelar` e `lista` dão `nao_autorizado`.
  - **V6.1.** O host executado direto, um pai que é cópia do Chrome, o Chrome com `LD_PRELOAD` e uma cópia do host sem setgid: todos ficam agente, sem `preenchimento`.
  - **Limites e log.** Mensagem acima de 64 KiB recebe `quadro_invalido` e o host sai 1. O log `trace` do daemon não tem `falso-` nem a origem pedida.
- **`make teste-sem-modo-teste`**: compila o `ward-nm` de release e roda `strings` nele (sem `WARD_DEV_*`).
- `CLAUDE.md`: linha nova do `make teste-nm`; a do `teste-docker-plataforma` cita a Fase 40.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1 (D5, D9).** Valor de credencial só sai em `preenchimento`, só para o papel extensão e só com a origem da aba igual à da entrada. A comparação é no daemon, com o parser estrito. O host não deixa passar resposta de outro papel nem de outra operação. Agente e app recebem `nao_autorizado` em sugerir e preencher; a extensão recebe `nao_autorizado` em toda operação de app e de agente.
  - **S5 (D46).** O host só fala com o servidor conferido: no release, pasta do root e par uid 0 (contêiner).
  - **S8 (D47).** O host setgid não é dumpable desde o `exec` e chama `PR_SET_DUMPABLE=0` de novo. O papel extensão sai do egid mais o pai Chrome pelo `/proc`.
  - **S13.** Daemon e host não registram origem nem valores. Conferido com `RUST_LOG=trace` e com a lista de variações.
- **D47, pai não-dumpable.** O Chrome real é dumpable, então a conferência pelo `/proc` (exe, inode, dono root, `TracerPid`, `LD_*`) vale e falha fechada. Não implementei o ramo mais fraco (`PPid` + `Name` + dono do exe), porque o `Name` se forja por `prctl(PR_SET_NAME)`. Se uma versão futura do Chrome ficar não-dumpable, a extensão para de funcionar (falha fechada), em vez de ficar aberta.
- **Testes de ataque escritos**
  - `crates/ward-core/src/origem.rs`:
    - `ataque_origens_parecidas_nao_casam`: `127.0.0.1` × `localhost`, outra porta, outro esquema, subdomínio, sufixo, userinfo nos dois sentidos, `%3a`, maiúsculas, zero à esquerda, `\`, `chrome-extension:`, `file:`, `javascript:`, espaço, `\n` e ponto final;
    - `url_invalida_nao_tem_origem`.
  - `crates/wardd/tests/extensao.rs`:
    - `v6_2_sugerir_so_a_origem_da_aba`;
    - `v6_2_preencher_so_na_origem_certa`: recusa sem valor, `travado`, outro tipo igual a inexistente;
    - `v6_1_agente_e_app_nao_sugerem_nem_preenchem`: as três camadas;
    - `ataque_extensao_sem_operacoes_de_app_nem_de_agente`: a tabela inteira, mais pedidos de app e de agente bem formados atendidos como extensão.
  - `crates/ward-nm/src/testes.rs`:
    - `ataque_operacoes_de_app_e_de_agente_nao_chegam_ao_daemon`;
    - `ataque_json_malformado_e_campos_a_mais`;
    - `ataque_resposta_do_daemon_fora_do_papel_ou_da_operacao_nao_sai`: segredo, token, lista ou preenchimento em outra operação, corpo cru e daemon fora do ar;
    - `estado_sem_o_papel_extensao_vira_nao_autorizado`;
    - `ataque_mensagem_acima_do_limite_vazia_ou_cortada`: 64 KiB + 1, 4 GiB, 0, cortada;
    - `escrever_recusa_acima_do_limite_do_chrome`;
    - `operacoes_sao_as_da_extensao`;
    - `origem_da_extensao`.
  - `crates/ward-nm/tests/binario.rs`:
    - `ataque_argumentos_fora_da_forma_saem_sem_nada_no_stdout`;
    - `ataque_mensagem_grande_demais_e_malformada`;
    - `sem_daemon_responde_interno`.
  - `dev/conteiner-plataforma/dentro.sh` (Fase 40): V6.1 (direto, cópia do Chrome, Chrome com `LD_PRELOAD`, cópia do host), V6.2, mensagem grande e log sem valor nem origem.
  - `dev/testes/nm.py` (`make teste-nm`): V6.1 no host e o controle do pai com o Chrome real.
- **Dependências novas:** nenhuma crate nova no `Cargo.lock`. O `ward-nm` passa a usar `ward-core`, `ward-plataforma` e `zeroize`, todas do workspace e já no lock, com o motivo no `Cargo.toml`. A dependência normal é sem `modo-teste`, e o `make teste-sem-modo-teste` confere.

## Ficou de fora
- **Extensão MV3 e popup:** Fases 41/42. O protocolo extensão ↔ host é o do daemon (`{"v":1,"op":…,"dados":…}`, segredos em base64), e a Fase 41 o segue.
- **`salvar_login` no daemon:** Fase 42. O host já repassa a operação, e o daemon responde `interno`.
- **V6.2 (UI)**, o popup que não sugere: Fase 41. Esta fase cobre o lado do daemon e do host.
- **Ramo do pai Chrome não-dumpable de D47:** não implementado, pelo motivo em Segurança.
- **Conferência do id da extensão no host:** ele aceita qualquer id bem formado. Quem restringe é o `allowed_origins` do manifesto, que o Chrome aplica. O id fixo só existe quando a extensão ganhar `key` (Fase 41) e o manifesto for instalado (Fase 51).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas`, `make dev-cofre`, `make dev-variacoes` (a 1ª vez falhou com o disco cheio, ver Problemas) | 0 |
| `cargo test -p ward-core --lib origem` | 0 |
| `cargo test -p wardd --all-features --test extensao` | 0 |
| `cargo test -p ward-nm` (com e sem `--all-features`) | 0 |
| `make teste-nm` | 0 |
| `make teste-docker-plataforma` | 0 |
| `make teste-sem-modo-teste` | 0 |
| `make fmt` | 0 |
| `make check` | 0 |

## Problemas descobertos
- **Disco cheio durante a fase.** O `/` chegou a 380 KB livres, com os `target/` dos worktrees e do checkout principal e o cache do Docker. Não apaguei nada de fora do meu worktree; esperei até o espaço voltar (alguém liberou 22 GB). Com várias fases em paralelo, o orchestrator precisa cuidar do espaço: cada `target/` tem de 3 a 12 GB.
- **`--load-extension` não serve no Chrome de marca (137+).** O teste carrega a extensão pelo CDP (`Extensions.loadUnpacked` com `--enable-unsafe-extension-debugging` e `--remote-debugging-pipe`). As Fases 41 e 44 e o QA vão precisar do mesmo caminho, ou do Chrome for Testing.
- **Native messaging com `--user-data-dir`.** O Chrome procura os manifestos em `<user-data-dir>/NativeMessagingHosts/`. É assim que os testes ficam fora de `~/.config/google-chrome`, e o QA da Fase 41 pode fazer igual.
- **Papel extensão no host de desenvolvimento.** Sem os grupos e sem setgid, o `ward-nm` de teste é sempre agente contra o `make dev-daemon`. O QA da Fase 41 (V6.2 (UI), V6.7) não vai conseguir preencher pelo Chrome de teste. Seria preciso uma exceção de `modo-teste` para a extensão, análoga a D63: sem o egid, mas com o pai Chrome instalado conferido pelo `/proc`. Isso é decisão do planner e não fiz. Hoje o caminho positivo só roda no contêiner.
- **`estado` no host.** O `codificar_resposta` exige que o papel do `estado` seja o da conexão, então o host não consegue repassar um `estado` com papel agente. Ele responde `nao_autorizado`, e a extensão (Fase 41) deve tratar isso como "host não reconhecido".
- **Uma conexão por mensagem.** O host não fica com conexão parada no daemon e não ocupa vaga entre mensagens (handoff 24a).
- **Daemon fora do ar.** O protocolo não tem código para isso, então o host responde `interno` à extensão. A Fase 41/42 talvez queira um código próprio para o popup.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/origem.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/entrada.rs` | alterado (`pub(crate) mod validacao`) |
| `crates/ward-core/src/entrada/validacao.rs` | alterado (duas funções `pub(crate)`) |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/extensao.rs` | criado |
| `crates/ward-nm/Cargo.toml` | alterado |
| `crates/ward-nm/src/lib.rs` | criado |
| `crates/ward-nm/src/main.rs` | alterado |
| `crates/ward-nm/src/testes.rs` | criado |
| `crates/ward-nm/tests/binario.rs` | criado |
| `crates/ward-nm/examples/chrome-falso.rs` | criado |
| `crates/ward-plataforma/examples/pai-chrome.rs` | criado |
| `dev/testes/nm.sh` | criado |
| `dev/testes/nm.py` | criado |
| `dev/testes/docker-plataforma.sh` | alterado |
| `dev/testes/sem-modo-teste.sh` | alterado |
| `dev/conteiner-plataforma/dentro.sh` | alterado |
| `Makefile` | alterado (`teste-nm`, `teste-docker-plataforma`) |
| `CLAUDE.md` | alterado (linhas dos alvos) |
| `Cargo.lock` | alterado (só as ligações do `ward-nm`) |
| `docs/mission/handoffs/phase-40.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados. Nenhum manifesto foi instalado no Chrome do usuário nem em `/etc`, e nenhum `sudo` rodou. O Chrome de teste usou perfil e HOME em `.dev/nm.*`, apagados no fim.
- [x] Teste de ataque para cada (SEG) tocado (V6.1, V6.2 lado do daemon e do host)
- [ ] (UI/DESIGN) Não se aplica
- [x] Ficou dentro do escopo da fase
