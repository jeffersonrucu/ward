# Fase 9a — (DESIGN) Fluxo "Nova credencial"

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), em
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo. Usei `ids`
  explícitos em todo comando, porque a Fase 10b trabalhava no mesmo documento ao mesmo
  tempo. Antes, li a skill `figcraft`, o PRD, o plano (com as regras do cabeçalho), o
  contrato, o `design-nodes.md` e os handoffs 4f, 5 e 6. Fiz 7 `save_file` no mesmo caminho
  e conferi o arquivo salvo lendo o `document.json` do disco.
- **Componente novo `Cartão de tipo`** (set `17607`) numa seção minha, `Seção / Nova
  credencial` (`17583`, Design System, x = 14000, y = 0). Ele tem as variantes
  `Estado=padrão | hover | selecionado` e as propriedades `Nome` (texto), `Descrição`
  (texto) e `Ícone` (troca, padrão `Ícone/tipo-login`). Todos os valores estão ligados a
  variables:
  - cartão: `superficie/padrao`, borda `borda/sutil` × `linha/fina`, raio `raio/cartao`,
    padding `espaco/20` (vertical) e `espaco/16` (horizontal), gap `espaco/12`;
  - avatar 40 (`controle/md`): `superficie/elevada`, `borda/padrao`, `raio/controle`, com o
    ícone em 20 (`icone/md`);
  - textos: nome em `Texto/Corpo forte` (`texto/primario`) e descrição em `Texto/Pequeno`
    (`texto/secundario`), gap `espaco/4`;
  - hover: `superficie/elevada` com `borda/padrao`, e o avatar com `borda/forte`;
  - selecionado: `superficie/selecionada` com `borda/foco`, e o avatar com
    `primaria/sutil` e `borda/foco`.
- **`Nova credencial / Escolher tipo`** (`17657`, Layout, x = 7200, y = 10400, 1280×800).
  É uma cópia de `Credenciais / Cheia` (`4063`), sem alterar o original, e mantém as três
  colunas.
  - Na lista, nenhum item fica selecionado.
  - No cabeçalho do painel de detalhe: avatar com `Ícone/mais`, o título "Nova
    credencial", o subtítulo "Escolha o tipo. Ele não muda depois de salvo." e um `Botão`
    secundário "Cancelar" no lugar de "Editar".
  - No corpo, uma grade de 3×3 (`tipos` `17975`, layout grid com 3 colunas `1fr`, gap
    `espaco/12` nos dois eixos) com os 9 `Cartão de tipo`, cada um com ícone, nome e uma
    linha do que guarda:
    - login: "URL, usuário, senha e TOTP";
    - banco: "Motor, host, porta, base e senha";
    - servidor: "SSH com senha ou chave";
    - api: "URL base, token e variável";
    - aws: "Access key, secret, região e role";
    - smtp: "Envio de e-mail com usuário e senha";
    - ftp/sftp: "Arquivos por senha ou chave";
    - nota segura: "Texto secreto, sem entrega";
    - genérico: "Só campos extras, do seu jeito".
  - Embaixo da grade, a nota "Toda entrada nova começa com aprovação. Mudar o nível depois
    pede o PIN."
  - Tirei o rodapé (metadados, Duplicar, Excluir), que não faz sentido antes de a entrada
    existir.
- **`Nova credencial / Tipo escolhido`** (`18055`, x = 8560, y = 10400, 1280×800). É a mesma
  tela com o cartão `login` em `Estado=selecionado`. O avatar do cabeçalho passa a mostrar o
  ícone do login, e o subtítulo diz "login escolhido. Abrindo o formulário…".
- **Transição para o formulário (protótipo):**
  - `Tipo escolhido` tem uma interação "afterDelay" de 400 ms que navega para `Formulário /
    login / Novo` (`7450`) com dissolve de 250 ms.
  - O cartão `login` selecionado também abre o `7450` com clique ou Enter.
  - Em `Escolher tipo`, o cartão `login` leva a `Tipo escolhido`. Os outros 8 cartões abrem
    direto o `Formulário / <tipo> / Novo` correspondente: banco `8537`, servidor `9900`, api
    `11272`, aws `12170`, smtp `13867`, ftp/sftp `14743`, nota segura `15399` e genérico
    `15845`.
  - "Cancelar" volta a `Credenciais / Cheia` (`4063`).
  - Criei o flow "Nova credencial", que começa em `4063`.
- **O botão `Nova credencial` de todas as telas leva ao fluxo.** Encontrei 26 instâncias com
  o rótulo "Nova credencial" fora dos meus frames e pus em cada uma um clique que navega
  para `Escolher tipo` (dissolve de 200 ms). Nenhuma delas tinha interação antes. Estão em:
  - Credenciais: Cheia `4142`, Filtrada `4629`, Vazia `4999` e Filtros `5449`;
  - Nível de acesso: `5805`, `6184` e `6563`;
  - os 18 formulários: `7116`, `7644`, `8102`, `8727`, `9171`, `9648`, `10090`, `10583`,
    `11022`, `11452`, `11885`, `12350`, `12810`, `13575`, `14061`, `14937`, `15593` e
    `16039`;
  - `Gerador de senha / No formulário`: `14501`.
- **Conferência:**
  - screenshot dos dois frames em 1x e da grade e dos cartões em 2x: o texto não encosta na
    borda, os cartões têm padding de 20 em cima e embaixo, e todos ficam com 154 de altura
    (descrição em duas linhas);
  - a auditoria (`f5/audit.py`) no `.figcraft` salvo, na seção e nos dois frames, não achou
    nenhum preenchimento, borda, raio, gap ou padding solto, e nenhum texto sem text style.
    A única exceção é a borda tracejada roxa do set, que é o padrão de todo set;
  - os ids desta fase existem no arquivo salvo, e a seção e os frames não se sobrepõem aos
    da 10b (`Seção / Campo senha` em x = 16000).

### Ids para design-nodes

Design System (página `2`):

| Nó | Id | Notas |
| --- | --- | --- |
| `Seção / Nova credencial` | `17583` | x = 14000, y = 0; título `17584` (`Título/Seção`), descrição `17585` (`Texto/Pequeno`, largura `largura/formulario`); `fundo/sutil`, `raio/painel`, padding `espaco/32`, gap `espaco/16` |
| `Cartão de tipo` (set) | `17607` | padding `espaco/32`, gap `espaco/24`, raio `raio/cartao`; props `Nome`, `Descrição`, `Ícone` (troca) |
| `Estado=padrão` | `17586` | avatar `17599` (ícone `17604`), textos `17600` (nome `17601`, descrição `17603`) |
| `Estado=hover` | `17608` | avatar `17609` (ícone `17610`), nome `17614`, descrição `17615` |
| `Estado=selecionado` | `17616` | avatar `17617` (ícone `17618`), nome `17622`, descrição `17623` |

Frames no Layout (`2796`):

| Frame | Id | Principais |
| --- | --- | --- |
| `Nova credencial / Escolher tipo` | `17657` | nav `17658`; lista `17716` (itens `17717`, nenhum selecionado; `Nova credencial` `17847`); detalhe `17851`: cabeçalho `17852` (avatar `17853` com `Ícone/mais` `17854`, título `17858`, subtítulo `17867`, "Cancelar" `17868`); corpo `17872`: grade `tipos` `17975` com `Cartão de tipo` login `17976`, banco `17984`, servidor `17992`, api `18000`, aws `18008`, smtp `18016`, ftp/sftp `18024`, nota segura `18032`, genérico `18040`; nota `18051` |
| `Nova credencial / Tipo escolhido` | `18055` | detalhe `18249`: cabeçalho `18250` (ícone login `18252`, subtítulo `18257`, "Cancelar" `18258`); corpo `18262`: grade `18263` com login **selecionado** `18264` (os outros 8: `18272` `18280` `18289` `18296` `18303` `18311` `18318` `18326`); nota `18334`; `Nova credencial` da lista `18245` |

Flow do protótipo: "Nova credencial" (início `4063`).

## Segurança
- A fase é só de design e não toca código. O fluxo não mostra nenhum valor:
  - as telas de escolha não têm campo secreto nem exemplo de segredo, e os nomes da lista
    são as entradas falsas do cofre de teste (V0.20);
  - **S9:** a nota diz que toda entrada nova começa com aprovação e que mudar o nível pede o
    PIN, igual aos formulários Novo das Fases 5 e 6. A escolha do tipo não oferece "livre";
  - **D10 (nota segura sem entrega):** a descrição do cartão diz "sem entrega";
  - o tipo é escolhido antes do formulário e "não muda depois de salvo", como na Edição das
    Fases 5 e 6, em que o Tipo fica desabilitado.
- Não toquei em cofre, credencial, `~/.ssh`, `~/.aws` nem no Figma.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. Os scripts de leitura do `.figcraft` usam só a biblioteca
  padrão do Python e ficam no scratchpad.

## Ficou de fora
- **Busca na escolha do tipo** (opcional no plano): com 9 tipos numa grade de 3×3, tudo
  cabe sem rolar em 800, e a busca só tomaria espaço. A grade pode ser percorrida pelo
  teclado (setas e Enter, com o estado `selecionado` como foco).
- **Ids no `design-nodes.md`:** estão acima, como pediu o orchestrator. A 10c consolida.
- **"Cancelar" dos formulários Novo voltando à escolha do tipo:** esses frames são das
  Fases 5 e 6, e não mudei a interação deles (hoje eles não têm nenhuma).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `get_node`, `inspect_document`, `screenshot` (frames em 1x, grade e cartões em 2x) | ok |
| `create_node`, `instance.create`/`setProperties`/`swap`, `component.addProperty`/`editProperty`/`combineVariants`/`addVariant`, `variable.bind`, `style.apply`, `node.set`, `edit.duplicate`/`delete`, `proto.interaction.add`, `proto.flow.add` | ok |
| `python3 -I f5/audit.py` e `f5/tree.py` no `.figcraft` salvo | 0 pendências (só a borda padrão do set) |
| `save_file` (7×, mesmo caminho) | ok |

## Problemas descobertos
- **Interações em frames de outras fases:** para cumprir "o botão Nova credencial das
  telas leva a esse fluxo", acrescentei uma interação de clique nas 26 instâncias de
  `Nova credencial` dos frames das Fases 4f, 5, 6 e 8. É só protótipo: não mudei
  propriedade, override, layout nem componente. Para desfazer, basta rodar
  `proto.interaction.clear` nos ids listados.
- **Cor do ícone no cartão selecionado:** o ícone continua em `icone/padrao`. Se eu
  mudasse a cor por override, ela se perderia ao trocar o `Ícone`, que é o mesmo problema
  da `Navegação lateral` registrado na 10c. Vale resolver junto.
- **Nenhum componente existente foi alterado.** O botão "Cancelar" usa a propriedade
  `Rótulo#10:0` do `Botão` secundário.
- **Altura do cabeçalho do detalhe:** sem os selos, o cabeçalho ficou com 99 em vez de 101,
  porque a linha do subtítulo tem 18 e a dos selos tinha 20. Não dá para notar e não afeta o
  corpo.
- **Ids pulando:** como a 10b criava nodes ao mesmo tempo, os ids desta fase não são
  contíguos. Os ids certos estão nas tabelas acima.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (`Seção / Nova credencial` com o set `Cartão de tipo`; 2 frames no Layout; interações de protótipo e o flow "Nova credencial") |
| `docs/mission/handoffs/phase-9a.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) no M0)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots 1x e 2x; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase (componente novo em seção própria; nenhum componente existente alterado; só interações acrescentadas nos botões das outras telas)
