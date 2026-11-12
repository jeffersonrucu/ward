# Fase 47 — `install/`, comentários do Makefile, `deny.toml`, gitleaks, toolchain e `Cargo.toml` raiz (Missão 2, trilha S)

## Implementado

Troca 1:1 pelo `docs/glossario.md`, sem mudar comportamento. Parte de `master` 773df90
(o worktree estava em b4d5e1b; `git reset --hard 773df90` com a árvore limpa).
Identificadores e comentários em inglês; as mensagens exibidas (`ward-instalar: …`,
`ward (preinst): …`, `ward-deb: …`, `ward-extensao: …`, `==> …`, recusas de D60, V9.2 e
das políticas do Chrome) ficam em pt-BR, iguais (m2:DT20), salvo onde citam um nome que
mudou (lista em "Literais").

- **`install/install.sh`**: `AQUI` → `HERE`, `RAIZ` → `REPO_ROOT` (o script já tem
  `ROOT`, a marca de uid 0), `ARVORE` → `TREE`, `RAIZ_ALVO` → `TARGET_ROOT`, `ACAO` →
  `ACTION`, `SISTEMA` → `SYSTEM`, `TIPOS`/`MODOS`/`DONOS`/`CAMINHOS`/`ORIGENS` →
  `KINDS`/`MODES`/`OWNERS`/`PATHS`/`ORIGINS`, `GRUPOS` → `OWNER_GROUPS` (não `GROUPS`:
  o bash ignora atribuição ao array especial `GROUPS`, e a coluna de grupo da árvore
  sumiria); funções `erro`/`aviso`/`passo` → `error`/`warning`/`step`,
  `conferir_sem_modo_teste` → `verify_no_test_mode`, `conferir_origem` →
  `verify_source`, `copiar_para` → `copy_to`, `conferir_elf` → `verify_elf`,
  `garantir_pasta` → `ensure_dir`, `instalar_arquivo` → `install_file`, `instalar` →
  `do_install`, `desinstalar` → `do_uninstall`; locais (`arq` → `file`, `achado` →
  `found`, `marca` → `mark`, `tam` → `size`, `de`/`para` → `from`/`to`, `lido` →
  `parsed`, `id_crx` → `crx_id`, `versao` → `version`, `par` → `pair`, `unidades` →
  `units`, `tipo modo dono grupo caminho origem resto` → `kind mode owner group path
  origin rest`).
  - **Subcomandos** `instalar`/`desinstalar` → `install`/`uninstall` (os chamadores são só
    o `Makefile` e o `install/deb.sh`, mudados junto). Como uma função `install` taparia
    o `/usr/bin/install`, as funções são `do_install`/`do_uninstall` e o despacho, que
    era `"$ACAO"` (nome da função = subcomando), virou um `case` explícito com os dois
    braços; o `case` da validação logo no começo continua recusando qualquer outro valor
    antes de qualquer conferência. Única mudança de forma do script.
- **`install/groups.sh`** (D60): `MARCA_GRUPOS` → `GROUPS_MARK`, `grupos_marcados` →
  `marked_groups`, `pasta_da_marca` → `mark_dir`, `gravar_marca` → `write_mark`,
  `marcar_grupo` → `mark_group`; locais (`nome`, `linhas`, `senha`, `membros`, `fonte` →
  `database`, `aceitas` → `accepted`, `alvo` → `target`, `dono inicio qtd` → `owner start
  count`, `criado` → `created`, `saida` → `output`, `antes` → `before`, `conteudo` →
  `content`, e no awk `achou`/`linha` → `found`/`line`). Usa `error`/`step` de quem inclui.
- **`install/chrome.sh`**: `POLITICAS_DO_WARD` → `WARD_POLICIES`; locais (`pasta` → `dir`,
  `achado` → `found`, `nomes` → `names`).
- **`install/dump.sh`**: `pasta_webkit` → `webkit_dir`, `AUXILIARES_WEBKIT` →
  `WEBKIT_HELPERS`, `lista_apport` → `apport_list`; locais (`arq` → `arch`, `pacote` →
  `package`, `conhecido` → `known`). Usa `error`/`warning` de quem inclui.
- **`install/deb.sh`**: `AQUI` → `HERE`, `RAIZ` → `ROOT`, `VERSAO` → `VERSION`, `ARQ` →
  `ARCH`, `PACOTE` → `PACKAGE`, `SAIDA` → `OUTPUT`, `TAM` → `SIZE`, `erro`/`passo` →
  `error`/`step`, `esperado`/`lista`/`real`/`ruim` → `expected`/`listing`/`actual`/`bad`;
  o Python embutido (`modelos`, `grupos`, `despejo`, `arvore`, `saida`, `pastas`,
  `linha`, `campos`, `trocas`, `texto`, `marca`, `conteudo` → `templates`, `groups`,
  `dump`, `tree`, `out_dir`, `dirs`, `line`, `fields`, `replacements`, `text`, `marker`,
  `content`); marcas de modelo `#@GRUPOS@`/`#@DESPEJO@`/`#@PASTAS@` →
  `#@GROUPS@`/`#@DUMP@`/`#@DIRS@` (`#@CHROME@` fica), nos quatro modelos de `install/deb/`.
- **`install/deb/*`**: `erro`/`aviso`/`passo` → `error`/`warning`/`step`; preinst
  `PASTAS` → `DIRS`, `conferir_pastas` → `verify_dirs`, `override` (fica) com `grupo
  caminho atual` → `group path current`; postinst `conferir_setgid` → `verify_setgid`,
  `uid_do_cofre` → `vault_uid`, `usuarios_humanos` → `human_users`, `ligar_socket` →
  `enable_socket`, `ligados` → `enabled`; postrm `tirar_grupos` → `remove_groups`,
  `tirar_overrides` → `remove_overrides`; prerm `unidades` → `units`. As linhas que o
  `dev/tests/deb.sh` procura com recuo exato (`^        conferir_politicas_chrome
  /etc/opt/chrome/policies/managed$`, `^        avisar_core_pattern$`, `^    usr/local/bin$`
  gerada pelo `#@DIRS@`) ficaram iguais.
- **`install/crx.py`**: docstring e nomes (`MAX_PACOTE`/`MAX_CABECALHO`/`MAX_MANIFESTO` →
  `MAX_PACKAGE`/`MAX_HEADER`/`MAX_MANIFEST`, `VERSAO` → `VERSION_RE`, `Recusa` →
  `Refusal`, `campos` → `fields`, `ler` → `read_package`, `chave`, `numero`, `tipo` →
  `kind`, `valor`, `dados`, `cabecalho`, `chaves_rsa`, `assinado`, `manifesto`,
  `versao_ext` → `ext_version`). Nenhum outro arquivo importa o módulo; a saída
  (`<id> <versão>`, `crx: …`, `uso: …`) é a mesma.
- **`install/pack-extension.sh`**: `AQUI` → `HERE`, `RAIZ` → `ROOT`, `erro` → `error`,
  `CHAVE` → `KEY`, `SAIDA` → `OUTPUT`, `ESPERADO` → `EXPECTED`, `lido` → `parsed`,
  `versao` → `version`. `EXTENSAO_DIST` e `CRX_SAIDA` ficam (ver "Exceções").
- **`install/tree.txt`**: comentários; origens `ger:` → `gen:` e `ger:politica` →
  `gen:policy` (só o `install.sh` lê a 6ª coluna; os testes leem as colunas 1 e 5). Os
  caminhos instalados não mudaram (congelados até a Fase 57).
- **Units, `tmpfiles.d`, `coredump.conf.d`**: só comentários. `Description=` das units
  (texto do `systemctl status`) fica em pt-BR.
- **`Makefile`**: todos os comentários (`#` e `##`) em inglês, inclusive o `@#` da receita
  do `dev-vault`; os blocos das Fases 2 (`compile`, `english-check`) já estavam.
  Conferido: fora das linhas de comentário, o único diff é `install/install.sh
  install|uninstall`. Os `echo` (`==> 0/6 origens …`, `==> 1/6`, `==> 4/6 cargo audit`,
  `==> make check: tudo limpo`, `não é o gitleaks`, `node não encontrado …`) ficaram
  iguais, então o `grep` do `dev/tests/attacks-foundation.sh` não muda. A varredura de
  V1.5 (padrões do `attacks-foundation.sh`) não acha nada no `Makefile` nem nos
  `dev/gitleaks-*.toml`.
- **`Cargo.toml` raiz, `deny.toml`, `app/src-tauri/deny.toml`**: só comentários. Os
  `reason` das exceções do cargo-deny e os títulos/descrições dos TOML do gitleaks são
  literais e ficam. O `rust-toolchain.toml` não tem comentário: sem mudança.
- **`.gitleaks.toml`, `dev/gitleaks-tree.toml`, `dev/gitleaks-extra.toml`,
  `dev/gitleaks-extra-tree.toml`**: só comentários; a allowlist de D15 e as regras ficam
  byte a byte iguais, exceto o id da regra `ward-chave-privada` → `ward-private-key`
  (nome; nada no repositório o cita).
- **`dev/tests/install.sh`** (Fase 44, nenhuma fase em curso nele): o harness que inclui
  o `dump.sh` define `error`/`warning` em vez de `erro`/`aviso` e chama `apport_list`. As
  saídas `ERRO:`/`AVISO:` que o teste procura não mudaram.

### Literais alterados (PRD regra 5)

| Arquivo | Antes | Depois | Motivo |
| --- | --- | --- | --- |
| `install/install.sh` | `"uso: install.sh instalar\|desinstalar"` (2×) | `"uso: install.sh install\|uninstall"` | cita o subcomando renomeado |
| `install/install.sh` | `ger-nm`, `ger-politica`, `ger-update`, `ger-apport`, `ger:*` | `gen-nm`, `gen-policy`, `gen-update`, `gen-apport`, `gen:*` | nomes dos gerados em `$TMP` e origem da árvore |
| `install/tree.txt` | `ger:update`, `ger:nm`, `ger:politica`, `ger:apport` | `gen:update`, `gen:nm`, `gen:policy`, `gen:apport` | origem lida pelo `install.sh` |
| `install/dump.sh` | `(AUXILIARES_WEBKIT em install/dump.sh)` | `(WEBKIT_HELPERS em install/dump.sh)` | mensagem cita a variável renomeada |
| `install/deb.sh` | `install.sh instalar`; `$DEB_DIR/raiz`, `$DEB_DIR/pacote`; `#@GRUPOS@`, `#@DESPEJO@`, `#@PASTAS@` | `install.sh install`; `$DEB_DIR/root`, `$DEB_DIR/package`; `#@GROUPS@`, `#@DUMP@`, `#@DIRS@` | subcomando; pastas de trabalho em `target/deb/` (nenhum teste as cita); marcas de modelo (só o `deb.sh` as troca; o teste procura `#@` genérico) |
| `install/deb/{preinst,postinst,postrm}` | `#@GRUPOS@`, `#@DESPEJO@`, `#@PASTAS@` | `#@GROUPS@`, `#@DUMP@`, `#@DIRS@` | marcas de modelo |
| `Makefile` | `install/install.sh instalar`/`desinstalar` | `install/install.sh install`/`uninstall` | subcomando |
| `dev/gitleaks-extra.toml` | `id = "ward-chave-privada"` | `id = "ward-private-key"` | id da regra |
| `dev/tests/install.sh` | `lista_apport` nos três `eval` | `apport_list` | nome da função |

Ficaram de propósito: os prefixos de mensagem `ward-instalar:`/`ward-extensao:`, os
temporários `.ward-instalar.XXXXXXXX`, `/tmp/ward-instalar.XXXXXXXX`, `.grupos.XXXXXXXX`
e `target/extension-crx.XXXXXX`, a pasta `perfil` do Chrome temporário, o `sed` que lê
`pub const ID_EXTENSAO` do `ward-nm` (nome da trilha R) e as mensagens que citam
`ID_EXTENSAO` (o `dev/tests/deb.sh` procura `não do ID_EXTENSAO`).

### Exceções

`dev/english-check/frozen.txt` (novas):
- `install/groups.sh conferir_grupo`, `install/chrome.sh conferir_politicas_chrome`,
  `install/chrome.sh avisar_manifestos_de_usuario` (Fase 57): o `dev/tests/deb.sh`
  (Fase 45, em curso noutro worktree) procura `^garantir_grupo()`, `^conferir_grupo()`,
  `^conferir_politicas_chrome()`, `^apagar_grupos_marcados()`,
  `^avisar_manifestos_de_usuario()` e `^avisar_core_pattern()` nos scripts do mantenedor
  montados. As seis funções ficam com o nome até a Fase 57 (que mexe em `install/deb/*`
  e no `dev/tests/deb.sh`). Só três delas são acusadas pelo checador (as outras não têm
  palavra da lista), por isso só três entradas.
- `install/deb/* grupos`, `install/deb/* criados` (Fase 57): o caminho instalado
  `/usr/local/lib/ward/grupos-criados` citado nos comentários do preinst/postrm (DT15).
- `install/pack-extension.sh extensao`, `install/pack-extension.sh saida` (Fase 56):
  `EXTENSAO_DIST` e `CRX_SAIDA`, que o `dev/tests/deb.sh` passa ao script.
- `Makefile extensao` (Fase 56): `WARD_DEV_EXTENSAO=chrome` citado nos comentários.
- `Makefile teste`, `Makefile selecao` (Fase 56): as pastas de socket de teste
  `ward-teste`, `ward-teste-e2e`, `ward-teste-selecao` citadas nos comentários.

`dev/english-check/allow.txt` (novas, permanentes):
- `Makefile frei` (frei0r, nome de plugin do GStreamer).
- `Makefile instalacao`, `Makefile migracao`, `install/* instalacao`: nomes de arquivo da
  documentação em pt-BR (`docs/instalacao.md`, `docs/migracao.md`; `docs/` fica em
  pt-BR pela regra 1 do PRD).

## Segurança

- **Invariantes tocadas**: S2/S8 (`dump.sh`, `coredump.conf.d`, units: `LimitCORE=0`,
  `LimitMEMLOCK`, sem `NoNewPrivileges`), S5 (units e `tmpfiles.d`: `/run/ward` do root),
  S14 (`deny.toml`, `app/src-tauri/deny.toml`), S15 (TOML do gitleaks), V9.2 (marca de
  `test-mode` no `install.sh`), D60 (`groups.sh`), Fase 51a (políticas do Chrome).
  **Nenhuma lógica mudou.** Conferência por token (script em
  `<scratchpad>/fase47/tokdiff.py`: `git show HEAD:<arq>` contra a árvore, sem linhas de
  comentário, alinhado por `difflib`): em todos os arquivos de `install/` o diff é só de
  pares nome antigo → nome novo e dos literais da tabela acima; a única mudança de forma
  é o despacho final do `install.sh` por `case` (ver "Implementado"). As regras de D60
  (gid, membros, gshadow, NSS, subgid, pam_group, sudoers, nsswitch), a recusa de
  `WARD_DEV_*`/`ward-ponte-teste`/`wardd-app-teste`/`ward-teste`, o id `^[a-p]{32}$`, a
  leitura do CRX3, a recusa de link/ELF/tamanho, a gravação por temporário + `rename` e
  os modos/donos da árvore estão iguais. O `GROUPS` do bash foi evitado de propósito (ver
  acima): com ele a coluna de grupo da árvore viraria lixo e o `ward-app`/`ward-nm`
  sairia sem o grupo certo.
- **Allowlist do gitleaks**: nenhum padrão novo; os valores de D15 estão iguais, um por
  linha, casados inteiros.
- **Testes de ataque escritos**: nenhum novo (a fase só renomeia). Os existentes que
  tocam estes arquivos (`make test-install`, `test-deb`, `test-chrome-policy`,
  `test-units`, `test-attacks-foundation`) rodam no M6 (PRD regra 6).
- **Dependências novas**: nenhuma.

## Ficou de fora

- Renomear as seis funções procuradas pelo `dev/tests/deb.sh` (Fase 45 em curso): Fase
  57, junto com o teste (ver "Exceções").
- `EXTENSAO_DIST`/`CRX_SAIDA` e os nomes de pasta de socket de teste: Fase 56.
- Caminhos instalados (`/usr/local/share/ward/extensao`, `grupos-criados`): Fase 57.
- Prefixos de mensagem `ward-instalar:`/`ward-extensao:`: texto exibido (DT20).
- Smoke do `install.sh install` com `DESTDIR` e do `deb.sh`: não rodei (pedido do
  orchestrator: sem suíte e sem install). Só `bash -n`, `py_compile` (no `make compile`)
  e a conferência por token.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `git reset --hard 773df90` | 0 |
| `make english-check PATHS="install Makefile deny.toml app/src-tauri/deny.toml rust-toolchain.toml Cargo.toml .gitleaks.toml dev/gitleaks-*.toml dev/english-check/frozen.txt dev/english-check/allow.txt"` (log `<scratchpad>/fase47/fase47-ec3.log`) | 0 (0 achados, 0 problemas nas exceções; antes: 2839 achados) |
| `python3 -I -B dev/english_check.py --self-test` | 0 (91/91) |
| `make compile` (log `<scratchpad>/fase47/fase47-make-compile.log`) | 0 (origens, `cargo check` do workspace, Vite, `cargo check` do `app/src-tauri`, sintaxe de 48 shell, 35 Python e 7 JS) |
| `bash -n` de `install/*.sh`, `install/deb/*`, `dev/tests/install.sh`; `make -n install`, `make -n deb`, `make -n check` | 0 |
| conferência por token (`tokdiff.py`) e diff do `Makefile` sem comentários contra o original | só nomes/literais listados |
| varredura V1.5 (padrões do `attacks-foundation.sh`) no `Makefile` e em `dev/` | só achados em `dev/fixtures/legacy/` (ver "Problemas") |
| `make check` | não rodado (PRD regra 6) |

## Problemas descobertos

1. **V1.5 vai falhar no M6 por causa da linha de base (Fase 3)**: o `v1_5` do
   `dev/tests/attacks-foundation.sh` varre todo `dev/` com os padrões de pasta proibida e
   acha três linhas em `dev/fixtures/legacy/` (`strings-0.1.4.txt`: o caminho do cofre
   real e o nome da variável do agente SSH; `help-0.1.4.txt`: a pasta do ssh no texto do
   `ward --help` 0.1.4). São literais da linha de base, não acesso a pasta; a varredura
   precisa excluir `dev/fixtures/legacy/` (ou a Fase 61 decide). Não mexi (fora do
   escopo; `attacks-foundation.sh` é da Fase 41, já feita).
2. **Seis funções congeladas por um teste de outra trilha**: o `dev/tests/deb.sh` procura
   nomes de função pt-BR nos scripts do mantenedor. A Fase 57 precisa trocar as seis
   (`garantir_grupo`, `conferir_grupo`, `conferir_politicas_chrome`,
   `apagar_grupos_marcados`, `avisar_manifestos_de_usuario`, `avisar_core_pattern`) nos
   dois lados e tirar as três entradas do `frozen.txt`.
3. **Conflito provável no merge**: `dev/english-check/frozen.txt` e `allow.txt` ganharam
   linhas no fim, como as outras fases em paralelo; a junção é só concatenar.
4. **Documentação**: `docs/` ainda cita `instalacao/instalar.sh`/subcomandos antigos em
   handoffs arquivados; a Fase 59 cuida do que está em `docs/` fora do arquivo. O
   `CLAUDE.md` não cita os subcomandos do `install.sh`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `install/install.sh` | alterado |
| `install/groups.sh` | alterado |
| `install/chrome.sh` | alterado |
| `install/dump.sh` | alterado |
| `install/crx.py` | alterado |
| `install/deb.sh` | alterado |
| `install/pack-extension.sh` | alterado |
| `install/deb/preinst`, `postinst`, `postrm`, `prerm` | alterado |
| `install/systemd/wardd@.service`, `wardd@.socket` | alterado |
| `install/tmpfiles.d/ward.conf`, `install/coredump.conf.d/ward.conf` | alterado |
| `install/tree.txt` | alterado |
| `Makefile` | alterado |
| `Cargo.toml`, `deny.toml`, `app/src-tauri/deny.toml` | alterado |
| `.gitleaks.toml`, `dev/gitleaks-tree.toml`, `dev/gitleaks-extra.toml`, `dev/gitleaks-extra-tree.toml` | alterado |
| `dev/tests/install.sh` | alterado |
| `dev/english-check/frozen.txt`, `dev/english-check/allow.txt` | alterado |
| `docs/mission/handoffs/phase-47.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks) — não: PRD regra 6
  (só `make compile` e `english-check`, ambos 0)
- [x] Só cofre de teste e credenciais falsas usados (nenhum usado)
- [ ] Teste de ataque para cada (SEG) tocado — nenhum novo; a fase só renomeia, os
  existentes rodam no M6
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (exceção: `dev/tests/install.sh`, 5 linhas, para
  acompanhar os nomes do `dump.sh`)
