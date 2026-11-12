# Roteiro do usuário: instalação real do Ward (M9)

Quem roda é você, na sua máquina. A missão não usa `sudo`. Detalhes em
`docs/instalacao.md`; aqui vai a ordem e o resultado esperado de cada passo.

Conferido nesta máquina (só leitura, 2026-11-09): `ptrace_scope=1`, `suid_dumpable=2`,
`group: files systemd` no nsswitch, `sshpass` instalado, Chrome em `/opt/google/chrome`,
`libwebkit2gtk-4.1-dev` instalado, `/etc/opt/chrome/policies/managed/` ainda não existe,
nenhum grupo `ward-app`/`ward-nm`. `target/release/` e `target/extensao/` ainda não existem.

1. **Pré-checagens (0.1, 0.4, 0.5).** Rode os `cat` do kernel, o `ls` de
   `~/.config/google-chrome/NativeMessagingHosts/` e o `grep` de políticas.
   Esperado: ptrace 1+, dumpable 0 ou 2; sem `br.ward.nm.json` no perfil; `grep` sem arquivos.
   Aqui o perfil tem 1password, claude_code e keepassxc.
2. **Retrato do SSH (0.2).** Gere `~/ward-antes/*` (quatro arquivos). Esperado: arquivos
   criados, `ambiente.txt` pode ficar vazio.
3. **Clone limpo e build (0.3, 1).** `git clone` para `~/ward-instalar`, copie a chave com
   `install -m 0600 .dev/extensao/chave.pem`, rode `make release`, `make extensao`,
   `make extensao-crx`. Esperado: termina 0 e imprime `EXTENSAO_ID=… EXTENSAO_CRX=…` com o
   id `nhkehjnmlgkebhmelajfpmpblepaichf`.
4. **Instalar (2a ou 2b).** `make deb …` e `sudo apt install /root/ward/ward_*.deb`
   (ou `sudo make install …`). Esperado: `dpkg-deb -c` com 28 caminhos root/root sem `s`;
   `dpkg-statoverride --list | grep ward` mostra os dois binários 2755.
5. **systemd (3).** Rode os comandos do passo 3, depois reinicie a sessão e repita.
   Esperado: socket `enabled`/`active`, `/run/ward` e `/run/ward/<uid>` `root:root 755`,
   `ward destravar` sobe o `wardd` como você, sem sudo.
6. **Chrome (4).** Feche e abra o Chrome. Esperado: em `chrome://policy` o
   `ExtensionSettings` com `force_installed` e `NativeMessagingUserLevelHosts=false`, sem
   erro; em `chrome://extensions` a extensão "instalada pela sua organização"; "Carregar sem
   compactação" de `extensao/dist/` recusado; popup lista sugestões num site de teste. Se a
   extensão não aparecer (`file://` no force-install não está confirmado), anote as duas
   telas e traga para a missão.
7. **Outros apps (4a).** Para cada manifesto, cole `conferir_host`. Esperado nesta máquina:
   `com.1password.1password` e `org.keepassxc.keepassxc_browser` dão `ok:`, o do Claude in
   Chrome (host em `~/.claude/chrome/`) dá `NÃO COPIE` e fica sem ponte. Depois da cópia,
   `ls -l /etc/opt/chrome/native-messaging-hosts/` mostra `root root 644`.
8. **SSH (5).** Os quatro `diff` saem vazios e `git push` funciona como antes.
9. **Uso (aprovar com PIN).** Destrave no app, `ward run`/`ward login` com uma entrada
   real; a aprovação pede o PIN; negar ou deixar expirar não entrega nada.
10. **Desinstalar (6), se quiser.** `sudo dpkg -r ward`. Esperado: sem `wardd@*`, sem
    `/run/ward`, grupos criados pelo Ward removidos, cofre em `~/.local/share/ward/`
    intacto, manifestos copiados no passo 7 continuam.
