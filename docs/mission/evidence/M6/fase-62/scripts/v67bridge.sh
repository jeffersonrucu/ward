#!/bin/bash -p
# fase62: m2:V6.7 with the test bridge: the approval entry opens the app (bridge), the
# free one does not. Usage: v67bridge.sh <root>  (daemon up with the default WARD_DEV_APP)
export PATH=/usr/bin:/bin
R="$1"; cd "$R" || exit 2
W="$R/target/debug/ward"
python3 -I - "$R" <<'EOF'
import base64, json, os, socket, struct, sys
root = sys.argv[1]
fx = json.load(open(f"{root}/dev/fixtures/entries.json"))
s = socket.socket(socket.AF_UNIX); s.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], "ward-test", "wardd.sock"))
b = json.dumps({"v": 2, "op": "unlock", "data": {"password": base64.b64encode(fx["master_password"].encode()).decode()}}).encode()
s.sendall(struct.pack(">I", len(b)) + b)
n = struct.unpack(">I", s.recv(4))[0]; print("unlock:", json.loads(s.recv(n)).get("type"))
EOF
bridges() { pgrep -fc "target/debug/ward-test-bridge" || true; }
log="$R/.dev/logs/wardd.log"
echo "## pontes antes: $(bridges)"
before=$(stat -c %s "$log")
echo "## V6.7 ward run acme-pg-dev -- true (livre)"
"$W" run acme-pg-dev -- true </dev/null; echo "exit=$?"
sleep 1; echo "pontes depois do livre: $(bridges)"
echo "linhas novas do log com open_app/app: $(tail -c +$((before+1)) "$log" | grep -ci 'open_app\|abre o app\|opening the app\|launch')"
before=$(stat -c %s "$log")
echo "## V6.7 ward run acme-pg-aprovacao -- true (aprovação; corte em 8 s)"
timeout 8 "$W" run acme-pg-aprovacao -- true </dev/null & c=$!
sleep 4; echo "pontes durante o pedido: $(bridges)"
echo "pedido pendente no log: $(tail -c +$((before+1)) "$log" | grep -c 'acme-pg-aprovacao')"
wait $c; echo "exit=$? (124 = esperava a decisão na janela)"
tail -c +$((before+1)) "$log" | grep -v '^\s*$' | cut -c1-200 | tail -n 15
