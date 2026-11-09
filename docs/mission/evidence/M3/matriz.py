import socket, sys, struct, json, base64, collections
path = sys.argv[1]
def req(s, obj):
    b = json.dumps(obj).encode() if not isinstance(obj, bytes) else obj
    s.sendall(struct.pack(">I", len(b)) + b)
    n = s.recv(4)
    if len(n) < 4: return b""
    L = struct.unpack(">I", n)[0]; d=b""
    while len(d) < L:
        c = s.recv(L-len(d))
        if not c: break
        d += c
    return d
def novo():
    s = socket.socket(socket.AF_UNIX); s.settimeout(60); s.connect(path); return s
b64 = lambda x: base64.b64encode(x.encode()).decode()
s = novo()
print(req(s, {"v":1,"op":"destravar","dados":{"senha":b64("falso-mestra-ward-01")}}))
lista = json.loads(req(s, {"v":1,"op":"lista","dados":{}}))
nomes = [i["nome"] for i in lista["dados"]["itens"]] if "itens" in lista["dados"] else None
print("lista keys", list(lista["dados"].keys()), "n=", len(nomes) if nomes else None)
nomes = nomes or []
nomes += ["gama-bloqueada","acme-pg-dev","inexistente"]
ops = ["estado","destravar","travar","lista","executar","ssh","login","playwright","entrada_padrao","janela","criar_cofre","detalhe","revelar","copiar","totp","salvar_entrada","remover_entrada","mudar_nivel","responder_aprovacao","emitir_token","trocar_pin","trocar_senha","tempo_trava","backups","restaurar_backup","sugerir","preencher","salvar_login","abrir_app"]
vari = [l.strip() for l in open(".dev/variacoes.txt") if len(l.strip())>=6]
cont = collections.Counter(); achados = 0; total=0
for op in ops:
    if op in ("travar","destravar"): continue
    for nome in set(nomes):
        for extra in [{}, {"revelar":True,"campo":"senha"}]:
            dados = {"entrada":nome,"campo":"senha","alvo":{"campo":"senha"},"pedido":1,"nivel":"livre","pin":b64("135790")}
            dados.update(extra)
            for obj in [{"v":1,"op":op,"dados":dados}, {"v":1,"op":op,"revelar":True,"campo":"senha","dados":dados}]:
                try:
                    s2 = novo(); r = req(s2, obj); s2.close()
                except Exception as e:
                    r = b"ERR"
                total+=1
                txt = r.decode("utf8","replace")
                try: cod = json.loads(txt)["dados"].get("codigo") or json.loads(txt)["tipo"]
                except Exception: cod = "?"
                cont[(op,cod)]+=1
                for v in vari:
                    if v in txt: achados+=1; print("ACHADO em", op, nome)
                    
print("total", total, "achados", achados)
for (op,cod),n in sorted(cont.items()): print(op, cod, n)
