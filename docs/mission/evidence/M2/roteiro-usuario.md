# Roteiro do usuário — M2

O M2 é só o núcleo: não há janela nativa, CLI nem daemon para testar à mão.
1. `make dev-cofre` — esperado: lista as 21 entradas e termina sem erro.
2. Rode de novo — esperado: mesmo resultado, `.dev/cofre-teste.ward` com modo 0600.
3. `make dev-cofre COFRE=/tmp/x.ward` — esperado: recusado ("dentro de .dev/").
4. `make test` — esperado: todos os testes passam.
