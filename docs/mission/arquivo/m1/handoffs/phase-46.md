# Fase 46 — Backup externo em pendrive por UUID via udisks2 (D17, D30)

## Implementado
- **Configuração (D30):** o corpo já tinha `BackupExterno { ligado, uuid }`. O padrão é
  desligado e sem UUID. **O corpo não mudou**, então não há conflito com a 22d.
- **Protocolo (`ward-core`)**, só papel app (tabela, asserção de compilação e 2ª camada
  no `atender_de`):
  - `backup_externo` (sem campo). Responde `backup_externo {ligado, uuid?}`.
  - `definir_backup_externo {ligado, uuid?}`. O UUID é conferido por
    `BackupExterno::novo`, com o formato de D45, e ligado exige UUID. A leitura e a
    escrita do pedido conferem isso. Grava pela gravação comum (geração, estado,
    backup local) e desfaz em caso de erro. Desligar não apaga nada do pendrive.
  - `pendrives` (sem campo). Responde `pendrives {itens:[{uuid, rotulo, montado}]}`,
    com até `MAX_PENDRIVES = 32` itens e rótulo de até `MAX_ROTULO = 128` bytes, sem
    caractere de controle. **Nenhum caminho de montagem vai ao cliente.**
  - Erro novo: `pendrives_indisponiveis` ("não deu para ver os pendrives (o udisks
    não respondeu)"; sem dígito, como as outras mensagens).
- **`ward-plataforma::pasta::renomear_sem_sobrescrever`:** `renameat2(RENAME_NOREPLACE)`
  relativo ao fd da pasta, para publicar sem `link(2)`, que o vfat não tem. Recusa nome
  com `/`, NUL, `.` ou `..`. O `unsafe` fica em `sys.rs`, com `// SAFETY:`.
- **`wardd::externo`** (cópia):
  - O pendrive só é conhecido com o cofre destravado, porque o UUID está na parte
    cifrada. Travado ou desligado: `Ok(None)`, sem consultar o udisks2.
  - `escolher`: exatamente um sistema de arquivos com o UUID, sem diferenciar
    maiúsculas. Dois com o mesmo UUID dão `Ambiguo`; nenhum dá `NaoConectado`.
  - A montagem é aberta com `O_NOFOLLOW|O_DIRECTORY`, e o `st_dev` do fd tem de ser o
    `DeviceNumber` que o udisks2 deu. Depois de aberta, o **udisks2 é consultado de
    novo** e tem de repetir UUID, dispositivo e montagem; se não repetir, dá `Trocado`.
    Daí em diante tudo passa por `/proc/self/fd/<n>`.
  - `<montagem>/ward-backup/` é criada se faltar e aberta com `O_NOFOLLOW`. Tem de ter o
    mesmo `st_dev` e o dono tem de ser o euid. O modo não é conferido, porque no vfat ele
    vem das opções de montagem.
  - Vão as versões do backup local (Fase 45, já cifradas) que faltam, até 30, da mais
    antiga para a mais nova. Cada uma é lida por `arquivos::ler_cofre` (arquivo comum do
    usuário, sem link, tamanho máximo) e precisa ter o cabeçalho do formato
    (`formato::ler_cabecalho`).
  - Publicação: temporário `.ward-tmp-*` com `O_EXCL|O_NOFOLLOW` e modo 0600, depois
    `fsync`, depois `renameat2(NOREPLACE)`, depois `fsync` da pasta. Se o nome existir,
    o temporário é apagado e nada muda. Temporários velhos são apagados no começo.
  - Rotação para 30 pela data do nome. Nunca apaga o que acabou de copiar, um link ou um
    arquivo sem nome de versão.
- **`wardd::udisks`:**
  - `Udisks` (`FontePendrives`): `GetManagedObjects` numa conexão nova a cada consulta,
    pelo `trava::conectar` (mesmo endereço e prazo da Fase 24).
  - `ler_objetos` aceita só objetos com `Block` e `Filesystem`, `IdUsage=filesystem` e
    `HintIgnore=false`, e ainda `HintSystem=false` ou interface `Loop` (a imagem do
    V7.4). O UUID passa pelo formato de D45, o rótulo é limpo, e a montagem tem de ser
    absoluta, sem NUL, sem `.`, `..` ou `//`, com até 4096 bytes. Fora disso, conta como
    "não montado". São lidos no máximo 4096 objetos.
  - `vigiar_udisks`: assina os sinais de `org.freedesktop.UDisks2`
    (`path_namespace=/org/freedesktop/UDisks2`) e cutuca a cópia. Reconecta com espera
    de 1 a 60 s.
  - `vigiar_copia`: espera um cutucão ou 300 s, junta a rajada (500 ms) e chama
    `Daemon::sincronizar_externo`. O log só tem textos fixos.
- **`nucleo.rs`:**
  - Campos `pendrives` (fonte), `cutucao` (`OnceLock<SyncSender>`) e `vez_externo`.
  - `com_pendrives`, `ligar_copia_externa`, `cutucar_externo` e `sincronizar_externo`.
    Este último só lê o UUID com o estado travado e copia sem segurar o mutex.
  - Destravar, criar, gravar, trocar senha ou PIN, restaurar e ligar o externo, quando
    dão certo, cutucam a cópia (`muda_o_externo`).
- **`main.rs`:** o barramento é decidido antes de criar o `Arc<Daemon>`. Com
  barramento, sobem as threads de logind, udisks2 e cópia. No daemon de teste sem
  `WARD_DEV_DBUS_SISTEMA`, nenhuma sobe, e o log continua com "logind desligado".
- `CLAUDE.md`: a linha do `dev-daemon` cita o udisks2.

## Segurança
- **Invariantes tocadas**
  - **S1:** as respostas novas não levam segredo nem caminho. As três operações são só
    do app.
  - **S2/S6:** o pendrive só recebe as versões já cifradas do backup local, byte a byte
    (Argon2id + XChaCha20-Poly1305). Nada é decifrado nem selado. O temporário é
    prefixo do cifrado.
  - **S7:** sem `link(2)` e sem sobrescrever. Uma interrupção deixa só um temporário,
    apagado na próxima vez. Uma versão já existente nunca é trocada.
  - **S3/D17:** desligado por padrão. A configuração fica na parte cifrada, e a cópia só
    acontece com o cofre destravado.
  - **S13:** o log não tem UUID, rótulo nem caminho.
  - **Entrada não confiável (D-Bus):** tudo do udisks2 é conferido. Um sinal só dispara
    uma consulta. O barramento nunca destrava, grava o cofre nem muda a configuração. A
    montagem é amarrada ao dispositivo pelo `st_dev` do fd.
- **Testes de ataque** (todos falham do lado do atacante):
  - `crates/wardd/tests/externo.rs`:
    - `v7_4_desligado_por_padrao_nao_grava_nada`: nem consulta o udisks2 e o pendrive
      fica vazio.
    - `v7_5_v7_6_copia_so_para_o_pendrive_escolhido_e_sem_segredo`:
      - B sozinho não recebe nada (`NaoConectado`); com A e B conectados, só o A recebe;
      - as cópias são iguais às locais e não há temporário;
      - a 2ª vez não muda nada, e uma gravação nova leva só ela;
      - UUID em minúsculas funciona;
      - V7.6: nenhum dos `SEGREDOS`, nem a senha ou o PIN, em claro ou em base64, e cada
        cópia abre com a senha de teste.
    - `ataque_pendrive_trocado_ou_uuid_repetido_nao_recebe_nada`: dispositivo que não
      bate dá `Trocado`; o udisks2 muda de dispositivo ou de montagem depois de aberto
      e dá `Trocado`; desconecta entre as consultas e dá `NaoConectado`; UUID repetido
      dá `Ambiguo`; sem montagem dá `NaoMontado`. Em todos os casos nada é escrito.
    - `ataque_montagem_ou_pasta_que_e_link`: montagem que é link dá `Trocado`;
      `ward-backup` que é link ou arquivo dá `PastaInsegura`. O alvo fica intacto.
    - `ataque_versao_existente_no_pendrive_nao_e_sobrescrita`: o arquivo com o nome da
      versão fica igual, o link com o nome de versão não é seguido nem trocado, o
      temporário velho some e o `LEIA-ME.txt` fica.
    - `pendrive_fica_com_30_versoes`: 35 antigas mais as novas resultam em 30, com as
      copiadas mantidas e o `notas.txt` mantido.
    - `travado_ou_desligado_nao_copia`: travado não consulta; destravar traz a
      configuração ligada; desligado não copia e não apaga.
    - `v7_8_agente_e_extensao_nao_ligam_nem_leem`: `nao_autorizado` pelo protocolo e
      direto no `Daemon::atender`. O cofre fica igual byte a byte, sem consulta.
    - `pendrives_sem_udisks_e_uuid_invalido`.
    - `udisks_falso_e_lido_com_as_conferencias`: num `dbus-daemon` privado com um
      udisks2 falso feito com o `zbus`, o disco de sistema é filtrado, o loop é aceito,
      o UUID `../etc` é filtrado, o rótulo é limpo e a montagem com `..` vira "não
      montado". Barramento sem udisks2 dá `None`.
    - `v7_5_sinal_do_udisks_copia_para_o_pendrive_montado`: threads reais. Com A
      desmontado, nada acontece; o sinal de montagem leva a cópia ao A e não ao B; uma
      gravação nova também chega.
    - `externo_nao_usa_link`: varre a fonte atrás de `hard_link`, `linkat`, `libc::link`
      e `fs::symlink(`.
  - `crates/ward-core/src/protocolo/testes.rs`: `v7_8_backup_externo_so_pelo_app`,
    `ataque_definir_backup_externo_fora_do_formato` e as tabelas V3.15 atualizadas.
  - `crates/ward-plataforma/src/pasta.rs`: `renomeia_e_nunca_sobrescreve` (inclusive
    link no destino) e `ataque_nome_que_sai_da_pasta_recusado`. Os dois cobrem o
    `unsafe` novo.
  - `crates/wardd/src/udisks.rs`: `ataque_montagem_estranha_conta_como_nao_montado` e
    `rotulo_sem_controle_e_cortado`.
- **Dependências novas:** nenhuma. São usados o `renameat2` e o `RENAME_NOREPLACE` da
  `libc` que já existia no `ward-plataforma`.

## Ficou de fora
- **Montar o pendrive:** o Ward nunca chama `Filesystem.Mount`. Ele copia quando o
  pendrive aparece montado, pela área de trabalho ou pelo usuário. Sem automontagem,
  nada é copiado até alguém montar (ver Problemas).
- **Teste com vfat real montado:** não montei nada no host, porque `udisksctl
  loop-setup` toca o udisks2 e o `/media` reais. Também não montei vfat em contêiner,
  porque o loop é do kernel do host e o udisks2 do host o veria. O vfat ficou coberto
  pelo desenho (sem `link`, só `renameat2(NOREPLACE)`, que o vfat suporta) e pela
  varredura da fonte. O V7.4 e o V7.5 de verdade ficam com o validador do M7, como diz
  o contrato: imagem vfat em `.dev/`, `udisksctl loop-setup` e `mount`, depois
  `make dev-daemon` com `WARD_DEV_DBUS_SISTEMA=/run/dbus/system_bus_socket`, destravar e
  `definir_backup_externo`.
- **UI** (Fase 48): ela usa `pendrives`, `backup_externo` e `definir_backup_externo`.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` | 0 |
| `cargo test -p ward-core --lib protocolo` | 0 |
| `cargo test -p ward-plataforma --lib pasta` | 0 |
| `cargo test -p wardd --all-features` (1ª: `montagem_valida` aceitava `/./`; 2ª: varredura achou `symlink_metadata`) | 101, 101, depois 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 |
| `make check` (1ª: `erros_sem_numero…` acusou o "2" de "udisks2" na mensagem; corrigido) | 2, depois 0 |

## Problemas descobertos
- **Cópia só com o cofre destravado.** O UUID está na parte cifrada (D30). Um pendrive
  conectado com o cofre travado só recebe a cópia no próximo destravar, se ainda
  estiver montado. Isso é coerente com D30, mas difere da leitura literal de "copia
  sempre que conectado".
- **Sem automontagem, sem cópia.** O Ward não monta. Montar por `Filesystem.Mount`
  passaria pelo polkit do udisks2 para um serviço do sistema com `User=`, o que não
  foi testado. O planner decide se monta.
- **Pasta no pendrive sem conferência de modo.** No vfat, o modo vem da montagem
  (`umask`). Se ela for 0777 e `/media/<usuário>` for atravessável por outros, outro
  usuário da máquina pode plantar um nome de versão e impedir aquela cópia (negação de
  serviço) ou apagar as cópias. Ele não lê nada em claro.
- **Sistemas de arquivos sem `st_dev` do bloco** (btrfs, por exemplo, usa dispositivo
  anônimo) dão `Trocado` e nunca recebem cópia. Pendrive normal (vfat, exFAT, ext4,
  NTFS) não tem esse problema.
- **`HintSystem` do loop:** aceitei a interface `Loop` mesmo com `HintSystem` verdadeiro,
  para o V7.4 funcionar com `udisksctl loop-setup`. A consequência é que um loop do
  sistema (snap, por exemplo, que é squashfs com UUID) aparece em `pendrives`. Nenhum
  vai receber cópia sem ser escolhido pelo UUID, e o snap é só leitura.
- **UUID repetido** (pendrive clonado com `dd`) bloqueia a cópia com aviso no log.
- **Threads de teste:** `v7_5_sinal_do_udisks…` deixa as threads do vigia vivas até o
  fim do binário de teste. Depois que o barramento morre, elas não escrevem nada.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/erros.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-plataforma/src/pasta.rs` | criado |
| `crates/ward-plataforma/src/sys.rs` | alterado (`renameat_sem_sobrescrever`) |
| `crates/ward-plataforma/src/lib.rs` | alterado |
| `crates/wardd/src/externo.rs` | criado |
| `crates/wardd/src/udisks.rs` | criado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/trava.rs` | alterado (`conectar` vira `pub(crate)`) |
| `crates/wardd/src/main.rs` | alterado |
| `crates/wardd/src/lib.rs` | alterado |
| `crates/wardd/tests/externo.rs` | criado |
| `CLAUDE.md` | alterado (linha do `dev-daemon`) |
| `docs/mission/handoffs/phase-46.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados (nenhum dispositivo montado, nenhum barramento real)
- [x] Teste de ataque para cada (SEG) tocado (V7.6, V7.8, pendrive trocado, links, sem sobrescrever, desligado por padrão)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica (fase sem tela)
- [x] Ficou dentro do escopo da fase
