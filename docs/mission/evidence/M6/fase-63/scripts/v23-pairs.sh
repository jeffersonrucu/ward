#!/bin/bash -p
# Phase 63 (m2:V2.3): runs v23_logic_diff.py on each pair 0.1.4 -> 0.2.0 of the role and
# peer checks. The 0.1.4 tree comes from `git archive v0.1.4 crates` in .dev/fase63-v014.
# Usage (from the repository root): v23-pairs.sh
export PATH=/usr/bin:/bin
set -uo pipefail
O=.dev/fase63-v014/crates
D=docs/mission/evidence/M6/fase-63/scripts/v23_logic_diff.py
while IFS=: read -r old new; do
    [ -n "$old" ] || continue
    echo "=== $old -> $new"
    if [ ! -f "$O/$old" ]; then echo "  (sem o antigo)"; continue; fi
    if [ ! -f "crates/$new" ]; then echo "  (sem o novo)"; continue; fi
    python3 -I "$D" "$O/$old" "crates/$new"
done <<'EOF'
ward-plataforma/src/papel.rs:ward-platform/src/role.rs
ward-plataforma/src/servidor.rs:ward-platform/src/server.rs
ward-plataforma/src/conectar.rs:ward-platform/src/connect.rs
ward-plataforma/src/socket.rs:ward-platform/src/socket.rs
ward-plataforma/src/sys.rs:ward-platform/src/sys.rs
ward-plataforma/src/processo.rs:ward-platform/src/process.rs
ward-plataforma/src/endurecimento.rs:ward-platform/src/hardening.rs
ward-nm/src/main.rs:ward-nm/src/main.rs
ward-nm/src/lib.rs:ward-nm/src/lib.rs
EOF
