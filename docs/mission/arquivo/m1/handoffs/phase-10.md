# Fase 10 — (DESIGN) Conferência final do M0

## Implementado
- Trabalhei no app FigCraft ao vivo (`mcp__figcraft-app__*`), no `Ward.figcraft` já aberto, sem `open_file`. Para conferir o arquivo salvo, usei a CLI headless numa cópia no scratchpad. A cópia tem o mesmo sha256 do arquivo em disco. Não houve timeout de UI. Fiz 2 `save_file`, o último depois da última mudança, e o app terminou com `dirty: false`.
- **Conferência de V0.1 a V0.22**, uma asserção por vez:
  - os scripts rodam sobre o `document.json` da cópia salva;
  - os screenshots de cada grupo foram tirados no app;
  - a página `1:3` do Figma foi lida só com `get_metadata`.
- **Evidência** em `docs/mission/evidence/fase-10/`:
  - `v0-auditoria.json`: resultado por asserção;
  - `v0-21-inspiracoes.json`;
  - `cli-info-reaberto.txt`: o arquivo reaberto pela CLI;
  - `layout/`: 51 PNGs, um por frame do Layout, em 0,5x;
  - `design-system/`: 25 PNGs das seções;
  - `scripts/`: `lib.py` e `v0.py`, só biblioteca padrão. Para rodar (de qualquer diretório, sem gerar `__pycache__`, ajuste da Fase 10d): `python3 -I <caminho>/v0.py <arquivo.figcraft> <design-nodes.md> <saida.json>` e `python3 -I <caminho>/forms.py <arquivo.figcraft> [saida.json]`.

| Asserção | Resultado | Evidência |
| --- | --- | --- |
| V0.1 | passa: `Cores` só com o modo `Escuro`, mais `Tipografia`, `Espaçamento`, `Raio` (6/10/12/16/20/24, pílula 9999) e `Largura`. As semânticas cobrem fundo, superfície, texto, borda, primária, perigo, aviso, sucesso, dev, stage e prod | `v0-auditoria.json` › V0.1 |
| V0.2 | passa: 0 valores soltos nos 51 frames. A conferência olhou preenchimento, borda, raio, gap, padding e text style fora de instância, e também os overrides de cor, borda e raio dentro de instâncias | › V0.2 |
| V0.3 | passa: todos os sets com as variantes pedidas. O `Campo secreto` tem as props `Mostrar revelar` e `Mostrar copiar`. Todos os componentes estão na Design System | › V0.3 |
| V0.4 | **passa depois da correção**: o fechar das 5 telas de aprovação era um frame solto e virou instância do componente novo `Botão ícone` (`18832`). Nenhuma instância aponta para componente ausente | › V0.4; `layout/Aprovação _ *.png` |
| V0.5 e V0.6 | passam: o PIN aceita de 4 a 6 dígitos e a tela explica que ele não destrava. O pedido do agente mostra a entrada, o comando exato e a contagem "1:42 restantes" | `layout/Primeiro uso _ *.png`, `Destravar _ *.png` |
| V0.7 | passa: a lista é agrupada por grupo, com badge de tipo e de ambiente em cada item. A Filtrada mostra o resumo "Tipo banco · ambiente stage · busca pg". Nenhum segredo aparece | `layout/Credenciais _ *.png` |
| V0.8 a V0.10 | passam: os 18 formulários existem. Cada um tem os campos do tipo, nome, grupo, ambiente, projeto, tags, campos extras com a chave segredo, nível de acesso e anexos. O login tem TOTP com o código e a semente em `Campo secreto`. O `servidor / Edição` tem `Conectar`. O grupo com autocompletar (combobox) aparece em `login / Novo` e `smtp / Novo` | script `forms.py` (`evidence/fase-10/scripts/`); `layout/Formulário _ *.png` |
| V0.11 | passa: as três opções têm explicação, e a mesma tela vale para uma entrada prod | `layout/Nível de acesso _ *.png` |
| V0.12 a V0.15 | passam: gerador avulso e no formulário; Geral com o backup externo desligado e sem opção de tema; backup ligado com o pendrive; versões datadas; confirmar com `Campo senha`; kit A4 (794×1123) com a linha em branco | PNGs correspondentes |
| V0.16 | passa: nos 5 estados aparecem os 4 botões com os rótulos exatos. "Negar" está em `Estado=foco`, e prod tem a faixa de destaque | › V0.4 (botões listados por script); PNGs |
| V0.17 | passa: Destravado tem abrir, travar agora e sair; Travado tem abrir, "Destravar…" e sair | PNGs |
| V0.18 | **passa com um frame a mais**: no popup de 600, o Grupo fica abaixo da dobra porque o corpo rola. Acrescentei `Extensão / Salvar login novo / Conteúdo inteiro` (`18850`), uma cópia sem rolagem que mostra tipo, grupo e ambiente | `layout/Extensão _ Salvar login novo _ Conteúdo inteiro.png` |
| V0.19 | passa: as duas páginas usam `Cores` = `Escuro` e nenhum node usa outro modo. Nenhum texto fala de tema ou de "claro" | › V0.19 (ver "Para aprovação": folha branca do kit e `Tom do ícone`) |
| V0.20 | passa: os 508 textos únicos do Layout estão em português, e os termos técnicos (Access key ID, passphrase, starttls) são nomes de campo ou de valor. Nenhum dos 19 segredos do cofre de teste aparece, nem em URL-encode ou base64, no `document.json` e nas versões do zip. Os únicos valores falsos de exemplo no arquivo são `falso-exemplo-9f3k`, o valor revelado de exemplo do `Campo senha`, e `FALSOACCESSKEY01`, o Access Key ID falso dos formulários aws, que é identificador e fica visível (decisão 5). Nenhum dos dois é segredo do cofre de teste (correção da Fase 10d) | › V0.20 |
| V0.21 | passa: a `1:3` do Figma é idêntica ao snapshot da Fase 1 (4 filhos, mesmos ids, nomes, posições e tamanhos). O `Ward.figcraft` não tem página Inspirações | `v0-21-inspiracoes.json` |
| V0.22 | passa: todo frame e todo componente do arquivo tem id no `design-nodes.md`. Dos 805 ids citados, os 25 ausentes estão todos marcados lá como apagados ou substituídos. O arquivo reaberto pela CLI tem o mesmo sha256 do salvo | › V0.22; `cli-info-reaberto.txt` |
| V0.23 | **pendente**: depende da aprovação do usuário (seção abaixo) | — |

- **Correções no arquivo** (pequenas e dentro do design aprovado):
  - componente `Botão ícone` (`18832`, ícone `18833`, prop `Ícone` de troca, padrão `Ícone/fechar`) na `Seção / Aprovação`, com as mesmas medidas e variables do frame que substitui. As instâncias novas são `18835`, `18838`, `18841`, `18844` e `18847`; os frames soltos 16714, 16822, 16897, 16972 e 17047 foram apagados. O screenshot depois da troca ficou igual ao de antes;
  - frame `Extensão / Salvar login novo / Conteúdo inteiro` (`18850`, x 8520, y 7600, 360×806). É só documentação: o popup real continua `17414`, com 600;
  - a descrição da `Seção / Aprovação` (`16327`) passou a citar o `Botão ícone`.
- **`design-nodes.md`**:
  - a tabela da 4e trocou `Campo` por `Campo senha` em `3679`, `3712`, `3372`, `3427` e `3484`;
  - o `Botão ícone` e as instâncias do fechar entraram na seção da Fase 7;
  - o frame novo entrou na Fase 9;
  - nova seção "Fase 10".

## Segurança
- Invariantes: a fase é só de design. Conferi as regras de segredo nas telas:
  - nenhum segredo do cofre de teste aparece no arquivo;
  - os campos secretos mostram só a máscara;
  - a senha mestra fica no `Campo senha`, que não tem copiar;
  - na extensão, a senha do site aparece em `Campo secreto` sem revelar nem copiar (S1, R6).
- Testes de ataque: não se aplicam, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma. Os scripts usam só a biblioteca padrão.

## Ficou de fora
- **V0.23 (aprovação do usuário):** fica para o orchestrator apresentar. O material está na seção abaixo.
- **As 4 imagens órfãs no `imageIndex` do arquivo** não aparecem em nenhum node. Não achei comando para limpá-las sem risco e elas não afetam nada, então ficaram.
- **Botões só com ícone dentro de componentes** (fechar do `Modal`, do `Toast` e dos diálogos da `Seção / App`) continuam como frames internos dos componentes. Não violam V0.4, porque estão dentro de componentes. Trocar pelo `Botão ícone` é refino para a fase de UI.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não existe antes da Fase 11 (fase só de design) |
| `edit.duplicate`, `node.reparent`, `component.create`, `component.addProperty`, `instance.create`, `edit.delete`, `node.set` | ok |
| `save_file` (2×) | ok |
| `FigCraft.AppImage cli info` / `cli export` (Layout e Design System, 0,5x) na cópia salva | 0 |
| `python3 -I v0.py final.figcraft design-nodes.md v0-auditoria.json` | 0 |
| `get_metadata` do Figma `1:3` (só leitura) | ok |

## Problemas descobertos
- **V0.19, leitura literal:** a folha do kit usa `impressao/*` (papel branco) dentro do modo `Escuro`. A coleção `Tom do ícone` tem 8 modos, que são tons de ícone e não temas. O `Ícone da bandeja` tem a variante `Cor=grafite` para painéis claros do sistema. Nenhum dos três é tema claro do app, mas um validador literal pode marcar.
- **Popup de 600 rola:** o QA que olhar só o screenshot do `17414` não vê o Grupo. O frame `18850` resolve isso para a validação.
- **JetBrains Mono não está instalada:** o texto `Mono/*` sai com a fonte de reserva nos exports, como já registrado desde a 4c.
- **Escopo, PRD e contrato** ainda dizem "cliente" em alguns pontos, onde as telas usam "grupo" (registrado na 4f). Não editei.

## Para aprovação do usuário

### Telas por grupo (página Layout `2796`)
- **Primeiro uso:**
  - `Primeiro uso / Criar cofre` `3537`;
  - `/ Senha mestra` `3619`;
  - `/ PIN` `3733`.
- **Destravar:**
  - `Destravar / Padrão` `3342`;
  - `/ Senha errada` `3393`;
  - `/ Pedido do agente` `3450`.
- **Credenciais:**
  - `Credenciais / Vazia` `4780`;
  - `/ Cheia` `4063`;
  - `/ Filtrada` `4410`;
  - `/ Filtros` `5259` (extra).
- **Nova credencial:**
  - `Nova credencial / Escolher tipo` `17657`;
  - `/ Tipo escolhido` `18055`.
- **Nível de acesso:**
  - `Nível de acesso / Escolher` `5615`;
  - `/ PIN` `5994`;
  - `/ PIN errado` `6373`.
- **Formulários (Novo / Edição):**
  - login `7450` / `6926`;
  - banco `8537` / `7908`;
  - servidor `9900` / `8981`;
  - api `11272` / `10389`;
  - aws `12170` / `11705`;
  - smtp `13867` / `9454`;
  - ftp/sftp `14743` / `10895`;
  - nota segura `15399` / `12683`;
  - genérico `15845` / `13459`.
- **Gerador:**
  - `Gerador de senha / Avulso` `13141`;
  - `/ No formulário` `14307`.
- **Configurações e backup:**
  - `Configurações / Geral` `15242`;
  - `/ Backup externo ligado` `16178`;
  - `Restaurar backup / Versões` `16349`;
  - `/ Confirmar` `16512`;
  - `Kit de emergência / Impressão` `16773`.
- **Aprovação:**
  - `Aprovação / Normal` `16701`;
  - `/ prod` `16810`;
  - `/ PIN errado` `16885`;
  - `/ PIN bloqueado` `16960`;
  - `/ Expirando` `17035`.
- **Bandeja:**
  - `Bandeja / Destravado` `17269`;
  - `/ Travado` `17300`.
- **Extensão:**
  - `Extensão / Sugestões do domínio` `17332`;
  - `/ Salvar login novo` `17414`;
  - `/ Salvar login novo / Conteúdo inteiro` `18850` (extra, Fase 10);
  - `/ Cofre travado` `17392`.
- **Ícone e logo (página Design System `2`):**
  - `Seção / Logo` `3286`: `Logo / Ícone do app` `3292`, `Logo / Símbolo`, `Logo / Horizontal`;
  - `Seção / Bandeja e extensão` `17137`: `Ícone da bandeja` `17141`, `Ícone da extensão` `17148`;
  - exports em `docs/design/logo/` e `docs/design/icones/`.

### Pontos de decisão em aberto (acumulados nos handoffs)
1. **Folha branca do Kit de emergência** (Fase 8): o papel é branco com tinta escura (`impressao/*`) para imprimir e escrever à mão. A alternativa é trocar as `impressao/*` por aliases escuros. Afeta a leitura de V0.19.
2. **Opções do tempo de trava** (Fase 8): hoje é um select com "4 horas" e "máximo 4 h", que só deixa encurtar. Falta definir a lista, por exemplo 15 min, 1 h e 4 h.
3. **Senha mestra e PIN depois de restaurar** (Fase 8): o aviso diz que passam a valer a senha e o PIN da versão restaurada. Isso decorre de D17, mas nenhum documento diz. Confirmar.
4. **"Negar" ou "Fechar" no PIN bloqueado** (Fase 7): o pedido já está negado, e o botão só fecha a janela. Ficou "Negar" porque o contrato pede os 4 botões em todos os estados.
5. **Access Key ID da AWS visível** (Fase 5): é tratado como identificador, com Copiar. Só o secret e o session token ficam mascarados. A alternativa é mascarar o ID também.
6. **Cor do gradiente da logo** (Fase 4g): o gradiente vai de `azul/400` a `azul/600`, combinando com a primária. A referência é um índigo mais violeta, e a troca é só de valor nas duas primitivas.
7. **Contorno da bandeja travada em 16 px** (Fases 9 e 10c): o travado é contorno e não só cor, mas em 16 px o traço fica pesado. Olhar `docs/design/icones/bandeja-travado-16.png`. Também decidir se a variante grafite (painéis claros) fica.
8. **Mínimo da senha mestra** (Fases 3 e 4e): "Mínimo de 12 caracteres" e a barra de força não estão definidos no PRD.
9. **Botão "Agora não" no pedido do agente** (Fase 3, `Destravar / Pedido do agente`): ele nega na hora. O escopo só diz que o pedido espera ou expira.
10. **Filtros em diálogo e grupos fora da navegação** (Fase 4f, a pedido do usuário durante a fase): o botão `Filtrar` abre o `Diálogo de filtros`, e o grupo aparece só como marcador colorido no cabeçalho da lista. Confirmar como versão final.
11. **Ícone da cor por modo** (Fase 10c): os ícones ganham cor pelo modo `Tom do ícone` (8 modos), e não por variantes.
12. **Tokens sem uso mantidos** (Fase 10a): `peso/*`, `tamanho/2xl`, `altura-linha/2xl`, `raio/nenhum`, `largura/conteudo`, `espaco/80`, `espaco/96`, `grupo/4..5/cor`; e os componentes `Toast`, `Modal` e `Logo / Ícone do app` sem instância nas telas. Manter ou podar.
13. **`Botão ícone` novo** (esta fase): confirmar o nome e se ele deve substituir também o fechar interno do `Modal`, do `Toast` e dos diálogos.

### Frames recomendados para exportar
- **Visão do app:** `4063` (Credenciais / Cheia), `4410`, `4780`, `17657`, `7450` (login / Novo), `8981` (servidor / Edição, com Conectar), `14307`.
- **Acesso:** `3537`, `3619`, `3733`, `3342`, `3450`.
- **Aprovação:** `16701`, `16810`, `16960` (PIN bloqueado; decisão 4), `17035`.
- **Configurações e kit:** `15242` (decisão 2), `16512` (decisão 3), `16773` (decisão 1).
- **Bandeja e extensão:** `17269`, `17300` (decisão 7), `17332`, `17414`, `18850`, `17392`.
- **Ícone e logo:** `3286` (`Seção / Logo`) e `17137` (`Seção / Bandeja e extensão`). Mais os PNGs `docs/design/logo/ward-icone-app-{16,32,512}.png` e `docs/design/icones/bandeja-*-16.png`.
- **Formulário AWS:** `11705` (decisão 5).
- Já exportados em 0,5x em `docs/mission/evidence/fase-10/layout/` e `design-system/`, para uma primeira olhada.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (`Botão ícone`, 5 instâncias, frame `18850`, descrição `16327`) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-10/v0-auditoria.json` | criado |
| `docs/mission/evidence/fase-10/v0-21-inspiracoes.json` | criado |
| `docs/mission/evidence/fase-10/cli-info-reaberto.txt` | criado |
| `docs/mission/evidence/fase-10/layout/*.png` (51) | criado |
| `docs/mission/evidence/fase-10/design-system/*.png` (25) | criado |
| `docs/mission/evidence/fase-10/scripts/lib.py`, `v0.py`, `forms.py` | criado |
| `docs/mission/handoffs/phase-10.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não existe antes da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) no M0
- [x] (UI/DESIGN) Conferido contra o FigCraft (screenshots, arquivo salvo reaberto pela CLI; `save_file` feito)
- [x] Ficou dentro do escopo da fase
