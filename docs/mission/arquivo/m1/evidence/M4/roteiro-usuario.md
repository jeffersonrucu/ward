# Roteiro do usuário — Milestone 4 (CLI e integração Claude)

O M4 não tem tela nova. Estes passos conferem, no seu terminal e com o cofre de teste
(`.dev/`, senha `falso-mestra-ward-01`), o que o Claude vai usar. Antes: `make dev-cofre`,
`make dev-servicos-subir` e `make dev-daemon`. Nunca use o cofre real.

1. `target/debug/ward lista` com o cofre travado. Esperado: aviso "travado", sai com código diferente de 0, nenhum nome de entrada.
2. `target/debug/ward destravar`. Esperado: pede a senha sem eco; digitando a de teste, destrava; com senha errada, recusa sem mostrar a senha.
3. `target/debug/ward lista`. Esperado: 21 entradas (sem `gama-bloqueada`), colunas nome, tipo, grupo, ambiente, projeto, URL/host, nenhuma senha. `lista --json` tem só esses campos.
4. `target/debug/ward run acme-pg-dev -- sh -c 'echo $PGPASSWORD $DATABASE_URL'`. Esperado: só `••••` na tela.
5. `target/debug/ward run acme-pg-dev -- sh -c 'exit 7'; echo $?`. Esperado: imprime 7.
6. `target/debug/ward ssh acme-ssh-senha-stage whoami`. Esperado: `deploy`. Repita com `acme-ssh-chave-prod` (deploy) e `acme-ssh-interno hostname` (pelo salto).
7. `target/debug/ward ssh acme-ssh-senha-stage` (sem comando), digite `whoami`, depois `exit`. Esperado: `deploy` e a CLI sai com 0.
8. Com o app fechado: `target/debug/ward run acme-pg-aprovacao -- touch /tmp/marca-ward`. Esperado: "aprovação indisponível", sai ≠ 0 e `/tmp/marca-ward` não existe. Com o app aberto (janela nativa), a janela de aprovação aparece e o PIN `135790` libera.
9. `target/debug/ward ssh --token abc acme-ssh-senha-stage true`. Esperado: recusa "o token do Conectar não vai na linha de comando".
10. `target/debug/ward travar` e depois `ward run acme-pg-dev -- true` num terminal e `ward destravar` em outro dentro de 2 min. Esperado: o primeiro segue e sai 0. Ao fim, `make dev-daemon-parar`.
