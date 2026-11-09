# Plano — Ward

Status: aprovado

Fontes: `docs/mission/prd.md` (regras R*, decisões D*, riscos RA*) e
`docs/mission/validation-contract.md` (asserções V*). Cada fase cabe em ~100k tokens:
um assunto, poucos arquivos, verificável sozinha e com testes. **(DESIGN)** desenha no
FigCraft pelo MCP (`mcp__figcraft__*`) no arquivo
`/home/jeffersonoliveira/documentos/Ward.figcraft` (páginas Inspirações só leitura,
Design System, Layout), registra os ids em `docs/mission/design-nodes.md` e termina com
`save_file` (D22). O Figma `bzu9PxxFur2tU3vlNCHvsY` é só leitura (histórico e
Inspirações `1:3`). **(UI)** monta uma tela a partir dos nodes de
`docs/mission/design-nodes.md`. Direção visual arredondada e premium (D23). Regra de respiro (usuário, 2026-11-08):
padding vertical simétrico e generoso em todo contêiner (cartões, opções, itens,
modais: mínimo 16 em cima e embaixo, 20–24 em cartões e opções com duas linhas), o
texto nunca encosta na borda; conferir por screenshot com zoom em cada variante; Referência-alvo do layout (usuário,
2026-11-08): `docs/design/referencias/alvo-layout-app.png` — janela escura com cantos
grandes; três colunas em painéis arredondados separados por respiro (navegação lateral
com logo, seções recolhíveis e cartão do usuário no pé; lista com ícone/avatar por
entrada, título + subtítulo e botão primário no pé; painel de detalhe com cabeçalho,
campos em caixas escuras com ações embutidas à direita — copiar, revelar, abrir — e
rodapé com metadados e ações Editar/Duplicar/Excluir); busca em pílula no topo;
superfícies em camadas com bordas sutis e brilho/gradiente leve; pastas/grupos com cor
própria (tint suave). Frames de formulário podem ser mais altos que 800 para mostrar o formulário inteiro (usuário,
2026-11-08); no app o corpo do detalhe rola com cabeçalho e rodapé fixos. Selos/badges pequenos e discretos (usuário, 2026-11-08): ~20 de altura, texto 11–12, tint
translúcido, sem borda forte, texto sempre em minúsculo (nunca caixa alta). Usar como estilo, sem copiar marca, textos ou ícones. Toda fase
(DESIGN) e (UI) segue isso. As Fases 1–4 (marcadas **(FIGMA)**) foram feitas no Figma
antes da troca de ferramenta e são migradas para o `Ward.figcraft` pelas Fases 4a–4f. Toda
fase a partir da 11 termina com `make check` limpo e escreve o teste do ataque de cada (SEG) que toca. Nenhuma fase lê ou mexe em
`~/.ssh`, `ssh_config`, `SSH_AUTH_SOCK`, `~/.aws`, no cofre real ou no KeePassXC (R19,
`CLAUDE.md`).

Crates previstos: `ward-core` (núcleo, sem dependência de Linux), `ward-plataforma`
(Linux), `wardd`, `ward` (CLI), `ward-nm` (native messaging), `app/` (Tauri + Svelte),
`extensao/` (Chrome MV3), `ward-ponte-teste` (só `modo-teste`).

## Milestone 0 — Design no FigCraft
Contrato: V0.1–V0.23

- [x] Fase 1 — **(FIGMA)** Ler a página Inspirações sem editar e salvar a lista dos
  filhos dela (para V0.21); criar as coleções de variables `Cores` (só o modo
  Escuro, D1), `Tipografia`, `Espaçamento`, `Raio` na página Design System. V0.1, V0.21. Feita no Figma; migrada pela Fase 4a.
- [x] Fase 2 — **(FIGMA)** Componentes base com variantes: `Botão`, `Campo`,
  `Campo secreto`, `Select`, `Badge ambiente`, `Badge tipo`, `Item de lista`, `Modal`,
  `Toast`, `Campo PIN`, todos ligados às variables. V0.2, V0.3. Feita no Figma;
  migrada pelas Fases 4b–4d.
- [x] Fase 3 — **(FIGMA)** Telas de destravar e primeiro uso: `Primeiro uso / *`,
  `Destravar / *` (inclusive "Pedido do agente"), todas no modo Escuro. V0.4–V0.6, V0.19. Feita no Figma; migrada pela
  Fase 4e.
- [x] Fase 4 — **(FIGMA)** Credenciais e nível de acesso: `Credenciais / Vazia`,
  `/ Cheia`, `/ Filtrada`, `Nível de acesso / *`. V0.7, V0.11. Feita no Figma; migrada
  pela Fase 4f.
- [x] Fase 4a — **(DESIGN)** Inventário e variables no `Ward.figcraft` (D22, D23). Abrir
  o arquivo (`open_file`) e levantar com `inspect_document` o que já existe (páginas,
  variables, estilos, componentes, frames); registrar em `docs/mission/design-nodes.md`.
  Se houver página Inspirações, só ler e salvar a lista dos filhos em
  `docs/mission/evidence/figcraft-inspiracoes.json` (V0.21); sem ela ou sem as imagens,
  ler a `1:3` do Figma (só leitura). Garantir as páginas Design System e Layout (ajustar
  as existentes, não duplicar). Recriar as coleções a partir do handoff da Fase 1 (só
  as primitivas em uso): `Primitivas`, `Cores` (só o modo `Escuro`, D1, com as
  semânticas da Fase 1 e as `icone/*` e `sombra/padrao` da Fase 2), `Tipografia`,
  `Espaçamento`, `Raio` com os valores novos de D23 e `Largura` (barra lateral,
  diálogo, formulário, selects). `save_file`. V0.1, V0.19, V0.21.
- [x] Fase 4b — **(DESIGN)** Estilos e ícones no `Ward.figcraft`: os 14 text styles e os
  2 effect styles da Fase 2 (sombras suaves de D23), ligados às variables; os 24
  `Ícone/*` em `Componentes / Ícones` e os da Fase 4 (`Ícone/chave`, `/gerador`,
  `/engrenagem`). Ids em `design-nodes.md`; `save_file`. V0.2, V0.3.
- [x] Fase 4c — **(DESIGN)** Componentes base, parte 1, já arredondados (D23): `Botão`,
  `Campo`, `Campo secreto`, `Select`, `Campo PIN`, com as variantes e propriedades da
  Fase 2, tudo ligado às variables (raio, espaçamento, largura). Ids em
  `design-nodes.md`; `save_file`. V0.2, V0.3.
- [x] Fase 4d — **(DESIGN)** Componentes base, parte 2, já arredondados (D23):
  `Badge ambiente`, `Badge tipo` (selos em pílula), `Item de lista` (badges alinhados em
  colunas), `Modal`, `Toast`, e os de apoio das Fases 3 e 4 (`Painel de marca`,
  `Indicador de passos`) e os de apoio da Fase 4 (`Navegação lateral`, `Item de
  navegação`, `Cabeçalho de grupo`, `Chip de filtro`, `Opção de nível`, `Diálogo de
  nível`). Revisar os paddings de todos (regra de respiro abaixo); o usuário apontou o
  texto colado na borda de baixo na `Opção de nível`. Religar as propriedades às camadas e aplicar text
  styles em todos (a importação perdeu isso, como na 4c). Pendências da 4c: alinhar a
  altura do `Botão` e do `Campo` numa altura padrão comum (como na referência-alvo) e
  igualar a altura dos estados do `Campo secreto`. Ids em `design-nodes.md`;
  `save_file`. V0.2, V0.3.
- [x] Fase 4g — **(DESIGN)** Logo do Ward (escolhida pelo usuário, 2026-11-08):
  `docs/design/referencias/logo-ward.png`, conceito da linha de cima (escudo com "W" e
  losango embaixo). Redesenhar em vetor no `Ward.figcraft` (não colar o PNG), com
  geometria limpa e cantos arredondados, cor ligada a `cor/primaria`/gradiente das
  variables: componentes `Logo / Ícone do app` (squircle com gradiente índigo, 512),
  `Logo / Símbolo` (cor e branco monocromático, legível em 16×16 — conferir por
  screenshot em 16, 24, 32), `Logo / Horizontal` (símbolo + "ward" em minúsculas,
  semibold). Trocar o cadeado provisório do `Painel de marca` pela logo. Exportar SVG e
  PNG (16–512) para `docs/design/logo/`. Ids em `design-nodes.md`; `save_file`. V0.23.
- [x] Fase 4e — **(DESIGN)** (Antes: trocar a marca antiga — cadeado + "Ward" — das variantes da `Navegação lateral` pela instância `Logo / Horizontal`.) Telas da Fase 3 no Layout do `Ward.figcraft`, no estilo
  novo e só com instâncias: `Primeiro uso / Criar cofre`, `/ Senha mestra`, `/ PIN`,
  `Destravar / Padrão`, `/ Senha errada`, `/ Pedido do agente` (conteúdo do handoff da
  Fase 3), conferidas por `screenshot` contra as Inspirações. Ids em
  `design-nodes.md`; `save_file`. V0.2, V0.4–V0.6, V0.19, V0.20.
- [x] Fase 4f — **(DESIGN)** (Antes: deixar `Campo`, `Campo secreto` e `Select` esticarem na largura do pai — hoje fixos em 320 — e trocar as larguras presas das telas da 4e.) (Credenciais em três colunas: navegação, lista e detalhe da entrada selecionada, como na referência-alvo) Telas da Fase 4 no Layout do `Ward.figcraft`, no estilo
  novo e só com instâncias: `Credenciais / Vazia`, `/ Cheia`, `/ Filtrada`,
  `Nível de acesso / Escolher`, `/ PIN`, `/ PIN errado` (conteúdo do handoff da Fase 4),
  conferidas por `screenshot`. Ids em `design-nodes.md`; `save_file`. V0.2, V0.4,
  V0.7, V0.11, V0.19, V0.20.
- [x] Fase 5 — **(DESIGN)** Formulários, parte 1: `login`, `banco`, `servidor` (com
  `Conectar`), `api`, `aws`, novo e edição, com TOTP e anexos. V0.8–V0.10.
- [x] Fase 6 — **(DESIGN)** Formulários, parte 2: `smtp`, `ftp/sftp`, `nota segura`,
  `genérico`, novo e edição, com campos extras e anexos; todos no modo Escuro.
  V0.8, V0.9, V0.19.
- [x] Fase 7 — **(DESIGN)** Janela de aprovação: `Aprovação / Normal`, `/ prod`,
  `/ PIN errado`, `/ PIN bloqueado`, `/ Expirando`, com os quatro botões em todos
  (R16) e foco em Negar; todos no modo Escuro. V0.16, V0.19.
- [x] Fase 8 — **(DESIGN)** Gerador e configurações: `Gerador de senha / *`,
  `Configurações / *` (sem opção de tema, D1), `Restaurar backup / *`,
  `Kit de emergência / Impressão`; todos no modo Escuro. V0.12–V0.15, V0.19.
- [x] Fase 9 — **(DESIGN)** Bandeja e extensão: `Bandeja / *`, `Extensão / *`, todos no modo
  Escuro; proposta de ícone. V0.17–V0.19.
- [x] Fase 9a — **(DESIGN)** Fluxo "Nova credencial" (usuário, 2026-11-08): ao clicar
  em `Nova credencial` na tela de Credenciais, o painel de detalhe mostra a escolha do
  tipo (os 9 tipos em cartões/lista com ícone, nome e uma linha do que guarda; busca
  opcional) e, escolhido o tipo, abre o `Formulário / <tipo> / Novo` já existente.
  Frames `Nova credencial / Escolher tipo` e `/ Tipo escolhido` (mostrando a transição
  para o formulário de login). Conferir que o botão `Nova credencial` das telas leva a
  esse fluxo. Ids em `design-nodes.md`; `save_file`. V0.8.
- [x] Fase 10a — **(DESIGN)** Limpeza de tokens no `Ward.figcraft`: remover da coleção
  `Primitivas` os tons que ficaram sem uso (claros do antigo modo Claro que a migração
  tenha trazido, ou valores de raio/espaçamento substituídos por D23). Antes de apagar,
  levantar por `inspect_document` o uso real de cada primitiva (alias em `Cores` e
  ligação em nodes); só apagar as sem nenhum uso (`neutro/0` e `neutro/50` estão em uso
  e ficam). `save_file`. V0.1, V0.19. Remover também `sombra/padrao` (`8:7`), sem uso desde a 4b. Avaliar também `largura/select`, `espacamento-letra/caixa-alta` e o `Chip de filtro`, sem uso desde a 4f.
- [x] Fase 10b — **(DESIGN)** Componente `Campo senha` (vazio, foco, erro, desabilitado;
  ação "revelar", sem "copiar"), ligado às variables; trocar por ele o `Campo` usado
  nas telas `Primeiro uso / Senha mestra`, `/ PIN`, `Destravar / *` e onde mais se digita
  senha mestra. V0.2, V0.3.
- [x] Fase 10c — **(DESIGN)** Refinamentos da 4d: cores de borda sutil por estado
  (sucesso, erro, aviso, ambientes) para Toast e Chip de filtro; ícone com cor por estado
  para o item ativo da `Navegação lateral` (a troca de ícone por propriedade não leva a
  cor); borda suave para os destaques de aviso/perigo/info (aviso da senha mestra e
  destaque do PIN, telas da 4e); medir contraste do `Badge ambiente` translúcido.
  Achados da 5: estado `vazio` (com placeholder) e ajuda como propriedade no `Campo
  secreto`; variante combobox (autocompletar) no `Select`; apagar itens ocultos das listas
  de api/aws. Navegação e lista com altura fixa no `Formulário / login / Novo` (`7450`)
  e na Fase 5 em geral (devem esticar com o frame); renomear a propriedade `Valor` do
  `Campo extra` segredo (a máscara é fixa). Ícone trocado no `Botão` primário fica cinza (deve
  ser branco). `Gerador de senha / No formulário` (`14307`) é cópia antiga de `6926`:
  conferir se acompanha o formulário final. Faixa de destaque da aprovação (repetida 3×) vira
  componente; ícones coloridos por override viram variantes. Achados da 9: variante escura do `Ícone da bandeja`
  para painéis claros; estado travado não só por cor (forma/indicador); popup `Salvar
  login novo` (892) rolando dentro do limite de 600 do Chrome; tirar ligações de
  gradiente que sobraram na variante cinza do `Ícone da extensão`. Achados da 10b: trocar o
  campo interno `13088` do `Diálogo de restauração` pelo `Campo senha` (e desfazer o
  override em `16683`); senha do site em `Extensão / Salvar login novo` (`17450`) vira
  `Campo secreto` sem copiar. Consolidar em `design-nodes.md` os ids da seção "Ids para design-nodes" dos
  handoffs 5–9 (fases feitas em paralelo).
  `save_file`. V0.2, V0.3.
- [x] Fase 10 — **(DESIGN)** Conferência final contra as Inspirações (só leitura):
  instâncias em todo o Layout, textos em português, nenhum segredo de exemplo; fechar
  `docs/mission/design-nodes.md` com todos os ids do FigCraft (conferidos reabrindo o
  arquivo salvo); `save_file`; apresentar ao usuário para aprovação no FigCraft (ícone
  incluído). V0.2, V0.4, V0.20–V0.23.
- [x] Fase 10d — **(DESIGN)** Ajustes do scrutiny do M0: limpar a ajuda copiada do `api`
  no Role ARN dos formulários aws (`10894`, `11594`); corrigir no handoff da Fase 10 a
  frase sobre segredos (`FALSOACCESSKEY01` aparece como identificador falso); scripts de
  evidência rodando de qualquer diretório e sem `__pycache__` (e `.gitignore`).
  QA do M0 (menores): Copiar em todos os campos não secretos do detalhe (porta, base);
  indicação de rolagem (fade) na lista atrás de `Nova credencial`; rodapé fixo cortando
  o cartão de nível em `login / Novo`; dados de exemplo coerentes (24 caracteres no
  gerador, grupo da aprovação existente na lista); PIN errado com comprimento variável;
  coluna de texto do "Tempo de trava" mais larga; respiro entre "De … para …" e "PIN"
  em `Nível de acesso / PIN errado`. `save_file`. V0.20.

Milestone 0 aprovado pelo usuário em 2026-11-08 (V0.23). As decisões em aberto da
Fase 10 (kit em folha branca, tempo de trava, senha/PIN após restaurar, "Negar" no PIN
bloqueado, Access Key ID visível, cor da logo, bandeja grafite/contorno, mínimo de 12 e
barra de força, "Agora não", filtros em diálogo e grupos fora da navegação) ficam como
estão no design aprovado; o PRD é ajustado se alguma mudar.

## Milestone 1 — Fundação
Contrato: V1.1–V1.6

- [x] Fase 11 — Workspace Cargo com os crates vazios, `rust-toolchain.toml`,
  `deny.toml`, `.gitleaks.toml` (allowlist só dos valores falsos de D15), `Makefile`
  com `make check` (fmt, clippy `-D warnings`, testes, `cargo audit`, `cargo deny`,
  gitleaks) e a seção "Comandos" do `CLAUDE.md`. V1.1–V1.3, V1.5, V1.6.
- [x] Fase 12 — `dev/compose.yml` com `postgres`, `ssh-senha`, `ssh-chave`,
  `ssh-salto`, `ssh-interno`, `wordpress` + `mariadb`, `phish`, `totp`, `redir`;
  chaves e senhas falsas geradas em `.dev/` por `make dev-servicos`; fixtures das
  entradas em `dev/fixtures/` (só dados falsos). V1.4, V1.5.

- [x] Fase 12a — Correções da auditoria de segurança do M1 (1ª reprovação):
  **alto** — os clientes SSH de `dev/servicos/comum.sh` e `ataques-servicos.sh` abrem
  `~/.ssh/id_*` do usuário (o ssh usa o home do `passwd`, não `$HOME`): `IdentityFile=none`,
  `CertificateFile=none` e teste de ataque sob strace que falha se algo do home real for
  aberto; **médio** — varredura extra de chave privada/AWS sem a allowlist global de
  caminhos do gitleaks (lockfiles, svg, pdf, png, gz); **médio** — `make check` falha com
  `.cargo/config*` com `[source`/`[patch`/`[registries`, `[patch` no `Cargo.toml` ou
  dependência por path fora da raiz; **baixo** — `GITLEAKS`/`CARGO` com `:=` e sha256 do
  binário do gitleaks conferido a cada varredura; anotar no `phase-11.md` que o
  `phase-11-ajuste.md` o substitui na allowlist.

- [x] Fase 12b — Achados médios e baixos da 2ª auditoria do M1 (aprovada), feita antes da
  Fase 13: `origens` vira lista de permitidas e roda como pré-checagem antes do passo 1
  (recusar qualquer `.cargo/config*` no repositório; no `CARGO_HOME` só chaves inofensivas;
  recusar `CARGO_*`/`RUSTC*`/`RUSTFLAGS*`/`RUSTDOCFLAGS` fora da lista — cobre `[alias]`,
  `rustc-wrapper`, `linker`, `rustflags`, `build.rustc`, `runner`, `[env]`); `cargo-audit`
  e `cargo-deny` pelo caminho absoluto; `PATH=/usr/bin:/bin` (+ `~/.cargo/bin`) nos scripts
  de `dev/` e no `Makefile`; `DOCKER_CONFIG` vazio em `.dev/`, `env -u DOCKER_HOST`,
  `GIT_CONFIG_GLOBAL=/dev/null` e `GIT_CONFIG_NOSYSTEM=1` nos testes; mutação de teste para
  o `CertificateFile=/dev/null`. Repros em `scratchpad/r2/` do auditor.

Pendência da Fase 12 (TOTP em `localhost:8083`), resolvida pelo planner em 2026-11-08:
o cofre de teste ganha `acme-totp-dev` (D28), acrescentada nas fixtures pela Fase 19 e
usada no V6.7.

## Milestone 2 — Núcleo: formato, cripto, tipos e máscara
Contrato: V2.1–V2.58

- [x] Fase 13 — Formato e cripto em `ward-core`: cabeçalho versionado com parâmetros
  Argon2id, chave de dados aleatória embrulhada, XChaCha20-Poly1305 com o cabeçalho
  como AAD, troca de senha mestra; vetores conhecidos e testes de adulteração.
  V2.1–V2.8.
  Notas da Fase 13: o V2.7 foi ajustado para `ctutils`/`cmov`, que substituíram o
  `subtle` no RustCrypto atual; o hash do PIN da
  Fase 22 deve passar por `cripto::argon2id` (o `argon2` 0.6 não zera a memória própria).
- [x] Fase 14 — Gravação atômica (temp + fsync + rename + fsync do diretório) e teste de
  morte no meio com processo filho; prova de que nada vai em claro para o disco.
  V2.9–V2.11.
- [x] Fase 15 — Modelo de entrada: os 9 tipos, campos secretos, campos extras, anexos,
  `grupo`/`ambiente`/`projeto`/tags, nível de acesso, validação em português; tipos de
  segredo com `Zeroizing` e `Debug` que imprime `••••`. V2.12, V2.13, V2.27, V2.28.
  Notas da Fase 15: as regras que ela decidiu sozinha estão em D25; o que faltou do
  V2.28 (`CofreAberto` e `SenhaMestra`) e a serialização vão para a Fase 15a.
- [x] Fase 15a — Corpo do cofre e acertos do núcleo (`entrada` → bytes → `formato`).
  Depende das Fases 13–15; não depende das 16–18 e pode rodar ao lado da 17. Arquivos:
  um módulo novo de serialização no `ward-core` (ex.: `crates/ward-core/src/corpo.rs` e
  `corpo/testes.rs`), `formato.rs`, `gravacao.rs` e a linha em `lib.rs`; não mexe em
  `entrega`, máscara, TOTP nem gerador.
  - Serialização de D26: codificação binária própria, versionada e canônica, sem
    dependência nova; buffer `Zeroizing` com capacidade calculada antes; leitura que
    confere cada comprimento contra o buffer e os limites de D25 antes de alocar, limite
    do corpo de 64 MiB e de 10 000 entradas; cada entrada relida por `Rascunho::validar`
    + `definir_nivel` (nível guardado) e o cofre por `conferir_no_cofre`; erro genérico
    sem valor. Funções `CofreAberto` → `Vec<Entrada>` e volta, usadas pelas Fases 19 e 22.
  - V2.28: `impl ZeroizeOnDrop for CofreAberto {}` e `for SenhaMestra {}` em
    `formato.rs`, com o teste de tempo de compilação da Fase 15 passando a cobri-los.
  - D27: `gravacao::gravar` vira `pub(crate)`; a troca de senha ganha uma função
    pública que sela e grava (como `gravar_cofre`); doctest `compile_fail` para o
    `gravar` de fora do crate.
  - Testes de ataque: ida e volta, adulteração (cortes, mutações, campos forjados),
    limites com alocador de contagem e alocador que procura segredo em bloco liberado
    (teste de integração próprio, por causa do `#[global_allocator]`). `make check`
    limpo. V2.28, V2.30–V2.34.
- [x] Fase 16 — Entrega: tipo → variáveis de ambiente (D10), todos os motores de
  `banco`, normalização de nomes de extras e anexos. V2.14–V2.16.
- [x] Fase 17 — Máscara em fluxo (D11): variações, pedaços quebrados, segredos
  sobrepostos, anexos, saída sem segredo intacta; teste de propriedade com cortes
  aleatórios. V2.17–V2.24.
- [x] Fase 18 — TOTP (RFC 6238) e gerador de senha com `OsRng`. V2.25, V2.26.
  Nota da Fase 16: fechar a parte do V2.14 que confere `WARD_TOTP` contra o código real
  (a 16 usou código falso pelo trait `Recursos`); avaliar restringir `?`, `#` e `/` na base
  de `banco` na validação (a 16 deixou o encoding só na URL).
- [x] Fase 19 — `make dev-cofre` (cria `.dev/cofre-teste.ward` com as 21 entradas do
  contrato pela serialização da Fase 15a; acrescenta `acme-totp-dev` de D28 em
  `dev/fixtures/entradas.json`) e `make dev-variacoes` (lista de segredos e variações
  para os validadores). Depende da Fase 15a. V2.29.
  Nota do planner (2026-11-08, D29, D30): os anexos das fixtures não podem ser vazios e
  somam no máximo 256 KiB por entrada (as chaves e o `app.env` já cabem). A 19 grava o
  corpo da forma que a 15a deixou; a seção de configuração com o hash do PIN de teste
  entra depois, pela Fase 19b, que ajusta o `make dev-cofre`. A 19 não precisa esperar.
- [x] Fase 19a — Limites e validação do núcleo (D29, D31–D34). Depende das Fases 15a–18;
  pode rodar ao lado da 19 porque não mexe em `dev/` nem no `Makefile`. Arquivos: um
  `crates/ward-core/src/limites.rs` novo, `entrada/validacao.rs`, `corpo.rs`, o
  `Gerador` da máscara e `tests/corpo_memoria.rs`.
  - `ward_core::limites` com constantes `pub` usadas por validação, corpo e máscara;
    sem número repetido (D32).
  - Anexos somados até 256 KiB por entrada; anexo vazio segue recusado; orçamento da
    máscara de 4 MiB de padrões por entrada com entrega, calculado pela mesma função do
    `Gerador` e conferido em `Rascunho::validar` (logo também na leitura do corpo),
    com a mensagem de D29 (D29).
  - Base do `banco` restrita: `[A-Za-z0-9_-]{1,63}`, e só número no `redis` (D33).
  - `CHAVE=valor` de anexo: o valor só vira padrão próprio com 6 caracteres ou mais
    (D34).
  - Alocador de teste com `alloc_zeroed` no lugar de `alloc` (D31).
  - Ajustar os testes de ida e volta da 15a ao V2.30 novo (anexos somando 256 KiB) e
    escrever o teste de ataque de cada (SEG). `make check` limpo. V2.30, V2.32,
    V2.35–V2.38, V2.40.
- [x] Fase 19b — Corpo v2 com seção de configuração (D30). Depende das Fases 19 e 19a.
  Arquivos: `corpo.rs` e testes, `formato.rs` (o `CofreAberto` passa a levar a
  configuração), o gerador do `make dev-cofre` da Fase 19.
  - Versão 2 do corpo: configuração obrigatória antes das entradas (hash PHC Argon2id
    do PIN por `cripto::argon2id`, tempo de trava em minutos com padrão 240, backup
    externo desligado com UUID opcional); a versão 1 vira desconhecida; cofre sem hash
    de PIN recusado; leitura com os mesmos cuidados de D26 (comprimentos conferidos
    antes de alocar, erro genérico). Tipos da configuração com `Zeroizing`/`Debug` de
    `••••` no hash (V2.27, V2.28).
  - Função que cria o hash a partir do PIN e outra que confere, esta em tempo constante
    pela biblioteca (o daemon da Fase 22 só chama).
  - `make dev-cofre` grava o hash do PIN de teste de D15; ida e volta, adulteração e
    limites (V2.30–V2.33) refeitos sobre o corpo v2. `make check` limpo. V2.29–V2.33,
    V2.39.
- [x] Fase 19c — Correções da auditoria de segurança do M2 (D36–D45). Depende da 19b
  e vem antes da Fase 20. Repros do auditor em
  `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/ataque/src/bin/`
  (`mascara.rs`, `validacao.rs`, `gravar.rs`): ler antes, rodar contra o código novo e
  transformar cada caso em teste de ataque do `ward-core`. Arquivos: `mascara.rs`,
  `entrada/validacao.rs`, `cripto.rs`, `formato.rs`, `configuracao.rs`, `corpo.rs`,
  `gravacao.rs`, `limites.rs`, os testes de cada um, `tests/corpo_memoria.rs`, o exemplo
  `dev-cofre` e `dev/testes/dev-cofre.sh`. Não toca em `wardd`, protocolo nem CLI.
  Ordem sugerida, do que muda o formato para o que só valida:
  - **Corpo v3 com `geracao`** (D39, só o núcleo): campo na seção de configuração, +1 em
    `gravar_entradas` e `trocar_senha` depois da gravação que deu certo, recusa em
    `u64::MAX`, leitura sem setter público, abertura com `geracao_minima` e erro
    `Retrocesso`; v2 e geração 0 recusadas; `make dev-cofre` grava a geração 1.
    V2.44.
  - **`gravar_cofre` e `CofreAberto::selar` em `pub(crate)`** (D44), com doctests
    `compile_fail`; `corpo_memoria.rs` passa a gravar por `gravar_entradas`. V2.50.
  - **Chave no heap** (D40): `cripto::Chave` = `Box<Zeroizing<[u8; 32]>>` para a chave
    de dados e a KEK, asserção de `size_of`, `buf` de `ler_phc_pin` em `Zeroizing`,
    saída de `conferir_pin` em `Zeroizing` comparada por `ctutils`. V2.46, V2.28, V2.33.
  - **Máscara** (D36, D37): passada por linha de `Gerador::anexo` em todo segredo de
    campo com `\n`; valor de `"chave": "valor"` e `chave: valor` nas linhas; formas
    `\u` do Go e do `ensure_ascii` e `\$` de aspas duplas; orçamento de D29 recontado
    pela mesma função, com o V2.36 refeito. Se uma entrada de D15 for recusada, a fase
    para e devolve ao planner. V2.41–V2.43.
  - **Validação** (D38, D41–D43): lista branca de texto por campo (sai a lista de
    `invisivel`), parser estrito de URL, variável do `api` por sufixo, usuário de
    `servidor`/`ftp/sftp` em `[A-Za-z0-9._-]`; tudo vale também na leitura do corpo.
    V2.45, V2.47–V2.49.
  - **Confirmações** (D45): teste da mensagem de `ErroCorpo::GrandeDemais` preso a
    `limites` (ou a mensagem montada a partir dele); conferir que os testes da 19a e
    da 19b cobrem as faixas confirmadas. V2.51.
  - Regravar `.dev/cofre-teste.ward` por `make dev-cofre`, refazer V2.29–V2.33 e V2.39
    sobre o corpo v3 e escrever o teste de ataque de cada (SEG). `make check` limpo.
    V2.29–V2.33, V2.36, V2.39, V2.41–V2.51.
- [x] Fase 19d — Lista do `make dev-variacoes` atualizada para D36 e D37 (handoff 19c,
  "Problemas"). Pequena; só `dev/variacoes.py` e o teste dele (um teste em Python só
  com a biblioteca padrão, rodado por `make teste-dev-cofre`, mais o que couber em
  `dev/testes/dev-cofre.sh`). Não toca em `ward-core`, `wardd`, `Makefile` nem nas
  fixtures. O `variacoes.py` continua oráculo independente:
  não importa nem chama o código da máscara.
  - D36: além das linhas aparadas e do `CHAVE=valor` que já existem, o valor de
    `"chave": "valor"` e de `chave: valor` em cada linha de segredo de várias linhas
    (chave privada, extra secreto com `\n`, anexos), com e sem aspas, só com 6+
    caracteres.
  - D37: para cada segredo e cada linha, as formas JSON do Go (`\u0026`, `\u003c`,
    `\u003e`, `\u2028`, `\u2029`), `ensure_ascii` do Python (`\uXXXX` minúsculo, com
    par substituto) e aspas duplas do shell (`\` antes de `$`, `` ` ``, `"` e `\`),
    cada uma só quando difere da literal. Hex em maiúsculas fica fora (RA3).
  - Atualizar a docstring do `variacoes.py` e a descrição do `make dev-variacoes` no
    `CLAUDE.md`. Testes com os valores falsos da V2.52, mais os de antes (V2.29).
    `make check` e `make teste-dev-cofre` limpos. V2.52, V2.29.

- [x] Fase 19e — Achados da reauditoria do M2 (D53–D55). Depende da 19d; **a Fase 22
  só começa depois dela**. Pode rodar ao lado da 21b (arquivos separados). Repros em
  `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/ataque/src/bin/`
  (`geracao.rs`, `desvio.rs`, `url.rs`): ler antes, rodar contra o código novo e
  transformar cada caso em teste de ataque. Arquivos: `formato.rs`, `corpo.rs`,
  `gravacao.rs` e os testes deles (geração); `mascara.rs` e `mascara/` (máscara);
  `entrada/validacao.rs` e testes (URL); `dev/variacoes.py`, o teste dele e
  `dev/testes/dev-cofre.sh` (oráculo e teste cruzado); se preciso, um exemplo novo do
  `ward-core` para o teste cruzado e a linha do `make dev-variacoes` no `CLAUDE.md`.
  Não toca em `ward-plataforma`, `wardd`, protocolo, CLI, `Makefile` nem nas fixtures.
  - **Geração queimada ao selar** (D53): o contador do `CofreAberto` sobe ao selar e
    fica lá em qualquer erro seguinte; `u64::MAX` recusa antes de selar, sem queimar;
    `abrir_entradas_desde(arquivo, senha, geracao_minima, geracao_queimada)` com o
    contador em max(disco, queimada); `trocar_senha(destino, atual, nova,
    geracao_minima, geracao_queimada)`; ajustar os chamadores (testes, exemplo
    `dev-cofre`). V2.44 (reescrita), V2.53.
  - **Máscara** (D54): `linhas` para todo valor secreto de entrega com `\n`; formas de
    texto por linha; aspas simples do shell, `printf %q` (ASCII, conferido contra o
    bash real), PHP `\/` + `ensure_ascii`, parser de linha com `chave : valor`,
    `chave:\tvalor`, `"k":"v"` e chave JSON com qualquer caractere menos `"`, campos de
    tab; orçamento de D29 pela mesma função, com o V2.36/V2.43 refeitos. Se uma entrada
    de D15 for recusada, a fase para e devolve ao planner. V2.54–V2.56.
  - **Oráculo** (D54): `variacoes.py` com a tabela única (sem base64 e miolos por
    linha; formas de texto e formas novas), docstring atualizada e teste cruzado
    oráculo → máscara em `dev/testes/dev-cofre.sh`. O oráculo segue sem importar nem
    chamar a máscara. V2.57, V2.52.
  - **URL** (D55): último rótulo do domínio começando por dígito (inclui `0x`) é
    recusado, ao salvar e na leitura do corpo. V2.58.
  - Regravar `.dev/cofre-teste.ward` por `make dev-cofre` se o formato do contador
    mudar, refazer V2.29 e V2.43, escrever o teste de ataque de cada (SEG). `make check`
    e `make teste-dev-cofre` limpos. V2.29, V2.36, V2.43, V2.44, V2.52–V2.58.
  - D56 (`sftp -l`) não é desta fase: fica para a Fase 27, que ganhou a nota.

Decisões do usuário em 2026-11-08 sobre a Fase 19c: RA4 (troca conjunta de cofre e estado com o daemon parado) fica fora do modelo de ameaça — o planner registra isso em `docs/escopo.md` › fora do modelo; D41 (http só em localhost) e D42 (sufixo obrigatório no `api`) confirmados; D43 mudou: usuário de `servidor`/`ftp-sftp` aceita `@` já na v1. Registrado pelo planner depois da 19c: RA4 no escopo e no PRD; D43 confirmado também para `banco`/`smtp` (ASCII visível, sem `-` no começo; V2.49); D37 e V2.42 com os escapes literais; Fase 19d criada; Fases 20 e 22 com a API nova do núcleo.

Decisão do usuário em 2026-11-09: acessibilidade (AT-SPI) desligada no app (`NO_AT_BRIDGE=1`, `WEBKIT_A11Y_BUS_ADDRESS=unix:path=/dev/null`); leitor de tela não funciona no Ward. Pendências da 29a para o planner: incluir as 3 variáveis de D48 (GStreamer, AT-SPI) na lista de D57; app conferir o papel na 1ª resposta do protocolo; como o app de teste ganha o papel app (Fases 22/23); log do código de saída do filho no daemon (Fase 23); texto da V5.31 (dumpable por `prctl`); `/tmp/nvidia-mps` (RA8); RA7 — RUSTSEC-2024-0429 talvez já não detectado.

Resolvido pelo planner em 2026-11-09: reauditoria da 19e em D61 (Fase 22a); Problemas do handoff 22 em D62 (Fase 22b); papel app só ao pid do lançador e app de teste em D63 (Fases 23 e 23a); pendências da 29a em D64 (lista de D57 e V5.31 já no PRD e no contrato; MPS, varredura e RA7 na Fase 29b). Acessibilidade desligada registrada em `docs/escopo.md` › "App desktop".

Decisão do usuário em 2026-11-09: limiares de força do gerador (Fase 37) aceitos — fraca <60 bits, razoável <80, forte <128, muito forte ≥128; a Fase 33 usa os mesmos. Pendências da 37 para o planner: faixa de tamanho 8–64 na interface (núcleo 4–128) no PRD; texto dos símbolos no FigCraft diverge de D35 (app segue D35) — corrigir numa fase de design.

Teste instável (2026-11-09): `ward-plataforma --test identificacao` › `ataque_binario_copiado_para_fora_da_instalacao` falhou uma vez com ETXTBSY ("Text file busy") ao executar o binário copiado — corrida clássica de fd de escrita herdado por fork de outra thread de teste. Corrigir na Fase 22b (fechar e sincronizar a cópia antes, ou repetir o spawn em ETXTBSY).

Pendências das Fases 32 e 25 para o planner (2026-11-09): (a) `DaemonSocket::trocar` do app ainda responde `SemProtocolo` — falta ligar a troca de quadros na conexão única (D47) e separar avisos sem pedido (`pedido_de_aprovacao`, `pedido_de_destrave`) — decidir a fase (23a ou 31); (b) o protocolo não tem operação que leve o valor ao app para "Revelar" (V5.3/V5.11 abertos); (c) N+1 em `listar_credenciais` (lista + 1 detalhe por entrada; a `ItemLista` do papel app pode ganhar usuário/porta); (d) datas criada/alterada não existem no protocolo; (e) V4.2 inteiro só no build de release (no `modo-teste` um falso não depurável passa) — validador roda a 2ª parte sem a feature, no contêiner, ou o texto do V4.2 muda.

Resolvido pelo planner em 2026-11-09 (D65): (a) transporte do app e fila de avisos na
Fase 23a; a 31 consome `pedido_de_destrave`, a 36, `pedido_de_aprovacao`; (b) "Revelar"
sem operação nova (a `revelar` já existe, só papel app, sem PIN, aprovação nem nível,
D4): daemon na Fase 22c, app na 33; (c) `resumo` na `ItemLista` do app, Fase 22c; (d)
datas no corpo v4, Fase 22d (provisória); (e) V4.2 em duas partes, a 2ª no contêiner,
Fase 28. Pendências da 37: faixa 8–64 e limiares em D65 (f); símbolos do FigCraft e
frames que faltaram na 32, Fase 37a.

Pendências da 37a (2026-11-09): contagens no FigCraft ficaram em 21/13, mas a fixture passou a 22 (D59) — atualizar numa próxima fase de design; cartões de nível dos 18 formulários e do gerador no formulário seguem soltos (ícone de check em "Com aprovação", herdado do M0) — trocar por instâncias do `Cartão de nível`.

Pendências da 33 para o planner (2026-11-09): anexos — não há operação para ler o conteúdo de um anexo e `salvar_entrada` substitui a entrada inteira, então editar entrada com anexo é recusado (`AnexosNaoMantidos`) e "Adicionar arquivo" está desabilitado; decidir operação (manter anexos por id no salvar, ou ler/gravar anexo) e fase. Botão Salvar no Novo vazio fica clicável (V5.10) ao contrário do design; placeholder da URL trocado por "Endereço com http ou https" (o `app-dist.py` recusa "https://").

Pendências da 22b (2026-11-09): V3.34 do lado do app (`VmLck: 0 kB` no app filho) fica para o contêiner da 23a/29b; `criar_cofre` com estado ilegível recusa com `falha_ao_gravar` sem dizer ao usuário o que fazer (mensagem a melhorar); gancho `com_gancho_depois_de_gravar` existe no release (nunca ligado) — auditoria do M3 olha.

Pendências da 34 (2026-11-09): editar com anexo segue bloqueado (mesma decisão da 33); o `ward-core` recusa nome de anexo começando por `.` (ex.: `.env`, citado no escopo) e a tela não renomeia — decidir; V5.24 ponta a ponta ("Conectar" → `emitir_token` no `wardd` responde `interno`, `ward ssh --token` vem na 27); lista de túnel não sabe quais servidores têm salto (erro só ao salvar); chave privada virou textarea mascarado; dependência direta nova `gtk =0.18.2` no `app/src-tauri` (diálogo de arquivo) — RA7/auditoria do M5 olha; `x-terminal-emulator` no COSMIC não conferido.

Pendências da 23a (2026-11-09): `make teste-sem-modo-teste` fica fora do `make check` (o scrutiny do M3 roda); o copiar não funciona na ponte aberta pelo daemon (ambiente mínimo sem `WAYLAND_DISPLAY`); binário de teste aceita g+w (umask 002); token da ponte no log 0600 (resíduo de D63); fila de avisos sem consumidor na interface (31/36); teste da 22b `v3_33_kill_9_entre_as_senhas_erradas` é sensível a tempo (907 ms > 900 ms sob carga) — dar margem.

Pendências da 35 (2026-11-09): 5º PIN errado deve ir a `Destravar / Padrão` (Fase 31); frame `6373` mostra dígitos no PIN errado, app esvazia (ajustar design); textos dos níveis em `credenciais.js` e `niveis.js` (unificar); teste `ataque_comandos_proprios_so_na_janela_e_origem_certas` do `app/src-tauri` falha se houver `wardd` de teste no socket fixo do uid — isolar o socket do teste.

Pendências da 26 (2026-11-09) — perguntar ao usuário: V4.13/RA1 reprova (environ e anexo do filho legíveis pelo mesmo uid, sem mitigação genérica); o ambiente da CLI (`PATH`, `BASH_ENV`, `NODE_OPTIONS`, `GIT_SSH_COMMAND`…) muda o que o comando aprovado faz e a janela de aprovação não mostra ambiente nem cwd. Para o planner: `<rt>` do release é `/run/user/<uid>/ward/` — sem sessão logind não existe (unit com `RuntimeDirectory=`?); `wardd` de produção agora espera até 2 min o `ward destravar` com cofre travado e sem app (V4.21); grupo inteiro morre no fim (neto com `setsid` escapa); V4.9/V4.12 não rodaram no worktree (serviços de outro checkout, sem `psql`) — validador do M4 roda; `teste-home-real` reprova em worktree (git abre `.git/worktrees/…`).

Pendências da 30 (2026-11-09): mínimo de 12 caracteres da senha mestra só no app (decidir se vai a `ward_core::limites` e ao daemon, inclusive troca de senha V5.22); normalização Unicode (NFC) da senha mestra entre app e CLI — decidir; rótulo de força "boa" do design vs rótulos do gerador; `make app-ponte` não serve para primeiro uso (`dev/daemon.sh` exige cofre).

Pendências da 22c (2026-11-09): lista do app com resumo repete URL/host; perto de 64 MiB de texto a lista passa de `MAX_QUADRO_RESPOSTA` e o daemon responde `interno` (decidir limite); log do `wardd` agora leva nome da entrada e campo em `revelar`/`copiar`/`totp` (D65 b; doc do `log.rs` atualizada); `Alvo::segredo_em` entrou no protocolo; V3.39 simula a extensão só por `argv[0]`. O `v3_33` ganhou margem (1500 ms + checagem relativa).

Pendências da 45 (2026-11-09) — decisão do planner/usuário: restaurar exige cofre destravado + PIN atual + senha da versão (além de D17/V7.7); com o cofre principal corrompido ou senha atual esquecida não há restaurar pelo app — decidir se existe restaurar com o cofre travado e como a 47 pede o PIN. Também: pedidos de aprovação pendentes não são cancelados ao restaurar (26/36); a 46 deve abrir a pasta pelo fd como o `wardd` e não pode usar `link(2)` em vfat; nome do backup em UTC (a 47 mostra no fuso local).

Pendências da 29b (2026-11-09) — canais novos ao planner: `gst-plugin-scanner` carrega `.so` de `~/.frei0r-1/lib`, `~/.ladspa`, `~/.libvisual/actor` (apagar o registry força; `FREI0R_PATH` fecha o frei0r, `GST_PLUGIN_LOADING_WHITELIST` fecha tudo mas o WebKit avisa appsink); `/dev/shm/cuda_injection_path_shm` e socket abstrato `@cuda-uvmfd-…`; `~/.nv/*`, `~/.mfx_trace`, `/dev/shm/GFX_MEDIA_TRACE` (família RA8); pasta de dados do WebKit `~/.local/share/local.ward.app/` (auditoria do M5 confere que nenhum valor revelado vai para lá); alinhar `unsound = "all"` no `deny.toml` da raiz.

Pendências da 31 (2026-11-09): V5.8 parcial — o aviso `pedido_de_destrave` não traz entrada nem comando (o cartão mostra só a contagem); fechar exige campo no aviso (entrada + comando neutralizado por D51) em `ward-core`/`wardd`/app — decidir; "Agora não" só esconde (não há como o app negar o destrave; expira em 2 min); espera de D20 em dobro (tela + daemon, até 30+30 s) — UX; senha errada esvazia o campo (ajustar frame `3393`, como o `6373` da 35); variable própria para a largura 560 do `Painel de marca`; com 16 avisos de aprovação parados na fila, destrave novo é descartado (falha fechada) até a 36 consumir.

Pendências da 22d (2026-11-09): relógio ilegível grava sem carimbo novo (entrada nova fica sem datas para sempre) — planner pode inverter para recusar; entrada antiga editada mostra só "Alterada em" (caso parcial fora do V5.34); datas seguem provisórias no escopo/PRD (D65 d).

Pendências da 46 (2026-11-09): cópia externa só com o cofre destravado (UUID está na parte cifrada) e só com o pendrive já montado (o Ward não monta; montar exigiria polkit) — planner decide; modo da `ward-backup` no vfat não é conferido (vem das opções de montagem); btrfs (dispositivo anônimo) nunca recebe cópia; loops do sistema (snap) aparecem em `pendrives`; V7.4/V7.5 com vfat real montado ficam para o validador do M7 (receita no handoff).

Pendências da 36 (2026-11-09): letras RTL fortes (hebraico/árabe) não são escapadas por D51 — decidir no escopo; fechar a janela nega todos os pendentes, inclusive os não vistos (falha segura; escopo fala em "o pedido"); protocolo não manda o prazo do pedido (app conta 120 s da chegada); texto do bloqueio do `CampoPin` difere do frame `16960`; V5.16 ponta a ponta e V5.28 (foco no COSMIC) ficam para o M5/usuário. Testes do `app/src-tauri` agora usam socket inexistente (`sem_daemon()`), independentes do `wardd` de teste no ar.

Pendências da 27 (2026-11-09): dependência nova `ssh-key =0.7.0-rc.12` (versão rc; acompanhar a estável; auditoria do M4 revisa); só Ed25519 (RSA dá `entrega falhou`) — decidir se o escopo exige RSA; protocolo `Ssh` sem cwd nem ambiente (filho com ambiente fixo, cwd = home) — decidir; modo chave usa `IdentityFile=<id.pub do Ward>` + `IdentitiesOnly` (D13 diz `IdentityFile=none` só no modo senha) — alinhar D13; `emitir_token` emite para `servidor` bloqueado (token inútil); caminho do `known_hosts` não fixado no escopo; V4.27 sem `inotifywait` e sem bwrap com home em tmpfs; V4.35/V4.37 por `/proc/<pid>/cmdline`, não por strace; V3.15 só varre o 1º nível de `wardd/src/` (deveria percorrer subpastas); RA2 a confirmar com o usuário; `make teste-ataques-servicos`/`teste-home-real` não rodam em worktree (git abre `.git/worktrees/…`).

Pendências da 38 (2026-11-09): frame `15242` diz "Trocar pede a senha mestra" no PIN, contrato e `wardd` pedem o PIN atual (corrigir FigCraft); máximo do tempo de trava: design 4 h, D45 1440 min — decidir; diálogos "Trocar senha mestra"/"Trocar PIN" sem frame; `trocar_senha` sem espera crescente de D20 na senha atual (só papel app) — decidir; mínimo de 12 e NFC continuam só no app; operação nova `configuracao` (só app) convive com `backup_externo` da 46.

Pendências da 24a (2026-11-09): 1º `kill -SEGV` cai no tratador de pilha do Rust e só o 2º mata (sem core nos dois; V3.6 fala em um); nesta máquina `suid_dumpable=2` e `core_pattern` é pipe do apport — o que impede o core é `PR_SET_DUMPABLE=0` (levar à instalação, M9); sem `gdb` no host; nome de variável que começa com byte inválido sai vazio na mensagem; extensão divide as 64 vagas com o agente.

Pendências da 39 (2026-11-09): menu da bandeja é acessível a qualquer processo do mesmo uid pelo barramento da sessão (só textos fixos e travar/mostrar/fechar; auditor do M5 confere); dependências novas só no app: `tray-icon 0.25.1`, `libappindicator 0.9.0`, `libappindicator-sys 0.9.0`, `libloading 0.7.4`; `configuracao` conta como uso no `wardd`, então a bandeja só lê o tempo de trava ao destravar/criar; ícone só até 32 px (falta export 64 px); `make teste-logs-app` falta na tabela do CLAUDE.md; bandeja e janela de aprovação no COSMIC ficam para o roteiro (`docs/mission/roteiro-usuario.md`).

Pendências da 48 (2026-11-09): frames `16178`/`16334` mostram caminho de montagem e "última cópia", que o protocolo não traz — corrigir FigCraft ou criar operação de última cópia (só data e contagem); diálogo "Escolher pendrive" sem frame; app só liga com pendrive conectado agora (o `wardd` aceita qualquer UUID no formato); `neutralizar` não escapa `\` (cosmético).

Pendências do scrutiny do M3, 2ª rodada (aprovado, 2026-11-09): `teste-socket-ataques` regrava `target/debug/wardd` (o mesmo do `dev-daemon`) — usar `--target-dir` próprio; vaga do app só com teste de unidade; `ganchos-teste` aceito (só dois ganchos, depende de `modo-teste`).

Pendências da 28 (2026-11-09): salto não usa `ProxyJump` (o OpenSSH 9.6 roda o ProxyJump por `$SHELL -c` e o ssh do salto ignoraria `-o`, lendo `~/.ssh`); o daemon faz a ponte local em `127.0.0.1:0` aceitando só o grupo de processos da sessão — ajustar o texto de D13/D43; túnel só aceita o grupo de processos do `ward run` (contêiner `--network host`/`setsid` não conectam); cada conexão do túnel faz um login SSH novo; `ward run` de `servidor` com salto não abre a ponte.

Auditoria de segurança do M3 (aprovada, 2026-11-09; `docs/mission/evidence/M3/seguranca/`): **alto** fora do M3 — token do "Conectar" vai no argv do terminal e do `ward ssh --token` (`app/nucleo/src/terminal.rs:127`, `crates/wardd/src/ssh.rs:355`), legível em `/proc/<pid>/cmdline` por qualquer processo do mesmo uid, que pode usá-lo primeiro e pular nível e aprovação (Fase 27a); **médio** — ambiente da CLI no `executar` (só `LD_*` sai; `PATH`/`BASH_ENV`/config de ferramentas mudam o comando aprovado e a janela não mostra), o auditor recomenda bloquear o M4 — decisão do usuário; **baixo** — recusa de `LD_PRELOAD` roda depois do `ld.so` carregar a biblioteca (V3.7 é defesa em profundidade; a unit do systemd garante ambiente limpo, M9).

M3 aprovado (2026-11-09): scrutiny (2ª rodada), segurança e uso; V3.1–V3.41 marcados. Ressalvas do uso: Destravar com senha errada não mostra o botão desabilitado com contagem do frame `3393`; V4.1 do contrato ainda diz 19 entradas (o cofre tem 22, a CLI lista 21).

Pendências da 40 (2026-11-09): papel extensão no host de desenvolvimento — sem grupos nem setgid o `ward-nm` de teste é sempre agente, então o QA de V6.2 (UI)/V6.7 no Chrome de teste precisa de uma exceção de `modo-teste` como D63 (pai Chrome conferido pelo `/proc`, sem egid) — decisão do planner na Fase 41; host não confere o id da extensão (o `allowed_origins` do manifesto confere; id fixo na 41/51); ramo do pai não-dumpable de D47 não implementado (falha fechada); daemon fora do ar vira `interno`; `--load-extension` não funciona no Chrome 137+ (usar `Extensions.loadUnpacked` pelo CDP).

Pendências da 27a (2026-11-09): token por `socketpair` herdado (`--token-fd`); testado com `cosmic-term` e `xterm`; terminais que entregam o comando a um servidor já aberto (gnome-terminal) não repassam o fd e o `ward ssh` falha fechado; D3 do PRD e V5.24 do contrato ainda falam em `--token <t>` em argv — atualizar o texto.

Pendências da 44 (2026-11-09): RA1 — processo do mesmo uid pode abrir `/proc/<chrome>/fd/3` e ler o perfil temporário durante a sessão; V6.15 diz "domínio não confere" e o código usa a mensagem de `OrigemDiferente`; TOTP só com `autocomplete="one-time-code"` (login em duas etapas cai em `LoginRecusado`); Playwright do Python fora do repositório (`PW_PYTHONPATH`); Chrome fora do `home-real.sh`; `SIMPLES` agora com 32.

Pendências da 49 (2026-11-09): `window.print()` não conferido no app real (WebKitGTK deve abrir a impressão do GTK; validador do M7 confere, senão comando nativo); pastas da folha são as fixas da instalação; texto "Como restaurar" depende da decisão da Fase 47 (restaurar exige cofre destravado + PIN); barra "Fechar/Imprimir" sem frame no FigCraft; pendrive desconectado com o backup ligado não coberto pelo QA.

## Milestone 3 — Daemon e protocolo
Contrato: V3.1–V3.41

- [x] Fase 20 — (Depende da Fase 19c; não depende da 19d.) Protocolo em `ward-core`: quadros com limite de tamanho, mensagens
  versionadas, papéis agente/app/extensão (D5) e a tabela operação × papel, sem
  operação que devolva segredo ao papel agente; testes de fuzz do decodificador; limite
  de quadro do cliente comum (V3.4) e envio de anexos de até 256 KiB somados pelo papel
  app (D29), com os números de `ward_core::limites` (D32). As respostas de erro do
  destravar e das gravações trazem, sem valor, os casos de D39 que a Fase 22 usa
  (`Retrocesso`: "o cofre no disco é mais antigo que o último aberto"; "o arquivo do
  cofre mudou fora do Ward"; `GeracaoEsgotada`). Nenhuma mensagem carrega a geração
  para o papel agente nem aceita geração vinda do cliente.
  V3.3, V3.4, V3.15.
- [x] Fase 21 — `ward-plataforma`: diretório do socket 0700, `SO_PEERCRED`, identificação
  do processo (exe instalado, dono root, `TracerPid`, `LD_*`, pai Chrome), cadeia de
  processos para exibir, endurecimento do processo (`PR_SET_DUMPABLE=0`,
  `RLIMIT_CORE=0`, recusa de `LD_*` e de depurador, `mlock` dos buffers de segredo,
  D14). V3.1, V3.2, V3.5–V3.7.
- [x] Fase 21b — (Liberada: D46 e D47 confirmadas pelo usuário em 2026-11-08; D48 no
  ramo de D47; D49 sem mudança.) Pode rodar ao lado da 19e. Arquivos:
  `crates/ward-plataforma/` (código e testes), `app/nucleo/src/daemon.rs` e os testes
  dele, o contêiner de teste e o alvo dele no `Makefile`, e a linha do alvo no
  `CLAUDE.md`; não toca em `ward-core`.
  - App (handoff 29): `DaemonSocket` em `app/nucleo/src/daemon.rs` procura hoje o
    socket em `/run/user/<uid>/ward/wardd.sock`. Passa a usar
    `/run/ward/<uid>/wardd.sock` e a mesma conferência do `conferir_servidor` (toda
    pasta do caminho do root, sem escrita para grupo e outros, sem link; socket do
    próprio uid e 0600; par com uid 0 no `SO_PEERCRED`), antes de mandar qualquer byte;
    o diretório `ward-teste/` e a exceção do uid 0 ficam só no `modo-teste`. Os
    ataques que já existem em `daemon.rs` continuam e ganham o do servidor falso do
    mesmo uid (V3.23). Basta `cargo test` do `app/nucleo`; o `make app-check` exige
    `libwebkit2gtk-4.1-dev` (R32).
  `ward-plataforma`, ajustes da Fase 21:
  - D46: `escutar_de_listen_fds()` (aceita o fd do systemd e confere que é socket Unix
    em escuta no caminho esperado); `conferir_servidor` passa a exigir pasta do root e
    par de uid 0, com a exceção só no `modo-teste`;
  - D47: papel pelo egid do `SO_PEERCRED` (grupos `ward-app`/`ward-nm` vindos da
    `Instalacao`), `PPid` do app igual ao `systemd --user` ou ao daemon, recusa com
    `ptrace_scope` 0 ou `suid_dumpable` 1; a leitura de `exe`/`environ`/`maps` fica só
    para o pai Chrome e a cadeia exibida;
  - D48: lista insegura da glibc no `conferir_ambiente_proprio`.
  Testes em contêiner (`ubuntu:24.04`, root só dentro dele). V3.5 (forma de D47),
  V3.23–V3.25.
- [x] Fase 22 — (Depende das Fases 19e e 21b; não começa antes delas.) `wardd`: estado travado/destravado, destravar com espera crescente
  (D20), travar com zeroização, PIN (confere pelo hash da seção de configuração do
  corpo v3 e pela função da Fase 19b, D30; 5 erros travam), mudar nível só do
  papel app com PIN, `lista` sem bloqueadas, logs sem segredo; lê e grava o corpo pela
  Fase 15a (D26) e só por `gravar_entradas` (D27, D44), com `limpar_temporarios` ao abrir.
  API do núcleo depois da 19c (handoff 19c):
  - abre por `corpo::abrir_entradas_desde(arquivo, senha, geracao_minima,
    geracao_queimada)` (API da 19e, D53), com `vista` e `queimada` lidas do arquivo de
    estado; `ErroAbrir::Retrocesso` mantém travado e mostra o aviso de D39; depois de
    abrir, sobe `vista` para a geração lida, se for maior;
  - grava por `gravar_entradas(destino, &mut cofre, entradas)`: o daemon guarda o
    `CofreAberto` destravado como `&mut` (um dono só, sem cópia). **Antes** de chamar,
    grava no estado `queimada` = `cofre.geracao()` + 1 (atômico, com fsync), e só
    então grava o cofre, cujo rename acontece dentro de `gravar_entradas`; se a escrita
    do estado falhar, o cofre não é gravado. O núcleo queima a geração ao selar, então
    em erro o `CofreAberto` já subiu e a próxima gravação usa outro número; depois da
    gravação que deu certo, `vista` = `cofre.geracao()` (D53). `GeracaoEsgotada`
    recusa a operação sem tocar no disco, com mensagem sem valor;
  - troca a senha mestra por `gravacao::trocar_senha(destino, atual, nova,
    geracao_minima, geracao_queimada)`, com o mesmo registro de `queimada` antes; trata
    `ErroGravacao::Retrocesso` igual à abertura, e um erro depois de selar trava o
    cofre (D53); `formato::trocar_senha` não sela e não é chamada direto pelo daemon;
  - `gravar_cofre`, `gravar` e `CofreAberto::selar` são `pub(crate)` e ficam fora do
    daemon.
  Contra rollback (D39, D53): arquivo de estado com `vista` e `queimada`, abertura com
  `geracao_minima`, recusa sem estado com cofre existente, conferência de `dev`/inode/
  `mtime`/tamanho por `fstat` antes de gravar ou reabrir (mudou → não grava, trava e
  avisa). RA4 (troca conjunta de cofre e estado com o daemon parado) está fora do modelo
  de ameaça por decisão do usuário (2026-11-08) e não ganha trabalho.
  Notas das Fases 20 e 21:
  - na troca de senha, `ErroGravacao::Formato` de `trocar_senha` responde
    `senha_incorreta`; em `gravar_entradas` segue `falha_ao_gravar` (D52);
  - a `lista` do papel agente sai sem o nível; o app continua recebendo (D50); entrada
    criada nasce "com aprovação", e criar com outro nível pede PIN (D50);
    `remover_entrada` e `tempo_trava` sem PIN, só do papel app (D50);
  - `main`: `iniciar_endurecido()` antes de tudo (V3.7); socket por `LISTEN_FDS` quando
    houver (D46) ou, só no daemon de teste, pelo diretório de teste; a cada conexão,
    `conferir_conexao` com log da recusa e papel pela classificação vigente
    (egid de D47, da Fase 21b; `ClassePar` da Fase 21 fica para o pai Chrome);
  - V3.2 por `make teste-docker-uid` (D49): contêiner `ubuntu:24.04`, `wardd` de teste
    como uid 1000 via `setpriv`, clientes uid 4242 sem e com `CAP_DAC_OVERRIDE`
    ambiente e uid 0, e um cliente do uid 1000 no fim; nada do `/run/user` do host é
    montado; o alvo fica fora do `make check`.
  Notas da 21b (D57, D60):
  - `make dev-daemon` sobe o daemon de teste com `env -i` e lista fixa (`HOME`, `PATH`,
    `XDG_RUNTIME_DIR`, `LANG`, mais as do `modo-teste`): `TMPDIR` está na lista
    insegura da glibc e o `wardd` recusaria subir (V3.30);
  - o papel vem de `ProcessoPar::classificar` sem depender do ramo do `systemd --user`,
    que a Fase 29a tira (D57; a assinatura não muda);
  - os testes do daemon não fixam o número de entradas do cofre de teste (21 hoje, 22
    depois da Fase 22a): contam pela fixture;
  - o daemon não lança processo nesta fase; quando lançar (Fases 23 e 26), nunca passa
    o próprio ambiente (`LISTEN_*`).
  V3.2, V3.8–V3.13, V3.17, V3.20, V3.22, V3.26–V3.28, V3.30.
- [x] Fase 22a — (Depende da 22: as duas mexem no `Makefile` e nas contagens do cofre de
  teste.) Fixture com caracteres especiais e Go com `ensure_ascii` (D58, D59).
  Arquivos: `dev/fixtures/entradas.json`, `crates/ward-core/examples/dev-cofre/`,
  `crates/ward-core/src/mascara.rs` e testes, `dev/variacoes.py` e testes,
  `dev/testes/dev-cofre.sh`, allowlist do gitleaks, `Makefile` (só textos de ajuda) e
  as contagens nos testes que existirem.
  - entrada `delta-api-simbolos` de D59 no cofre de teste (22 entradas);
  - variação combinada Go + `ensure_ascii` de D58 na máscara e no oráculo, com o teste
    cruzado e o V2.36/V2.43 refeitos pela função de custo de D29 (se uma entrada de
    D15 passar a ser recusada, a fase para e devolve ao planner);
  - texto de ajuda do `dev-variacoes` no `Makefile` atualizado para a tabela única de
    D54 (vem da 19d; o da linha no `CLAUDE.md` já foi feito na 19e).
  V3.29 (e V2.29/V2.30 com 22 entradas).
  - **Reauditoria da 19e (D61, decidida em 2026-11-09):** na máscara e no oráculo, com
    teste cruzado e o mesmo orçamento: valores JSON na linha (par `"chave": "string"`
    em qualquer posição de linha de texto, e no segredo de uma linha que começa por `{`
    ou `[`, decodificado, 6+ → valor de linha); YAML/dotenv com comentário no fim,
    chave YAML com espaço ou `/` e `''` do YAML; campos de tab de linha que não é UTF-8
    só pelo literal e percent. `repr()`/`util.inspect`, `iconv`, array JSON e flow do
    YAML ficam em RA3 (não implementar). Os repros estão em
    `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/ataque/src/bin/contorno19e.rs`
    (só leitura; os testes da fase são próprios e com valores falsos). Se uma entrada de
    D15 passar a ser recusada pelo orçamento, a fase para e devolve ao planner.
    V3.31, V3.32.
  `make teste-dev-cofre` falha em master até a 22a (tabela já pede `delta-api-simbolos`).
- [x] Fase 23 — Motor de aprovação com relógio injetado e aprovador em processo só nos
  testes (D8): uma vez, 1 hora (D2), sempre, expirar em 2 min, fechar nega, nível
  combinado (D19), aprovação indisponível (D21); espera de até 2 min pelo destrave
  (D16). O texto do pedido (`comando`, `cadeia`) vai exato para o app, e a função que o
  formata para exibir, no `ward-core`, neutraliza controle e bidi por escape visível
  (D51), com testes. Operação `abrir_app` (D57): sem segredo, de qualquer papel, uma
  instância por vez, que chama o lançador da Fase 29a; o motor abre o app por ela
  quando precisa da janela, e sem sessão Wayland válida responde "aprovação
  indisponível" (D21). Se a 29a ainda não tiver fechado, a operação entra com o
  lançador como dependência e a fase espera. V3.14–V3.16, V3.18.
  Nota do planner (D63, 2026-11-09), para a fase em andamento:
  - o lançador guarda o pidfd do processo que abriu; o papel app passa a exigir egid
    `ward-app` **e** pid igual ao do pidfd, vivo, **e** `PPid` = daemon (qualquer outro
    filho do daemon fica agente). A classificação fica num ponto só, para a 23a
    acrescentar a exceção do `modo-teste`; **não** faça a exceção de teste aqui;
  - a resposta de `estado` traz o papel (`agente`/`app`/`extensao`), sem outro dado;
  - o log do daemon registra o código de saída (ou sinal) do app lançado, só o número.
  V3.36 (a parte do classificador), V3.38 (a parte do protocolo e do log).
- [x] Fase 23a — (Depende da 23.) App de teste com o papel app (D63). Arquivos:
  `crates/wardd` (config e classificação, só atrás de `modo-teste`),
  `crates/ward-plataforma` (`lancar_para_teste`: ambiente fixo mínimo para a ponte,
  saída em `.dev/logs/<binário>.log` 0600), `app/nucleo` e `crates/ward-ponte-teste`
  (conferir o papel na primeira resposta e sair sem `app`), `dev/daemon.sh`, `Makefile`
  (`dev-daemon` com `WARD_DEV_APP`, `app-ponte` pelo `abrir_app`) e as linhas no
  `CLAUDE.md`.
  - no `modo-teste`, o papel app vale para o pid do lançador sem exigir o egid; o
    `abrir_app` lança só o `WARD_DEV_APP` (absoluto, arquivo regular do uid, sem link,
    dentro do repositório), nunca algo vindo do pedido;
  - teste sem a feature (par sem egid aberto pelo lançador fica agente), conferência de
    que nenhum membro do workspace liga `wardd/modo-teste` ou
    `ward-plataforma/modo-teste` em dependência normal (`cargo tree -e features`), e
    `strings` do release sem `WARD_DEV_APP`;
  - ataques do V3.37 (cliente Python, ponte à mão, cópia, app fora do lançador,
    `abrir_app` com caminho/ambiente no pedido).
  - transporte do app (D65 a), em `app/nucleo/src/daemon.rs` e na ponte:
    `DaemonSocket::trocar` por `escrever_quadro`/`ler_quadro` na conexão única de D47;
    thread leitora única, um pedido por vez sob mutex, avisos (`pedido_de_aprovacao`,
    `pedido_de_destrave`) numa fila limitada a 16 que a 31 e a 36 consomem; resposta
    sem pedido, quadro acima do limite ou tipo fora do papel app fecham a conexão;
    prazo de 130 s; testes com daemon falso que intercala avisos. Sem evento do Tauri
    nesta fase.
  V3.36, V3.37, V3.38, V5.33.
- [x] Fase 24 — Trava automática: `Lock`/`LockedHint` e `PrepareForSleep` do logind,
  4 h sem uso com relógio injetado, lidas do tempo de trava da configuração do cofre
  (D30); units de teste `wardd@.socket`/`wardd@.service` do sistema (D46), com
  `LimitCORE=0` e **sem** `NoNewPrivileges=`, `RestrictSUIDSGID=`,
  `MemoryDenyWriteExecute=` nem opção que implique `no_new_privs` com `User=` (D57:
  senão o setgid do app não vale). V3.19, V3.21.
  Nota do planner (D62, 2026-11-09), para a fase em andamento: as units de teste levam
  `LimitMEMLOCK=infinity` (o `mlockall` entra na 22b e, com limite, o Argon2 falharia).
  A trava automática não mexe no contador de espera do D20, que a 22b separa por papel.
- [x] Fase 22b — (Depende das 23 e 24: as três mexem em `crates/wardd`.) Problemas do
  handoff 22 (D62). Arquivos: `crates/wardd`, `crates/ward-plataforma`
  (`iniciar_endurecido`), `crates/ward-core` (`gravar_entradas` devolve a identidade do
  fd; criação com geração inicial, se faltar) e os testes.
  - espera do D20 com contador por papel; o do agente no arquivo de estado (linha nova,
    formato antigo lido como 0, fsync antes da resposta de erro), aplicado ao subir;
  - `mlockall(MCL_CURRENT | MCL_FUTURE | MCL_ONFAULT)` no `iniciar_endurecido`: release
    falha fechada sem `RLIMIT_MEMLOCK` infinito; `modo-teste` só avisa;
  - `criar_cofre` com estado velho nasce na geração `queimada` + 1;
  - identidade do cofre gravado pelo `fstat` do temporário, sem `stat` depois do rename;
  - testes de ataque: `kill -9` entre as senhas erradas, agente atrasando o app, cópia
    antiga depois do `criar_cofre`, troca logo depois do rename, `VmLck` no contêiner.
  V3.33–V3.35.
- [x] Fase 22c — (Depende da 22b e da 23a: mexe em `crates/wardd`, no protocolo e no
  `app/nucleo`.) Operações de valor do app e lista do app (D65 b, c). Arquivos:
  `crates/ward-core/src/protocolo*` (só `ItemLista`/`ResumoItem`), `crates/wardd`,
  `app/nucleo` (`cliente.rs`, `credenciais.rs`, `daemon_falso.rs`) e os testes.
  - `wardd` atende `revelar`, `copiar` e `totp` do papel app: cofre destravado, sem PIN,
    sem aprovação e sem olhar o nível (D4), `Segredo` em `Zeroizing`, log sem valor;
    agente e extensão seguem "não autorizado";
  - `ItemLista.resumo`, `Some` só para o papel app, com a recusa nos dois sentidos de
    D50; o app monta a 2ª linha e a busca pela `lista`, sem um `detalhe` por entrada;
  - testes de ataque: cliente Python agente pedindo `revelar`/`copiar`/`totp`
    (`acme-pg-dev` e `gama-bloqueada`), quadro forjado com `resumo` no papel agente,
    log contra `.dev/variacoes.txt`. V3.39, V3.40.
- [x] Fase 22d — (Depende da 22c e da 22a.) Datas de criação e alteração (D65 d,
  provisória). Arquivos: `crates/ward-core` (`entrada`, `corpo` v4, `protocolo` só no
  `DetalheEntrada` e no rascunho), `crates/wardd` (carimbo pelo relógio injetado),
  `crates/ward-core/examples/dev-cofre/`, `app/nucleo/src/credenciais.rs`,
  `app/src/telas/Credenciais.svelte` (só o rodapé) e os testes.
  - corpo v4 com `criada_em`/`alterada_em`; v3 recusada; `make dev-cofre` regrava o
    cofre de teste e o V2.29 é refeito;
  - nova → as duas; edição e `mudar_nivel` → `alterada_em`; data no rascunho recusada;
  - testes de formato de V2 (byte alterado, gravação morta) repetidos com o v4.
  V3.41, V5.34.

- [x] Fase 24a — Correções do scrutiny do M3 (2026-11-09; `docs/mission/evidence/M3/`).
  - V3.25 (reprovada): `nome_para_mensagem` (`crates/ward-plataforma/src/endurecimento.rs`)
    corta no primeiro byte fora de `[A-Za-z0-9_]` e no 64º, sem `?` nem `…`; teste
    atualizado.
  - Conexão do papel app (assinante de avisos) não é despejada pelo limite de 64
    conexões (`crates/wardd/src/servidor.rs` `reservar`): vaga reservada ao app.
  - V3.3/V3.4 no repositório: levar `docs/mission/evidence/M3/{ataque,matriz}.py` para
    `dev/testes/` (10 000 conexões caladas; matriz operação × entrada × campo pelo
    socket, sem padrão de `.dev/variacoes.txt`) e ligar num alvo `make`.
  - V3.6 no binário: teste que sobe o `wardd` e confere `/proc/<pid>/{mem,environ}`,
    `limits` (core 0) e `PTRACE` negado.
  - `com_gancho_depois_de_gravar` e `com_programas_ssh` fora do release (`#[cfg(test)]` ou
    feature só de teste aceita pelo V3.15); V3.15 percorre subpastas de `wardd/src/`.
  - Alvos que usam `.dev/` com nomes fixos (`teste-dev-cofre`) com trava (`flock`) ou
    nomes únicos por execução.
  - `plan.md`: linha duplicada "`ward-plataforma`, ajustes da Fase 21:".

## Milestone 4 — CLI e integração Claude (run/ssh)
Contrato: V4.1–V4.38

- [x] Fase 25 — CLI `ward`: `lista [--json]`, `travar`, `destravar` (senha do TTY, só
  depois de conferir que o servidor é o `wardd` instalado), mensagens em português.
  A conferência é a de D46 (pasta do root e par de uid 0). V4.1, V4.2, V4.18, V4.22.
  Nota do planner (D62, D63, 2026-11-09), para a fase em andamento: a CLI é papel
  agente; o prazo de leitura do `ward destravar` tem de passar dos 30 s da espera de
  D20 mais o Argon2 (a espera do agente passa a sobreviver ao reinício do daemon, Fase
  22b). A resposta de `estado` ganha o campo do papel (Fase 23); a CLI não depende dele
  e não deve recusar um campo a mais vindo da 23.
- [x] Fase 26 — `ward run`: o daemon cria o filho com argv, cwd e ambiente da CLI sem
  `LD_*` (D13), entrega de D10, anexos em `<rt>tmp/` 0600 com limpeza em toda saída e
  na partida, saída mascarada, código de saída, espera do destrave. Pesquisa e
  registro de RA1 (environ e anexos lidos por outro processo do mesmo uid), com teste
  do ataque. O ambiente do filho vem só do da CLI, nunca do daemon (sem `LISTEN_*`,
  D60). A CLI mede argv + ambiente antes de mandar e passa de 750 KiB com
  mensagem clara (D52). Teste de ataque de D63 no contêiner: `ward run <livre> --
  /usr/local/bin/ward-app` (e `ward-nm`) fica agente. V4.3–V4.17, V4.19–V4.21, V4.36,
  V4.38.
- [x] Fase 27 — `ward ssh` com senha e com chave: `sshpass -e`, agente temporário só do
  `ssh` filho (D12), `-F /dev/null` e `known_hosts` próprio (D13), pty interativo,
  token do "Conectar" ainda sem app (token inválido vale o nível, D3); no pty, o fim
  segurado pela máscara é solto como `••••` após 300 ms sem saída nova, guardando o
  estado do casamento (D34). O usuário vai só por `-l <usuário>`, o host depois de
  `--`, por `execve` sem shell, nunca `usuário@host` nem `-o User=` (D43); como o
  usuário pode ter `@` (`deploy@cliente.com.br`), a fase tem teste de ataque com um
  usuário com `@` conferindo o argv. Confirmar
  RA2 com o usuário antes de fechar o milestone. No `sftp` e no `scp`, onde `-l` é
  limite de banda, o usuário vai como `-o` + `User=<usuário>`, com `-F /dev/null` e o
  host depois de `--` (D56), com teste de argv por `strace` e login real no SSH de
  teste.
  V4.14, V4.23, V4.24, V4.26–V4.28, V4.31, V4.32, V4.34, V4.35, V4.37.
- [x] Fase 28 — Salto (`ProxyJump`) e túnel de `banco` por `servidor`, com nível
  combinado (D19); logs da suíte do M4 sem segredo. 2ª parte do V4.2 (D65 e) no
  contêiner de `make teste-docker-plataforma`: CLI sem `modo-teste`, `wardd` copiado e
  falso não-dumpable do uid 1000, `bind` em `/run/ward/1000/` recusado.
  V4.2, V4.25, V4.27, V4.29, V4.30, V4.33.

- [x] Fase 27a — Correção do achado alto da auditoria do M3: o token do "Conectar" nunca
  vai em argv. O app entrega o token ao `ward ssh` por um fd herdado (ou outro canal que
  nenhum outro processo do mesmo uid leia), o `x-terminal-emulator` não o recebe em
  argv/ambiente, e o `ward ssh` lê o token desse canal; `--token` em argv é recusado.
  Teste de ataque: leitor de `/proc/*/cmdline` e `/proc/*/environ` em laço durante o
  "Conectar" não acha o token. V5.24, S9, S10.
- [ ] Fase 26a — Ambiente do `executar` por lista fixa (decisão do usuário, achado médio
  da auditoria do M3): o daemon monta o ambiente do comando do zero — `PATH` fixo
  `/usr/local/bin:/usr/bin:/bin`, só variáveis de uma lista permitida (`HOME`, `USER`,
  `LOGNAME`, `LANG`, `LC_*`, `TERM`, `TZ`, `COLORTERM`, `NO_COLOR` e as que a entrega da
  credencial define) e nada da CLI fora dela (`BASH_ENV`, `ENV`, `NODE_OPTIONS`,
  `PYTHONPATH`, `PERL5LIB`, `GIT_*`, `SSH_*`, `PG*` não entregues, `LD_*` etc. somem);
  o comando com caminho relativo é resolvido pelo `PATH` fixo (nunca pelo da CLI) e o
  diretório de trabalho continua o da CLI, mostrado na janela de aprovação. Mesmo
  tratamento no `ward ssh` e no salto/túnel. Testes de ataque: `PATH` com pasta do
  atacante na frente, `BASH_ENV`, `NODE_OPTIONS=--require`, `GIT_SSH_COMMAND`,
  `PYTHONSTARTUP`, `PGSERVICEFILE`. Atualizar `teste-cli` e o roteiro da skill (comando
  fora do `PATH` fixo vai com caminho absoluto). V4.3–V4.8, V4.36.

## Milestone 5 — App desktop
Contrato: V5.1–V5.35

- [x] Fase 29 — Esqueleto Tauri + Svelte: capabilities só com os comandos próprios,
  CSP estrita, sem conteúdo remoto, devtools fora do release; cliente do socket no lado
  Rust; "copiar" resolvido no Rust (D4); tokens e componentes do M0 em Svelte; ponte de
  teste só com `modo-teste` (D7). V5.1, V5.2, V5.4–V5.6. O lado do app de D47 não foi
  feito nesta fase (handoff 29) e foi para a Fase 29a; o caminho e a conferência do
  socket de D46 no `daemon.rs`, para a 21b.
  Notas do handoff 29: o `make app-check` precisa de `libwebkit2gtk-4.1-dev` (R32); os
  advisories RUSTSEC-2024-0370 e RUSTSEC-2024-0429 ficam ignorados só no `deny.toml` do
  app (RA7, provisório, vai ao usuário).
- [x] Fase 29a — App com D47 e D57 (depende das Fases 21b e 29; vem antes da 30; pode
  rodar ao lado da 22, porque não toca em `crates/wardd` nem no protocolo). Arquivos: o
  `main` do `app/src-tauri`, um módulo de partida no `app/nucleo` e, em
  `crates/ward-plataforma`, o lançador e o `processo.rs`.
  - `ward-plataforma`: tira o ramo do `systemd --user` do
    `ProcessoPar::classificar` (o pai do app só pode ser o daemon) e cria o lançador
    de D57 (`fork` + `execve` do `ward-app` instalado, ambiente montado do zero com
    os valores fixos, um só `wayland-<n>`, sem `DISPLAY`, fds fechados), que a Fase 23
    chama pela operação `abrir_app`;
  - app: conecta ao socket antes do GTK, larga o grupo (`setresgid` para o gid real)
    e chama `PR_SET_DUMPABLE=0` de novo; confere o ambiente recebido contra D57 antes
    do GTK e sai se não bate; fd do socket com `CLOEXEC`; se a conexão cair e o
    `PPid` ainda for o daemon, se executa de novo; com outro pai, não usa o papel e
    sai com mensagem (o pedido `abrir_app` ao daemon entra na Fase 23);
  - confere com o WebKitGTK real que o ambiente de D57 basta (módulos do GTK,
    GStreamer, IBus) e, se algo de fora do root for carregado pelo ambiente, fixa a
    variável que desliga a fonte (D48) e a acrescenta à lista de D57;
  - confere, só lendo, quantos `wayland-*` o COSMIC deste host cria em `/run/user/<uid>`
    e registra no handoff (se forem dois, o "um só" de D57 volta ao planner);
  - roda o `make app-check` e confirma o lock do Tauri corrigido na 21b (D60, R32).
  Testes de ataque no contêiner da 21b (binário setgid de teste), incluindo o pai que
  imita o `systemd --user`. Exige `libwebkit2gtk-4.1-dev` (R32). V3.5 (forma de D47),
  V3.24, V5.31.
- [x] Fase 29b — (Depende da 23a: as duas mexem no lançador.) Pendências da 29a (D64).
  Arquivos: `crates/ward-plataforma` (`ambiente_do_app` e `conferir_ambiente_do_app`),
  `app/src-tauri/deny.toml` e o teste com o WebKitGTK real.
  - `CUDA_MPS_PIPE_DIRECTORY=/proc/ward-sem-mps` no ambiente de D57, conferido pelo app;
  - `strace -f` do `ward-app` com o ambiente de D57: nada em `/tmp/nvidia-mps`, e a
    lista de `open`/`connect` em pastas graváveis fora do root vai ao handoff; canal
    novo volta ao planner;
  - RUSTSEC-2024-0429: sai do `deny.toml` do app se o `cargo audit` em
    `app/src-tauri/` não a achar;
  - `make app-check` limpo. V5.30, V5.32.
- [x] Fase 30 — **(UI)** Primeiro uso (o PIN vai para a seção de configuração do corpo
  v2 como hash, D30). V5.7, V5.25, V5.26.
- [x] Fase 31 — **(UI)** Destravar, inclusive o estado "Pedido do agente". Consome
  `pedido_de_destrave` da fila da 23a e o leva à tela por evento do Tauri (D65 a). V5.8.
- [x] Fase 32 — **(UI)** Credenciais (vazia, cheia, filtrada, busca). V5.3, V5.9.
- [x] Fase 33 — **(UI)** Formulário, parte 1: `login`, `api`, `aws`, `smtp`,
  `nota segura`, `genérico`, com campo secreto (revelar 30 s, copiar com limpeza em
  30 s), campos extras, TOTP e gerador no formulário (símbolos de D35; `otpauth://`
  fora de SHA1/6/30 recusado, D35). V5.3, V5.5, V5.10, V5.11, V5.13, V5.14.
  Nota do planner (D65, 2026-11-09), para a fase em andamento: "revelar" usa a operação
  `revelar` que já existe (só papel app; nenhuma operação nova): `cliente::revelar`,
  resposta no daemon falso, comando `revelar_campo` na capability, valor na webview só
  no clique e fora dela em 30 s, ao esconder ou ao sair (padrão `Revelada` da 37, sem
  ligaduras); vale também para o `Campo secreto` do detalhe de Credenciais. O daemon
  real atende na 22c. Gerador no formulário: tamanho 8–64 e limiares da 37; símbolos de
  `ward_core::gerador::SIMBOLOS`, não do texto do FigCraft. Datas do rodapé ficam para
  a 22d.
- [x] Fase 34 — **(UI)** Formulário, parte 2: `banco`, `servidor` (com "Conectar" e
  token de uso único, D3), `ftp/sftp`, anexos (aviso ao escolher arquivo acima de
  256 KiB ou soma acima disso, arquivo vazio recusado, recusa do orçamento da máscara
  no campo, números de `ward_core::limites`; D29, D32); base do `banco` com o formato
  de D33. V5.10, V5.12, V5.24.
- [x] Fase 35 — **(UI)** Nível de acesso com PIN. V5.15.
- [x] Fase 36 — **(UI)** Janela de aprovação em primeiro plano, aberta pelo daemon, com
  os cinco estados do M0; comando e cadeia exibidos pela função de D51, nunca como HTML.
  Consome `pedido_de_aprovacao` da fila da 23a (D65 a). V5.16–V5.20, V5.29.
- [x] Fase 37 — **(UI)** Gerador de senha avulso (símbolos de D35). V5.21.
- [x] Fase 37a — **(DESIGN)** Correções do design (D65 f e design pendente; pode rodar a
  qualquer momento, só mexe no `Ward.figcraft` e em `docs/mission/design-nodes.md`):
  descrição de "Símbolos" com o conjunto de D35 no avulso e no popover do formulário;
  frame `Credenciais / Sem resultado`; cartão de nível bloqueado no detalhe; contagens
  da fixture nos frames de Credenciais. Inspirações só leitura; termina com
  `save_file` e PNG das telas ao usuário. V5.35.
- [x] Fase 38 — **(UI)** Configurações (PIN, senha mestra, tempo de trava gravado na
  configuração do cofre, D30; backup externo só exibido desligado). V5.22.
- [x] Fase 39 — **(UI)** Bandeja; `docs/mission/roteiro-usuario.md` da janela nativa;
  varredura de logs do app. V5.23, V5.27, V5.28.

## Milestone 6 — Extensão, `ward login` e `ward playwright`
Contrato: V6.1–V6.16

- [x] Fase 40 — `ward-nm` (native messaging): mensagens com limite, papel extensão no
  daemon (D47; pai Chrome conferido com o Chrome real: lançado
  direto ou por intermediário, navegador dumpable ou não, handoff 21), origem da aba
  comparada no daemon (D9).
  V6.1, V6.2.
- [ ] Fase 41 — **(UI)** Extensão MV3 e popup "Sugestões do domínio": sugere por origem,
  preenche usuário, senha e TOTP só no documento da origem certa, sem
  `externally_connectable`, nada guardado em `chrome.storage`. V6.2, V6.4–V6.7.
- [ ] Fase 42 — **(UI)** Popup "Salvar login novo" e "Cofre travado". V6.8, V6.9.
- [ ] Fase 43 — `ward login`: daemon → `ward-nm` → aba ativa, confere a origem,
  preenche, envia e limpa os campos, com nível e aprovação. V6.3, V6.10, V6.11, V6.15.
- [x] Fase 44 — `ward playwright`: Chrome headless com `--remote-debugging-pipe` e perfil
  temporário (D6), storage state 0600, limpeza ao travar e na partida; logs sem
  segredo. V6.12–V6.16.

## Milestone 7 — Backup
Contrato: V7.1–V7.11

- [x] Fase 45 — Backup local automático (30 versões, mesmo formato, gravação atômica,
  dentro do `ward-core` pelo `gravar` interno, D27) e restaurar no daemon, só pelo papel
  app (D17); a versão restaurada é selada com `geracao` = maior vista + 1 e o arquivo
  de estado é atualizado (D39). V7.1–V7.3, V7.8, V7.11.
- [x] Fase 46 — Backup externo em pendrive por UUID via udisks2, desligado por padrão;
  ligado/desligado e UUID ficam na configuração do cofre (D30). V7.4–V7.6, V7.8.
- [ ] Fase 47 — **(UI)** Restaurar backup. V7.7.
- [x] Fase 48 — **(UI)** Configurações › backup externo. V7.9.
- [x] Fase 49 — **(UI)** Kit de emergência para imprimir. V7.10.

## Milestone 8 — Skill `ward`
Contrato: V8.1–V8.4

- [ ] Fase 50 — Skill `ward` do zero em `skill/ward/` (fluxo, comandos, SSH com
  `~/.ssh` fica de fora; nunca imprimir anexo nem variável de credencial em nenhuma
  codificação, RA3) e o trecho de deny para `~/.claude/settings.json`; a instalação
  no `~/.claude` é feita pelo usuário no M10. V8.1–V8.4.

## Milestone 9 — Instalação
Contrato: V9.1–V9.11

- [ ] Fase 51 — `make install`/`uninstall` com `DESTDIR` e `PREFIX=/usr/local` (o
  `sudo` uma vez), units `wardd@.socket`/`wardd@.service` do sistema, `/run/ward/` por
  `tmpfiles.d` e `systemctl enable --now wardd@<uid>.socket` (D46), grupos
  `ward-app`/`ward-nm` e binários 2755 (D47; grupos com `groupadd --system`, gid < 1000,
  fora das faixas do `/etc/subgid`, sem membros, senha travada `!` no `gshadow`, e
  recusa de grupo existente fora disso, D60), units sem `NoNewPrivileges=` nem opção
  que o implique (D57), `LimitMEMLOCK=infinity` (D62), build de release por pacote
  (`-p`, sem `modo-teste`, D63), manifesto de native messaging, política
  do Chrome com force-install e update local, recusa de artefato `modo-teste`. V9.1–V9.4, V9.6,
  V9.7, V9.10, V9.11.
- [ ] Fase 52 — `.deb` por `dpkg-deb` a partir do staging (dependência `sshpass`), teste
  num contêiner `ubuntu:24.04` e roteiro de instalação para o usuário (política do
  Chrome, systemd, `~/.ssh` intacto). V9.5, V9.8, V9.9.

## Milestone 10 — Migração e saída do KeePassXC
Contrato: V10.1–V10.4

- [ ] Fase 53 — `docs/migracao.md`: guia da etapa final, executado pelo usuário, com
  conferência e volta atrás em cada passo (chaves pessoais para `~/.ssh`, chaves de
  servidor para entradas `servidor`, gnome-keyring e "Chrome Safe Storage", desfazer
  `montagem.md`, senhas do Chrome, troca da skill, desinstalar o KeePassXC por último).
  A fase não executa nenhum passo. V10.1–V10.4.
