# Fase 51b — Registrar o RA10 e fechar o resto da 2ª auditoria do M9

## Implementado
- `docs/escopo.md`:
  - **"Fora do modelo de ameaça":** item novo do RA10 (decisão do usuário,
    2026-11-09). Um processo do mesmo uid se passa pelo Chrome diante do `ward-nm` por
    dois caminhos: o exec com troca de pai e a extensão adulterada no perfil
    (`sw.js`, `computed_hashes.json`, `Service Worker`). O item diz o que esse processo
    ganha (sugestão e preenchimento de `login` sem clique, para qualquer origem), que
    vale também para o próprio agente se ele rodar um programa assim, que a parte
    "preencher só pela extensão no domínio certo" do S1 deixa de valer contra ele, e o
    que ainda reduz o risco (`NativeMessagingUserLevelHosts: false`, `ward-nm` setgid e
    não-dumpable).
  - **Modelo 2 reescrito:** a defesa passa a ser "binários em pasta do root (app e host
    setgid)" e o texto diz que a extensão não entra nela, porque o Chrome do Linux roda
    a cópia desempacotada no perfil.
  - **"Arquitetura":** a frase "não pode trocar os binários nem a extensão" virou "não
    pode trocar os binários", com a ressalva da extensão (RA10).
- `docs/mission/prd.md`:
  - **D18 reescrito** pela árvore real (`instalacao/arvore.txt`): units do sistema
    `wardd@`, `tmpfiles.d`, grupos com a marca, setgid 2755, manifesto, política com
    `NativeMessagingUserLevelHosts: false` e a recusa de política conflitante, `.deb`
    pelo mesmo staging (statoverride, Depends) e o limite RA10. Era pendência desde a
    51.
  - O resumo "Fora do modelo" ganhou o RA10.
  - **RA10 novo** em "Riscos abertos", fechado como risco aceito pelo usuário. A
    segurança do M9 conta como aprovada depois desta fase.
- `docs/instalacao.md` 4a:
  - Saiu "esse risco é do app, não do Ward". Entrou o "Quando não copiar": um host
    copiado para o sistema cujo `path`, ou alguma pasta acima dele, seja gravável pelo
    usuário deixa um programa do usuário rodar como filho do Chrome e reabre o ataque.
    O Claude in Chrome, com host no home, cai nesse caso e fica sem a ponte.
  - O passo 1 define a função `conferir_host` (`sudo python3 -I`, lendo a cópia em
    `/root`). Ela recusa "NÃO COPIE" quando:
    - o `name` não confere;
    - o `path` não é absoluto e normalizado;
    - algum componente, do `path` até `/`, é link, não é do root ou tem escrita para
      grupo ou outros;
    - o alvo não é arquivo comum;
    - o alvo é um script `#!` (o interpretador e o que ele carrega podem vir do home).
  - O passo 2 só instala se `conferir_host` passar de novo.
- `instalacao/grupos.sh` (D60, achado baixo): o `conferir_grupo` recusa `group:` e
  `initgroups:` do `/etc/nsswitch.conf` com fonte fora de `files`/`systemd`. O
  `subid:` e o `sudoers:` continuam só com `files`. O parser do `nsswitch.conf` ficou
  mais estrito e casa a base como o glibc a lê:
  - espaço antes do nome e antes do `:`;
  - `:` colado na fonte (`group:sss`);
  - sem diferença de caixa;
  - ação `[...]` sem espaço dentro é ignorada; com espaço, o caso é recusado.
- Testes nos contêineres: `dev/conteiner-install/dentro.sh` ganhou
  `group: files sss` e `initgroups: files ldap`. `dev/conteiner-deb/dentro.sh` ganhou
  `group: files sss` pelo preinst.

## Segurança
- **Invariantes tocadas:**
  - S1 e S8 (D47, D60): o resíduo de S1 fica registrado como RA10. O D60 não aceita
    mais grupo vindo de fonte do NSS que a instalação não lê.
  - S5/S8 na 4a: o guia não manda mais copiar um host que o usuário pode trocar.
- **Testes de ataque escritos:**
  - `dev/conteiner-install/dentro.sh`: "V9.11 nsswitch com group: files sss" e "V9.11
    nsswitch com initgroups: files ldap" (recusa, mensagem, nenhum arquivo do Ward,
    grupo recém-criado apagado).
  - `dev/conteiner-deb/dentro.sh`: "V9.11 (.deb) nsswitch com group: files sss (Fase
    51b)".
  - **Não rodei os dois alvos** (ver "Comandos" e "Problemas"). Para conferir a lógica
    sem eles:
    - rodei o `conferir_grupo` real num `ubuntu:24.04` descartável, sem rede e como
      root, com 7 casos. Aceitou `files`, `files systemd` e `files [SUCCESS=merge]
      systemd`. Recusou `files sss`, `compat`, `initgroups: files ldap` e `subid: sss`;
    - rodei 16 casos num bwrap com `/etc/group`, `gshadow`, `nsswitch.conf`, `sudoers`
      e `subid` falsos, com o mawk (o awk do host e do contêiner). Entre eles: colado,
      espaço antes do `:`, maiúscula, indentado, ação com espaço, e `groupx:` que não é
      `group:`. Os scripts estão no scratchpad e não entraram no repositório.
  - `conferir_host` do guia, extraído do Markdown, rodado contra:
    - `/usr/bin/ls`: ok;
    - `/usr/bin/lsb_release`: recusado como script;
    - `/bin/ls`: recusado, porque o `/bin` é link;
    - pasta do uid 1000: recusada;
    - `/tmp`: recusado (1777);
    - caminho com `..`, caminho relativo e caminho inexistente: recusados;
    - `name` trocado e `br.ward.nm`: recusados.

    A função passa no `bash -n` e no `zsh -n`.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Fechar o RA10 de fato:** fora da fase, porque o usuário aceitou o risco.
- **`validation-contract.md`:** sem mudança. O V9.11 não lista os casos de NSS; acrescentar
  é do planner.
- **Lint automático do `docs/instalacao.md`:** não existe, e a fase não pedia.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make origens` | 0 |
| `bash -n` em `instalacao/grupos.sh`, `dev/conteiner-install/dentro.sh`, `dev/conteiner-deb/dentro.sh` | 0 |
| `conferir_grupo` real num `ubuntu:24.04` sem rede (`docker run --rm --network none`, 7 casos) | 0 (todos como esperado) |
| `conferir_grupo` num bwrap com `/etc` falso (16 casos, mawk) | todos como esperado |
| `conferir_host` extraído do guia (11 casos) + `bash -n`/`zsh -n` | como esperado |
| `make check` | não rodado |
| `make teste-install` / `make teste-deb` | não rodados |

## Problemas descobertos
- **`make check`, `teste-install` e `teste-deb` não rodaram.**
  - O worktree não tem `target/`, e um build do workspace mais o `target/sem-modo-teste`
    custa vários GB.
  - O disco tinha 6,8 GB livres, com outro worker e um QA rodando.
  - As imagens `ward-teste/install-systemd:1` e `deb-systemd:1` não existem e
    precisariam de build com rede.
  - A fase não muda Rust: só shell e documentação.
  - O gitleaks também não rodou à mão: o isolamento do worktree barrou a chamada do
    binário por caminho absoluto. As mudanças não têm chave nem senha.
  - Antes do merge, o orchestrator precisa rodar no checkout `make check`,
    `make teste-install` e `make teste-deb`. Os 3 casos novos de NSS só foram
    exercitados fora desses alvos.
- **Glibc sem linha `group:`:** um `nsswitch.conf` sem `group:` usa o padrão do glibc,
  e o `conferir_grupo` não recusa. Nos sistemas suportados (Pop!_OS e Ubuntu) a linha
  existe.
- **Outras bases do NSS:** `netgroup` e outras bases ficam de fora. O
  `netgroup: nis` do Pop!_OS só importa para `sudoers` com `+netgroup`, e o `sudoers:`
  já exige `files`.
- **Alcance do `conferir_host`:** a função é uma foto do momento da cópia. Um app que se
  atualize e passe a ter `path` no home exige refazer a 4a, e o texto já manda refazer
  quando o manifesto muda. A função também não confere o que um binário ELF do root lê
  do home, como configuração ou plugins.
- **Escopo × código:**
  - O S1 do escopo continua dizendo "preencher só pela extensão no domínio certo". O
    item do RA10 em "Fora do modelo" registra a exceção, mas o texto do S1 em si não
    mudou. A decisão de mexer nele é do planner ou do usuário.
  - O `docs/escopo.md` › "Extensão do Chrome" não fala do RA10; deixei como está.
- **PRD D18 × Debian Policy:** a nota de `/usr/local` num `.deb` (handoff 52) continua
  valendo.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/escopo.md` | alterado |
| `docs/mission/prd.md` | alterado |
| `docs/instalacao.md` | alterado |
| `instalacao/grupos.sh` | alterado |
| `dev/conteiner-install/dentro.sh` | alterado |
| `dev/conteiner-deb/dentro.sh` | alterado |
| `docs/mission/handoffs/phase-51b.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não. Só
  `make origens`, por disco e porque a fase não muda Rust (ver "Problemas").
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (casos de NSS no `teste-install` e no
  `teste-deb`; não rodados nesses alvos, conferidos em contêiner e bwrap avulsos)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
