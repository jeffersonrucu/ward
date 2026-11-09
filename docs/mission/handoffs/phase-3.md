# Fase 3 — (FIGMA) Telas de destravar e primeiro uso

## Implementado
- Sessão do Figma no arquivo `bzu9PxxFur2tU3vlNCHvsY`. Inspirações (`1:3`) lida só por
  screenshot e pelo script de conferência da Fase 1, sem nenhuma escrita.
- Dois componentes novos na página **Design System** (`1:2`), dentro do frame
  `Seção / Telas de acesso` (`16:80`), com tudo ligado a variables e text styles:
  - `Painel de marca` (`16:81`), o painel esquerdo de todas as telas de acesso: logo
    provisório (cadeado sobre `primaria/padrao`, já que o ícone do produto é da Fase 9),
    "Ward", frase "Credenciais de trabalho, fora da conversa." e três garantias (offline;
    Argon2id + XChaCha20-Poly1305; uso sensível só com aprovação e PIN). Tem só texto
    fixo e nenhum dado do cofre.
  - `Indicador de passos` (set `16:123`), variante `Passo` = 1 (`16:105`), 2 (`16:111`) e
    3 (`16:117`): rótulo "Passo N de 3 · <nome>" e três barras.
- Seis frames na página **Layout** (`0:1`), cada um com 1280×800, modo `Escuro` explícito
  na coleção `Cores` e layout em duas colunas (instância do `Painel de marca` +
  formulário com instâncias da Design System):

| Frame | Node | Conteúdo |
| --- | --- | --- |
| `Primeiro uso / Criar cofre` | `17:85` | Passo 1; cartão com cifrado em repouso, "fica só nesta máquina" (`~/.local/share/ward/`), trava sozinho; botão `Criar cofre` |
| `Primeiro uso / Senha mestra` | `17:149` | Passo 2; `Campo` senha mestra (foco) e confirmação, ambos mascarados `••••`; barra de força; aviso de que a senha não tem recuperação (kit de emergência); `Voltar` / `Continuar` |
| `Primeiro uso / PIN` | `17:234` | Passo 3; `Campo PIN` completo + confirmação (digitando), "4 a 6 dígitos"; destaque "O PIN não destrava o cofre", que só aprova usos e mudanças de nível, e que 5 erros seguidos travam; `Voltar` / `Concluir` |
| `Destravar / Padrão` | `18:229` | "Cofre travado"; `Campo` senha mestra (vazio, ajuda "Só a senha mestra destrava. O PIN não serve aqui."); `Destravar`; nota da trava automática (bloqueio, suspensão, 4 h) |
| `Destravar / Senha errada` | `18:282` | `Campo` em erro "Senha mestra incorreta."; botão desabilitado "Tente de novo em 4 s"; nota "Cada tentativa errada aumenta a espera da próxima, até 30 s." (D20, sem contar as tentativas) |
| `Destravar / Pedido do agente` | `18:335` | Cartão "Pedido do agente" com contagem "1:42 restantes" (de 2 min), entrada `clientex / stage / banco` com `Badge tipo` banco e `Badge ambiente` stage, comando exato em mono (`ward run clientex-stage-banco -- psql -c "select count(*) from pedidos"`); `Campo` senha mestra (foco); `Agora não` / `Destravar`; notas: destravar não aprova o pedido (se a entrada pedir aprovação, a janela de aprovação abre depois, com PIN), e "Agora não", fechar ou deixar expirar nega |

- Auditoria por script nos 6 frames e na seção nova da Design System: nenhuma
  pendência. Todo preenchimento e borda sólidos estão ligados a variable de cor, toda
  espessura de borda a `linha/*`, todo padding, gap e raio maior que zero a variables,
  e todo texto tem text style. Os seis frames têm modo explícito `Escuro` (`6:2`).
  Instâncias por frame: Painel de marca, Indicador de passos, Campo, Campo PIN, Botão,
  Badge tipo, Badge ambiente e ícones.
- Conferência visual por screenshot de cada frame (texto sem corte nem sobreposição).
- Inspirações reconferida no fim com o script da Fase 1: os 4 filhos estão idênticos ao
  snapshot (id, posição, tamanho e `imageHash`).

## Segurança
- Invariantes tocadas: nenhuma S1–S15 diretamente, porque a fase é só de design. O
  desenho segue o escopo:
  - Senha mestra e PIN aparecem só mascarados (`••••` / `•`), e nenhum valor de exemplo
    é real (V0.20).
  - O campo de senha mestra usa `Campo` com máscara, não `Campo secreto`: o `Campo
    secreto` tem a ação "copiar", e copiar a senha mestra não deve existir.
  - R24: o PIN aparece explicitamente como algo que não destrava, e a tela de destravar
    diz que o PIN não serve ali.
  - D20: a tela de senha errada mostra a espera crescente, sem dizer quantas tentativas
    houve.
  - R22/S10: o pedido do agente mostra a entrada e o comando exato, com contagem de
    2 min. Destravar não aprova o pedido, e negar, fechar ou expirar nega.
- Testes de ataque: não se aplica, porque o M0 não tem asserção (SEG).
- Dependências novas: nenhuma.

## Ficou de fora
- Ícone do produto: o logo do `Painel de marca` usa o `Ícone/cadeado` como provisório.
  Quando a Fase 9 criar o ícone, basta trocar no componente.
- Ação "revelar" na digitação da senha mestra: o `Campo` não tem essa ação. Fica para
  decidir na Fase 31 (UI), se o usuário quiser.

## Comandos executados
| Comando | Exit code |
| --- | --- |
| `make check` | não executado: o `Makefile` ainda não existe (é criado na Fase 11) |
| `use_figma` criação dos componentes na Design System | ok |
| `use_figma` criação dos 6 frames no Layout | ok |
| `use_figma` auditoria de ligações (Layout e seção nova) | ok, 0 pendências |
| `use_figma` conferência de `1:3` com o script da Fase 1 | ok, idêntico |

## Problemas descobertos
- Falta na Design System um campo de digitação de senha com estados vazio, foco e erro
  e com a ação revelar (sem copiar). O `Campo secreto` só tem oculto, revelado e
  copiado, e sempre traz "copiar". Por isso usei `Campo` com valor mascarado e ícone de
  cadeado. Se o QA ou a Fase 10 quiserem um componente próprio (`Campo senha`), ele
  precisa ser criado e as telas trocadas.
- O contrato V0.6 diz "mostra a entrada e o comando que esperam". Não está definido se
  "Agora não" (negar na hora) existe no destravar. O escopo só diz que o pedido espera
  até 2 min. Incluí o botão porque fechar já nega (S10), mas a decisão é do
  orchestrator.
- A barra de força da senha mestra é só visual. Escopo e PRD não definem um mínimo para
  a senha mestra. No texto usei "Mínimo de 12 caracteres", que precisa ser confirmado
  ou ajustado no PRD antes da Fase 30.
- O texto "Tente de novo em 4 s" é só um exemplo de D20 (1 s, 2 s, 4 s… até 30 s).
- A coluna do formulário ficou com 592 px de largura (padding `espaco/64` dentro de
  720 px), porque não existe token de largura máxima. Já estava registrado na Fase 2 que
  não há token de largura.

## Arquivos criados/alterados
| Arquivo | Ação |
| --- | --- |
| `docs/mission/handoffs/phase-3.md` | criado |

Nodes no Figma:
| Item | Página | Id |
| --- | --- | --- |
| `Seção / Telas de acesso` | Design System | `16:80` |
| `Painel de marca` (componente) | Design System | `16:81` |
| `Indicador de passos` (set) | Design System | `16:123` (Passo=1 `16:105`, 2 `16:111`, 3 `16:117`) |
| `Primeiro uso / Criar cofre` | Layout | `17:85` |
| `Primeiro uso / Senha mestra` | Layout | `17:149` |
| `Primeiro uso / PIN` | Layout | `17:234` |
| `Destravar / Padrão` | Layout | `18:229` |
| `Destravar / Senha errada` | Layout | `18:282` |
| `Destravar / Pedido do agente` | Layout | `18:335` |

## Procedimentos
- [ ] `make check` rodado (fmt, clippy, testes, audit, deny, gitleaks): o `Makefile` só existe a partir da Fase 11
- [x] Só cofre de teste e credenciais falsas usados (nenhum cofre ou credencial tocado)
- [x] Teste de ataque para cada (SEG) tocado: nenhum (SEG) tocado
- [x] (UI/FIGMA) Conferido contra o Figma (screenshots, auditoria de ligações, Inspirações reconferida)
- [x] Ficou dentro do escopo da fase
