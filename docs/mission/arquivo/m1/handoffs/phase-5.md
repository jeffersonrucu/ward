# Fase 5 — (DESIGN) Formulários, parte 1: login, banco, servidor, api, aws

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo. Antes, li a
  skill `figcraft`, PRD, plano, contrato, `design-nodes.md`, os handoffs 4a–4g e a
  referência-alvo (só estilo). Fiz `save_file` no mesmo caminho várias vezes (depois de
  cada frame) e no fim; o arquivo salvo foi auditado por script.
- Desde o pedido do coordinator, outros workers editam o mesmo documento. Por isso usei
  sempre `ids` explícitos e não alterei componentes de outras fases. Fiquei em
  y 4000–7199 no Layout e na `Seção / Formulário` na Design System.
- **Componentes novos** em `Seção / Formulário` (`6796`, x = 640, y = 8124), ligados às
  variables (auditoria: 0 valores soltos). Os outros workers vão reutilizar esses
  componentes, então os nomes ficam como estão:
  - `Interruptor` (set `6838`): `Ligado=sim` `6836`, `Ligado=não` `6839`. Pílula de 36×20
    com `primaria/padrao` ou `superficie/hover` e botão de 16.
  - `Título de seção` (`6841`): props `Título`, `Descrição`, `Mostrar descrição`,
    `Mostrar ação`, com um `Botão` fantasma dentro (rótulo e ícone por override).
  - `Anexo` (`6849`): props `Nome` e `Detalhe` (tamanho · `WARD_ARQUIVO_<NOME>`), com
    `Ação de campo` "Remover". O conteúdo do arquivo nunca aparece.
  - `Campo extra` (set `6878`): `Segredo=não` `6861` e `Segredo=sim` `6879`; props
    `Nome`, `Valor` e `Variável`, mais `Interruptor` "segredo" e "Remover". A variante sim
    mostra só `••••••••`, e o valor não é propriedade dela.
  - `Código TOTP` (set `6909`): `Estado=ativo` `6894` (código, contagem em pílula com
    relógio e Copiar; ajuda "Novo código a cada 30 s.") e `Estado=vazio` `6910` ("Sem
    semente"). A semente fica num `Campo secreto` ao lado.
  - Variable nova `largura/interruptor` = 36 (`VariableID:84`, coleção `Largura`).
- **10 frames de formulário** no Layout. O formulário abre no painel de detalhe da tela
  de Credenciais, que continua em três colunas (navegação, lista, detalhe), com a entrada
  editada selecionada na lista. Cada frame tem largura 1280, e a altura mostra o
  formulário inteiro (opção (a) aprovada pelo usuário). No app, o corpo do detalhe rola e
  o cabeçalho e o rodapé ficam fixos. Os frames só usam instâncias para campos, selects,
  campos secretos, botões, badges, itens, cabeçalhos de grupo e as peças novas.
  - **Estrutura comum do corpo:**
    - Nome | Tipo; na Edição, o Tipo fica desabilitado, porque o tipo não muda depois de
      criado.
    - Grupo (select com autocompletar) | Ambiente.
    - Projeto | Tags.
    - `Título de seção` "Acesso" com as variáveis de entrega, seguido dos campos do tipo.
    - "Campos extras" com Adicionar campo.
    - "Anexos" com Adicionar arquivo.
    - Cartão do nível de acesso com "Mudar nível", que pede o PIN.
  - **Rodapé:**
    - Novo: "Obrigatórios: nome, grupo e ambiente.", Cancelar e Salvar (desabilitado).
    - Edição: data da alteração, Cancelar e Salvar alterações.
  - **Toda entrada nova começa com aprovação**, e mudar isso pede o PIN.
  - **Segredos:** só no `Campo secreto` oculto (`••••••••`). No Novo, o campo secreto
    aparece vazio, com revelar/gerar e copiar.

| Frame | Id | Conteúdo |
| --- | --- | --- |
| `Formulário / login / Novo` | `7450` | grupo com **autocompletar aberto** ("ac" → acme, Criar grupo "ac"; `Select` aberto `7691` sobreposto ao `Select` foco `7902`); URL, usuário, senha com gerar, semente TOTP, `Código TOTP` vazio; extras e anexos vazios |
| `Formulário / login / Edição` | `6926` | `acme-wp-dev` (acme, dev, com aprovação): URL, usuário com Copiar, senha e **semente TOTP mascaradas**, **código TOTP 482 913 com contagem 18 s**; extra `SITE_ID`; anexos vazio |
| `Formulário / banco / Novo` | `8537` | motor postgres, host, porta, base, usuário, senha com gerar, túnel SSH "Nenhum" |
| `Formulário / banco / Edição` | `7908` | `acme-pg-tunel` (stage, livre): postgres, host `postgres`, 5432, `app_teste`, `app`, senha mascarada, túnel `acme-ssh-senha-stage` (nível do servidor também vale, D19); extra `PGSSLMODE`; anexo `ca-teste.pem` |
| `Formulário / servidor / Novo` | `9900` | host em foco, porta, usuário, autenticação Senha, senha com gerar, servidor de salto "Nenhum" |
| `Formulário / servidor / Edição` | `8981` | `acme-ssh-chave-prod` (prod, livre): **botão `Conectar` no cabeçalho** (`9192`, D3); host 127.0.0.1, 2223, `deploy`, autenticação Chave, **chave privada e passphrase mascaradas**, salto "Nenhum"; extra `DIR_DEPLOY`; **anexo `acme-vpn.ovpn`** |
| `Formulário / api / Novo` | `11272` | nome em foco, URL base, token, nome da variável (`Ex.: GITLAB_TOKEN`) |
| `Formulário / api / Edição` | `10389` | `beta-github-api` (beta, dev, livre): URL `https://api.github.com`, token mascarado, variável `GITHUB_TOKEN` ("vai nesta variável e em WARD_TOKEN"); extra `GITHUB_ORG` |
| `Formulário / aws / Novo` | `12170` | access key ID, secret access key, região, session token, role ARN |
| `Formulário / aws / Edição` | `11705` | `beta-aws-dev`: `FALSOACCESSKEY01` (identificador, não segredo), secret e session token mascarados, `sa-east-1`, role `arn:aws:iam::000000000000:role/teste`; **extra secreto `EXTERNAL_ID`** (`Segredo=sim`, interruptor ligado) |

- **Grade no Layout:** linha y = 4000 (Novo) e y = 5600 (Edição); colunas x = 0 (login),
  1360 (banco), 2720 (servidor), 4080 (api) e 5440 (aws).
- **Lista em cada frame:**
  - nos frames do grupo beta, o cabeçalho de grupo é "beta · 4 entradas", com marcador
    `grupo/2/cor`;
  - na lista do login, os itens `acme-ssh-*` e `acme-wp-*` substituem os do grupo acme;
  - nos frames de banco e servidor, a lista é a mesma de Credenciais / Cheia.
- **Conferência:**
  - screenshot de cada frame inteiro e zoom de 1,3–2x no corpo do servidor e na seção de
    componentes; o texto não encosta na borda, os cartões, anexos e extras têm padding 16
    e o nível tem 20;
  - auditoria no `.figcraft` salvo (script em
    `/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/f5/audit.py`),
    feita nos 10 frames e na seção: **0** preenchimentos, bordas, espessuras, raios, gaps
    ou paddings soltos e 0 textos sem text style fora de instâncias;
  - varredura dos valores secretos do contrato (senhas, tokens, sementes, passphrase)
    em todos os textos e propriedades do arquivo: **0 ocorrências**;
  - todos os ids desta fase existem no arquivo salvo.

## Segurança
- Invariantes: a fase é só de design e não toca código. O desenho segue o escopo:
  - **S1 e R6:** valores secretos (senha, semente TOTP, token, secret key, session
    token, chave privada, passphrase e extra secreto) aparecem só mascarados. O valor
    revelado nunca entra numa tela (V0.20). Copiar e Revelar ficam só no app, e a ajuda
    diz que eles não pedem PIN, como na 4f.
  - **R8:** a tela mostra o código TOTP e a semente fica mascarada. A ajuda do componente
    diz que o código muda a cada 30 s.
  - **R9:** o anexo mostra só nome, tamanho e `WARD_ARQUIVO_<NOME>`, nunca o conteúdo.
  - **S9:** o nível vem "com aprovação" em toda entrada nova, e mudar o nível pede o PIN.
    Nem na criação dá para escolher "livre" sem PIN.
  - **D3:** `Conectar` só aparece no servidor salvo (Edição). No Novo ele fica oculto,
    porque ainda não existe entrada para o token de uso único.
- Exemplos só falsos, do cofre de teste do contrato. Não toquei em cofre, credencial,
  `~/.ssh`, `~/.aws` nem no Figma.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. Os scripts de leitura usam só a biblioteca padrão do
  Python e ficaram no scratchpad.

## Ficou de fora
- `smtp`, `ftp/sftp`, `nota segura` e `genérico`: são da Fase 6.
- O gerador aberto no formulário é da Fase 8. Aqui só aparece a ação "gerar" no campo de
  senha do Novo.
- O formulário de adicionar campo extra aberto (nome, valor e segredo antes de salvar)
  ficou de fora. O Novo mostra o estado vazio com "Adicionar campo", e a Edição mostra
  os extras salvos com o interruptor "segredo".

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `inspect_document`, `get_node`, `screenshot` (1x a 2x) | ok |
| `instance.create`/`setProperties`/`swap`, `component.addProperty`/`editProperty`/`combineVariants`/`addVariant`, `variable.create`/`bind`, `style.apply`, `node.set`/`reparent`/`reorder`, `edit.duplicate`/`delete` | ok |
| `python3 -I f5/audit.py` (10 frames e a seção) no `.figcraft` salvo | 0 pendências |
| `save_file` (várias vezes, mesmo caminho) | ok |

## Problemas descobertos
- **App travado no meio da fase:** por um tempo, toda chamada voltou "timed out waiting
  for the UI", porque a janela estava sem foco ou fora da tela. Depois a sessão voltou ao
  normal, e encontrei o frame `login / Edição` deslocado para (-97, 4103). Coloquei de
  volta em (0, 5600).
- **Componente compartilhado não alterado:** o `Campo secreto` não tem estado vazio nem
  placeholder. No Novo, deixei a máscara vazia por override no texto `máscara`. Em
  campos de meia largura, usei 8 pontos para a máscara não ser cortada. Um estado
  `vazio` com placeholder no `Campo secreto` cabe na 10b/10c.
- **Componente compartilhado não alterado:** a ajuda do `Campo secreto` é um texto fixo,
  sem propriedade. Em "Chave privada" e "Senha" do servidor, mudei o texto por override.
- **Autocompletar de grupo:** é um `Select` aberto (absoluto) sobre um `Select` em foco.
  Para isso, o corpo do `login / Novo` (`7669`) ficou com `reverseZ`, para o menu ficar
  por cima das linhas de baixo. O `Select` não tem uma variante de combobox com texto
  digitado.
- **Medidas sem token:** a largura do `Select` absoluto (246) e as alturas dos frames de
  formulário (1370–1502), como as telas anteriores, que também não têm token de altura.
- **`Item de lista` escondidos:** nas listas de api e aws, 3 itens do grupo acme ficaram
  ocultos (`visible: false`) em vez de apagados, para mostrar só o grupo beta.
- **Access key ID da AWS:** tratei como identificador (visível, com Copiar), como a
  própria AWS trata. Só o secret e o session token ficam mascarados. Se o usuário
  quiser, dá para mascarar o ID também.

## Ids para design-nodes
- Seção `Seção / Formulário` `6796` (título `6797`, descrição `6798`) na Design System,
  x = 640, y = 8124.
- `Interruptor` set `6838`: sim `6836` (botão `6837`), não `6839` (botão `6840`).
- `Título de seção` `6841`: textos `6842` (título `6847`, descrição `6848`), botão `6843`.
- `Anexo` `6849`: avatar `6850`, textos `6851` (nome `6859`, detalhe `6860`), ação
  `6852`.
- `Campo extra` set `6878`: não `6861`, sim `6879` (máscara `6893`).
- `Código TOTP` set `6909`: ativo `6894` (código `6898`, contagem `6899`, ajuda
  `6897`), vazio `6910` (texto `6925`, ajuda `6924`).
- Variable `largura/interruptor` `VariableID:84` = 36 (`Largura`, WIDTH_HEIGHT).
- Frames no Layout (`2796`):

| Frame | Id | Detalhe | Corpo | Notas |
| --- | --- | --- | --- | --- |
| `Formulário / login / Novo` | `7450` | `7648` | `7669` | `Select` foco `7902` + aberto `7691` (autocompletar); extras vazio `7900`; anexos vazio `7861` |
| `Formulário / login / Edição` | `6926` | `7120` | `7141` | `Código TOTP` `7421`; `Campo extra` `7278`; nível `7221` |
| `Formulário / banco / Novo` | `8537` | `8731` | `8752` | — |
| `Formulário / banco / Edição` | `7908` | `8106` | `8127` | túnel `8475`; anexo `8507` |
| `Formulário / servidor / Novo` | `9900` | `10094` | `10119` | `Conectar` oculto `10113` |
| `Formulário / servidor / Edição` | `8981` | `9175` | `9196` | **`Conectar` `9192`**; anexo `9387`; extra `9365` |
| `Formulário / api / Novo` | `11272` | `11456` | `11475` | — |
| `Formulário / api / Edição` | `10389` | `10587` | `10608` | token `10857`; variável `10882` |
| `Formulário / aws / Novo` | `12170` | `12354` | `12373` | — |
| `Formulário / aws / Edição` | `11705` | `11889` | `11908` | extra secreto `12036` |

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (seção `Seção / Formulário` com 5 componentes, 1 variable, 10 frames de formulário) |
| `docs/mission/handoffs/phase-5.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) no M0)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots com zoom; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase (só a seção nova na Design System; nenhum componente compartilhado alterado)
