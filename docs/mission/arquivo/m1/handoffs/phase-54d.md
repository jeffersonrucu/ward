# Fase 54d — Aviso dos backups antigos no FigCraft (DESIGN)

## Implementado
- `Diálogo de troca` › `Tipo=senha mestra` (`19918`, Design System), no `corpo` `19934`:
  - explicação `19924` trocada para o texto do app: "A senha atual confirma a troca.
    Depois dela, o cofre só abre com a nova." (o design tinha uma frase sobre backups
    que o app não mostra);
  - texto novo `aviso backups` `22962` logo abaixo, com o estilo `8:12` (Inter 14/20) e o
    fill ligado a `aviso/texto` (`6:83`, `#F5B544` = `--cor-aviso-texto`): "Os backups
    antigos, no computador e no pendrive, continuam abrindo com a senha antiga. Se você
    troca porque a senha antiga vazou, apague esses backups." (igual a
    `TrocarSenha.svelte` › `.aviso`).
- Set `Diálogo de troca` `19978`: `Tipo=PIN` `19979` desceu para y 652 e o set foi para
  1228 de altura, porque a variante da senha mestra cresceu 56 px e cobria a do PIN.
- `Configurações / Trocar senha mestra` (`20085`, Layout, 1280×800): a instância `20411`
  herdou a explicação (`20418`) e o aviso (`22963`); ficou igual à captura do app
  `docs/mission/evidence/fase-54/trocar-senha-aviso.png` (ordem, cor e quebra de linha).
- Ids registrados em `docs/mission/design-nodes.md` › "Fase 54d".

Como foi salvo: o `open_file` do caminho original caiu numa aba velha e suja do headless
(histórico "39d contagens 22", sem os nodes da 49b). Copiei o arquivo para o scratchpad,
abri a cópia (aba nova), editei, conferi que o sha256 do original não tinha mudado
(`3a2afbfb…7476`) e salvei por cima com `save_file`. Conferência: `unzip -t` sem erro,
`document.json` com o aviso (2 ocorrências: componente e tela) e com os frames da 49b;
reaberto numa outra cópia, o node `22963` renderiza com o aviso.

## Segurança
- Invariantes tocadas: nenhuma de código. O aviso documenta no design o limite já aceito
  na Fase 54 (backups antigos seguem com a senha antiga). Nenhum segredo no arquivo: os
  campos mostram só máscara e o "Valor revelado" falso que já existia.
- Testes de ataque: não se aplica (fase só de design).
- Dependências novas: nenhuma.

## Ficou de fora
- Código do app (já feito na Fase 54) e qualquer arquivo do repositório fora de
  design-nodes/evidência/handoff: a Fase 54b edita o código em paralelo.
- `make check` não rodou, por instrução do orchestrator (outro worker compila em paralelo).

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do original antes de salvar (igual ao da cópia) | 0 |
| `unzip -t` e leitura do `document.json` do arquivo salvo | 0 |
| `make check` | não rodado (instrução do orchestrator) |

## Problemas descobertos
- Tamanho de texto divergente (anterior à fase): no design a explicação do `Tipo=senha
  mestra` (`19924`) usa o estilo `8:12` (14/20); no app a `.explicacao` e o `.aviso` de
  `TrocarSenha.svelte` usam `--tamanho-sm`/`--altura-linha-sm` = 13/18 (estilo `8:14`).
  O aviso novo seguiu a explicação do design (14/20) para os dois ficarem iguais no
  diálogo; o app fica 13/18 nos dois. Não escolhi lado: ou o app passa para
  `--tamanho-md`/20 nos dois, ou o design troca `19924` e `22962` para `8:14`.
- O headless do FigCraft guarda várias abas antigas com o caminho do `Ward.figcraft`
  (índices 1–4; a 1 suja com a 39d). Quem usar `open_file` no caminho original pode
  editar uma versão velha; usar uma cópia, como aqui e na 49b.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado |
| `docs/mission/design-nodes.md` | alterado (seção "Fase 54d") |
| `docs/mission/evidence/fase-54d/configuracoes-trocar-senha-mestra.png` | criado |
| `docs/mission/evidence/fase-54d/dialogo-de-troca.png` | criado |
| `docs/mission/handoffs/phase-54d.md` | criado |

## Procedimentos
- [ ] `make check` rodado (não rodado por instrução: a 54b compila em paralelo)
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre tocado)
- [x] Teste de ataque para cada (SEG) tocado (nenhuma SEG tocada)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
