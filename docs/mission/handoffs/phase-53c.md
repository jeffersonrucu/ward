# Fase 53c — Design: botão "Carregar de arquivo" e estados do campo "Chave privada"

## Implementado
- **Design System**: seção nova `Seção / Campo chave privada` `20976` com o componente
  `Campo chave privada` (set `20988`):
  - `Estado=vazio` `20979`: caixa de várias linhas (`entrada` `20981`) com o
    placeholder "Cole a chave privada (formato OpenSSH ou PEM)", o `Botão` secundário
    **Carregar de arquivo** com ícone `clipe` (`20982`) e a ajuda "Cole a chave ou use
    Carregar de arquivo. Depois de salvo, ela só aparece com revelar.";
  - `Estado=aviso` `20989`: a mesma caixa e botão; a linha de ajuda vira o motivo fixo
    ("o arquivo não é uma chave privada OpenSSH ou PEM") em `perigo/texto`;
  - props `Rótulo`, `Ajuda`, `Aviso`. Tudo ligado a variables e estilos que já existiam
    (`espaco/*`, `raio/controle`, `superficie/campo`, `borda/sutil`, `texto/*`,
    `perigo/texto`, `largura/campo`, `Texto/Rótulo`, `Texto/Corpo`, `Texto/Legenda`).
    Nenhuma variable nova.
  - Exemplo do estado "chave carregada" `20998`: instância do `Campo secreto` oculto
    (sem revelar, copiar ou gerar), com a ajuda "Chave carregada do arquivo (411 bytes).
    Ela fica oculta e vai para o cofre ao salvar.". É o que o app mostra, e não precisou
    de componente novo.
- **Layout** (1280 de largura; a altura cresce para o corpo caber acima do rodapé, como
  nos outros formulários):
  - `Formulário / ftp/sftp / Novo` `14743`: alterado no lugar. A "Chave privada" era um
    `Campo secreto` vazio de uma linha, diferente do app. Passou a usar o `Campo chave
    privada` vazio `21023` (altura 1470 → 1574);
  - `Formulário / servidor / Novo / Chave` `21032`: cópia de `9900` com Autenticação
    "Chave", o campo vazio com o botão e a Passphrase (opcional);
  - `Formulário / servidor / Novo / Chave com aviso` `21452`;
  - `Formulário / servidor / Novo / Chave carregada` `21847`;
  - `Formulário / servidor / Edição / Trocar chave` `22268`: cópia de `8981` depois do
    "Trocar valor" na chave salva (campo vazio com o botão; a passphrase continua
    oculta).
  - `9900` (Autenticação "Senha") e `8981` (chave salva, oculta) ficam como estavam,
    porque nesses estados o app não mostra o botão. Os estados novos saem desses dois
    frames.
- `docs/mission/design-nodes.md`: seção "Fase 53c" com os ids.
- PNG em `docs/mission/evidence/fase-53c/`, exportados do arquivo salvo:
  `servidor-novo-chave.png`, `servidor-novo-chave-aviso.png`,
  `servidor-novo-chave-carregada.png`, `servidor-edicao-trocar-chave.png`,
  `ftp-sftp-novo.png`, `ds-campo-chave-privada.png`.

## Segurança
- **Invariantes tocadas**: nenhuma em código. O desenho segue S2/S11/S12 da 53a: a
  chave carregada só aparece como máscara, sem revelar nem copiar, e a ajuda diz que ela
  vai ao cofre ao salvar. O aviso é o texto fixo do app, sem caminho (S13). Não há
  área de transferência no fluxo.
- **Testes de ataque**: não se aplicam (fase só de design).
- **Dependências novas**: nenhuma.
- Textos e valores falsos: "411 bytes" e as entradas `acme-*` do cofre de teste. Nenhum
  conteúdo de chave no arquivo de design.

## Ficou de fora
- **`make check`**: não rodou. A fase não muda código, e o disco está apertado (pedido
  do orchestrator para não compilar). Os arquivos alterados são só `.md` e `.png`.
- **Hover do botão no aviso**: o screenshot da 53a mostra o botão com hover depois do
  clique. O design usa o estado padrão, como as outras telas.
- **`Formulário / ftp/sftp / Edição` `10895`**: usa autenticação por senha. Não ganhou
  estado de chave, porque o plano não pede.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `sha256sum` do `Ward.figcraft` antes de editar e antes de salvar (iguais: `7729f573…`) | 0 |
| `unzip -t /home/jeffersonoliveira/documentos/Ward.figcraft` depois do `save_file` | 0 |
| `unzip -p … document.json` (frames e componente novos presentes) | 0 |
| `FigCraft.AppImage cli export <Ward.figcraft> --node <id>` (6 PNG, lidos do disco) | 0 |
| `make check` | não rodado (ver "Ficou de fora") |

## Problemas descobertos
- **Escopo.** `docs/escopo.md` não cita o botão "Carregar de arquivo" nem o estado
  "chave carregada". Hoje a feature está só no plano (53a), no `docs/migracao.md` e no
  app. Registro sem escolher um lado: falta uma linha no escopo, ou a decisão fica no
  plano.
- **Abas velhas no MCP headless.** A sessão do `figcraft` tinha 4 abas "Ward" abertas,
  e uma delas (índice 1) tinha mudanças antigas não salvas. O `open_file` no original
  volta para essa aba e não relê o disco. Por isso conferi pelo `unzip` e pelo export da
  CLI, não pelo `open_file`. Quem salvar daquela aba sobrescreve esta fase.
- **Ajuda da chave em `8981`.** A chave salva mostra "Chave ed25519 do servidor.", mas
  o app mostra "Revelar e copiar não pedem PIN." no estado salvo. É anterior a esta fase
  e não foi mexido.
- **Máscara do `Campo secreto` oculto.** O componente tem 12 pontos e o app 8. Nas
  instâncias desta fase troquei para 8 por override (`21001`, `22245`). O componente
  não foi mudado, para não alterar as outras telas.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `/home/jeffersonoliveira/documentos/Ward.figcraft` | alterado (fora do repositório; sha256 novo `7efbd69b…`) |
| `docs/mission/design-nodes.md` | alterado |
| `docs/mission/evidence/fase-53c/*.png` (6) | criado |
| `docs/mission/handoffs/phase-53c.md` | criado |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): não rodado, a fase não toca código e o disco está apertado
- [x] Só cofre de teste e credenciais falsas usados
- [x] Teste de ataque para cada (SEG) tocado (nenhum SEG tocado em código)
- [x] (UI/DESIGN) Conferido contra o FigCraft (fase DESIGN: `save_file` feito)
- [x] Ficou dentro do escopo da fase
