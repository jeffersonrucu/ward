# Fase 28e — Achados da auditoria do M4

## Implementado
- **Crítico (oráculo de prefixo, D34 revisto):** o `Fluxo` (`ward-core/src/mascara.rs`) não segura mais nada. Cada `mascarar` devolve o pedaço inteiro na hora (`emitir_ate(lido)`). Saíram `segurar_desde`, `segurando` e `soltar`, e no `wardd/src/execucao.rs` saíram a `PAUSA_INTERATIVA` e a soltura no pty.
  - O estado do autômato continua de um pedaço para o outro.
  - Segredo inteiro num pedaço: sai `••••`.
  - Segredo quebrado entre pedaços: o começo que já saiu fica em claro, e o pedaço que completa o segredo sai marcado a partir do corte.
  - Isso não abre oráculo novo: a marca só aparece quando a saída já contém o padrão inteiro. O comportamento é o mesmo com e sem pty.
- **Alto (`!cmd` do sftp):** `agente_ssh.rs` prende o agente ao primeiro par aceito cujo pai é o `sftp` filho. Esse par é o `ssh` que o `sftp` cria, que conecta primeiro porque roda com `BatchMode=yes`. Depois disso só esse pid exato passa. Nem o próprio `sftp` nem outro filho dele prendem ou usam o agente.
- **Médio (grupo de processos):** em `ward-plataforma/src/sys.rs`, todo filho (pipes, pty e pipe de depuração do Chrome) faz `setsid()`.
  - Saiu o `setpgid` do lado do pai: rodando antes do `setsid` do filho, ele faria o `setsid` falhar.
  - A ponte (`ponte.rs`) exige que o processo dono do socket esteja no grupo **e** na sessão do líder.
  - Não usei o pid exato que o plano cita. Quem conecta é um descendente (`sshpass`→`ssh`, `sftp`→`ssh`, shell→`psql`). Uma sessão não pode ser trocada por outra que já existe, e o kernel recusa `setpgid` entre sessões.
- **Baixo (known_hosts):** `Daemon::known_hosts()` agora devolve `Result` e chama `arquivos::preparar_known_hosts`.
  - A pasta tem de ser diretório de verdade (sem link), do uid efetivo e 0700.
  - O arquivo, se existe, tem de ser regular, sem link (`O_NOFOLLOW`), com `nlink == 1`, do uid e 0600.
  - Se não existe, nasce vazio e 0600 (`create_new`, `O_NOFOLLOW`).
  - Qualquer coisa mais frouxa é recusada antes de rodar qualquer `ssh`: `entrega_falhou` no `ward ssh` e erro de recurso no túnel do `ward run`.
- Documentos:
  - `prd.md`: D11 (buffer), o item interativo antigo de D34 riscado e a nota em S4.
  - `validation-contract.md`: notas de D34 revisto em V2.18, V2.24 e V4.17.
  - `cli.py`: a seção do V4.34 virou o ataque do oráculo, e o V4.28 deixa em 0600 o known_hosts que ele escreve.

## Segurança
- Invariantes tocadas:
  - **S4:** a garantia passa a ser o segredo inteiro no mesmo pedaço, sem segurar nada e sem oráculo.
  - **S1:** o agente do sftp não é usável por `!cmd`.
  - **S8/S2:** como nada é segurado, sobra menos segredo em memória entre pedaços; o pedaço em trabalho continua `Zeroizing`.
  - **D12/D13** (agente e known_hosts) e **D19** (ponte).
  - `unsafe` só em `ward-plataforma/src/sys.rs`, no bloco que já existia, com o `SAFETY` atualizado.
- Testes de ataque escritos. Os seis `d34_*` do `ward-core` falham contra o `mascara.rs` antigo: conferi trocando o arquivo, e a busca em profundidade seguiu "recuperando" até ser parada.
  - `crates/ward-core/src/mascara/testes.rs`:
    - `d34_ataque_oraculo_de_prefixo_da_auditoria_nao_recupera_nada`: a busca em profundidade do `oraculo_pty.py` contra uma máscara com todas as famílias (literal, percent, base64 e miolos, Basic, URL, formas Go/aspas, valor de linha JSON/YAML/dotenv/tab, licença, chave privada por linha). Não aceita nenhum candidato.
    - `d34_ataque_prefixo_certo_e_errado_dao_a_mesma_resposta`: para cada forma, prefixo certo e errado dão a mesma resposta até o fim do padrão (no base64, com folga para o miolo).
    - `d34_ataque_tempo_nada_segurado_ate_o_resto_chegar` (o `tempo.py`).
    - `d34_ataque_prefixo_no_fim_do_pedaco_sai_na_hora_e_igual`.
    - `d34_escape_percent_pela_metade_no_fim_do_pedaco_sai_na_hora`.
    - `d34_pty_prefixo_parado_sai_em_claro_e_o_resto_vira_marca`.
    - Os ajudantes `um_bloco`/`em_pedacos` conferem byte a byte, em todos os testes V2.x, que nada fica segurado.
  - `crates/wardd/tests/ssh.rs`:
    - `d34_ataque_pty_prefixo_certo_e_errado_saem_iguais_e_na_hora` (pty real);
    - `d34_ataque_tempo_sem_pty_prefixo_nao_atrasa`;
    - `ataque_sftp_bang_nao_usa_o_agente`: `!cmd` com o ssh vivo e depois que ele sai; o ssh continua atendido;
    - `ataque_known_hosts_frouxo_recusa_sem_executar`.
  - `crates/wardd/tests/ssh_falso.py`: modos novos `tempo` e `sftp-bang`, e o prefixo errado no modo `pty`.
  - `crates/wardd/src/agente_ssh.rs` › `ataque_sftp_bang_outro_filho_do_sftp_e_recusado`.
  - `crates/wardd/src/ponte.rs` › `ataque_processo_no_grupo_de_outra_sessao_e_recusado`, mais o `par_do_grupo_de_verdade` (grupo sem a sessão é recusado).
  - `crates/ward-plataforma/src/filho.rs` › `ataque_outro_processo_nao_entra_no_grupo_do_filho` (o `entrar_grupo.py` leva EPERM).
  - `crates/wardd/src/arquivos.rs` › `ataque_known_hosts_frouxo_ou_link_e_recusado`: modos 644/640/602/660, pasta frouxa, link, link quebrado, link duro, diretório no lugar do arquivo e pasta que é link.
  - `dev/testes/cli.py` (`make teste-cli`, com os serviços de verdade): primeiro nível do oráculo no pty (nenhum candidato aceito, todos chegam na hora), oráculo por tempo (o candidato nunca espera o `Z`) e a senha inteira num `printf` só, que sai `••••`.
- Conferido ao vivo, com os scripts da auditoria sem mudança, contra o daemon de teste com socket próprio (`WARD_DEV_SOCKET_DIR=/run/user/1000/ward-28e`):
  - `oraculo_pty.py`: "consultas=7 … nada recuperado";
  - `tempo.py`: todo candidato chega em 1,0 s e o `Z` em 2,5 s;
  - `entrar_grupo.py`: "setpgid falhou [Errno 1]" e nada volta do banco.
  - O log do daemon em `trace` não tem nenhum padrão de `variacoes.txt`.
- Dependências novas: nenhuma.

## Ficou de fora
- Teste ao vivo do `!exec` do sftp no `cli.py`: o cofre de teste não tem entrada `ftp-sftp` com chave (`beta-sftp-dev` é por senha). Ficou coberto pelo teste Rust com o sftp falso e pelo teste de unidade.
- Pid exato na ponte: troquei pela conferência de grupo + sessão, pelo motivo acima.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core` | 0 |
| `cargo test --all-features -p wardd -p ward-plataforma` | 0 |
| `make check` | 0 |
| `make dev-daemon` (`WARD_DEV_SOCKET_DIR` próprio) | 0 |
| `make teste-cli` (mesmo socket) | 0 ("0 falha(s)") |
| `oraculo_pty.py` / `tempo.py` / `entrar_grupo.py` da auditoria | os ataques falham |
| `make dev-daemon-parar` | 0 |

## Problemas descobertos
- **Escopo × decisão do usuário:** `docs/escopo.md` › S4 ainda diz "inclusive quando o valor chega quebrado entre dois pedaços da saída". Com D34 revisto, só o pedaço que completa o segredo sai marcado; o começo que já saiu não. Não editei o escopo, que é a fonte de verdade: planner e usuário precisam alinhar S4 com D34 revisto. No PRD, S4 e D11 só ganharam nota.
- **V2.18, V2.24 e V4.17:** o texto original pressupunha segurar o prefixo. Pus notas no contrato; V2.18 e V2.24 continuam marcadas e V4.17 continua aberta. Fica para o planner revisar.
- **`CLAUDE.md`:** a linha do `make teste-cli` ainda cita "V4.31/V4.34 num pty". Não editei o `CLAUDE.md`; o orchestrator atualiza.
- **Mudança de comportamento para o usuário:** um `known_hosts` que já existe com 0644 passa a ser recusado. Ele pode ter sido criado assim pelo `ssh` antigo, com a `umask`. Na máquina real, o `~/.local/share/ward/known_hosts` de antes desta fase pode precisar de `chmod 600` (e a pasta, de 700). O daemon não corrige permissão sozinho: só recusa, com mensagem fixa no log.
- **Túnel com `setsid psql`:** um descendente que sai da sessão por conta própria não usa mais o túnel. Aceito, porque é o lado mais seguro.
- **Saída byte a byte:** um segredo impresso um byte por vez (eco do pty, rede picada em pacotes) agora mostra o começo em claro e só o fim vira `••••`. É o preço aceito de D34 revisto.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/mascara.rs` | alterado |
| `crates/ward-core/src/mascara/automato.rs` | alterado (comentário) |
| `crates/ward-core/src/mascara/testes.rs` | alterado |
| `crates/ward-plataforma/src/sys.rs` | alterado |
| `crates/ward-plataforma/src/filho.rs` | alterado |
| `crates/wardd/src/execucao.rs` | alterado |
| `crates/wardd/src/ssh.rs` | alterado |
| `crates/wardd/src/agente_ssh.rs` | alterado |
| `crates/wardd/src/ponte.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/tests/ssh.rs` | alterado |
| `crates/wardd/tests/ssh_falso.py` | alterado |
| `crates/wardd/tests/run.rs` | alterado (comentário) |
| `dev/testes/cli.py` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/mission/validation-contract.md` | alterado |
| `docs/mission/handoffs/phase-28e.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (cópia do `.dev/` de teste no worktree, apagada no fim)
- [x] Teste de ataque para cada (SEG) tocado (S4/V4.17, V4.26/D12, V4.29 ponte, V4.28 known_hosts)
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
