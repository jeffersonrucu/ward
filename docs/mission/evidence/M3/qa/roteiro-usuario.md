# Roteiro da janela nativa (M3: daemon + app)

O M3 não tem janela nativa nova; os passos abaixo valem quando o app Tauri abrir (M5). No M3, use `make dev-daemon` e `make app-ponte`.

1. Suba o daemon de teste e abra o app. Esperado: tela "Cofre travado", sem lista.
2. Digite uma senha errada. Esperado: "Senha mestra incorreta."; sem dizer quantas tentativas restam; a espera cresce a cada erro (até 30 s).
3. Digite a senha de teste. Esperado: lista de credenciais; `ward lista` na CLI mostra as mesmas (sem `gama-bloqueada`).
4. Abra `acme-pg-dev`, clique no olho: a senha aparece sem PIN; oculte. Esperado: nada fica no texto da página depois de ocultar.
5. Clique em Copiar na senha. Esperado: copia (no app nativo; na ponte de teste o copiar falha por falta de área de transferência, limitação anotada).
6. Crie uma entrada genérica com 1 campo extra. Esperado: nasce "Com aprovação"; rodapé mostra "Criada em / Alterada em".
7. Mude o nível para "Livre" com PIN errado. Esperado: "PIN incorreto. Nada foi alterado."; com o PIN de teste, "Nível de acesso alterado."
8. Erre o PIN 5 vezes seguidas. Esperado: o cofre trava e `ward lista` responde "travado".
9. Clique em "Travar agora". Esperado: volta a tela de destravar; `ward run` responde "travado".
10. Rode `ward run acme-pg-aprovacao -- env` com o app aberto. Esperado (M5): janela de aprovação com PIN; no M3 não há janela e o pedido expira em 2 min.
