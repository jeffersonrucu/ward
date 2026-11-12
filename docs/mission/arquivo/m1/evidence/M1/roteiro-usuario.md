# Roteiro do usuário — Milestone 1 (Fundação)

O M1 não tem janela nativa nem interface; o roteiro confere a fundação pelo terminal.

1. Na raiz do repositório, rode `make check`. Esperado: seis etapas (fmt, clippy, test, audit, deny, gitleaks) e a linha final "make check: tudo limpo", saída 0.
2. Rode `make teste-home-real`. Esperado: todas as linhas "ok" e "Nenhum cliente de teste abre caminho do home real".
3. Rode `make dev-servicos-conferir` com os serviços de pé. Esperado: todas as linhas "ok" e "Serviços de teste ok (V1.4)".
4. Rode `make teste-ataques-fundacao`. Esperado: "Todos os ataques foram barrados."
5. Crie um `vazamento.txt` com uma chave de teste (`ssh-keygen -t ed25519 -N '' -f /tmp/chave-qa && cp /tmp/chave-qa vazamento.txt`) e rode `make check`. Esperado: falha apontando `vazamento.txt`; apague o arquivo e a chave, e `make check` volta a sair 0.
6. Rode `git check-ignore .dev/qualquer`. Esperado: o caminho é impresso (ignorado).
7. Abra `CLAUDE.md` > "Comandos". Esperado: build, `make check`, serviços de teste, senha mestra `falso-mestra-ward-01` e PIN `135790`; `make dev-cofre` e o daemon aparecem como "Fase 19"/"Milestone 3".
