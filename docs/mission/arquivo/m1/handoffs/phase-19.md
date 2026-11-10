# Fase 19 — `make dev-cofre` e `make dev-variacoes`

## Implementado
- **Fixtures (D28):** `acme-totp-dev` entrou em `dev/fixtures/entradas.json` (login, `acme`,
  `dev`, livre, `http://localhost:8083/login`, `wp_admin`, `falso-WP-senha-01`, semente
  `JBSWY3DPEHPK3PXP`). Agora são 21 entradas. O `_sobre` cita D28 e o `dev-variacoes`.
  Nenhum valor novo: o `gerar.py` e a allowlist do gitleaks ficaram como estavam, e o
  `make dev-servicos` segue igual.
- **`make dev-cofre`** chama o exemplo `crates/ward-core/examples/dev-cofre/` (`cargo run
  --example dev-cofre`). Não é binário instalado. Ele só usa a API pública do `ward-core`:
  - lê `dev/fixtures/entradas.json` (até 1 MiB) com um leitor JSON estrito, feito no
    exemplo para não trazer dependência. Campo desconhecido em qualquer nível é erro;
  - monta cada `Rascunho` e passa por `Rascunho::validar` (Fase 15), depois por
    `definir_nivel` com o nível da fixture;
  - lê a chave privada só de `.dev/servicos/chaves/` (gerada por `make dev-servicos`, que
    é pré-requisito do alvo) e os anexos só de `dev/fixtures/anexos/`. Nenhum dos dois
    aceita `..` ou link simbólico em componente algum, e o tamanho tem limite (chave
    64 KiB, anexo 256 KiB por D29);
  - cria o cofre com `CofreAberto::novo` (Argon2id `PADRAO`) e grava com
    `gravar_entradas` (D27, gravação atômica 0600);
  - **reabre** com `abrir_entradas` e a senha de teste e confere que o corpo canônico
    relido tem exatamente os mesmos bytes que o das fixtures (mesmos campos, segredos e
    níveis). Imprime só nome, tipo e nível.
  - Rodar de novo recria o arquivo pelo mesmo caminho atômico (V2.29, "duas vezes").
  - `COFRE=.dev/<nome>.ward` troca o destino. O valor vai ao programa pelo ambiente
    (`WARD_DEV_SAIDA`), sem passar pelo shell.
- **`make dev-cofre-conferir`**: só abre e confere. O exemplo também aceita
  `--conferir --senha-stdin`, que lê a senha do stdin e é usado no teste de senha errada.
  Não existe opção de senha em argv.
- **Caminho de saída (V2.29):**
  1. Primeiro vem a conferência **só pelo texto**: relativo conta a partir da raiz, `..` é
     recusado, o caminho tem de ficar dentro de `<raiz>/.dev/`, e o nome tem de ser
     `*.ward` sem começar com `.`. Um caminho de fora do repositório nem chega a ser
     consultado no disco.
  2. Depois vem a do disco. `.dev` tem de ser diretório de verdade, filho da raiz, e é
     criado 0700 se faltar. Nenhum diretório do meio pode ser link. Se o destino existir,
     tem de ser arquivo comum: link, inclusive quebrado, e diretório são recusados.
- **`make dev-variacoes`** (`dev/variacoes.py`, só biblioteca padrão) grava
  `.dev/variacoes.txt` em 0600, de forma atômica, com as mesmas regras de saída (`*.txt`)
  e `VARIACOES=.dev/<nome>.txt`. São 299 padrões, um por linha, ordenados e
  determinísticos.
  - Valores cobertos: os `segredos` das fixtures, os extras secretos, os anexos, a chave
    privada de `.dev/`, a senha mestra e o PIN.
  - Para cada valor entram:
    - o literal;
    - o URL-encoded: `quote` sem nada seguro e com `/` seguro, `quote_plus`, hexadecimal
      maiúsculo e minúsculo, e todo byte codificado;
    - o base64 padrão e o URL-safe, com e sem `=`, mais o miolo do base64 nos 3
      alinhamentos;
    - o Basic `usuário:segredo`;
    - o escape JSON/PHP;
    - o `DATABASE_URL` das entradas `banco`.
  - Valor de várias linhas (chave, anexo): entra cada linha aparada com 6 ou mais
    caracteres, e o valor de `CHAVE=valor` com 6 ou mais (D34).
  - O script falha fechado se sair padrão vazio, com menos de 6 bytes ou com quebra de
    linha (isso faria o `grep -F -f` casar com tudo). Na tela só aparecem o caminho e a
    contagem; a lista nunca é impressa.
  - O script é independente da máscara da Fase 17 de propósito: é o oráculo de quem
    confere.
- **`make teste-dev-cofre`** (`dev/testes/dev-cofre.sh`) faz o teste ponta a ponta de
  V2.29 e os ataques (lista abaixo).
- `CLAUDE.md` › "Comandos": `dev-cofre`, `dev-cofre-conferir`, `dev-variacoes` e
  `teste-dev-cofre` estão como "pronto". `Makefile`: alvos novos no lugar dos stubs.

## Segurança
- Invariantes tocadas:
  - **S15 / R19:** saída só em `.dev/`, que é ignorado pelo git (conferido no teste).
    Nada é lido fora do repositório. Nenhum valor novo entra no repositório, a lista e o
    cofre ficam só em `.dev/`, e o V1.5 continua valendo: `dev/` não cita pasta do usuário.
  - **S2:** o arquivo só é gravado por `gravar_entradas`, que sela antes. Nenhum segredo
    nem variação aparece em claro no `.ward` (`grep -aF -f` da lista dá 0).
  - **S9:** os três níveis de D15 saem das fixtures. Se a montagem ignorasse o nível, dois
    testes cairiam (mutação abaixo).
  - **S13:** nenhum segredo vai para stdout, stderr ou mensagem de erro. Os erros só têm
    texto fixo, nome de entrada e campo, e caminho do repositório. O teste confere toda
    saída dos alvos contra a lista.
  - **S3 (análogo):** a senha nunca vai em argv. O `--senha` não existe e é recusado.
  - **S8:** os segredos lidos vão para `Segredo` numa alocação do tamanho exato. Os
    textos do JSON nascem com a capacidade final e zeram ao sair (`Zeroizing`), e o buffer
    das fixtures é `Zeroizing`. O que não zera está em "Problemas".
- Testes de ataque escritos:
  - `crates/ward-core/examples/dev-cofre/testes.rs` (16 testes, rodam no `cargo test`
    por `test = true`):
    - `v2_29_fixtures_iguais_a_tabela_do_contrato`: as 21 entradas das fixtures dão o
      mesmo corpo canônico, byte a byte, que o `cofre_teste.rs` da 15a (tabela do
      contrato com `acme-totp-dev`);
    - `v2_29_niveis_da_tabela`;
    - `v2_29_grava_reabre_e_confere`: grava duas vezes, 0600, sem segredo em claro.
      Recusa senha errada, cofre com uma entrada a menos e cofre 0640;
    - `v2_29_saida_dentro_de_dev_aceita`, `dev_ausente_e_criado_0700`;
    - `ataque_saida_fora_de_dev_recusada`, com 17 caminhos: absoluto de fora, `/tmp`, raiz,
      `dev/`, `.dev/../`, `../`, `.devx/`, `.dev-mau/`, `.dev`, `.dev/`, `.dev/.ward`,
      oculto, `.txt`, `.ward.tmp`, diretório inexistente e o `.dev` de outra raiz. Nada é
      criado fora;
    - `ataque_link_simbolico_no_caminho_recusado`: diretório-link, destino-link para
      arquivo de fora (que não muda), link quebrado e destino que é diretório;
    - `ataque_dev_que_e_link_recusado`: `.dev` que é link (o alvo fica vazio) e `.dev`
      que é arquivo;
    - `ataque_arquivo_de_entrada_fora_da_pasta_recusado`: 11 casos (absoluto, `..`,
      pasta errada, link, diretório-link, `/etc/hostname`, inexistente com a dica do
      `make dev-servicos`);
    - `ataque_arquivo_de_entrada_grande_demais_recusado`;
    - `ataque_json_malformado_recusado` (26 casos: chave repetida, sobra, vírgula final,
      fração, expoente, zero à esquerda, fora do i64, escape inválido, substituto
      solto ou inválido, controle e nulo crus, UTF-8 inválido, 40 níveis…),
      `json_le_o_que_as_fixtures_usam`;
    - `ataque_fixture_com_campo_desconhecido_recusada`: o valor não aparece no erro;
    - `ataque_fixture_invalida_recusada_sem_valor_no_erro`: URL `javascript:`, tipo e
      nível fora da lista, host `-oProxyCommand=x`, porta 70000, versão 2, senha vazia,
      PIN curto. Nem o segredo, nem a senha, nem o PIN aparecem no erro;
    - `ataque_cofre_inconsistente_nao_grava`: nome repetido, túnel para entrada que não
      existe;
    - `opcoes_da_linha_de_comando`: `--senha` em argv recusado.
  - `dev/testes/dev-cofre.sh` (`make teste-dev-cofre`, 30 checagens, todas ok):
    - `dev-variacoes`:
      - determinístico, 0600, sem linha vazia nem curta;
      - tem todo literal das fixtures e as variações de D11 calculadas à parte com
        coreutils (`%3A…`, hex minúsculo, `DATABASE_URL`, base64 com e sem `=`, URL-safe,
        Basic de `wp_editor` e de `deploy`, a linha do anexo);
      - tem a chave de `.dev/`, inteira em base64 e linha a linha;
      - acha o segredo dentro de um base64 maior (`x-access-token:`) e de um formulário;
    - `dev-cofre`:
      - roda duas vezes: arquivo 0600, `.dev` 0700, ignorado pelo git, sem temporário que
        sobre, magia `WARDCOFR`;
      - o `dev-cofre-conferir` lista exatamente as 21 linhas (nome, tipo, nível) **lidas
        da tabela do `validation-contract.md`**, uma fonte independente das fixtures;
      - nomes, tipos e níveis não casam com a lista (sem falso positivo);
      - nenhuma variação aparece em claro no `.ward`;
      - abre com a senha certa pelo stdin e recusa a errada; senha em argv é recusada;
    - os dois alvos recusam 11 saídas pelo `make … COFRE=`/`VARIACOES=`: absoluta de fora,
      `.dev/../`, raiz, `../`, diretório-link, arquivo-link, link quebrado, extensão
      errada, injeção com `"; touch …` e `$(touch …)`. Nada é criado nem muda fora, e o
      arquivo de injeção não aparece;
    - nenhuma saída dos alvos tem segredo.
  - Mutações conferidas e revertidas, todas pegas:
    - sem o prefixo `.dev`: 3 ou mais testes caem;
    - sem conferir os links do meio: cai o `ataque_link_simbolico_…`;
    - destino-link aceito: cai o mesmo;
    - nível ignorado na montagem: caem 2 testes;
    - campo desconhecido aceito: cai o `ataque_fixture_com_campo_…`;
    - Python sem o prefixo `.dev`: a saída de fora ainda é recusada (pela conferência dos
      links do meio, mas com traceback), e o `teste-dev-cofre` acusaria "falhou sem a
      recusa do caminho".
- Dependências novas: nenhuma. O `Cargo.lock` não mudou. O exemplo usa só `ward-core` e
  `zeroize`, que já são dependências do crate. O leitor JSON foi feito no exemplo porque
  `serde_json` seria dependência nova só para ferramenta de desenvolvimento, e JSON não é
  criptografia.

## Ficou de fora
- Hash do PIN de teste no cofre: é da Fase 19b (D30). O corpo v1 não tem onde guardar.
  Hoje o `dev-cofre` só confere que o `pin` da fixture tem 6 dígitos e não o grava.
- Ligar a lista de variações à máscara da Fase 17 (conferir que a máscara cobre cada
  padrão): não está no escopo, e a lista é independente de propósito.
- `make check` totalmente limpo: depende da Fase 19a fechar (ver "Problemas").

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked --example dev-cofre` (16 testes, ~1,7 s) | 0 |
| 5 mutações no exemplo + 1 no `variacoes.py`, e reversão (`cmp` igual) | 101 → 0 |
| `make dev-variacoes` | 0 |
| `make dev-cofre` (1ª tentativa, com o `ward-core` da 19a no meio da edição) | 2 (erro de compilação em `corpo.rs`, da 19a) |
| `make teste-dev-cofre` (1ª: 2 falhas do próprio script, contagem e `git check-ignore`) | 1 |
| `make teste-dev-cofre` (30 checagens, rodado 3 vezes) | 0 |
| `cargo clippy -p ward-core --all-targets --locked -- -D warnings` | 0 |
| `rustfmt --edition 2024 --check crates/ward-core/examples/dev-cofre/main.rs` (com `testes.rs`) | 0 |
| `make check` | 2 (para no `fmt-check`, só por `crates/ward-core/tests/corpo_memoria.rs`, da 19a) |
| `make clippy` / `make audit` / `make deny` / `make gitleaks` | 0 / 0 / 0 / 0 |
| `make test` | 2 (8 testes da lib da 19a em andamento: `entrada`, `entrega`, `corpo`, `mascara`; nenhum desta fase) |

## Problemas descobertos
- **Apaguei um diretório do scratchpad que não era meu.** No teste de mutação do Python
  usei `$SCRATCHPAD/fora` achando que o `mkdir -p` o criava. Ele já existia, com um
  `Cargo.toml` e um `src/`, e o `rm -rf` no fim o apagou. O scratchpad é compartilhado
  pelos agentes da sessão. Pode ter sido a "cópia de trabalho" da Fase 19a (V2.35) ou
  algo de uma fase anterior. Fora do repositório não toquei em mais nada. O orchestrator
  deve avisar a 19a, se for dela. Depois disso passei a usar só `scratchpad/fase19/`.
- **`make check` não fecha por causa da Fase 19a**, que roda em paralelo: o fmt de
  `tests/corpo_memoria.rs` e 8 testes da lib (`entrada`, `entrega`, `corpo`, `mascara`).
  clippy, audit, deny e gitleaks passam, e os testes desta fase também. Precisa rodar de
  novo quando a 19a fechar. O `v2_29_fixtures_iguais_a_tabela_do_contrato` compara as
  fixtures com o `cofre_teste.rs`, que a 19a também mexeu. Ele passou com o estado atual,
  e vai pegar se os dois se desencontrarem.
- **Padrão curto na lista falha fechado.** O `dev-variacoes` recusa padrão com menos de
  6 bytes, porque deixaria o `grep -F -f` casar com quase tudo. D34 manda mascarar
  segredo de campo de qualquer tamanho, então uma fixture futura com senha de 4
  caracteres faria o alvo falhar. Para o validador, a saída seria procurar esse valor à
  parte. Decisão do planner.
- **Falsos positivos possíveis na lista:**
  - o PIN `135790` (6 dígitos) pode aparecer por acaso numa saída numérica (PID, porta,
    tamanho);
  - as linhas `-----BEGIN/END OPENSSH PRIVATE KEY-----` da chave de teste estão na lista,
    então o cabeçalho de **qualquer** chave na saída conta como vazamento.

  Os dois ficaram por segurança. O validador deve ver o contexto antes de reprovar só
  por eles.
- **Variações além de D11** (miolo de base64, JSON/PHP, todo byte codificado, hex
  minúsculo): acham mais casos, e todos são vazamento de fato. Se a máscara da Fase 17
  não cobrir algum deles, o validador vai acusar. Hoje ela cobre: visões decodificadas,
  miolo e JSON.
- **Memória que não zera (só valores falsos, só ferramenta de desenvolvimento):**
  - no exemplo, os campos não secretos do JSON viram `String` comum;
  - no `variacoes.py`, os segredos ficam em objetos Python, que não zeram.
- **TOCTOU no caminho de saída:** entre a conferência e a gravação, um processo do mesmo
  usuário pode trocar um diretório de `.dev/` por um link. A gravação usa temp `O_EXCL`
  + rename no diretório conferido, e isso não reconfere o caminho. Esse atacante (mesmo
  uid, mexendo em `.dev/` durante o `make`) fica fora do modelo de uma ferramenta de
  desenvolvimento. Para o daemon da Fase 22, que não é ferramenta de desenvolvimento,
  vale abrir o diretório com `O_NOFOLLOW` e usar `*at`.
- O exemplo fica em `crates/ward-core/examples/` e compila no `cargo test`/`clippy
  --all-targets`. A Fase 19b deve ajustá-lo para gravar o hash do PIN (D30). As regras
  de caminho e de fixture ficam como estão.
- `make dev-cofre` depende de `dev-servicos`, que reescreve as senhas de serviço em
  `.dev/servicos/` a cada rodada. Ele não precisa de docker.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `dev/fixtures/entradas.json` | alterado (`acme-totp-dev` de D28; `_sobre`) |
| `crates/ward-core/examples/dev-cofre/main.rs` | criado |
| `crates/ward-core/examples/dev-cofre/testes.rs` | criado |
| `crates/ward-core/Cargo.toml` | alterado (`[[example]] dev-cofre`, `test = true`) |
| `dev/variacoes.py` | criado |
| `dev/testes/dev-cofre.sh` | criado |
| `Makefile` | alterado (`dev-cofre`, `dev-cofre-conferir`, `dev-variacoes`, `teste-dev-cofre`; stubs retirados) |
| `CLAUDE.md` | alterado ("Comandos": os quatro alvos como "pronto") |
| `docs/mission/handoffs/phase-19.md` | criado |
| `.dev/cofre-teste.ward`, `.dev/variacoes.txt` | gerados (fora do git) |

Não toquei em `crates/ward-core/src/` (área da Fase 19a) nem no `Cargo.lock`.

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): sai 2 por
  arquivos e testes da Fase 19a em andamento. clippy, audit, deny e gitleaks dão 0, os
  testes desta fase passam e o fmt dos arquivos dela está limpo
- [x] Só cofre de teste e credenciais falsas usados (valores de D15/D28; chave gerada
  em `.dev/`; nada fora do repositório foi lido; o cofre real não foi tocado nem
  consultado)
- [x] Teste de ataque para cada (SEG) tocado (V2.29/S15, S2, S9, S13)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase (com a exceção do diretório do scratchpad apagado,
  em "Problemas")
