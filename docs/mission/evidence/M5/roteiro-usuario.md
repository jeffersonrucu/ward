# Roteiro da janela nativa (M5), curto

Só cofre de teste. Senha mestra `falso-mestra-ward-01`, PIN `135790`. Preparo e comandos completos em
`docs/mission/roteiro-usuario.md` (seção 0). O que o QA no Chrome não alcança:

1. Abra o app pelo daemon (`dev/abrir-app.py`). Esperado: janela "Ward" 1280×800, escura, na tela "Cofre travado"; abrir de novo não cria segunda janela.
2. Destrave com a senha mestra. Esperado: "Credenciais" com 22 entradas.
3. Em outro terminal: `ward run acme-pg-aprovacao -- touch .dev/marca`. Esperado: a janela de aprovação aparece NA FRENTE (foco inicial em "Negar") com entrada, comando e cadeia de processos.
4. Digite o PIN e clique "Permitir uma vez". Esperado: o comando roda, `.dev/marca` existe, a janela fecha.
5. Abra `acme-pg-dev`, clique "Copiar" na senha e cole num editor. Esperado: o valor cola; aparece o toast de sucesso (no Chrome de teste o copiar falha de propósito, só o nativo copia).
6. Espere 30 s e cole de novo. Esperado: a área de transferência não tem mais a senha. Repita copiando outro texto antes dos 30 s: o outro texto fica.
7. Revele a senha e tente selecionar com o mouse e dar Ctrl+C (pendência da Fase 39g). Esperado ideal: nada vai para a área de transferência; hoje pode ir (anote).
8. Bandeja: "Travar agora" no menu. Esperado: o app vai para "Cofre travado" e `ward lista` diz "travado"; "Abrir" mostra a janela de novo.
9. Em `acme-ssh-salto` (Editar), clique "Conectar". Esperado: abre o terminal já conectado, sem janela nem PIN.
10. Cometa 5 PINs errados na janela de aprovação. Esperado: cofre trava, pedido negado, janela mostra "PIN bloqueado"; feche a janela e destrave com a senha.
