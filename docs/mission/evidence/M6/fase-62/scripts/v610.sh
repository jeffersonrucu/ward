#!/bin/bash -p
# fase62: m2:V6.10 (ward lock by hand), m2:V2.6 (host part with the test daemon) and
# m2:V2.7 (ward lock kills a running ward run, also a setsid descendant). Usage: v610.sh <root>
export PATH=/usr/bin:/bin
R="$1"; cd "$R" || exit 2
W="$R/target/debug/ward"
step() { printf '\n## %s\n' "$*"; }
pid=$(cat "$R/.dev/wardd.pid")
step "V2.6 daemon de teste (pid do .dev/wardd.pid): cmdline e dumpable"
tr '\0' ' ' < /proc/$pid/cmdline; echo
python3 -I -c "import ctypes,sys; print('ok')" >/dev/null
grep -E '^(TracerPid|NoNewPrivs)' /proc/$pid/status
step "V2.6 strace -p no daemon do mesmo uid"
timeout 5 strace -p "$pid" -e trace=none 2>&1 | head -n 2; echo "exit=${PIPESTATUS[0]}"
step "V2.6 /proc/<pid>/mem e environ ilegíveis"
head -c 1 /proc/$pid/environ >/dev/null 2>&1 && echo "environ legível: SIM" || echo "environ legível: não"
step "V2.7 ward lock com um ward run em curso (filho com setsid)"
marker="$R/.dev/fase62-setsid-pid"; rm -f "$marker"
"$W" run acme-pg-dev -- /bin/sh -c 'setsid /bin/sh -c "echo \$\$ > '"$marker"'; sleep 300" & sleep 300' </dev/null > "$R/.dev/logs/fase62-v27-run.out" 2>&1 &
cli=$!
for i in $(seq 50); do [ -s "$marker" ] && break; sleep 0.2; done
sp=$(cat "$marker"); echo "descendente setsid vivo: $( [ -d /proc/$sp ] && echo sim || echo não)"
"$W" lock; echo "lock exit=$?"
wait $cli; echo "ward run exit=$?"
cat "$R/.dev/logs/fase62-v27-run.out"
sleep 1
echo "descendente setsid depois da trava: $( [ -d /proc/$sp ] && echo VIVO || echo morto)"
rm -f "$marker"
step "V6.10 travado: ward list"
"$W" list; echo "exit=$?"
step "V6.10 travado: ward run acme-pg-dev -- true espera o destrave (corte em 5 s pelo timeout)"
timeout 5 "$W" run acme-pg-dev -- true </dev/null; echo "exit=$? (124 = ainda esperava)"
