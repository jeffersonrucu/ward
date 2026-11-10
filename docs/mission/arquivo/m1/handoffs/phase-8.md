# Fase 8 — (DESIGN) Gerador de senha, Configurações, Restaurar backup e Kit de emergência

## Implementado
- Trabalhei no app FigCraft aberto (`mcp__figcraft-app__*`), no arquivo
  `/home/jeffersonoliveira/documentos/Ward.figcraft`, sem abrir outro arquivo e sem usar
  comandos que dependem da seleção (sempre com `ids` explícitos e sem `page.select`). Os
  workers das Fases 5 e 6 estavam no mesmo documento ao mesmo tempo. Antes de começar, li
  a skill `figcraft`, o PRD, o plano, o contrato, o `design-nodes.md`, os handoffs 4a–4g
  e a referência-alvo (só como estilo). Fiz 12 `save_file` no mesmo caminho.
- **Seção nova na Design System:** `Seção / Gerador e configurações` (`9453`, x = 5600,
  y = 0), com fundo `fundo/sutil`, raio `raio/painel`, padding `espaco/32` e gap
  `espaco/16`. Os componentes ficam todos nela:
  - `Controle deslizante` (set `12604`), variante `Posição` = 25% `9896`, 50% `12605` e
    75% `12609`. É uma grade de 3 colunas (`preenchido`, alça de 20, `restante`) com
    trilhas `fr`, para a proporção valer em qualquer largura. As cores vêm de
    `primaria/padrao`, `superficie/hover` e `texto/primario`, com a borda da alça em
    `borda/foco`/`linha/grossa`. O raio é `raio/pilula`; a altura, `icone/md` e
    `espaco/6`.
  - `Linha de opção` (set `10379`), variante `Controle` = interruptor `10368`, botão
    `10380` e select `10825`. Propriedades: `Título`, `Descrição` e `Mostrar descrição`.
    O `Interruptor`, o `Botão` secundário ("Trocar") e o `Select` (sem rótulo e com
    largura `largura/select-filtro`) estão expostos para a instância. É um cartão
    `superficie/padrao` com borda `borda/sutil`, raio `raio/item`, padding `espaco/20` e
    gap `espaco/16`.
  - `Item de versão` (set `11264`), `Estado` = padrão `11243` e selecionado `11265`.
    Propriedades: `Data`, `Detalhe` e `Quando`. Tem rádio de 20 (`icone/md`, borda
    `borda/forte`; no selecionado, `borda/foco` e ponto `primaria/padrao`). O
    selecionado usa `superficie/selecionada` com borda `borda/foco`. Padding `espaco/16`.
  - `Popover do gerador` (`11671`), com largura `largura/dialogo`, `superficie/elevada`,
    `Sombra/Flutuante`, raio `raio/cartao` e padding `espaco/20`. Traz o título com
    fechar, a prévia em `Campo secreto` oculto ("Senha gerada"), o tamanho com o
    controle deslizante, quatro interruptores (minúsculas, maiúsculas, dígitos e
    símbolos), "Gerar de novo" (fantasma, ícone gerador) e "Usar esta senha" (primário).
    O "Copiar" fica no próprio `Campo secreto`.
  - `Diálogo de restauração` (set `13033`), `Etapa` = versões `12631` e confirmar
    `13034`. Usa `largura/dialogo-largo`, `raio/janela`, `Sombra/Modal` e
    `superficie/elevada`.
    - **Versões:** 5 `Item de versão` datados (um vem do pendrive), legenda "Mostrando 5
      de 30 versões…", Cancelar e Continuar.
    - **Confirmar:** a versão escolhida, o `Campo` foco "Senha mestra desta versão"
      (mascarado, com cadeado e a nota "O PIN não serve aqui") e um aviso em
      `aviso/sutil` dizendo que o atual vira backup e que passam a valer a senha e o PIN
      da versão. Termina com Voltar e "Restaurar esta versão".
- **Variables novas** (todas com o code syntax WEB):
  - Em `Cores` (modo único `Escuro`): `impressao/papel` (`VariableID:85` → `neutro/0`),
    `impressao/texto` (`86` → `neutro/950`), `impressao/texto-secundario` (`87` →
    `neutro/600`) e `impressao/linha` (`88` → `neutro/400`). São só para a folha
    impressa.
  - Em `Largura`: `pagina/a4-largura` = 794 (`89`) e `pagina/a4-altura` = 1123 (`90`),
    que é o A4 em 96 dpi.
  - Agora são 212 variables.
- **Oito frames no Layout** (página `2796`, modo `Escuro` herdado da página), na grade da
  minha área: linha y = 10400 (x 0, 1360, 2720, 4080) e linha y = 11400 (x 0, 1360,
  2720).

| Frame | Id | Conteúdo |
| --- | --- | --- |
| `Gerador de senha / Avulso` | `13141` | 1280×800; nav com `Gerador de senha` ativo; painel "Opções" (`13200`): tamanho com `Controle deslizante` 25% "24 caracteres" e 4 `Linha de opção` com interruptor ligado; painel `resultado` (`13335`): "Senha gerada", `Campo secreto` oculto (`••••`), barra de força "muito forte · cerca de 150 bits", nota "Não fica no cofre… área de transferência limpa em 30 s", rodapé com "Gerar de novo" e "Copiar" |
| `Gerador de senha / No formulário` | `14307` | cópia de `Formulário / login / Edição` (`6926`, Fase 5) com o `Campo secreto` Senha (`14622`) em `Mostrar gerar` e o `Popover do gerador` (`15135`, absoluto) aberto logo abaixo do campo |
| `Configurações / Geral` | `15242` | nav com `Configurações` ativo; "Acesso" (`15289`): Senha mestra (Trocar), PIN de aprovação (Trocar), Tempo de trava (select "4 horas", no máximo 4 h); "Backup" (`15325`): Backup local (Restaurar), Backup externo em pendrive (**desligado**), Kit de emergência (Imprimir) e nota "Backups sempre cifrados". Não há opção de tema |
| `Configurações / Backup externo ligado` | `16178` | o mesmo com o interruptor ligado e o cartão do pendrive escolhido (`16318`): "WARD-BACKUP", "UUID 7C1E-42A9 · conectado em /media/usuario/WARD-BACKUP", "Última cópia hoje às 14:32 · 30 versões em ward-backup/" e "Trocar" |
| `Restaurar backup / Versões` | `16349` | Configurações sob `sobreposição` (`16457`, `fundo/sobreposicao`) com o `Diálogo de restauração` Versões (`16458`) |
| `Restaurar backup / Confirmar` | `16512` | o mesmo, com o diálogo Confirmar (`16621`, em `16620`) |
| `Kit de emergência / Impressão` | `16773` | A4 794×1123 (largura e altura ligadas a `pagina/a4-*`), folha branca em `impressao/*`. Traz a logo `Logo / Horizontal` (`16781`, nome em `impressao/texto`) e "Kit de emergência" com a data. Seções: "Onde está o cofre" (`~/.local/share/ward/`); "Onde estão os backups" (cópias locais em `~/.local/share/ward/backups/` e o pendrive "WARD-BACKUP · UUID 7C1E-42A9 · pasta ward-backup/"); "Senha mestra" com duas linhas **em branco** e "Escreva à mão. O Ward nunca preenche este espaço."; "Como restaurar" (4 passos). Rodapé: o PIN não aparece na folha |

- **Conferência:** fiz um screenshot de cada frame em 1x e do popover, dos sets e do
  rodapé em 1,5x e 2x. O texto não encosta na borda: as linhas e os cartões têm 20, os
  itens 16, o diálogo e os painéis 24 e a folha 64. Rodei também uma auditoria no
  `.figcraft` salvo (`scripts/audit8.py` no scratchpad, adaptado do da 4f e agora também
  olhando o raio em `corners`) na seção `9453` e nos 8 frames. Deu 0 preenchimento,
  borda, espessura, raio, gap ou padding solto e 0 texto sem text style. Só sobraram as 4
  bordas tracejadas roxas de apresentação dos component sets, que são o mesmo padrão do
  `Interruptor` da Fase 5. Conferi também no arquivo salvo que todos os ids abaixo
  existem e que não há modo "Claro".

## Segurança
- Invariantes: a fase é só de design e não toca código. O desenho segue S1, R10, R23,
  R27 e D17:
  - **Senha gerada só mascarada:** no avulso e no popover, ela aparece só no
    `Campo secreto` oculto (`••••`). O texto diz que ela só aparece no revelar e que
    copiar limpa a área de transferência em 30 s. Nenhum valor gerado aparece em claro
    nas telas.
  - **Avulso não grava:** "Nada do que é gerado aqui fica salvo". Para guardar a senha,
    o caminho é o gerador dentro do formulário.
  - **Configurações:** trocar a senha mestra pede a senha atual, e trocar o PIN pede a
    senha mestra. O PIN aparece como "aprova usos, não destrava". O tempo de trava vai
    até 4 h no máximo, que é o teto do escopo. O backup externo vem **desligado** na
    `Geral`. Não há opção de tema (D1).
  - **Restaurar:** a lista mostra só metadados do arquivo (data, origem, tamanho), nunca
    o conteúdo. A confirmação pede a senha mestra **da versão** ("O PIN não serve aqui")
    e diz que o cofre atual é guardado como backup antes da troca (D17). Uma senha
    errada não muda nada.
  - **Kit:** a linha da senha mestra fica em branco, o PIN não aparece e nenhum segredo
    é impresso (V0.15).
- Exemplos só falsos: o pendrive "WARD-BACKUP" com UUID inventado `7C1E-42A9`, datas
  fictícias e caminhos genéricos (`/media/usuario/…`). Não toquei em cofre, credencial,
  `~/.ssh` ou `~/.aws`, nem no Figma.
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. Os scripts de leitura do `.figcraft` usam só a biblioteca
  padrão do Python e ficaram no scratchpad.

## Ficou de fora
- Telas dos fluxos "Trocar senha mestra", "Trocar PIN" e "Escolher pendrive": o contrato
  não pede, e as linhas de Configurações só apontam para eles.
- Estados de erro do restaurar (senha da versão errada, arquivo corrompido) e do
  gerador (nenhum conjunto ligado): o contrato não pede.
- Não editei `docs/mission/design-nodes.md`, como foi pedido. Os ids estão abaixo.

## Ids para design-nodes

Seção e componentes (página Design System `2`):

| Item | Id | Variantes / filhos |
| --- | --- | --- |
| `Seção / Gerador e configurações` | `9453` (x 5600, y 0) | título `9890`, descrição `9891` |
| `Controle deslizante` (set) | `12604` | Posição 25% `9896` (preenchido `9898`, alça `9899`, restante `9897`), 50% `12605`, 75% `12609` |
| `Linha de opção` (set) | `10379` | Controle interruptor `10368` (Interruptor `10370`), botão `10380` (Botão `10384`), select `10825` (Select `10829`) |
| `Item de versão` (set) | `11264` | padrão `11243`, selecionado `11265` |
| `Popover do gerador` | `11671` | cabeçalho `11672`, Campo secreto `11673`, tamanho `11698`, conjuntos `11699`, rodapé `11700` |
| `Diálogo de restauração` (set) | `13033` | Etapa versões `12631`, confirmar `13034` |

Variables: `impressao/papel` `VariableID:85`, `impressao/texto` `VariableID:86`,
`impressao/texto-secundario` `VariableID:87`, `impressao/linha` `VariableID:88` (em
`Cores`), `pagina/a4-largura` `VariableID:89`, `pagina/a4-altura` `VariableID:90` (em
`Largura`).

Frames de tela (página Layout `2796`):

| Frame | Id | Posição |
| --- | --- | --- |
| `Gerador de senha / Avulso` | `13141` | x 0, y 10400 |
| `Gerador de senha / No formulário` | `14307` | x 1360, y 10400 (1280×1392) |
| `Configurações / Geral` | `15242` | x 2720, y 10400 |
| `Configurações / Backup externo ligado` | `16178` | x 4080, y 10400 |
| `Restaurar backup / Versões` | `16349` | x 0, y 11400 |
| `Restaurar backup / Confirmar` | `16512` | x 1360, y 11400 |
| `Kit de emergência / Impressão` | `16773` | x 2720, y 11400 (794×1123) |

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` só existe a partir da Fase 11 |
| `inspect_document`, `get_node`, `screenshot` (1x, 1,5x, 2x) | ok |
| `node.create`, `instance.create`/`setProperties`/`swap`, `component.combineVariants`/`addVariant`/`addProperty`/`exposeInstance`, `variable.create`/`set`/`update`/`bind`, `style.apply`, `edit.duplicate`/`delete`, `node.set`/`reorder` | ok |
| `python3 -I scripts/audit8.py` (seção + 8 frames) no `.figcraft` salvo | 0 pendências (só as 4 bordas de apresentação dos sets) |
| `save_file` (12×, mesmo caminho) | ok |

## Problemas descobertos
- **Kit de emergência em folha branca:** imprimir em fundo escuro gasta toner e
  atrapalha a escrita à mão. Por isso a folha usa as variables novas `impressao/*`
  (branco e tinta escura), dentro do modo único `Escuro` de `Cores`. Não é variante
  clara nem tema, mas um validador literal de V0.19 pode marcar a folha. Precisa da
  decisão do usuário. Se ele preferir, dá para trocar as `impressao/*` por aliases
  escuros sem mexer no frame.
- **"Tempo de trava" configurável vs. escopo:** o escopo fixa a trava em 4 h sem uso
  (R23), e Configurações pede "tempo de trava" (R25). Desenhei um select com 4 horas e
  "máximo 4 h", que só permite **encurtar**. Assim a opção não afrouxa a segurança. As
  opções exatas (15 min, 1 h, 4 h?) não estão definidas nos documentos.
- **Senha e PIN depois de restaurar:** o aviso diz que passam a valer a senha mestra e o
  PIN da versão restaurada. É a consequência natural de D17 (o arquivo inteiro volta),
  mas os documentos não dizem isso em lugar nenhum. Vale confirmar.
- **Nome do arquivo do cofre** não está definido no PRD nem no escopo (só a pasta
  `~/.local/share/ward/`). O kit mostra só a pasta.
- **Ícone trocado no `Botão` primário não fica branco:** com `Ícone#10:34` trocado
  (ex.: copiar), o ícone mantém a cor `icone/padrao` (cinza) em vez de
  `icone/sobre-cor`. No `Gerador de senha / Avulso`, corrigi por override nos vetores
  (`13457` `13791`). O ideal é corrigir no componente `Botão` (compartilhado; não alterei
  para não mexer no trabalho das Fases 5 e 6). Sugestão: entra na 10c.
- **Override de tamanho e posição nos filhos de instância não existe no FigCraft.** Por
  isso o `Controle deslizante` virou um set com 3 posições, e não um único componente
  ajustável.
- **`Gerador de senha / No formulário` é uma cópia** de `Formulário / login / Edição`
  (`6926`) tirada durante a Fase 5. Se a Fase 5 mudar o formulário, a cópia não acompanha.
  Na Fase 10, vale refazer a cópia ou conferir.
- O `Campo secreto` estreito (meia coluna, com revelar, gerar e copiar) corta a máscara
  para "••••••". É só visual.
- JetBrains Mono continua sem estar instalada (fallback nos valores mono do kit e do
  tamanho), como nas fases anteriores.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (seção `9453` com 5 componentes/sets, 6 variables, 7 telas) |
| `docs/mission/handoffs/phase-8.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhum (SEG) tocado)
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots com zoom; arquivo salvo auditado; `save_file` feito)
- [x] Ficou dentro do escopo da fase (componentes existentes não alterados; o problema do ícone no `Botão` primário ficou registrado)
