# Fixed environment of every dev/ script (Phase 12b). Usage, right at the start of the script:
#
#     #!/bin/bash -p
#     export PATH=/usr/bin:/bin
#     ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
#     . "$ROOT/dev/env.sh"
#
# The user's shell brings things that change what the tests execute or read:
# - PATH with `~/.local/bin`, nvm etc. A `docker`, `git` or `python3` planted there
#   would run in place of the system one. Here PATH is only /usr/bin:/bin
#   (`attacks-foundation` appends ~/.cargo/bin at the end, for rustup's cargo).
# - BASH_ENV/ENV (file read by every non-interactive bash) and exported functions:
#   `#!/bin/bash -p` reads neither; here we also remove the variables from the
#   children's environment.
# - git: reads `~/.gitconfig`, `~/.config/git/{config,ignore,attributes}` and
#   `/etc/gitconfig`. A `core.fsmonitor` there runs a program on every `git diff`/`status`
#   (even inside gitleaks), and a global `ignore` makes `git check-ignore` pass without
#   the repository's `.gitignore`. The variables below swap the global one for
#   /dev/null, turn off the system one and, with command-line precedence, pin
#   `core.excludesFile`, `core.attributesFile`, `core.fsmonitor` and `core.hooksPath`
#   (it also holds against the local `.git/config`).

export PATH=/usr/bin:/bin
unset BASH_ENV ENV CDPATH GLOBIGNORE

# Every inherited GIT_* variable leaves (GIT_DIR, GIT_SSH_COMMAND, GIT_EXEC_PATH, GIT_CONFIG_*...).
while IFS= read -r _var; do unset "$_var"; done < <(compgen -e | /usr/bin/grep -E '^GIT_' || true)
unset _var
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_COUNT=4
export GIT_CONFIG_KEY_0=core.excludesFile GIT_CONFIG_VALUE_0=/dev/null
export GIT_CONFIG_KEY_1=core.attributesFile GIT_CONFIG_VALUE_1=/dev/null
export GIT_CONFIG_KEY_2=core.fsmonitor GIT_CONFIG_VALUE_2=false
export GIT_CONFIG_KEY_3=core.hooksPath GIT_CONFIG_VALUE_3=/dev/null

# Home of the passwd (ssh, curl and git use this one, not $HOME).
PASSWD_HOME="$(/usr/bin/getent passwd "$(/usr/bin/id -u)" | /usr/bin/cut -d: -f6)"
[ -n "$PASSWD_HOME" ] && [ "$PASSWD_HOME" != / ] || { echo "home do passwd indefinido" >&2; exit 1; }
