# Roteiro do usuário — Milestone 0 (aprovação do design, V0.23)

M0 é só design: não há app nem janela nativa ainda. O roteiro é no FigCraft, com
`/home/jeffersonoliveira/documentos/Ward.figcraft`. Os PNGs em
`docs/mission/evidence/M0/layout/` (1x) servem para uma primeira olhada.

1. Abra o `Ward.figcraft`, página **Layout**. Esperado: 51 frames, todos no tema escuro, sem botão de tema.
2. Veja `Credenciais / Cheia` (4063) ao lado de `docs/design/referencias/alvo-layout-app.png`. Esperado: três painéis arredondados com respiro, logo e cartão "Cofre destravado" na lateral, busca em pílula, selos pequenos e em minúsculo, bordas sutis.
3. Confira `Credenciais / Filtrada` (4410) e `Filtros` (5259). Esperado: busca ativa, "Filtros · 2", resumo com "Limpar"; o grupo aparece só como marcador colorido (decisão 10).
4. Abra `Nova credencial / Escolher tipo` (17657) e `Formulário / login / Novo` (7450). Esperado: 9 tipos, grupo com autocompletar, ambiente obrigatório, nenhum segredo legível.
5. Abra `Formulário / servidor / Edição` (8981). Esperado: botão **Conectar** no cabeçalho, anexos listados com Remover.
6. Veja as 5 telas de Aprovação (16701, 16810, 16885, 16960, 17035). Esperado: entrada, ambiente, comando e cadeia de processos; quatro botões em todas, inclusive prod; foco em **Negar**; prod com faixa vermelha; PIN bloqueado com os três de permitir apagados.
7. Veja `Destravar / Padrão`, `/ Senha errada` e `/ Pedido do agente`. Esperado: só a senha mestra destrava (aviso de que o PIN não serve); espera visível; contagem "1:42 restantes" no pedido.
8. Veja Configurações (15242, 16178), Restaurar backup (16349, 16512) e o Kit (16773). Esperado: backup externo desligado e depois ligado com o pendrive; restauração pede a senha mestra da versão; kit A4 com linha em branco para a senha.
9. Veja Bandeja (17269, 17300) e Extensão (17332, 17392, 17414, 18850). Esperado: Destravado com abrir, travar agora e sair; Travado com abrir e destravar; "Salvar login novo" mostra tipo, grupo e ambiente.
10. Decida os pontos em aberto do handoff da Fase 10 (folha branca do kit, lista de tempo de trava, ícone da bandeja em 16 px) e registre a aprovação em `plan.md`.
