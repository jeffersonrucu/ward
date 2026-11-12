# Fase 22c — Operações de valor do app e lista do app (D65 b, c)

## Implementado
- **`wardd` atende `revelar`, `copiar` e `totp` do papel app** (`crates/wardd/src/nucleo.rs`, `valor_do_app` e `totp_do_app`).
  - Exige papel app, conferido em três camadas: a tabela do protocolo, o `atender_de` e a própria função.
  - Exige cofre destravado. Não pede PIN, não abre janela de aprovação e não olha o nível, então a entrada bloqueada também é revelável (D4, D65 b).
  - Respostas de erro: travado → `travado`; entrada que não existe → `entrada_inexistente`; campo que o tipo não tem, campo vazio, extra comum ou extra inexistente → `campo_inexistente`.
  - O valor sai em `Resposta::Segredo` (`Segredo`, que zera ao sair). O TOTP sai em `CodigoTotp` com `Zeroizing`.
  - O log registra `revelar <entrada> <campo>`, `copiar <entrada> <campo>` ou `totp <entrada>`, sem valor. O nome do extra vai com escape `{:?}`. A doc de S13 em `crates/wardd/src/log.rs` foi atualizada.
  - O trait `Relogio` ganhou `segundos_unix()`. O padrão é `SystemTime`; os testes injetam o valor. Relógio ilegível responde `interno`, nunca um código.
- **`Alvo::segredo_em(&Entrada)`** (`crates/ward-core/src/protocolo/pedido.rs`) liga o identificador do protocolo (`senha`, `chave_privada`, `totp_semente`…) ou o extra secreto ao `Segredo` da entrada.
  - O mapeamento casa direto em `Dados`, sem comparar rótulo de texto.
  - O `wardd` e o daemon falso do app usam a mesma função. O mapeamento próprio do daemon falso foi removido.
- **`ItemLista.resumo: Option<ResumoItem>`** (`crates/ward-core/src/protocolo/resposta.rs`, D65 c).
  - `ResumoItem` tem `campos` (os mesmos campos de texto do `detalhe`), `segredos` (só os identificadores), `extras` e `anexos` (só as contagens).
  - É `Some` só para o papel app e segue a regra de D50 nos dois sentidos:
    - `codificar_resposta` recusa item com resumo para quem não é app;
    - `conferir` exige resumo no app e o proíbe no agente;
    - a leitura do agente recusa item com `resumo`, e a do app recusa item sem ele.
  - A conferência é a mesma do `detalhe`: id de texto conhecido, id de segredo conhecido e limites. A parte comum foi fatorada em `campos_e_segredos`, `campos_ok`, `escrever_campos` e `ler_campos`.
  - `MAX_NOS_RESPOSTA` (`json.rs`) passou a contar os nós do resumo (`MAX_NOS_RESUMO`).
- **App** (`app/nucleo`).
  - `ItemCredencial::novo(&ItemLista)` monta a 2ª linha e o usuário da busca a partir do resumo. Item sem resumo ou sem nível derruba a resposta inteira com `RespostaInvalida`.
  - `Servico::listar_credenciais` faz uma troca só, só a `lista`. Acabou o N+1 de `detalhe` por entrada.
  - O daemon falso já devolve o resumo, porque usa `ItemLista::de_entrada(App)`.
- **App de teste do `wardd`** (`crates/wardd/app-teste/main.rs`). Só quando recebe o papel app, ele faz, contra o cofre de teste:
  - revela a senha de `acme-pg-dev` e a de `gama-bloqueada`, copia, tira o TOTP, pede um campo inexistente, pede a lista, trava e revela travado;
  - imprime só `igual`, `diferente` ou o código. Nenhum valor sai na saída.
- **Margem do `v3_33_kill_9_entre_as_senhas_erradas`** (`crates/wardd/tests/socket.rs`).
  - A 1ª espera passou de < 900 ms para < 1500 ms.
  - Entrou uma asserção relativa: a 3ª espera ≥ a 1ª + 1200 ms. Assim continua provado que a 1ª tentativa não esperou 1 s.

## Segurança
- **Invariantes tocadas e como a fase as respeita:**
  - **S1:**
    - `revelar`, `copiar` e `totp` são só do app nas três camadas;
    - agente e extensão recebem `nao_autorizado` antes de qualquer campo, inclusive com PIN no pedido;
    - nenhuma resposta ao agente leva valor, nível ou resumo.
  - **S13:** o log tem operação, entrada e campo, nunca o valor. Conferido contra os segredos de `cofre_teste.rs`, contra o base64 deles e contra `.dev/variacoes.txt`.
  - **S8:** o valor fica em `Segredo` ou `Zeroizing` até o quadro, que também é `Zeroizing`.
  - **D50:** a lista do agente continua sem nível, agora também sem resumo e sem usuário.
  - **D4:** o app revela sem PIN e sem aprovação, inclusive a entrada bloqueada.
- **Testes de ataque escritos:**
  - `crates/wardd/tests/app_valor.rs` › `v3_39_v3_40_valores_e_lista_do_app_pelo_daemon_de_teste`. Usa o binário real (`modo-teste`) e as 22 entradas do cofre de teste:
    - um **cliente Python** agente destrava com a senha pelo stdin, nunca por argv;
    - outro cliente Python com `argv[0]=ward-nm`, fora do Chrome, faz o papel da "extensão";
    - os dois pedem `revelar`/`copiar` de `acme-pg-dev`, `gama-bloqueada` e do extra `CHAVE_LICENCA`, e `totp` de `acme-totp-dev` e `gama-bloqueada`: as 8 respostas são `nao_autorizado`, sem segredo;
    - a lista do agente vem sem `resumo`, `nivel`, `usuario` e sem a bloqueada;
    - o app de teste, aberto pelo `abrir_app`, recebe os valores certos, `travado` e `campo_inexistente`;
    - o log tem as linhas `revelar acme-pg-dev senha` e afins e nenhum padrão de segredo.
    - Mutação conferida: com o valor colocado no `aviso!`, o teste falha.
  - `crates/wardd/tests/valor_app.rs`:
    - `ataque_v3_39_agente_e_extensao_nao_revelam_nem_copiam_nem_tiram_totp`: pelo protocolo e direto no núcleo, para os papéis agente e extensão;
    - `v3_39_app_revela_e_copia_sem_pin_nem_aprovacao_ate_a_bloqueada`: campos inexistentes e travado;
    - `v3_39_totp_do_app_pelo_relogio_do_daemon`: relógio injetado e relógio ilegível;
    - `v3_40_lista_do_app_com_resumo_e_do_agente_sem_nivel_nem_resumo`.
  - `crates/ward-core/src/protocolo/testes.rs`:
    - `d65_lista_do_app_com_resumo_e_do_agente_sem`. Cobre quadro forjado com resumo lido como agente, item com resumo mandado ao agente, item do app sem resumo, resumo com `senha` em `campos`, com `valor`, com chave a mais e com id de segredo desconhecido;
    - `d65_alvo_acha_so_os_segredos_da_entrada`.
  - `app/nucleo/src/cliente.rs` › `ataque_d65_lista_sem_resumo_ou_com_segredo_no_resumo_e_recusada`.
  - `app/nucleo/src/servico.rs` › `d65_lista_numa_troca_so_com_resumo_e_usuario`: confere que a lista sai em uma troca só.
- **Dependências novas:** nenhuma. O teste usa `/usr/bin/python3`, já usado pelo `make check` (`dev/origens.py`).

## Ficou de fora
- **Interface (Svelte):** nada mudou. O formato de `ItemCredencial` é o mesmo. O `revelar_campo` do app é da Fase 33 e já chamava `cliente::revelar`; agora o daemon real responde.
- **Anexos:** continuam sem `Alvo` e não são reveláveis. A pendência das Fases 33/34 segue aberta.
- **`emitir_token`** segue respondendo `interno` (Fase 27).
- **Smoke manual com `make dev-daemon` + ponte** não foi feito. O teste de integração sobe o mesmo binário de teste, com o mesmo lançador e o cofre de teste.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make ferramentas` (2×: 504 do GitHub no download do gitleaks) | 2 |
| cópia do gitleaks do checkout principal + `make gitleaks-binario` (sha256 conferido) | 0 |
| `make dev-cofre` | 0 |
| `make dev-variacoes` | 0 |
| `cargo test -p ward-core --lib d65` | 0 |
| `cargo test -p wardd --all-features --test valor_app --test app_valor --test app_teste` | 0 |
| `cargo test -p ward-app-nucleo --all-features` | 0 |
| `make fmt` | 0 |
| `make check` (1ª: clippy `type_complexity` em `ler_campos`, corrigido) | 2 |
| `make check` | 0 |
| `make app-front` | 0 |
| `make app-check` (com `PKG_CONFIG_PATH`/`PKG_CONFIG_SYSROOT_DIR` do scratchpad, R32) | 0 |

## Problemas descobertos
- **Fora da lista de arquivos da fase**, por necessidade:
  - `crates/ward/src/{testes,texto}.rs` e `app/nucleo/src/daemon.rs`: só `resumo: None` nos literais de teste de `ItemLista`. Pode conflitar de leve com a Fase 26, que também mexe na CLI;
  - `app/nucleo/src/servico.rs`: `listar_credenciais` e um teste. A Fase 35 também mexe nesse arquivo;
  - `crates/wardd/app-teste/main.rs`;
  - `Alvo::segredo_em` em `protocolo/pedido.rs`. A fase dizia "só `ItemLista`/`ResumoItem`" no protocolo; preferi não duplicar o mapeamento id → segredo entre o `wardd` e o daemon falso.
- **Tamanho da lista do app.** `MAX_QUADRO_RESPOSTA` (2 × `MAX_CORPO` + 1 KiB por entrada) supõe o texto do corpo aparecendo uma vez. O resumo repete o texto (URL e host aparecem também em `endereco`). Num cofre perto de 64 MiB só de texto, a lista do app pode passar do limite. Nesse caso `codificar_resposta` recusa (`GrandeDemais` → o daemon responde `interno`): falha fechada, sem vazamento. Não mexi em `limites.rs`. Decidir se o limite sobe ou se o resumo deixa de repetir o endereço.
- **Log com nome de entrada.** O `log.rs` dizia que nenhuma chamada recebe valor de entrada. D65 b pede `revelar <entrada> <campo>`, e a doc foi ajustada com essa exceção. O nome vem do cofre, com ASCII restrito pela validação.
- **`make ferramentas` falhou duas vezes** com 504 do GitHub. Copiei o gitleaks 8.30.1 de `.dev/ferramentas/` do checkout principal; o Makefile confere o sha256 antes de cada uso.
- **O teste de integração usa `$XDG_RUNTIME_DIR`** (ou `/run/user/<uid>`) para o socket, como `socket.rs` e `app_teste.rs` já fazem. É o diretório de runtime do usuário, não o `/run` do sistema.
- **A "extensão" do V3.39** é simulada só por `argv[0]=ward-nm` com pai que não é o Chrome. Não há um Chrome falso como pai.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `crates/ward-core/src/protocolo.rs` | alterado |
| `crates/ward-core/src/protocolo/json.rs` | alterado |
| `crates/ward-core/src/protocolo/pedido.rs` | alterado |
| `crates/ward-core/src/protocolo/resposta.rs` | alterado |
| `crates/ward-core/src/protocolo/testes.rs` | alterado |
| `crates/wardd/src/nucleo.rs` | alterado |
| `crates/wardd/src/log.rs` | alterado |
| `crates/wardd/app-teste/main.rs` | alterado |
| `crates/wardd/tests/valor_app.rs` | criado |
| `crates/wardd/tests/app_valor.rs` | criado |
| `crates/wardd/tests/socket.rs` | alterado |
| `crates/ward/src/testes.rs` | alterado |
| `crates/ward/src/texto.rs` | alterado |
| `app/nucleo/src/cliente.rs` | alterado |
| `app/nucleo/src/credenciais.rs` | alterado |
| `app/nucleo/src/daemon_falso.rs` | alterado |
| `app/nucleo/src/daemon.rs` | alterado |
| `app/nucleo/src/servico.rs` | alterado |
| `docs/mission/handoffs/phase-22c.md` | criado |

## Procedimentos
- [x] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks)
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (V3.39, V3.40)
- [ ] (UI/DESIGN) Conferido contra o FigCraft: não se aplica (fase sem tela)
- [x] Ficou dentro do escopo da fase (exceções em "Problemas")
