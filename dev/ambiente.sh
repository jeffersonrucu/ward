# Ambiente fixo de todo script de dev/ (Fase 12b). Uso, logo no começo do script:
#
#     #!/bin/bash -p
#     export PATH=/usr/bin:/bin
#     RAIZ="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
#     . "$RAIZ/dev/ambiente.sh"
#
# O shell do usuário traz coisas que mudam o que os testes executam ou leem:
# - PATH com `~/.local/bin`, nvm etc. Um `docker`, `git` ou `python3` plantado ali
#   rodaria no lugar do do sistema. Aqui o PATH é só /usr/bin:/bin (o `ataques-fundacao`
#   acrescenta ~/.cargo/bin no fim, para o cargo do rustup).
# - BASH_ENV/ENV (arquivo lido por todo bash não interativo) e funções exportadas: o
#   `#!/bin/bash -p` não lê nenhum dos dois; aqui ainda tiramos as variáveis do ambiente
#   dos filhos.
# - git: lê `~/.gitconfig`, `~/.config/git/{config,ignore,attributes}` e o
#   `/etc/gitconfig`. Um `core.fsmonitor` ali roda programa a cada `git diff`/`status`
#   (inclusive dentro do gitleaks), e um `ignore` global faz `git check-ignore` passar
#   sem o `.gitignore` do repositório. As variáveis abaixo trocam o global por
#   /dev/null, desligam o do sistema e, com a precedência de linha de comando, fixam
#   `core.excludesFile`, `core.attributesFile`, `core.fsmonitor` e `core.hooksPath`
#   (vale também contra o `.git/config` local).

export PATH=/usr/bin:/bin
unset BASH_ENV ENV CDPATH GLOBIGNORE

# Toda variável GIT_* herdada sai (GIT_DIR, GIT_SSH_COMMAND, GIT_EXEC_PATH, GIT_CONFIG_*...).
while IFS= read -r _var; do unset "$_var"; done < <(compgen -e | /usr/bin/grep -E '^GIT_' || true)
unset _var
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_COUNT=4
export GIT_CONFIG_KEY_0=core.excludesFile GIT_CONFIG_VALUE_0=/dev/null
export GIT_CONFIG_KEY_1=core.attributesFile GIT_CONFIG_VALUE_1=/dev/null
export GIT_CONFIG_KEY_2=core.fsmonitor GIT_CONFIG_VALUE_2=false
export GIT_CONFIG_KEY_3=core.hooksPath GIT_CONFIG_VALUE_3=/dev/null

# Home do passwd (o ssh, o curl e o git usam este, não o $HOME).
CASA_PASSWD="$(/usr/bin/getent passwd "$(/usr/bin/id -u)" | /usr/bin/cut -d: -f6)"
[ -n "$CASA_PASSWD" ] && [ "$CASA_PASSWD" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }
