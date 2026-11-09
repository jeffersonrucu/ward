# Fase 28b — Correções do scrutiny do M4 (1ª reprovação)

## Implementado
- **V4.27 com o home do passwd coberto** (`make teste-ssh-home`, novo; `dev/testes/ssh-home.sh` + `dev/testes/ssh_home.py`):
  - Roda num bwrap. A raiz é o tmpfs do próprio bwrap, com cada pasta de `/` de volta por bind. O home do passwd e o `/tmp` ficam em tmpfs, e só o repositório volta.
  - A raiz própria existe por um motivo: no namespace de usuário, as pastas do root aparecem como de `nobody`. O `wardd` de teste recusa uma pasta de socket com pasta de `nobody` acima (D7/D46). Com a raiz do bwrap, o socket fica em `/ward-ssh-home/sock`, só com pastas do usuário acima. O código do daemon não mudou.
  - Iscas no home coberto, todas falsas:
    - na pasta do ssh, um `config` com `Host *`, `ProxyCommand touch /tmp/sentinela-ssh`, `LocalCommand` de sentinela, `IdentityAgent` do agente falso e `IdentityFile`;
    - um `known_hosts` com chave de host trocada para todos os serviços (se fosse lido, o ssh recusaria);
    - um `id_ed25519` de sentinela;
    - FIFOs em `id_rsa`, `id_ecdsa`, `*-cert.pub`, `known_hosts2`, `authorized_keys`, `environment`, `rc`, `.pgpass`, `.psqlrc`, `.pg_service.conf` e `.netrc`.
  - Detectores:
    1. `strace`: `/usr/bin/ssh`, `sshpass` e `sftp` são trocados, só dentro do bwrap, por um invólucro. Ele roda o original sob `strace -D -f`, e o `-D` mantém o pid, que o agente de D12 e a ponte conferem. Cada traço passa pelo analisador de caminhos, o mesmo critério do `home-real.sh`, com o diretório de partida gravado pelo invólucro.
    2. inotify em todas as pastas do home, por ctypes (sem dependência nova).
    3. Escritores das FIFOs, que anotam cada abertura.
    4. Agente falso na variável do agente do ssh, posta no ambiente do daemon e no da CLI.
  - `wardd` de teste só do script, com `HOME` = o home coberto, cópia do cofre e `known_hosts` vazio na pasta própria em `.dev/ssh-home.*`.
  - Suíte: `ward ssh` por senha, por chave, pelo salto, `sftp`, interativo num pty, e `ward run acme-pg-tunel` pelo túnel. Cada passo tem de dar certo.
  - Resultado: nenhuma sentinela, nenhum evento do inotify, nenhuma FIFO aberta, nenhuma conexão ao agente falso e nenhum caminho do home nos 7 traços (7 `ssh`, 5 `sshpass`, 1 `sftp`). Traços, saída da CLI e log trace do daemon saem sem padrão de `.dev/variacoes.txt`.
  - Controles positivos: um `ssh` sem o isolamento cria a sentinela, gera evento no inotify, abre FIFO, conecta no agente falso e é acusado pelo analisador.
- **V4.21 cronometrado** no `dev/testes/cli.py` (`make teste-cli`):
  - Com o cofre travado, `ward run acme-pg-dev -- /bin/true` saiu 125 em 120,0 s. A conferência aceita 118 a 122 s.
  - Um segundo `ward run` fica esperando. O `ward destravar` de outro terminal o solta, e ele sai 0.
  - A suíte fica cerca de 2 min mais longa.
- **V4.1** no `cli.py`: confere explicitamente 21 entradas visíveis.
- **`required-features`** em `crates/wardd/Cargo.toml`:
  - `run`, `socket`, `trava`, `app_teste` e `app_valor` pedem `modo-teste`; `ssh`, `login` e `playwright` pedem `ganchos-teste`.
  - Antes, `cargo test -p wardd` sem a feature falhava em tempo de execução: 6 testes em `run`, 3 em `socket` e 2 em `trava`.
- **Contrato**:
  - V4.1 passa a dizer 21 entradas visíveis (as 22 menos `gama-bloqueada`).
  - V4.9 passa a usar `bash -c '… <<< ls'`, ou `sh -c` com `printf 'ls\n' |`, e `SSHPASS="$WARD_PASS"` explícito.
- **PRD**:
  - Divergências registradas em D3 (`--token-fd` no lugar de `--token`), D12 (agente só Ed25519, por causa do RUSTSEC-2023-0071) e D13 (salto sem `ProxyJump`, pela ponte com `ssh -W`), mais uma nota no item de D43 sobre o `ProxyJump`.
  - RA1: consequência do aceite do usuário (2026-11-09) registrada. O aceite já tinha vindo do master.
- O master foi trazido por merge (fast-forward até `f9bbb56`, com o RA1 aceito e a D66).

## Segurança
- Invariantes tocadas: S1 e R19/D13. O V4.27 é o teste do ataque: um config com `ProxyCommand`, um `known_hosts` trocado, identidades e um agente plantados no home não são usados, e os controles provam que os detectores funcionam.
  - S1: nenhum segredo nos traços, na saída nem no log.
  - Nenhuma mudança em código de produção.
- Testes de ataque escritos: `dev/testes/ssh_home.py` (V4.27, com os 5 controles) e `dev/testes/cli.py` › V4.21 cronometrado.
- Dependências novas: nenhuma. O inotify vai por ctypes; `bwrap` e `strace` já eram usados pelo `teste-home-real`.

## Ficou de fora
- **V4.13 (RA1)**: risco aceito pelo usuário; só a consequência foi registrada no PRD.
- **`CLAUDE.md`**: não foi alterado (configuração do usuário). Falta acrescentar na tabela uma linha para `make teste-ssh-home`, com o texto do comentário do Makefile, e citar o V4.21 cronometrado na linha do `teste-cli`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p wardd --tests` (sem feature, antes da correção: 11 falhas em run/socket/trava) | 101 (registrado) |
| `make teste-ssh-home` | 0 |
| `WARD_DEV_SOCKET_DIR=/run/user/1000/ward-teste-28b make dev-daemon` + `make teste-cli` | 0 (138 ok, 0 falhas) |
| `make dev-daemon-parar` (mesmo socket) | 0 |
| `cargo test -p wardd --test run` (sem feature, depois): "requires the features: `modo-teste`" | 101 (esperado) |
| `make check` | 0 (tudo limpo) |

## Problemas descobertos
- O contrato V4.32 ainda cita `ward ssh --token <aleatório>`. A divergência agora está no PRD (D3), mas o texto do contrato devia passar a `--token-fd`. Não mexi porque o plano lista só V4.1 e V4.9.
- A varredura do V1.5 (`dev/testes/ataques-fundacao.sh`) procura caminhos proibidos em `dev/` e no `Makefile`. Arquivos que já existiam batem nela (`dev/conteiner-deb/dentro.sh`, `dev/conteiner-units/dentro.sh`, `dev/conteiner-install/dentro.sh`, `dev/testes/migracao.py`, `dev/testes/skill.py`, `dev/testes/install.sh`, linhas 395 e 444 do `Makefile`). Não corrigi; vai ao orchestrator. No `cli.py`, que esta fase já alterava, montei por partes o nome da variável do agente. Os arquivos novos estão limpos.
- O `wardd` de teste avisa "memória sem travar" (RLIMIT_MEMLOCK finito) fora da unit. É esperado no modo de teste; não muda nada aqui.
- No worktree, o `.dev/` foi copiado do checkout principal (cofre de teste, `variacoes.txt`, `servicos/`, `ferramentas/`), porque as chaves precisam bater com os serviços que já estavam de pé. Nenhum dado real.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/testes/ssh-home.sh` | criado |
| `dev/testes/ssh_home.py` | criado |
| `Makefile` | alterado (alvo `teste-ssh-home`) |
| `dev/testes/cli.py` | alterado (V4.21 cronometrado, V4.1 = 21, nome por partes) |
| `crates/wardd/Cargo.toml` | alterado (`[[test]]` com `required-features`) |
| `docs/mission/validation-contract.md` | alterado (V4.1, V4.9) |
| `docs/mission/prd.md` | alterado (D3, D12, D13, D43, RA1) |
| `docs/mission/handoffs/phase-28b.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V4.27)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (exceto o nome por partes no `cli.py`, registrado)
