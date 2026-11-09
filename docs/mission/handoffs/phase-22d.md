# Fase 22d — Datas de criação e alteração (D65 d, provisória)

## Implementado
- **`ward-core::entrada::Datas`** (novo, em `entrada.rs`): `criada_em`/`alterada_em`
  (`Option<u64>`, segundos UTC). A regra é conferida na construção e na leitura:
  - cada data vai de 1 a `limites::MAX_DATA` (253 402 300 799 = 9999-12-31 23:59:59, cabe
    no `Number` do JS);
  - se existe `criada_em`, existe também `alterada_em`, e ela não é menor.

  Construtores:
  - `Datas::carimbo_novo(agora)` preenche as duas;
  - `Datas::alterado(agora)` mantém a criação e põe `alterada_em = max(agora, anterior,
    criada)`. Um relógio que volta não faz a data descer.

  Os campos são privados, e `de_partes` é `pub(crate)`: fora do crate, uma `Datas` só nasce
  de uma entrada ou de um carimbo.

  Na `Entrada`: `datas()` e `definir_datas()`, esta só para o daemon, como o
  `definir_nivel`. `Rascunho::validar` cria a entrada sem datas, porque o rascunho não tem
  data.
- **Corpo v4** (`corpo.rs`, `VERSAO_CORPO = 4`):
  - cada entrada termina com `criada (u64) ‖ alterada (u64)`, e 0 quer dizer "sem data";
  - a 1ª passada lê e confere as datas pela regra, sem alocar. A 2ª passada aplica as
    datas depois do `validar`;
  - a v3 é recusada como versão desconhecida (`ErroCorpo::Invalido`), sem migração;
  - a escrita confere a faixa de novo, para garantir que o que se grava sempre reabre;
  - a doc do formato foi atualizada.
- **Protocolo**:
  - `DetalheEntrada.datas: Datas`. A escrita só manda `criada_em`/`alterada_em` quando
    existem;
  - a leitura aceita só inteiros que respeitam a regra de `Datas`, sem chave repetida;
    `conferir` faz a mesma checagem;
  - o `detalhe` continua `SO_APP` (D5). A `ItemLista`/resumo não leva datas;
  - o rascunho (`ler_rascunho`/`ler_dados`) já recusava chave desconhecida com `fim()`, então
    data vinda do cliente é recusada. Isso agora tem teste.
- **`wardd`** (`nucleo.rs`), com o carimbo dado pelo `Relogio::segundos_unix()`, que os testes
  injetam:
  - entrada nova (`salvar_entrada` sem `anterior`) recebe `carimbo_novo`;
  - edição, inclusive renomear, mantém `criada_em` da entrada editada e atualiza a alteração;
  - `mudar_nivel` atualiza a alteração. O desfazer de uma gravação que falhou volta o nível
    e as datas;
  - restaurar não faz nada de especial: a versão é selada de novo com as datas que ela já
    tinha;
  - relógio ilegível ou fora da faixa: a gravação segue sem carimbo novo (entrada nova sem
    datas, edição mantém as datas que tinha) e o log registra um aviso de texto fixo. Ver
    "Problemas".
- **`make dev-cofre`**:
  - regrava o cofre de teste em v4. As 22 entradas da fixture nascem **sem datas**;
  - V2.29 refeito: `make teste-dev-cofre` roda o `dev-cofre` duas vezes e confere as 22
    entradas e o corpo v4;
  - mudaram os textos "v3" → "v4" em `dev/testes/dev-cofre.sh`, na doc e no assert
    `corpo[..2] == [4, 0]` do exemplo.
- **App**:
  - `app/nucleo/src/credenciais.rs`: `DetalheCredencial` ganhou `criada_em`/`alterada_em`
    (`Option<u64>`, `null` no JSON);
  - `daemon_falso.rs` carimba pela mesma regra do `wardd`, usando o relógio do processo;
  - `Credenciais.svelte`, só no rodapé: o `<span class="espaco">` virou o `metadados` do
    node `4400`. O texto é "Criada em dd/mm/aaaa · Alterada em dd/mm/aaaa" no fuso local
    (`Date` do navegador), só com as datas que existem; sem data, o span fica vazio e
    continua ocupando o espaço. Estilo pelos tokens: `--tamanho-xs` 12px,
    `--altura-linha-xs` 16px, `--peso-regular`, `--cor-texto-secundario` (#9AA1AE =
    variable `6:65` do FigCraft), `fill`.

## Segurança
- **Invariantes tocadas e como a fase as respeita**
  - **S1:**
    - as datas só saem no `detalhe`, que é só do app (tabela, `codificar_resposta` e
      leitura do agente recusam);
    - a lista do app e a do agente não levam data;
    - data vinda do cliente (`salvar_entrada`, `salvar_login`, `mudar_nivel`, no topo, nos
      campos ou no envelope) é recusada pelo decodificador.
  - **S6:** as datas estão dentro do corpo cifrado e autenticado. 1 bit trocado em
    qualquer um dos 48 bytes cifrados das datas faz a tag recusar. Pelo Argon2, a abertura
    dá `SenhaOuArquivo`, como no V2.1/V2.2.
  - **S7/D39/D53:** nada muda na geração. As datas não entram em nenhuma decisão de
    segurança.
  - **S8/S13:** as datas não são segredo e não vão para o log. O aviso do relógio é texto
    fixo. Os erros de corpo continuam genéricos e sem valor; o teste confere que o valor
    forjado não aparece no erro.
  - **Disco não confiável:** data fora da regra recusa o corpo inteiro, e a 1ª passada não
    aloca (V2.32 continua passando com o controle v4).
- **Testes de ataque escritos**
  - `crates/ward-core/src/corpo/testes.rs`:
    - `d65_corpo_v3_recusado`: v3 exato, v4 rotulado 3, v4 sem datas, versão 5 e v3 vazio;
    - `d65_datas_forjadas_no_corpo`: criação depois da alteração, criação sem alteração,
      acima do máximo, `u64::MAX`, além das formas válidas com `conferir_aceito`;
    - `d65_cofre_v3_no_disco_recusado_sem_tocar_no_arquivo`: um cofre v3 autêntico no
      disco é recusado por `abrir_entradas_lida` com `Corpo(Invalido)`, e o arquivo, o
      mtime e o diretório ficam iguais;
    - `d65_datas_so_mudam_os_16_bytes_da_entrada`;
    - `d65_regra_das_datas`, que inclui o relógio voltando;
    - `v2_31_campos_forjados` passou a cobrir as versões 3 e 5.
  - `crates/ward-core/src/gravacao/testes.rs`:
    - `d65_v4_byte_alterado_nas_datas_falha_a_abertura` (V2.2 com o v4);
    - `d65_v4_sigkill_em_cada_ponto_mantem_as_datas` (V2.9 com o v4: SIGKILL em cada ponto
      deixa a versão antiga ou a nova inteira, e as datas voltam).
  - `crates/ward-core/src/protocolo/testes.rs`:
    - `d65_datas_so_no_detalhe_do_app`;
    - `ataque_d65_detalhe_com_data_forjada_recusado`: 12 formas forjadas;
    - `ataque_d65_data_no_rascunho_recusada`.
  - `crates/ward-core/tests/corpo_memoria.rs`: o caso "versão 3" foi incluído no V2.32
    (recusa sem alocar).
  - `crates/wardd/tests/datas.rs` (relógio injetado):
    - `v3_41_datas_pelo_relogio_do_daemon`: nova, edição, `mudar_nivel`, PIN errado sem
      efeito, relógio voltando, renomear, e reabrir num daemon novo;
    - `ataque_v3_41_datas_nao_vao_ao_agente`: o agente e a extensão recebem
      `nao_autorizado` no `detalhe`; a lista não leva datas; o quadro com as datas não
      tem segredo, nem em base64;
    - `ataque_v3_41_data_vinda_do_cliente_recusada`: o cofre fica com os mesmos bytes;
    - `relogio_ilegivel_grava_sem_data_nova`;
    - `restaurar_mantem_as_datas`.
  - `app/nucleo/src/credenciais.rs` › `d65_datas_no_detalhe_da_tela`.
  - Conferência manual, feita uma vez e apagada depois: o `.dev/cofre-teste.ward` v3 de
    antes da fase, num `wardd` de teste com o estado na geração 1. O `destravar` respondeu
    `cofre_invalido`, o cofre ficou travado e o arquivo e o mtime não mudaram.
- **Dependências novas:** nenhuma.

## Ficou de fora
- **Teste de v3 no daemon dentro do repositório:** fora do `ward-core` não há como selar um
  corpo arbitrário, e abrir uma API para isso seria regressão de D27. O teste permanente
  fica no núcleo, em `abrir_entradas_lida`, que é a mesma chamada do `destravar`. A
  conferência pelo `wardd` foi manual (acima).
- **Datas no cofre de teste:** as 22 entradas da fixture ficam sem datas. Assim o caso
  "entrada sem datas não mostra o texto" do V5.34 tem dado real.
- **`docs/escopo.md`/PRD:** não mexi. As datas continuam provisórias (D65 d).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` | 0 |
| `make dev-cofre` (antes e depois da troca para v4) | 0 |
| `make dev-variacoes` | 0 |
| `cargo test -p ward-core` (1ª: `v2_31_campos_forjados` ainda listava a 4 como desconhecida, corrigido) | 101 → 0 |
| `cargo test -p wardd --all-features --test datas` | 0 |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `make app-front` | 0 |
| ponte `--daemon-falso cheio` + Playwright Python headless (Chrome, 1280×800, `qa22d.py` no scratchpad; a 1ª rodada caiu no seletor `.rodape` ambíguo) | 1 → 0 |
| `make fmt` | 0 |
| `make teste-dev-cofre` | 0 |
| `make check` | 0 |
| `make app-check` (com `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do R32) | 0 |

## Problemas descobertos
- **Relógio ilegível:** escolhi gravar sem carimbo novo em vez de recusar a gravação. As
  datas são só informação, e o `totp` responde `interno` nesse caso. Assim, uma entrada
  nova criada com o relógio quebrado fica sem datas para sempre. O planner pode inverter a
  escolha.
- **Entrada sem criação:** uma entrada sem datas que é editada ganha só `alterada_em`, e o
  rodapé mostra só "Alterada em …". O V5.34 fala de "uma entrada sem datas"; o caso parcial
  não está no contrato.
- **Fora da lista de arquivos da fase:**
  - `app/nucleo/src/daemon_falso.rs`: o carimbo, para a ponte de teste seguir a regra do
    `wardd`;
  - `crates/ward-core/src/limites.rs`: `MAX_DATA`;
  - `crates/ward-core/src/entrada/validacao.rs`: inicialização sem datas;
  - `crates/ward-core/src/gravacao/testes.rs`, `crates/ward-core/tests/corpo_memoria.rs` e
    `dev/testes/dev-cofre.sh`: texto v3 → v4.
- **Comparação com o FigCraft:**
  - o design mostra datas fixas (02/11 e 07/11); o app mostra as do daemon;
  - o headless do FigCraft renderiza o rodapé no tema claro, e o app está no escuro (como
    nas outras fases).
- **Fuso:** o app formata no fuso local, e o daemon guarda UTC.
- **Console da ponte:** o 404 do favicon é anterior a esta fase.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/entrada.rs` | alterado |
| `crates/ward-core/src/entrada/validacao.rs` | alterado |
| `crates/ward-core/src/limites.rs` | alterado |
| `crates/ward-core/src/corpo.rs` | alterado |
| `crates/ward-core/src/corpo/testes.rs` | alterado |
| `crates/ward-core/src/gravacao/testes.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/ward-core/tests/corpo_memoria.rs` | alterado |
| `crates/ward-core/examples/dev-cofre/main.rs` | alterado |
| `crates/ward-core/examples/dev-cofre/testes.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/tests/datas.rs` | criado |
| `app/nucleo/src/credenciais.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/src/telas/Credenciais.svelte` | alterado |
| `dev/testes/dev-cofre.sh` | alterado |
| `docs/mission/evidence/fase-22d/` (`credenciais-cheia-com-datas.png`, `credenciais-sem-datas.png`, `rodape-com-datas.png`, `figcraft-4148-rodape.png`, `qa.json`) | criado |
| `docs/mission/handoffs/phase-22d.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V3.41)
- [x] (UI/DESIGN) Conferido contra o FigCraft (node `4400`/`4148`, só leitura; nada salvo)
- [x] Ficou dentro do escopo da fase (exceções em "Problemas")
