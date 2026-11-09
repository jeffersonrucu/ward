# Roteiro da janela nativa — Milestone 7 (backup)

Use só o cofre de teste (`make dev-cofre`, senha `falso-mestra-ward-01`, PIN `135790`).

1. Abra o app e destrave com a senha mestra. Esperado: lista de 22 entradas.
2. Edite uma entrada (por exemplo, as tags de `gama-nota`) e salve duas vezes. Esperado: cada salvar gera uma versão nova.
3. Configurações › Backup › Restaurar. Esperado: lista de até 30 versões, da mais nova para a mais antiga; "Continuar" só habilita com uma versão escolhida.
4. Escolha uma versão antiga e continue. Digite uma senha errada: "Essa senha não abre esta versão. Nada mudou no cofre atual."
5. Com a senha certa e PIN errado: "PIN incorreto. Nada foi alterado."
6. Com senha e PIN certos, "Restaurar esta versão". Esperado: volta à lista com as entradas da versão (as edições posteriores somem); o cofre atual fica como backup.
7. Teclado: Tab percorre fechar, versões e Cancelar dentro do diálogo; Esc fecha e o foco volta ao botão "Restaurar".
8. Configurações › Backup externo em pendrive: ligue com um pendrive de teste (imagem em loop) montado e confira que a pasta `ward-backup/` recebe as versões; com outro pendrive, nada é copiado. Sem udisks, o app mostra "Não deu para ver os pendrives".
9. Configurações › Kit de emergência › Imprimir. Esperado: folha A4 com cofre, backups, pendrive e linhas em branco para a senha; imprima em PDF (o `window.print()` no WebKitGTK real ainda não foi conferido) e confira que não há senha nem PIN.
10. Trave pela bandeja e destrave de novo: a versão restaurada abre com a senha dela.
