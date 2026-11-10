# Roteiro da janela nativa (M10)

Faça no seu terminal e nas suas janelas, com o app instalado. Nada disto vai para a conversa com o Claude.

1. Abra o app do Ward e destrave com a senha mestra. Esperado: lista de credenciais, "Cofre destravado".
2. Nova credencial, tipo servidor, Autenticação "Chave". Esperado: campo "Chave privada" com o botão "Carregar de arquivo" (ícone de clipe) e a ajuda "Cole a chave ou use Carregar de arquivo...".
3. Clique em "Carregar de arquivo" e cancele a janela de escolha. Esperado: nada muda, a caixa de texto continua.
4. Carregue um arquivo que não é chave (um .txt qualquer). Esperado: aviso vermelho "o arquivo não é uma chave privada OpenSSH ou PEM"; a caixa continua.
5. Carregue uma chave de teste OpenSSH ou PEM (nunca uma real). Esperado: a caixa vira 8 pontos e "Chave carregada do arquivo (N bytes). Ela fica oculta e vai para o cofre ao salvar."; sem revelar nem copiar.
6. Preencha nome, grupo, ambiente, host e usuário e salve. Esperado: entrada na lista; ao abrir, a chave só aparece com "Revelar Chave privada" e some com "Ocultar".
7. Selecione um arquivo da pasta em memória `/run/user/<uid>/migracao/` no passo 5 (se o seletor nativo mostrar essa pasta). Esperado: carrega igual; depois apague o arquivo com `rm` do guia.
8. Trave pelo botão "Travar agora" ou pela bandeja. Esperado: volta à tela de destravar; o app não mostra a chave.
9. Leia `docs/migracao.md` com o KeePassXC ainda instalado e siga cada "Conferir" antes de avançar; registre "Chrome manteve as senhas" no `plan.md` só depois do passo 6 (V10.4).
