# Skill `ward` e regras de deny

Esta pasta tem o que o usuário instala no Claude Code no M10 (troca da skill `cofre`).
Nada aqui é instalado por `make`: a instalação é manual, feita pelo usuário.

| Arquivo | Vai para |
| --- | --- |
| `ward/SKILL.md` | `~/.claude/skills/ward/SKILL.md` |
| `settings-deny.json` | as regras de `permissions.deny` entram no `~/.claude/settings.json` |

## Instalar (M10, pelo usuário)

1. Copie a pasta `ward/` para `~/.claude/skills/ward/`.
2. Junte a lista `permissions.deny` de `settings-deny.json` à lista
   `permissions.deny` que já existe no `~/.claude/settings.json` (não substitua o
   arquivo; acrescente as regras que faltam).
3. Confira com `/permissions` no Claude Code que as regras aparecem em "Deny".
   Confira também que uma regra com `*` no início nega de fato: peça ao Claude para
   rodar `/usr/local/bin/ward run entrada-que-nao-existe -- env` (um nome que não
   está no cofre, para nada vazar mesmo se a regra falhar) e veja a recusa da
   permissão, sem o comando rodar. Se o comando rodar (aparece "entrada
   inexistente"), as regras com `*` no início não valem na sua versão do Claude
   Code: avise antes de seguir a migração.
4. Tire a skill `cofre` antiga só quando o KeePassXC sair (docs/escopo.md › "Skill `ward`").

## Destravar: só você, pelo caminho completo

A skill nunca destrava (a regra `*ward*destravar*` nega). Quando o Claude avisar que o
Ward está travado, destrave pelo app ou, num terminal seu (fora do Claude Code, sem
`!`), com `/usr/local/bin/ward destravar`, nunca com o `ward` que o `PATH` acha. Um
programa malicioso rodando como você pode pôr um `ward` falso antes no `PATH` (em
`~/.local/bin`, por exemplo), que mostra o mesmo pedido e guarda a senha mestra. O
caminho completo pula o `PATH`; não protege de um `~/.bashrc` ou `~/.zshrc` adulterado
(uma função do shell, ou um alias do zsh, pode ter o nome `/usr/local/bin/ward`), caso
que fica fora do que o Ward cobre (docs/escopo.md › "Fora do modelo de ameaça", RA8).

## O que as regras cobrem

- `Read`/`Edit` (valem para todas as ferramentas internas que leem ou editam
  arquivos): o cofre e o estado (`~/.local/share/ward/**`), a configuração
  (`~/.config/ward/**`), o diretório de execução com socket, anexos temporários e
  sessões do Playwright (`/run/user/*/ward/**`, escrito `//run/…` porque é caminho
  absoluto), o backup no pendrive (`/media/**/ward-backup/**` e
  `/run/media/**/ward-backup/**`, conforme a distribuição monta o pendrive) e o
  `~/.ssh`.
- `Bash`: cinto de segurança contra erro sem má intenção (RA3). Todas começam com
  `*` (sem âncora), para valer também com `/usr/local/bin/ward run`, depois de um
  `cd … &&` ou com espaço a mais. Negam:
  - comandos que citam os caminhos acima, inclusive com curinga (`~/.local/share/w*`,
    `~/.local/*/w?rd`), por variável (`$XDG_DATA_HOME`, `$XDG_RUNTIME_DIR`) ou depois
    de `cd`; qualquer comando com `/run/user`, `/run/media` ou `/proc/` e qualquer
    texto com `environ`;
  - `ward destravar` (a senha mestra nunca passa pelo agente) e `ward ssh --token-fd`
    (só o botão Conectar do app usa);
  - dentro de `ward run`: `env`/`printenv`, `set`, `export`, `declare`, `sh -x`,
    `printf`, `echo … $`, `base64`, `base32`, `xxd`, `od`, `hexdump`, `rev`, `tr`,
    `cut`, `fold`, `head -c`, `tee`, `python… -c`, `node -e`, `perl -e`, `ruby -e`,
    `php -r`, qualquer `>` (redirecionamento), `.pgpass`, `.netrc`, `git config` e
    `cat`/`head`/`cp`/`mv`/`ln`/`<` de `WARD_ARQUIVO_*`.

  `make teste-skill` confere cada forma (o comando inteiro e cada subcomando) e que
  nenhum exemplo da skill é negado. Algumas regras negam comando inocente (um
  `update … set` do `psql` dentro do `ward run`, um `2>&1`, um pipe para `cut` depois
  do `ward run`, um `ls ~/.local/share/fonts`). É de propósito: na dúvida, nega. A
  skill manda o Claude parar e avisar quando um comando for negado, sem reescrever
  para passar.

As regras de `Bash` comparam texto e são fáceis de contornar (outro nome de
ferramenta, um script gravado antes, um curinga que nenhuma regra previu). Elas
**não** são a barreira: quem barra é o daemon (nenhuma operação devolve segredo), a
máscara de saída e a janela de aprovação, que mostra o comando exato. As de
`Read`/`Edit` valem para as ferramentas internas do Claude Code, não para um `cat`
dentro do `Bash`.

## Nível "livre" = o Claude consegue ler o segredo

Uma entrada "livre" (ou que virou "livre" com "Permitir sempre") é usada pelo Claude
sem perguntar. Na prática, isso quer dizer que ele **consegue** ler o segredo dela,
por engano ou por uma instrução maliciosa: basta um comando que recodifique a
variável de um jeito que a máscara não cobre (RA3), e a skill e as regras acima só
pegam os casos comuns. Deixe "livre" só a credencial que você aceitaria que o Claude
visse. Para as outras, use "com aprovação": a janela mostra o comando exato antes de
entregar.
