# Fase 53d — Achados da 2ª auditoria do M10

## Implementado
- **Médio: lint do `dev/testes/migracao.py`.** A chave não volta ao disco por bloco do guia.
  - Todo argumento de bloco é recusado se tiver:
    - `..`;
    - `$` fora de `$(id -u)`;
    - `~` fora de `~/`;
    - curinga (`*?[`, exceto o `'Login Data*'`);
    - chaves (`{`, exceto `{}` e `a{ss}`);
    - pasta do ssh fora de `~/.ssh/` (`/home/u/.ssh`, `$HOME/.ssh`, `.ssh` relativo);
    - pasta em memória escrita de outro jeito que não `/run/user/$(id -u)/migracao/`.
  - **`cp`/`mv`/`install`.** As opções são fixas: `cp` [] ou `-p`; `mv` []; `install` `-m 600` ou `-d -m 700`. Com isso, `--target-directory=…` é recusado. Uma chave, vinda da pasta do ssh (fora `config`/`known_hosts`) ou da pasta em memória, só sai por `install -m 600`, e só nestes sentidos:
    - do ssh para a pasta em memória;
    - da pasta em memória para ela mesma;
    - da pasta em memória para o `~/.ssh`.
  - **`find`.** Só as duas formas exatas da troca dos `Login Data*`, com destino fixo e o `cd` certo antes. Os argumentos do `-exec cp` também são conferidos.
  - **Outros comandos:**
    - `chmod` só 600/700;
    - `grep` só como filtro de pipe e sem arquivo (`-x`, `-qx`, ou `-v '^#'`);
    - `busctl` só `--user status org.freedesktop.secrets`, ou `call` de `SearchItems`/`NameHasOwner`;
    - `cd` só para as pastas do Chrome (e `-`);
    - `diff` só no PAM;
    - `ssh-keygen` com opções na lista; `-F/-l/-R` só nos dois `known_hosts`; `-y/-p` só em chave da pasta em memória ou do ssh.
- **Médio: passo 2a do `docs/migracao.md`.** A chave pessoal agora:
  1. é salva na pasta em memória;
  2. recebe `chmod 600`, e o `ssh-keygen -y` grava a `.pub` no `~/.ssh`;
  3. ganha passphrase com `ssh-keygen -p -f` ainda na pasta em memória;
  4. vai para o `~/.ssh` por `install -m 600`;
  5. tem a cópia em memória apagada com `rm -f`.

  O lint exige essa ordem e recusa o texto "direto em `~/.ssh`".
- **Baixo: passo 3.** `chmod 600 ~/.local/share/ward/known_hosts` logo depois do `>>`, com o motivo: o `preparar_known_hosts` recusa modo ≠ 0600. Não usei `install -m 600 /dev/null`, porque ele zeraria um `known_hosts` que já existe. O lint exige o `chmod` depois do `>>` e antes do primeiro `ward ssh`.
- **Baixo: `atualizar_bandeja`** (`app/src-tauri/src/main.rs`). Na mudança de estado, chama `Servico::na_passagem_da_bandeja(antes, depois)`, que roda `descartar_formulario()` quando o cofre sai de `Destravado` para Travado, SemCofre ou SemDaemon. A regra fica em `bandeja::descartar_na_passagem`, no núcleo, onde tem teste.
- **Baixo: diálogo de arquivo.** `dialogo_de_arquivo` desliga `gtk-recent-files-enabled` nas `GtkSettings::default()` e lê o valor de volta. Se não conseguir desligar, o diálogo não abre (`SeletorIndisponivel`, falha fechada). Vale para "Carregar de arquivo" e "Adicionar arquivo".
- **Baixo: `app/nucleo/src/chave.rs`.** O `ler_ate` lê num buffer fixo `Zeroizing<vec![0; MAX_CHAVE + 1]>`, num laço de `read` (com `Interrupted`), e só faz `truncate`. Não há realocação, e o `Segredo` fica com o mesmo bloco.
- **`docs/instalacao.md` 4a** (baixos do QA do M9):
  - O bloco da `conferir_host` saiu da lista e foi para a coluna 0. Com recuo, o `PY` não fecha o heredoc e o Python dá erro de indentação. O texto explica isso e diz como copiar.
  - Ficou dito que um `path` por `/bin/...` ou `/sbin/...` dá "é um link" e que o mesmo programa por `/usr/bin/...` passa. Há um comando (python como root, na cópia do root) que troca o `path` para `/usr/...`, seguido de reconferir e rodar a `conferir_host` de novo.
  - Conferi os blocos extraídos do guia contra um manifesto falso no scratchpad, sem sudo. `/bin/true` deu `NÃO COPIE: /bin é um link`; depois da troca, `ok: /usr/bin/true …`.
- O comentário do alvo `teste-migracao` no `Makefile` foi atualizado (pendência da 53b).

## Segurança
- **Invariantes tocadas:**
  - S2: a chave não vai para o disco. Isso vale para o lint, para o 2a e para o buffer da chave.
  - S11/S12: a chave fica só no Rust e é descartada na trava de fora.
  - S13: o caminho do arquivo escolhido não vai para o `recently-used.xbel`.
  - V10.2/V10.3 continuam valendo. Nenhum passo do guia foi executado.
- **Testes de ataque:**
  - `dev/testes/migracao.py` › `ataques()`: 23 novos com prefixo `53d:`, 85 ao todo, cada um conferido pelo motivo de recusa.
    - Os 9 casos do auditor: `cp`/`mv` da pasta em memória, `..` para `/tmp`, `..` na origem, `find -exec cp` do ssh, `find` com destino trocado, `chmod 644`, `grep .` de chave, `busctl … Item Delete`.
    - Formas vizinhas: `install` da pasta em memória para o home, `mv ~/.ssh` inteiro, `/home/u/.ssh`, curinga, `$HOME`, `cd ~/.ssh`, `--target-directory`, `diff` de chave, `ssh-keygen -F` em chave, pasta em memória com o uid escrito.
    - Regressões do 2a e do passo 3: salvar direto no ssh, passphrase já no ssh, `cp` no lugar do `install`, `known_hosts` sem `chmod 600`.
  - `app/nucleo/src/chave.rs` › `ataque_leitura_nao_realoca_o_buffer_da_chave`: capacidade `MAX_CHAVE+1` e mesmo ponteiro no `Segredo`, para chave pequena e no limite; acima do limite, recusa. Com o código antigo, falha.
  - `app/nucleo/src/bandeja.rs` › `ataque_trava_de_fora_descarta_o_formulario`.
  - `app/nucleo/src/servico.rs` › `ataque_trava_de_fora_descarta_a_chave_carregada`: depois de Destravado→Travado, SemDaemon ou SemCofre, salvar com `"carregada"` dá `PedidoInvalido`. Destravado→Destravado não descarta.
- **Dependências novas:** nenhuma. `GtkSettingsExt` já vem do `gtk` 0.18.2.

## Ficou de fora
- O `recently-used.xbel` não foi conferido com o diálogo GTK real: o teste precisa de uma sessão gráfica e de interação. Fica para o roteiro nativo. O código lê o valor de volta e falha fechado.
- `atualizar_bandeja` sem bandeja (AppIndicator ausente): não há vigia periódica, então a trava de fora só descarta o formulário quando o app consulta o estado por outra via. É o mesmo comportamento de antes para a bandeja.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make dev-variacoes` | 0 |
| `make teste-migracao` (controle, 85 ataques recusados, guia real ok) | 0 |
| `cargo test --locked -p ward-app-nucleo` (180 ok) | 0 |
| `make check`: origens, fmt, clippy, test, audit e deny ok; o gitleaks faltava no worktree | 2 |
| `make gitleaks` (binário copiado do checkout principal, conferido por sha256 pelo Makefile): "no leaks found" nas varreduras (rodado com `\| tail`; o exit code não foi capturado à parte, e a segunda chamada foi bloqueada pelo isolamento do worktree) | 0 (pela saída) |
| `make app-check` (app-front, fmt, clippy, 16 testes de ACL, audit, deny, release + V5.6) | 0 |

Os `target/` (raiz e `app/src-tauri`) e a cópia do gitleaks em `.dev/ferramentas` foram apagados no fim.

## Problemas descobertos
- O lint ficou mais rígido. Comando novo no guia que use `..`, curinga, `$VAR`, `find`, `grep` com arquivo ou `busctl` fora dos três usos é recusado, e a 53d quer isso. Quem editar o guia ajusta o lint junto.
- O `>>` do passo 3 segue link simbólico. Se o `known_hosts` do Ward for um link, o `>>` escreve no alvo. O daemon recusa o link depois (`O_NOFOLLOW`, `nlink`), mas a escrita já aconteceu. O risco é baixo: só há chave pública ali e a pasta é 0700.
- `docs/escopo.md` não cita o "Carregar de arquivo" (pendência da 53c, não mexi).

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/testes/migracao.py` | alterado |
| `docs/migracao.md` | alterado |
| `docs/instalacao.md` | alterado |
| `Makefile` | alterado (só comentário) |
| `app/nucleo/src/chave.rs` | alterado |
| `app/nucleo/src/bandeja.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `app/src-tauri/src/main.rs` | alterado |
| `docs/mission/handoffs/phase-53d.md` | criado |

## Procedimentos
- [x] `make check` rodado: fmt, clippy, testes, audit e deny ok; gitleaks rodado à parte, ok
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
