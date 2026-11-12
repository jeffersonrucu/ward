# Fase 6 — (DESIGN) Formulários, parte 2: smtp, ftp/sftp, nota segura, genérico

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), em
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo e com `ids`
  explícitos em todo comando, em paralelo com as Fases 5 e 8. Antes, li a skill `figcraft`,
  o PRD, o plano, o contrato, o `design-nodes.md`, os handoffs 4a–4g e o escopo ("Tipos
  prontos", "App desktop", "Missão 0"). Fiz 9 `save_file` no mesmo caminho e conferi o
  arquivo salvo lendo o `document.json` do disco.
- **Oito telas** no Layout (página `2796`, modo `Escuro` herdado da página), no padrão de
  três colunas da 4f/Fase 5: navegação, lista e formulário no painel de detalhe. Usei como
  modelo os frames da Fase 5 (`Formulário / login / Novo` `7450` e `/ Edição` `6926`),
  duplicando-os sem alterar os originais. Só há instâncias de `Campo`, `Campo secreto`,
  `Select`, `Botão`, `Badge *`, `Item de lista`, `Cabeçalho de grupo`, `Título de seção`,
  `Campo extra`, `Anexo` e `Campo nota segura`. Todos os campos da tabela "Tipos prontos
  (v1)" estão lá, mais `grupo` (o autocompletar aparece aberto em `smtp / Novo`) e
  `ambiente`, que são obrigatórios, além de `projeto`, tags, campos extras com o
  interruptor "segredo", anexos, nível de acesso e a descrição de entrega (D10) de cada
  tipo.
  - `smtp`: host e porta, usuário e segurança (`starttls`), senha (`Campo secreto`; no
    Novo, com gerar). A Edição é a `beta-smtp-stage`, com o extra `TIMEOUT`.
  - `ftp/sftp`: protocolo, host e porta, usuário e autenticação. A Edição é a
    `beta-sftp-dev` (sftp, autenticação **senha**, senha mascarada, extra `PASTA_REMOTA`).
    O Novo mostra a autenticação **chave**, com `Chave privada` e `Passphrase (opcional)`
    como `Campo secreto` vazios, no mesmo arranjo do `servidor` da Fase 5.
  - `nota segura`: seção "Conteúdo" ("Sem entrega: o ward run recusa este tipo...") com o
    componente novo `Campo nota segura`. Na Edição ele fica oculto (texto só em `••••`, com
    revelar e copiar); no Novo, vazio, com placeholder. Extras e anexos dizem que ficam com
    a nota, sem entrega.
  - `genérico`: só campos extras e anexos. A Edição é a `gama-generico`, com `REGIAO` /
    `r-01` (sem segredo), `CHAVE_LICENCA` (segredo, `••••••••`) e o anexo `app.env`
    (`WARD_ARQUIVO_APP_ENV`, com Remover e "Adicionar arquivo": adicionar, listar e
    remover, V0.9). O Novo tem os mesmos dois extras e os anexos vazios.
- **Listas da Edição** com os grupos certos do cofre de teste. A `beta` (marcador
  `grupo/2/cor`) tem 4 entradas: aws, github-api, sftp e smtp. A `gama` (`grupo/3/cor`) tem
  3: `gama-bloqueada`, `gama-generico` e `gama-nota`. Em cada tela, a entrada editada aparece
  selecionada. As telas Novo mantêm a lista `acme` do modelo, sem seleção, como a Fase 5.
- **Componente novo** em `Seção / Nota segura` (Design System, x = 4600, y = 0):
  `Campo nota segura` (set) com `Estado=oculto` e `Estado=vazio`, e as propriedades
  `Rótulo`, `Ajuda` e `Resumo`. Tudo está ligado a variables e text styles: entrada com
  `superficie/campo`, `borda/sutil`, `raio/controle`, `espaco/16`/`12`; máscara em
  `Texto/Corpo`; revelar 32 (`controle/sm`) e `Ação de campo` Copiar. Precisei dele porque
  o `Campo secreto` só tem uma linha, e a nota é um texto longo e secreto.
- **Respiro:** reaproveitei os paddings dos componentes (cartões 16–20, painéis 24). Conferi
  por screenshot de cada frame em 1x e do painel de detalhe com zoom. Os textos longos de
  extras foram encurtados para a variável `WARD_*` não quebrar.

### Ids para design-nodes

Frames de tela (página Layout `2796`; Novo em y = 7200, Edição em y = 8800; colunas x = 0,
1360, 2720, 4080):

| Frame | Id | Tamanho | Principais |
| --- | --- | --- | --- |
| `Formulário / smtp / Novo` | `13867` | 1280×1370 | detalhe `14065`, corpo `14086`; Grupo foco `14103` + aberto `14115` ("be" → beta / Criar grupo); `linha host` `14201` (Host `14285`, Porta `14293`); `linha usuário` `14167` (Usuário `14168`, Segurança `14301`); Senha com gerar `14176` |
| `Formulário / ftp/sftp / Novo` | `14743` | 1280×1462 | corpo `14962`; Protocolo `15236`; Host `15036`, Porta `15044`; Usuário `15053`, Autenticação "Chave" `15061`; `linha chave` `15210` (Chave privada `15067`, Passphrase `15211`) |
| `Formulário / nota segura / Novo` | `15399` | 1280×1240 | corpo `15618`; "Conteúdo" `15683`; `Campo nota segura` vazio `15828` |
| `Formulário / genérico / Novo` | `15845` | 1280×1296 | corpo `16064`; Campos extras `16135`; `Campo extra` não `16290`, sim `16304`; anexos vazio `16153` |
| `Formulário / smtp / Edição` | `9454` | 1280×1392 | lista beta `9514` (selecionado `9614`); detalhe `9652`, corpo `9673`; `linha host` `9794` (Host `9742`, Porta `10833`); `linha usuário` `9755` (Usuário `9756`, Segurança `10846`); Senha `9769`; extra `9843`; nível `9867` |
| `Formulário / ftp/sftp / Edição` | `10895` | 1280×1488 | lista beta `10955` (selecionado `10990`); corpo `11047`; Protocolo `11143`; `linha host` `11149` (Host `11116`, Porta `11658`); `linha usuário` `11129` (Usuário `11150`, Autenticação "Senha" `11652`); Senha `11163`; extra `11196` |
| `Formulário / nota segura / Edição` | `12683` | 1280×1296 | lista gama `12743` (selecionado `12793`); corpo `12835`; "Conteúdo" `12896`; `Campo nota segura` oculto `13122`; extras vazio `13139`; anexos vazio `13006` |
| `Formulário / genérico / Edição` | `13459` | 1280×1296 | lista gama (selecionado `13541`); corpo `13600`; extras `13739` (REGIAO), `13753` (CHAVE_LICENCA, segredo); `Anexo` app.env `13767` |

Design System (página `2`):

| Nó | Id | Notas |
| --- | --- | --- |
| `Seção / Nota segura` | `12580` | x = 4600, y = 0; título `12581` (`Título/Seção`), descrição `12582` (`Texto/Pequeno`, largura `largura/formulario`); raio `raio/painel`, fundo `fundo/sutil`, padding `espaco/32`, gap `espaco/16` |
| `Campo nota segura` (set) | `12613` | raio `raio/cartao`, padding `espaco/32`, gap `espaco/24` |
| `Estado=oculto` | `12583` | largura mínima `largura/campo`, gap `espaco/8`; rótulo `12584`, entrada `12585`, máscara `12587`, rodapé `12588` (resumo `12589`, ações `12594`: revelar `12595`/ícone `12596`, `Ação de campo` `12599`), ajuda `12586` |
| `Estado=vazio` | `12614` | igual, com placeholder `12617` (`texto/secundario`) e ações ocultas (`12620`) |

## Segurança
- Invariantes: a fase é só de design e não toca código. O desenho segue S1, S11, R6 e
  V0.20:
  - todo valor secreto aparece só mascarado: senha do smtp e do sftp, chave privada,
    passphrase, extra `CHAVE_LICENCA` e o texto da nota (`••••`). Nos Novo, os campos
    secretos ficam vazios. Nenhum valor revelado entra em tela;
  - o conteúdo do anexo nunca aparece. O `Anexo` mostra só nome, tamanho e a variável;
  - `Copiar` aparece só em campos não secretos (host, usuário) ou dentro do `Campo secreto`
    e do `Campo nota segura`, cujo copiar é resolvido no app (D4). A ajuda diz que revelar e
    copiar não pedem PIN, como nas Fases 4f e 5;
  - a nota segura diz em dois lugares que não tem entrega e que o Claude nunca recebe o
    texto (D10, "tipo sem entrega");
  - o nível de acesso das Edições é "Livre", como no cofre de teste (`beta-smtp-stage`,
    `beta-sftp-dev`, `gama-nota`, `gama-generico`), com "Mudar nível" pedindo PIN. Nos
    Novo fica "Com aprovação" (toda entrada nova começa assim).
- Exemplos só falsos, tirados do cofre de teste do contrato (`smtp.teste.invalid`,
  `127.0.0.1:2222`, `sftpuser`, `mailer`, `r-01`, `app.env`). Os valores secretos
  (`falso-SMTP-01` etc.) **não** foram escritos em nenhum texto. Fiz uma varredura no
  `.figcraft` salvo e não encontrei `falso-SMTP`, `falso-SFTP`, `falso-nota`,
  `falso-licenca`, `falso-anexo` nem semente TOTP nos 8 frames. A propriedade oculta
  `Valor revelado` do `Campo secreto` ficou com o valor que veio do modelo
  (`falso-exemplo-9f3k`, obviamente falso).
- Não toquei em cofre, credencial, `~/.ssh` ou `~/.aws`, e não usei o Figma.
- Testes de ataque: não se aplicam (o M0 não tem asserção (SEG)).
- Dependências novas: nenhuma. Os scripts de leitura do `.figcraft` (só biblioteca padrão
  do Python) ficaram no scratchpad.

## Ficou de fora
- Ids no `docs/mission/design-nodes.md`: estão acima, como pediu o orchestrator.
- Variante "revelado" do `Campo nota segura`: não fiz para não pôr nem um texto falso
  revelado em tela. A Fase 33 (UI) define o comportamento de revelar por 30 s.
- O `ftp/sftp` Edição com chave e o Novo com senha: mostrei uma autenticação em cada
  frame (senha na Edição, como o `beta-sftp-dev`; chave no Novo).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `get_node`, `inspect_document`, `screenshot` (8 frames em 1x, detalhe com zoom, seção nova) | ok |
| `edit.duplicate`, `object.rename`, `instance.create`/`setProperties`/`swap`, `node.set`/`reparent`/`reorder`, `edit.delete`, `variable.bind`, `style.apply`, `component.addProperty`/`combineVariants`/`addVariant` | ok |
| `python3 -I scripts/tree.py` e auditoria no `.figcraft` salvo (ids, nomes, posições, segredos, text styles, ligações) | 0 pendências |
| `save_file` (9×, mesmo caminho) | ok |

## Problemas descobertos
- **Modelo da Fase 5 (`Formulário / login / Novo` `7450`):** a `Navegação lateral`
  (`7451`) e a `lista` (`7509`) têm altura fixa (1358 e 1288), e o corpo da `colunas` tem
  1266. Ao mudar a altura do frame, elas não acompanham (a lista passa 22 do painel). Nas
  minhas cópias, pus as duas em `fill`. Não alterei o `7450`; vale a Fase 5 ou a 10
  ajustar.
- **Grupo do Novo:** a Fase 5 deixa o `Select` de Grupo aberto com o autocompletar só no
  `login / Novo`, e eu fiz o mesmo só no `smtp / Novo`. Nos outros Novo, o grupo aparece
  preenchido.
- **`Campo extra` › `Segredo=sim`:** a máscara do valor é texto fixo, e a propriedade
  `Valor` não muda o que aparece. Isso é bom para a segurança, mas o nome da propriedade
  engana. Mesmo assim, defini `Valor` como `••••••••`. Fica para a Fase 5 ou a 10 decidir.
- **Componente novo fora da `Seção / Formulário`:** o `Campo nota segura` ficou numa seção
  própria (x = 4600), como mandava a regra de trabalho em paralelo. A Fase 10 pode
  movê-lo para junto dos componentes de formulário.
- **Larguras do mestre sem token** (como na 4f): o `Campo nota segura` mestre tem 480 de
  largura de apresentação; só o mínimo (`largura/campo`) é ligado. Nas telas ele fica em
  `fill`.
- **Altura dos frames sem token**, como já registrado na 4f: 1240–1488, porque cada
  formulário aparece inteiro.
- **"cliente" x "grupo"** nos documentos continua como registrado na 4f. As telas usam
  "grupo".
- JetBrains Mono continua sem estar instalada (fallback), mas esta fase não usa mono.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (8 telas no Layout; `Seção / Nota segura` com o set `Campo nota segura` na Design System) |
| `docs/mission/handoffs/phase-6.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado; segredos do contrato não escritos)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots 1x e com zoom; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase (componentes compartilhados não alterados; componente novo em seção própria)
