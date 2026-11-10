# Fase 17 — Máscara em fluxo (D11)

## Implementado
- **`ward-core::mascara`** (público; sem dependência nova, sem `unsafe`, sem E/S):
  - `ConstrutorMascara::new().entrada(&e).entrega(&g)….construir() -> Result<Mascara, ErroMascara>`
    e o atalho `Mascara::da_entrada(&e, Option<&Entrega>)`. Aceita mais de uma
    entrada/entrega (salto e túnel: o daemon junta as duas entradas de D19).
  - Fontes dos padrões:
    - `Entrada::segredos()` (V2.13): campos secretos do tipo, extras secretos e anexos;
    - da `Entrega`, **só as variáveis com `e_segredo()`** (senha, token, `DATABASE_URL` e
      demais URLs com senha, passphrase). Host, porta, base, usuário, `WARD_TOTP` e
      caminhos de `WARD_ARQUIVO_*` não entram;
    - o usuário da entrada (login, banco, servidor, smtp, ftp/sftp) para o Basic auth.
  - Variações de cada segredo (D11 e um pouco além):
    - literal;
    - base64 padrão e URL-safe, com e sem `=`;
    - base64 de `usuário:senha`;
    - miolo do base64 do segredo nos 3 alinhamentos (só com 6+ bytes e miolo de 8+
      caracteres), que pega o segredo dentro de um base64 maior (`x-access-token:token`);
    - escape JSON (`\"`, `\\`, controle) e o do PHP (`\/`).
  - **Percent-encoding não vira padrão.** O fluxo lê a saída por três visões no mesmo
    autômato: crua, com `%XX` decodificado (qualquer caixa) e com `%XX` + `+`→espaço.
    O casamento numa visão decodificada volta para os bytes originais. Isso cobre
    maiúsculo, minúsculo, formulário e qualquer mistura (ex.: `urllib.parse.quote`,
    que deixa a `/` crua).
  - Anexos: conteúdo inteiro, sem a quebra final e seu base64; cada linha (sem `\r`) com
    6+ caracteres, e aparada; em linha `CHAVE=valor` / `export CHAVE=valor`, o valor
    (com e sem aspas), com todas as variações de segredo.
  - `Mascara::fluxo()` → `Fluxo` (um para stdout, outro para stderr):
    `mascarar(pedaco, &mut saida)` e `terminar(self, &mut saida)`; `mascarar_tudo` como
    atalho. `MARCA = "••••"`.
  - Busca por Aho-Corasick (`mascara/automato.rs`), linear no tamanho da saída. O fluxo
    segura só a profundidade do estado em cada visão (o fim que ainda pode ser começo de
    segredo) e um `%X` pela metade. O resto sai na hora.
  - Trechos cobertos que se sobrepõem ou se encostam viram um `••••` só, usando a união
    de todos os casamentos, inclusive os de sufixo. Assim não sobra nada de segredos
    contidos, colados ou sobrepostos.
  - `Mascara::tem_segredo_curto()`: algum segredo, fora os anexos, tem menos de 6
    caracteres. É para o app avisar (D11); o segredo é mascarado do mesmo jeito.
  - `LIMITE_PADROES = 8 MiB` (soma dos padrões). Acima dele `ErroMascara::GrandeDemais`:
    falha fechada.
- `lib.rs`: só a linha `pub mod mascara;` e um comentário.

## Segurança
- Invariantes tocadas:
  - **S4 (V2.17–V2.23):** a máscara cobre os segredos e as variações. O valor quebrado
    entre pedaços é coberto porque o fluxo nunca devolve um byte que ainda possa ser
    começo de segredo em alguma das visões (prova: o estado do autômato é o maior
    sufixo lido que é prefixo de padrão).
  - **S8 / V2.28:** o autômato (todas as tabelas), os buffers de geração e o pendente
    do fluxo ficam em `Zeroizing<Vec<_>>`. O crescimento passa por `reservar`, que copia
    para uma alocação nova e zera a antiga. O autômato é alocado uma vez, com a
    capacidade da 1ª passada. Ao tirar bytes da frente do pendente, a sobra é zerada.
    `Mascara` e `Fluxo` têm `ZeroizeOnDrop` e não têm `Clone`. Fluxo descartado sem
    `terminar` não devolve o que segurava. Por causa da S8 também não usei o crate
    `aho-corasick`: ele guardaria cópias dos segredos que não zeram.
  - **S13:** o `Debug` de `Mascara`, `Fluxo` e `ConstrutorMascara` é `…(••••)`, e
    `ErroMascara` só tem texto fixo.
  - **DoS pela saída:** com Aho-Corasick a saída do comando, que o agente controla, não
    consegue fazer o daemon gastar tempo quadrático.
- Testes (`crates/ward-core/src/mascara/testes.rs`), 28 no total:
  - Contrato V2.17–V2.24:
    - `v2_17_literal`;
    - `v2_18_cada_ponto_de_corte`: todo k em 2 pedaços, todo par (k, j) em 3 pedaços e
      byte a byte;
    - `v2_18_prefixo_no_fim_do_pedaco_fica_segurado`: o que sai antes do resto chegar
      não tem o começo do segredo; descartar o fluxo não solta nada;
    - `v2_19_percent_encoding`: maiúsculo, minúsculo, formulário, `quote`, misturado e
      letras codificadas, inteiro e byte a byte;
    - `v2_19_formulario_com_espaco`;
    - `v2_20_base64`: padrão e URL, com e sem `=`, Basic auth;
    - `v2_20_base64_url_safe_difere_do_padrao`;
    - `v2_21_database_url_inteiro`: URL codificado e decodificado, sem nenhum pedaço de
      4+ bytes da senha crua ou codificada;
    - `v2_22_segredo_que_contem_outro`: colados nas 5 ordens, e o menor dentro do começo
      do maior que depois diverge;
    - `v2_23_anexo_inteiro_linha_e_valor` e `anexo_com_varias_linhas`: aspas, `export`,
      CRLF, linha curta fora;
    - `v2_24_saida_sem_segredo_sai_igual`: texto, UTF-8 inválido, 1 MiB aleatório com
      escapes e prefixos, e o fim segurado (`falso-PG:s3`, `%4`, `%`), inteiro, em
      pedaços e byte a byte.
  - Propriedade: `propriedade_cortes_aleatorios`, 300 rodadas com ruído, variações
    sorteadas e cortes aleatórios. O resultado em pedaços é igual ao inteiro, não sobra
    variação nem pedaço da senha, e o número de marcas fica entre 1 e o número de
    inserções.
  - Ataques:
    - `ataque_segredos_sobrepostos`;
    - `ataque_segredo_dentro_de_base64_maior`;
    - `ataque_json_escapado`;
    - `ataque_saida_patologica_continua_linear`: segredo `a…a` de 64 KiB contra 1 MiB de
      `a` e contra `%61` repetido;
    - `ataque_anexo_grande_demais_falha_fechado`;
    - `ataque_debug_sem_segredo`.
  - Outros:
    - `entrega_marca_o_que_e_segredo`: host, porta, base e usuário passam;
    - `codigo_totp_nao_e_mascarado`: o código passa, a semente e a senha não;
    - `stdout_e_stderr_em_fluxos_separados`;
    - `salto_e_tunel_juntam_as_duas_entradas`;
    - `segredo_curto_e_mascarado_e_avisado`;
    - `mascara_e_fluxo_zeram_e_nao_clonam`: em tempo de compilação;
    - `automato_alocado_uma_vez`;
    - `base64_rfc4648`: vetores da RFC 4648 e o alfabeto URL;
    - `mascara_sem_segredo_nao_mexe_em_nada`.
- Mutações conferidas e revertidas, todas pegas:
  - só a visão crua: 4 testes falham, entre eles os de V2.19;
  - sem segurar o fim: 14 falham;
  - sem herdar a saída pela falha: falhava só depois que acrescentei o caso "menor
    dentro do começo do maior" ao V2.22, que agora falha;
  - sem Basic auth: falha o V2.20;
  - ignorando `e_segredo` da entrega: falha o V2.21.
- Dependências novas: nenhuma. Aho-Corasick e base64 foram feitos à mão porque não são
  criptografia, e uma biblioteca guardaria segredo em memória que não zera (S8). O
  base64 tem os vetores da RFC 4648, e o autômato tem os testes de propriedade e de
  mutação acima.

## Ficou de fora
- Ligar a máscara ao `ward run`/`ward ssh` (Fases 26/27). O daemon deve:
  - montar com a entrada, a entrega e a entrada do salto ou do túnel;
  - recusar o comando se `construir` falhar;
  - usar um `Fluxo` por stream e zerar o próprio buffer de leitura do pipe.
- O aviso de segredo curto na tela (Fase 33). Aqui só existe `tem_segredo_curto()`.
- O alvo `make dev-variacoes` (Fase 19). Ele pode gerar as variações pela mesma lógica,
  mas isso não foi exposto.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `cargo test -p ward-core --locked --lib mascara` (28 testes, ~1 s; `CARGO_TARGET_DIR` no scratchpad) | 0 |
| 5 mutações e reversão | 101 → 0 |
| `cargo clippy -p ward-core --all-targets --locked -- -D warnings` | 0 |
| `rustfmt --edition 2024 --check crates/ward-core/src/mascara.rs` (com os filhos) | 0 |
| `make check` | 2 (para no `fmt-check` só por `corpo.rs`, `corpo/testes.rs`, `corpo/cofre_teste.rs` e `tests/corpo_memoria.rs`, da Fase 15a em andamento) |
| `make clippy` / `make test` (148 ok, 1 ignorado) / `make audit` / `make deny` / `make gitleaks` | 0 / 0 / 0 / 0 / 0 |

## Problemas descobertos
- **`make check` não fecha por causa da Fase 15a**, que roda em paralelo: os arquivos
  `corpo*` não estão formatados. Não mexi neles. Precisa rodar de novo quando a 15a
  fechar.
- **Decisões minhas, para o orchestrator confirmar** (fui pelo lado mais seguro):
  - **Valor de `CHAVE=valor` em anexo sem tamanho mínimo.** D11 só exige 6 caracteres
    para a linha. Então `PORT=8080` faz todo `8080` da saída virar `••••`. É seguro, mas
    pode atrapalhar. Se quiserem um mínimo para o valor, é uma linha em
    `Gerador::anexo`.
  - **Limite de 8 MiB de padrões.** Acima dele a máscara recusa e o comando não roda.
    Na prática, um anexo acima de ~1 MiB deixa a entrada sem `ward run`, porque o
    anexo entra inteiro, sem a quebra final, em 4 formas de base64 e por linha (~7× o
    tamanho). O autômato gasta ~21 bytes por byte de
    padrão (até ~170 MB no limite). Anexos até 16 MiB (D25) cabem no cofre mas não na
    máscara; o app pode avisar ao salvar.
  - **Variações além de D11:** visões decodificadas, miolo de base64 em 3 alinhamentos,
    escape JSON/PHP. Elas mascaram mais e não deixam escapar nada da lista de D11.
  - Trechos colados ou sobrepostos viram um `••••` só, não um por segredo.
- **Saída interativa (`ward ssh`, Fase 27):** o fluxo segura o fim que pode ser começo
  de segredo até chegar mais saída ou até `terminar`. Um prompt que termina, por
  exemplo, com `f` (começo de `falso-…`) fica segurado. Não pus soltura por tempo,
  porque ela vazaria o prefixo. A Fase 27 decide (ex.: soltar após N ms mascarando o
  prefixo como `••••`).
- **Limites conhecidos** (fora de D11 e do modelo de ameaça, que exclui o processo
  feito para driblar a máscara):
  - base64 quebrado em linhas (`base64 -w76`) de um anexo grande;
  - hex, percent-encoding duplo (`%253A`) e entidades HTML;
  - metade do segredo no stdout e metade no stderr.
- O `Fluxo` guarda os deslocamentos (não secretos) num `VecDeque<u64>` comum. Com uma
  saída que casa um prefixo longo de um padrão enorme, ele chega a 8 bytes por byte
  segurado.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/mascara.rs` | criado |
| `crates/ward-core/src/mascara/automato.rs` | criado |
| `crates/ward-core/src/mascara/testes.rs` | criado |
| `crates/ward-core/src/lib.rs` | alterado (só `pub mod mascara;` e o comentário) |
| `docs/mission/handoffs/phase-17.md` | criado |

`Cargo.toml` e `Cargo.lock` não foram alterados. Também não toquei nos arquivos das
Fases 15a (`corpo*`, `formato*`, `gravacao*`) e 18 (`totp*`, `gerador*`).

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): sai 2 só no
  `fmt-check`, pelos arquivos `corpo*` da Fase 15a. Os outros passos fecham limpos, e o
  fmt dos arquivos desta fase está limpo.
- [x] Só cofre de teste e credenciais falsas usados (valores de D15; nada em `.dev/` tocado)
- [x] Teste de ataque para cada (SEG) tocado (V2.17–V2.23/S4, mais S8/V2.28 e S13)
- [x] (UI/DESIGN) não se aplica
- [x] Ficou dentro do escopo da fase
