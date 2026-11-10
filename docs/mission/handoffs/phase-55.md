# Fase 55 — Achados da reauditoria final

## Implementado
- **Médio (S8): descendente que sai do grupo não sobrevive.**
  - `wardd` liga `PR_SET_CHILD_SUBREAPER` na partida (`main.rs`); sem ele, não sobe
    (falha fechada). Todo órfão de um `ward run`/`ward ssh`/ponte/Chrome do playwright
    vira filho do daemon em vez de sumir para o `init`/`systemd --user`.
  - Módulo novo `ward-plataforma/src/descendentes.rs`: o fim de uma sessão
    (`Grupo::matar_com_sessoes_filhas`, usado no fim do comando, na CLI que cai, no
    `Drop` e na trava) mata **a árvore inteira do líder** pela cadeia de `ppid` em
    `/proc` e, no daemon, os **órfãos da sessão** (`sid` = líder) e os **órfãos sem dono**
    (filho do daemon fora do registro, com sessão que não é de líder vivo nem a do
    daemon: quem fez `setsid`), mais os descendentes deles. Tudo leva `SIGSTOP` em
    rodadas até nenhum novo aparecer; depois `SIGKILL`; todo sinal por `pidfd`, conferido
    pelo `starttime` do `stat` (nunca acerta quem herdou o pid). O órfão zumbi é colhido
    (`waitpid` só de pid fora do registro e fora da sessão do daemon).
  - Registro dos líderes vivos (`LIDERES`): travado do `fork` ao registro e na colheita,
    para a varredura nunca tomar um líder novo por órfão.
  - A trava também chama `descendentes::matar_orfaos()` (haja sessão em curso ou não).
  - `Relatorio { mortos, sobraram }`; `Grupo::sobraram()`. A CLI só ouve "o comando foi
    encerrado" quando nada sobrou; senão, "N processo(s) do comando continuam vivos"
    (`execucao::aviso_do_fim`).
  - `filhos_com_sessao_propria` (Fase 54c/54l) saiu: a varredura nova o cobre.
- **Baixo: trava.** `conta_como_uso` não conta mais, do papel app, `detalhe` (a janela de
  aprovação pede para mostrar o pedido), `totp` (o formulário renova sozinho) nem
  `responder_aprovacao` (o "Negar" a um `ward run` repetido). O pedido permitido segue
  anotando o uso ele mesmo depois da autorização (`registrar_uso`).
- **Baixo: troca de senha.** O daemon lê o cofre para a troca por `arquivos::ler_cofre`
  (`O_NOFOLLOW | O_NONBLOCK | O_NOCTTY`, só arquivo comum do usuário, identidade do
  mesmo fd conferida com a do cofre aberto) e passa os bytes ao núcleo:
  `gravacao::trocar_senha_lido` / `trocar_senha_com_backup_lido`. As versões que liam
  por caminho com `File::open` ficaram só nos testes do núcleo (`#[cfg(test)]`). Fecha a
  janela entre a conferência da identidade e a 2.ª abertura.
- `processo.rs`: o teste `ataque_processo_com_u202e_no_nome_chega_inteiro_na_cadeia`
  passou a repetir o `spawn` em `ETXTBSY` (corrida antiga com `fork` de outro teste em
  paralelo, que ficou mais frequente com os testes novos de `fork`).

## Segurança
- Invariantes tocadas:
  - **S8** (segredo não segue em uso depois da trava/fim): o ambiente com segredo não
    sobra em processo nenhum que descenda da sessão; a varredura não toca processo de
    fora (outra sessão em curso, filho comum do daemon sem `setsid`, estranho do uid).
    O `sid` não é forjável: o kernel não reusa um pid enquanto ele é id de sessão de
    alguém vivo, e ninguém entra na sessão do líder ou do daemon de fora.
  - **Trava por inatividade (D30/R23)**: tráfego automático da interface e um `ward run`
    negado/expirado não seguram o cofre aberto.
  - **Leitura do cofre** (D39/D62, Fase 54l): sem seguir link nem travar em FIFO também
    na troca de senha.
- Testes de ataque escritos:
  - `crates/ward-plataforma/tests/subreaper.rs` › `ataque_f55_descendente_fora_do_grupo_nao_sobrevive`
    (binário próprio: controle **sem** o subreaper — `setsid` duplo e `setpgid` sobrevivem
    ao fim do comando —, depois com ele: morrem e são colhidos; trava com o comando vivo;
    `matar_orfaos` não leva o líder nem outra sessão; o fim de uma sessão não leva a
    árvore de outra; órfão de filho comum do processo (como o app) não leva sinal).
  - `crates/ward-plataforma/src/filho.rs` › `ataque_f55_neto_fora_do_grupo_morre_na_trava`
    (2.º passo: neto com `setpgid(0,0)` e bisneto com `setsid`, líder vivo; controle com
    `matar` e correção com `matar_com_sessoes_filhas`).
  - `crates/ward-plataforma/src/descendentes.rs` › `alvos_da_sessao_com_subreaper`,
    `sem_subreaper_so_a_arvore_do_lider`, `zumbi_nao_e_alvo`, `stat_com_parenteses_no_nome`.
  - `crates/wardd/tests/run.rs` › `f55_ataque_descendente_fora_do_grupo_morre_no_fim_e_na_trava`
    (`wardd` de verdade: `setsid -f` que escreveria a senha 3 s depois do fim e
    `setpgid` que a escreveria depois da trava; nada escreve, CLI ouve "encerrado").
    Conferido: com o `virar_subreaper` desligado no `main.rs`, o teste falha ("o neto com
    setsid usou a senha depois do fim").
  - `crates/wardd/tests/trava.rs` › `ataque_f55_ward_run_negado_e_janela_nao_adiam_a_trava`
    (a cada 10 min por 4 h: `ward run` numa entrada com aprovação negado, `detalhe` e
    `totp` do app; a 4h00 trava). Conferido: falha com o `trava.rs` de antes.
  - `crates/wardd/src/trava.rs` › `ataque_f55_detalhe_totp_e_resposta_da_janela_nao_contam`.
  - `crates/wardd/tests/configuracoes.rs` › `f55_ataque_fifo_no_lugar_do_cofre_nao_prende_a_troca_de_senha`
    (FIFO parado no lugar: `CofreMudou` na hora; FIFO entrando e saindo do lugar durante
    os pedidos: cada pedido volta em < 5 s). Conferido: com o `gravacao.rs`/`nucleo.rs` de
    antes, o pedido fica preso no FIFO ("a troca de senha ficou presa num FIFO").
  - `crates/wardd/src/execucao.rs` › `aviso_do_fim_so_diz_encerrado_sem_sobra`.
- Dependências novas: nenhuma.
- `unsafe` novo só em `sys.rs` (`prctl` do subreaper, `getpid`/`getsid`,
  `pidfd_send_signal`, `waitpid(WNOHANG)`), cada um com `// SAFETY:` e coberto pelos
  testes acima.

## Ficou de fora
- cgroup v2 por sessão: o daemon de teste não roda sob o systemd, e o `wardd@` não tem
  `Delegate=`; o subreaper com a varredura vale nos dois. Fica como reforço possível.
- Controle "sem a correção" contra o `wardd` inteiro só foi rodado à mão (desligando o
  `virar_subreaper` no `main.rs`); no repositório, o controle automático está no
  `subreaper.rs` (a mesma varredura sem o subreaper). Não pus variável `WARD_DEV_*` para
  desligar o subreaper: seria mais uma porta no daemon de teste.
- `docs/escopo.md`/PRD/contrato não foram editados (só a fase de código foi pedida); ver
  Problemas.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` (967 testes, audit, deny, gitleaks) | 0 |
| `make dev-servicos-subir` | 0 |
| `make dev-daemon WARD_DEV_APP= WARD_DEV_SOCKET_DIR=/run/user/1000/ward-55` | 0 |
| `make teste-cli WARD_DEV_SOCKET_DIR=/run/user/1000/ward-55` (157 ok, 0 falhas) | 0 |
| `make dev-daemon-parar WARD_DEV_SOCKET_DIR=/run/user/1000/ward-55` | 0 |
| `make dev-servicos-parar` | 0 |
| `make teste-ataques-fundacao` | ver abaixo |

## Problemas descobertos
- **Órfão sem dono morre no fim de qualquer sessão** (decisão por segurança): quem sai de
  uma sessão com `setsid` não diz de qual veio. Efeitos: o `chrome_crashpad_handler` do
  Chrome do `ward playwright` (faz `fork` duplo com `setsid`, medido) morre quando
  qualquer `ward run` termina — o Chrome segue, só perde o relatório de crash; um
  descendente do **app** que faça `setsid` e fique órfão (o app fechou) também morre no
  próximo fim de sessão. O terminal do "Conectar" (filho do app, sem `setsid`) não é
  afetado.
- Zumbi de órfão do app (sessão do daemon) depois que o app fecha não é colhido pela
  varredura (para não roubar o status de um filho `std::process` do daemon); fica até o
  daemon sair. Só acontece quando um filho do app sobrevive ao app.
- `detalhe`/`totp` do app não contam mais como uso também quando o usuário clica numa
  entrada ou copia o TOTP (o daemon não distingue o clique do pedido automático); só
  `revelar`, `copiar` e as gravações contam. Segurança antes de conveniência.
- `docs/mission/validation-contract.md` (linha ~433, V2.50) e o comentário do PRD citam
  `gravacao::trocar_senha`; o nome público agora é `trocar_senha_lido` (o teste V2.50 em
  `ward-core/tests/fontes.rs` foi atualizado). O orchestrator decide se ajusta o texto.
- `docs/escopo.md` › "Segurança" (S8) não cita o subreaper nem o órfão sem dono; vale
  registrar lá a regra nova.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-plataforma/src/descendentes.rs` | criado |
| `crates/ward-plataforma/tests/subreaper.rs` | criado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/processo.rs` | alterado |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/trava.rs` | alterado |
| `crates/wardd/tests/run.rs` | alterado |
| `crates/wardd/tests/trava.rs` | alterado |
| `crates/wardd/tests/configuracoes.rs` | alterado |
| `crates/ward-core/src/gravacao.rs` | alterado |
| `crates/ward-core/tests/fontes.rs` | alterado |
| `docs/mission/handoffs/phase-55.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica
- [x] Ficou dentro do escopo da fase
