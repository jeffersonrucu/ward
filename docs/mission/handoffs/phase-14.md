# Fase 14 — Gravação atômica

## Implementado
- **`ward-core::gravacao`** (público, `#[cfg(unix)]`, só std, sem `unsafe`). Aproveitei
  quase todo o código parcial do worker anterior. O módulo de produção
  (`crates/ward-core/src/gravacao.rs`) ficou como ele deixou; as correções foram todas
  nos testes.
  - `gravar(destino, arquivo)`: grava bytes que já são um cofre cifrado. Primeiro
    confere a estrutura com `formato::ler_cabecalho` (bytes sem cara de cofre dão
    `NaoECofre` sem tocar no disco). Depois:
    1. cria o temporário `.<nome>.ward-tmp-<32 hex aleatórios>` no mesmo diretório,
       com `O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC` e modo 0600 (também fixado no
       descritor);
    2. escreve em pedaços de 1 MiB;
    3. faz o `fsync` do temporário;
    4. faz o `rename` por cima do destino;
    5. faz o `fsync` do diretório.

    Se a gravação falha antes do rename, um guarda (`Temporario`) apaga o temporário.
  - `gravar_cofre(destino, &CofreAberto, conteudo)`: sela com `CofreAberto::selar` e
    grava. É o caminho do daemon.
  - `limpar_temporarios(destino)`: apaga só arquivos comuns com o prefixo do próprio
    cofre. Não mexe em link simbólico, diretório nem temporário de outro cofre.
  - `ErroGravacao` (`NaoECofre`, `Formato`, `DestinoInvalido`, `Io { etapa, tipo }`) e
    `Etapa`: mensagens fixas em português, sem nenhum dado do cofre.
  - Pontos de injeção internos (`Ponto`, `gravar_com_pontos`), usados só pelos testes.
- **Correções nos testes** (`crates/ward-core/src/gravacao/testes.rs`):
  - Os dois avisos do clippy: `for k in 1..` virou `iter().cycle()`, e
    `for i in 0..PONTOS.len()` virou `iter().enumerate()`.
  - `v2_9_sigkill_em_cada_ponto` e `v2_9_sigkill_em_200_pontos_aleatorios` falhavam
    porque o libtest escreve `test … ... ` sem quebra de linha antes da saída do
    filho. A marca saía como `test gravacao::testes::filho_gravacao ... WARD-PONTO`,
    e a comparação exata (`l == esperada`) nunca casava. Cada um dos 6 pontos esperava
    60 s até estourar o tempo, o que explica o travamento. Agora o filho começa a marca
    numa linha nova, e o pai aceita a linha que termina com a marca.
  - `v2_10_v2_11_strace_ordem_e_nada_em_claro` falhava no parser: o strace alinha o
    retorno com espaços (`fsync(3)          = 0`), e o código procurava `") = "`. Agora
    ele separa em `" = "` e confere se o lado esquerdo termina em `)`.
  - O filho tem tempo de vida máximo (`VIDA_FILHO`, 90 s). Se o pai morrer sem matá-lo,
    o filho sai sozinho. Antes, ele dormia 3600 s em laço no modo `ponto`, ou gravava
    para sempre no modo `laco`.
  - V2.11 também procura no log do strace o segredo falso do conteúdo
    (`falso-PG:s3nha/01@#`) e a senha mestra de teste, além da marca aleatória.
  - Teste novo `destino_link_simbolico_nao_e_seguido`.

## Segurança
- Invariantes tocadas:
  - **S7**: temporário no mesmo diretório → fsync → rename → fsync do diretório. Uma
    morte ou falha antes do rename deixa a versão anterior inteira. Uma falha depois
    deixa a nova inteira. Nunca fica uma mistura das duas.
  - **S2**: o módulo só aceita bytes com estrutura de cofre. O temporário deixado por
    uma morte é um prefixo do arquivo cifrado, nunca texto em claro. O strace prova que
    nenhum `write` leva conteúdo em claro.
  - **S13**: os erros não carregam dado do cofre.
  - Arquivos 0600, e o temporário nunca segue link nem reaproveita arquivo
    (`O_EXCL`).
- Testes de ataque (`crates/ward-core/src/gravacao/testes.rs`):
  - `v2_9_sigkill_em_cada_ponto`: SIGKILL num processo filho exatamente depois de cada
    etapa: criar, meio da escrita, fim da escrita, fsync, rename e fsync do diretório.
    Antes do rename fica a versão antiga, e depois, a nova. O temporário que sobra é
    0600, cifrado, prefixo do novo, e a limpeza o apaga.
  - `v2_9_sigkill_em_200_pontos_aleatorios`: o filho grava sem parar três versões de
    ~2,5 MiB e morre 200 vezes em momentos aleatórios. O destino é sempre uma versão
    inteira que abre com a senha. Na última rodada, 145 das 200 mortes caíram no meio
    de uma gravação, e as três versões apareceram.
  - `v2_9_falha_de_es_injetada_em_cada_ponto`: `StorageFull` injetado em cada etapa,
    com o erro e a etapa certos, sem sobrar temporário, com a versão certa no destino
    e com modo 0600.
  - `v2_10_v2_11_strace_ordem_e_nada_em_claro`: roda `strace -f -s 100000 -e
    trace=openat,open,creat,write,pwrite64,writev,fsync,fdatasync,rename,renameat,renameat2`
    numa gravação e confere:
    - V2.10: a ordem openat do temporário (mesmo diretório, `O_CREAT|O_EXCL|O_CLOEXEC`,
      0600) → writes → fsync dele → rename para o destino → openat e fsync do
      diretório. Nada mais é aberto para escrita no diretório, e nada é escrito depois
      do fsync.
    - V2.11: nenhum buffer contém a marca, o segredo falso ou a senha mestra. Uma
      escrita de controle prova que o detector enxerga, e cofre e temporários não têm
      a marca.
  - `recusa_gravar_bytes_que_nao_sao_cofre`: texto em claro, cofre truncado, magia
    errada, vazio e zeros. Nada é criado e o destino fica intacto.
  - `temporario_nao_segue_link_simbolico_nem_reaproveita_arquivo`: link plantado,
    link pendente e arquivo existente.
  - `destino_link_simbolico_nao_e_seguido` (novo): o destino é um link para um
    arquivo da vítima. A vítima fica intacta, e o destino vira arquivo comum 0600.
  - `limpar_temporarios_so_apaga_os_do_cofre`: link e diretório com o prefixo e
    temporário de outro cofre ficam.
  - Também: `destino_diretorio_falha_sem_deixar_temporario`,
    `nomes_de_temporario_nao_repetem`, `temporario_nasce_0600_com_umask_qualquer` e
    `erro_nao_carrega_conteudo`.
- Mutações conferidas à mão, todas revertidas (`gravacao.rs` voltou byte a byte, por
  `diff`):
  - sem o fsync do diretório → V2.10 falha;
  - fsync do temporário depois do rename → V2.10 falha;
  - cópia em claro gravada ao lado do cofre → V2.11 falha.
- Dependências novas: nenhuma.

## Ficou de fora
- **Backup datado de cada gravação (30 versões, PRD "Backup")**: o plano não o põe
  nesta fase.
- **Chamar `limpar_temporarios` ao abrir o cofre**: é do daemon (Fase 22). A função
  está pronta e testada.
- `mlock`/D14: é da Fase 21.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `timeout 300 cargo clippy -p ward-core --all-targets --locked -- -D warnings` (antes da correção) | 101 (2 avisos) |
| `timeout 200 cargo test … v2_9_sigkill_em_cada_ponto` (antes da correção) | 101 (marca não casava, 60 s por ponto) |
| `timeout 120 cargo test … v2_10` (antes da correção do parser) | 101 |
| `timeout 280 cargo test -p ward-core --locked gravacao` (16 testes, ~9 s) | 0 |
| 3 mutações em `gravacao.rs` (cada uma derruba o teste esperado) e reversão | 101 → 0 |
| `timeout 590 make check` (65 testes no ward-core) | 0 |

## Problemas descobertos
- **O travamento anterior** vinha da marca do filho na mesma linha do `test … ... `
  do libtest: cada ponto esperava 60 s, e o filho do modo `ponto` dormia 3600 s em
  laço. Um teste interrompido deixava filhos órfãos. Corrigido, e agora o filho tem
  vida máxima de 90 s.
- **`gravar` confere a estrutura, não a autenticidade.** `ler_cabecalho` recusa texto
  em claro, truncado e magia errada, mas um buffer com cabeçalho válido e "corpo"
  arbitrário do tamanho declarado passaria. Conferir a autenticidade exigiria a chave.
  O caminho seguro para conteúdo é `gravar_cofre`, que sempre sela. `gravar` existe
  para a saída de `selar`/`trocar_senha`. Quem chama é código do próprio Ward, nunca
  entrada do socket. A auditoria pode decidir se `gravar` vira `pub(crate)` quando o
  daemon existir.
- **V2.10/V2.11 exigem `/usr/bin/strace`** e ptrace de filho permitido
  (`kernel.yama.ptrace_scope` ≤ 1; aqui é 1). Sem strace, o teste **falha**, com a
  mensagem "apt install strace", em vez de pular. Escolhi falhar por segurança, e
  isso também vale para o `make check` em outra máquina.
- O diretório do cofre é considerado confiável (do usuário, 0700). Um atacante com
  escrita nesse diretório já poderia trocar o cofre. O `O_EXCL` e o nome aleatório
  cobrem só link plantado e colisão.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/gravacao.rs` | criado (pelo worker anterior; mantido sem mudança) |
| `crates/ward-core/src/gravacao/testes.rs` | criado (pelo worker anterior) e alterado |
| `crates/ward-core/src/lib.rs` | alterado (pelo worker anterior: `#[cfg(unix)] pub mod gravacao;`) |
| `docs/mission/handoffs/phase-14.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): 0
- [x] Só cofre de teste e credenciais falsas usados (senha `falso-mestra-ward-01`,
  conteúdo falso de D15, arquivos só em `target/debug/ward-teste-gravacao/`, que fica
  vazio no fim)
- [x] Teste de ataque para cada (SEG) tocado (V2.9, V2.10, V2.11 e mais link no
  destino, link no temporário e bytes que não são cofre)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (Fase 15 / `entrada` não foi tocada)
