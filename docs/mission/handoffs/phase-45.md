# Fase 45 — Backup local automático e restaurar no daemon (D17, D27, D39)

## Implementado
- **`ward-core::backup`** (novo, `crates/ward-core/src/backup.rs`):
  - `Versao`: nome `ward-AAAAMMDD-HHMMSS.ward` em **UTC** (o núcleo não tem fuso; a UI
    da Fase 47 mostra no fuso local). Duas versões no mesmo segundo ganham `-1` … `-99`.
    A ordem é a do tempo (data, hora, sufixo), não a do texto. `de_nome` é estrito
    (ASCII, até 28 bytes, data e hora possíveis, sufixo sem zero à esquerda): barra,
    `..` e caminho nunca passam. `do_instante` usa só aritmética inteira (`civil_from_days`).
  - `PastaBackup { dir, uid }`: toda operação confere que `dir` é diretório do `uid`,
    sem nenhum bit para grupo e outros (0700). `listar()` devolve da mais nova para a
    mais antiga e só arquivos comuns (`DirEntry::file_type`, sem seguir link) com o nome
    exato.
  - Gravação (`guardar`, `pub(crate)`): primeiro apaga temporários velhos da pasta (só
    `.backup.ward-tmp-*` comuns), depois grava pelo caminho interno do `gravacao`
    (temporário `O_EXCL` 0600 → fsync → **`link(2)` para o nome final**, que nunca
    sobrescreve → apaga o temporário → fsync do diretório) e por fim gira: apaga as mais
    antigas até sobrarem **30**, nunca a que acabou de gravar e nunca um link.
  - `guardar_atual(pasta, cofre, arquivo, instante)` (pública): grava o arquivo do cofre
    atual como backup **só se ele foi selado por este `CofreAberto`** (mesmo cabeçalho de
    0..128 e corpo que abre pela AEAD com a chave de dados, sem Argon2). É o "grava o
    atual como backup antes de trocar" de D17.
- **`ward-core::gravacao`** (D27: o backup usa o `gravar` interno):
  - `escrever_temporario` e `sincronizar_diretorio` saíram de dentro do
    `gravar_com_pontos` (o comportamento e os pontos de teste do V2.9 são os mesmos);
    `gravar_novo_com_pontos` (`pub(crate)`) publica por `hard_link` sem sobrescrever.
  - Públicas novas: `gravar_entradas_com_backup` e `trocar_senha_com_backup`. Selam e
    gravam o cofre como antes e, **só depois que a gravação deu certo inteira**, gravam
    os **mesmos bytes selados** na pasta. Nada é selado de novo, então o backup não
    queima nem repete geração (D53). Devolvem `Gravado { identidade, backup }`: erro
    do backup não desfaz a gravação do cofre.
  - `gravar_entradas`/`trocar_senha` continuam com a mesma assinatura (delegam a
    `gravar_entradas_selado`/`trocar_senha_selado`).
- **`ward-core::formato`**: `CofreAberto::autentica(arquivo)` (`pub(crate)`) e
  `ChavesCofre::abrir_proprio` (AEAD da biblioteca, nada de cripto à mão).
- **Protocolo** (padrão das operações existentes):
  - `backups` (só app, sem campo) → resposta `backups` (só app):
    `{"versoes": ["ward-…", …]}`, só nomes no formato exato, até `MAX_BACKUPS = 64`.
  - `restaurar_backup` (só app): `{"versao": "<nome exato>", "senha": b64, "pin": b64}`.
    A leitura recusa nome fora do formato, senha vazia, PIN fora do formato e campo a
    mais (`"geracao"`, por exemplo).
  - Erros novos: `backup_inexistente` ("a versão do backup não existe") e
    `backup_indisponivel` ("a pasta de backups não está disponível ou não é segura").
  - Asserções de compilação: `backups` e `restaurar_backup` não são do agente nem da
    extensão.
- **`wardd`**:
  - `arquivos::abrir_pasta_backup(dir)`: cria a pasta 0700 se faltar (sem criar os
    pais), abre com `O_NOFOLLOW | O_DIRECTORY | O_CLOEXEC` e confere pelo `fstat` (dono
    euid, nenhum bit para grupo e outros). `PastaAberta::caminho()` = `/proc/self/fd/<n>`:
    tudo que o núcleo faz na pasta passa pelo fd, então trocar a pasta por um link (ou
    por outro diretório) depois de aberta não desvia nada. `dir_backups(cofre)` =
    `<pasta do cofre>/backups`: `~/.local/share/ward/backups` no release e
    `.dev/backups` no `modo-teste` (V7.1). `Caminhos` não mudou.
  - Toda gravação do cofre (`gravar`, `criar_cofre`, `trocar_senha`) faz backup quando a
    pasta abre. Se a pasta não abre ou não é segura, o cofre é gravado assim mesmo e o
    log avisa com texto fixo, sem caminho.
  - `backups`: só com o cofre destravado.
  - `restaurar_backup`: exige o cofre destravado e o **PIN atual** (conta para os 5 que
    travam) e a senha mestra da versão. Ordem:
    1. lê a versão pelo fd da pasta com `O_NOFOLLOW` (`arquivos::ler_cofre`) e abre com
       a senha dela, sem geração mínima;
    2. confere a identidade do cofre atual (D39/D62; mudou → trava, `cofre_mudou`) e o
       guarda como backup (`guardar_atual`). Se não guardar, não troca:
       `backup_indisponivel`;
    3. queima no estado a geração `q + 1`, com `q` = max(`queimada`, geração do cofre
       aberto), sela a versão com ela pela gravação comum (com backup) e confirma
       `vista = queimada = q + 1`;
    4. o daemon segue destravado com a versão restaurada, sem as liberações de 1 h e com
       os erros de PIN zerados.

    Erro depois de selar trava o cofre, como na troca de senha, e o destravar reabre com
    a `queimada` do estado.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1:** as respostas novas não levam segredo, caminho nem geração. `backups` e
    `restaurar_backup` são só do app: tabela, asserção de compilação e segunda camada
    no `atender_de`.
  - **S2/S6:** o backup é o arquivo cifrado que acabou de ser selado (mesmo formato,
    Argon2id + XChaCha20-Poly1305). Nenhum byte em claro passa pelo módulo, e os
    temporários são prefixos do cifrado.
  - **S7:** gravação atômica pelo caminho interno, com link sem sobrescrever. Uma morte
    em qualquer ponto não mexe nos backups que já estavam lá. A rotação só apaga
    depois que a versão nova entrou.
  - **S9/D39:** restaurar exige o PIN atual (a versão traz outros níveis e outro PIN) e
    nunca baixa a geração. A cópia à mão do backup continua em `retrocesso` (V3.22).
  - **S13:** os logs novos têm só textos fixos (`codigo_backup`), sem caminho nem valor.
  - **D27/D44:** nenhuma gravação pública de bytes soltos. O backup só grava bytes que a
    própria gravação selou, ou o cofre atual autenticado pela chave do cofre aberto.
    O V2.50 (`tests/fontes.rs`) foi atualizado: as duas públicas novas, `fs::hard_link`
    e `linkat` só em `gravacao.rs`, e as funções `*_selado` serializam ou desserializam
    antes de selar.
- **Testes de ataque** (todos falham do lado do atacante)
  - `crates/ward-core/src/backup/testes.rs`:
    - `v7_1_31_gravacoes_deixam_30_versoes`: 31 gravações → 30 arquivos, o mais antigo
      saiu. Arquivos 0600 e pasta 0700, cada backup igual ao cofre gravado; a lista vem
      da mais nova para a mais antiga; abre na geração certa com as 22 entradas de
      `cofre_de_teste` (mesmo espelho da fixture);
    - `mesmo_segundo_ganha_sufixo_sem_sobrescrever`;
    - `v7_2_backup_cifrado_senha_errada_e_byte_alterado`: nenhum dos `SEGREDOS` nem o
      PIN em claro ou em base64. A senha errada não abre. O bit trocado em **cada**
      posição não autentica (AEAD com a chave de dados). Com a senha certa, 10
      posições, uma em cada campo do cabeçalho e do corpo, não abrem. A abertura por
      senha de cada posição é o V2.2, no mesmo formato;
    - `v7_3_sigkill_em_cada_ponto_do_backup`: antes do link, nenhum nome novo e o backup
      anterior intacto; depois, o novo inteiro; a próxima gravação apaga o temporário;
    - `v7_3_sigkill_em_100_pontos_aleatorios`: filho gravando backups sem parar, com
      rotação. Todo backup que fica é uma versão inteira 0600; no máximo 1 temporário,
      que é prefixo do cifrado e não tem segredo; a contagem nunca cai abaixo de
      min(anterior, 30); a gravação seguinte volta às 30;
    - `ataque_pasta_com_permissao_aberta_recusada` (0750/0705/0755/0777/0701,
      inexistente, arquivo, outro uid): nada é gravado ou listado e o cofre é gravado;
    - `ataque_link_com_nome_de_versao_nao_e_seguido`: o link plantado com o nome da
      versão não é seguido nem sobrescrito (vira `-1`), e a lista e a rotação o ignoram;
    - `ataque_guardar_atual_so_aceita_o_arquivo_autentico`: o cofre de outra chave, o
      adulterado e bytes quaisquer dão `NaoAutentico`;
    - `ataque_nome_fora_do_formato_recusado` e `nome_do_instante_em_utc_e_ida_e_volta`
      (bissextos, fim de ano);
    - `troca_de_senha_vai_para_o_backup_com_a_senha_nova`.
  - `crates/ward-core/src/protocolo/testes.rs`:
    - `v7_8_restaurar_e_backups_so_pelo_app`: agente e extensão recebem
      `nao_autorizado` com qualquer campo; o cliente não codifica e a resposta não sai
      nem é lida por outro papel;
    - `ataque_restaurar_com_caminho_no_nome_recusado`: `../`, caminho absoluto,
      `backups/…`, `-01`, senha vazia, PIN `12ab`, sem PIN e `"geracao"` a mais; a
      resposta com nome ruim ou com 65 nomes é recusada;
    - `v3_15_tabela_operacao_papel` e as listas de um de cada atualizadas.
  - `crates/wardd/tests/backup.rs`:
    - `v7_1_cada_gravacao_do_daemon_vai_para_o_backup`: 8 gravações → 8 versões, nas
      gerações 1 a 8, a mais nova igual ao cofre, sem segredo, senha ou PIN em claro;
    - `v7_8_agente_e_extensao_nao_restauram`: pelo protocolo e direto no
      `Daemon::atender`, com o PIN certo; cofre e estado iguais byte a byte;
    - `v7_11_restaurar_sela_geracao_nova_e_copia_a_mao_e_retrocesso`:
      1. n+2 → restaurar a versão n deixa cofre e estado em n+3, com as entradas de n;
      2. o atual (n+2) fica na pasta byte a byte;
      3. travar e destravar com a senha da versão funciona;
      4. a cópia à mão de n cai em `retrocesso`, também num daemon novo, e não é
         regravada;
    - `ataque_restaurar_sem_pin_senha_errada_e_travado`: PIN errado, senha da versão
      errada, versão inexistente e `../` não mexem em nada; 5 PINs errados pelo
      restaurar travam; travado, nem lista nem restaura;
    - `ataque_pasta_de_backups_trocada_por_link`: a pasta vira link para outra com uma
      cópia autêntica → `backup_indisponivel` em listar e restaurar; a gravação do cofre
      segue sem escrever no alvo; a pasta 0755 também é recusada;
    - `ataque_versao_trocada_por_link`: a versão vira link para uma cópia autêntica fora
      da pasta → não restaura (`O_NOFOLLOW`) e some da lista;
    - `ataque_restaurar_com_cofre_trocado_fora_do_ward`: outro inode com os mesmos
      bytes → `cofre_mudou`, trava, nada novo na pasta;
    - `ataque_restaurar_sem_gravar_o_estado_nao_troca_o_cofre`: diretório 0500 →
      `falha_ao_gravar`, o cofre fica igual, o daemon segue com as entradas de antes e
      a geração seguinte não repete;
    - `restaurar_pede_a_senha_da_versao`: a troca de senha vai para o backup com a
      senha nova; a versão anterior só restaura com a antiga.
- **Dependências novas:** nenhuma. `wardd` passou a usar também `libc::O_DIRECTORY`
  (constante, como as que já usava).

## Ficou de fora
- **UI de restaurar:** é a Fase 47.
- **Backup externo:** pendrive e udisks2 são da Fase 46. O ligar e desligar do externo
  pelo socket (a outra metade do V7.8) chega com ela.
- **Fixture e `make dev-cofre`:** não mudaram, então `make teste-dev-cofre` não foi
  necessário. O `make dev-cofre` não apaga `.dev/backups/` ao recriar o cofre. As
  cópias velhas são do cofre de teste, com a mesma senha falsa.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` e `make dev-cofre` | 0 |
| `cargo test -p ward-core` (lib, `corpo_memoria`, `fontes`, doctests) | 0 |
| `cargo test -p wardd --all-features` (inclui `tests/backup.rs`) | 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 |
| `make check` (1ª vez: `fmt-check` acusou `tests/backup.rs`; `make fmt` e de novo) | 2 → 0 |

## Problemas descobertos
- **Restaurar exige o cofre destravado e o PIN atual.** Isso é mais que o texto de
  D17/V7.7, que fala só na senha da versão. Motivo: S9 (a versão traz outros níveis e
  outro PIN) e "o atual vira backup", que só dá para autenticar com a chave do cofre
  aberto. Consequência: com o cofre principal corrompido, ou a senha atual esquecida,
  não há restaurar pelo app. A cópia à mão cai em `retrocesso` (D39). Recuperar disso
  pede apagar o estado e recriar à mão, e não há caminho guiado. O planner decide se
  existe restaurar com o cofre travado (só a senha da versão, o atual copiado sem
  autenticar) e como a Fase 47 pede o PIN.
- **Nome em UTC.** O nome em UTC pode surpreender quem lê a pasta à mão. A Fase 47 deve
  mostrar a data no fuso local.
- **Pedidos de aprovação pendentes não são cancelados pelo restaurar.** As liberações de
  1 h são limpas. Uma resposta "permitir sempre" que chegue depois (com PIN) muda a
  entrada de mesmo nome da versão restaurada. O motor não tem "cancelar todos". Fica
  para a fase da aprovação/uso (26/36) decidir.
- **Resíduo de mesmo uid (RA4/S3).** Um processo do usuário pode pôr na pasta uma cópia
  autêntica antiga com nome novo, e o usuário pode escolhê-la. Isso não abre nada sem
  a senha daquela versão. A versão restaurada ganha geração nova e o atual vira
  backup.
- **Pasta só verificada pelo daemon.** O `PastaBackup` do núcleo segue o caminho que
  recebe (`fs::metadata`). Quem garante que não é um link é o `wardd`, pelo fd com
  `O_NOFOLLOW`. Um chamador futuro do núcleo (Fase 46) precisa abrir a pasta do mesmo
  jeito.
- **Pendrive da Fase 46.** A publicação por `link(2)` não funciona em vfat. A Fase 46
  precisa de outro modo de publicar sem sobrescrever (ou de checar e renomear).
- **Contrato (V7.1).** Diz `.dev/backups/`. É o que o daemon de teste usa (pasta do
  cofre de teste). Os testes automatizados usam diretórios próprios em `target/`.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/backup.rs` | criado |
| `crates/ward-core/src/backup/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado |
| `crates/ward-core/src/formato.rs` | alterado |
| `crates/ward-core/src/gravacao.rs` | alterado |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/erros.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-core/tests/fontes.rs` | alterado |
| `crates/wardd/src/arquivos.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/backup.rs` | criado |
| `docs/mission/handoffs/phase-45.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V7.2, V7.3, V7.8, V7.11, mais pasta trocada por link, versão trocada por link, cofre trocado fora, estado que não grava)
- [ ] (UI/DESIGN) Conferido contra o FigCraft — não se aplica (fase sem tela)
- [x] Ficou dentro do escopo da fase
