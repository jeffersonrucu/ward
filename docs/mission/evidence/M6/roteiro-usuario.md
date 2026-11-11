# Roteiro da janela nativa (M6, QA) — o que só você confere

O QA não abre a janela nativa (Tauri) nem a bandeja. O roteiro completo de instalação e
atualização está em `docs/mission/roteiro-usuario.md`; aqui só o que o QA não cobriu.

1. Abra o Ward pelo menu de aplicativos. Esperado: janela 1280×800 com barra de título
   escura, tela "Cofre travado" igual à 0.1.x.
2. Destrave com a senha mestra. Esperado: Credenciais com 21 ou 22 entradas, sem erro.
3. Abra uma entrada com TOTP. Esperado: "Código TOTP" em dois grupos de 3 dígitos, a
   contagem em segundos e **Copiar**; a semente não aparece (só em Editar).
4. Feche a janela e abra o Ward de novo pelo menu. Esperado: o app estava na bandeja e a
   janela principal volta (também com a janela minimizada, ganhando foco).
5. Menu da bandeja. Esperado: "Cofre destravado", **Travar agora** e **Sair**; ao
   clicar em Travar agora, o cofre trava e o menu diz "Cofre travado".
6. Rode `ward run <entrada-com-aprovacao> -- true` num terminal. Esperado: janela de
   aprovação com entrada, ambiente, comando exato e cadeia de processos; 4 botões; os
   três de permitir pedem o PIN; fechar nega.
7. Trave pelo bloqueio de tela (Super+L) e destrave a sessão. Esperado: o cofre travou
   (bandeja "Cofre travado"); um `ward run` espera o destrave por até 2 min.
8. Extensão: abra o Chrome e confira em `chrome://extensions` a 0.2.0 (id
   `nhkehjnmlgkebhmelajfpmpblepaichf`). Numa página de login do cofre (origem `https`),
   o ícone sugere a entrada certa; em página de outra origem, nada é sugerido.
