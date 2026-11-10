# Roteiro do usuário — Milestone 8 (skill `ward`)

Só leitura e conferência; nada aqui instala a skill (isso é o M10).

1. Abra `skill/ward/SKILL.md`. Esperado: o frontmatter tem `name: ward` e uma descrição que cita login, senha, SSH, banco e "teste Playwright".
2. Procure "KeePassXC" e "cofre-" no arquivo. Esperado: nenhuma ocorrência.
3. Leia a seção "Fluxo". Esperado: ordem listar, cruzar grupo/ambiente/tipo, avisar qual entrada vai usar, usar o comando.
4. Leia "Nunca faça". Esperado: proíbe imprimir variável de credencial (inclusive base64/hex), ler os caminhos do Ward e o `~/.ssh`, e imprimir anexo.
5. Leia a última seção. Esperado: `git push` e SSH com chave do `~/.ssh` são feitos direto, sem o Ward.
6. Abra `skill/settings-deny.json`. Esperado: regras `Read`/`Edit` para `~/.local/share/ward`, `~/.config/ward`, `//run/user/*/ward`, `//media/**/ward-backup` e `~/.ssh`.
7. Com o Claude Code aberto numa conversa de teste (skill ainda não instalada), peça "conecta no banco de teste do acme". Esperado sem a skill: nada do Ward acontece; a conferência com a skill foi feita pelo QA com o cofre de teste.
8. Quando instalar a skill (M10), peça os três pedidos de V8.3 e confira que ele avisa `acme-pg-dev`, `acme-ssh-senha-stage` e `acme-wp-prod` antes de rodar.
