import socket, sys, struct, json, time, resource
resource.setrlimit(resource.RLIMIT_NOFILE, (20000, 20000))
path = sys.argv[1]
def req(s, op, dados=None):
    b = json.dumps({"v":1,"op":op,"dados":dados or {}}).encode()
    s.sendall(struct.pack(">I", len(b)) + b)
    n = s.recv(4)
    if len(n) < 4: return None
    L = struct.unpack(">I", n)[0]
    d = b""
    while len(d) < L:
        c = s.recv(L-len(d))
        if not c: break
        d += c
    return d
conns=[]
fails=0
for i in range(10000):
    try:
        c = socket.socket(socket.AF_UNIX); c.connect(path); conns.append(c)
    except Exception as e:
        fails+=1
print("abertas", len(conns), "falhas de connect", fails)
t=time.time()
s = socket.socket(socket.AF_UNIX); s.settimeout(30); s.connect(path)
print("normal:", req(s, "estado"), round(time.time()-t,2))
# quadros ruins
for raw in [struct.pack(">I", 2*1024*1024)+b"x", struct.pack(">i", -1), struct.pack(">I", 5)+b"{nao}", struct.pack(">I", 2)+b"{}" ]:
    c = socket.socket(socket.AF_UNIX); c.settimeout(10); c.connect(path)
    c.sendall(raw)
    try:
        r = c.recv(4096)
    except Exception as e:
        r = repr(e)
    print("ruim", raw[:6], "->", r[:90])
b = json.dumps({"v":1,"op":"inventada","dados":{}}).encode()
c = socket.socket(socket.AF_UNIX); c.settimeout(10); c.connect(path)
print("op desconhecida", req(c, "inventada"))
s2 = socket.socket(socket.AF_UNIX); s2.settimeout(10); s2.connect(path)
print("depois:", req(s2, "estado"))
