# Roteiro do usuário: janela nativa do app (M5)

Este roteiro cobre o que só dá para conferir na janela de verdade, na sua sessão COSMIC
(Wayland): a janela abre, a aprovação aparece na frente, a bandeja funciona e o copiar
limpa em 30 s (V5.28). O QA automático roda no Chrome headless, pela ponte de teste, e
não alcança nada disso.

Use só o cofre de teste e os valores falsos. Senha mestra `falso-mestra-ward-01`, PIN
`135790`. Nada daqui toca no cofre real, no KeePassXC, no `~/.ssh` ou no `~/.aws`.

Marque cada passo com `[x]` quando o resultado for o esperado. Se algo divergir, anote
o número do passo e o que apareceu.

## 0. Preparar (uma vez)

Rode tudo da raiz do repositório.

1. `make dev-cofre` e `make dev-variacoes`.
   - Esperado: o cofre de teste lista as 22 entradas e `.dev/variacoes.txt` é gravado (0600).
2. `make app-front`.
   - Esperado: sai 0.
3. Compile o app de teste:
   `cd app/src-tauri && ~/.cargo/bin/cargo build --locked --features modo-teste && cd -`
   - Precisa de `libwebkit2gtk-4.1-dev`. Sem ela, use o `PKG_CONFIG_PATH` do R32.
4. Se outro daemon de teste estiver no ar, pare antes: `make dev-daemon-parar`.
5. Suba o daemon de teste com o app nativo:
   `make dev-daemon WARD_DEV_APP=$PWD/app/src-tauri/target/debug/ward-app`
   - Esperado: "daemon de teste no ar".
6. Compile a CLI de teste:
   `~/.cargo/bin/cargo build --locked -p ward --features modo-teste --bin ward`

## 1. O app abre

1. [ ] Abra o app: `python3 -I -B dev/abrir-app.py /run/user/$(id -u)/ward-teste/wardd.sock`
   - Esperado: a janela "Ward" abre em 1280×800, tema escuro, na tela `Destravar` ("Cofre travado").
2. [ ] Abra de novo com o mesmo comando, com a janela aberta.
   - Esperado: não abre uma segunda janela.

## 2. Primeiro uso

O daemon de teste exige um cofre (pendência da Fase 30), então o primeiro uso nativo
ainda não roda. Confira no Chrome, pela ponte, quando a pendência fechar. Pule esta
seção por enquanto.

## 3. Destravar

1. [ ] Digite uma senha errada qualquer e confirme.
   - Esperado: mensagem de senha incorreta, campo vazio e o cofre continua travado.
2. [ ] Digite `falso-mestra-ward-01` e confirme.
   - Esperado: a tela `Credenciais` aparece, com a navegação lateral à esquerda.
3. [ ] No terminal: `target/debug/ward lista`.
   - Esperado: lista as entradas, sem nenhuma senha.

## 4. Credenciais e copiar

1. [ ] Busque por `acme` e filtre por tipo e por ambiente.
   - Esperado: a lista filtra e os grupos continuam agrupados.
2. [ ] Abra `acme-pg-dev` e clique em revelar a senha.
   - Esperado: o valor aparece. Ao clicar em ocultar, some.
3. [ ] Clique em copiar a senha e cole num editor qualquer.
   - Esperado: cola o valor falso.
4. [ ] Espere 30 s e cole de novo.
   - Esperado: não cola mais nada. A área de transferência foi limpa.
5. [ ] Copie de novo, copie outro texto qualquer por cima antes dos 30 s e espere passar o prazo.
   - Esperado: o seu texto continua lá. O Ward só apaga o que ele mesmo copiou.

## 5. Formulários

1. [ ] Em "Nova credencial", escolha `login`, preencha os campos com valores falsos e salve.
   - Esperado: a entrada nova aparece na lista.
2. [ ] Abra `acme-wp-dev` e confira o código TOTP: ele muda a cada 30 s e copia.
3. [ ] Abra um `servidor` com aprovação (`acme-ssh-salto`) e clique em "Conectar".
   - Esperado: abre o terminal padrão já conectado (precisa dos serviços de teste no ar). Anote qual terminal abriu.
4. [ ] Num formulário, clique em "Adicionar arquivo".
   - Esperado: abre o diálogo do sistema. Anexar acima de 256 KiB mostra o aviso.

## 6. Nível de acesso com PIN

1. [ ] Em `acme-pg-dev`, mude o nível para "com aprovação".
   - Esperado: o app pede o PIN. Com `135790` o nível muda.
2. [ ] Repita com um PIN errado.
   - Esperado: mensagem de PIN errado e o nível não muda.
3. [ ] Volte o nível de `acme-pg-dev` para "livre".

## 7. Janela de aprovação em primeiro plano (COSMIC/Wayland)

1. [ ] Deixe o app destravado e ponha outra janela na frente (o terminal, por exemplo). No terminal, rode:
   `target/debug/ward run acme-pg-aprovacao -- true`
   - Esperado: a janela "Pedido de aprovação · Ward" aparece **na frente** de todas, com o foco em "Negar".
   - Se ela abrir atrás, ou só piscar na barra, anote. É o ponto que mais importa no COSMIC.
2. [ ] Aperte Enter sem digitar o PIN.
   - Esperado: nada é permitido.
3. [ ] Aperte Esc.
   - Esperado: nega, e o `ward run` termina com "negado".
4. [ ] Rode o mesmo `ward run`, digite `135790` e clique em "Permitir uma vez".
   - Esperado: o comando roda e termina.
5. [ ] Rode de novo e feche a janela pelo X.
   - Esperado: nega.
6. [ ] Rode de novo e não responda.
   - Esperado: o aviso de expirando aparece perto do fim, e em 2 min o pedido é negado.

## 8. Gerador de senha

1. [ ] Abra "Gerador de senha", mude o tamanho (de 8 a 64) e desligue os símbolos.
   - Esperado: a força acompanha.
2. [ ] Clique em "Gerar de novo", revele e copie.
   - Esperado: o valor cola e some em 30 s, como no passo 4.4.

## 9. Configurações

1. [ ] Abra "Configurações".
   - Esperado: trocar o PIN pede o PIN atual; trocar a senha mestra pede a atual; o tempo de trava mostra 4 h; o backup externo aparece desligado.
   - Se a tela ainda disser "Esta tela ainda não existe", a Fase 38 não entrou: pule.

## 10. Bandeja (V5.23)

1. [ ] Olhe a área de status do painel do COSMIC.
   - Esperado: aparece o ícone do Ward (escudo cheio), com o cofre destravado.
   - Se não aparecer, confira se o applet "Área de status" (Status Area) está no painel e anote.
2. [ ] Clique no ícone.
   - Esperado: menu com "Cofre destravado", "Trava sozinho após o tempo sem uso" (os dois apagados), "Abrir o Ward", "Travar agora" e "Sair".
3. [ ] Clique em "Travar agora" (pelo menu da bandeja).
   - Esperado: em até 2 s a janela vai para `Destravar` e o ícone troca para o de travado (escudo em contorno).
   - Esperado também: `target/debug/ward lista` responde "travado".
4. [ ] Clique no ícone de novo.
   - Esperado: menu com "Cofre travado", "Destrave com a senha mestra", "Abrir o Ward", "Destravar…" e "Sair".
5. [ ] Feche a janela principal pelo X.
   - Esperado: a janela some, o ícone continua na bandeja e o processo `ward-app` continua vivo.
6. [ ] No menu da bandeja, clique em "Destravar…".
   - Esperado: a janela volta na tela `Destravar`. Destrave com a senha de teste.
7. [ ] Feche a janela de novo e, no menu, clique em "Abrir o Ward".
   - Esperado: a janela volta.
8. [ ] Com o cofre destravado, rode no terminal `target/debug/ward travar`.
   - Esperado: em até 2 s o ícone e o menu mudam para travado, e a janela vai para `Destravar`.
9. [ ] Feche a janela, deixe o cofre travado e rode `target/debug/ward run acme-pg-dev -- true`.
   - Esperado: a janela principal volta sozinha pedindo a senha (pedido do agente). Destrave e o comando segue.
10. [ ] Use "Travar agora" da navegação lateral (dentro da janela).
    - Esperado: vai para `Destravar` e o ícone da bandeja muda para travado.
11. [ ] No menu da bandeja, clique em "Sair".
    - Esperado: o app fecha e o ícone some da bandeja.

## 11. Varredura de logs (V5.27)

1. [ ] Depois de tudo, rode `make teste-logs-app`.
   - Esperado: o autoteste passa e todo arquivo de `.dev/logs/` (`wardd.log`, `ward-app.log` e o resto) sai como "limpo", "0 com segredo".
2. [ ] Opcional: abra o inspetor da janela (o app de teste tem devtools) e confira que o console não mostra nenhuma senha. Isso cobre o console da webview.

## 12. Encerrar

1. `make dev-daemon-parar`.
2. Responda ao orchestrator com os passos que falharam, ou com "roteiro confirmado".
