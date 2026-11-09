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
4. Tire a skill `cofre` antiga só quando o KeePassXC sair (docs/escopo.md › "Skill `ward`").

## O que as regras cobrem

- `Read`/`Edit` (valem para todas as ferramentas internas que leem ou editam
  arquivos): o cofre e o estado (`~/.local/share/ward/**`), a configuração
  (`~/.config/ward/**`), o diretório de execução com socket, anexos temporários e
  sessões do Playwright (`/run/user/*/ward/**`, escrito `//run/…` porque é caminho
  absoluto), o backup no pendrive (`/media/**/ward-backup/**`) e o `~/.ssh`.
- `Bash`: cinto de segurança contra erro sem má intenção (RA3): comandos que citam
  esses caminhos, `/proc/*/environ`, `ward destravar` (a senha mestra nunca passa
  pelo agente), `ward ssh --token-fd` (só o botão Conectar do app usa) e, dentro de
  `ward run`, as formas mais comuns de imprimir variável ou anexo (`env`, `printenv`,
  `set -x`, `base64`, `xxd`…).

As regras de `Bash` comparam texto e são fáceis de contornar (outro nome de
ferramenta, um script). Elas **não** são a barreira: quem barra é o daemon (nenhuma
operação devolve segredo), a máscara de saída e a janela de aprovação, que mostra o
comando exato. As de `Read`/`Edit` valem para as ferramentas internas do Claude Code,
não para um `cat` dentro do `Bash`.
