# Roteiro do usuário: `ward login` com a janela de aprovação (M6)

Este roteiro cobre o V6.11 e o V6.17 (UI), que o QA automático não alcança: o
`make teste-login` usa uma janela de teste (`wardd-aprovador-teste`, Fase 44c) e nunca
abre a janela de aprovação de verdade (scrutiny do M6, Fase 44a). Aqui
`ward login acme-wp-dev` (nível "com aprovação") abre a janela nativa do app; negar não
preenche nada, permitir entra no WordPress de teste. Como `localhost:8081` é `http`, a
janela mostra a origem, e mesmo a entrada livre `acme-wp-prod` pede a confirmação dela
(D9, Fase 44c).

Use só o cofre de teste e os valores falsos. Senha mestra `falso-mestra-ward-01`, PIN
`135790`. O Chrome roda com perfil e HOME próprios em `.dev/roteiro-m6/`: nada daqui
toca no seu perfil do Chrome, no cofre real, no KeePassXC, no `~/.ssh` ou no `~/.aws`.

Marque cada passo com `[x]` quando o resultado for o esperado. Se algo divergir, anote
o número do passo e o que apareceu.

## 0. Preparar (uma vez)

Rode tudo da raiz do repositório.

1. `make dev-cofre`, `make dev-variacoes` e `make dev-servicos-subir`.
   - Esperado: o cofre de teste com 22 entradas e o WordPress em `http://localhost:8081`.
2. `make app-front` e `make extensao`.
   - Esperado: saem 0; a extensão fica em `extensao/dist/`.
3. Compile o app, a CLI e o `ward-nm` de teste:
   - `cd app/src-tauri && ~/.cargo/bin/cargo build --locked --features modo-teste && cd -`
     (precisa de `libwebkit2gtk-4.1-dev`; sem ela, o `PKG_CONFIG_PATH` do R32)
   - `~/.cargo/bin/cargo build --locked -p ward --features modo-teste --bin ward`
   - `~/.cargo/bin/cargo build --locked -p ward-nm --features modo-teste --bin ward-nm`
4. Se outro daemon de teste estiver no ar, pare antes: `make dev-daemon-parar`.
5. Suba o daemon de teste com o app nativo e o papel extensão do modo de teste:
   `WARD_DEV_EXTENSAO=chrome make dev-daemon WARD_DEV_APP=$PWD/app/src-tauri/target/debug/ward-app`
   - Esperado: "daemon de teste no ar".
6. Perfil do Chrome só deste roteiro, com o manifesto de native messaging apontando
   para o `ward-nm` de teste:

   ```sh
   mkdir -m 700 -p .dev/roteiro-m6/home .dev/roteiro-m6/perfil/NativeMessagingHosts
   printf '{"name":"br.ward.nm","description":"Ward (teste)","path":"%s","type":"stdio","allowed_origins":["chrome-extension://nhkehjnmlgkebhmelajfpmpblepaichf/"]}\n' \
     "$PWD/target/debug/ward-nm" > .dev/roteiro-m6/perfil/NativeMessagingHosts/br.ward.nm.json
   ```
7. Abra esse Chrome (janela visível, HOME e perfil de `.dev/roteiro-m6/`):

   ```sh
   env HOME="$PWD/.dev/roteiro-m6/home" /opt/google/chrome/chrome \
     --user-data-dir="$PWD/.dev/roteiro-m6/perfil" --password-store=basic \
     --no-first-run --no-default-browser-check http://localhost:8081/wp-login.php &
   ```
8. Nesse Chrome, `chrome://extensions` › ligue "Modo do desenvolvedor" › "Carregar sem
   compactação" › escolha `extensao/dist/`.
   - Esperado: a extensão "Ward" aparece com o ID `nhkehjnmlgkebhmelajfpmpblepaichf`.
     Outro ID: pare aqui e anote (o `ward-nm` só aceita esse).
9. Abra o app e destrave:
   `python3 -I -B dev/abrir-app.py /run/user/$(id -u)/ward-teste/wardd.sock`, senha
   `falso-mestra-ward-01`.
   - Esperado: a lista com as 22 entradas.

## 1. Negar (V6.11)

1. [ ] Deixe a aba `localhost:8081/wp-login.php` ativa no Chrome do passo 0.7 (clique
   nela para essa janela ser a última em foco) e, no terminal, rode:
   `target/debug/ward login acme-wp-dev`
   - Esperado: a janela "Pedido de aprovação · Ward" aparece na frente, com
     `acme-wp-dev`, o pedido de login, a origem `http://localhost:8081` em destaque e o
     foco em "Negar".
2. [ ] Clique em "Negar".
   - Esperado: o `ward login` termina com a linha `ward: pedido negado` e sai 1.
   - Esperado: na aba, usuário e senha continuam **vazios** e a página não mudou
     (nenhum envio). Confira também que o popup da extensão não mostrou nada novo.
3. [ ] Repita o passo 1 e aperte Esc na janela de aprovação.
   - Esperado: nega, campos vazios.
4. [ ] Repita o passo 1 e feche a janela de aprovação pelo X.
   - Esperado: nega, campos vazios.

## 2. Permitir (V6.11, contraprova)

1. [ ] Com a mesma aba em `wp-login.php`, rode de novo `target/debug/ward login acme-wp-dev`,
   digite o PIN `135790` e clique em "Permitir uma vez".
   - Esperado: o `ward login` sai 0 e a aba chega ao painel do WordPress
     (`/wp-admin/`) logado como `wp_admin`.
   - Esperado: o terminal não mostra usuário nem senha.
2. [ ] Saia do WordPress (menu do usuário › "Sair"), volte a `wp-login.php` e rode
   o `ward login acme-wp-dev` mais uma vez, digitando um PIN errado (`000000`).
   - Esperado: nada é permitido; negue em seguida e confira os campos vazios.

## 2a. Confirmar a origem numa entrada livre (V6.17)

1. [ ] Volte a `wp-login.php` (saia do WordPress, se preciso) e rode
   `target/debug/ward login acme-wp-prod` (nível livre).
   - Esperado: a janela abre com `acme-wp-prod`, a origem `http://localhost:8081` e o
     aviso de que a origem é `http`. Não há campo de PIN; os botões são "Negar" e
     "Confirmar a origem".
2. [ ] Clique em "Negar".
   - Esperado: `ward: pedido negado`, sai 1, campos vazios.
3. [ ] Rode de novo e clique em "Confirmar a origem".
   - Esperado: sai 0 e a aba chega ao painel como `wp_editor`.
4. [ ] Saia do WordPress, volte a `wp-login.php`, abra o popup da extensão e clique em
   `acme-wp-prod`.
   - Esperado: o popup diz que espera a janela do Ward e fecha quando ela pega o foco;
     a janela mostra "A extensão do Chrome quer preencher uma senha", a origem e "Confirmar a origem". Negar:
     campos vazios. Repetir e confirmar: usuário e senha preenchidos.
## 3. Logs sem segredo (V6.16)

1. [ ] `grep -c -F -f .dev/variacoes.txt .dev/logs/wardd.log`
   - Esperado: `0`.

## 4. Encerrar

1. Feche o Chrome do roteiro e o app.
2. `make dev-daemon-parar`.
3. `rm -rf .dev/roteiro-m6`.
4. Se ninguém mais usa os serviços de teste: `make dev-servicos-parar`.
