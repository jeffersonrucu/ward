# Fase 53 — `docs/migracao.md`: guia da etapa final (Milestone 10)

## Implementado
- **`docs/migracao.md`**: o guia que o usuário executa sozinho, com um "Antes de
  começar" e 8 passos na ordem do V10.1. Cada passo tem "Conferir" e "Voltar atrás"
  (V10.3):
  1. cadastrar as credenciais de trabalho à mão no Ward (as pessoais vão para o
     1Password ou o Chrome);
  2. mover as chaves SSH: as pessoais para o `~/.ssh`, com `chmod 600` e a `.pub`
     gerada; as de servidor para entradas `servidor`, passando pela pasta em memória
     `/run/user/<uid>/migracao/` e coladas no app com `wl-copy <` arquivo;
  3. conferir cada servidor com `ward ssh <entrada> hostname`, com salto e túnel,
     comparando a impressão digital gravada no `known_hosts` do Ward com a que o
     usuário já conhece;
  4. devolver o chaveiro ao gnome-keyring, inclusive "Chrome Safe Storage" e as outras
     entradas criadas por aplicativos:
     - desliga o FdoSecrets do KeePassXC e confere `NameHasOwner`;
     - liga os segredos com `gnome-keyring-daemon --start --components=secrets`;
     - grava cada item com `secret-tool store`, que pede o valor num prompt;
     - antes de tudo, faz backup do `Login Data` do Chrome e anota quantas senhas ele
       tem;
  5. desfazer a montagem, um item por subseção (5a override, 5b ativação D-Bus,
     5c autostart, 5d PAM com `diff` e console de root aberto, 5e sair e entrar).
     Cada arquivo vai para `~/ward-migracao/` e não é apagado;
  6. conferir as senhas do Chrome (número de senhas, "mostrar", um só item
     `application chrome`). Termina com o aviso "Chrome manteve as senhas" para o
     registro do V10.4;
  7. instalar a skill `ward` pelo `skill/README.md` e mover a `cofre` e os
     `cofre-*` para o backup;
  8. só então desinstalar o KeePassXC e guardar o `.kdbx` cifrado.
- **`dev/testes/migracao.py`** e **`make teste-migracao`**, fora do `make check`. O
  script só lê o guia e confere o V10.1–V10.3: ordem e assunto dos passos, os 4 itens
  da `montagem.md` em subseções próprias do passo 5, "Conferir" e "Voltar atrás" em
  cada passo e subseção, as regras de V10.2 e a ausência de segredo em argv.
  - Antes da conferência roda um autoteste com controle e 18 ataques plantados.
- **CLAUDE.md**: linha nova do `make teste-migracao`.

## Segurança
- **Invariantes tocadas:**
  - **V10.2 / R19.** O guia abre dizendo que quem executa é o usuário, num terminal
    fora do Claude Code (nem com `!`). Também diz para nunca colar segredo nem a saída
    dos comandos na conversa. A missão não executou nenhum passo: nada no `~/.ssh`,
    no chaveiro, no KeePassXC, no `~/.aws` nem no cofre real foi tocado.
    - A única leitura fora do repositório foi a da
      `~/.claude/skills/cofre/references/montagem.md` e do `SKILL.md`, para listar os
      itens a desfazer. Nenhum dos dois tem segredo.
  - **S2/S3 no guia.**
    - Segredo nunca em argv: `secret-tool store` lê o valor no prompt e `wl-copy`
      recebe arquivo por `<`.
    - Chave de servidor só passa pela pasta em tmpfs e é apagada logo depois. Nenhuma
      exportação em claro (sem CSV, sem `keepassxc-cli export`).
    - Conferência do chaveiro com `2>&1 >/dev/null`, que descarta o valor e mostra só
      os atributos.
  - **Volta segura do passo 4.** O guia manda conferir que o dono do
    `org.freedesktop.secrets` é o gnome-keyring antes do `secret-tool clear`. Sem
    isso, o `clear` apagaria a cópia do KeePassXC.
  - **S15.** O guia não traz nenhum valor, nem falso.
- **Testes de ataque** (`dev/testes/migracao.py` › `autoteste`): todos são recusados.
  - Ordem dos passos: passos 4 e 5 trocados; KeePassXC sai antes da skill.
  - Estrutura: passo 3 sem "Voltar atrás"; PAM fora do passo 5.
  - Conversa com o Claude: "cole a passphrase na conversa com o Claude"; "peça para o
    Claude rodar `ssh-add`"; regra de não colar removida; "quem executa é você"
    removido; comando com `!`.
  - Segredo em argv: `sshpass -p`; `echo … | secret-tool`; `secret-tool … <<<`;
    `wl-copy <texto>`; `ssh-keygen -N`.
  - Exportação em claro: `.csv`; `keepassxc-cli export`.
  - Valor no guia: chave privada; padrão de `variacoes.txt`.
- **Dependências novas:** nenhuma (só a biblioteca padrão do Python).

## Ficou de fora
- **V10.4.** É do usuário: ele executa o guia e confirma. O registro no `plan.md` é
  do orchestrator.
- **Nada do guia foi executado.** Os nomes de menu do KeePassXC 2.7.6 em português e o
  comportamento do `gnome-keyring-daemon --start --components=secrets` com o override
  ativo saíram da documentação e da `montagem.md`, não de teste na máquina (ver
  Problemas).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `python3 -I -B dev/testes/migracao.py <variacoes do checkout principal>` | 0 |
| `make teste-migracao` (sem `.dev/variacoes.txt` no worktree) | 2 (falha esperada: "rode `make dev-variacoes`") |
| `make teste-migracao` (com cópia do `variacoes.txt` de teste em `.dev/`) | 0 |
| `make check` (1ª vez: parou no gitleaks, que faltava em `.dev/ferramentas/` do worktree) | 2 |
| `make check` (com o gitleaks do checkout principal copiado; sha256 conferido pelo Makefile) | 0 |

## Problemas descobertos
- **Ordem do V10.1 × a técnica.** O contrato põe "devolver o chaveiro" antes de
  "desfazer a montagem". Para isso dar certo, o passo 4 liga os segredos do
  gnome-keyring só na sessão, com o FdoSecrets do KeePassXC desligado, e o passo 5
  torna isso permanente.
  - Não foi testado na máquina, porque a missão não toca no chaveiro.
  - Se `--start --components=secrets` não pegar o nome com o override ativo, o guia
    manda parar e voltar. A alternativa seria fazer 5a–5e antes de gravar a chave.
- **A skill `cofre` para no passo 4**, porque os `cofre-*` usam o Serviço Secreto do
  KeePassXC. Até lá o escopo diz "até lá a skill antiga continua". O guia avisa para
  fazer os passos 4–7 na mesma sessão. A troca da skill fica no passo 7, por exigência
  do V10.1.
- **Atributos do "Chrome Safe Storage".** O guia usa `application chrome` e
  `xdg:schema chrome_libsecret_os_crypt_password_v2` só como exemplo. Ele manda copiar
  os atributos que o KeePassXC mostra na entrada, porque a versão do esquema muda com
  a do Chrome.
- **Sem `.desktop` do app** (pendência da 51). O passo 1 diz "abra o app do Ward" sem
  dizer como. Hoje ele abre pelo daemon ou por `ward-app` no terminal.
- **`make teste-migracao` e o `.dev/` do worktree.** O alvo precisa de
  `.dev/variacoes.txt`. No worktree usei uma cópia do arquivo de teste do checkout
  principal (só valores falsos), apagada no fim. O gitleaks também foi copiado do
  checkout principal para `.dev/ferramentas/bin/`; o `make check` confere o sha256 dele.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/migracao.md` | criado |
| `dev/testes/migracao.py` | criado |
| `Makefile` | alterado (alvo `teste-migracao` e `.PHONY`) |
| `CLAUDE.md` | alterado (linha do `make teste-migracao`) |
| `docs/mission/handoffs/phase-53.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V10.2)
- [ ] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito): não se aplica
- [x] Ficou dentro do escopo da fase
