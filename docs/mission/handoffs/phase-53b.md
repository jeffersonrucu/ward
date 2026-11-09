# Fase 53b — Guia e lint da migração (achados médios/baixos da auditoria do M10)

## Implementado
- **`docs/migracao.md`** (correções da auditoria do M10):
  - **Achado alto (S2).** O passo 2b não usa mais `wl-copy` para a chave. A chave vai para a pasta em memória e o campo "Chave privada" recebe o arquivo pelo botão **Carregar de arquivo** da Fase 53a. O "Antes de começar" diz que, sem esse botão, o 2b não se faz. `wl-copy` só aparece como `wl-copy --clear`, no fim de cada parte que copia algo no KeePassXC (passos 1, 2b e 4).
  - **Chaves de servidor que hoje só existem no `~/.ssh`.** Também vão para o Ward:
    - `install -m 600` para a pasta em memória, depois **Carregar de arquivo**;
    - o passo 3 apaga a chave do `~/.ssh` (`rm -f ~/.ssh/<chave> …`) só depois de conferir o servidor, numa subseção "Depois de conferir";
    - o `~/.ssh/config` é guardado inteiro em `~/ward-migracao/config-do-ssh.antes`, porque não tem segredo;
    - nenhuma chave vai para `~/ward-migracao` (o `ssh-servidores/` saiu do guia).
  - **"nenhum deles tem segredo" corrigido.** A `~/ward-migracao` é descrita como sensível, porque guarda os bancos de senhas do Chrome, e chave ou senha em claro nunca vão para ela.
  - **Passo 3.** Antes do primeiro `ward ssh`, a linha do host sai do `~/.ssh/known_hosts` por `ssh-keygen -F` e vai para `~/.local/share/ward/known_hosts`, inclusive para salto e destino. Depois vem a conferência com `ssh-keygen -l -F`. Se o host não estiver lá, a impressão digital vem por outro canal antes de conectar.
  - **Passo 2a.** Chave pessoal sem passphrase ganha uma com `ssh-keygen -p -f` (interativo). O guia mantém "não tire a passphrase".
  - **Chaveiro (passo 4).**
    - A contagem usa `busctl … SearchItems 'a{ss}' 1 application chrome`, que devolve só caminhos. É feita antes, com o KeePassXC como dono, e depois, no gnome-keyring, e conferida pelo número, sem prometer rótulo nem formato de caminho.
    - Sai o `secret-tool search`.
    - Caso de mais de um item do Chrome: copiar todos. Se dois tiverem os mesmos atributos, parar.
    - O dono é conferido com `grep -x 'Comm=gnome-keyring-d'`.
  - **Voltar atrás do passo 4.** O `secret-tool clear` só roda encadeado à guarda: `busctl --user status org.freedesktop.secrets | grep -qx 'Comm=gnome-keyring-d' && secret-tool clear …`. Se o dono for o KeePassXC, o `clear` não roda.
  - **Voltar atrás do passo 6.** Devolve todos os `Login Data*` de todos os perfis. Os atuais vão antes para `~/ward-migracao/chrome-depois`, por `cp --parents` seguido de `rm`, com o Chrome fechado.
  - **Passo 8.** Antes do `apt remove`: abrir a cópia guardada do `.kdbx` no KeePassXC e conferir as entradas; guardar o arquivo-chave, se houver, separado da cópia. O "Voltar atrás" cita o arquivo-chave.
  - "Exporte para CSV" saiu do texto: agora é "nunca gere uma cópia em texto dele".
- **`dev/testes/migracao.py`** reescrito (o alvo `make teste-migracao` não mudou):
  - **Lista positiva de comandos** em todo bloco cercado (``` ou ~~~, qualquer linguagem ou info string). Regras de argumento:
    - `wl-copy` só `--clear`;
    - `secret-tool` só `store --label=…` e `clear`, e o `clear` só depois de `busctl --user status … | grep -qx 'Comm=gnome-keyring-d' &&`;
    - `rm` só `-f`, e só na pasta em memória e no `~/.ssh`;
    - `sudo` só `cp` do PAM;
    - `ssh-keygen` sem `-N`/`-P`, inclusive colados (`-N''`, `-pN`);
    - chave do `~/.ssh` só para a pasta em memória;
    - `find` sem `-delete`, e `-exec rm` só na troca dos `Login Data*` do Chrome;
    - redirecionamento só para `.pub`, o `known_hosts` do Ward e `/dev/null`; pipe só para `grep`;
    - sem `<`, heredoc, crase nem `$(` fora de `$(id -u)`;
    - nada em `~/cofre` nem `.kdbx`.
  - **V10.2, conversa.** Toda frase que cita Claude, conversa, chat, assistente, IA, LLM, bot, missão ou agente (fora de "agente SSH"/"agente do sistema") tem de estar na lista fechada `FRASES_DESTINO`. Frase dessas com verbo de envio é recusada mesmo com negação, salvo as exceções fixas: a regra, a proibição do `!`, "a missão não executa", o aviso do V10.4, o exemplo do `ward lista` e o aviso do RA3.
  - **`!` (Claude Code)** é recusado em linha de prosa, em código em linha (exceto o `` `!` `` isolado da regra) e em bloco. Bloco indentado (4+ espaços) também é recusado.
  - **Texto todo.** São recusados:
    - `SSHPASS=` e qualquer `*PASS/SENHA/TOKEN/SECRET/KEY*=`;
    - `mysql -p<senha>`, URI com senha, `curl -u`;
    - pipe para `wl-copy`/`secret-tool`, `<<`;
    - `secret-tool lookup/search`, qualquer uso do cli do KeePassXC;
    - `export*`, CSV/HTML/XML, `/tmp` e `/var/tmp`;
    - chave em `~/ward-migracao`;
    - "tire/remova a passphrase" sem negação, "Enter vazio", "deixe sem passphrase";
    - apagar ou mover `~/cofre`/`.kdbx` (prosa ou bloco) antes do passo 8.
  - **Estrutura da 53b.**
    - 2b com **Carregar de arquivo**; 2a com `ssh-keygen -p -f`.
    - Toda parte (`##`/`###`) que manda copiar no KeePassXC ou na área de transferência tem `wl-copy --clear` na mesma parte.
    - Passo 3: `ssh-keygen -F … >> known_hosts do Ward` antes do primeiro `` `ward ssh < ``, e o `rm -f ~/.ssh/<chave>`.
    - Passo 4 com `SearchItems`; Voltar atrás do passo 6 com `-name 'Login Data*'` devolvendo ao Chrome.
    - Passo 8 abre a cópia do `.kdbx` e cita o arquivo-chave antes do `apt remove`.
  - **Autoteste.** Controle mais 62 ataques, todos recusados, cada um pelo motivo esperado (conferido um por um): os 18 da Fase 53, os 22 do auditor (`burla-m10.py`) e 22 novos da 53b.

## Segurança
- **Invariantes tocadas**
  - **S2: chave em claro no disco.** O 2b não manda mais a chave para a área de transferência. O lint recusa qualquer `wl-copy` que não seja `--clear`, `wl-copy <`, pipe para `wl-copy` e qualquer `/tmp`. A chave só passa pela pasta em memória e é apagada logo depois. As que estavam no `~/.ssh` saem do disco depois do passo 3, e não ficam cópias em `~/ward-migracao`.
  - **S3: argv.** Os casos do auditor (`SSHPASS=`, `mysql -p`, URI, `curl -u`, `-N''`) são recusados no texto todo.
  - **V10.2 / R19.** A missão não executou nenhum passo do guia. Não toquei em KeePassXC, `~/.ssh`, `~/.aws`, chaveiro, Chrome nem cofre real. Li o `burla-m10.py` e não o executei; os ataques foram portados para o autoteste.
  - **TOFU do `ward ssh`.** O `accept-new` do Ward (D13) passa a receber a linha que o usuário já tinha antes da primeira conexão.
- **Testes de ataque** (`dev/testes/migracao.py` › `ataques()`; quem roda é o `autoteste`):
  - **Os 22 do auditor, com prefixo `M10:`.**
    - Os nomes de pasta são montados por partes para o V1.5 e o `curl -u` também, para o gitleaks.
    - Dois foram adaptados ao texto novo, com a mesma intenção:
      - "clear sem guarda" agora tira o `grep -qx … &&` da frente do `clear`;
      - "wl-copy sem --clear" tira o `wl-copy --clear` do 2b.
    - Os outros 20 usam as mesmas âncoras e strings do auditor.
  - **Os 22 novos da 53b:**
    - `wl-copy <` da chave (o achado alto); 2b sem **Carregar de arquivo**;
    - `mv` de chave do ssh para `~/ward-migracao`; `ssh-servidores`;
    - `ward ssh` antes do `known_hosts`; passo 3 sem apagar a chave;
    - 2a sem `ssh-keygen -p`; "deixe sem passphrase";
    - `secret-tool search`; passo 4 sem `SearchItems`;
    - um só `Login Data` no Voltar atrás; passo 8 sem abrir a cópia; passo 8 sem arquivo-chave;
    - apagar `~/cofre` no passo 4;
    - `rm` fora da lista; `cat` no bloco; redirecionamento para o home; `sudo` fora do PAM;
    - bloco `~~~` com `tee`; `$(cat …)`; `find -delete`; frase nova citando a missão.
- **Dependências novas:** nenhuma. Só a biblioteca padrão do Python (`shlex` para tokenizar os blocos).

## Ficou de fora
- **Depende da Fase 53a.** O passo 2b cita o botão **Carregar de arquivo** do campo "Chave privada", que ainda não existe no app. O guia diz para não fazer o 2b sem ele.
  - Se a 53a der outro nome ao botão, mudam o 2b, o "Antes de começar" e a checagem `"**Carregar de arquivo**"` do `migracao.py`.
  - Se a 53a aceitar só arquivo em tmpfs, o caminho do guia já é esse: as chaves que estão no `~/.ssh` também passam pela pasta em memória.
- Nenhum passo do guia foi executado (V10.4 é do usuário).
- Não compilei Rust nem rodei o `make check` completo, por pedido do orchestrator: disco baixo, e esta fase não mexe em Rust.
- Não mexi no comentário do alvo `teste-migracao` no `Makefile`, que ainda diz "Fase 53": o escopo era só guia, lint e handoff.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make teste-migracao` (controle + 62 ataques recusados + guia real ok) | 0 |
| `make gitleaks` (1ª vez: `curl -u admin:…` do ataque e um `.pyc` meu do `py_compile` casaram `curl-auth-user`; corrigi montando por partes e apaguei o `__pycache__`) | 2 → 0 |
| `python3 -I -B -m py_compile dev/testes/migracao.py` | 0 |
| grep dos padrões do V1.5 em `dev/testes/migracao.py` | sem achado |

## Problemas descobertos
- **Formato do `busctl --user status`.** O guia e a guarda do `clear` assumem a linha `Comm=gnome-keyring-d` (e `Comm=keepassxc`). Isso não foi testado na máquina. Se o formato for outro, a guarda falha fechada: o `clear` não roda e o usuário vê que o `grep -x` não achou. Vale o usuário conferir no passo 4.6 antes de seguir.
- **`wl-copy --clear` não grava a entrada em `/tmp`.** Pelo código do wl-clipboard, o arquivo temporário só é criado quando há dados para copiar. Não foi testado aqui, porque testar mexeria na área de transferência do usuário.
- **Histórico da área de transferência do COSMIC.** Um gerenciador de histórico pode gravar em disco a passphrase e o valor do "Chrome Safe Storage" copiados do KeePassXC. O guia manda limpar o histórico; o KeePassXC marca a cópia como senha, mas não sei se o applet respeita essa marca.
- **`SearchItems` com o KeePassXC como dono.** Não sei se o KeePassXC pede confirmação nessa chamada. Ela não devolve segredo.
- **Dois itens do Chrome com os mesmos atributos.** O guia manda parar, sem dizer como escolher qual o Chrome usa. Isso é raro, mas, se acontecer, o usuário fica sem caminho no guia.
- **O lint tem a lista fechada `FRASES_DESTINO`.** Toda frase nova do guia que cite Claude, missão, conversa ou agente precisa entrar nela. É de propósito: frase nova com destino é recusada por padrão.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/migracao.md` | alterado |
| `dev/testes/migracao.py` | alterado |
| `docs/mission/handoffs/phase-53b.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não, por pedido do orchestrator (disco baixo; a fase não mexe em Rust). Rodei `make teste-migracao` e `make gitleaks`, os dois com exit 0.
- [x] Só cofre de teste e credenciais falsas usados (nenhum passo do guia executado)
- [x] Teste de ataque para cada (SEG) tocado (V10.2, S2, S3)
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica
- [x] Ficou dentro do escopo da fase
