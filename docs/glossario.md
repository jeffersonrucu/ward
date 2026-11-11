# Glossário pt-BR → inglês

Missão 2 (código em inglês), Fase 1. Fonte única das traduções: toda fase que renomeia
usa esta tabela, para que o mesmo termo vire sempre a mesma palavra (PRD regra 3). Nome
novo que não está aqui entra aqui na mesma fase que o cria.

Inventário feito em 2026-11-10 sobre o checkout em `master` (0.1.4), fora de `docs/`,
`.claude/`, `skill/**/*.md`, `README.md`, lockfiles e binários: identificadores de Rust,
JS, Svelte, CSS, Python, shell, Makefile, TOML e YAML (sem comentários nem literais),
nomes de arquivo e pasta, e os nomes externos lidos no código (protocolo, cofre, estado,
Tauri, native messaging, CLI, variáveis de ambiente, Makefile).

## 1. Como usar

- **Uma tradução por termo e por sentido.** A seção 2 tem os termos. Quando um termo tem
  dois sentidos no código (estado do protocolo × estado interno, papel × papel de
  impressão, usuário pessoa × campo usuário), a linha separa os sentidos.
- **Ordem do inglês.** As construções com "de" invertem: `cabecalho_de_grupo` →
  `group_header`, `PedidoDeAprovacao` → `ApprovalRequest`, `TipoEntrada` → `EntryType`,
  `nome_variavel` → `variable_name`.
- **Forma.** Rust: `snake_case` para funções, módulos e variáveis, `CamelCase` para tipos,
  `SCREAMING_CASE` para constantes. JS/Svelte: `camelCase` e componentes em `PascalCase`.
  Arquivos de script e de pasta: `kebab-case` onde já era (`dev-cofre.sh` →
  `dev-vault.sh`), `snake_case` onde já era (`socket_ataque.py` → `socket_attack.py`).
- **`tipo` em Rust e JS** vira `kind` em nome de campo ou variável (`type` é palavra
  reservada do Rust), `Type` em nome de tipo (`TipoResposta` → `ResponseType`) e `"type"`
  na chave JSON do protocolo v2 e do `ward list --json`.
- **Nomes de fio** (seções 7, 8 e 10–13) ficam congelados até a fase que os muda (PRD regra 4,
  DT3): o identificador Rust/JS muda antes, o literal não. Campo `serde` renomeado leva
  `#[serde(rename = "<antigo>")]` até a Fase 54; nome que o checador acusa vai para
  `dev/english-check/frozen.txt` com a fase que o muda.
- **Literais** exibidos ao usuário não se traduzem (PRD regra 5). Os rótulos pt-BR dos
  tipos e níveis ("banco", "com aprovação", "nota segura") continuam na interface e na
  tabela da CLI.
- **Dados do usuário** não se traduzem: nomes de entrada (`acme-pg-dev`, `gama-generico`),
  grupos, nomes de extra e de anexo e o `WARD_<CAMPO>` que vem deles (`WARD_REGIAO`,
  `WARD_CHAVE_LICENCA`, `WARD_ARQUIVO_APP_ENV` vira `WARD_FILE_APP_ENV` só pelo prefixo),
  e os valores falsos do cofre de teste (`falso-…`).
- **IDs nos comentários** (DT2): `Dnn`, `RAnn`, `Vn.n`, `Mn` e "Fase nn" sem prefixo são
  da missão 1; "Fase" vira "Phase" e o ID fica. ID desta missão leva `m2:` (`m2:V4.3`).
- **Palavras raras** dos nomes de teste em frase (contagem 1–2 no inventário) se traduzem
  pelo sentido; só entram aqui se virarem termo recorrente.
- **Lista do checador**: `dev/english-check/pt-words.txt` (radicais, palavras, palavras
  funcionais e ambíguas), formato descrito no cabeçalho dele.

## 2. Termos

Um termo pt-BR por linha, com as flexões que aparecem no código, e a tradução única. Quando
o mesmo termo tem dois sentidos no código, a linha diz o sentido de cada tradução; fora
desses casos, a tradução é sempre a mesma. Dois termos pt-BR podem ter a mesma tradução
(gravar e escrever → write) quando não convivem no mesmo escopo; se convivem, a fase usa
a forma composta indicada.

| pt-BR | inglês | nota |
|---|---|---|
| aba, abas | tab, tabs | aba do navegador |
| abaixo | below | |
| abrir, abre, abriu, aberto(s), aberta(s) | open, opens, opened, open | `abrir_app` → `open_app` |
| abertura, aberturas | opening, openings | |
| abridor | opener | `AbridorApp` → `AppOpener` |
| absoluto | absolute | |
| ação, ações (acao) | action, actions | |
| AcaoDeCampo | FieldAction | componente |
| aceitar, aceita, aceito(s), aceitas | accept, accepts, accepted | |
| acento | accent | |
| acesso | access | nível de acesso → access level |
| achar, achou, achado(s), achadas | find, found, finding(s) | `achados` (do checador/varredura) → findings |
| acima | above | |
| acionar | trigger | |
| aconselhar | advise | |
| acordar, acordou | wake, woke | |
| acrescentar | append | |
| acumulado | accumulated | |
| acusar, acusados | flag, flagged | checador/varredura |
| adicionar | add | |
| adulterado(a) | tampered | |
| agente, agentes | agent, agents | papel `agente` → `agent` |
| agente SSH (agente_ssh) | ssh agent (ssh_agent) | |
| agora | now | |
| agrupar | group | |
| aguardar | await | `aguardar_login` → `await_login` |
| agulha, agulhas | needle, needles | busca da máscara |
| ainda | still / yet | |
| ajuda | help | subcomando `ajuda` some (só `help`) |
| ajustar, ajustes | adjust, adjustments | |
| aleatório(s), aleatória | random | `AleatorioIndisponivel` → `RandomUnavailable` |
| aleatoriedade | randomness | |
| além | beyond | |
| alfabeto | alphabet | |
| algoritmo | algorithm | |
| algum | some / any | |
| alterar, alterado(a), alterada_em | modify, modified, modified_at | campo de data `alterada_em` → `modified_at` |
| altura | height | `--altura-linha-*` → `--line-height-*` |
| alvo, alvos | target, targets | `Alvo` (pedido revelar/copiar) → `Target` |
| âmbar (ambar) | amber | token |
| ambiente, ambientes | environment, environments | ambiente da entrada (dev/stage/prod) e variáveis do `run`; abreviado `amb` → `env` |
| ambíguo | ambiguous | |
| ampliar | widen | |
| analisar | parse | |
| andar | walk | |
| anexo, anexos, anexar, anexado | attachment, attachments, attach, attached | |
| ano | year | |
| anotador | annotator | |
| anterior(es) | previous | campo `anterior` → `previous` |
| antigo(a) | old | nomes antigos → old names |
| apagar, apaga, apagam, apagado(s), apagadas | delete, deletes, deleted | |
| aparar, aparado(a) | trim, trimmed | |
| aparecer, aparece | appear, appears | |
| aparência | appearance | |
| aplicar, aplicadas | apply, applied | |
| apontar, apontam | point, point to | |
| aprovação, aprovações | approval, approvals | nível `com-aprovacao` → `approval` |
| aprovar, aprova, aprovado(s), aprovadas | approve, approves, approved | |
| aprovador | approver | `wardd-aprovador-teste` → `wardd-test-approver` |
| aproximado | approximate | |
| aqui | here | |
| área (de transferência) | clipboard | `area_transferencia` → `clipboard`; `area_gtk` → `gtk_clipboard`; `area_wl` → `wl_clipboard`; o CSS `area` do HTML não muda |
| argumento(s) | argument(s) | |
| armado | armed | |
| arquivo, arquivos (arq, arqs) | file, files | `WARD_ARQUIVO_*` → `WARD_FILE_*`; entrada padrão `arquivo` → `file` |
| arrasto | drag | |
| arroba | at sign | |
| árvore (arvore) | tree | `arvore.txt` → `tree.txt`; CDP `frameTree` |
| aspa, aspas | quote, quotes | |
| assinar, assinado, assinou, assinatura | subscribe, subscribed, subscription | sinais do D-Bus e avisos; `desassinar` → `unsubscribe` |
| assinante | subscriber | |
| atacante | attacker | |
| ataque, ataques | attack, attacks | |
| atender | serve | atender conexão |
| ativar, ativa(s), ativo, ativação | activate, active, activation | `ativacao.rs` → `activation.rs` |
| atraso(s), atrasada | delay(s), delayed | |
| atributo | attribute | |
| atual | current | campo `atual` → `current` |
| atualizar | update | |
| ausente | missing | |
| autenticação, autenticações (autent) | authentication(s) (auth) | `Autenticacao` → `Auth` |
| autêntico(a), autênticas | authentic | |
| autômato (automato) | automaton | `mascara/automato.rs` → `mask/automaton.rs` |
| autoridade | authority | |
| autorização, autorizar, autorizado | authorization, authorize, authorized | erro `nao_autorizado` → `unauthorized` |
| autoteste | self-test | `--autoteste` → `--self-test` |
| auxiliar(es) | helper(s) | |
| avaliar, avaliação | evaluate, evaluation | CDP |
| avançar | advance | |
| avisar, avisou | notify, notified | |
| aviso, avisos | notice, notices; warning (cor) | aviso à tela (`ouvir_avisos`) → notice; `--cor-aviso-*` e níveis de log → warning |
| azul | blue | token |
| baixo | low | |
| banco (de dados) | database | tipo `banco` → `database` |
| bandeja | tray | |
| barra, barras | bar, slash | barra lateral → sidebar; `/` → slash |
| barramento | bus | D-Bus |
| base (do banco) | db_name | campo `base` → `db_name`; "base" de tokens/css (`--tamanho-base`) → base |
| biblioteca(s) | library, libraries | |
| binário(a) | binary | |
| bisneto | great_grandchild | |
| bloco(s) | block(s) | |
| bloqueado(a), bloqueou | blocked | nível `bloqueado` → `blocked` |
| bom | good | |
| bombear | pump | |
| borda | border | |
| botão, botões | button, buttons | |
| branco | white | |
| brilho | glow | token |
| bruto(a), brutos | raw | |
| busca | search | |
| cabe, caber | fits, fit | |
| cabeça | head | |
| cabeçalho(s) | header(s) | |
| cada | each | |
| cadastro | registration | |
| cadeia | chain | cadeia de saltos/processos |
| cair, cai, caiu | drop, drops, dropped | conexão que cai |
| caixa(s) | box(es) | `CaixaDeCodigo` → `CodeBox` |
| camadas | layers | |
| caminho(s) | path(s) | |
| campo(s) | field(s) | |
| canal, canais | channel(s) | |
| cancelar, cancela, cancelado | cancel, cancels, canceled | |
| candidato(s) (cand) | candidate(s) | |
| capacidade | capacity | |
| capturar, captura | capture | |
| caractere(s) | character(s) | |
| carga(s) | load(s) | |
| carimbo | stamp | carimbo de hora → timestamp |
| carregar, carrega, carregado(a)(s), carregadas, carregando, carregou, carregamento | load, loads, loaded, loading | |
| carregáveis | loadable | |
| cartão, cartões | card(s) | |
| casar, casa | match, matches | casar padrão |
| caso(s) | case(s) | |
| cena, cenário | scene, scenario | |
| centro | center | |
| certo(a) | right / correct | |
| chamar, chamada(s), chamou | call, calls, called | |
| chave, chaves | key, keys | chave privada → private_key; chave de dados → data_key |
| checar | check | |
| chegar, chega, chegam, chegou, chegada(s) | arrive, arrives, arrived, arrival(s) | |
| cheio(a)(s) | full | |
| ciano | cyan | token |
| ciente | aware | parâmetro `ciente` do Tauri → `acknowledged` |
| cifra, cifrar, cifrado(a) | cipher, encrypt, encrypted | |
| claro (em claro) | plaintext | segredo em claro → plaintext secret; "claro" de tema → light |
| classe | class | |
| classificar | classify | |
| cliente | client | |
| cobrir, cobre, cobertos | cover, covers, covered | |
| codificar, codificado | encode, encoded | |
| codificador, decodificador | encoder, decoder | |
| código(s) | code(s) | código TOTP → TOTP code; código de saída → exit code |
| cofre | vault | |
| cofre de teste | test vault | |
| colado, colar | pasted, paste | |
| coleção, coleções | collection(s) | |
| coletor | collector | |
| colher, colhido | collect, collected | |
| coluna(s) | column(s) | tamanho do terminal `colunas` → `cols` |
| comando(s) | command(s) | |
| combinado | combined | |
| comentário | comment | |
| compacto(a) | compact | |
| comparar | compare | |
| completo(a) | complete | |
| componente(s) | component(s) | `lib/componentes` → `lib/components` |
| composta | composite | |
| comum, comuns | common | `comum.sh` → `common.sh`; `tests/comum/` → `tests/common/` |
| conectar, conectado, conectou, conectando | connect, connected, connecting | botão "Conectar" (texto) fica |
| conexão, conexões | connection(s) | `CONEXOES` → `CONNECTIONS` |
| conferir, confere, conferido(a)(s) | verify, verifies, verified | `reconferir` → `reverify`; "conferência" → verification |
| confiável | trusted | não confiável → untrusted |
| configuração (do cofre) | config | op `configuracao` → `config`; `configuracao.rs` → `config.rs` |
| configurações (tela do app) | settings | `Configuracoes.svelte` → `Settings.svelte` |
| configurar, configura | configure, configures | |
| confirmar, confirma, confirmação, confirmada | confirm, confirms, confirmation, confirmed | |
| conhecido(a)(s) | known | |
| conjunto(s) | set(s) | |
| constante | constant | |
| construir, construtor | build, builder | |
| consultar, consulta(s), consultando | query, queries | |
| conta, contas | account, accounts | `WARD_CONTAS` → `WARD_ACCOUNTS` |
| contar, contam, contando, contagem, contador | count, counting, count, counter | componente `Contagem` → `Countdown` |
| contato | contact | token de elevação |
| contém (contem) | contains | |
| contêiner (conteiner) | container | `dev/conteiner-*` → `dev/container-*` |
| conteúdo | content | |
| contexto | context | |
| continuar, continua | continue, continues | |
| contrato | contract | |
| controle | control | `ControleDeslizante` → `Slider` |
| conversa, conversar | conversation, talk | |
| copiar, cópia(s), copiado, copiadas | copy, copies, copied | |
| copiador | copier | |
| cor | color | `--cor-*` → `--color-*` |
| corpo | body | corpo do cofre |
| corte, cortado(a) | cut | |
| credencial, credenciais | credential(s) | |
| criar, criado(a), criada_em | create, created, created_at | |
| cripto | crypto | |
| cru, crua(s), crus | raw | modo cru do terminal → raw mode |
| cruzado | cross | |
| curto(a) | short | |
| custo | cost | Argon2id |
| cutucar, cutucão, cutucões | poke, poke, pokes | |
| dados | data | envelope `dados` → `data` |
| datas | dates | |
| de (prefixo de construtor: `de_nome`, `de_bytes`) | from | `de_nome` → `from_name`; `de_bytes` → `from_bytes` |
| decidir, decisão | decide, decision | |
| decodificar | decode | |
| decomposta | decomposed | |
| decorrido | elapsed | |
| definir, definições | set, definitions | `definir_backup_externo` → `set_external_backup` |
| deixar, deixa | leave, leaves | |
| delegado | delegate | |
| demais | too many / other | `GrandeDemais` → `TooLarge` |
| dentro | inside | `dentro.sh` → `inside.sh` |
| depois | after | |
| depurar, depuração, depurador | debug, debugging, debugger | |
| derivar | derive | |
| desabilitado(a) | disabled | |
| descartar, descarta, descartados | discard, discards, discarded | |
| descendentes | descendants | |
| desconhecido(a) | unknown | erro `versao_desconhecida` → `unknown_version` |
| descrever, descrição | describe, description | |
| desde | since | |
| desenho | drawing | |
| desfazer | undo | |
| desfecho(s) | outcome(s) | |
| desfoque | blur | token |
| desinstalar | uninstall | |
| desligado, desligar | off, turn_off | backup externo ligado/desligado → enabled/disabled |
| deslizante | slider | |
| deslocamento (desloc) | offset | |
| desserializar | deserialize | |
| destaque | highlight | `Destaque.svelte` → `Highlight.svelte`; `--cor-ambiente-prod-destaque` → `--color-environment-prod-highlight` |
| destino | destination; recipient | `WARD_DESTINO_REDIR` → `WARD_REDIR_TARGET`; quem recebe algo do guia de migração (`DESTINO`, `FRASES_DESTINO` no `migration-guide.py`, Fase 41) → recipient |
| destravar, destrava, destravado | unlock, unlocks, unlocked | subcomando `destravar` → `unlock` |
| destrave | unlock | `pedido_de_destrave` → `unlock_request` |
| detalhe(s), detalhado | detail(s), detailed | |
| deve | must | |
| dia(s) | day(s) | |
| diálogo | dialog | |
| dica | hint | |
| dicionário (dic) | dictionary (dict) | |
| diferente(s) | different | erro `origem_diferente` → `origin_mismatch` |
| difícil | hard | |
| digitar, digitado(a), digitável | type, typed, typeable | |
| dígito(s) | digit(s) | |
| direção | direction | |
| direto | direct | |
| diretório | directory | |
| disco | disk | |
| disfarçada | disguised | |
| disparar | fire | |
| dispensado | waived | |
| disponível, indisponível, indisponíveis | available, unavailable | |
| dispositivo | device | |
| documento | document | |
| dois, duas | two | |
| domínio (dom) | domain | |
| dono(s) | owner(s) | |
| dormir | sleep | |
| dupla(s) | pair(s) | |
| dura | lasts | |
| edição, editar, editada, editando, editável | edit, edit, edited, editing, editable | |
| efetivo | effective | uid efetivo → effective uid |
| elemento | element | |
| elevação, elevada | elevation, raised | `--elevacao-*` → `--elevation-*`; superfície elevada → raised surface |
| embrulhada | wrapped | chave embrulhada → wrapped key |
| emergência (emerg) | emergency | `KitEmergencia` → `EmergencyKit` |
| emitir, emitido | issue, issued | `emitir_token` → `issue_token` |
| emprestado | borrowed | |
| empurrar | push | |
| emulador | emulator | |
| encaminhar | forward | |
| encerrar | terminate | |
| enchimento | padding | |
| endereço | address | `endereco` (lista) → `address` |
| endurecer, endurecido, endurecimento | harden, hardened, hardening | |
| entorno | environment (de teste) | trait `Entorno` → `Env`; não confundir com ambiente (environment) |
| entrada (do cofre), entradas | entry, entries | |
| entrada (padrão) | stdin | op `entrada_padrao` → `stdin`; `TipoEntradaPadrao` → `StdinKind` |
| entrada (dado recebido, não confiável) | input | "entrada não confiável" → untrusted input |
| entrar, entra, entram, entrou | enter, enters, entered | |
| entre | between | |
| entrega, entregas, entregar, entregue | delivery, deliveries, deliver, delivered | |
| enviar, enviado, enviando, envio(s) | send, sent, sending, submission(s) | `envio.js` → `submit.js`; resultado de login `enviado` → `submitted` |
| errar, errado(a) | miss, wrong | |
| erro(s) | error(s) | |
| escapar, escapa, escapado(a)(s) | escape, escapes, escaped | |
| escolher, escolha, escolhido(a)(s), escolhendo | choose, choice, chosen, choosing | |
| esconder | hide | |
| escopo | scope | |
| escravo | slave | lado do pty |
| escrever, escreve, escrevendo, escrita(s), escrito(s) | write, writes, writing, write(s), written | |
| escritor, escritores | writer(s) | |
| escuro | dark | tema e `--primitiva-sobreposicao-escuro` |
| escutar, escuta(s), escutando | listen, listener(s), listening | |
| esgotado(a)(s) | exhausted | `geracao_esgotada` → `generation_exhausted`; `tempo esgotado` → timed out |
| espaçamento | spacing | `--espacamento-letra-*` → `--letter-spacing-*` |
| espaço | space | `--espaco-*` → `--space-*` |
| esperar, espera(s), esperado(a)(s), esperando | wait, wait(s), expected, waiting | |
| espião, espiar | spy, spy | |
| esquema | schema | |
| estado (do protocolo e da tela) | status | op `estado` → `status`; `estado_do_cofre` → `vault_status` |
| estado (interno, arquivo de estado) | state | arquivo `estado` (nome no disco fica) → state file; `Estado` do Tauri → `AppState` |
| estático | static | |
| estilo | style | `lib/estilo` → `lib/style` |
| estourar, estourou | overflow, overflowed | |
| estranho | odd | |
| estrito | strict | |
| estrutura | structure | |
| esvaziar | drain | |
| etapa | step | |
| etiqueta | tag | só a classe `.texto-etiqueta` → `.text-tag` |
| evento(s) | event(s) | |
| evidência (evid) | evidence | |
| exato | exact | |
| exceção, exceções | exception(s) | |
| executar, executado | run, ran | op `executar` → `run`; `PedidoExecutar` → `RunRequest` |
| execução | execution | `execucao.rs` → `execution.rs` |
| executável, executáveis | executable(s) | |
| exemplo | example | |
| exibir, exibição, exibido(a) | display, display, displayed | `exibicao.rs` → `display.rs` |
| exigir, exige | require, requires | `WARD_EXIGIR_OATHTOOL` → `WARD_REQUIRE_OATHTOOL` |
| existir, existe, existente | exist, exists, existing | |
| expirar, expirado(s), expirando | expire, expired, expiring | |
| explicação | explanation | |
| expor | expose | `Segredo::expor` → `Secret::expose` |
| extensão, extensões (extens) | extension(s) | papel `extensao` → `extension` |
| externo(a) | external | |
| faixa(s) | range(s) | |
| falha(s), falhar, falhou | failure(s), fail, failed | `FalhaCripto` → `CryptoFailure`; `falha_ao_gravar` → `write_failed` |
| falso(a)(s) | fake | `daemon_falso` → `fake_daemon`; valores "falso-…" do cofre de teste são dados e ficam |
| faltar, falta, faltam, faltando, faltou | lack, missing | |
| família(s) | family, families | `--familia-*` → `--font-family-*` |
| fantasma | ghost | |
| fase | phase | "Fase nn" nos comentários → "Phase nn" (DT2) |
| fazer, faz, feito | do, does, done | |
| fechar, fecha, fechado(a), fechou, fechamento | close, closes, closed, closing | |
| ferramenta(s) | tool(s) | alvo `ferramentas` → `tools` |
| ficar, fica, ficam, ficou | stay, stays, stayed | |
| fila | queue | |
| filho(s), filhas | child, children | |
| filtrar, filtro(s), filtrando | filter, filters, filtering | |
| fim | end | resposta `fim` → `exit` (fim da execução) |
| fino(a) | thin | `--linha-fina` → `--line-thin` |
| fio(s) | wire(s) | "fio congelado" → frozen wire |
| fixo(a)(s) | fixed | |
| flutuante | floating | |
| fluxo | stream | `Fluxo` (saída/erro) → `Stream` |
| focar, foco | focus | |
| folha(s) | leaf, leaves | |
| fonte(s) | source(s); font(s) | código-fonte → source; tipografia → font |
| força, forças | strength(s) | força da senha |
| forçado(s) | forced | |
| forjar, forja, forjado(a)(s), forjador | forge, forges, forged, forger | |
| forma(s) | form(s) / shape(s) | formas de texto do segredo → forms |
| formato | format | |
| formulário (formul) | form | `Formulario.svelte` → `Form.svelte` |
| forte | strong | |
| fraco(a) | weak | |
| frase(s) | sentence(s) | |
| fugitivo | runaway | |
| função | function | |
| fundo | background | `--cor-fundo-*` → `--color-bg-*` |
| fuso | timezone | |
| futuro(a)(s) | future | |
| gancho(s) | hook(s) | |
| ganhar, ganha | gain, gains | |
| garantia(s) | guarantee(s) | |
| genérico | generic | tipo `generico` → `generic` |
| geometria | geometry | |
| geração, gerações | generation(s) | geração do cofre (D39) |
| gerar, gerado(a), gerando | generate, generated, generating | |
| gerador | generator | |
| geral | general | |
| gerente | manager | `WARD_GERENTE_PROGRAMA` → `WARD_MANAGER_PROGRAM` |
| grande(s) | large | |
| grau | degree | |
| gravar, grava, gravado, gravável | write, writes, written, writable | gravar no disco; o módulo `gravacao` → `atomic_write` |
| gravação, gravações | write, writes | gravação atômica → atomic write |
| grosso(a) | thick | `--linha-grossa` → `--line-thick` |
| grupo(s) | group(s) | |
| guardar, guarda, guardado(s), guardadas | keep, keeps, kept | o substantivo "guarda" (conferência antes de um comando, `migration-guide.py`, Fase 41) → guard |
| habilitado(a) | enabled | |
| herdar, herdado(a), herdável | inherit, inherited, inheritable | |
| hora | hour | `uma_hora` → `one_hour` |
| hostil | hostile | |
| ícone(s) | icon(s) | |
| idade(s) | age(s) | |
| identidade | identity | |
| identificação, identificar | identification, identify | |
| ignorar | ignore | |
| igual, iguais | equal | |
| ilegível | unreadable | |
| imagem | image | |
| implementar, implementa | implement, implements | |
| impressão, impressa, imprimir | print, printed, print | `--cor-impressao-*` → `--color-print-*` |
| inatividade, inativo | inactivity, inactive | |
| inconsistente | inconsistent | |
| incorreto(a) | incorrect | `senha_incorreta` → `wrong_password`; `pin_incorreto` → `wrong_pin` |
| indicador | indicator | `IndicadorDePassos` → `StepIndicator` |
| índice | index | |
| inesperado | unexpected | |
| inexistente | not_found | erros `*_inexistente` → `*_not_found` (`entrada_inexistente` → `entry_not_found`); em texto, "não existe" |
| infinito | infinite | |
| inflado | inflated | |
| inibir, inibidor(es) | inhibit, inhibitor(s) | logind |
| iniciar, inicial, iniciais, início | start, initial, start | |
| injetar | inject | |
| inseguro(a)(s) | insecure | |
| inserido(a)(s) | inserted | |
| instalar, instalado, instalação | install, installed, install | `instalacao/` → `install/` |
| instância | instance | |
| instante | instant | |
| inteiro(a) | whole; integer | número inteiro → integer; "segredo inteiro" → whole |
| interessante(s) | interesting | |
| intermediário | intermediate | |
| interno | internal | erro `interno` → `internal` |
| interpretar | interpret | |
| interruptor | switch | `Interruptor.svelte` → `Switch.svelte` |
| intervalo | interval | |
| inválido(a)(s) | invalid | |
| inventado | invented | |
| invertido(a)(s) | inverted | |
| invisível | invisible | |
| irmão | sibling | |
| isca(s) | decoy(s) | `iscas-home.sh` → `home-decoys.sh` |
| isolado | isolated | |
| item, itens | item, items | |
| janela | window | janela da interface; op `janela` (tamanho do terminal) → `resize` |
| juntar, junto | join, together | |
| kit de emergência | emergency kit | |
| laço | loop | |
| lado | side | |
| lançar, lançado, lançamento | launch, launched, launch | `lancamento.rs` → `launch.rs` |
| lançador, lançadores | launcher(s) | |
| largar | release | |
| largo | wide | |
| largura | width | `--largura-*` → `--width-*` |
| legenda | caption | |
| legível | readable | |
| leitor, leitores, leitura(s), ler, lido(a)(s) | reader(s), read(s), read, read | |
| letra | letter | |
| levar, leva | carry, carries | |
| liberar, liberação, liberações | release | |
| licença | license | |
| líder, líderes | leader(s) | líder de sessão/grupo |
| ligar, liga, ligado(s) | enable, enables, enabled | backup ligado → enabled; "ligar" de rede (bind) → bind |
| limite(s), limitado | limit(s), limited | |
| limpar, limpa, limpo(s), limpeza(s) | clear, clears, clean, cleanup(s) | |
| linha(s) | line(s) | tamanho do terminal `linhas` → `rows` |
| lista(s), listar | list(s), list | op `lista` → `list` |
| listável | listable | |
| livre | free | nível `livre` → `free` |
| lixo(s) | garbage | |
| local, locais | local | |
| lógico | boolean | valor JSON lógico → boolean |
| longo(a), longe | long, far | |
| lote | batch | |
| lugar | place | |
| magia | magic | `WARDCOFR` fica |
| maior, menor | larger, smaller | |
| mais, menos | more, less | |
| maiúsculas, minúsculas, minúsculo | uppercase, lowercase | |
| mal, mau, ruim | bad | |
| mandar | send | |
| manter, mantém, mantidos | keep, keeps, kept | `manter_anexos` → `keep_attachments` |
| mapa(s), mapeado | map(s), mapped | |
| marca, marcas, marcar, marcado(s) | mark(s), mark, marked; brand | sinal/marcador → mark; identidade visual (`--cor-marca-*`, `PainelDeMarca`) → brand |
| máscara, mascarar, mascarado | mask, mask, masked | |
| matar, mata, morre, mortes, mortos | kill, kills, dies, deaths, dead | |
| máximo, mínimo | max, min | |
| medir, médio | measure, medium | `--peso-medio` → `--weight-medium` |
| meio, metade | middle, half | |
| melhor, pior | best, worst | |
| membro(s) | member(s) | |
| memória | memory | |
| mensagem, mensagens | message(s) | |
| mesmo(a)(s) | same | |
| mestra, mestre | master | senha mestra → master password |
| metadados | metadata | |
| método | method | |
| meu, minha, nosso | my, our | |
| milhar | thousands | |
| minutos | minutes | `minutos_trava` → `lock_minutes` |
| miolo(s) | core(s) | miolo do segredo (máscara) |
| misto | mixed | |
| modelo | model / template | |
| modo(s) | mode(s) | modo de teste → test mode; `WARD_MODO` → `WARD_MODE` |
| monotônico | monotonic | |
| montar, monta, montado(a)(s), montagem | build, builds, built, assembly | montar a entrega/o comando → build; pendrive montado → mounted; "montado por partes" → built in pieces; a montagem da skill antiga (`montagem.md`, `MONTAGEM` no `migration-guide.py`, Fase 41) → setup |
| mostrar, mostra, mostrado | show, shows, shown | |
| motivo | reason | |
| motor, motores | engine, engines | campo `motor` → `engine` |
| móvel | movable | |
| mudar, muda, mudado(a)(s), mudam, mudando, mudou | change, changes, changed, changing | op `mudar_nivel` → `set_level`; erro `cofre_mudou` → `vault_changed` |
| muito(s) | very, many | força `muito-forte` → `very-strong` |
| nada, nenhum(a), ninguém | nothing, none, nobody | segurança SMTP `nenhuma` → `none` |
| navegação, navegar | navigation, navigate | `NavegacaoLateral` → `SideNav` |
| navegador | browser | erro `navegador_falhou` → `browser_failed` |
| negar, nega, negado | deny, denies, denied | decisão `negar` → `deny`; erro `negado` → `denied` |
| negrito | bold | `--peso-negrito` → `--weight-bold` |
| neto(s) | grandchild(ren) | |
| neutralizar | neutralize | |
| neutro | neutral | token |
| nível, níveis | level(s) | `NivelAcesso` → `AccessLevel` |
| noite | night | |
| nome(s) | name(s) | |
| normalizar | normalize | |
| nota (segura) | (secure) note | tipo `nota-segura` → `secure-note` |
| novo(a)(s) | new | campo `novo`/`nova` (troca) → `new` |
| núcleo | core | `app/nucleo` → `app/core`; `nucleo.rs` do wardd → `core.rs` |
| nulo(a) | null | entrada padrão `nula` → `null` |
| número(s) | number(s) | |
| objeto(s) | object(s) | |
| obrigatório | required | |
| observador | observer | |
| obter, obtido | get, obtained | |
| ocioso(a) | idle | |
| oculto(a) | hidden | `entrada_oculta` → `hidden_entry`; `.visualmente-oculto` → `.visually-hidden` |
| ocupado | busy | erro `ocupado` → `busy` |
| olhar | look | |
| opção, opções (opc) | option(s) | |
| opcional | optional | |
| operação, operações | operation(s) | |
| oráculo | oracle | |
| orçamento | budget | |
| ordem, ordenado(a)(s) | order, sorted | `fora_de_ordem` → `out_of_order` |
| órfão, órfãos | orphan(s) | |
| origem, origens | origin(s) | |
| original, originais | original | |
| ouvir, ouvinte | listen, listener | `ouvir_avisos` → `listen_notices` |
| pacote | package | |
| padrão, padrões (padr) | default; pattern(s) | valor padrão/token `-padrao` → default; padrão de busca (máscara, variações) → pattern |
| página(s) | page(s) | `pagina.js` → `page.js` |
| pai | parent | |
| painel | panel | |
| palavra(s) | word(s) | |
| palheiro | haystack | |
| pânico | panic | |
| papel (do protocolo), papéis | role(s) | `Papel` → `Role`; `papel.rs` → `role.rs` |
| papel (de impressão) | paper | `--cor-impressao-papel` → `--color-print-paper` |
| parar, parado(s) | stop, stopped | alvo `dev-daemon-parar` → `dev-daemon-stop` |
| paralelismo | parallelism | |
| parâmetros | params | `ParametrosKdf` → `KdfParams` |
| parecer, parece, parecidos | seem, seems, similar | |
| parede | wall | relógio de parede → wall clock |
| parte(s) | part(s) | |
| partida | startup | partida do app (D57) |
| passar, passa, passou, passada(s), passagem, passam | pass, passes, passed, passage | |
| passo(s) | step(s) | |
| pasta(s) | dir(s) | `pasta.rs` → `dir.rs`; `PastaAberta` → `OpenDir` |
| pausa, pausar | pause | |
| pedaço(s) | chunk(s) | pedaço da saída |
| pedir, pede, pediu, pedida | request, requests, requested | |
| pedido(s) (ped) | request(s) | `pedido_de_aprovacao` → `approval_request` |
| pedinte | requester | |
| pegar, pegou | take, took | |
| pendente(s) | pending | |
| pendrive(s) | usb_drive(s) | op `pendrives` → `usb_drives`; texto exibido "pendrive" fica |
| pequeno(a) | small | |
| perdido(a) | lost | |
| perfil, perfis | profile(s) | |
| perguntar | ask | |
| perigo | danger | `--cor-perigo-*` → `--color-danger-*` |
| período | period | |
| permissão, permissões | permission(s) | |
| permitir, permitido(a)(s) | allow, allowed | decisão "permitir" |
| perto | near | |
| pessoal | personal | |
| peso | weight | `--peso-*` → `--weight-*` |
| pilha | stack | |
| pílula | pill | `--raio-pilula` → `--radius-pill` |
| plano | plan | |
| plantar, plantado(s) | plant, planted | autotestes |
| plataforma | platform | `ward-plataforma` → `ward-platform` |
| poder, pode | can | |
| política(s) | policy, policies | política do Chrome |
| ponta(s) | end(s), tip(s) | espaço nas pontas → leading/trailing space |
| ponte | bridge | `ward-ponte-teste` → `ward-test-bridge` |
| ponto(s) | point(s) | |
| porta | port | |
| posição, posições | position(s) | |
| preencher, preenche, preenchido, preenchimento | fill, fills, filled, fill | op `preencher` → `fill`; resposta `preenchimento` → `fill` |
| prazo(s) | deadline(s) | `PrazosLogin` → `LoginDeadlines` |
| prefixo(s), prefixado | prefix(es), prefixed | |
| prender, prende, preso | hold, holds, held | |
| preparar, preparo, preparado | prepare, preparation, prepared | |
| primário(a) | primary | |
| primeiro(a) | first | primeiro uso → first use |
| primitiva | primitive | `--primitiva-*` → `--primitive-*` |
| principal | main | janela `principal` → `main` |
| privado(a) | private | |
| problema(s) | problem(s) | |
| processo(s) | process(es) | |
| procurar, procurando | search, searching | |
| produção | production | |
| profundidade | depth | |
| programa(s) | program(s) | |
| pronto (para ler) | ready (to read) | `pronto_para_ler` → `ready_to_read` (m2: Fase 7) |
| proibido(a)(s) | forbidden | |
| projeto | project | |
| próprio(a) | own | |
| prosa | prose | |
| protocolo(s) | protocol(s) | `WARD_PROTOCOLO` → `WARD_PROTOCOL` |
| prova | proof | |
| próximo(a)(s) | next | |
| público(a)(s) | public | `chave-publica.txt` → `public-key.txt` |
| pular | skip | |
| puxar | pull | |
| quadro(s) | frame(s) | quadro do protocolo; CDP frame |
| qualquer | any | |
| quantidade, quantas | count, how many | |
| quebrar, quebra, quebrado | break, breaks, broken | |
| queimado(a), queimar | burned, burn | geração queimada (estado) → burned |
| raio | radius | `--raio-*` → `--radius-*` |
| raiz | root | |
| rajada | burst | |
| rascunho | draft | `Rascunho` → `Draft` |
| razoável | fair | força `razoavel` → `fair` |
| reaberto, reabrir | reopened, reopen | |
| real, reais | real | |
| rebaixado | downgraded | |
| recarregar | reload | |
| receber, recebe, recebido(s) | receive, receives, received | |
| recente | recent | |
| recurso(s) | resource(s) | |
| recusar, recusa(s), recusado(a)(s), recusam | refuse, refusal(s), refused | `login_recusado` → `login_refused` |
| redirecionar | redirect | |
| referência | reference | |
| região, regiões | region(s) | campo `regiao` → `region` |
| registro(s), registrar | record(s), record | `WARD_REGISTRO` → `WARD_RECORD` |
| regra(s) | rule(s) | |
| reiniciar | restart | |
| relatar, relato, relatório | report | |
| relógio | clock | |
| remoto | remote | |
| remover | remove | op `remover_entrada` → `remove_entry` |
| renomear | rename | |
| repassar | relay | |
| repetido(a)(s) | repeated / duplicate | |
| repositório (repo) | repository (repo) | |
| reserva, reservado, reservar | reserve, reserved | |
| responder, responde, respondido(a), respondendo | answer, answers, answered, answering | `responder_aprovacao` → `answer_approval` |
| resposta(s) | response(s) | `TipoResposta` → `ResponseType` |
| restante, resto | remaining, rest | campo `restante` → `remaining` |
| restaurar, restaura, restaurado, restauração, restaurações | restore, restores, restored, restore | |
| resultado(s) | result(s) | |
| resumo, resumida | summary | |
| retângulo | rect | |
| retirado(a) | removed | |
| retrocesso | rollback | erro `retrocesso` → `rollback` |
| revelar, revela, revelado(a)(s) | reveal, reveals, revealed | |
| rodada, rodar, rodando | round, run, running | |
| rodapé | footer | |
| rolagem | scroll | |
| rota | route | |
| rótulo(s) | label(s) | `.texto-rotulo` → `.text-label` |
| roubo | theft | |
| ruído | noise | |
| saída(s) | output(s) | resposta `saida` → `output`; `Fluxo::Saida` → `Stream::Stdout`; como o processo saiu (`Lancador::saida`) → `exit_status`; pontas do pipe `entrada`/`saida`/`erro` do filho → `stdin`/`stdout`/`stderr` (m2: Fase 7) |
| sair, sai, saem, saiu | exit, exits, exited | código de saída → exit code |
| salgado(a) | salted | |
| salto | jump | servidor de salto → jump host; campo `salto` → `jump` |
| salvar, salvo, salvando, salvamento | save, saved, saving, save | |
| seção, seções | section(s) | |
| secreto(s) | secret | |
| secundário | secondary | |
| segmento(s) | segment(s) | |
| segredo(s) | secret(s) | |
| seguido(s) | consecutive | senhas erradas seguidas |
| segundo(s), segunda | second(s) | |
| seguro(a), segurado, segurança(s) | safe, held, security | segurança SMTP → security |
| selar, selado(a)(s), selo(s) | seal, sealed, seal(s) / badge | cripto → seal; `--cor-selo-*`, `.texto-selo` → badge |
| seleção, selecionado(a) | selection, selected | |
| seletor | picker | seletor de arquivo |
| semente | seed | `totp_semente` → `totp_seed` |
| senha(s) | password(s) | |
| sentinela | sentinel | |
| separador, separar | separator, split | |
| serializar | serialize | |
| serviço(s) | service(s) | `servico.rs` → `service.rs`; `dev/servicos/` → `dev/services/` |
| servidor(es) | server(s) | tipo `servidor` → `server` |
| servir, servido | serve, served | |
| sessão, sessões | session(s) | |
| simbólico | symbolic | link simbólico → symlink |
| símbolo(s) | symbol(s) | |
| simples | simple | |
| simular, simulado(s), simulada(s) | simulate, simulated | `simulados.rs` → `simulated.rs` |
| simultâneas | concurrent | |
| sinal, sinais | signal(s) | |
| sincronizar | sync | |
| sistema | system | `WARD_DEV_DBUS_SISTEMA` → `WARD_DEV_SYSTEM_DBUS` |
| situação | situation | |
| sobrar, sobra(s), sobraram | remain, leftover(s) | |
| sobre | on; about | `--cor-*-sobre` (cor sobre fundo) → `on`; `sobre-cor` → `on-color` |
| sobreposição | overlay | |
| sobrescrever, sobrescreve | overwrite | |
| soltar, solto | release, loose | |
| soma, somar, somados | sum, add up, summed | |
| sombra | shadow | |
| sonda, sondar | probe | |
| sorteio | draw | sorteio do gerador |
| sozinho | alone | |
| subir, sobe | start, starts; bring up | `dev-servicos-subir` → `dev-services-up`; `daemon.sh subir` → `up` |
| substituir | replace | |
| subtítulo | subtitle | |
| sucesso | success | |
| suficiente | enough | |
| sufixo | suffix | |
| sugerir, sugestão, sugestões | suggest, suggestion(s) | op `sugerir` → `suggest` |
| sujo(s) | dirty | |
| sumir, sumiu | vanish, vanished | |
| superfície | surface | |
| suportado(a) | supported | |
| suspensão | suspend | |
| sutil | subtle | |
| tabela | table | |
| tamanho(s) | size(s) | |
| tecla, teclar | key, press | |
| tela(s) | screen(s) | `telas/` → `screens/` |
| tema | theme | |
| temporário(a)(s) | temporary (temp) | |
| temporizador | timer | |
| tentar, tentativa(s), tentados | try, attempt(s), tried | |
| terminar | finish | |
| termo | term | |
| testar, testadas | test, tested | |
| teste(s) | test(s) | `testes.rs` → `tests.rs` |
| teto | cap | |
| texto(s) | text(s) | |
| tipo(s) | type(s) | em identificador Rust/JS, `tipo` → `kind` (`type` é palavra reservada); em tipos (`TipoEntrada` → `EntryType`) e chaves JSON (`"tipo"` → `"type"`) fica `type` |
| tirar, tirado(s) | take, taken | |
| título | title | |
| todo(a)(s), tudo | all, everything | `TODOS` (papéis) → `ALL` |
| tom | tone | `--tom-icone` → `--icon-tone` |
| topo | top | |
| trabalho (trab) | work | |
| traço(s) | dash(es) | |
| transferência | transfer | área de transferência → clipboard |
| translúcido | translucent | |
| transporte | transport | |
| tratar | handle | |
| trava(s) | lock(s) | `trava.rs` → `lock.rs`; tempo de trava → lock timeout |
| travar, travado(a)(s), travou | lock, locked | subcomando `travar` → `lock` |
| trazer, traz | bring, brings | |
| trecho(s) | excerpt(s) | |
| três | three | |
| trocar, troca(s), trocado(s), trocada, trocou | change, change(s), changed | `trocar_pin` → `change_pin`; "trocar chave de host" → rotate host key; nos autotestes da Fase 41: trocar dois passos de lugar → swap, trocar um trecho do texto → replace (`trocas` → `swaps`) |
| trocador | swapper | |
| truncado | truncated | |
| túnel, túneis | tunnel(s) | |
| último(a) | last | |
| único | single | token de uso único → one-time token |
| unidade(s) | unit(s) | |
| urgente | urgent | |
| usar, usado(a)(s) | use, used | |
| uso(s) | use(s), usage | `uso.rs` → `usage.rs`; `ErroUso` → `UsageError`; primeiro uso → first use |
| usuário (pessoa, uid) | user | |
| usuário (campo da credencial) | username | campo `usuario` → `username` |
| útil, úteis | useful | |
| vaga(s) | slot(s) | |
| validar, validação, válido(a)(s), validada | validate, validation, valid, validated | |
| valor(es) | value(s) | |
| variação, variações | variation(s) | `variacoes.py` → `variations.py` |
| variante | variant | |
| variável, variáveis | variable(s) | campo `variavel` → `variable` |
| vários(as), várias | several | |
| varredor, varrer, varridos | scanner, scan, scanned | `varredura-logs.py` → `log-scan.py` |
| vazamento | leak | |
| vazio(a)(s) | empty | |
| velho(a) | old | |
| vencer, venceu | expire, expired | prazo vencido |
| ver, viu, visto(s), visão, visões | see, seen, view(s) | geração `vista` (estado) → `seen` |
| verdadeiro(a) | true / real | instância verdadeira do app → real instance |
| verde | green | token |
| vermelho | red | token |
| versão, versões | version(s) | |
| vetor(es) | vector(s) | vetores de teste |
| vez, vezes | time(s) | `uma_vez` → `once` |
| vida | life | tempo de vida → lifetime |
| vigiar, vigia, vigiado(a)(s) | watch, watcher, watched | |
| virar, vira, viram | become, becomes | |
| vírgula | comma | |
| visitante, visitar | visitor, visit | |
| visível, visíveis, visualmente | visible, visually | |
| vítima | victim | |
| vitrine | showcase | `Vitrine.svelte` → `Showcase.svelte` |
| vivo(s) | alive | |
| vizinho(a)(s) | neighbor(s) | |
| voltar, voltou, voltaram, volta | return, returned, return | a parte "Voltar atrás" do guia de migração → go back (`tem_conferir_e_voltar` → `has_verify_and_go_back`, Fase 41) |
| zerar, zera, zerado(a) | zero, zeroes, zeroed | `zeroize` |
| zumbi | zombie | |
| tomar | take | |
| precisar, precisa | need, needs | |
| seguir, segue, seguiu | follow, follows, followed | |
| mundo | world | contexto de execução do CDP (`Mundo` → `World`) |
| quase | almost | |
| vir, vem | come, comes | |
| ciência | awareness | `ciente` |
| restritivo | restrictive | |
| números por extenso (um, dois, três, quatro, cinco, oito…) | one, two, three, four, five, eight… | |

### Palavras funcionais e verbos comuns

Aparecem sobretudo nos comentários e nos nomes de teste em frase
(`ataque_arquivo_que_cresce_nao_realoca_o_buffer` →
`attack_growing_file_does_not_reallocate_the_buffer`). Traduzidas pelo sentido; as que
viram parte fixa de identificador estão na coluna da direita.

| pt-BR | inglês | em identificador |
|---|---|---|
| de, do, da, dos, das, num, numa | of, from, in | some do nome invertendo a ordem: `cabecalho_de_grupo` → `group_header`, `PedidoDeAprovacao` → `ApprovalRequest`; `de_nome`/`de_bytes` → `from_name`/`from_bytes` |
| em, no, na, nos, nas | in, on, at | `criada_em` → `created_at` |
| o, a, os, as, um, uma, uns | the, a | some |
| e, ou | and, or | `AGENTE_E_APP` → `AGENT_AND_APP`; `SenhaOuArquivo` → `PasswordOrFile` |
| que, quem, qual, onde, quando | that/which, who, which, where, when | |
| com | with | `com-aprovacao` (nível) → `approval`; `ComResultado` → `WithResult` |
| sem | without, no | `SemCofre` → `NoVault`; `sem_segredo` → `no_secret`; `TipoSemEntrega` → `TypeWithoutDelivery` |
| para, por, pelo, pela, pelos, ao, aos | for/to, by, by the, to the | `para_o_protocolo` → `for_protocol`; `destravar_por` → `unlock_by` |
| não (nao) | not, no | `NaoAutorizado` → `Unauthorized`; `nao_tem` → `lacks`; `NaoClone` → `NotClone` |
| só (so) | only | `SO_AGENTE` → `AGENT_ONLY`; `so_geracoes` → `generations_only` |
| já (ja), até (ate), ainda | already, until, still | `cofre_ja_existe` → `vault_already_exists` |
| mais, menos, cerca de, quase | more, less, about, almost | |
| sempre, nunca, nem | always, never, nor | decisão `sempre` → `always` |
| outro(a)(s) | other, another | entrada padrão `outra` → `other` |
| este, esta, deste, esse, dele | this, of this, its | |
| ser, é, são, foi, ter, tem, vai, vem, nasce | be, is, are, was, have, has, goes, comes, is born | `tem` (método) → `has`; `nao_tem` → `lacks` |
| todo(s), toda(s), todas | all | `TODOS` → `ALL` |
| meia, metade | half | |

## 3. Crates, pacotes, binários e feature (DT13; Fase 4)

| Antigo | Novo | Onde |
|---|---|---|
| pasta `crates/ward-plataforma`, pacote `ward-plataforma`, lib `ward_plataforma` | `crates/ward-platform`, `ward-platform`, `ward_platform` | |
| pasta `crates/ward-ponte-teste`, pacote e binário `ward-ponte-teste`, lib `ward_ponte_teste` | `crates/ward-test-bridge`, `ward-test-bridge`, `ward_test_bridge` | |
| pasta `app/nucleo`, pacote `ward-app-nucleo`, lib `ward_app_nucleo` | `app/core`, `ward-app-core`, `ward_app_core` | |
| feature `modo-teste` (`cfg(feature = "modo-teste")`) | `test-mode` | Cargo, `cfg`, Makefile, scripts, `dev/origens.py` |
| feature `ganchos-teste` do `wardd` (`cfg(feature = "ganchos-teste")`) | `test-hooks` | `crates/wardd/Cargo.toml`, `nucleo.rs`, testes do `wardd` (Fase 4) |
| binário `wardd-app-teste` (`crates/wardd/app-teste/`) | `wardd-test-app` (`crates/wardd/test-app/`) | |
| binário `wardd-aprovador-teste` (`crates/wardd/aprovador-teste/`) | `wardd-test-approver` (`crates/wardd/test-approver/`) | |
| exemplo `wardd-cliente-teste` (`examples/cliente-teste.rs`) | `wardd-test-client` (`examples/test-client.rs`) | |
| exemplo `ward-plataforma-conteiner` (`examples/conteiner.rs`) | `ward-platform-container` (`examples/container.rs`) | |
| exemplo `pai-chrome` (`examples/pai-chrome.rs`) | `chrome-parent` (`examples/chrome-parent.rs`) | |
| exemplo `chrome-falso` (`crates/ward-nm/examples/chrome-falso.rs`) | `fake-chrome` (`examples/fake-chrome.rs`) | |
| exemplo `dev-cofre` (`crates/ward-core/examples/dev-cofre/`) | `dev-vault` (`examples/dev-vault/`) | |
| exemplo `partida-conteiner` (`app/nucleo/examples/partida-conteiner.rs`) | `startup-container` (`app/core/examples/startup-container.rs`) | |
| testes `[[test]]` do `wardd`: `trava`, `app_teste`, `app_valor` | `lock`, `test_app`, `app_value_binary` | `crates/wardd/Cargo.toml` |

Ficam: `ward`, `wardd`, `ward-nm`, `ward-core`, `ward-app` (pacote do `app/src-tauri`).

## 4. Pastas e arquivos (DT13; Fases 4, 5, 7–47)

O caminho novo vale para o arquivo e para todo uso dele (`mod`, `use`, `import`,
Makefile, scripts, `CLAUDE.md`, `.gitignore`, `.gitleaks.toml`, docs). `testes.rs` vira
`tests.rs` em todo módulo.

### Rust

| Antigo | Novo |
|---|---|
| `crates/ward/src/{cliente,comandos,testes,texto}.rs` | `client.rs`, `commands.rs`, `tests.rs`, `text.rs` (`args.rs`, `lib.rs`, `main.rs` ficam) |
| `crates/ward-nm/src/testes.rs`, `tests/binario.rs` | `src/tests.rs`, `tests/binary.rs` |
| `crates/ward-core/src/limites.rs`, `limites/testes.rs` | `limits.rs`, `limits/tests.rs` |
| `…/cripto.rs`, `gerador.rs` (+`gerador/testes.rs`), `origem.rs`, `exibicao.rs` | `crypto.rs`, `generator.rs` (+`generator/tests.rs`), `origin.rs`, `display.rs` |
| `…/formato.rs` (+`formato/testes.rs`) | `format.rs` (+`format/tests.rs`) |
| `…/gravacao.rs` (+`gravacao/testes.rs`) | `atomic_write.rs` (+`atomic_write/tests.rs`) |
| `…/entrada.rs`, `entrada/{segredo,validacao,testes}.rs` | `entry.rs`, `entry/{secret,validation,tests}.rs` |
| `…/configuracao.rs` (+`configuracao/testes.rs`) | `config.rs` (+`config/tests.rs`) |
| `…/corpo.rs`, `corpo/{cofre_teste,testes}.rs` | `body.rs`, `body/{test_vault,tests}.rs` |
| `…/backup/testes.rs` | `backup/tests.rs` |
| `…/entrega.rs` (+`entrega/testes.rs`) | `delivery.rs` (+`delivery/tests.rs`) |
| `…/mascara.rs`, `mascara/{automato,testes}.rs` | `mask.rs`, `mask/{automaton,tests}.rs` |
| `…/protocolo.rs`, `protocolo/{campos,erros,json,pedido,quadro,resposta,testes}.rs` | `protocol.rs`, `protocol/{fields,errors,json,request,frame,response,tests}.rs` |
| `…/totp/testes.rs` | `totp/tests.rs` |
| `crates/ward-core/tests/{corpo_memoria,fontes}.rs` | `tests/{body_memory,sources}.rs` |
| `crates/ward-core/examples/dev-cofre/{main,testes}.rs` | `examples/dev-vault/{main,tests}.rs` |
| `crates/ward-plataforma/src/{conectar,descendentes,endurecimento,filho,lancador,memoria,papel,pasta,processo,relogio,servidor,teste}.rs` | `ward-platform/src/{connect,descendants,hardening,child,launcher,memory,role,dir,process,clock,server,testing}.rs` (`app`, `lib`, `socket`, `sys`, `terminal` ficam) |
| `crates/ward-plataforma/tests/{ativacao,endurecimento,identificacao}.rs`, `tests/comum/mod.rs` | `tests/{activation,hardening,identification}.rs`, `tests/common/mod.rs` (`socket`, `subreaper` ficam) |
| `crates/ward-ponte-teste/src/{area_wl,partida,simulados}.rs`, `tests/ataques.rs` | `ward-test-bridge/src/{wl_clipboard,startup,simulated}.rs`, `tests/attacks.rs` (`http`, `lib`, `main` ficam) |
| `crates/wardd/src/{agente_ssh,aprovacao,arquivos,execucao,externo,lancamento,nucleo,ponte,servidor,ssh_testes,trava,uso}.rs` | `ssh_agent.rs`, `approval.rs`, `files.rs`, `execution.rs`, `external.rs`, `launch.rs`, `core.rs`, `bridge.rs`, `server.rs`, `ssh_tests.rs`, `lock.rs`, `usage.rs` (`cdp`, `config`, `lib`, `log`, `login`, `main`, `playwright`, `ssh`, `udisks` ficam) |
| `crates/wardd/tests/{anexos_edicao,app_teste,app_valor,aprovacao,configuracoes,datas,extensao,externo,memoria,nucleo,trava,valor_app}.rs` | `attachment_edit.rs`, `test_app.rs`, `app_value_binary.rs`, `approval.rs`, `settings.rs`, `dates.rs`, `extension.rs`, `external.rs`, `memory.rs`, `core.rs`, `lock.rs`, `app_value_core.rs` (`backup`, `d62`, `login`, `playwright`, `run`, `socket`, `ssh`, `units` ficam) |
| `crates/wardd/tests/comum/mod.rs`, `chrome_falso.py`, `ssh_falso.py` | `tests/common/mod.rs`, `fake_chrome.py`, `fake_ssh.py` |
| `app/nucleo/src/{anexos,aprovacao,area_transferencia,backup_externo,bandeja,chave,cliente,comandos,configuracoes,credenciais,daemon_falso,destrave,erro,formulario,gerador,nivel,partida,primeiro_uso,restaurar,segredo,servico}.rs` | `app/core/src/{attachments,approval,clipboard,external_backup,tray,key,client,commands,settings,credentials,fake_daemon,unlock,error,form,generator,level,startup,first_use,restore,secret,service}.rs` (`daemon`, `lib`, `terminal` ficam) |
| `app/nucleo/tests/configuracao_tauri.rs` | `app/core/tests/tauri_config.rs` |
| `app/src-tauri/src/area_gtk.rs` | `gtk_clipboard.rs` |
| `app/src-tauri/capabilities/{aprovacao,principal}.json` | `capabilities/{approval,main}.json` (Fase 54, com o identificador) |
| `app/src-tauri/icons/bandeja-{destravado,travado}-32.png` | `icons/tray-{unlocked,locked}-32.png` |

### Interface (`app/src`, Fases 35–38)

| Antigo | Novo |
|---|---|
| `telas/` | `screens/` |
| `lib/componentes/`, `lib/estilo/`, `lib/ponte/`, `lib/teste/` | `lib/components/`, `lib/style/`, `lib/bridge/`, `lib/test/` |
| `lib/ponte/{comandos,ponte-teste}.js` | `lib/bridge/{commands,test-bridge}.js` (`index.js` fica) |
| `lib/teste/Vitrine.svelte` | `lib/test/Showcase.svelte` |
| `lib/componentes/{icones,niveis,tipos}.js`, `ward-simbolo.svg` | `icons.js`, `levels.js`, `types.js`, `ward-symbol.svg` |
| `telas/Configuracoes.svelte`, `configuracoes.js` | `screens/Settings.svelte`, `settings.js` |
| `telas/Credenciais.svelte`, `credenciais.js` | `Credentials.svelte`, `credentials.js` |
| `telas/Destravar.svelte` | `Unlock.svelte` |
| `telas/EscolherPendrive.svelte`, `EscolherTipo.svelte` | `ChooseUsbDrive.svelte`, `ChooseType.svelte` |
| `telas/Formulario.svelte`, `formulario.js` | `Form.svelte`, `form.js` |
| `telas/GeradorSenha.svelte` | `PasswordGenerator.svelte` |
| `telas/JanelaAprovacao.svelte`, `aprovacao.js` | `ApprovalWindow.svelte`, `approval.js` |
| `telas/KitEmergencia.svelte`, `kit-emergencia.js` | `EmergencyKit.svelte`, `emergency-kit.js` |
| `telas/PrimeiroUso.svelte`, `primeiro-uso.js` | `FirstUse.svelte`, `first-use.js` |
| `telas/RestaurarBackup.svelte`, `restaurar.js` | `RestoreBackup.svelte`, `restore.js` |
| `telas/TrocarPin.svelte`, `TrocarSenha.svelte` | `ChangePin.svelte`, `ChangePassword.svelte` |
| `app/scripts/gerar_design.py` | `app/scripts/generate_design.py` |
| `app/design/icones/` | `app/design/icons/` (Fase 5; os nomes dos `.svg` são as chaves de `ICONES` e mudam com elas na Fase 35) |

Componentes de `lib/componentes/` (`.svelte`):

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `AcaoDeCampo` | `FieldAction` | `ControleDeslizante` | `Slider` |
| `Anexo` | `Attachment` | `Destaque` | `Highlight` |
| `BadgeAmbiente` | `EnvironmentBadge` | `DialogoDeNivel` | `LevelDialog` |
| `BadgeTipo` | `TypeBadge` | `DialogoFiltros` | `FiltersDialog` |
| `Botao` | `Button` | `Icone` | `Icon` |
| `BotaoIcone` | `IconButton` | `IndicadorDePassos` | `StepIndicator` |
| `CabecalhoDeGrupo` | `GroupHeader` | `Interruptor` | `Switch` |
| `CaixaDeCodigo` | `CodeBox` | `ItemDeLista` | `ListItem` |
| `Campo` | `Field` | `ItemDeVersao` | `VersionItem` |
| `CampoExtra` | `ExtraField` | `LinhaDeOpcao` | `OptionRow` |
| `CampoPin` | `PinField` | `NavegacaoLateral` | `SideNav` |
| `CampoSecreto` | `SecretField` | `OpcaoDeNivel` | `LevelOption` |
| `CampoSenha` | `PasswordField` | `PainelDeMarca` | `BrandPanel` |
| `CartaoDeNivel` | `LevelCard` | `PopoverGerador` | `GeneratorPopover` |
| `CartaoDePendrive` | `UsbDriveCard` | `TextoDoAgente` | `AgentText` |
| `CartaoDeTipo` | `TypeCard` | `TituloDeSecao` | `SectionTitle` |
| `CodigoTotp` | `TotpCode` | `Contagem` | `Countdown` |
| `ComboboxGrupo` | `GroupCombobox` | `Logo`, `Modal`, `Select`, `Toast` | ficam |

Os nomes dos componentes e telas no `Ward.figcraft` e em `docs/mission/design-nodes.md`
não mudam (PRD regra 2); os comentários que citam o nome do node do FigCraft citam-no
como está lá.

Fase 35: nos comentários de `app/src`, o node do FigCraft vai pelo id e por uma descrição
em inglês (`Button (FigCraft set 263)`), porque o nome em português do node é acusado
pelo checador; o nome no `Ward.figcraft` não muda. Os textos visíveis em pt-BR citados
nos comentários também viram descrição (o texto fica só no código).

Módulos de `lib/components/` e `lib/bridge/` (Fase 35):

| Antigo | Novo |
|---|---|
| `icones.js` › `ICONES` | `icons.js` › `ICONS` (gerado por `generate_design.py`) |
| `tipos.js` › `TIPOS`, `tipoDe(id)`, `{ rotulo, icone }` | `types.js` › `TYPES`, `entryType(id)`, `{ label, icon }` (chaves do fio entre aspas: `'banco'`…) |
| `niveis.js` › `NIVEIS_DE_ACESSO`, `ORDEM_DOS_NIVEIS`, `nivelDe(nome)`, `{ rotulo, minusculo, icone, cartao, opcao }` | `levels.js` › `ACCESS_LEVELS`, `LEVEL_ORDER`, `accessLevel(name)`, `{ label, lower, icon, card, option }` |
| `ward-simbolo.svg` | `ward-symbol.svg` |
| `ponte/comandos.js`, `ponte/ponte-teste.js` | `bridge/commands.js`, `bridge/test-bridge.js` |
| `chamar(comando, args)`, `ouvirAvisos(...)`, `telaPedida()` | `call(...)`, `listenNotices(...)`, `requestedScreen()` (só o nome exportado; o resto da ponte é da Fase 38) |
| `bidi.js` › `partesBidi(texto)` → `{ texto, rtl }` | `bidiParts(text)` → `{ text, rtl }` |

Ícones de `app/design/icons/` (chave de `ICONS`, valor de `icon`/`name`):

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `alerta` | `alert` | `mais` | `plus` |
| `busca` | `search` | `olho` | `eye` |
| `cadeado` | `lock` | `olho-fechado` | `eye-closed` |
| `chave` | `key` | `relogio` | `clock` |
| `chevron-baixo` | `chevron-down` | `sucesso` | `success` |
| `chevron-direita` | `chevron-right` | `tipo-api`, `tipo-aws`, `tipo-ftp` | `type-api`, `type-aws`, `type-ftp` |
| `clipe` | `paperclip` | `tipo-banco` | `type-database` |
| `copiar` | `copy` | `tipo-generico` | `type-generic` |
| `engrenagem` | `gear` | `tipo-login`, `tipo-nota` | `type-login`, `type-note` |
| `erro` | `error` | `tipo-servidor`, `tipo-smtp` | `type-server`, `type-smtp` |
| `fechar` | `close` | `check` | fica |
| `filtro` | `filter` | `gerador` | `generator` |

Props dos componentes (Fase 35; os mesmos em todos os usos): `rotulo` → `label`,
`icone` → `icon`, `desabilitado` → `disabled`, `valor` → `value`, `ajuda` → `help`,
`erro` → `error`, `titulo` → `title`, `descricao` → `description`, `nome` → `name`,
`detalhe` → `detail`, `aberto` → `open`, `ligado` → `on`, `marcado` → `checked`,
`selecionado` → `selected`, `copiado` → `copied`, `revelado` → `revealed`, `vazio` →
`empty`, `mascara` → `mask`, `entrada` (elemento `<input>`) → `input`, `entrada` (a
entrada do cofre) → `entry`, `nivel` → `level`, `ambiente` → `environment`, `grupo(s)`
→ `group(s)`, `tamanho` (do ícone) → `size`, `tom` → `tone`, `variante` → `variant`,
`compacto/compacta` → `compact`, `grande` → `large`, `pilula` → `pill`, `situacao` →
`situation`, `estado` (do pendrive) → `status`, `montado` → `mounted`, `guardado` →
`stored`, `estatico` → `staticView`, `impressao` → `print`, `minimo`/`maximo` →
`min`/`max`, `segundos` → `seconds`, `urgente` → `urgent`, `passo` → `step`, `cor` →
`color`, `contagem` → `count`, `codigo` → `code`, `restante` → `remaining`, `digitos`
(do PIN) → `digits`, `bloqueado` → `blocked`, `campo` → `field`. `tipo` vira `kind` (o
tipo da entrada, o tipo do Toast e do Highlight) ou `type` (o `type` do HTML no `Button`
e no `Field`). Eventos: `onfechar` → `onclose`, `onmudar`/`ontrocar` → `onchange`,
`oncopiar` → `oncopy`, `onrevelar` → `onreveal`, `onesconder` → `onhide`, `ongerar` →
`ongenerate`, `onremover` → `onremove`, `onescolher` → `onchoose`, `onaplicar` →
`onapply`, `onusar` → `onuse`, `onenviar` → `onsubmit`, `onnavegar` → `onnavigate`,
`ontravar` → `onlock`, `ontravado` → `onlocked`, `onmudado` → `onchanged`,
`ondestravado` → `onunlocked`, `oncriado` → `oncreated`. Snippets: `rodape` → `footer`,
`controle` → `control`, `acao` → `action`.

Valores: `variant` do `Button` `primario`/`secundario`/`perigo`/`fantasma` →
`primary`/`secondary`/`danger`/`ghost`; `kind` do `Toast` `sucesso`/`erro`/`aviso` →
`success`/`error`/`warning`; `kind` do `Highlight` `neutro`/`aviso`/`info`/`perigo`/`prod`
→ `neutral`/`warning`/`info`/`danger`/`prod`; `tone` do `Icon` (e `data-icon-tone`)
`sobre-cor`/`forte`/`desabilitado`/`marca`/`perigo`/`aviso`/`sucesso` →
`on-color`/`strong`/`disabled`/`brand`/`danger`/`warning`/`success`. Atributos `data-*`
dos componentes: `data-teste` → `data-test`, `data-forca` → `data-strength`,
`data-etapa` → `data-step`, `data-nivel` → `data-level`, `data-anexo` →
`data-attachment`, `data-tipo` → `data-type`, `data-valor` → `data-value` (os valores
que vêm do fio ficam).

Telas `Form`, `Credentials` e `ChooseType` (Fase 36):

| Antigo | Novo |
|---|---|
| `formulario.js` › `AMBIENTES_FORM`, `REGIOES_AWS`, `SEGURANCAS`, `MOTORES`, `PROTOCOLOS`, `AUTENTICACOES` | `form.js` › `FORM_ENVIRONMENTS`, `AWS_REGIONS`, `SECURITY_OPTIONS`, `ENGINES`, `PROTOCOLS`, `AUTHENTICATIONS` |
| `TIPOS_FORM` (`{ acesso, tituloAcesso, exemplo, linhas }`), `TEXTOS_EXTRAS`, `TEXTOS_ANEXOS` (chave `padrao`), `CARTOES_TIPO` (`{ tipo, nome, descricao, depois }`) | `FORM_TYPES` (`{ access, accessTitle, example, rows }`), `EXTRAS_TEXTS`, `ATTACHMENTS_TEXTS` (chave `default`), `TYPE_CARDS` (`{ kind, name, description, later }`); chaves do fio entre aspas |
| campo do formulário `{ id, rotulo, tipo, opcoes, local, gerar, auth, multilinha, porta, modo, ajuda, placeholder, url }` | `{ id, label, kind, options, local, generate, auth, multiline, port, inputMode, help, placeholder, url }` |
| `tipo` do campo `texto`/`segredo`/`select`/`totp`/`nota`; `auth` `senha`/`chave`; select local `autenticacao` | `text`/`secret`/`select`/`totp`/`note`; `password`/`key`; `authentication` (o `id` dos outros campos é do fio e fica) |
| chaves de `opcoesSelect` `regioes`, `segurancas`, `motores`, `protocolos`, `autenticacoes`, `servidores`, `saltos` | `regions`, `security`, `engines`, `protocols`, `authentications`, `servers`, `jumps` |
| `acessoDoBanco`, `portaPadrao`, `camposDoTipo`, `campoVisivel`, `tamanhoLegivel`, `variavelDoAnexo`, `urlComPunycode`, `tagsDe`, `novoVazio`, `montarFormulario`, `errosPorCampo` (`nfc` fica) | `databaseAccess`, `defaultPort`, `fieldsOfType`, `fieldVisible`, `readableSize`, `attachmentVariable`, `urlWithPunycode`, `tagsOf`, `newIsEmpty`, `buildForm`, `errorsByField` |
| estado do segredo `{ modo, digitado, tamanho }`, `modo` `salvo`/`digitando`/`gerada`/`carregada` | `{ mode, typed, length }`, `saved`/`typing`/`generated`/`loaded` (o valor que vai ao fio continua `manter`/`gerada`/`carregada`/`vazio`/`{ novo }`) |
| extra `{ chave, nome, valor, segredo, guardado, original }`; anexo `{ id, nome, tamanho }`; `guardados` | `{ key, name, value, secret, stored, original }`; `{ id, name, size }`; `stored` |
| `credenciais.js` › `AMBIENTES`, `FILTROS_VAZIOS` (`{ grupo, tipo, ambiente }`), `normalizar`, `contarEntradas`, `filtrosLigados`, `casaBusca`, `filtrar`, `gruposDe` (`{ nome, cor }`), `agrupar` (`{ itens, contagem }`), `resumoDoTopo`, `resumoDosFiltros`, `NIVEIS` (`{ rotulo, icone, explicacao }`) | `credentials.js` › `ENVIRONMENTS`, `EMPTY_FILTERS` (`{ group, kind, environment }`, o mesmo do `FiltersDialog`), `normalize`, `countEntries`, `activeFilters`, `matchesSearch`, `filterItems`, `groupsOf` (`{ name, color }`), `groupItems` (`{ items, count }`), `topSummary`, `filtersSummary`, `LEVELS` (`{ label, icon, explanation }`) |
| props `Formulario` `tipo`, `detalhe`, `grupos`, `servidores`, `oncancelar`, `onsalvo`; `EscolherTipo` `onescolher`, `oncancelar` | `Form` `kind`, `detail`, `groups`, `servers`, `oncancel`, `onsaved`; `ChooseType` `onchoose`, `oncancel` |
| `modo` da tela Credenciais `detalhe`/`escolher`/`formulario` | `detail`/`choose`/`form` |
| ids do DOM `form-nome`, `form-tipo`, `form-grupo`, `form-ambiente`, `form-projeto`, `form-codigo`, `formulario-credencial`, `aviso-anexo`, `erro-anexos`, `secao-acesso/extras/anexos`, `titulo-credenciais`, `busca`, `campo-<id>`, `segredo-<id>`, `grupo-<cor>-<nome>` | `form-name`, `form-kind`, `form-group`, `form-environment`, `form-project`, `form-code`, `credential-form`, `attachment-notice`, `attachments-error`, `section-access/extras/attachments`, `credentials-title`, `search`, `field-<id>`, `secret-<id>`, `group-<color>-<name>` (`form-<id do fio>` e `extra-<…>` ficam) |
| `data-entrada`, `data-acao="carregar-chave"`, `data-conectado="sim"` (app) | `data-entry`, `data-action="load-key"`, `data-connected="yes"` (o `data-entrada` do popup da extensão é da Fase 39) |

Telas `ApprovalWindow`, `PasswordGenerator`, `FirstUse`, `RestoreBackup` e `EmergencyKit`
(Fase 37):

| Antigo | Novo |
|---|---|
| `aprovacao.js` › `SEGUNDOS_URGENTE`, `pinSuficiente`, `ID_NEGAR`, `pedePin(decisao)`, `SEPARADOR`, `TEXTOS`, `avisoDaOrigem(origem)`, `mensagemDoResultado(resultado)` | `approval.js` › `URGENT_SECONDS`, `pinSufficient`, `DENY_ID`, `needsPin(decision)`, `SEPARATOR`, `TEXTS`, `originNotice(origin)`, `resultMessage(result)` |
| chaves de `TEXTOS` `titulo`, `subtitulo`, `resumo`, `nivel`, `nivelValor`, `comando`, `diretorio`, `quemPediu`, `cadeiaVazia`, `pinAjuda`, `pinFaltando`, `pinIncorreto`, `expirado`, `negar`, `umaVez`, `umaHora`, `sempre`, `fecharENegar`, `nota`, `prodDestaque`, `expirando`, `bloqueadoDestaque`, `bloqueadoResumo`, `bloqueadoNota`, `semPedido`, `prodDestaqueOrigem`, `resumoOrigem`, `subtituloPopup`, `origem`, `origemDestaque`, `origemDestaqueLocal`, `resumoConfirmacao`, `nivelConfirmacao`, `confirmar`, `entradaDestaque`, `entradaCiente`, `entradaFaltando`, `notaConfirmacao` | `title`, `subtitle`, `summary`, `level`, `levelValue`, `command`, `directory`, `requestedBy`, `emptyChain`, `pinHelp`, `pinMissing`, `pinIncorrect`, `expired`, `deny`, `once`, `oneHour`, `always`, `closeAndDeny`, `note`, `prodHighlight`, `expiring`, `blockedHighlight`, `blockedSummary`, `blockedNote`, `noRequest`, `prodHighlightOrigin`, `originSummary`, `popupSubtitle`, `origin`, `originHighlight`, `originHighlightLocal`, `confirmationSummary`, `confirmationLevel`, `confirm`, `stdinHighlight`, `stdinAcknowledged`, `stdinMissing`, `confirmationNote` (`pin` fica) |
| `primeiro-uso.js` › `MIN_CARACTERES_SENHA`, `MIN_DIGITOS_PIN`, `MAX_DIGITOS_PIN`, `MENSAGENS` (`senhaCurta`, `senhaDiferente`, `pinInvalido`, `pinComLetra`, `pinLongo`, `pinDiferente`), `caracteres`, `erroDaSenha`, `pinValido`, `forcaDaSenha`, `mensagemDoRust(motivo)` | `first-use.js` › `MIN_PASSWORD_CHARS`, `MIN_PIN_DIGITS`, `MAX_PIN_DIGITS`, `MESSAGES` (`passwordShort`, `passwordMismatch`, `pinInvalid`, `pinWithLetter`, `pinLong`, `pinMismatch`), `charCount`, `passwordError`, `isValidPin`, `passwordStrength`, `rustMessage(reason)` |
| força da senha `{ ate, codigo, rotulo, segmentos, tom }`, `tom` `perigo`/`aviso`/`sucesso`, atributo `data-tom` (gerador, primeiro uso, trocar senha) | `{ upTo, code, label, segments, tone }`, `danger`/`warning`/`success`, `data-tone` (o `code` e as chaves do `STRENGTHS` do gerador são os valores do fio: `fraca`…) |
| constantes do gerador `TAMANHO_MIN`, `TAMANHO_MAX`, `TAMANHO_PADRAO`, `PRAZO_REVELAR_MS`, `PRAZO_COPIADO_MS`, `FORCAS` | `MIN_LENGTH`, `MAX_LENGTH`, `DEFAULT_LENGTH`, `REVEAL_DEADLINE_MS`, `COPIED_DEADLINE_MS`, `STRENGTHS` (os mesmos do `GeneratorPopover`) |
| `restaurar.js` › `diaDaVersao`, `dataDaVersao`, `quandoFoi`, `detalheDaVersao`, `legendaDasVersoes`, `avisoDaTroca`, `mensagemRestaurado(instante, entradas)` | `restore.js` › `versionDay`, `versionDate`, `howLongAgo`, `versionDetail`, `versionsCaption`, `swapWarning`, `restoredMessage(instant, entries)` |
| `kit-emergencia.js` › `PASTA_COFRE`, `PASTA_BACKUPS`, `PASTA_NO_PENDRIVE`, `PASSOS_RESTAURAR`, `dataImpressa`, `linhaDoPendrive(externo)` → `{ rotulo, valor, mono }`; `externo` `{ ligado, uuid, pendrive: { rotulo } }` | `emergency-kit.js` › `VAULT_DIR`, `BACKUPS_DIR`, `USB_DRIVE_DIR`, `RESTORE_STEPS`, `printedDate`, `usbDriveLine(external)` → `{ label, value, mono }`; `external` `{ enabled, uuid, usbDrive: { label } }` (montado na tela a partir da resposta do fio) |
| props `RestaurarBackup` `aberto`, `onfechar`, `onrestaurado` (`{ instante, entradas }`), `ontravado`; `KitEmergencia` `aberto`, `onfechar`, `ontravado`; `PrimeiroUso` `oncriado` | `RestoreBackup` `open`, `onclose`, `onrestored` (`{ instant, entries }`), `onlocked`; `EmergencyKit` `open`, `onclose`, `onlocked`; `FirstUse` `oncreated` |
| `etapa` do restaurar `versoes`/`confirmar` (e `data-etapa`) | `step` `versions`/`confirm` (`data-step`) |
| ids do DOM `aprovacao-titulo`, `aprovacao-negar`, `aprovacao-entrada`, `aprovacao-pin`; `gerador-titulo`, `opcoes-titulo`, `resultado-titulo`, `gerador-tamanho`, `gerador-minusculas/maiusculas/digitos/simbolos`, `gerador-senha`; `primeiro-uso-criar/senha/confirmar/pin/confirmar-pin`; `restaurar-titulo/subtitulo/senha/pin/versao-<i>/escolhida`; `kit-titulo` | `approval-title`, `approval-deny`, `approval-stdin`, `approval-pin`; `generator-title`, `options-title`, `result-title`, `generator-length`, `generator-lowercase/uppercase/digits/symbols`, `generator-password`; `first-use-create/password/confirm/pin/confirm-pin`; `restore-title/subtitle/password/pin/version-<i>/chosen`; `kit-title` |
| `data-teste` `simbolos`, `restaurar-backup`, `folha-kit`, `kit-pendrive` | `data-test` `symbols`, `restore-backup`, `kit-sheet`, `kit-usb-drive` |

Telas `Settings`, `Unlock`, `ChangePassword`, `ChangePin`, `ChooseUsbDrive`, `lib/bridge` e
`Showcase` (Fase 38):

| Antigo | Novo |
|---|---|
| `configuracoes.js` › `MIN_MINUTOS_TRAVA`, `MAX_MINUTOS_TRAVA`, `OPCOES_TRAVA`, `rotuloMinutos`, `opcoesDeTrava`, `nomeDoPendrive`, `situacaoNaLista`, `cartaoDoPendrive` (`{ nome, situacao, estado, montado }`), `AVISO_BACKUP_LOCAL` | `settings.js` › `MIN_LOCK_MINUTES`, `MAX_LOCK_MINUTES`, `LOCK_OPTIONS`, `minutesLabel`, `lockOptions`, `usbDriveName`, `situationInList`, `usbDriveCard` (`{ name, situation, status, mounted }`), `LOCAL_BACKUP_WARNING` |
| config local da tela Configurações `{ minutos_trava, backup_externo, backup_local }` | `{ lockMinutes, externalBackup, localBackup }` (lida da resposta de `configuracoes`, que fica) |
| `dialogo` da tela Configurações `senha`/`pin`/`pendrive`/`restaurar`/`kit` | `dialog` `password`/`pin`/`usb-drive`/`restore`/`kit` |
| props `Configuracoes` `ontravado`; `Destravar` `pedido`, `ondestravado`, `onerro` | `Settings` `onlocked`; `Unlock` `request`, `onunlocked`, `onerror` (sem o alias da Fase 35) |
| props `TrocarSenha` `aberto`, `onfechar`, `ontrocada`, `onfalha`; `TrocarPin` os mesmos com `ontrocado` e `ontravado` | `ChangePassword` `open`, `onclose`, `onchanged`, `onfailure`; `ChangePin` os mesmos com `onchanged` e `onlocked` |
| props `EscolherPendrive` `aberto`, `trocar`, `atual`, `onfechar`, `onescolhido`, `ontravado` | `ChooseUsbDrive` `open`, `replace`, `current`, `onclose`, `onchosen`, `onlocked` |
| `Destravar` › `ESPERA_MAXIMA`, `COMANDO_DA_OPERACAO`, `comandoDaOperacao`, `comPedido`, `errosSeguidos`, `dispensado`, `rotuloBotao` | `Unlock` › `MAX_WAIT`, `OPERATION_COMMAND` (chave do fio `'executar'` entre aspas), `operationCommand`, `withRequest`, `consecutiveErrors`, `dismissed`, `buttonLabel` |
| erros das trocas `erros` (chaves do fio `atual`, `nova`/`novo`, `confirmacao`), `pinErrado`, `recusa(campo)`, `focar(campo)` | `errors` (as mesmas chaves, entre aspas), `wrongPin`, `refusal(field)`, `focusField(field)` |
| ids do DOM `senha-mestra`, `configuracoes-titulo`, `acesso-titulo`, `backup-titulo`, `config-senha`, `config-trava`, `config-tempo-trava`, `config-backup-local`, `config-backup-local-aviso`, `config-backup-externo`, `trocar-senha-atual/nova/confirmacao`, `trocar-pin-atual/novo/confirmacao`, `v-grupo`, `v-senha`, `v-ambiente` | `master-password`, `settings-title`, `access-title`, `backup-title`, `config-password`, `config-lock`, `config-lock-timeout`, `config-local-backup`, `config-local-backup-warning`, `config-external-backup`, `change-password-current/new/confirmation`, `change-pin-current/new/confirmation`, `v-group`, `v-password`, `v-environment` (`config-pin`, `config-kit` ficam) |
| `data-teste="trocar-senha"`, `"trocar-senha-aviso-backups"`, `"trocar-pin"`; `data-tom` do medidor de força | `data-test="change-password"`, `"change-password-backups-notice"`, `"change-pin"`; `data-tone` (os valores vêm do `first-use.js`: `danger`/`warning`/`success`) |
| `lib/bridge/commands.js` › `COMANDOS` | `COMMANDS` (a lista do fio fica) |
| `lib/bridge/index.js` › `AVISOS`, `obterTransporte`, `falha(codigo, mensagem)` | `NOTICES`, `getTransport`, `failure(code, message)` (o objeto de erro continua `{ codigo, mensagem }`, do fio) |
| `lib/bridge/test-bridge.js` › `criarTransporte`, `lerToken`, `ouvirPorConsulta(aoChegar)` | `createTransport`, `readToken`, `listenByPolling(onArrive)` (o caminho `/ponte/<comando>`, o `&tela=` da URL e os campos `ok`/`valor`/`codigo`/`mensagem` da resposta da ponte ficam) |
| `Vitrine` › `modalAberto`, `.vitrine`, `.linha`, `.lista` | `Showcase` › `modalOpen`, `.showcase`, `.row`, `.list` |

### Extensão (Fases 5 e 39)

| Antigo | Novo |
|---|---|
| `extensao/` | `extension/` |
| `extensao/chave-publica.txt` | `extension/public-key.txt` |
| `extensao/servico.js`, `pagina.js`, `envio.js` | `service.js`, `page.js`, `submit.js` (`login.js`, `manifest.json`, `popup.*` ficam) |
| `dev/extensao/montar.py` | `dev/extension/build.py` |
| `extensao/dist/` | `extension/dist/` |
| `target/extensao/ward.crx` (saída do `make extensao-crx`) | `target/extension/ward.crx` (Fase 5) |

Código da extensão e do montador (Fase 39). Os campos do protocolo de native messaging e
das mensagens internas (`t`, `aba`, `usuario`, `senha`, `origem`, `erro`, `documento`,
`entrada`, `nome`, `grupo`, `ambiente`, `problemas`…) ficam até a Fase 52 (seção 12;
chaves entre aspas e `frozen.txt`); os motivos de `fillInPage` (`'origem'`, `'quadro'`,
`'sem_campo'`, `'formulario'`, `'falha'`) e os desfechos do `ward login` (`'enviado'`,
`'recusado'`, `'aba_mudou'`…) também, porque viram código de erro do fio.

| Antigo | Novo |
|---|---|
| `page.js` › `pedeJanela`, `sondar`, `sondarDepois`, `preencherNaPagina(origemEsperada, usuario, senhaB64, totp, enviar)` | `needsWindow`, `probe`, `probeAfterSubmit`, `fillInPage(expectedOrigin, username, passwordB64, totp, submit)` |
| resultado de `sondar`/`sondarDepois` `{ origem, topo, pronto, senha }` | `{ origin, top, ready, password }` |
| resultado de `preencherNaPagina` `{ ok, motivo, enviado, usuario, totp }` | `{ ok, reason, submitted, username, totp }` (os valores de `reason` ficam) |
| `login.js` › `iniciarLogin`, `porta`, `esperando`, `pedir`, `fechar`, `caiu`, `conectar`, `laco`, `atender`, `injetar`, `depoisDoEnvio`, `dormir` | `startLogin`, `port`, `waiting`, `request`, `close`, `dropped`, `connect`, `loop`, `serve`, `inject`, `afterSubmit`, `sleep` |
| `service.js` › `pendentes`, `descartar`, `enviar`, `origemDa`, `urlDaEntrada`, `doContentScript`, `doPopup`, `aoEnviar`, `pedirPorPorta`, `preencherComJanela`, `salvar`, `PRAZO_MS`, `PRAZO_JANELA_MS`, `AMBIENTES` | `pendingLogins`, `discard`, `send`, `originOf`, `entryUrl`, `fromContentScript`, `fromPopup`, `onSubmitted`, `requestByPort`, `fillWithWindow`, `save`, `TIMEOUT_MS`, `WINDOW_TIMEOUT_MS`, `ENVIRONMENTS` |
| `submit.js` › `aoEnviar`, `MINIMO`, `pegar`, `tipoDe`, `valorDe`, `desabilitado`, `atributo`, `dentro`, `retangulo`, `antes`, `ehForm`, `visivel` | `onSubmit`, `MINIMUM`, `getter`, `typeOf`, `valueOf`, `disabled`, `attribute`, `within`, `rect`, `before`, `isForm`, `visible` |
| `popup.js` › `raiz`, `texto`, `icone`, `simbolo`, `cabecalho`, `botaoAbrirWard`, `mostrar`, `mostrarTravado`, `mostrarAviso`, `mostrarSugestoes`, `mostrarSalvar`, `falhou`, `preencher`, `pedirAoServico`, `sugestoesDoHost`, `campoTexto`, `campoSelect`, `codigoDe`, `iniciar` | `root`, `text`, `icon`, `symbol`, `header`, `openWardButton`, `show`, `showLocked`, `showNotice`, `showSuggestions`, `showSave`, `fail`, `fill`, `askService`, `hostSuggestions`, `textField`, `selectField`, `errorCode`, `start` |
| `popup.js` › `NOTA`, `NOTA_VAZIA`, `FALHAS`, `AGUARDANDO_JANELA`, `FALHA_PADRAO`, `ROTULOS_HOST`, `ROTULO_CAMPO`, `FALHAS_SALVAR` | `NOTE`, `EMPTY_NOTE`, `FAILURES`, `WAITING_WINDOW`, `DEFAULT_FAILURE`, `HOST_LABELS`, `FIELD_OF_PROBLEM`, `SAVE_FAILURES` (novo: `STATUS_LABEL`, o texto `destravado`/`travado` do selo, que antes vinha da classe) |
| classes do popup: `cabecalho`, `simbolo`, `simbolo-botao`, `nome-produto`, `estado` (`destravado`, `travado`), `corpo`, `origem`, `origem-textos`, `origem-rotulo`, `origem-valor`, `secundario`, `lista`, `item-textos`, `linha`, `item-nome`, `item-detalhe`, `badge-ambiente`, `badge-tipo`, `icone`, `nota`, `erro`, `rodape`, `botao`, `secundario-botao`, `primario` | `header`, `symbol`, `button-symbol`, `product-name`, `status` (`unlocked`, `locked`), `body`, `origin`, `origin-texts`, `origin-label`, `origin-value`, `secondary`, `list`, `item-texts`, `row`, `item-name`, `item-detail`, `badge-environment`, `badge-type`, `icon`, `note`, `error`, `footer`, `button`, `secondary-button`, `primary` (`item`, `avatar`, `popup`, `select` ficam) |
| classes do travado e do salvar: `corpo-travado`, `avatar-grande`, `titulo-travado`, `texto-travado`, `com-formulario`, `popup-salvar`, `corpo-salvar`, `introducao`, `titulo-salvar`, `texto-salvar`, `campo`, `rotulo`, `entrada`, `entrada-mascara`, `mascara`, `entrada-select`, `ajuda`, `ajuda-fixa`, `com-erro`, `linha-dupla`, `nivel-inicial`, `nivel-textos`, `nivel-titulo`, `nivel-texto`, `rodape-salvar` | `locked-body`, `avatar-large`, `locked-title`, `locked-text`, `with-form`, `popup-save`, `save-body`, `intro`, `save-title`, `save-text`, `field`, `label`, `field-input`, `field-input-mask`, `mask`, `select-input`, `help`, `help-fixed`, `with-error`, `double-row`, `initial-level`, `level-texts`, `level-title`, `level-text`, `save-footer` |
| ids do salvar `nome`, `usuario`, `tipo`, `ambiente`, `grupo`; `data-entrada` | `entry-name`, `entry-username`, `entry-type`, `entry-environment`, `entry-group`; `data-entry` |
| `dist/`: `fontes.css`, `fontes/`, `marca.js` (`SIMBOLO` `{ caixa, t, d }`), `icones/extensao-<n>.png` | `fonts.css`, `fonts/`, `brand.js` (`SYMBOL` `{ box, t, d }`), `icons/extension-<n>.png` (a fonte `docs/design/icones/extensao-destravado-<n>.png` fica) |
| `dev/extension/build.py` › `RAIZ`, `FONTE`, `ARQUIVOS`, `FONTES`, `TAMANHOS_ICONE`, `PERMISSOES`, `CHAVES_PROIBIDAS`, `PADROES_CONTENT`, `EXTENSOES`, `PROIBIDOS`, `PROIBIDOS_POR_ARQUIVO`, `falha`, `chave_publica`, `id_da_chave`, `id_do_host`, `conferir_manifesto`, `fontes_css`, `marca_js`, `conferir_pacote`; prefixo `montar-extensao:` | `ROOT`, `SOURCE`, `FILES`, `FONTS`, `ICON_SIZES`, `PERMISSIONS`, `FORBIDDEN_KEYS`, `CONTENT_PATTERNS`, `EXTENSIONS`, `FORBIDDEN`, `FORBIDDEN_PER_FILE`, `fail`, `public_key`, `key_id`, `host_id`, `check_manifest`, `fonts_css`, `brand_js`, `check_package`; `extension-build:` |

### Instalação (Fases 5 e 47)

| Antigo | Novo |
|---|---|
| `instalacao/` | `install/` |
| `instalacao/arvore.txt`, `instalar.sh`, `grupos.sh`, `despejo.sh`, `empacotar-extensao.sh` | `install/tree.txt`, `install.sh`, `groups.sh`, `dump.sh`, `pack-extension.sh` |
| `instalacao/{chrome.sh,crx.py,deb.sh,deb/*,desktop/*,systemd/*,tmpfiles.d/ward.conf,coredump.conf.d/ward.conf}` | mesmos nomes sob `install/` |

### `dev/` (Fases 5, 40–46)

| Antigo | Novo |
|---|---|
| `dev/ambiente.sh`, `origens.py`, `variacoes.py`, `ponte.sh`, `abrir-app.py` | `dev/env.sh`, `origins.py`, `variations.py`, `bridge.sh`, `open-app.py` (`daemon.sh`, `compose.yml` ficam) |
| `dev/gitleaks-arvore.toml`, `gitleaks-extra-arvore.toml` | `gitleaks-tree.toml`, `gitleaks-extra-tree.toml` (`gitleaks-extra.toml` fica) |
| `dev/fixtures/entradas.json`, `fixtures/anexos/` | `fixtures/entries.json`, `fixtures/attachments/` (`gama-generico/app.env` é dado e fica) |
| `dev/conteiner-{deb,install,plataforma,uid,units}/` | `dev/container-{deb,install,platform,uid,units}/` |
| `dev/conteiner-*/dentro.sh`, `conteiner-units/iniciar.sh` | `inside.sh`, `start.sh` |
| `dev/conteiner-plataforma/{falso,marca}.c` | `container-platform/{fake,mark}.c` |
| `dev/servicos/` | `dev/services/` |
| `dev/servicos/{comum,conferir}.sh`, `gerar.py` | `common.sh`, `verify.sh`, `generate.py` (`compose.sh` fica) |
| `dev/servicos/ssh/entrada.sh`, `web/servidor.py`, `wordpress/instalar.sh` | `ssh/entrypoint.sh`, `web/server.py`, `wordpress/install.sh` |
| `dev/testes/` | `dev/tests/` |
| `apport_lista.py`, `area-transferencia.sh`, `area_transferencia.py` | `apport_list.py`, `clipboard.sh`, `clipboard.py` |
| `ataques-fundacao.sh`, `ataques-servicos.sh` | `attacks-foundation.sh`, `attacks-services.sh` |
| `canais-gst.sh`, `crx_falso.py`, `dev-cofre.sh`, `docker-plataforma.sh` | `gst-channels.sh`, `fake_crx.py`, `dev-vault.sh`, `docker-platform.sh` |
| `extensao.py`, `extensao.sh`, `falso_v6.py`, `home-real.sh`, `iscas-home.sh` | `extension.py`, `extension.sh`, `fake_v6.py`, `real-home.sh`, `home-decoys.sh` |
| `migracao.py`, `politica-chrome.sh`, `politica_chrome.py` | `migration-guide.py`, `chrome-policy.sh`, `chrome_policy.py` |
| `selecao-revelado.sh`, `selecao_revelado.py`, `sem-modo-teste.sh` | `revealed-selection.sh`, `revealed_selection.py`, `no-test-mode.sh` |
| `socket-ataques.sh`, `socket_ataque.py`, `socket_matriz.py` | `socket-attacks.sh`, `socket_attack.py`, `socket_matrix.py` |
| `terminal-teste.py`, `variacoes_teste.py`, `varredura-logs.py` | `terminal-test.py`, `variations_test.py`, `log-scan.py` |
| ficam em `dev/tests/` | `app-dist.py`, `app-e2e.sh`, `app-release.sh`, `app_e2e.py`, `bidi.mjs`, `bidi_icu.py`, `cli.py`, `deb.sh`, `docker-uid.sh`, `install.sh`, `login.py`, `login.sh`, `nm.py`, `nm.sh`, `pg_select.py`, `playwright.py`, `playwright.sh`, `skill.py`, `ssh-home.sh`, `ssh_home.py`, `units.sh`, `webkit-real.sh` |
| novos (Fases 2 e 3) | `dev/english_check.py`, `dev/syntax_check.py` (sintaxe do `make compile`), `dev/english-check/{pt-words,allow,frozen}.txt`, `dev/fixtures/legacy/` (`vault-0.1.4.ward`, `state-0.1.4`, `backup-0.1.4/`, `protocol-v1.jsonl`, `list-v1.json`, `help-0.1.4.txt`, `strings-0.1.4.txt`, `SHA256SUMS`), `crates/ward-core/src/wire_v1.rs` (testes de fio contra a linha de base) |

Identificadores dos scripts `dev/tests/cli.py` e `extension.py` (Fase 40), pela seção 2:
`conferir`/`confere` → `verify`, `falhas` → `failures`, `relato` → `report`, `padroes` →
`patterns`, `ambiente()` → `environment()`, `rodar_pty` → `run_pty`, `sem_variacao` →
`no_variation`, `tem_segredo` → `has_secret`, `avaliar` → `evaluate`, `bombear` →
`pump`, `abrir_popup`/`fechar_popup`/`anexar_popup`/`popup_novo` →
`open_popup`/`close_popup`/`attach_popup`/`new_popup`, `clicar_entrada` →
`click_entry`, `preencher_forcado` → `forced_fill`, `captura` → `screenshot`,
`valores` → `values`, `modo_janela` → `set_window_mode`, `pedidos_da_janela` →
`window_requests`, `Varredor` → `Scanner`, `LeitorDeProc` → `ProcReader`,
`ssh_com_canal` → `ssh_with_channel`, `TERMINAL_FALSO` → `FAKE_TERMINAL`,
`interativo` → `interactive`, `capturar` → `capture`, `secao_fase_42` →
`phase_42_section`, `tema_continua_escuro` → `theme_stays_dark`, `sem_prender` →
`unstick`. Chaves do `qa.json`, nomes de fio, ids e classes do DOM ficam (literais).

Identificadores de `dev/variations.py`, `dev/tests/variations_test.py`, `login.py` e
`revealed_selection.py` (Fase 42), pela seção 2: `Recusa` → `Refusal`, `variacoes` →
`variations`, `inteiro` → `whole`, `miolos_b64` → `b64_cores`, `escapadas` →
`escaped_forms`, `aspas_duplas`/`aspas_simples` → `double_quoted`/`single_quoted`,
`valores_da_linha` → `line_values`, `valores_json` → `json_values`, `campos_de_tab`
(`_binarios`) → `tab_fields` (`binary_tab_fields`), `por_linha` → `per_line`,
`de_linha` → `line_patterns`, `linhas_de_anexo` → `attachment_lines`, `da_entrada` →
`entry_patterns` (chamada também pelo teste cruzado do `dev-vault.sh`), `coletar` →
`collect`, `conferir_padroes` → `verify_patterns`, `validar_saida` → `validate_output`,
`ler_do_repositorio` → `read_from_repo`, `listavel` → `listable`, `caracteres` →
`char_count`; sentidos próprios: `BRANCO` (espaço em branco ASCII) → `WHITESPACE`,
`BARRA` (`chr(92)`) → `BACKSLASH`, `ESQUEMAS` (esquema de URL) → `SCHEMES`. No
`login.py`, como no `extension.py`: `envios` → `submissions`, `servir_paginas` →
`serve_pages`, `atender_pausa` → `handle_pause`, `ward_nm_vivo` → `ward_nm_alive`. No
`revealed_selection.py`: `modo_ler`/`modo_webkit` → `read_mode`/`webkit_mode`, `ler` →
`read_selections` (e a opção interna `--ler` → `--read`), `no_principal` → `on_main`,
`sem_valor` → `no_value`, `prova` → `proof`, `neutralizar` → `neutralize`.

Identificadores da Fase 45 (`origins.py`, `services/web/server.py`, `services/generate.py`,
`compose.yml`, `nm.py`, `socket_attack.py`, `socket_matrix.py`, `real-home.sh`,
`webkit-real.sh`, `deb.sh`), pela seção 2: `PERMITIDAS_HOME`/`ENV_PERMITIDAS`/
`TOOLCHAIN_PERMITIDAS` → `HOME_ALLOWED`/`ENV_ALLOWED`/`TOOLCHAIN_ALLOWED`, `CANAL_FIXO` →
`FIXED_CHANNEL`, `PULAR` → `SKIP`, `CASA` (home do passwd, local) → `PASSWD_HOME`,
`terminar` → `finish`; no `server.py`: `tela_*` → `*_screen`, `cadastrar`/`entrar` →
`sign_up`/`log_in`, `CONTAS_CARREGADAS` → `LOADED_ACCOUNTS`, `TRAVA_*` → `*_LOCK`; no
`generate.py`: `diretorio` → `ensure_dir`, `SERVICOS_SSH` → `SSH_SERVICES`, `trocar`
(chave de host) → `rotate`, e a primeira linha da docstring (descrição do `--help`) virou
a constante `DESCRIPTION` com o mesmo texto; nos testes: `rastrear` → `trace_run`,
`acusados` → `flagged`, `isca` → `decoy`, `valor_fixo` → `fixed_value`, `LANCADOR` →
`LAUNCHER`, `SEM_MPS`/`SEM_39C` (locais) → `NO_MPS`/`NO_39C`, `RUINS` → `BAD_FRAMES`,
`VAGAS` → `SLOTS`, `sessao` → `session`. Feitos na Fase 45 (antes da 56): `WARD_MODO` =
`password`/`key`/`jump`/`internal` (com o `ssh/entrypoint.sh`), `WARD_PORTA` →
`WARD_LISTEN_PORT`, `WARD_REGISTRO`/`WARD_CONTAS`/`WARD_DESTINO_REDIR` →
`WARD_RECORD`/`WARD_ACCOUNTS`/`WARD_REDIR_TARGET` (só `compose.yml` e `server.py` os
usam); âncoras YAML `seguranca`/`wp-ambiente` → `security`/`wp-env`, rede `interno` →
`internal`; opção interna `--so-host` do `deb.sh` → `--host-only`. Ficam (dados do cofre
de teste, `allow.txt`): os serviços `ssh-senha`, `ssh-chave`, `ssh-salto`, `ssh-interno`.
Ficam para a Fase 46 (contrato do `common.sh`): `RAIZ` em quem faz `source` dele,
`OPCOES_SSH`, `IMAGEM_PG`, `DS`, `SEG` e as funções; para a 56: `WARD_HOME_COBERTO`,
`WARD_TESTE_PG_SENHA_ARQUIVO`, `WARD_PAI_CHROME_SOCK`.

Identificadores de `dev/tests/dev-vault.sh`, `ssh_home.py`, `install.sh`,
`attacks-services.sh`, `chrome_policy.py`, `app_e2e.py` e `playwright.py` (Fase 44), pela
seção 2 e como nas Fases 40–42: `LISTA` (lista de padrões) → `PATTERN_LIST`, `TRAVA`
(arquivo do `flock`) → `LOCK_FILE`, `limpar` → `cleanup`, `sem_segredo` → `no_secret`,
`cofre_duas_vezes` → `vault_twice`, `cruzado` (teste) → `cross_check`, `saida_fora`/
`saida_dentro` → `output_outside`/`output_inside`, `faltou` → `missing`; `CASA` (home do
passwd, no Python) → `PASSWD_HOME`, `fase` (etapa do teste) → `stage`, `escritor` →
`writer`, `acusados` → `flagged`, `pastas_vigiadas` → `watched_dirs`, `pty_rodar` →
`pty_run`, `encerrar_daemon` → `stop_daemon`; `confere` (contém) → `verify`, `nao_tem` →
`lacks`, `recusa` (espera recusa) → `expect_refusal`, `recusado` (idem, comando) →
`expect_refused`, `nada_do_ward` → `nothing_from_ward`, `despejo` → `dump`,
`politica_outra` → `other_policy`, `nao_saiu0` → `not_exit0`, opção `--so-destdir` →
`--destdir-only`; `cenario` → `scenario`, `rotulo` → `label`, `ler_pagina_de_politicas` →
`read_policy_page`, `FOLHAS` → `LEAVES`; `checar` → `check`, `pedido_na_janela` →
`request_in_window`, `responder` (botão) → `answer`, `fim_do_run` → `run_result`,
`run_em_fundo` → `run_in_background`; `Vigia` → `Watcher`, `pids_do_chrome` →
`chrome_pids`, `inodes_escutando` → `listening_inodes`. O `RAIZ` de `common.sh` e o
`CASA_PASSWD` de `env.sh` ficam para a Fase 46 (os scripts que leem `common.sh` fazem
`RAIZ="$ROOT"` antes do `.`).

Contrato de `dev/env.sh` e `dev/services/common.sh` (Fase 46; quem carrega define `ROOT`
antes do `.`, e os `RAIZ="$ROOT"` saíram): `RAIZ` → `ROOT`, `CASA_PASSWD` → `PASSWD_HOME`,
`DS` → `SERVICES_DIR`, `SEG` → `SECRETS_DIR`, `DOCKER_VAZIO` → `DOCKER_EMPTY`, `IMAGEM_PG` →
`PG_IMAGE`, `OPCOES_SSH` → `SSH_OPTIONS`; funções `exigir` → `require`, `cliente` → `client`,
`montar_opcoes_ssh` → `build_ssh_options`, `ssh_senha` → `ssh_password`, `ssh_chave` →
`ssh_key`, `ssh_interno` → `ssh_internal`, `esperar_wordpress` → `wait_wordpress`
(`psql_app`, `wp_login`, `COMPOSE` ficam). Resto de `dev/` (Fase 46), pela seção 2 e como
nas Fases 40–45: no `daemon.sh`, `subir`/`parar` → `start`/`stop_daemon`, `pid_vivo` →
`live_pid`, `MARCA_APP` → `APP_MARK`, `NIVEL_LOG` → `LOG_LEVEL`, `UID_ATUAL` → `CURRENT_UID`,
`PASTA_SOCKET` (local) → `SOCKET_DIR`; `COFRE` → `VAULT`, `ESTADO` → `STATE`, `TRAB` →
`WORK_DIR`, `IMAGEM` → `IMAGE`, `CLIENTE` → `CLIENT`, `APROVADOR` → `APPROVER`, `FALSO` →
`FAKE`, `SONDA` → `PROBE`, `GUARDA` (cópia do cofre) → `BACKUP`, `ABERTAS` → `OPENED`,
`rodada` → `run_round`, `rodar` (etapa) → `run_stage`; nos serviços SSH, `definir_senha` →
`set_password`, `so_chave` → `key_only`, `autorizar_chave` → `authorize_key`,
`banco_pronto` → `db_ready`; no `pg_select.py`, `Conexao` → `Connection`, `sal`/`salgada` →
`salt`/`salted`, `chave_cliente` → `client_key`, `guardada` → `stored_key`, `prova` →
`proof`, `assinatura` → `signature`; no `terminal-test.py`, `mestre`/`escravo` →
`master`/`slave`, `varrer_proc` → `scan_proc`; no `bidi_icu.py`, `faixas_*` → `*_ranges`.
Ficam (literais de outras fases ou dados): `terminal-teste.*` e os modos do
`terminal-test.py`, `aprovador.modo`/`confirmar`, `--adulterado`/`--versao` do
`fake_crx.py`, os cenários e chaves JSON do `chrome-policy.sh` com o `chrome_policy.py`, a
pasta `tracos` do `ssh-home.sh`, os caminhos dos contêineres e dos serviços SSH
(`/etc/ssh/autorizadas`, `senha-*`, `ward-ssh-entrada`), `WARD_*` (Fase 56).

### Evidência dos scripts (DT1; Fase 5)

`docs/mission/evidence/fase-NN/` → `docs/mission/evidence/<suíte>/`, pelo alvo novo:
`fase-41` → `test-extension`, `fase-42` (estados do popup, mesma suíte) →
`test-extension/popup`, `fase-43` → `test-login`, `fase-39c` → `test-app-e2e`.

## 5. Caminhos em `.dev/` (DT13; Fase 5)

| Antigo | Novo |
|---|---|
| `.dev/cofre-teste.ward`, `cofre-teste.estado` | `.dev/test-vault.ward`, `test-vault.state` |
| `.dev/variacoes.txt` | `.dev/variations.txt` |
| `.dev/servicos/{chaves,segredos,home-cliente,ssh-host,known_hosts}` | `.dev/services/{keys,secrets,client-home,ssh-host,known_hosts}` |
| `.dev/extensao/chave.pem` | `.dev/extension/key.pem` (só por `mv`, 0600; o id não muda; V1.9) |
| `.dev/ferramentas/` | `.dev/tools/` |
| `.dev/logs/ward-ponte-teste.log`, `wardd-app-teste.log`, `wardd-aprovador-teste.log` | `.dev/logs/ward-test-bridge.log`, `wardd-test-app.log`, `wardd-test-approver.log` (`wardd.log`, `ward-app.log` ficam; os dois últimos saem do nome do binário e mudam com ele na Fase 56) |
| `marca.c` gerado pelos testes em `.dev/canais-gst/` e na pasta do `webkit-real.sh` | `mark.c` (Fase 5) |
| `.dev/area-transferencia/`, `selecao-revelado/`, `canais-gst/`, `varredura-autoteste/` | `.dev/clipboard/`, `revealed-selection/`, `gst-channels/`, `log-scan-self-test/` |
| `.dev/.teste-dev-cofre.trava` | `.dev/.test-dev-vault.lock` |
| prefixos temporários `.dev/{conteiner-*,extensao,login,nm,politica-chrome,socket-ataques,teste-deb,teste-install}.XXXXXX` | `.dev/{container-*,extension,login,nm,chrome-policy,socket-attacks,test-deb,test-install}.XXXXXX` |
| `.dev/teste-ataque-*` (testes do `dev-cofre`) | `.dev/test-attack-*` |
| `.dev/e2e-marca` | `.dev/e2e-mark` |
| pasta do socket de teste `$XDG_RUNTIME_DIR/ward-teste/` | `$XDG_RUNTIME_DIR/ward-test/` (Fase 56, não 5: o nome é a marca `ward-teste` que o `install.sh` procura nos binários; seção 9) |
| novos (Fase 3) | `.dev/baseline/` (worktree `src/` só durante a Fase 3; `bin-release/`, `bin-test-mode/`, `deb-0.1.4/` com o CRX3 falso e o id, `extension-0.1.4/`, `SHA256SUMS`, `generator-protocol-v1.rs`, `linha_base.rs`, `alternatives/`) |

Ficam: `.dev/docker/`, `.dev/backups/`, `.dev/known_hosts`, `.dev/webkit-real/`,
`.dev/wardd.pid`, `.dev/pw-lib/`. O caminho do cofre real (`~/.local/share/ward/cofre.ward`,
`estado`, `ward-*.ward`, pasta `ward-backup`) não muda (PRD regra 11).

## 6. Makefile (DT14; Fase 6)

Alvos, sem alias:

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `ferramentas` | `tools` | `origens` | `origins` |
| `gitleaks-binario` | `gitleaks-binary` | `teste-ataques-fundacao` | `test-attacks-foundation` |
| `dev-servicos` | `dev-services` | `dev-servicos-subir` | `dev-services-up` |
| `dev-servicos-parar` | `dev-services-down` | `dev-servicos-estado` | `dev-services-status` |
| `dev-servicos-conferir` | `dev-services-verify` | `dev-servicos-trocar-chave-host` | `dev-services-rotate-host-key` |
| `teste-ataques-servicos` | `test-attacks-services` | `teste-iscas-home` | `test-home-decoys` |
| `teste-home-real` | `test-real-home` | `dev-cofre` | `dev-vault` |
| `dev-cofre-conferir` | `dev-vault-verify` | `dev-variacoes` | `dev-variations` |
| `teste-dev-cofre` | `test-dev-vault` | `app-ponte` | `app-bridge` |
| `teste-logs-app` | `test-app-logs` | `teste-area-transferencia` | `test-clipboard` |
| `teste-webkit-real` | `test-webkit-real` | `teste-app-e2e` | `test-app-e2e` |
| `teste-selecao-revelado` | `test-revealed-selection` | `teste-canais-gst` | `test-gst-channels` |
| `teste-docker-plataforma` | `test-docker-platform` | `teste-docker-uid` | `test-docker-uid` |
| `teste-units` | `test-units` | `teste-install` | `test-install` |
| `teste-politica-chrome` | `test-chrome-policy` | `extensao-crx` | `extension-crx` |
| `teste-deb` | `test-deb` | `teste-cli` | `test-cli` |
| `teste-ssh-home` | `test-ssh-home` | `teste-playwright` | `test-playwright` |
| `teste-sem-modo-teste` | `test-no-test-mode` | `teste-socket-ataques` | `test-socket-attacks` |
| `teste-skill` | `test-skill` | `teste-migracao` | `test-migration-guide` |
| `teste-nm` | `test-nm` | `extensao` | `extension` |
| `teste-extensao` | `test-extension` | `teste-login` | `test-login` |
| `dev-daemon-parar` | `dev-daemon-stop` | | |

Ficam: `build`, `fmt`, `fmt-check`, `clippy`, `check`, `test`, `audit`, `deny`,
`gitleaks`, `app-front`, `app-check`, `release`, `install`, `uninstall`, `deb`,
`dev-daemon`. Novos: `compile` e `english-check` (Fase 2).

Variáveis:

| Antiga | Nova | Antiga | Nova |
|---|---|---|---|
| `COFRE` | `VAULT` | `VARIACOES` | `VARIATIONS` |
| `SERVICO` | `SERVICE` | `CONEXOES` | `CONNECTIONS` |
| `EXTENSAO_ID` | `EXTENSION_ID` | `EXTENSAO_CRX` | `EXTENSION_CRX` |
| `CHAVE_EXTENSAO` | `EXTENSION_KEY` | `PONTE_DE_TESTE` | `TEST_BRIDGE` |
| `SEM_MPS` | `NO_MPS` | `SEM_39C` | `NO_39C` |
| `SEM_PLAYWRIGHT_PYTHON` | `NO_PLAYWRIGHT_PYTHON` | `CASA` | `PASSWD_HOME` |
| `FERRAMENTAS_DIR` | `TOOLS_DIR` | `APP_AUDIT_IGNORAR` | `APP_AUDIT_IGNORE` |
| `CARGO_AUDIT_VERSAO` | `CARGO_AUDIT_VERSION` | `CARGO_DENY_VERSAO` | `CARGO_DENY_VERSION` |
| `GITLEAKS_VERSAO` | `GITLEAKS_VERSION` | `GITLEAKS_OPCOES` | `GITLEAKS_OPTIONS` |
| `COMPOSE_TESTE` | `COMPOSE_TEST` | `WARD_DEV_SAIDA` (export do `dev-cofre`) | `WARD_DEV_OUTPUT` |

Ficam: `NODE_DIR`, `PREFIX`, `DESTDIR`, `WARD_UID`, `BIN_*`, `DEB_DIR`, `PW_PYTHONPATH`,
`LOGS`, `CARGO`, `CARGO_AUDIT`, `CARGO_DENY`, `GITLEAKS*` (fora os acima), `APP_NPM`,
`PYTHON`, `WARD_DEV_APP`, `WARD_DEV_SOCKET_DIR`.

Argumentos dos scripts de `dev/` chamados pelo Makefile:

| Antigo | Novo |
|---|---|
| `dev/daemon.sh subir`, `parar`, `estado` | `up`, `stop`, `status` |
| `--sem-mps`, `--sem-39c` (`webkit-real.sh`) | `--no-mps`, `--no-39c` |
| `--trocar-chave-host` (`servicos/gerar.py`) | `--rotate-host-key` |
| `--saida` (`dev-cofre`, `variacoes.py`) | `--output` |
| `--autoteste` | `--self-test` |
| `--adulterado`, `--ativar`, `--conectar`, `--ler`, `--versao` | `--tampered`, `--activate`, `--connect`, `--read`, `--version` |
| `WARD_MODO` = `senha`, `chave`, `salto`, `interno` (`compose.yml`, `ssh/entrada.sh`) | `WARD_MODE` = `password`, `key`, `jump`, `internal` |

Feitos na Fase 6: `dev/daemon.sh` (`up`, `stop`, `status`), `--no-mps`/`--no-39c`,
`--rotate-host-key`, `--output` (`dev-cofre` e `variations.py`) e `--self-test` do
`log-scan.py`. Os outros desta tabela (`--self-test` de outros scripts, `--tampered`,
`--version`, `--read`, `WARD_MODE` …) não passam pelo Makefile e vão com o script, nas
Fases 40–46. O `--conferir` do exemplo `dev-cofre` (e `--senha-stdin`, `--cruzado`)
vai com o exemplo, na Fase 43.

Feitos na Fase 43 (exemplos e scripts de contêiner, sem alias para o nome velho):

| Antes | Depois |
| --- | --- |
| `dev-cofre --conferir`, `--senha-stdin`, `--cruzado` (prefixo `dev-cofre:` na saída) | `dev-vault --verify`, `--password-stdin`, `--cross` (prefixo `dev-vault:`) |
| `falso --ativar`, `--conectar` (`dev/container-platform/fake.c`) | `--activate`, `--connect` |
| subcomandos do `ward-platform-container`: `ativar`, `servidor`, `cliente`, `prender`, `lancar-app`, `escutar`, `ambiente` (`traceme`, `bind`, `--user` ficam) | `activate`, `server`, `client`, `attach`, `launch-app`, `listen`, `environment` |
| opções do `ward-platform-container`: `--lancar`, `--abrir-app`, `--abrir-app-com`, `--sistema-em`, `--pausar` (`--chrome` fica) | `--launch`, `--open-app`, `--open-app-with`, `--system-at`, `--pause` |
| subcomando `servir` do `chrome-parent` | `serve` |
| cópias no contêiner `/opt/teste/chrome-falso`, `/opt/teste/wardd-cliente-teste` | `/opt/teste/fake-chrome`, `/opt/teste/wardd-test-client` |
| `/usr/local/sbin/iniciar` (PID 1 das imagens `units`, `install` e `deb`) | `/usr/local/sbin/start` |
| funções dos `inside.sh`: `confere`, `nao_tem`, `igual`, `falhou`, `como_1000`, `como_usuario`, `instalar`, `caso_grupo`, `nada_do_ward`, `retrato_ssh`, `linha_app`, `ultimo_erro_pid`, `conferir_cli` | `check`, `lacks`, `equal`, `failed`, `as_1000`, `as_user`, `install_ward`, `group_case`, `no_ward_file`, `ssh_snapshot`, `app_line`, `last_error_pid`, `verify_cli` |

Os marcadores impressos pelos exemplos (`servidor: pronto`, `papel=…`, `lancador:`,
`gerente:`, `inicio`/`depois`/`erro`/`queda` do relatório do app, `saida=`,
`resposta:`), os subcomandos do `wardd-test-client` (`estado`, `lista`, `destravar`, nomes
de operação do protocolo, Fase 51), o nome do teste `area_gtk::testes::sessao_real` e o
protocolo de linhas do `clipboard.py` (vão com o teste do `app/src-tauri`) e os outros
nomes de dentro do contêiner (`/opt/teste`, `cliente`, `plataforma`, `falso`,
`ward-servidor`, `libmarca.so`) ficam.

Pasta de build (Fase 6): `target/sem-modo-teste/` → `target/no-test-mode/`.

## 7. CLI `ward` (DT10; Fases 51, 55)

| Antigo | Novo |
|---|---|
| `ward lista [--json]` | `ward list [--json]` |
| `ward destravar` | `ward unlock` |
| `ward travar` | `ward lock` |
| `ward ajuda` | `ward help` |
| `run`, `ssh`, `login`, `playwright`, `-h`/`--help`, `-V`/`--version`, `--json`, `--token-fd`, `--` | ficam |

Sem alias: os nomes antigos dão "comando desconhecido" (saída 2, mensagem pt-BR).
`ward list --json` (Fase 51): chaves `name`, `type`, `group`, `environment`, `project`,
`address`, com os valores do corpo v5 (`database`, `secure-note`…). A tabela de texto
continua com o cabeçalho `NOME TIPO GRUPO AMBIENTE PROJETO URL/HOST` e mostra os rótulos
pt-BR do tipo (hoje ela imprime `TipoEntrada::nome()`, o nome gravado: na Fase 51 passa a
imprimir o rótulo, para continuar "banco", e não "database").

## 8. Variáveis de ambiente

### Entregues pelo `ward run` (DT11; Fase 53)

| Antiga | Nova |
|---|---|
| `WARD_ENTRADA` | `WARD_ENTRY` |
| `WARD_GRUPO` | `WARD_GROUP` |
| `WARD_AMBIENTE` | `WARD_ENVIRONMENT` |
| `WARD_PROTOCOLO` | `WARD_PROTOCOL` |
| `WARD_ARQUIVO_<NOME>` | `WARD_FILE_<NAME>` |
| `WARD_ARQUIVO_CHAVE` | `WARD_FILE_KEY` |

Ficam: `WARD_URL`, `WARD_USER`, `WARD_PASS`, `WARD_TOTP`, `WARD_TOKEN`, `WARD_HOST`,
`WARD_PORT`, `WARD_PASSPHRASE`, `PG*`, `MYSQL_*`, `REDIS*`, `MONGODB_URI`,
`DATABASE_URL`, `AWS_*`, `SMTP_*`, a variável do token da `api` (dado) e os
`WARD_<CAMPO>` dos extras (dado: `WARD_REGIAO`, `WARD_CHAVE_LICENCA`).

### Internas e de teste (DT12; Fase 56)

| Antiga | Nova | Onde |
|---|---|---|
| `WARD_DEV_EXTENSAO` | `WARD_DEV_EXTENSION` | `wardd` `modo-teste`, Makefile, scripts |
| `WARD_DEV_DBUS_SISTEMA` | `WARD_DEV_SYSTEM_DBUS` | idem |
| `WARD_DEV_SAIDA` | `WARD_DEV_OUTPUT` | `make dev-cofre` |
| `WARD_GERENTE_PROGRAMA` | `WARD_MANAGER_PROGRAM` | `examples/conteiner.rs`, `conteiner-plataforma/dentro.sh` |
| `WARD_CAMINHO`, `WARD_ESPERADO` | `WARD_PATH`, `WARD_EXPECTED` | `ward-plataforma/tests/ativacao.rs` |
| `WARD_ATIVACAO` | `WARD_ACTIVATION` | idem |
| `WARD_PLATAFORMA_FILHO` | `WARD_PLATFORM_CHILD` | `tests/endurecimento.rs` |
| `WARD_PAI_CHROME_SOCK` | `WARD_CHROME_PARENT_SOCK` | `examples/pai-chrome.rs` |
| `WARD_EXIGIR_OATHTOOL` | `WARD_REQUIRE_OATHTOOL` | testes do TOTP |
| `WARD_HOME_COBERTO` | `WARD_HOME_COVERED` | `home-real.sh` |
| `WARD_ISCAS_DENTRO` | `WARD_DECOYS_INSIDE` | `iscas-home.sh` |
| `WARD_SSH_HOME_DENTRO`, `WARD_SSH_HOME_TRAB` | `WARD_SSH_HOME_INSIDE`, `WARD_SSH_HOME_WORK` | `ssh-home.sh`, `ssh_home.py` |
| `WARD_TESTE_GRAVACAO_{MODO,DIR,MARCA,CONTROLE}` | `WARD_TEST_WRITE_{MODE,DIR,MARK,CONTROL}` | `gravacao/testes.rs` |
| `WARD_TESTE_BACKUP_{MODO,DIR,BASE}` | `WARD_TEST_BACKUP_{MODE,DIR,BASE}` | `backup/testes.rs` |
| `WARD_TESTE_MANTER_PERFIL` | `WARD_TEST_KEEP_PROFILE` | `wardd/tests/playwright.rs` |
| `WARD_TESTE_PG_SENHA_ARQUIVO` | `WARD_TEST_PG_PASSWORD_FILE` | testes do túnel |
| `WARD_CONTROLE` | `WARD_CONTROL` | `wardd/tests/socket.rs` |
| `WARD_MODO` | `WARD_MODE` | `dev/compose.yml`, `ssh/entrada.sh` |
| `WARD_PORTA` | `WARD_LISTEN_PORT` | `web/servidor.py` (não `WARD_PORT`, que é entregue) |
| `WARD_REGISTRO`, `WARD_CONTAS`, `WARD_DESTINO_REDIR` | `WARD_RECORD`, `WARD_ACCOUNTS`, `WARD_REDIR_TARGET` | `web/servidor.py`, `compose.yml` |
| `WARD_ESTADO` (exemplo da skill) | `WARD_STATE` | `skill/ward/SKILL.md`, `dev/testes/skill.py` |
| `ATQ_ID`, `ATQ_MARCA`, `ATQ_NM`, `ATQ_SAIDA` | `ATK_ID`, `ATK_MARK`, `ATK_NM`, `ATK_OUTPUT` | testes de ataque |
| `CRX_SAIDA`, `EXTENSAO_DIST`, `PASTA_SOCKET` | `CRX_OUTPUT`, `EXTENSION_DIST`, `SOCKET_DIR` | scripts de `instalacao/` e `dev/` |

Ficam: `WARD_DEV_DIR`, `WARD_DEV_APP`, `WARD_DEV_SOCKET_DIR`, `WARD_UID`, `WARD_SOCKET`,
`RUST_LOG`, `LISTEN_FDS`, `LISTEN_PID`, `XDG_RUNTIME_DIR`, `PW_PYTHONPATH`. O teste que
procura `WARD_APROV` (palavra proibida em `wardd/tests/aprovacao.rs`) passa a procurar
também a forma nova, sem tirar a antiga.

## 9. Marcas de `modo-teste` e caminhos instalados (DT12, DT15; Fases 56, 57)

| Antigo | Novo | Nota |
|---|---|---|
| feature `modo-teste` | `test-mode` | |
| marca `ward-ponte-teste` no binário | `ward-test-bridge` | o `install/install.sh` e o `make release` recusam a nova **e** a antiga |
| marca `wardd-app-teste` | `wardd-test-app` | idem |
| marca `ward-teste` (pasta do socket de teste) | `ward-test` | idem |
| `WARD_DEV_*` | `WARD_DEV_*` (com os nomes novos) | a recusa por `WARD_DEV_` pega as duas formas |
| `/usr/local/share/ward/extensao/` | `/usr/local/share/ward/extension/` | a instalação apaga o antigo |
| `/usr/local/lib/ward/grupos-criados` | `/usr/local/lib/ward/created-groups` | o uninstall lê os dois |

Ficam (PRD "Fora do escopo"): `/run/ward/<uid>/wardd.sock`, `wardd@.socket`,
`wardd@.service`, grupos `ward-app`/`ward-nm`, binários `ward`, `wardd`, `ward-app`,
`ward-nm`, host `br.ward.nm`, `local.ward.app`, id e chave da extensão,
`/usr/local/lib/tmpfiles.d/ward.conf`.

## 10. Protocolo v2 (DT8; Fases 50–52)

Congelado como literal até a Fase 50 (`ward-core`) e as Fases 51–52 (clientes). As
tabelas abaixo completam a de DT8 com todos os nomes lidos em
`crates/ward-core/src/protocolo*`. Todo nome é `snake_case`.

### Envelope

| v1 | v2 | Nota |
|---|---|---|
| `v` = 1 | `v` = 2 | `VERSAO` → `VERSION` |
| `op` | `op` | |
| `tipo` (resposta) | `type` | |
| `dados` | `data` | |
| erro: `{"codigo", "mensagem"}` | `{"code", "message"}` | a mensagem é pt-BR e não muda |
| erro `validacao`: `problemas` = `[{"campo", "posicao", "motivo"}]` | `problems` = `[{"field", "position", "reason"}]` | `campo` e `motivo` são textos pt-BR da validação ("usuário", "obrigatório") e não mudam |

### Operações

| v1 | v2 | v1 | v2 |
|---|---|---|---|
| `estado` | `status` | `destravar` | `unlock` |
| `travar` | `lock` | `lista` | `list` |
| `executar` | `run` | `ssh`, `login`, `playwright`, `totp`, `backups` | iguais |
| `entrada_padrao` | `stdin` | `janela` | `resize` |
| `criar_cofre` | `create_vault` | `detalhe` | `detail` |
| `revelar` | `reveal` | `copiar` | `copy` |
| `salvar_entrada` | `save_entry` | `remover_entrada` | `remove_entry` |
| `mudar_nivel` | `set_level` | `responder_aprovacao` | `answer_approval` |
| `emitir_token` | `issue_token` | `trocar_pin` | `change_pin` |
| `trocar_senha` | `change_password` | `tempo_trava` | `lock_timeout` |
| `restaurar_backup` | `restore_backup` | `configuracao` | `config` |
| `backup_externo` | `external_backup` | `definir_backup_externo` | `set_external_backup` |
| `pendrives` | `usb_drives` | `sugerir` | `suggest` |
| `preencher` | `fill` | `salvar_login` | `save_login` |
| `aguardar_login` | `await_login` | `preencher_login` | `fill_login` |
| `resultado_login` | `login_result` | `abrir_app` | `open_app` |

### Tipos de resposta

| v1 | v2 | v1 | v2 |
|---|---|---|---|
| `ok` | `ok` | `estado` | `status` |
| `erro` | `error` | `lista` | `list` |
| `saida` | `output` | `fim` | `exit` |
| `sessao_playwright` | `playwright_session` | `detalhe` | `detail` |
| `segredo` | `secret` | `token` | `token` |
| `pedido_de_aprovacao` | `approval_request` | `pedido_de_destrave` | `unlock_request` |
| `codigo_totp` | `totp_code` | `sugestoes` | `suggestions` |
| `preenchimento` | `fill` | `pedido_de_login` | `login_request` |
| `backups` | `backups` | `configuracao` | `config` |
| `backup_externo` | `external_backup` | `pendrives` | `usb_drives` |

### Papéis

| v1 | v2 | Constantes (Rust) |
|---|---|---|
| `agente` | `agent` | `TODOS` → `ALL`, `SO_AGENTE` → `AGENT_ONLY`, `SO_APP` → `APP_ONLY`, `SO_EXTENSAO` → `EXTENSION_ONLY`, `AGENTE_E_APP` → `AGENT_AND_APP`, `APP_E_EXTENSAO` → `APP_AND_EXTENSION` |
| `app` | `app` | |
| `extensao` | `extension` | |

A matriz operação × papel não muda.

### Códigos de erro

| v1 | v2 | v1 | v2 |
|---|---|---|---|
| `nao_autorizado` | `unauthorized` | `travado` | `locked` |
| `travado_tempo_esgotado` | `locked_timed_out` | `pedido_invalido` | `invalid_request` |
| `operacao_desconhecida` | `unknown_operation` | `versao_desconhecida` | `unknown_version` |
| `quadro_invalido` | `invalid_frame` | `ocupado` | `busy` |
| `senha_incorreta` | `wrong_password` | `cofre_invalido` | `invalid_vault` |
| `cofre_inexistente` | `vault_not_found` | `cofre_ja_existe` | `vault_already_exists` |
| `retrocesso` | `rollback` | `cofre_mudou` | `vault_changed` |
| `geracao_esgotada` | `generation_exhausted` | `falha_ao_gravar` | `write_failed` |
| `pin_incorreto` | `wrong_pin` | `entrada_inexistente` | `entry_not_found` |
| `campo_inexistente` | `field_not_found` | `negado` | `denied` |
| `expirado` | `expired` | `aprovacao_indisponivel` | `approval_unavailable` |
| `origem_diferente` | `origin_mismatch` | `login_recusado` | `login_refused` |
| `navegador_falhou` | `browser_failed` | `sessao_com_segredo` | `session_has_secret` |
| `validacao` | `validation` | `tipo_sem_entrega` | `type_without_delivery` |
| `entrega_falhou` | `delivery_failed` | `backup_inexistente` | `backup_not_found` |
| `backup_indisponivel` | `backup_unavailable` | `pendrives_indisponiveis` | `usb_drives_unavailable` |
| `interno` | `internal` | | |

### Campos (pedidos e respostas)

| v1 | v2 | v1 | v2 |
|---|---|---|---|
| `access_key_id` | `access_key_id` | `alterada_em` | `modified_at` |
| `ambiente` | `environment` | `anexos` | `attachments` |
| `anterior` | `previous` | `argv` | `argv` |
| `atual` | `current` | `backup_externo` | `external_backup` |
| `backup_local` | `local_backup` | `base` | `db_name` |
| `cadeia` | `chain` | `caminho` | `path` |
| `campo` | `field` | `campos` | `fields` |
| `chave_privada` | `private_key` | `codigo` | `code` |
| `cofre_existe` | `vault_exists` | `colunas` | `cols` |
| `comando` | `command` | `conferida` | `verified` |
| `confirmacao` | `confirmation` | `conteudo` | `content` |
| `criada_em` | `created_at` | `cwd` | `cwd` |
| `dados` (bytes da `entrada_padrao`) | `data` | `decisao` | `decision` |
| `diretorio` | `directory` | `endereco` | `address` |
| `entrada` | `entry` | `entrada_confirmada` | `confirmed_entry` |
| `entrada_oculta` | `hidden_entry` | `entradas` | `entries` |
| `extra` | `extra` | `extras` | `extras` |
| `fluxo` | `stream` | `fora_de_ordem` | `out_of_order` |
| `grupo` | `group` | `host` | `host` |
| `id` | `id` | `itens` | `items` |
| `janela` (campo do `ssh`: tamanho do terminal) | `window_size` | `ligado` | `enabled` |
| `linhas` | `rows` | `manter_anexos` | `keep_attachments` |
| `mensagem` | `message` | `minutos` | `minutes` |
| `minutos_trava` | `lock_minutes` | `montado` | `mounted` |
| `motivo` | `reason` | `motor` | `engine` |
| `nivel` | `level` | `nome` | `name` |
| `nova` / `novo` (troca de senha/PIN) | `new` | `operacao` | `operation` |
| `origem` | `origin` | `papel` | `role` |
| `passphrase` | `passphrase` | `pedido` | `request` |
| `pin` | `pin` | `porta` | `port` |
| `posicao` | `position` | `problemas` | `problems` |
| `projeto` | `project` | `protocolo` | `protocol` |
| `regiao` | `region` | `restante` | `remaining` |
| `resultado` | `result` | `resumo` | `summary` |
| `role_arn` | `role_arn` | `rotulo` | `label` |
| `salto` | `jump` | `secret_access_key` | `secret_access_key` |
| `segredo` | `secret` | `segredos` | `secrets` |
| `seguranca` | `security` | `senha` | `password` |
| `session_token` | `session_token` | `tags` | `tags` |
| `tamanho` | `size` | `texto` | `text` |
| `tipo` | `type` | `tipo_entrada` | `stdin_kind` |
| `token` | `token` | `totp` | `totp` |
| `totp_semente` | `totp_seed` | `travado` | `locked` |
| `tunel` | `tunnel` | `url` | `url` |
| `usuario` | `username` | `uuid` | `uuid` |
| `valor` | `value` | `variavel` | `variable` |
| `versao` | `version` | `versoes` | `versions` |

Campos secretos do tipo (`CAMPOS_SECRETOS` → `SECRET_FIELDS`): `password`, `totp_seed`,
`private_key`, `passphrase`, `token`, `secret_access_key`, `session_token`, `text`.
Campos de texto (`CAMPOS_TEXTO` → `TEXT_FIELDS`): `url`, `username`, `engine`, `host`,
`port`, `db_name`, `tunnel`, `jump`, `variable`, `access_key_id`, `region`, `role_arn`,
`security`, `protocol`.

### Valores

| Campo | v1 | v2 |
|---|---|---|
| `tipo_entrada` → `stdin_kind` | `terminal`, `nula`, `pipe`, `arquivo`, `outra` | `terminal`, `null`, `pipe`, `file`, `other` |
| `decisao` → `decision` | `negar`, `uma_vez`, `uma_hora`, `sempre`, `confirmar` | `deny`, `once`, `one_hour`, `always`, `confirm` |
| `resultado` (login) → `result` | `enviado`, `recusado`, `sem_formulario`, `aba_mudou`, `falha` | `submitted`, `refused`, `no_form`, `tab_changed`, `failure` |
| `fluxo` → `stream` | `saida`, `erro` | `stdout`, `stderr` |
| `papel` (no `estado`) → `role` | `agente`, `app`, `extensao` | `agent`, `app`, `extension` |
| `tipo`, `nivel`, `seguranca`, `motor`, `ambiente`, `protocolo` | nomes do corpo v4 | nomes do corpo v5 (seção 11) |

## 11. Nomes gravados

### Corpo do cofre v5 (DT4; Fase 48)

`VERSAO_CORPO` → `BODY_VERSION`, 4 → 5. São os únicos nomes em texto dentro do corpo
(conferido em `corpo.rs`: tipo, nível, ambiente, motor, segurança SMTP e protocolo FTP vão
por `nome()`; o resto é binário).

| Grupo | v4 (0.1.x) | v5 (0.2.0) |
|---|---|---|
| tipo | `login`, `banco`, `servidor`, `api`, `aws`, `smtp`, `ftp-sftp`, `nota-segura`, `generico` | `login`, `database`, `server`, `api`, `aws`, `smtp`, `ftp-sftp`, `secure-note`, `generic` |
| nível | `bloqueado`, `com-aprovacao`, `livre` | `blocked`, `approval`, `free` |
| segurança SMTP | `nenhuma`, `starttls`, `tls` | `none`, `starttls`, `tls` |
| ambiente | `dev`, `stage`, `prod` | iguais |
| motor | `postgres`, `mysql`, `mongo`, `redis`, `sqlserver` | iguais |
| protocolo | `ftp`, `sftp` | iguais |

Os rótulos exibidos (`rotulo()` → `label()`: "banco", "com aprovação", "nota segura",
"genérico", "ftp/sftp") ficam em pt-BR.

### Arquivo de estado (DT6; Fase 49)

O nome no disco (`estado`, `.dev/test-vault.state` no teste) não muda o do cofre real.

| `ward-estado 1` | `ward-state 2` |
|---|---|
| cabeçalho `ward-estado 1` | `ward-state 2` |
| `vista <n>` | `seen <n>` |
| `queimada <n>` | `burned <n>` |
| `falhas_agente <n>` | `agent_failures <n>` |

### Outros nomes no disco que ficam

`WARDCOFR` (magia do formato), `cofre.ward`, `estado`, `ward-<data>.ward` (versões do
backup), pasta `ward-backup` do pendrive, `known_hosts` do Ward, prefixos de temporário
`.tmp-` e `ward-anexos-` (`ward-anexos-` → `ward-attachments-`, só em `/run/user/<uid>/ward/`,
nunca persistido; muda com o código que o cria).

## 12. Native messaging e mensagens da extensão (DT9; Fase 52)

- Extensão ↔ `ward-nm` ↔ daemon: o próprio protocolo v2 (seção 10). O `ward-nm` repassa o
  quadro e só aceita as operações da extensão (`OPERACOES` → `OPERATIONS`); a resposta
  pronta de erro interno dele (`{"v":1,"tipo":"erro","dados":{"codigo":"interno",…}}`)
  passa a `{"v":2,"type":"error","data":{"code":"internal",…}}`. `ID_EXTENSAO` →
  `EXTENSION_ID` (valor igual); `MAX_MENSAGEM_ENTRADA`/`SAIDA` → `MAX_MESSAGE_IN`/`OUT`.
- Mensagens internas da extensão (`chrome.runtime.sendMessage` entre content script,
  popup e service worker): a chave `t` fica; valores e campos:

| Antigo | Novo |
|---|---|
| `t: 'enviado'` (content script → service worker) | `t: 'submitted'` |
| `t: 'pendente'`, `'manter'`, `'descartar'`, `'salvar'`, `'preencher'` | `'pending'`, `'keep'`, `'discard'`, `'save'`, `'fill'` |
| campos `aba`, `usuario`, `senha`, `origem`, `erro` | `tab`, `username`, `password`, `origin`, `error` |
| código local `sem_resposta` | `no_response` |
| constante `HOST` (`br.ward.nm`) | fica |

## 13. Tauri: comandos, janelas, avisos e `serde` da webview (Fase 54)

Congelados até a Fase 54 (o Rust usa `#[tauri::command(rename_all…)]`/`rename` se a fase
anterior mudar o identificador). A lista `COMANDOS` → `COMMANDS` do
`app/nucleo/src/comandos.rs` e do `app/src/lib/ponte/comandos.js` e as permissões
`allow-<comando-com-hífen>` acompanham.

### Comandos

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `estado_do_cofre` | `vault_status` | `copiar_campo` | `copy_field` |
| `listar_credenciais` | `list_credentials` | `detalhe_credencial` | `credential_detail` |
| `gerar_senha` | `generate_password` | `copiar_senha_gerada` | `copy_generated_password` |
| `revelar_senha_gerada` | `reveal_generated_password` | `descartar_senha_gerada` | `discard_generated_password` |
| `revelar_campo` | `reveal_field` | `codigo_totp` | `totp_code` |
| `copiar_codigo_totp` | `copy_totp_code` | `usar_senha_gerada` | `use_generated_password` |
| `descartar_formulario` | `discard_form` | `salvar_credencial` | `save_credential` |
| `mudar_nivel` | `set_level` | `escolher_anexo` | `choose_attachment` |
| `remover_anexo` | `remove_attachment` | `carregar_chave` | `load_key` |
| `conectar` | `connect` | `criar_cofre` | `create_vault` |
| `destravar` | `unlock` | `travar` | `lock` |
| `ouvir_avisos` | `listen_notices` | `configuracoes` | `settings` |
| `trocar_pin` | `change_pin` | `trocar_senha_mestra` | `change_master_password` |
| `definir_tempo_trava` | `set_lock_timeout` | `backup_externo` | `external_backup` |
| `listar_pendrives` | `list_usb_drives` | `definir_backup_externo` | `set_external_backup` |
| `listar_backups` | `list_backups` | `restaurar_backup` | `restore_backup` |
| `pedido_de_aprovacao` | `approval_request` | `responder_aprovacao` | `answer_approval` |

Parâmetros: `anterior` → `previous`, `campo` → `field`, `canal` → `channel`, `ciente` →
`acknowledged`, `dados` → `data`, `decisao` → `decision`, `digitos` → `digits`, `edicao`
→ `edit`, `entrada` → `entry`, `formulario` → `form`, `janela` → `window`, `ligado` →
`enabled`, `maiusculas` → `uppercase`, `minusculas` → `lowercase`, `minutos` → `minutes`,
`nivel` → `level`, `pedido` → `request`, `senha` → `password`, `simbolos` → `symbols`,
`tamanho` → `size`, `valor` → `value` (`id`, `pin`, `uuid`, `app` ficam; `estado:
State<…>` → `state`).

### Janelas, capabilities e avisos

| Antigo | Novo |
|---|---|
| janela `principal` (`JANELA_PRINCIPAL`) | `main` (`MAIN_WINDOW`) |
| janela `aprovacao` (`JANELA_APROVACAO`) | `approval` (`APPROVAL_WINDOW`) |
| capability `principal`, `aprovacao` | `main`, `approval` |
| aviso `estado-mudou` | `status-changed` |
| aviso `pedido-de-aprovacao` | `approval-request` |
| aviso `pedido-de-destrave` | `unlock-request` |
| ids da bandeja e nomes de thread `ward-bandeja`, `ward-app-bandeja`, `ward-app-travar`, `ward-app-negar`, `ward-app-aprovacao`, `ward-app-destrave`, `ward-app-vigia`, `ward-app-leitora` | `ward-tray`, `ward-app-tray`, `ward-app-lock`, `ward-app-deny`, `ward-app-approval`, `ward-app-unlock`, `ward-app-watcher`, `ward-app-reader` |

### Campos `serde` que vão à webview

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `alterada_em` | `modified_at` | `ambiente` | `environment` |
| `anexos` | `attachments` | `anexos_mantidos` | `kept_attachments` |
| `atual` | `current` | `aviso` | `notice` |
| `backup_externo` | `external_backup` | `backup_local` | `local_backup` |
| `cadeia` | `chain` | `campo` | `field` |
| `campos` | `fields` | `codigo` | `code` |
| `comando` | `command` | `conferida` | `verified` |
| `confirmacao` | `confirmation` | `confirmacao_pin` | `pin_confirmation` |
| `criada_em` | `created_at` | `criado` | `created` |
| `diretorio` | `directory` | `endereco` | `address` |
| `entrada` | `entry` | `entrada_oculta` | `hidden_entry` |
| `entradas` | `entries` | `erros` | `errors` |
| `estado` | `status` | `extras` | `extras` |
| `forca` | `strength` | `fora_de_ordem` | `out_of_order` |
| `grupo` | `group` | `ha_outro` | `has_other` |
| `instante` | `instant` | `ligado` | `enabled` |
| `mantidos` | `kept` | `mensagem` | `message` |
| `minutos_trava` | `lock_minutes` | `montado` | `mounted` |
| `motivo` | `reason` | `nivel` | `level` |
| `nome` | `name` | `nova`, `novo` | `new` |
| `operacao` | `operation` | `origem` | `origin` |
| `pedido` | `request` | `pendrive` | `usb_drive` |
| `pendrives_visiveis` | `visible_usb_drives` | `posicao` | `position` |
| `projeto` | `project` | `restante` | `remaining` |
| `restaurado` | `restored` | `resultado` | `result` |
| `resumo` | `summary` | `rotulo` | `label` |
| `salvo` | `saved` | `segredo` | `secret` |
| `segredos` | `secrets` | `senha` | `password` |
| `simbolos` | `symbols` | `subtitulo` | `subtitle` |
| `tamanho` | `size` | `tipo` | `type` |
| `trocado` | `changed` | `usuario` | `username` |
| `valor` | `value` | `versao` | `version` |

Ficam: `id`, `uuid`, `pin`, `tags`, `bits`.

### Valores que vão à webview

| Antigo | Novo |
|---|---|
| códigos de `ErroApp` (`erro.rs`): `daemon-indisponivel`, `daemon-nao-confiavel`, `conexao-perdida`, `pedido-invalido`, `travado`, `area-transferencia`, `entrada-inexistente`, `campo-inexistente`, `nao-autorizado`, `recusado`, `resposta-invalida`, `aleatorio-indisponivel`, `sem-senha-gerada`, `pin-incorreto`, `seletor-indisponivel`, `terminal-indisponivel`, `nao-e-servidor`, `cofre-ja-existe`, `cofre-nao-gravado`, `senha-incorreta`, `cofre-retrocesso`, `pendrives-indisponiveis`, `pendrive-nao-conectado`, `backup-inexistente`, `backup-indisponivel`, `cofre-mudou` | `daemon-unavailable`, `daemon-untrusted`, `connection-lost`, `invalid-request`, `locked`, `clipboard`, `entry-not-found`, `field-not-found`, `unauthorized`, `refused`, `invalid-response`, `random-unavailable`, `no-generated-password`, `wrong-pin`, `picker-unavailable`, `terminal-unavailable`, `not-a-server`, `vault-already-exists`, `vault-not-written`, `wrong-password`, `vault-rollback`, `usb-drives-unavailable`, `usb-drive-not-connected`, `backup-not-found`, `backup-unavailable`, `vault-changed` |
| estado do cofre (`EstadoCofre`): `sem-cofre`, `travado`, `destravado` | `no-vault`, `locked`, `unlocked` |
| força da senha: `fraca`, `razoavel`, `forte`, `muito-forte` | `weak`, `fair`, `strong`, `very-strong` |
| `Escolha` e `Carregamento` (tag `resultado` → `result`): `anexado`, `carregada`, `cancelado`, `aviso` | `attached`, `loaded`, `canceled`, `notice` |
| `ValorSecreto`: `vazio`, `manter`, `gerada`, `carregada`, `novo`; `ValorExtra`: `novo`, `manter` | `empty`, `keep`, `generated`, `loaded`, `new`; `new`, `keep` |
| resultado da aprovação `respondido`; estados do pedido exibido (`pendente`, `bloqueado`, `pin-incorreto`…) | `answered`; `pending`, `blocked`, `wrong-pin` (pela seção 2) |
| ids dos campos do formulário (`formulario.rs`): `nome`, `grupo`, `ambiente`, `projeto`, `usuario`, `senha`, `variavel`, `regiao`, `porta`, `seguranca`, `texto`, `motor`, `base`, `tunel`, `salto`, `protocolo`, `autenticacao`, `anexos`, `formulario` | `name`, `group`, `environment`, `project`, `username`, `password`, `variable`, `region`, `port`, `security`, `text`, `engine`, `db_name`, `tunnel`, `jump`, `protocol`, `auth`, `attachments`, `form` (os mesmos do protocolo, seção 10) |
| operação exibida (`executar`, `ssh`…) e tipo/nível/ambiente | os do protocolo v2 e do corpo v5 |

## 14. CSS (Fases 35, 39)

Tokens de `app/src/lib/estilo/tokens.css` (→ `lib/style/tokens.css`) e classes globais.
Propriedades locais do `Slider`: `--progresso` → `--progress`, `--ponto` → `--point`.
Mesmos valores, só o nome (V3.6). Atributo `data-tom-icone="sobre-cor"` →
`data-icon-tone="on-color"`. As classes locais dos componentes seguem a seção 2
(`.cabecalho` → `.header`, `.rodape` → `.footer`). `extension/popup.css` e o montador
da extensão usam os mesmos nomes.

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `--primitiva-neutro-0` | `--primitive-neutral-0` | `--primitiva-neutro-50` | `--primitive-neutral-50` |
| `--primitiva-neutro-400` | `--primitive-neutral-400` | `--primitiva-neutro-600` | `--primitive-neutral-600` |
| `--primitiva-neutro-700` | `--primitive-neutral-700` | `--primitiva-neutro-800` | `--primitive-neutral-800` |
| `--primitiva-neutro-850` | `--primitive-neutral-850` | `--primitiva-neutro-900` | `--primitive-neutral-900` |
| `--primitiva-neutro-950` | `--primitive-neutral-950` | `--primitiva-azul-300` | `--primitive-blue-300` |
| `--primitiva-azul-400` | `--primitive-blue-400` | `--primitiva-azul-500` | `--primitive-blue-500` |
| `--primitiva-azul-900` | `--primitive-blue-900` | `--primitiva-vermelho-400` | `--primitive-red-400` |
| `--primitiva-vermelho-500` | `--primitive-red-500` | `--primitiva-vermelho-600` | `--primitive-red-600` |
| `--primitiva-vermelho-900` | `--primitive-red-900` | `--primitiva-ambar-400` | `--primitive-amber-400` |
| `--primitiva-ambar-500` | `--primitive-amber-500` | `--primitiva-ambar-900` | `--primitive-amber-900` |
| `--primitiva-verde-400` | `--primitive-green-400` | `--primitiva-verde-500` | `--primitive-green-500` |
| `--primitiva-verde-900` | `--primitive-green-900` | `--primitiva-ciano-400` | `--primitive-cyan-400` |
| `--primitiva-ciano-500` | `--primitive-cyan-500` | `--primitiva-ciano-900` | `--primitive-cyan-900` |
| `--primitiva-sobreposicao-escuro` | `--primitive-overlay-dark` | `--primitiva-sombra-preto-24` | `--primitive-shadow-black-24` |
| `--primitiva-azul-500-40` | `--primitive-blue-500-40` | `--primitiva-vermelho-500-16` | `--primitive-red-500-16` |
| `--primitiva-vermelho-500-24` | `--primitive-red-500-24` | `--primitiva-azul-600` | `--primitive-blue-600` |
| `--primitiva-ciano-400-16` | `--primitive-cyan-400-16` | `--primitiva-ambar-400-16` | `--primitive-amber-400-16` |
| `--primitiva-vermelho-400-16` | `--primitive-red-400-16` | `--primitiva-neutro-400-12` | `--primitive-neutral-400-12` |
| `--primitiva-violeta-400` | `--primitive-violet-400` | `--primitiva-rosa-400` | `--primitive-pink-400` |
| `--primitiva-laranja-400` | `--primitive-orange-400` | `--primitiva-verde-400-16` | `--primitive-green-400-16` |
| `--primitiva-azul-400-32` | `--primitive-blue-400-32` | `--primitiva-ambar-400-32` | `--primitive-amber-400-32` |
| `--primitiva-vermelho-400-32` | `--primitive-red-400-32` | `--primitiva-verde-400-32` | `--primitive-green-400-32` |
| `--primitiva-vermelho-300` | `--primitive-red-300` | `--neutro-900-0` | `--neutral-900-0` |
| `--cor-fundo-padrao` | `--color-bg-default` | `--cor-fundo-sutil` | `--color-bg-subtle` |
| `--cor-fundo-sobreposicao` | `--color-bg-overlay` | `--cor-superficie-padrao` | `--color-surface-default` |
| `--cor-superficie-elevada` | `--color-surface-raised` | `--cor-superficie-campo` | `--color-surface-field` |
| `--cor-superficie-hover` | `--color-surface-hover` | `--cor-superficie-selecionada` | `--color-surface-selected` |
| `--cor-superficie-desabilitada` | `--color-surface-disabled` | `--cor-texto-primario` | `--color-text-primary` |
| `--cor-texto-secundario` | `--color-text-secondary` | `--cor-texto-desabilitado` | `--color-text-disabled` |
| `--cor-texto-inverso` | `--color-text-inverse` | `--cor-texto-marca` | `--color-text-brand` |
| `--cor-borda-padrao` | `--color-border-default` | `--cor-borda-forte` | `--color-border-strong` |
| `--cor-borda-foco` | `--color-border-focus` | `--cor-primaria-padrao` | `--color-primary-default` |
| `--cor-primaria-hover` | `--color-primary-hover` | `--cor-primaria-sutil` | `--color-primary-subtle` |
| `--cor-primaria-sobre` | `--color-primary-on` | `--cor-perigo-padrao` | `--color-danger-default` |
| `--cor-perigo-hover` | `--color-danger-hover` | `--cor-perigo-sutil` | `--color-danger-subtle` |
| `--cor-perigo-texto` | `--color-danger-text` | `--cor-perigo-sobre` | `--color-danger-on` |
| `--cor-aviso-padrao` | `--color-warning-default` | `--cor-aviso-sutil` | `--color-warning-subtle` |
| `--cor-aviso-texto` | `--color-warning-text` | `--cor-sucesso-padrao` | `--color-success-default` |
| `--cor-sucesso-sutil` | `--color-success-subtle` | `--cor-sucesso-texto` | `--color-success-text` |
| `--cor-ambiente-dev-fundo` | `--color-environment-dev-bg` | `--cor-ambiente-dev-texto` | `--color-environment-dev-text` |
| `--cor-ambiente-dev-borda` | `--color-environment-dev-border` | `--cor-ambiente-stage-fundo` | `--color-environment-stage-bg` |
| `--cor-ambiente-stage-texto` | `--color-environment-stage-text` | `--cor-ambiente-stage-borda` | `--color-environment-stage-border` |
| `--cor-ambiente-prod-fundo` | `--color-environment-prod-bg` | `--cor-ambiente-prod-texto` | `--color-environment-prod-text` |
| `--cor-ambiente-prod-borda` | `--color-environment-prod-border` | `--cor-ambiente-prod-destaque` | `--color-environment-prod-highlight` |
| `--cor-ambiente-prod-sobre` | `--color-environment-prod-on` | `--cor-icone-padrao` | `--color-icon-default` |
| `--cor-icone-forte` | `--color-icon-strong` | `--cor-icone-desabilitado` | `--color-icon-disabled` |
| `--cor-icone-marca` | `--color-icon-brand` | `--cor-icone-sobre-cor` | `--color-icon-on-color` |
| `--cor-borda-sutil` | `--color-border-subtle` | `--cor-sombra-suave` | `--color-shadow-soft` |
| `--cor-primaria-brilho` | `--color-primary-glow` | `--cor-perigo-translucido` | `--color-danger-translucent` |
| `--cor-perigo-translucido-hover` | `--color-danger-translucent-hover` | `--cor-perigo-borda` | `--color-danger-border` |
| `--cor-marca-gradiente-inicio` | `--color-brand-gradient-start` | `--cor-marca-gradiente-fim` | `--color-brand-gradient-end` |
| `--cor-marca-simbolo` | `--color-brand-symbol` | `--cor-marca-sobre-cor` | `--color-brand-on-color` |
| `--cor-ambiente-dev-tint` | `--color-environment-dev-tint` | `--cor-ambiente-stage-tint` | `--color-environment-stage-tint` |
| `--cor-ambiente-prod-tint` | `--color-environment-prod-tint` | `--cor-selo-neutro-fundo` | `--color-badge-neutral-bg` |
| `--cor-grupo-1-cor` | `--color-group-1-color` | `--cor-grupo-2-cor` | `--color-group-2-color` |
| `--cor-grupo-3-cor` | `--color-group-3-color` | `--cor-grupo-4-cor` | `--color-group-4-color` |
| `--cor-grupo-5-cor` | `--color-group-5-color` | `--cor-impressao-papel` | `--color-print-paper` |
| `--cor-impressao-texto` | `--color-print-text` | `--cor-impressao-texto-secundario` | `--color-print-text-secondary` |
| `--cor-impressao-linha` | `--color-print-line` | `--cor-marca-sobre-cor-inativo` | `--color-brand-on-color-inactive` |
| `--cor-marca-inativo` | `--color-brand-inactive` | `--cor-sucesso-tint` | `--color-success-tint` |
| `--cor-info-borda-suave` | `--color-info-border-soft` | `--cor-aviso-borda-suave` | `--color-warning-border-soft` |
| `--cor-perigo-borda-suave` | `--color-danger-border-soft` | `--cor-sucesso-borda-suave` | `--color-success-border-soft` |
| `--cor-info-sutil` | `--color-info-subtle` | `--cor-marca-grafite` | `--color-brand-graphite` |
| `--cor-fundo-sutil-transparente` | `--color-bg-subtle-transparent` | `--familia-texto` | `--font-family-text` |
| `--familia-mono` | `--font-family-mono` | `--peso-regular` | `--weight-regular` |
| `--peso-medio` | `--weight-medium` | `--peso-semibold` | `--weight-semibold` |
| `--peso-negrito` | `--weight-bold` | `--tamanho-xs` | `--size-xs` |
| `--altura-linha-xs` | `--line-height-xs` | `--tamanho-sm` | `--size-sm` |
| `--altura-linha-sm` | `--line-height-sm` | `--tamanho-base` | `--size-base` |
| `--altura-linha-base` | `--line-height-base` | `--tamanho-md` | `--size-md` |
| `--altura-linha-md` | `--line-height-md` | `--tamanho-lg` | `--size-lg` |
| `--altura-linha-lg` | `--line-height-lg` | `--tamanho-xl` | `--size-xl` |
| `--altura-linha-xl` | `--line-height-xl` | `--tamanho-2xl` | `--size-2xl` |
| `--altura-linha-2xl` | `--line-height-2xl` | `--espacamento-letra-normal` | `--letter-spacing-normal` |
| `--espacamento-letra-titulo` | `--letter-spacing-title` | `--tamanho-3xl` | `--size-3xl` |
| `--altura-linha-3xl` | `--line-height-3xl` | `--tamanho-2xs` | `--size-2xs` |
| `--espaco-0` | `--space-0` | `--espaco-2` | `--space-2` |
| `--espaco-4` | `--space-4` | `--espaco-6` | `--space-6` |
| `--espaco-8` | `--space-8` | `--espaco-12` | `--space-12` |
| `--espaco-16` | `--space-16` | `--espaco-20` | `--space-20` |
| `--espaco-24` | `--space-24` | `--espaco-32` | `--space-32` |
| `--espaco-40` | `--space-40` | `--espaco-48` | `--space-48` |
| `--espaco-64` | `--space-64` | `--controle-sm` | `--control-sm` |
| `--controle-md` | `--control-md` | `--controle-lg` | `--control-lg` |
| `--icone-sm` | `--icon-sm` | `--icone-md` | `--icon-md` |
| `--icone-lg` | `--icon-lg` | `--linha-fina` | `--line-thin` |
| `--linha-grossa` | `--line-thick` | `--linha-icone` | `--line-icon` |
| `--espaco-80` | `--space-80` | `--espaco-96` | `--space-96` |
| `--controle-padrao` | `--control-default` | `--icone-xs` | `--icon-xs` |
| `--raio-nenhum` | `--radius-none` | `--raio-sm` | `--radius-sm` |
| `--raio-md` | `--radius-md` | `--raio-lg` | `--radius-lg` |
| `--raio-xl` | `--radius-xl` | `--raio-2xl` | `--radius-2xl` |
| `--raio-total` | `--radius-full` | `--raio-3xl` | `--radius-3xl` |
| `--raio-controle` | `--radius-control` | `--raio-item` | `--radius-item` |
| `--raio-cartao` | `--radius-card` | `--raio-painel` | `--radius-panel` |
| `--raio-janela` | `--radius-window` | `--raio-pilula` | `--radius-pill` |
| `--largura-janela` | `--width-window` | `--largura-barra-lateral` | `--width-sidebar` |
| `--largura-conteudo` | `--width-content` | `--largura-formulario` | `--width-form` |
| `--largura-campo` | `--width-field` | `--largura-select-filtro` | `--width-filter-select` |
| `--largura-dialogo` | `--width-dialog` | `--largura-dialogo-largo` | `--width-wide-dialog` |
| `--largura-toast` | `--width-toast` | `--largura-popup` | `--width-popup` |
| `--largura-coluna-tipo` | `--width-column-type` | `--largura-coluna-ambiente` | `--width-column-environment` |
| `--largura-coluna-nivel` | `--width-column-level` | `--largura-coluna-acesso` | `--width-column-access` |
| `--largura-coluna-lista` | `--width-column-list` | `--largura-busca` | `--width-search` |
| `--largura-interruptor` | `--width-switch` | `--pagina-a4-largura` | `--page-a4-width` |
| `--pagina-a4-altura` | `--page-a4-height` | `--largura-menu` | `--width-menu` |
| `--largura-select-compacto` | `--width-compact-select` | `--elevacao-nenhum` | `--elevation-none` |
| `--elevacao-flutuante-contato-y` | `--elevation-floating-contact-y` | `--elevacao-flutuante-contato-desfoque` | `--elevation-floating-contact-blur` |
| `--elevacao-flutuante-ambiente-y` | `--elevation-floating-environment-y` | `--elevacao-flutuante-ambiente-desfoque` | `--elevation-floating-environment-blur` |
| `--elevacao-modal-contato-y` | `--elevation-modal-contact-y` | `--elevacao-modal-contato-desfoque` | `--elevation-modal-contact-blur` |
| `--elevacao-modal-ambiente-y` | `--elevation-modal-environment-y` | `--elevacao-modal-ambiente-desfoque` | `--elevation-modal-environment-blur` |
| `--elevacao-brilho-y` | `--elevation-glow-y` | `--elevacao-brilho-desfoque` | `--elevation-glow-blur` |
| `--tom-icone` | `--icon-tone` | `--sombra-brilho-primario` | `--shadow-glow-primary` |
| `--sombra-flutuante` | `--shadow-floating` | `--sombra-modal` | `--shadow-modal` |
| `.visualmente-oculto` | `.visually-hidden` | `.mono-corpo` | `.mono-body` |
| `.mono-pequeno` | `.mono-small` | `.mono-pin` | `.mono-pin` |
| `.texto-corpo` | `.text-body` | `.texto-corpo-forte` | `.text-body-strong` |
| `.texto-etiqueta` | `.text-tag` | `.texto-legenda` | `.text-caption` |
| `.texto-pequeno` | `.text-small` | `.texto-rotulo` | `.text-label` |
| `.texto-selo` | `.text-badge` | `.texto-selo-grande` | `.text-badge-large` |
| `.titulo-destaque` | `.title-highlight` | `.titulo-modal` | `.title-modal` |
| `.titulo-pagina` | `.title-page` | `.titulo-secao` | `.title-section` |

## 15. Tipos públicos do Rust

Todo `pub`/`pub(crate)` `struct`, `enum`, `trait` e `type` com nome pt-BR (308 no
inventário; os já em inglês, como `Api`, `Aws`, `Cdp`, `Cli`, `Config`, `Daemon`,
`Login`, `Pin`, `Pty`, `Smtp`, `Stdio`, `Terminal`, `Token`, `Totp`, `Udisks`, `WlCopy`,
`R`, `Bytes`, `Base`, `Dir`, `Site`, ficam). Variantes de enum seguem
a seção 2 (`ErroResposta::NaoAutorizado` → `ResponseError::Unauthorized`).

| Antigo | Novo | Antigo | Novo |
|---|---|---|---|
| `Aberto` | `Open` | `Abertura` | `Opening` |
| `AbridorApp` | `AppOpener` | `AbridorReal` | `RealOpener` |
| `Acao` | `Action` | `AgentePreparado` | `PreparedAgent` |
| `AgenteSsh` | `SshAgent` | `Alvo` | `Target` |
| `Ambiente` | `Environment` | `AmbienteApp` | `AppEnvironment` |
| `Anexo` | `Attachment` | `AnexoDetalhe` | `AttachmentDetail` |
| `Anexos` | `Attachments` | `Aparencia` | `Appearance` |
| `AppDeTeste` | `TestApp` | `AppLancado` | `LaunchedApp` |
| `AprovacaoSimulada` | `SimulatedApproval` | `Aprovacoes` | `Approvals` |
| `AreaFalsa` | `FakeClipboard` | `AreaGtk` | `GtkClipboard` |
| `AreaTransferencia` | `Clipboard` | `ArquivoEstado` | `StateFile` |
| `Assinante` | `Subscriber` | `Atual` | `Current` |
| `Autent` (wardd `ssh.rs`) | `SshAuth` | `Autenticacao` | `Auth` |
| `Automato` | `Automaton` | `Autorizacao` | `Authorization` |
| `Aviso` | `Notice` | `BackupExterno` | `ExternalBackup` |
| `BackupExternoVisto` | `ExternalBackupView` | `BackupFalso` | `FakeBackup` |
| `Banco` | `Database` | `BinarioDeTeste` | `TestBinary` |
| `BinarioPermitido` | `AllowedBinary` | `BufferTravado` | `LockedBuffer` |
| `Cabecalho` | `Header` | `CaminhoInvalido` | `InvalidPath` |
| `Caminhos` | `Paths` | `CampoExtra` | `ExtraField` |
| `Campos` | `Fields` | `CampoSecreto` | `SecretField` |
| `CampoTexto` | `TextField` | `CanalConectar` | `ConnectChannel` |
| `Carregamento` | `Loading` | `Chave` | `Key` |
| `ChavesCofre` | `VaultKeys` | `ClassePar` | `PeerClass` |
| `Cliente` | `Client` | `CodigoTotp` | `TotpCode` |
| `CofreAberto` | `OpenVault` | `Comando` | `Command` |
| `Conexao` | `Connection` | `Configuracao` | `VaultConfig` (é a configuração do cofre; `Config` já existe no `wardd`) |
| `Configuracoes` | `Settings` | `ConstrutorMascara` | `MaskBuilder` |
| `Conta` | `Account` | `Copia` (wardd `externo.rs`) | `BackupCopy` (não `Copy`, que é o trait) |
| `Copiador` | `Copier` | `CorpoLido` | `ReadBody` |
| `Cred` | `Cred` | `CredenciaisPar` | `PeerCredentials` |
| `Dados` | `Data` | `DaemonFalso` | `FakeDaemon` |
| `DaemonSocket` | `DaemonSocket` | `Datas` | `Dates` |
| `Decisao` | `Decision` | `Destino` | `Destination` |
| `DetalheCredencial` | `CredentialDetail` | `DetalheEntrada` | `EntryDetail` |
| `Digitado` | `Typed` | `DiretorioSocket` | `SocketDir` |
| `DirTeste` | `TestDir` | `EdicaoDosAnexos` | `AttachmentEdit` |
| `ElementoCadeia` | `ChainElement` | `Entorno` | `Env` |
| `Entrada` | `Entry` | `EntradaExibida` | `DisplayedEntry` |
| `Entrega` | `Delivery` | `ErroAbrir` | `OpenError` |
| `ErroAbrirApp` | `OpenAppError` | `ErroAceitar` | `AcceptError` |
| `ErroAgente` | `AgentError` | `ErroApp` | `AppError` |
| `ErroAtivacao` | `ActivationError` | `ErroBackup` | `BackupError` |
| `ErroBinarioDeTeste` | `TestBinaryError` | `ErroCampo` | `FieldError` |
| `ErroCanal` | `ChannelError` | `ErroCdp` | `CdpError` |
| `ErroCodificar` | `EncodeError` | `ErroComando` | `CommandError` |
| `ErroConexao` | `ConnectionError` | `ErroConfiguracao` | `ConfigError` |
| `ErroCorpo` | `BodyError` | `ErroDiretorio` | `DirError` |
| `ErroEntrega` | `DeliveryError` | `ErroEscuta` | `ListenError` |
| `ErroEstado` | `StateError` | `ErroExec` | `ExecError` |
| `ErroExterno` | `ExternalError` | `ErroFormato` | `FormatError` |
| `ErroGerador` | `GeneratorError` | `ErroGravacao` | `WriteError` |
| `ErroInicio` | `StartError` | `ErroInstalacao` | `InstallError` |
| `ErroLancador` | `LauncherError` | `ErroLancar` | `LaunchError` |
| `ErroMascara` | `MaskError` | `ErroMensagem` | `MessageError` |
| `ErroPartida` | `StartupError` | `ErroQuadro` | `FrameError` |
| `ErroRecurso` | `ResourceError` | `ErroResposta` | `ResponseError` |
| `ErroSessao` | `SessionError` | `ErrosValidacao` | `ValidationErrors` |
| `ErroTerminal` | `TerminalError` | `ErroTotp` | `TotpError` |
| `ErroUso` | `UsageError` | `ErroValidacao` | `ValidationError` |
| `Escolha` | `Choice` | `EscolhaDoPendrive` | `UsbDriveChoice` |
| `EscritaSessao` | `SessionWriter` | `Escritor` | `Writer` |
| `Escuta` | `Listener` | `Estado` | `State` (o do Tauri: `AppState`) |
| `EstadoBandeja` | `TrayState` | `EstadoCofre` | `VaultStatus` |
| `Etapa` | `Step` | `Evento` | `Event` |
| `ExtensaoDeTeste` | `TestExtension` | `Extra` | `Extra` |
| `ExtraDetalhe` | `ExtraDetail` | `ExtraFormulario` | `FormExtra` |
| `FalhaCripto` | `CryptoFailure` | `FalhaDaemon` | `DaemonFailure` |
| `Falta` | `Missing` | `FilaAvisos` | `NoticeQueue` |
| `Filho` | `Child` | `FilhoCru` | `RawChild` |
| `Fim` | `End` | `Fluxo` | `Stream` |
| `FontePendrives` | `UsbDriveSource` | `Fontes` | `Sources` |
| `Forca` | `Strength` | `Forma` (wardd `execucao.rs`) | `Shape` |
| `Formulario` | `Form` | `FtpSftp` | `FtpSftp` |
| `Gancho` | `Hook` | `Geracoes` | `Generations` |
| `Gerada` | `Generated` | `Gerador` | `Generator` |
| `GrandeDemais` | `TooLarge` | `Gravado` | `Written` |
| `Grupo` | `Group` | `HashPin` | `PinHash` |
| `Icone` | `Icon` | `IdentidadeArquivo` | `FileIdentity` |
| `InfoPedido` | `RequestInfo` | `Instalacao` | `Install` |
| `Intermediario` | `Intermediate` | `ItemCredencial` | `CredentialItem` |
| `ItemLista` | `ListItem` | `Janela` | `WindowSize` (tamanho do terminal) |
| `JsonInvalido` | `InvalidJson` | `Lancador` | `Launcher` |
| `LerVersao` | `ReadVersion` | `Limites` | `Limits` |
| `Logins` | `Logins` | `Mascara` | `Mask` |
| `Memoria` | `Memory` | `ModoCru` | `RawMode` |
| `Montado` (`app/core/form.rs`: as partes montadas do rascunho) | `Built` | `MorreNaTrava` | `DiesOnLock` |
| `Motor` | `Engine` | `NivelAcesso` | `AccessLevel` |
| `NivelMudado` | `LevelChanged` | `NotaSegura` | `SecureNote` |
| `NovoCofre` | `NewVault` | `Objetos` | `Objects` |
| `Opcoes` | `Options` | `OpcoesGerador` | `GeneratorOptions` |
| `Origem` | `Origin` | `OrigemSegredo` | `SecretOrigin` |
| `Pai` | `Parent` | `PaiDiferente` | `ParentMismatch` |
| `Papeis` | `Roles` | `Papel` | `Role` |
| `ParametrosKdf` | `KdfParams` | `Partida` | `Startup` |
| `PastaAberta` | `OpenDir` | `PastaBackup` | `BackupDir` |
| `Pedido` | `Request` | `PedidoDeAprovacao` | `ApprovalRequest` |
| `PedidoDeDestrave` | `UnlockRequest` | `PedidoExecutar` | `RunRequest` |
| `PedidoExibido` | `DisplayedRequest` | `PedidoPendente` | `PendingRequest` |
| `Pendente` | `Pending` | `Pendrive` | `UsbDrive` |
| `PendriveVisto` | `UsbDriveView` | `Permitido` | `Allowed` |
| `PhcPin` | `PinPhc` | `PipeDepuracao` | `DebugPipe` |
| `Pipes` | `Pipes` | `PoliticaKernel` | `KernelPolicy` |
| `PontasPai` | `ParentEnds` | `Ponte` | `Bridge` |
| `PonteRodando` | `RunningBridge` | `Ponto` | `Point` |
| `PrazosLogin` | `LoginDeadlines` | `Preparo` | `Preparation` |
| `PreparoExec` | `ExecPreparation` | `Problema` | `Problem` |
| `ProcessoPar` | `PeerProcess` | `Programa` | `Program` |
| `ProgramasSsh` | `SshPrograms` | `Protocolo` | `Protocol` |
| `Rascunho` | `Draft` | `RascunhoAnexo` | `AttachmentDraft` |
| `RascunhoDados` | `DataDraft` | `RascunhoExtra` | `ExtraDraft` |
| `Recursos` | `Resources` | `Recusa` | `Refusal` |
| `RecusaCaminho` | `PathRefusal` | `RecusaServidor` | `ServerRefusal` |
| `Referencia` | `Reference` | `Regra` | `Rule` |
| `Relatorio` | `Report` | `Relogio` | `Clock` |
| `RelogioFalso` | `FakeClock` | `RelogioMonotonico` | `MonotonicClock` |
| `RelogioMotor` | `EngineClock` | `RelogioReal` | `RealClock` |
| `Respondida` | `Answered` | `Resposta` | `Response` |
| `RespostaInvalida` | `InvalidResponse` | `Restauracao` | `Restore` |
| `Restaurado` | `Restored` | `ResultadoAprovacao` | `ApprovalResult` |
| `ResultadoCriar` | `CreateResult` | `ResultadoLogin` | `LoginResult` |
| `ResultadoSalvar` | `SaveResult` | `ResultadoTroca` | `ChangeResult` |
| `ResumoItem` | `ItemSummary` | `Revelada` | `Revealed` |
| `Rodando` | `Running` | `Rota` | `Route` |
| `SaidaDeErro` | `Stderr` | `Salvamento` | `Save` |
| `Segredo` | `Secret` | `SegredoDaEntrada` | `EntrySecret` |
| `SegredoMontado` | `BuiltSecret` | `SegurancaSmtp` | `SmtpSecurity` |
| `Seletor` | `Picker` | `SeletorDePasta` | `DirPicker` |
| `SemApp` | `NoApp` | `SemCliente` | `NoClient` |
| `SemSeletor` | `NoPicker` | `SemTerminal` | `NoTerminal` |
| `SenhaMestra` | `MasterPassword` | `SenhaTrocada` | `PasswordChanged` |
| `Servico` | `Service` | `Servidor` | `Server` |
| `ServidorEsperado` | `ExpectedServer` | `Sessao` | `Session` |
| `SessoesRun` | `RunSessions` | `Sistema` | `System` |
| `Sugestao` | `Suggestion` | `Temporarios` | `TempFiles` |
| `TerminalDeTeste` | `TestTerminal` | `TerminalDoRepositorio` | `RepoTerminal` |
| `TerminalDoSistema` | `SystemTerminal` | `TextoSecreto` | `SecretText` |
| `TipoEntrada` | `EntryType` | `TipoEntradaPadrao` | `StdinKind` |
| `TokenConectar` | `ConnectToken` | `TrocaPin` | `PinChange` |
| `TrocaSenha` | `PasswordChange` | `Valor` | `Value` |
| `ValorExtra` | `ExtraValue` | `ValorSecreto` | `SecretValue` |
| `Variavel` | `Variable` | `Versao` | `Version` |
| `VersaoBackup` | `BackupVersion` | `VersaoVista` | `VersionView` |
| `Visto` | `Seen` | `TipoResposta` | `ResponseType` |
| `Operacao` | `Operation` | `ErroEnvelope` | `EnvelopeError` |

Variantes fixadas na m2: Fase 8 (aparecem no `Debug` que os scripts do contêiner e do
`make test-nm` procuram): `Recusa` → `Refusal` com `UidDiferente` → `UidMismatch`,
`ProcessoSumiu` → `ProcessVanished`, `SemAcesso` → `NoAccess`, `Leitura` → `Read`,
`ExeNaoPermitido` → `ExeNotAllowed`, `ExeTrocado` → `ExeSwapped`,
`InstalacaoNaoConfiavel` → `UntrustedInstall`, `Depurador` → `Debugger`, `VariavelLd` →
`LdVariable`, `AmbienteGrande` → `LargeEnvironment`, `BibliotecaNaoConfiavel` →
`UntrustedLibrary`, `PaiNaoPermitido` → `ParentNotAllowed`, `GrupoComum` →
`CommonGroup`, `PoliticaKernel` → `KernelPolicy`, `PaiDoAppNaoPermitido` →
`AppParentNotAllowed`; `ClassePar` → `PeerClass` com `App`, `HostExtensao` →
`ExtensionHost`, `Comum { motivo }` → `Common { reason }`; `ErroExec` → `ExecError` com
`Sistema` → `System`, `Filho { etapa, errno }` → `Child { step, errno }`; `PontasPai` →
`ParentEnds` com `Pipes { stdin, stdout, stderr }`, `Pty`, `DebugPipe { commands,
responses }`; `Stdio::Depuracao` → `Stdio::DebugPipe`; `descendentes::Relatorio` →
`descendants::Report { killed, remaining }`; `registro()` dos líderes → `record()`.

Fase 9 (o `Debug` de `ServerRefusal` e de `ActivationError` aparece no
`dev/container-platform/inside.sh` e nos testes de ativação): `RecusaServidor` →
`ServerRefusal` com `Ausente` → `Missing`, `CaminhoInvalido` → `InvalidPath`,
`PastaNaoConfiavel` → `UntrustedDir`, `SocketNaoConfiavel` → `UntrustedSocket`,
`ParNaoConfiavel { uid }` → `UntrustedPeer { uid }`, `EnderecoDiferente` →
`AddressMismatch`; `ErroAtivacao` → `ActivationError` com `SemAtivacao` →
`NotActivated`, `PidDiferente` → `PidMismatch`, `QuantidadeDeFds` → `FdCount`,
`FdIndisponivel` → `FdUnavailable`, `NaoEscutaUnix` → `NotUnixListener`,
`CaminhoDiferente` → `PathMismatch`, `PastaNaoConfiavel` → `UntrustedDir`,
`SocketNaoConfiavel` → `UntrustedSocket`;
`ErroDiretorio` → `DirError` com `CaminhoInvalido` → `InvalidPath`, `PaiNaoConfiavel` →
`UntrustedParent`, `NaoEDiretorio` →
`NotDirectory`, `DonoErrado { uid }` → `WrongOwner { uid }`, `PermissaoErrada { modo }` →
`WrongMode { mode }`, `Trocado` → `Swapped`; `ErroEscuta` → `ListenError` com
`NomeInvalido` → `InvalidName`, `JaEmUso` → `AlreadyInUse`, `CaminhoOcupado` →
`PathTaken`, `Diretorio` → `Dir`; `ErroAceitar` → `AcceptError` com `UidRecusado` → `UidRefused`,
`SemIdentidade` → `NoIdentity` (o log do `wardd` mantém o texto `(UidRecusado)` e
`(SemIdentidade)`, que o `dev/container-uid/inside.sh` procura); `Memoria` → `Memory`
com `Travada` → `Locked`, `SemTravar` → `NotLocked`; `PaiDiferente` → `ParentMismatch` com `Orfao` →
`Orphan`, `NaoEoDaemon` → `NotDaemon`, `PaiDumpable` → `DumpableParent`, `Saiu` →
`Exited`, `Leitura` → `Read`; `ErroInicio` → `StartError` com `Endurecer` → `Harden`, `VariavelInsegura` →
`InsecureVariable`,
`Depurador` → `Debugger`, `LerStatus` → `ReadStatus`, `LimiteMemlock` → `MemlockLimit`,
`TravarMemoria` → `LockMemory`.

Fase 10 (`ward-core`: `limits`, `crypto`, `totp`, `generator`, `origin`, `display`):
`ErroTotp` → `TotpError` com `SementeInvalida` → `InvalidSeed`, `DigitosInvalidos` →
`InvalidDigits`, `RelogioInvalido` → `InvalidClock`; `CodigoTotp` (o tipo do `totp`,
não a resposta do protocolo) → `TotpCode` (`codigo()` → `code()`, `restante()` →
`remaining()`), `Totp::de_semente`/`com_digitos`/`codigo_em`/`codigo_agora` →
`from_seed`/`with_digits`/`code_at`/`code_now`; `ErroGerador` → `GeneratorError` com
`TamanhoInvalido` → `InvalidSize`, `NenhumConjunto` → `NoSet`,
`AleatorioIndisponivel` → `RandomUnavailable`; `OpcoesGerador { tamanho, minusculas,
maiusculas, digitos, simbolos }` → `GeneratorOptions { size, lowercase, uppercase,
digits, symbols }`; `gerar`/`gerar_com` → `generate`/`generate_with`,
`token_conectar` → `connect_token`, `tokens_iguais` → `tokens_equal`; `cripto::Chave`
→ `crypto::Key` (o `Key` do `chacha20poly1305` entra como `AeadKey`), `FalhaCripto` →
`CryptoFailure`, `PhcPin` → `PinPhc`, `selar`/`abrir`/`aleatorio`/`chave_aleatoria`/
`derivar_kek`/`conferir_pin`/`ler_phc_pin`/`iguais_em_tempo_constante` →
`seal`/`open`/`random`/`random_key`/`derive_kek`/`verify_pin`/`read_pin_phc`/
`constant_time_eq`, `TAM_CHAVE`/`TAM_NONCE`/`TAM_TAG` → `KEY_SIZE`/`NONCE_SIZE`/
`TAG_SIZE`; `Origem::da_url` → `Origin::from_url`, `mesma_origem` → `same_origin`;
`exibir_argumentos`/`exibir_texto`/`neutralizar`/`invisivel`/`AVISO_CORTE` →
`display_args`/`display_text`/`neutralize`/`invisible`/`CUT_NOTICE`. Constantes de
`limits` fora da regra direta: `MAX_BASE` → `MAX_DB_NAME`, `MAX_BASE_REDIS` →
`MAX_REDIS_DB_NAME`, `MAX_ANEXOS_SOMADOS` → `MAX_ATTACHMENTS_TOTAL`, `LIMITE_PADROES`
→ `PATTERN_LIMIT`, `ORCAMENTO_MASCARA` → `MASK_BUDGET`, `MINIMO_CONFIAVEL` →
`TRUSTED_MINIMUM`, `MIN_CARACTERES_SENHA_MESTRA` → `MIN_MASTER_PASSWORD_CHARS`,
`MINUTOS_TRAVA_PADRAO` → `DEFAULT_LOCK_MINUTES`, `MAX_QUADRO_COMUM`/`_APP`/`_RESPOSTA`
→ `MAX_COMMON_FRAME`/`MAX_APP_FRAME`/`MAX_RESPONSE_FRAME`, `MAX_VARIAVEIS_AMBIENTE` →
`MAX_ENV_VARIABLES`, `MAX_TEXTO_EXIBIDO` → `MAX_DISPLAYED_TEXT`. O `backup::MAX_NOME`
(Fase 15) e o `MAX_CORPO` da ponte de teste (Fase 29) são outros itens e não mudaram.

Fase 11 (`ward-core`: `format`, `atomic_write`): módulos `formato` → `format` e
`gravacao` → `atomic_write`; `MAGIA`/`VERSAO`/`TAM_SAL`/`TAM_CABECALHO`/`MAX_CONTEUDO`
→ `MAGIC`/`VERSION`/`SALT_SIZE`/`HEADER_SIZE`/`MAX_CONTENT` (a magia `WARDCOFR` fica);
`ParametrosKdf { memoria_kib, passes, paralelismo }` → `KdfParams { memory_kib, passes,
parallelism }` com `PADRAO`/`MINIMO`/`MAXIMO` → `DEFAULT`/`MINIMUM`/`MAXIMUM`;
`Cabecalho { versao, kdf, tamanho_corpo }` → `Header { version, kdf, body_size }`;
`ErroFormato` → `FormatError` com `SenhaOuArquivo` → `PasswordOrFile`,
`VersaoNaoSuportada` → `UnsupportedVersion`, `ParametrosAbaixoDoMinimo`/
`ParametrosAcimaDoMaximo` → `ParamsBelowMinimum`/`ParamsAboveMaximum`,
`ConteudoGrande` → `ContentTooLarge`, `SenhaVazia` → `EmptyPassword`, `Aleatorio` →
`Random`, `Cifrar` → `Encrypt`; `SenhaMestra` → `MasterPassword` (`nova`/`de_bytes`/
`digitada`/`serve_de_nova` → `new`/`from_bytes`/`typed`/`is_valid_new`);
`CofreAberto` → `OpenVault` (`novo`/`novo_com`/`depois_da_geracao`/`de_partes`/
`geracao`/`proxima_geracao`/`queimar`/`parametros`/`configuracao`/`configuracao_mut`/
`selar`/`autentica`/`geracao_selada` → `new`/`new_with`/`after_generation`/`from_parts`/
`generation`/`next_generation`/`burn`/`params`/`config`/`config_mut`/`seal`/
`authenticates`/`sealed_generation`); `ChavesCofre` → `VaultKeys` (`chave_dados` →
`data_key`, `abrir_proprio` → `open_own`); `ler_cabecalho` → `read_header`, `abrir` →
`open`, `trocar_senha` → `change_password`, `SenhaTrocada` → `PasswordChanged`.
`IdentidadeArquivo` → `FileIdentity`; `Etapa` → `Step` com `Ler` → `Read`,
`CriarTemporario` → `CreateTemp`, `Escrever` → `Write`, `FsyncTemporario` →
`FsyncTemp`, `Renomear` → `Rename`, `FsyncDiretorio` → `FsyncDir`; `ErroGravacao` →
`WriteError` com `GeracaoEsgotada` → `GenerationExhausted`, `Retrocesso` → `Rollback`,
`NaoECofre` → `NotVault`, `Formato` → `Format`, `Corpo` → `Body`, `DestinoInvalido` →
`InvalidDestination`, `Io { etapa, tipo, identidade }` → `Io { step, kind, identity }`;
`Ponto` → `Point` com `DepoisDeCriar`/`Escrevendo`/`DepoisDeEscrever`/`DepoisDoFsync`/
`DepoisDoRename`/`DepoisDoFsyncDir` → `AfterCreate`/`Writing`/`AfterWrite`/`AfterFsync`/
`AfterRename`/`AfterFsyncDir`; `Gravado { identidade, backup }` → `Written { identity,
backup }`; `gravar_entradas`(`_com_backup`) → `write_entries`(`_with_backup`),
`trocar_senha_lido`/`trocar_senha_com_backup_lido` → `change_password_from_bytes`/
`change_password_with_backup_from_bytes` ("lido" = com os bytes já lidos pelo
chamador → `from_bytes`), `gravar_cofre` → `write_vault`, `gravar` → `write`,
`limpar_temporarios` → `clean_temp_files` (só o do `atomic_write`: os
`limpar_temporarios` de `wardd/execucao.rs` e `wardd/externo.rs` são outros e ficam
para as fases deles), `gravar_novo_com_pontos` → `write_new_with_points`,
`Temporario` → `TempFile`. Os ids `ErroResposta::Retrocesso`/`GeracaoEsgotada` do
protocolo (Fase 17) e `ErroAbrir::Retrocesso` do corpo (Fase 14) são outros e não
mudaram; o `backup::geracao_selada` (Fase 15) também não.

Fase 12 (`ward-core`: `entry`, `entry/secret`, `entry/validation`): módulo `entrada` →
`entry` (`segredo` → `secret`, `validacao` → `validation`, `testes` → `tests`);
`Segredo` → `Secret` (`de_texto`/`de_bytes`/`expor`/`expor_texto` → `from_text`/
`from_bytes`/`expose`/`expose_text`; o `Segredo` do `app/core` é outro e não mudou);
`Ambiente` → `Environment`, `NivelAcesso` → `AccessLevel` com `Bloqueado`/`ComAprovacao`/
`Livre` → `Blocked`/`Approval`/`Free` (`mais_restritivo` → `most_restrictive`);
`TipoEntrada` → `EntryType` com `Banco` → `Database`, `Servidor` → `Server`, `NotaSegura`
→ `SecureNote`, `Generico` → `Generic`; `Motor` → `Engine` (o `Motor` do
`wardd/aprovacao.rs` é outro); `SegurancaSmtp` → `SmtpSecurity` com `Nenhuma` → `None`;
`Protocolo` → `Protocol`; `TODOS`/`TODAS` → `ALL`; `nome`/`de_nome`/`rotulo`/
`porta_padrao` → `name`/`from_name`/`label`/`default_port` (os literais de `name()` são
nomes gravados no cofre e ficam até a Fase 48; os de `label()` são texto exibido e ficam);
`Autenticacao { Senha, Chave { chave_privada, passphrase } }` → `Auth { Password, Key {
private_key, passphrase } }`; campos `usuario`/`senha`/`totp_semente`/`motor`/`porta`/
`base`/`tunel`/`autenticacao`/`salto`/`variavel`/`regiao`/`seguranca`/`protocolo`/`texto`
→ `username`/`password`/`totp_seed`/`engine`/`port`/`db_name`/`tunnel`/`auth`/`jump`/
`variable`/`region`/`security`/`protocol`/`text`; `Banco`/`Servidor`/`NotaSegura` (structs)
→ `Database`/`Server`/`SecureNote`; `Dados` → `Data` (`tipo()` → `kind()`, `Generico` →
`Generic`); `ValorExtra { Texto, Segredo }` → `ExtraValue { Text, Secret }` (o do
`app/core/formulario.rs` é outro); `CampoExtra` → `ExtraField` (`e_segredo` →
`is_secret`); `Anexo` → `Attachment` (`conteudo` → `content`; os `Anexo` de
`wardd/playwright.rs` e `app/core/credenciais.rs` são outros); `OrigemSegredo { Campo,
Extra, Anexo }` → `SecretOrigin { Field, Extra, Attachment }`; `SegredoDaEntrada {
origem, valor }` → `EntrySecret { origin, value }`; `Datas { criada_em, alterada_em }` →
`Dates { created_at, modified_at }` (`NENHUMA`/`instante`/`de_partes`/`carimbo_novo`/
`alterado` → `NONE`/`instant`/`from_parts`/`new_stamp`/`modified`); `Entrada` → `Entry`
(`grupo`/`ambiente`/`projeto`/`nivel`/`definir_nivel`/`datas`/`definir_datas`/`tipo`/
`dados`/`anexos`/`segredos`/`substituir_por`/`referencia`/`conferir_no_cofre` → `group`/
`environment`/`project`/`level`/`set_level`/`dates`/`set_dates`/`kind`/`data`/
`attachments`/`secrets`/`replace_with`/`reference`/`verify_in_vault`).
`ErroValidacao { campo, posicao, motivo }` → `ValidationError { field, position, reason
}`; `ErrosValidacao` → `ValidationErrors` (`lista`/`tem`/`tem_motivo` → `list`/`has`/
`has_reason`); `Rascunho` → `Draft` (`validar` → `validate`), `RascunhoDados` →
`DataDraft`, `RascunhoExtra { nome, valor, segredo }` → `ExtraDraft { name, value, secret
}`, `RascunhoAnexo` → `AttachmentDraft`; `GRANDE_PARA_MASCARA` → `TOO_LARGE_FOR_MASK`;
`nome_variavel` → `variable_name`, `origem_da_url` → `url_origin`,
`origem_local_da_url` → `local_url_origin`, `url_valida` → `valid_url`,
`separar_host_porta` → `split_host_port`, `VARIAVEIS_COMUNS` → `COMMON_VARIABLES`,
`variaveis_do_tipo`/`anexos_do_tipo` → `type_variables`/`type_attachments`; `Coletor` →
`Collector`; predicados `*_valido(a)` → `valid_*` (`nome_entrada_valido` →
`valid_entry_name`, `chave_parece_privada` → `looks_like_private_key`).

Fase 13 (`ward-core`: `config`, `entry/tests`): módulo `configuracao` → `config`
(`testes` → `tests`); `Configuracao` → `VaultConfig` (`nova`/`de_partes`/`conferir_pin`/
`hash_pin`/`trocar_hash_pin`/`minutos_trava`/`definir_minutos_trava`/`definir_backup` →
`new`/`from_parts`/`verify_pin`/`pin_hash`/`change_pin_hash`/`lock_minutes`/
`set_lock_minutes`/`set_backup`); `HashPin` → `PinHash` (`criar`/`conferir`/`parametros`/
`de_phc` → `create`/`verify`/`params`/`from_phc`); `BackupExterno { ligado, uuid }` →
`ExternalBackup { enabled, uuid }` (`desligado`/`novo`/`ligado` → `disabled`/`new`/
`enabled`); `ErroConfiguracao` → `ConfigError` com `PinInvalido` → `InvalidPin`,
`TempoDeTrava` → `LockTimeout`, `UuidInvalido` → `InvalidUuid`, `BackupSemUuid` →
`BackupWithoutUuid` (`Hash` fica); `KDF_PIN_MINIMO`/`KDF_PIN_MAXIMO` →
`KDF_PIN_MINIMUM`/`KDF_PIN_MAXIMUM` (`KDF_PIN` fica); `conferir_texto` → `verify_text`,
`minutos_validos` → `valid_minutes`, `uuid_valido` → `valid_uuid`, `pin_valido` →
`valid_pin`. Os homônimos `Operacao::Configuracao`/`BackupExterno`/`DefinirBackupExterno`
do protocolo (Fase 17), `BackupExternoVisto` e `uuid_valido` do `app/core/backup_externo.rs`
e `minutos_validos` do `app/core/configuracoes.rs` (Fase 33) são outros e não mudaram.
Nos testes de `entry`: `rascunho` → `draft`, `sem` → `without`, `NaoClone` → `NotClone`,
`zera_ao_sair` → `zeroes_on_drop`.

Fase 29 (`ward-test-bridge`): módulos `area_wl`/`partida`/`simulados` →
`wl_clipboard`/`startup`/`simulated`, `tests/ataques.rs` → `tests/attacks.rs`; `Ponte` →
`Bridge` (`abrir`/`url_de_abertura`/`origem`/`token_para_teste`/`servir`/`responder` →
`open`/`opening_url`/`origin`/`token_for_test`/`serve`/`respond`); `Pedido { metodo,
caminho, corpo }` → `Request { method, path, body }` (`novo`/`cabecalho` → `new`/
`header`); `Resposta { status, corpo }` → `Response { status, body }` (`corpo_json` →
`body_json`); `ler_pedido` → `read_request`; `MAX_CABECALHO`/`MAX_CORPO`/`MAX_CONEXOES`
→ `MAX_HEADER`/`MAX_BODY`/`MAX_CONNECTIONS`; `SEM_PAPEL_APP` → `NO_APP_ROLE`,
`conectar_com_papel_app` → `connect_with_app_role`, `dist_do_repositorio`/`repositorio`
→ `repository_dist`/`repository`; `SeletorDePasta`/`TerminalDeTeste`/
`TerminalDoRepositorio` → `DirPicker`/`TestTerminal`/`RepoTerminal` (`do_repositorio` →
`from_repository`); `WlCopy::novo` → `WlCopy::new`. O caminho `/ponte/<comando>`, o
prefixo `ward-ponte-teste:` das mensagens, o `USAGE` (antes `USO`) e as opções
`--daemon-falso`/`--anexos` com os modos ficam (fio e marca; Fase 56). Os nomes que vêm do
`ward_app_core` (`Servico`, `ErroApp`, `Comando`, os métodos dos traits…) mudam nas Fases
30–34.
Fase 14 (`ward-core`: `body`, `body/test_vault`, `body/tests`): módulo `corpo` → `body`
(`cofre_teste` → `test_vault`, `testes` → `tests`); `VERSAO_CORPO` → `BODY_VERSION` (o
valor continua 4 até a Fase 48); `ErroCorpo` → `BodyError` com `Invalido` → `Invalid`,
`GrandeDemais` → `TooLarge`, `Inconsistente` → `Inconsistent`; `ErroAbrir` → `OpenError`
com `Formato` → `Format`, `Corpo` → `Body`, `Retrocesso` → `Rollback`; `abrir_entradas` →
`open_entries`, `abrir_entradas_desde` → `open_entries_since`, `abrir_entradas_lida` →
`open_entries_with_generation` (devolve também a geração lida), `serializar`/`desserializar`
→ `serialize`/`deserialize`, `CorpoLido` → `ReadBody`, `geracao_do_corpo` →
`body_generation`; privados: `Milhar` → `Thousands`, `Campo` → `Field`, `esquema` →
`schema`, `Saida` → `Output`, `Leitor` → `Reader`, `Lista` → `List`, `Bruta` → `RawEntry`,
`BrutaConfiguracao` → `RawConfig`, `MAX_DADOS` → `MAX_DATA_FIELDS`, `ler_cabecalho` →
`read_header`, `ler_bruta` → `read_raw`, `montar`/`montar_dados`/`montar_configuracao` →
`build`/`build_data`/`build_config`, `conferir_estrutura`/`conferir_ordenadas` →
`check_structure`/`check_sorted`. Cofre de teste: `SENHA_MESTRA` → `MASTER_PASSWORD`,
`PIN_TESTE` → `TEST_PIN`, `SEGREDOS` → `SECRETS`, `configuracao_de_teste`/
`configuracao_nova` → `test_config`/`new_config`, `cofre_de_teste` → `test_vault_entries`,
`um_de_cada_cheio` → `one_of_each_full`, `chave_falsa` → `fake_key`, `validada` →
`validated`, `bytes_0_a_255` → `bytes_0_to_255`. Os homônimos `ErroAbrirApp` do `wardd`,
`ErroResposta::Retrocesso` (Fase 17), o `SEGREDOS` de `entrega/testes.rs` e de
`wardd/tests/comum`, o `chave_falsa` de `entrega/testes.rs` e de `app/core/chave.rs` e o
re-export `daemon_falso::SEGREDOS` (Fase 31) são outros e não mudaram.
Fase 30 (`app/core`: `service`, `secret`, `error`, `lib`): módulos `servico` → `service`,
`segredo` → `secret`, `erro` → `error`. `Servico` → `Service` (`novo` → `new`; os métodos
seguem os comandos da seção 13: `estado_do_cofre` → `vault_status`, `listar_credenciais`
→ `list_credentials`, `mudar_nivel` → `set_level`, `escolher_anexo` → `choose_attachment`
etc.; os outros: `pedido_de_destrave` → `unlock_request`, `info_do_pedido` →
`request_info`, `aprovacoes` → `approvals`, `negar_aprovacoes_pendentes` →
`deny_pending_approvals`, `com_seletor`/`com_terminal` → `with_picker`/`with_terminal`,
`estado_da_bandeja` → `tray_state`, `na_passagem_da_bandeja` → `on_tray_transition`,
`ver_passagem` → `check_transition`, `ao_sair` → `on_exit`); `ResultadoSalvar { salvo,
erros }` → `SaveResult { saved, errors }`, `ResultadoCriar { criado, erros }` →
`CreateResult { created, errors }`, `CodigoTotp { codigo, restante }` (o do `service`) →
`TotpCode { code, remaining }`, com `#[serde(rename = "<antigo>")]` até a Fase 54;
`FontesDoServico` → `ServiceSources`, `CAMPOS_GERAVEIS` → `GENERATABLE_FIELDS`. O
`Segredo` do `app/core` → `Secret` (`novo`/`expor` → `new`/`expose`; `Debug`
`"Secret(••••)"`). `ErroApp` → `AppError` (`codigo()` → `code()`; os códigos do fio ficam)
com os variantes pelos códigos da seção 13: `DaemonUnavailable`, `DaemonUntrusted`,
`ConnectionLost`, `InvalidRequest`, `Locked`, `Clipboard`, `EntryNotFound`,
`FieldNotFound`, `Unauthorized`, `Refused`, `InvalidResponse`, `RandomUnavailable`,
`NoGeneratedPassword`, `WrongPin`, `PickerUnavailable`, `TerminalUnavailable`,
`NotAServer`, `VaultAlreadyExists`, `VaultNotWritten`, `WrongPassword`, `VaultRollback`,
`UsbDrivesUnavailable`, `UsbDriveNotConnected`, `BackupNotFound`, `BackupUnavailable`,
`VaultChanged`. Os homônimos `cliente::CodigoTotp`, `Comando::CodigoTotp` e
`Resposta::CodigoTotp` são outros e não mudaram.
Fase 31 (`app/core`: `client`, `daemon`, `fake_daemon`): módulos `cliente` → `client`,
`daemon_falso` → `fake_daemon` (`daemon` fica). `Daemon::trocar` → `Daemon::exchange` (troca
de pedido e resposta: "exchange", não "change"); `EstadoCofre` → `VaultStatus` (`SemCofre`/
`Travado`/`Destravado` → `NoVault`/`Locked`/`Unlocked`; `codigo()` → `code()`, os códigos
`sem-cofre`/`travado`/`destravado` ficam até a Fase 54); `Aviso` → `Notice` (`Aprovacao`/
`Destrave` → `Approval`/`Unlock`; `destrave_de_teste`/`destrave_hostil_de_teste` →
`test_unlock`/`hostile_test_unlock`); `FilaAvisos` → `NoticeQueue` (`tomar`/`esperar`/
`esperar_destrave`/`esperar_aprovacao`/`simular`/`tamanho`/`descartados` → `take`/`wait`/
`wait_unlock`/`wait_approval`/`simulate`/`size`/`dropped`); `PRAZO_RESPOSTA` →
`RESPONSE_TIMEOUT`, `MAX_AVISOS` → `MAX_NOTICES`; `DaemonSocket` fica (`padrao` →
`standard`, `conectar_agora` → `connect_now`, `vigiar_fechamento` → `watch_close`, `avisos`
→ `notices`, `sistema_em`/`teste_em`/`de_par` → `system_at`/`test_at`/`from_pair`,
`caminho` → `path`); privados `Fila` → `Queue`, `Pedinte` → `Requester`, `Transporte` →
`Transport`, `ler` → `read_loop`, `PRAZO_ESCRITA` → `WRITE_TIMEOUT`. Funções do `client`
pelas operações: `estado` → `status`, `criar_cofre` → `create_vault`, `papel` → `role`,
`tem_papel_app` → `has_app_role`, `abrir_app`/`abrir_app_como_agente` →
`open_app`/`open_app_as_agent`, `lista` → `list`, `detalhe` → `detail`, `copiar` → `copy`,
`revelar` → `reveal`, `emitir_token` → `issue_token`, `salvar` → `save`, `mudar_nivel` →
`set_level`, `responder_aprovacao` → `answer_approval`, `destravar` → `unlock`,
`configuracao` → `settings`, `trocar_pin`/`trocar_senha` → `change_pin`/`change_password`,
`tempo_trava` → `set_lock_timeout`, `travar` → `lock`, `backup_externo` →
`external_backup`, `pendrives` → `usb_drives`, `definir_backup_externo` →
`set_external_backup`, `restaurar_backup` → `restore_backup` (`totp`, `backups` ficam);
`cliente::CodigoTotp { codigo, restante }` → `client::TotpCode { code, remaining }` (sem
`serde`), `Salvamento { Salvo, Problemas }` → `Save { Saved, Problems }`, `Respondida { Ok,
PinIncorreto, Expirado }` → `Answered { Ok, WrongPin, Expired }`. Daemon falso: `DaemonFalso`
→ `FakeDaemon` (`cheio`/`vazio`/`travado`/`sem_cofre`/`backup_plantado`/
`backup_local_inseguro`/`backup_externo_ligado` → `full`/`empty`/`locked`/`no_vault`/
`planted_backup`/`insecure_local_backup`/`external_backup_enabled`; `com_pendrives`/
`com_backups`/`com_aprovacao`/`sem_espera` → `with_usb_drives`/`with_backups`/
`with_approval`/`without_wait`; `nomes_dos_backups`/`quantas_entradas`/`erros_de_pin`/
`aprovacoes_pendentes`/`nivel_de`/`pin_confere` → `backup_names`/`entry_count`/
`pin_errors`/`pending_approvals`/`level_of`/`pin_matches`), `BackupFalso` → `FakeBackup`,
`EntradasDoBackup { Teste, Guardadas }` → `BackupEntries { Test, Stored }`,
`AprovacaoSimulada` → `SimulatedApproval`, `aprovacoes_simuladas` → `simulated_approvals`,
`backups_de_teste`/`pendrives_de_teste` → `test_backups`/`test_usb_drives`,
`MAX_ERROS_PIN` → `MAX_PIN_ERRORS`, `VERSOES_FALSAS` → `FAKE_VERSIONS`; o re-export
`daemon_falso::SEGREDOS` (achado da Fase 14) vira `fake_daemon::SECRETS`. Os modos da ponte
(`aprovacao`, `aprovacao-prod`…, `--daemon-falso`) ficam (fio da ponte; Fase 56).
Fase 32 (`app/core`: `form`, `approval`, `attachments`, `restore`, `tray`): módulos
`formulario` → `form`, `aprovacao` → `approval`, `anexos` → `attachments`, `restaurar` →
`restore`, `bandeja` → `tray`. Formulário: `TextoSecreto` → `SecretText` (`Debug`
`"SecretText(••••)"`), `ValorSecreto { Vazio, Manter, Gerada, Carregada, Novo }` →
`SecretValue { Empty, Keep, Generated, Loaded, New }`, `ValorExtra { Novo, Manter }` (o do
`app/core`) → `ExtraValue { New, Keep }`, `ExtraFormulario { nome, segredo, valor }` →
`FormExtra { name, secret, value }`, `Formulario { tipo, nome, grupo, ambiente, projeto,
campos, segredos, anexos, anexos_mantidos }` → `Form { kind, name, group, environment,
project, fields, secrets, attachments, kept_attachments }`, `ErroCampo { campo, posicao,
motivo }` → `FieldError { field, position, reason }`, todos com `#[serde(rename =
"<antigo>")]` até a Fase 54; `Fontes` → `Sources` (`guardado`/`gerada`/`carregada` →
`stored`/`generated`/`loaded`), `Falta { App, Campos }` → `Missing { App, Fields }`,
`Montado { dados }` → `Built { data }`, `SegredoMontado` → `BuiltSecret`;
`TOTP_NAO_SUPORTADO`/`TOTP_URI_INVALIDA` → `TOTP_UNSUPPORTED`/`TOTP_INVALID_URI`,
`tipo_do_formulario` → `form_kind`, `campos_do_tipo` → `fields_of_kind`,
`campo_do_problema` → `problem_field`, `erros_do_daemon` → `daemon_errors`,
`normalizar_semente` → `normalize_seed`, `montar` → `build`, `copiar_rascunho` →
`copy_draft`, `validar_local` → `validate_local`. Aprovação: `PRAZO_APROVACAO` →
`APPROVAL_TIMEOUT`, `SEGUNDOS_URGENTE` → `URGENT_SECONDS` (o mesmo nome do `approval.js`),
`MAX_PENDENTES` → `MAX_PENDING`, `SEPARADOR_CADEIA` → `CHAIN_SEPARATOR`,
`decisao_de_nome` → `decision_from_name`, `EntradaExibida { nome, tipo }` →
`DisplayedEntry { name, kind }`, `PedidoExibido` → `DisplayedRequest` (campos pela seção
13: `entries`, `environment`, `operation`, `command`, `directory`, `chain`, `origin`,
`confirmation`, `hidden_entry`, `remaining`, `status`), `ResultadoAprovacao { resultado,
ha_outro }` → `ApprovalResult { result, has_other }` (com `rename`), `Atual` → `Current`,
`Aprovacoes` → `Approvals` (`chegou`/`chegou_ha`/`atual`/`guardar_tipos`/`confirmacao`/
`entrada_oculta`/`situacao`/`remover`/`marcar_bloqueado`/`pendentes`/`ha_pedido`/
`esvaziar` → `arrived`/`arrived_ago`/`current`/`store_kinds`/`confirmation`/
`hidden_entry`/`state_of`/`remove`/`mark_blocked`/`pending`/`has_request`/`clear`),
`exibir` → `display`, `vigiar` → `watch` (o `partida.vigiar` da Fase 34 é outro). Anexos:
`Seletor` → `Picker` (`escolher`/`escolher_chave` → `choose`/`choose_key`), `SemSeletor` →
`NoPicker`, `Pendente { nome, conteudo }` → `Pending { name, content }`, `Escolha {
Anexado { nome, tamanho }, Cancelado, Aviso { motivo } }` → `Choice { Attached { name,
size }, Canceled, Notice { reason } }` (tag `resultado` e valores com `rename`),
`EdicaoDosAnexos { entrada, mantidos }` → `AttachmentEdit { entry, kept }` (com `rename`),
`Base { soma, quantidade }` → `Base { total, count }`, `Anexos` → `Attachments`
(`anexar`/`anexar_sobre`/`escolhidos`/`remover`/`limpar`/`soma` → `attach`/`attach_over`/
`chosen`/`remove`/`clear`/`total`), `AVISO_*` → `*_NOTICE`, `nome_do_anexo` →
`attachment_name`. Restaurar: `VersaoVista` → `VersionView` (`de_nome` → `from_name`),
`Restauracao { versao, senha, pin }` → `Restore { version, password, pin }`, `Restaurado {
restaurado, entradas }` → `Restored { restored, entries }` (todos com `rename`),
`versoes_de` → `versions_from`, `versao_pedida` → `requested_version`. Bandeja:
`EstadoBandeja { Destravado, Travado, SemCofre, SemDaemon }` → `TrayState { Unlocked,
Locked, NoVault, NoDaemon }` (`de` → `from_status`), `Icone` → `Icon`, `Aparencia` →
`Appearance`, `Acao { Abrir, Travar, Destravar, Sair }` → `Action { Open, Lock, Unlock,
Quit }`, `ID_TITULO`/`ID_SUBTITULO`/`ID_ABRIR`/`ID_ACAO`/`ID_SAIR` → `ID_TITLE`/
`ID_SUBTITLE`/`ID_OPEN`/`ID_ACTION`/`ID_QUIT` (os valores `bandeja-*` ficam até a Fase 54),
`TEXTO_ABRIR`/`TEXTO_SAIR` → `OPEN_TEXT`/`QUIT_TEXT`, `BIBLIOTECAS` → `LIBRARIES`,
`PASTAS_DO_SISTEMA` → `SYSTEM_DIRS`, `PASTA_DOS_ICONES` → `ICONS_DIR` (valor
`ward-bandeja` fica), `ler_configuracao_na_passagem` → `read_settings_on_transition`,
`descartar_na_passagem` → `discard_on_transition`, `subtitulo_destravado` →
`unlocked_subtitle`, `pasta_dos_icones(_da_sessao)` → `(session_)icons_dir`,
`biblioteca_disponivel`/`biblioteca_do_sistema` → `library_available`/`system_library`.
O auxiliar de teste `trocar` do `credenciais.rs` → `exchange` (como o `Daemon::exchange`).
Fase 28 (clientes `ward` e `ward-nm`; nomes próprios deles, os de `ward-core`/
`ward-platform` ficam para as fases desses crates): módulos `cliente`/`comandos`/
`texto`/`testes` → `client`/`commands`/`text`/`tests`; `principal` → `main`;
`Comando` → `Command` (`Lista`/`Travar`/`Destravar`/`Ajuda`/`Versao` → `List`/`Lock`/
`Unlock`/`Help`/`Version`; campos `entrada`/`comando` → `entry`/`command`); `ErroUso` →
`UsageError` (`SemComando`/`ComandoDesconhecido`/`ArgumentoInesperado`/`NaoUtf8`/
`UsoRun`/`UsoSsh`/`TokenEmArgv`/`UsoPlaywright`/`UsoLogin` → `NoCommand`/
`UnknownCommand`/`UnexpectedArgument`/`NotUtf8`/`RunUsage`/`SshUsage`/`TokenInArgv`/
`PlaywrightUsage`/`LoginUsage`; `mensagem()` → `message()`); `AJUDA` → `HELP`;
`interpretar` → `parse`; `ErroConexao` → `ConnectionError` (`Ausente`/`NaoConfiavel`/
`SemResposta`/`Fechada`/`RespostaInvalida`/`PedidoInvalido` → `Missing`/`Untrusted`/
`NoResponse`/`Closed`/`InvalidResponse`/`InvalidRequest`); `Conexao` → `Connection`
(`conectar`/`reconferir`/`pedir`/`pedir_com_prazo`/`abrir_sessao` → `connect`/`reverify`/
`request`/`request_with_timeout`/`open_session`); `Sessao`/`EscritaSessao` → `Session`/
`SessionWriter` (`receber`/`entrada`/`janela` → `receive`/`send_stdin`/`send_window`);
`servidor_esperado` → `expected_server`; `Entorno` → `Env` (`endurecer`/`ler_senha`/
`saida`/`erro`/`ambiente`/`diretorio`/`entrada_padrao`/`tipo_entrada`/
`tamanho_terminal`/`modo_cru`/`ler_token` → `harden`/`read_password`/`stdout`/`stderr`/
`environment`/`current_dir`/`stdin`/`stdin_kind`/`terminal_size`/`raw_mode`/
`read_token`); `Sistema` → `System`; `executar` (despacho dos comandos) → `execute`
(não `run`, que é o `ward run`); `SAIDA_OK`/`SAIDA_ERRO`/`SAIDA_USO`/`SAIDA_RUN_ERRO` →
`EXIT_OK`/`EXIT_ERROR`/`EXIT_USAGE`/`EXIT_RUN_ERROR`; `MSG_RUN_GRANDE`/`MSG_RUN_CAMPO`/
`MSG_RUN_TRAVADO`/`MSG_TOKEN_FORMATO` → `MSG_RUN_TOO_LARGE`/`MSG_RUN_FIELD`/
`MSG_RUN_LOCKED`/`MSG_TOKEN_FORMAT`; `mensagem_do_daemon`/`mensagem_do_login` →
`daemon_message`/`login_message`; `pedido_run`/`pedido_ssh` → `run_request`/
`ssh_request`; `classificar_entrada`/`tipo_da_entrada_padrao` → `classify_stdin`/
`detect_stdin_kind`; `lista_json`/`lista_tabela`/`neutralizar`/`invisivel`/`json_texto` →
`list_json`/`list_table`/`neutralize`/`invisible`/`json_text`. `ward-nm`: `ID_EXTENSAO` →
`EXTENSION_ID` (com a leitura em `dev/extension/build.py`, `dev/tests/extension.py`,
`dev/tests/login.py`, `dev/tests/chrome_policy.py`, `install/pack-extension.sh` e a
mensagem que o `dev/tests/deb.sh` procura); `MAX_MENSAGEM_ENTRADA`/`SAIDA` →
`MAX_MESSAGE_IN`/`OUT`; `OPERACOES` → `OPERATIONS`; `resposta_cabe` → `response_fits`;
`origem_da_extensao_valida` → `valid_extension_origin`; `ErroMensagem` → `MessageError`
(`Fim`/`Cortada`/`Vazia`/`GrandeDemais` → `End`/`Truncated`/`Empty`/`TooLarge`);
`ler_mensagem`/`escrever_mensagem` → `read_message`/`write_message`; `FalhaDaemon` →
`DaemonFailure` (`Indisponivel`/`Conversa` → `Unavailable`/`Conversation`; `texto()` →
`text()`); `Daemon::trocar` → `exchange`; `atender`/`servir` → `handle`/`serve`.

Fase 22 (`wardd`: `playwright`, `cdp`, `login`, `bridge`; nomes próprios desses arquivos,
os de `ward-core`/`ward-platform` e de outros arquivos do `wardd` ficam para as fases
deles): módulo `ponte` → `bridge`; `cdp`: `MAX_MENSAGEM` → `MAX_MESSAGE`, `ErroCdp` →
`CdpError` (`Prazo`/`Fechado`/`Protocolo`/`Comando`/`Io` → `Deadline`/`Closed`/
`Protocol`/`Command`/`Io`), `Escritor` → `Writer` (`novo`/`abre_objeto`/`fecha_objeto`/
`abre_lista`/`fecha_lista`/`chave`/`texto`/`inteiro`/`logico`/`nulo`/`fim` → `new`/
`open_object`/`close_object`/`open_list`/`close_list`/`key`/`text`/`integer`/`boolean`/
`null`/`finish`), `pior_caso` → `worst_case`, `Regra { origem, quadro }` → `Rule {
origin, frame }`, `Cdp { sessao, regra, bloqueou, cargas }` → `Cdp { session, rule,
blocked, loads }` (`novo`/`enviar`/`comando`/`comando_de`/`esperar_carga` → `new`/`send`/
`command`/`command_as`/`wait_load`); `playwright`: `PASTA_SESSOES`/`PREFIXO_PERFIL`/
`PRAZO_SESSAO` → `SESSIONS_DIR`/`PROFILE_PREFIX`/`SESSION_DEADLINE`, `PREENCHER`/
`LER_PAGINA` → `FILL_SCRIPT`/`READ_PAGE_SCRIPT` (o JavaScript dentro é literal e fica),
`pasta_sessoes`/`apagar_sessoes`/`limpar`/`argv_chrome`/`tem_segredo` → `sessions_dir`/
`delete_sessions`/`clean_up`/`chrome_argv`/`has_secret`, `entrar` → `log_in`; `login`:
`PrazosLogin { aguardar, retirada, total }` → `LoginDeadlines { wait, pickup, total }`
(`wait`, não `await`, que é palavra reservada do Rust), `Logins::novo` → `new`, o pedido em
curso `Trabalho` → `Job` (não `Work`: é um pedido de login pendente) com `Fase { Esperando,
Entregue, Preenchido }` → `Phase { Waiting, Delivered, Filled }`, `Daemon::aguardar_login`/
`preencher_login`/`resultado_login` → `await_login`/`fill_login`/`login_result` (as
operações do fio ficam até a Fase 50); `bridge`: `Ponte` → `Bridge` (`abrir`/`porta`/
`iniciar` → `open`/`port`/`start`), `PonteRodando` → `RunningBridge`, `Abertura` →
`Opening` (`alvo_host`/`alvo_porta`/`programas`/`mascara`/`limites`/`travas` →
`target_host`/`target_port`/`programs`/`mask`/`limits`/`locks`), `Intermediario { host,
porta, usuario, cred }` → `Intermediate { host, port, username, cred }`, `Limites {
simultaneas, total }` → `Limits { concurrent, total }`, `LIMITES_SALTO`/`LIMITES_TUNEL` →
`JUMP_LIMITS`/`TUNNEL_LIMITS`, `SaidaDeErro` → `Stderr`, `par_do_grupo`/`socket_do_par`/
`processo_do_grupo_com_socket` → `peer_in_group`/`peer_socket`/`group_process_has_socket`,
`pai_do_stat`/`grupo_do_stat`/`sessao_do_stat` → `stat_parent`/`stat_group`/
`stat_session`. Os nomes das threads (`wardd-ponte*`) são literais e ficam.

Fase 23 (`wardd`: `server`, `external`, `udisks` e os testes `common`, `units`, `memory`,
`socket`, `dates`; nomes próprios desses arquivos): módulos `servidor` → `server`,
`externo` → `external`, `tests/comum` → `tests/common`, `tests/memoria` → `tests/memory`,
`tests/datas` → `tests/dates` (`mod testes` → `tests`). `server`: `MAX_CONEXOES`/
`MAX_CONEXOES_APP` → `MAX_CONNECTIONS`/`MAX_APP_CONNECTIONS`, `PRAZO_LEITURA`/
`PRAZO_ESCRITA` → `READ_DEADLINE`/`WRITE_DEADLINE`; `Vaga { ociosa, desde }` → `Slot {
idle, since }`, `Vagas { lista, proximo }` → `Slots { list, next_id }` (`reservar`/
`marcar`/`soltar` → `reserve`/`mark`/`release`), `Soltar` → `ReleaseOnDrop`; `Papeis {
instalacao, app, app_de_teste, extensao_de_teste }` → `Roles { install, app, test_app,
test_extension }` (`papel` → `role`); `papel_da_classe` → `role_from_class`; `responder`
→ `answer`; `AssinanteSocket { escrita }` → `SocketSubscriber { write_lock }`;
`ClienteSocket { par }` → `SocketClient { peer }`; `atender_conexao` → `serve_connection`;
`servir` → `serve`. `external`: `PASTA` → `BACKUP_DIR`, `PREFIXO_TEMPORARIO` →
`TEMP_PREFIX`; `Visto { rotulo, montagem, dispositivo }` → `Seen { label, mount, device }`;
`FontePendrives::pendrives` → `UsbDriveSource::usb_drives`; `ErroExterno` →
`ExternalError` com `SemUdisks`/`NaoConectado`/`Ambiguo`/`NaoMontado`/`Trocado`/
`PastaInsegura`/`SemBackupLocal` → `NoUdisks`/`NotConnected`/`Ambiguous`/`NotMounted`/
`Swapped`/`UnsafeDir`/`NoLocalBackup` (`codigo` → `code`); `Copia { copiadas, apagadas }`
→ `BackupCopy { copied, deleted }`; `escolher`/`copiar`/`rodar` → `choose`/`copy`/
`rotate`; privados `pelo_fd` → `via_fd`, `abrir_montagem`/`abrir_destino` → `open_mount`/
`open_destination`, `versoes_em`/`geracoes_em` → `versions_in`/`generations_in`,
`ler_versao`/`gravar_versao` → `read_version`/`write_version`, `limpar_temporarios` →
`clean_temp_files`, `teto` → `cap`. `udisks`: `CAMINHO_UDISKS` → `UDISKS_PATH`,
`MAX_OBJETOS`/`MAX_MONTAGEM` → `MAX_OBJECTS`/`MAX_MOUNT`, `ESPERA_RAJADA` → `BURST_WAIT`,
`INTERVALO_COPIA` → `COPY_INTERVAL`, `FILA` → `QUEUE`, `Objetos` → `Objects`;
`montagem_valida`/`rotulo_limpo`/`ler_objetos`/`consultar`/`registrar` → `valid_mount`/
`clean_label`/`read_objects`/`query`/`record`; `Udisks::novo` → `new`; `vigiar_copia`/
`vigiar_udisks` → `watch_copy`/`watch_udisks` (`cutucoes` → `pokes`). Testes (`common`):
`SENHA`/`PIN_ERRADO`/`SEGREDOS` → `PASSWORD`/`WRONG_PIN`/`SECRETS`, `RelogioFalso` →
`FakeClock`, `Dir::novo`/`caminhos` → `new`/`paths`, `daemon_em` → `daemon_at`, `ped`/
`pedir` → `req`/`request`, `erro` → `error`, `destravar` → `unlock`, `servidor`/`banco`
→ `server`/`database`, `mudar_nivel` → `set_level`, `cofre_de_teste` → `test_vault`,
`codificado` → `encoded`, `contem` → `contains`, `geracoes`/`geracao_no_disco` →
`generations`/`generation_on_disk`, `com_janela` → `with_window` (`JanelaFalsa` →
`FakeWindow`); `socket.rs`: `DirSocket` → `SocketDir`, `subir`/`conectar`/`mandar`/
`receber`/`pedir_por` → `start`/`connect`/`send`/`receive`/`request_on`; `dates.rs`:
`RelogioDeParede` → `WallClock`. Os nomes das threads (`wardd-conexao`, `wardd-externo`,
`wardd-udisks`) e os textos de log (`UidRecusado`, `SemIdentidade`) são literais e ficam.

Fase 21 (`wardd`: `execution`, `ssh_agent`, `ssh`, `ssh_tests`): módulos `execucao`/
`agente_ssh`/`ssh_testes` → `execution`/`ssh_agent`/`ssh_tests` (`mod testes` → `tests`).
`execution`: `Temporarios { pasta, arquivos }` → `TempFiles { dir, files }` (`criar`/
`gravar` → `create`/`write`), `Arquivo` (privado) → `SessionFile`, `pasta_temporarios`
→ `temp_dir`, `PASTA_TEMPORARIOS` → `TEMP_DIR`, `zerar` → `zero`, `limpar_temporarios`
→ `clean_temp_files` (o mesmo inglês do `atomic_write`; o homônimo do `wardd/externo.rs`
é outro e fica para a fase dele); `Preparo` → `Preparation` (campos `comando`/`mascara`/
`temporarios`/`ponte`/`agente`/`trava`/`entrada_nula` → `command`/`mask`/`temp_files`/
`bridge`/`agent`/`lock`/`null_stdin`); `MorreNaTrava { sessoes, travas, inicial }` →
`DiesOnLock { sessions, locks, initial }` (`travou` → `locked`); `AgentePreparado` →
`PreparedAgent` (`filho_do_filho` → `child_of_child`); `RecursosRun` → `RunResources`,
`ContextoTunel` → `TunnelContext`; `Forma { Locale, Terminal, Fuso, Sinal }` → `Shape {
Locale, Terminal, Timezone, Flag }` ("sinal" aqui é a variável-bandeira `NO_COLOR`, não
sinal de processo); `ENTRAM_DA_CLI` → `FROM_CLI`, `PATH_DO_COMANDO` → `COMMAND_PATH`,
`DO_DAEMON` → `FROM_DAEMON`, `MAX_VALOR_DA_LISTA` → `MAX_LIST_VALUE`, `LOTE` → `BATCH`,
`FILA_ENTRADA` → `STDIN_QUEUE`, `PRAZO_RESTO_DA_SAIDA` → `REST_OF_OUTPUT_DEADLINE`,
`PASSO_LEITURA_MS` → `READ_STEP_MS`; `Conta { usuario, home }` → `Account { user, home }`
(`do_passwd` → `from_passwd`); `ajustar_locale` → `adjust_locale`, `entra_da_cli` →
`enters_from_cli`, `valor_aceito` → `value_accepted`, `montar_ambiente` →
`build_environment`, `ler_lote` → `read_batch`, `ler_saida` → `read_output`,
`escrever_entrada` → `write_stdin`, `ler_da_cli` → `read_from_cli`, `por_na_fila` →
`enqueue`, `aviso_do_fim` → `end_notice`, `Lancado` → `Launched`, `sessao`/`sessao_com`
→ `session`/`session_with`, `Daemon::preparar_execucao` → `prepare_run`; `Cli::enviar`/
`enviar_saida` → `send`/`send_output`. `ssh_agent`: `AgenteSsh { pasta, escuta, chave }`
→ `SshAgent { path, listener, key }` (`criar`/`publica`/`iniciar` → `create`/
`public_key`/`start`), `Rodando` → `Running`, `Permitido { pid, filho_do_filho }` →
`Allowed { pid, child_of_child }`, `ErroAgente` → `AgentError` com `ChaveInvalida` →
`InvalidKey`, `AlgoritmoNaoSuportado` → `UnsupportedAlgorithm`, `Sistema` → `System`
(`Passphrase` fica); `Chave` (privado) → `Key` (`ler`/`publica_em_texto`/`assinar` →
`read`/`public_as_text`/`sign`); `NOME_SOCKET`/`NOME_PUBLICA`/`PREFIXO_PASTA` →
`SOCKET_NAME`/`PUBLIC_NAME`/`DIR_PREFIX`; `limpar_pastas` → `clean_dirs`, `laco` →
`serve_loop`, `atender` → `serve`, `par_permitido(_pid)` → `peer_allowed(_pid)` (`preso`
→ `held`), `responder_mensagem` → `answer_message`. "Assinar" aqui é assinatura
criptográfica → `sign`/`signature` (não `subscribe`, que é o sentido do D-Bus).
`ssh`: `ProgramasSsh` → `SshPrograms` (`do_sistema` → `from_system`), `TokenConectar
{ valor, entrada, ate }` → `ConnectToken { value, entry, until }`, `PRAZO_TOKEN` →
`TOKEN_DEADLINE`, `PATH_FILHO` → `CHILD_PATH`; `Destino { host, porta, usuario }` →
`Destination { host, port, username }`; `Autent { Senha, Chave { agente, publica } }` →
`SshAuth { Password, Key { agent, public_key } }`; `Programa { Ssh { comando }, Sftp,
Encaminhar { host, porta } }` → `Program { Ssh { command }, Sftp, Forward { host, port }
}`; `Cred { Senha, Chave { chave, passphrase } }` → `Cred { Password, Key { key,
passphrase } }`; `montar_argv(_com_alias)` → `build_argv(_with_alias)`,
`alias_do_destino` → `destination_alias`, `usuario_ok` → `username_ok`, `host_simples`
→ `simple_host`, `cred_de` → `cred_from`, `intermediario` → `intermediate`,
`conferir_tipo` → `verify_kind`, `Daemon::emitir_token`/`usar_token`/`preparar_ssh` →
`issue_token`/`use_token`/`prepare_ssh`. Os nomes que vêm de `nucleo`, `aprovacao`,
`ponte`, `log` (Fases 19, 20 e 22) e do `ward-core` (`entrega`, `mascara`, `protocolo`,
Fases 15–17) ficam como estão até as fases deles.

Fase 15 (`ward-core`: `backup`, `delivery`): módulo `entrega` → `delivery` (`testes` →
`tests`, também em `backup`). Backup: `VERSOES` → `VERSIONS`, `MAX_SUFIXO` → `MAX_SUFFIX`,
`MAX_NOME` → `MAX_NAME`, `MARGEM_FUTURO` → `FUTURE_MARGIN`, `BASE_TEMPORARIO` → `TEMP_BASE`;
`Versao { data, hora, sufixo }` → `Version { date, time, suffix }` (`do_instante`/`de_nome`/
`nome`/`fim_do_segundo`/`com_sufixo`/`possivel` → `from_instant`/`from_name`/`name`/
`end_of_second`/`with_suffix`/`is_possible`); `ErroBackup` → `BackupError` com
`PastaInsegura` → `UnsafeDir`, `NaoAutentico` → `NotAuthentic`, `Relogio` → `Clock`,
`NomesEsgotados` → `NamesExhausted`, `Gravacao` → `Write` (`Io` fica); `LerVersao` →
`ReadVersion`; `PastaBackup` → `BackupDir` (`nova`/`com_leitor`/`caminho`/`listar`/
`listar_ate`/`guardar`/`guardar_com_pontos`/`rodar`/`conferir` → `new`/`with_reader`/`path`/
`list`/`list_until`/`keep`/`keep_with_points`/`rotate`/`check`); `para_apagar_conferidas` →
`to_delete_verified`, `fora_de_ordem` → `out_of_order` (só a função: o campo
`fora_de_ordem` do fio fica até a Fase 50), `geracao_selada` → `sealed_generation`,
`guardar_atual` → `keep_current`, `nome_valido` → `valid_name`; privados `teto_de` →
`ceiling_of`, `ler_versao_padrao` → `read_version_default`, `civil_de_dias` →
`civil_from_days`, `digitos` → `digits`. Entrega: `ErroRecurso` → `ResourceError`;
`Recursos` → `Resources` (`codigo_totp`/`arquivo`/`tunel` → `totp_code`/`file`/`tunnel`);
`ErroEntrega` → `DeliveryError` com `TipoSemEntrega` → `TypeWithoutDelivery`, `Recurso` →
`Resource`, `RecursoInvalido` → `InvalidResource`, `Invalida` → `Invalid`; `Variavel` (da
entrega) → `Variable` (`nome`/`expor`/`e_segredo` → `name`/`expose`/`is_secret`); `Entrega`
→ `Delivery` (`variaveis`/`valor`/`nomes` → `variables`/`value`/`names`); `montar` →
`build`; privados `Montagem` → `Builder` (`texto`/`segredo`/`arquivo`/`conferir` →
`text`/`secret`/`file`/`check`), `prefixado` → `prefixed`, `autenticacao` → `auth`, `banco`
→ `database`, `url_do_banco` → `database_url`, `juntar` → `join_parts`, `livre` →
`unreserved`, `codificar_em` → `encode_into`, `texto_do_segredo` → `secret_text`,
`nome_de_variavel_valido` → `valid_variable_name`. Os nomes das variáveis entregues
(`WARD_ENTRADA`, `WARD_ARQUIVO_*`…) ficam até a Fase 53. Os homônimos
`ErroResposta::TipoSemEntrega`/`EntregaFalhou` e `ErroEnvelope::Versao` (Fase 17),
`protocolo::Variavel` (Fase 17), `VersaoBackup` (Fase 18), `ErroMascara::SemEntrega`
(Fase 16), `RunResources` (Fase 21) e `RecursosDoOrcamento` (Fase 16) são outros e não
mudaram.
Fase 24 (testes do `wardd`: `tests/ssh.rs`, `tests/run.rs`, `tests/ssh_falso.py` →
`tests/fake_ssh.py`): em `ssh.rs`, `SENHA_SSH`/`SENHA_BANCO` → `SSH_PASSWORD`/
`DB_PASSWORD`, `RelogioManual` → `ManualClock` (`avancar` → `advance`), `Rt::novo` →
`Rt::new`, `Cena { relogio }` → `Scene { clock }` (`cena` → `scene`; `modo`/`registros`/
`registro`/`nada_executado` → `set_mode`/`records`/`record`/`nothing_ran`), `Reg` → `Rec`
(`todos`/`um` → `all`/`one`), `Sessao { saida, erro, pedacos, fim, falha }` → `Session {
stdout, stderr, chunks, end, failure }` (`texto` → `text`), `salvar` → `save`,
`servidor_json` → `server_json`, `auth_senha` → `password_auth`, `gerar_chave` →
`generate_key`, `pedido_ssh` → `ssh_request`, `mandar` → `send`, `sessao`/`sessao_de` →
`session`/`session_of`, `fechar_entrada` → `close_stdin`, `opcoes` → `options`,
`esperar` → `wait_for`, `rt_sem_agente` → `rt_without_agent`, `esperar_registro` →
`wait_record`, `depois_de` → `after`, `estranho_e_recusado` → `stranger_is_refused`
("estranho" aqui é o processo de fora da sessão, `stranger`, não `odd`),
`inode_da_escuta`/`escuta_fechada` → `listener_inode`/`listener_closed`, `ninguem_com_w`
→ `nobody_with_w`, `conferir_argv_da_ponte` → `verify_bridge_argv`. Em `run.rs`,
`SENHA_LIVRE`/`LICENCA`/`ANEXO`/`APROVADOR` → `FREE_PASSWORD`/`LICENSE`/`ATTACHMENT`/
`APPROVER`, `DirSocket::novo` → `SocketDir::new`, `Wardd { filho }` → `Wardd { child }`,
`subir`/`subir_com` → `start`/`start_with`, `receber` → `receive`, `conectar` →
`connect`, `destravar_por` → `unlock_through`, `pedido_executar(_com)` →
`run_request(_with)`, `entrada_padrao` → `send_stdin`, `ler_sessao` → `read_session`,
`cofre` → `vault`, `tmp_vazio` → `tmp_empty`, `vivo` → `alive`. Em `fake_ssh.py`, `NOME`
→ `NAME`, `ler` → `read_file`, `registrar` → `record`, `opcao` → `option`, `pedir` →
`ask`, `cliente_do_agente` → `agent_client`, `NETO` → `GRANDCHILD`, `esperar_continuar`/
`esperar_arquivo` → `wait_continue`/`wait_file`, `valor` → `value`,
`ponte_do_lado_do_salto` → `bridge_on_the_jump_side`, `conversar` → `talk`, `principal`
→ `main`. Os arquivos que o falso grava e lê (`registro-<nome>-<pid>.txt`, `modo`,
`esperar`, `continuar`, `pty-pronto`…) e as chaves do registro (`agente.lista`, `neto`,
`depois`…) são literais e ficam. Os nomes do `comum` (Fase 23), do `nucleo`/`aprovacao`
(Fase 20) e do `ward-core` (`protocolo`, Fases 17–18) ficam como estão até as fases deles.
Fase 27 (testes do `wardd`: `playwright.rs`, `login.rs` ficam; `extensao.rs` →
`extension.rs`, `app_teste.rs` → `test_app.rs`, `app_valor.rs` → `app_value_binary.rs`,
`valor_app.rs` → `app_value_core.rs`, `anexos_edicao.rs` → `attachment_edit.rs`,
`chrome_falso.py` → `fake_chrome.py`; `[[test]]` `app_teste`/`app_valor` →
`test_app`/`app_value_binary`): em `playwright.rs`, `SENHA_LOGIN` → `LOGIN_PASSWORD`,
`Rt::novo` → `Rt::new`, `Cena`/`cena` → `Scene`/`scene` (`registro`/`eventos`/`sessao`/
`sem_perfil` → `records`/`events`/`session`/`no_profile`), `formas_da_senha` →
`password_forms`, `site::{Pedido { metodo, caminho, corpo }, porta, pedidos, Rota, subir,
atender}` → `site::{Request { method, path, body }, port, requests, Route, start,
serve}`, `formulario` → `form`, `com_clobber` → `with_clobber`, `login_em` → `login_at`,
`CenaReal`/`cena_real` → `RealScene`/`real_scene` (`phish_limpo`/`senha_fora_da_url` →
`phish_clean`/`password_out_of_url`), `CASOS` → `CASES`, `pagina` → `page`. Em
`extension.rs`, `SEMENTE`/`AGORA` → `SEED`/`NOW`, `RelogioFixo` → `FixedClock`, `cofre`
→ `vault`, `sugerir` → `suggest`, `preencher_cru`/`preencher` → `fill_raw`/`fill`,
`nomes` → `names`, `sem_segredo` → `no_secret`, `salvar_login` → `save_login`,
`nivel_na_lista` → `level_in_list`, `SENHA_NOVA` → `NEW_PASSWORD`, `SaiLogo` →
`LeavesSoon`. Em `login.rs`, `ORIGEM` → `ORIGIN`, `prazos` → `deadlines` (`retirada_ms` →
`pickup_ms`), `cofre` → `vault`, `login_do_agente` → `agent_login`, `aguardar` →
`await_login`, `pegar` → `take`, `preencher` → `fill`, `resultado` → `outcome`,
`sem_valor` → `no_value`, `ClienteQueSai` → `LeavingClient`, `RelogioParado` →
`StoppedClock`, `AppFalso` → `FakeApp`, `SemAbrir` → `NoOpen`, `responder` → `answer`,
`login_http`/`login_na_url`/`login_http_completo` → `http_login`/`login_at_url`/
`full_http_login`, `CaiDepois` → `DropsLater`. Em `app_value_binary.rs` e
`test_app.rs`, `APP_TESTE` → `TEST_APP`, `CLIENTE` → `CLIENT`, `DirSocket::novo` →
`SocketDir::new`, `esperar` → `wait_for`, `ler` → `read`, `agulhas` → `needles`,
`sem_segredo` → `no_secret`, `cliente_python` → `python_client`, `mandar` → `send`,
`pedir` (local) → `request`, `a_mao` → `by_hand`. Em `app_value_core.rs`, `RelogioFixo`
→ `FixedClock`, `com_totp` → `with_totp`, `alvo_campo` → `field_target`, `valor` →
`value`, `pedido_valor` → `value_request`. Em `attachment_edit.rs`, `ANEXO_A`/`ANEXO_B`
→ `ATTACHMENT_A`/`ATTACHMENT_B`, `generico` → `generic`, `editar` → `edit`, `no_disco`/
`anexos_no_disco` → `on_disk`/`attachments_on_disk`. Em `fake_chrome.py`, `PASTA`/`MODO`
→ `FOLDER`/`MODE`, `registrar` → `record` (recebe um dict; as chaves ficam), `origem` →
`origin`, `ler`/`escrever` → `read_message`/`write_message`, `responder` → `reply`,
`evento` → `event`, `pendentes`/`contador` → `pending`/`counter`, `pausar` → `pause`,
`url_atual` → `current_url`, `usuario_senha` → `credentials`, `proximo` →
`next_command`. Os arquivos `modo`/`registro`, os modos (`mente`, `morre`…) e as chaves
do registro (`evento`, `inicio`, `preencher`, `decisao`, `seguiu`, `ambiente`, `alvos`…)
são literais e ficam, como o código `"usuario, senha"` que o falso procura no script do
daemon. O código Python embutido no `CLIENT` de `app_value_binary.rs` é literal e fica.
Fase 25 (testes do `wardd`: `aprovacao.rs` → `approval.rs`, `trava.rs` → `lock.rs`,
`nucleo.rs` → `core.rs`; `[[test]]` `trava` → `lock`): em `lock.rs`, a macro `emitir` →
`emit`, `SESSAO`/`IFACE_SESSAO` → `SESSION`/`SESSION_IFACE`, `RelogioManual` →
`ManualClock` (`novo`/`esperas`/`por`/`ilegivel` → `new`/`waits`/`set`/`unreadable`),
`cofre_com_relogio` → `vault_with_clock`, `lista_ok` → `list_ok`, `executar` (local) →
`run_request`, `BarramentoFalso` → `FakeBus` (`filho`/`subir`/`endereco`/`cliente`/
`matar` → `child`/`start`/`address`/`client`/`kill`), `Inibidores` → `Inhibitors`,
`LogindFalso` → `FakeLogind` (`inibidor` → `inhibitor`), `daemon_vigiado` →
`watched_daemon`, `espera_travar` → `wait_locked`, `mandar` → `send`, `pedir_por` →
`request_over`, `Filho` → `KillOnDrop`, `esperar_no_log` → `wait_in_log`, `forjador` →
`forger`. Em `core.rs`, `lista` → `list`, `nivel_de` → `level_of`, `sou_root` →
`is_root`, `cofre_n2` → `vault_n2`, `geracoes_em` → `generations_at`. Em `approval.rs`,
`PRAZO` → `DEADLINE`, `RelogioManual` → `ManualClock` (`ir`/`avancar` → `set`/`advance`),
`Recebido { Aprovacao, Destrave }` → `Received { Approval, Unlock }`, `AppFalso` →
`FakeApp`, `AoSair` → `OnExit`, `AbridorFalso { resultado, aberturas, ao_sair }` →
`FakeOpener { result, opens, on_exit }` (`novo`/`sair` → `new`/`exit`), `Cena { relogio,
abridor }` → `Scene { clock, opener }`, `cena`/`cena_com` → `scene`/`scene_with`,
`assinar` → `subscribe`, `executar_json(_em)` → `run_json(_in)`, `usar` → `use_entry`,
`pedido` → `approval_request`, `responder` → `answer`, `nivel` → `level`,
`ExpirarSeCair` → `ExpireOnPanic`, `com_janela` (local) → `with_approval_window` (o
`with_window` do `common` é outro), `fontes_rs` → `rs_sources`, `GANCHOS_DE_TESTE`/
`CFG_GANCHOS` → `TEST_HOOKS`/`CFG_HOOKS`, `conferir_v3_15` → `check_v3_15` (aqui
`assinatura` é a assinatura da função, `signature`, não o `subscription` do D-Bus),
`destrave` → `unlock_request`, `usar_com_entrada` → `use_with_stdin`,
`com_janela_e_entrada` → `with_window_and_stdin`, `responder_ciente` →
`answer_confirmed`, `ClienteForjado` → `ForgedClient`, `Caiu` → `Dropped`. Os nomes de
arquivo de `src/` que o V3.15 procura (`trava.rs`, `nucleo.rs`, `aprovacao.rs`,
`uso.rs`…) e os ganchos (`com_chrome`, `com_programas_ssh`…) são literais e mudam com as
Fases 19–20; os métodos dos traits de fora (`Relogio::agora`/`dormir`,
`RelogioMotor::agora`, `Assinante::entregar`, `AbridorApp::abrir`, `Cliente::cadeia`/
`saiu`) ficam até as fases deles.
Fase 26 (testes do `wardd`: `tests/externo.rs` → `tests/external.rs`,
`tests/configuracoes.rs` → `tests/settings.rs`; `backup.rs` e `d62.rs` ficam): em
`external.rs`, `Fonte { vistos, proximas, consultas }` → `Source { seen, next, queries }`
(`com`/`por` → `with`/`set`), `pendrive` → `usb_drive`, `montagem` → `mount_point`,
`ligar` → `turn_on`, `locais` → `local_versions`, `arvore` → `tree`, `pasta_externa` →
`external_dir`, `no_pendrive` → `on_drive`, `BarramentoFalso { filho }` → `FakeBus {
child }` (`subir`/`endereco` → `start`/`address`), `Objeto { caminho, rotulo,
dispositivo, montagem, sistema, de_loop }` → `Object { path, label, device, mount,
system, is_loop }`, `Objetos`/`objetos`/`objeto` → `Objects`/`objects`/`object`,
`Gerente { lista }` → `Manager { list }`, `UdisksFalso { lista }` → `FakeUdisks { list }`
(`novo`/`montou` → `new`/`mounted`), `cutucoes` → `nudges`. Em `backup.rs`, `pasta` →
`backup_dir`, `modo` → `mode`, `versoes` → `versions`, `restaurar` → `restore`,
`nomes_da_lista` → `list_names`, `tempo_trava` → `lock_timeout`, `versao_em` →
`version_at`, `plantar` → `plant`. Em `settings.rs`, `configuracao` → `settings`,
`tempo_trava` → `lock_timeout`, `trocar_pin` → `change_pin`, `trocar_senha` →
`change_password`. Em `d62.rs`, `RelogioMovel { agora, esperas }` → `MovableClock { now,
waits }` (`andar` → `advance`), `Preso { entrou, soltar }` → `Stuck { entered, release
}`, `daemon_movel` → `movable_daemon`, `destravar_como` → `unlock_as`, `texto_estado` →
`state_text`, `falhas_no_disco` → `failures_on_disk`, `cofre_travado` → `locked_vault`,
`trocar_por_copia` → `swap_for_copy`. Os métodos `dormir`/`agora` do trait `Relogio`, os
nomes do `nucleo` (`com_pendrives`, `sincronizar_externo`, `ligar_copia_externa`,
`com_gancho_depois_de_gravar`, `Caminhos`, `ESPERA_MAXIMA`, `espera_destrave`), do
`arquivos` (`Geracoes`, `ler_estado_completo`, `gravar_estado`) e do `ward-core`
(`protocolo`) ficam até as fases deles (17–20).

Fase 16 (`ward-core`: `mask`): módulo `mascara` → `mask` (`automato` → `automaton`,
`testes` → `tests`). `MARCA` → `MARK`; `ErroMascara` → `MaskError` com `GrandeDemais` →
`TooLarge` e `SemEntrega` → `NoDelivery`; `ConstrutorMascara` → `MaskBuilder` (`entrada`/
`entrega`/`construir` → `entry`/`delivery`/`build`); `Mascara` → `Mask` (`da_entrada`/
`tem_segredo_curto`/`fluxo`/`mascarar_tudo` → `from_entry`/`has_short_secret`/`stream`/
`mask_all`); `Fluxo` → `Stream` (`mascarar`/`terminar` → `mask`/`finish`; campos
`pendente`/`emitido`/`lido`/`visoes`/`cobertos`/`em_marca` → `pending`/`emitted`/
`consumed`/`views`/`covered`/`in_mark`; `emitir_ate` → `emit_until`: aqui "emitir" é soltar
saída, `emit`, e não o `issue` do token); `custo_da_entrada` → `entry_cost`; `somar`
(`pub(crate)`) → `sum_patterns`. Privados: `RecursosDoOrcamento` → `BudgetResources`,
`visitar`/`Visitante` → `visit`/`Visitor`, `Gerador` → `Generator` (`segredo_de_campo`/
`valor_minimo`/`json_da_linha`/`comentario_e_aspas`/`segredo`/`formas_de_texto`/`anexo`/
`linhas`/`valor_da_linha` → `field_secret`/`min_value`/`line_json`/`comment_and_quotes`/
`secret`/`text_forms`/`attachment`/`lines`/`line_value`), `B64_PADRAO` → `B64_STD`,
`caracteres` → `char_count`, `sem_quebra_final` → `without_final_newline`,
`valor_de_chave`/`valor_de_dois_pontos`/`valor_de_chave_composta` → `key_value`/
`colon_value`/`compound_key_value`, `antes_de_comentario`/`miolo_antes_de_comentario` →
`before_comment`/`core_before_comment`, `pular_branco_json`/`fim_string_json`/
`decodificar_string_json` → `skip_json_blank`/`json_string_end`/`decode_json_string`,
`sem_aspas` → `unquoted`, `json_escapa`/`json_muda` → `json_escapes`/`json_changes`,
`JsonEscapado` → `EscapedJson` (`json_escapado`/`_u`/`_muda` → `escaped_json`/`_u`/
`_changes`), `aspas_duplas(_escapa)`/`aspas_simples` → `double_quotes(_escapes)`/
`single_quotes`, `printf_q_escapa`/`printf_q_muda` → `printf_q_escapes`/`printf_q_changes`,
`Modo { Cru, Percent, Formulario }` → `Mode { Raw, Percent, Form }`, `Escape { Nada,
Percent { ini }, Hex { ini, alto, char_alto } }` → `Escape { Nothing, Percent { start },
Hex { start, high, high_char } }`, `Visao` → `View` (`andar`/`ler`/`fechar` → `walk`/
`read`/`close`), `cobrir` → `cover`. Autômato: `Automato` → `Automaton` (campos `filho`/
`irmao`/`falha`/`saida`/`prof`/`raiz` → `child`/`sibling`/`fail`/`output`/`depth`/`root`;
`com_capacidade`/`novo_no`/`nos`/`filho_de`/`inserir`/`finalizar`/`passo`/`saida`/
`profundidade` → `with_capacity`/`new_node`/`nodes`/`child_of`/`insert`/`finalize`/`step`/
`output`/`depth`), `NENHUM`/`RAIZ` → `NO_NODE`/`ROOT`, `reservar`/`empurrar`/`indice` →
`reserve`/`push`/`index`. O `Debug` dos tipos acompanha o nome (`"Mask(••••)"`,
`"MaskBuilder(••••)"`, `"Stream(••••)"`, como o `"Secret(••••)"` da Fase 12). Os
homônimos `ErroCodificar::GrandeDemais`, `ErroQuadro::GrandeDemais` e
`ErroResposta::TipoSemEntrega` (Fase 17) são outros e não mudaram.

Fase 17 (`ward-core`: `protocol`, `protocol/{fields,errors,json,request,frame}`): módulo
`protocolo` → `protocol` (`campos`/`erros`/`pedido`/`quadro`/`resposta`/`testes` →
`fields`/`errors`/`request`/`frame`/`response`/`tests`; os dois últimos só mudam de
caminho, o conteúdo é da Fase 18). `VERSAO` → `VERSION`; `Papel { Agente, App, Extensao }`
→ `Role { Agent, App, Extension }` (`TODOS` → `ALL`); `Papeis` → `Roles` (campos `agent`/
`app`/`extension`, `de` → `of`, `tem` → `has`); as constantes da tabela pela seção 10; macro
`tabela!` → `table!` (`TODAS` → `ALL`, `nome`/`papeis`/`de_nome`/`permitida` → `name`/
`roles`/`from_name`/`allowed`); `Operacao` → `Operation` e `Pedido` → `Request` com as
variantes pela operação v2 da seção 10 (`Estado` → `Status`, `EntradaPadrao` → `Stdin`,
`Janela` → `Resize`, `Configuracao` → `Config`, `Pendrives` → `UsbDrives`, `ResultadoLogin`
→ `LoginResult`…; `operacao()`/`conferir()`/`escrever()` → `operation()`/`check()`/
`write()`); `TipoResposta` → `ResponseType` pelos tipos de resposta v2 (`Saida` →
`Output`, `Fim` → `Exit`, `Preenchimento` → `Fill`…; `carrega_credencial` →
`carries_credential`); `ErroCodificar { NaoPermitidaAoPapel, GrandeDemais, Invalida }` →
`EncodeError { NotAllowedForRole, TooLarge, Invalid }`; `RespostaInvalida` →
`InvalidResponse`; `codificar_pedido`/`decodificar_pedido`/`codificar_resposta`/
`decodificar_resposta`/`erro_do_quadro` → `encode_request`/`decode_request`/
`encode_response`/`decode_response`/`frame_error`; privados `Envelope { nome, topo }` →
`Envelope { name, top }`, `ErroEnvelope { Json, Versao }` → `EnvelopeError { Json,
Version }`, `ler_envelope` → `read_envelope`. Erros: `ErroResposta` → `ResponseError` com as
variantes pelos códigos v2 da seção 10 (`NaoAutorizado` → `Unauthorized`, `Retrocesso` →
`Rollback`, `Validacao` → `Validation`, `TipoSemEntrega` → `TypeWithoutDelivery`…),
`SIMPLES` → `SIMPLE`, `codigo`/`mensagem`/`de_codigo`/`de_validacao` → `code`/`message`/
`from_code`/`from_validation`; `Problema { campo, posicao, motivo }` → `Problem { field,
position, reason }`; `MSG_RETROCESSO`/`MSG_COFRE_MUDOU`/`MSG_GERACAO_ESGOTADA` →
`MSG_ROLLBACK`/`MSG_VAULT_CHANGED`/`MSG_GENERATION_EXHAUSTED`. Pedido: `Variavel { nome,
valor }` → `Variable { name, value }` (homônimo do `delivery::Variable`, em outro módulo);
`PedidoExecutar { entrada, argv, cwd, ambiente, tipo_entrada }` → `RunRequest { entry, argv,
cwd, env, stdin_kind }`; `TipoEntradaPadrao { Terminal, Nula, Pipe, Arquivo, Outra }` →
`StdinKind { Terminal, Null, Pipe, File, Other }` (`oculta` → `hidden`); `Janela { linhas,
colunas }` → `WindowSize { rows, cols }`; `Alvo { Campo, Extra }` → `Target { Field, Extra }`
(`segredo_em` → `secret_in`); `Decisao { Negar, PermitirUmaVez, PermitirUmaHora,
PermitirSempre, Confirmar }` → `Decision { Deny, AllowOnce, AllowOneHour, AllowAlways,
Confirm }` (`pede_pin` → `requires_pin`); `ResultadoLogin { Enviado, Recusado, SemFormulario,
AbaMudou, Falha }` → `LoginResult { Submitted, Refused, NoForm, TabChanged, Failure }`;
`id_login_ok` → `login_id_ok`; `juntar_anexos_mantidos` → `merge_kept_attachments`;
`CAMPOS_SECRETOS` → `SECRET_FIELDS`; campos das variantes do `Request` pela tabela de campos
da seção 10 (`senha` → `password`, `janela` → `window_size`, `tipo_entrada` → `stdin_kind`,
`rascunho` → `draft`, `manter_anexos` → `keep_attachments`, `entrada_confirmada` →
`confirmed_entry`, `atual`/`novo`/`nova` → `current`/`new`, `ligado` → `enabled`…);
privados `CampoRascunho { Texto, Segredo }` → `DraftField { Text, Secret }`,
`campos_do_rascunho`/`ler_rascunho`/`escrever_rascunho`/`ler_dados` → `draft_fields`/
`read_draft`/`write_draft`/`read_data`. Quadro: `BLOCO`/`CABECALHO` → `BLOCK`/`HEADER`,
`limite_do_quadro`/`LIMITE_RESPOSTA` → `frame_limit`/`RESPONSE_LIMIT`, `ErroQuadro { Fechado,
Truncado, Vazio, GrandeDemais, Io }` → `FrameError { Closed, Truncated, Empty, TooLarge, Io }`,
`ler_cabecalho`/`validar_tamanho`/`ler_quadro`/`escrever_quadro` → `read_header`/
`validate_size`/`read_frame`/`write_frame`. Campos (crate): `Campos` → `Fields` (`de`/
`sem_conferir`/`sem_repetidas`/`vazio`/`em_ordem`/`tirar`/`fim`/`exigir` → `from_value`/
`unchecked`/`no_duplicates`/`empty`/`in_order`/`take`/`finish`/`require`; `texto`/
`texto_ou_vazio`/`texto_opc`/`segredo`/`segredo_opc`/`inteiro`/`logico`/`lista`/
`lista_ou_vazia`/`objeto`/`objeto_ou_vazio` → `text`/`text_or_empty`/`text_opt`/`secret`/
`secret_opt`/`integer`/`boolean`/`list`/`list_or_empty`/`object`/`object_or_empty`),
`texto_de`/`bytes_de`/`segredo_de`/`lista_de`/`textos_de` → `text_of`/`bytes_of`/
`secret_of`/`list_of`/`texts_of`. JSON (crate): `Valor { Nulo, Logico, Inteiro, Texto, Lista,
Objeto }` → `Value { Null, Bool, Integer, Text, List, Object }` (o `Debug` acompanha o nome,
`"Text(••••)"`), `JsonInvalido` → `InvalidJson`, `Leitor` → `Reader`, `Escritor` → `Writer`
(`abre_objeto`/`fecha_objeto`/`abre_lista`/`fecha_lista`/`chave`/`texto`/`inteiro`/`logico`/
`objeto`/`lista` → `open_object`/`close_object`/`open_list`/`close_list`/`key`/`text`/
`integer`/`boolean`/`object`/`list`), `GrandeDemais` → `TooLarge`, `ler`/`codificar` →
`read`/`encode`, `de_base64`/`tamanho_base64` → `from_base64`/`base64_len`,
`MAX_PROFUNDIDADE`/`MAX_NOS`/`MAX_NOS_RESPOSTA`/`MAX_CHAVES` → `MAX_DEPTH`/`MAX_NODES`/
`MAX_RESPONSE_NODES`/`MAX_KEYS`. Os nomes de fio (operação, tipo, campo, código, papel e
valores) continuam os literais v1 até a Fase 50. O `ler_cabecalho` de `resposta.rs` e o
`Papeis` do `wardd/servidor.rs` são outros e não mudaram; o `ErroPartida::Papel` do
`app/core` (variante) também não.

Funções e métodos não estão listados um a um: o nome sai da seção 2 na ordem do inglês
(`conferir_arquivo` → `verify_file`, `ler_cofre` → `read_vault`, `codificar_resposta` →
`encode_response`, `exigir_pin` → `require_pin`). Os poucos fixados por outra seção:
`nome()`/`de_nome()` → `name()`/`from_name()`, `rotulo()` → `label()`, `expor()` →
`expose()`, `papeis()` → `roles()`, `permitida()` → `allowed()`, `carrega_credencial()` →
`carries_credential()`.
