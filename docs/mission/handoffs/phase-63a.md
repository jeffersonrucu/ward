# Fase 63a — Correção da Bateria C

## Implementado

- **Base**: o worktree nasceu em `b4d5e1b`. Com a árvore limpa, rodei `git reset --hard
  3401bed` (master atual: "fases 62a e 63 registradas; fase 63a de correção").
- **Defeito do produto (m2:DT16, Problema 3b da 63)**: um `dpkg -i` recusado no preinst
  reiniciava o `wardd` e travava o cofre. O dpkg roda o `postrm abort-upgrade` da versão
  nova e o `postinst abort-upgrade` da instalada, e os dois chamavam `start_enabled_sockets`,
  que faz `try-restart` em todo `wardd@*.service` ativo. Correção em `install/upgrade.sh`:
  - marca `/run/ward-upgrade-stopped`: pasta do root, 0700, com o arquivo `sockets`
    listando os sockets que o preinst parou;
  - é criada só depois do stop, por `mkdir`, que não segue link e falha se o caminho já
    existe; o arquivo dentro dela é gravado com `set -C`;
  - `stop_running_daemons mark` (só o preinst passa `mark`) grava a marca. Se nada foi
    parado, não grava;
  - `restart_stopped_sockets` (novo, usado no `abort-upgrade` do postrm e do postinst):
    lê a marca só se ela for pasta (não link), do root, 0700, com arquivo regular dentro,
    e cada linha tem de casar `^wardd@[0-9]+\.socket$`. Dá `systemctl start` só nesses
    sockets, nunca `try-restart`, e apaga a marca. Sem marca, não toca em nada;
  - o preinst apaga a marca velha (`clear_stop_mark`, `rm -rf` no caminho: um link some,
    o alvo fica) antes de todas as conferências. Assim um preinst recusado nunca deixa
    marca;
  - `start_enabled_sockets` (configure e `make install`, depois da troca) continua com o
    `try-restart` e apaga a marca.
- **Mensagem de parada (Problema 2c)**: `stop_running_daemons` agora avisa "parando
  wardd@N.service antes de trocar os binários…" para cada serviço ativo **antes** de parar
  os sockets (o `Requires=` derruba o serviço junto com o socket, por isso o aviso não saía
  antes). Isso vale para o `make install` e para o preinst do `.deb`, que antes não
  avisava nada. A expectativa dos testes da Fase 57 ficou como estava.
- **`make install`**: não tem o mesmo caminho. Toda recusa vem antes do stop, e um erro
  depois do stop sai sem religar nada. Mesmo assim, escrevi o teste de ataque (abaixo).
- **Testes (Problemas 1, 2a, 2b e 3a da 63)**:
  - `dev/container-units/inside.sh`: `estado`/`lista` trocados por `status`/`list`.
  - Controle do vigia nas duas suítes:
    - roda de `/var/tmp` (o `/tmp` do contêiner é `noexec`), numa cópia setgid
      (`root:nogroup` 2755) executada pelo uid 1000. O processo fica não-dumpable e de
      outro uid, como o `wardd`;
    - antes de vigiar, confere que `/proc/<pid>` é do usuário e o `environ` é do root;
    - chama o `setpriv` direto, porque com a função `as_1000` o `$!` seria um subshell do
      root e o vigia nunca veria nada.
  - `dev/tests/install.sh`: `--cap-add SYS_PTRACE` no contêiner. Sem ele, o vigia de
    `/proc/<pid>/exe` passava sem ver nada.
  - `ward unlock` no pty: `pty_unlock` (install) e `unlock_with_cli` (deb):
    - o pty lê de um FIFO, e a senha falsa só é digitada depois que aparece o prompt
      "Senha mestra" (a CLI desliga o eco antes de escrever o prompt). Sem prompt em 10 s,
      nada é digitado;
    - o teste `no_password` confere que a senha não aparece na saída;
    - o V9.3 (servidor falso) usa o mesmo helper. Antes, a senha ia para o pty mesmo
      quando a CLI recusava, e o eco a punha em `$r`.
  - `dev/tests/deb.sh`: as conferências estáticas passam a exigir `stop_running_daemons
    mark`, `restart_stopped_sockets` e `clear_stop_mark`, e nenhum `start_enabled_sockets`
    nem `try-restart` nos blocos `abort-upgrade`.

## Segurança

- **Invariantes tocadas**:
  - S8 / m2:DT16: uma instalação recusada não trava o cofre nem troca o daemon. Uma
    atualização que falha depois do stop religa só o socket parado (falha fechada: o
    cofre já travou).
  - S5: a marca vive em `/run`, onde só o root escreve, e não é seguida por link. Se
    houver um link no lugar dela, o preinst recusa depois do stop (os sockets ficam
    parados).
  - S13: a senha falsa não volta mais para a saída do pty.
- **Testes de ataque escritos**:
  - `dev/container-deb/inside.sh` (Fase 57, ampliado):
    - "m2:DT16 ataque (Fase 63a): dpkg -i recusado: mesmo wardd no ar (pid …)";
    - "… o cofre continua destravado" (`list` responde `list` antes e depois);
    - "… o preinst recusado não parou nada";
    - "… nenhuma marca de parada";
    - "… a versão instalada continua configurada";
    - marca plantada por link antes de uma recusa: "o link da marca foi apagado", "o alvo
      do link da marca intacto", "marca por link: mesmo wardd no ar" e "o cofre continua
      destravado".
  - `dev/container-deb/inside.sh`, controle positivo da falha depois do stop:
    - conflito de arquivo com o pacote `fase63a-conflito`, achado no desempacotamento;
    - o preinst parou o wardd, o abort religou o socket (`active` exato), nenhum wardd
      reiniciado (`inactive` exato), a marca sumiu, o pacote continua `ii`, a próxima
      conexão sobe o daemon e o cofre está travado.
  - `dev/container-install/inside.sh`: "m2:DT16 ataque (Fase 63a): make install
    recusado: mesmo wardd no ar (pid …)", "… o cofre continua destravado", "… recusado
    antes de parar" e "… o alvo do link intacto".
  - `dev/tests/install.sh` (no host, com systemctl falso):
    - sem marca, o abort não religa nem reinicia nada;
    - marca por link não é seguida (o link some e o alvo fica);
    - link no lugar da marca recusa;
    - marca 0755 não é lida;
    - da marca só sai `wardd@<uid>.socket` (`.service`, `x`, `../`, `; reboot` são
      ignorados);
    - stop que falha não deixa marca.
  - Os mesmos três testes, mais `no_password` (pty), "o processo vigiado é do usuário e
    não-dumpable" e "o vigia vê um binário trocado".
- **Prova do teste de ataque**: rodei o `make test-deb` com `install/upgrade.sh` e
  `install/deb/*` voltados ao master (`fase63a-test-deb-sem-correcao.log`):
  - "dpkg -i recusado: o wardd mudou (4343 -> 4457)" e "o cofre continua destravado
    (… veio 'resposta: erro locked')", além das conferências estáticas;
  - depois, os arquivos corrigidos foram copiados de volta.
- **Dependências novas**: nenhuma.
- **Evidência sem segredo**: `grep -F -f .dev/variations.txt` e a senha mestra de teste
  não aparecem em `docs/mission/evidence/M6/fase-63a/`. Caminhos do home e do worktree
  trocados por `<home>` e `<repo>`.
- **Root e cofre**: nada de `sudo` nem `make install` no host; root só nos contêineres.
  Não toquei no cofre real, no daemon instalado nem em `/usr/local`. Os serviços
  `ward-teste` do compose e os `studiovisual-forja-*` não foram tocados.

## Ficou de fora

- `make install` com erro depois do stop (ex.: falha ao copiar um arquivo) deixa os
  sockets parados. É falha fechada e não é o caminho da recusa. Não mudei.
- O `make check` inteiro não foi rodado (o pedido listou as suítes, o `compile` e o
  `english-check`).

## Comandos executados

Logs em `docs/mission/evidence/M6/fase-63a/` (`fase63a-*.log`). Suítes sob `systemd-run
--user --scope -p TasksMax=2000`. `df -h`: 18 GB livres antes, 17 GB no fim.

| Comando | Exit code |
| --- | --- |
| cópia de `.dev/baseline/` e `.dev/tools/` do checkout principal; `sha256sum -c SHA256SUMS` | 0 |
| `make dev-vault && make dev-variations` (284 padrões) | 0 |
| `make test-units` (rodada final) | 0 (31 ok) |
| `make test-install` (rodada final) | 0 (437 ok) |
| `make test-deb` (rodada final) | 0 (273 ok) |
| `make test-deb` com o produto do master (prova do ataque) | 2 (esperado) |
| `make compile` | 0 |
| `make english-check` | 0 |
| rodadas intermediárias: o controle do vigia falhou (`$!` era o subshell da função), e o controle da falha depois do stop também (o diretório no lugar do arquivo não fez o dpkg falhar; o bind mount só-leitura impediu a limpeza do dpkg e os `abort-*` não rodaram), daí o conflito de pacote | 2, corrigidos |

## Problemas descobertos

1. `check` com "active" casa também "inactive" (é por substring). Os testes antigos da
   Fase 57 ("m2:V5.7: o socket religou") têm essa fraqueza. Nos testes novos usei
   `equal`. Os antigos ficaram como estavam, fora do escopo.
2. Com o produto antigo, o controle da falha depois do stop também falhou: "a próxima
   conexão… veio 'bytes: 0'". Não investiguei a causa (provavelmente o `start`/`try-restart`
   duplo dos dois `abort-upgrade` com a marca antiga). Com a correção, passa.
3. A versão 0.1.4 não faz nada no `abort-upgrade` (conferido no `.deb` da linha de base).
   Uma 0.2.0 recusada por cima dela não religa nem reinicia nada pela versão antiga.

## Arquivos criados/alterados

| Arquivo | Ação |
| --- | --- |
| `install/upgrade.sh` | alterado (marca de parada, `restart_stopped_sockets`, aviso antes do stop) |
| `install/deb/preinst` | alterado (`clear_stop_mark`, `stop_running_daemons mark`) |
| `install/deb/postinst` | alterado (`abort-upgrade` → `restart_stopped_sockets`) |
| `install/deb/postrm` | alterado (`abort-upgrade` → `restart_stopped_sockets`) |
| `dev/container-units/inside.sh` | alterado (`status`/`list`) |
| `dev/container-install/inside.sh` | alterado (pty sem eco, controle do vigia, ataque do `make install` recusado) |
| `dev/container-deb/inside.sh` | alterado (pty sem eco, controle do vigia, ataques da recusa e da marca por link, controle da falha depois do stop) |
| `dev/tests/install.sh` | alterado (`SYS_PTRACE`, testes da marca no host) |
| `dev/tests/deb.sh` | alterado (conferências estáticas do `abort-upgrade`) |
| `docs/mission/evidence/M6/fase-63a/*` | criados |
| `docs/mission/handoffs/phase-63a.md` | criado |

## Procedimentos
- [ ] `make check` rodado: não. Rodei `make compile`, `make english-check` e as três suítes
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado, com prova de que falha sem a correção
- [ ] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase:
  - nenhum contêiner meu de pé;
  - as imagens `ward-teste/*:1` ficam;
  - `target/`, `app/src-tauri/target` e `.dev/` do worktree apagados no fim.
