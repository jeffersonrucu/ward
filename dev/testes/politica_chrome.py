#!/usr/bin/python3 -I
"""Um cenário do `make teste-politica-chrome` (Fase 51a; V9.5, V9.6; achado crítico da
auditoria do M9). Roda **dentro** do bwrap do `politica-chrome.sh`, em que
/etc/opt/chrome é a pasta `<trab>/etc-chrome` (o resto do sistema é o do host, sem
nada gravável novo). Nunca toca o perfil real do Chrome: perfil, HOME e manifestos ficam
em `<trab>/<cenario>/`.

Uso: politica_chrome.py <raiz> <trab> <cenario> <politica|-> [<wardd> <ward-nm> [<wardd-aprovador-teste>]]

Cenários (o mesmo ataque da auditoria: um processo do uid planta um
`br.ward.nm.json` no perfil, `<perfil>/NativeMessagingHosts/`):
  precedencia  hosts falsos que só gravam o que recebem: um no manifesto do sistema
               (/etc/opt/chrome/native-messaging-hosts/) e outro no do perfil. A
               extensão de teste (id da chave de `<trab>/k.pem`) manda uma mensagem
               para `br.ward.nm`. Imprime {"sistema": bool, "usuario": bool}.
  roubo        o ataque inteiro da auditoria (`e2e/rodar.py` e `e2e/hospedeiro.py`): o
               host do perfil faz `exec` do `ward-nm` de teste com o argv da extensão do
               Ward e fica com o stdin/stdout dele.
               - sem política (`-`, o controle): roda FORA do bwrap, num host sem
                 /etc/opt/chrome, contra um `wardd` de teste só deste cenário
                 (`WARD_DEV_EXTENSAO=chrome`, cópia do cofre de teste, socket próprio).
                 Dentro do bwrap não daria: o `wardd` e o `ward-nm` de teste conferem
                 as pastas acima do socket, e no namespace do bwrap as do root aparecem
                 como do nobody. Com /etc/opt/chrome no host, sai "pulado".
               - com a política: no bwrap, sem `wardd`; o manifesto do sistema aponta
                 para um script que marca que rodou e faz exec do `ward-nm` de teste.
                 O que decide é o host do atacante não rodar.
               Imprime {"atacante_rodou": bool, "sistema_rodou": bool,
               "respostas": {pedido: tipo}, "com_segredo": n} — nunca um valor.

`<politica>`: o `ward.json` gerado pelo `instalar.sh` (staging), copiado para
policies/managed/; `-` = sem política (o controle, que mostra o ataque funcionando).
"""

import base64
import hashlib
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import time

CHROME = "/opt/google/chrome/chrome"


def main():
    raiz, trab, cenario, politica = sys.argv[1:5]
    if cenario not in ("precedencia", "roubo"):
        sys.exit(f"cenário desconhecido: {cenario}")
    os.umask(0o077)
    etc = f"{trab}/etc-chrome"
    # Controle do roubo: fora do bwrap (o wardd e o ward-nm de teste conferem as pastas
    # acima do socket, e no namespace do bwrap as do root aparecem como do nobody), num
    # host sem /etc/opt/chrome (sem política nem manifesto do sistema).
    fora = cenario == "roubo" and politica == "-"
    if fora:
        if os.path.lexists("/etc/opt/chrome"):
            print(json.dumps({"pulado": "o host já tem /etc/opt/chrome; o controle precisa de um Chrome sem política"}))
            return
    else:
        if not os.path.isdir("/etc/opt/chrome") or os.path.realpath("/etc/opt/chrome") != "/etc/opt/chrome":
            sys.exit("/etc/opt/chrome não está montado (rode pelo politica-chrome.sh)")
        # Prova de que /etc/opt/chrome é a pasta do teste (o bwrap), e não a do sistema.
        marca = f"{etc}/.marca-{os.getpid()}"
        open(marca, "w").close()
        montado = os.path.exists(f"/etc/opt/chrome/.marca-{os.getpid()}")
        os.remove(marca)
        if not montado:
            sys.exit("/etc/opt/chrome não é a pasta do teste")
        nmh, managed = f"{etc}/native-messaging-hosts", f"{etc}/policies/managed"
        for d in (nmh, managed):
            os.makedirs(d, exist_ok=True)
            for f in os.listdir(d):
                os.remove(f"{d}/{f}")
        if politica != "-":
            shutil.copyfile(politica, f"{managed}/ward.json")

    der = subprocess.run(["openssl", "rsa", "-in", f"{trab}/k.pem", "-pubout", "-outform", "DER"],
                         capture_output=True, check=True).stdout
    eid = "".join(chr(ord("a") + int(c, 16)) for c in hashlib.sha256(der).hexdigest()[:32])
    rotulo = f"{cenario}-{'politica' if politica != '-' else 'sem-politica'}"
    t = f"{trab}/{rotulo}"
    shutil.rmtree(t, ignore_errors=True)
    ext, perfil, home = f"{t}/ext", f"{t}/perfil", f"{t}/home"
    for d in (ext, f"{perfil}/NativeMessagingHosts", home):
        os.makedirs(d)

    def manifesto(caminho, destino):
        json.dump({"name": "br.ward.nm", "description": "teste", "path": caminho, "type": "stdio",
                   "allowed_origins": [f"chrome-extension://{eid}/"]}, open(destino, "w"))

    env_chrome = {"HOME": home, "PATH": "/usr/bin:/bin"}
    wardd = None
    if cenario == "precedencia":
        for quem in ("sistema", "usuario"):
            host = f"{t}/host-{quem}.py"
            open(host, "w").write(f"""#!/usr/bin/python3 -I
import struct, sys
n = struct.unpack("=I", sys.stdin.buffer.read(4))[0]
open({t + '/recebido-' + quem!r}, "wb").write(sys.stdin.buffer.read(n))
o = b'{{"quem":"{quem}"}}'
sys.stdout.buffer.write(struct.pack("=I", len(o)) + o)
sys.stdout.buffer.flush()
""")
            os.chmod(host, 0o700)
        manifesto(f"{t}/host-sistema.py", f"{nmh}/br.ward.nm.json")
        manifesto(f"{t}/host-usuario.py", f"{perfil}/NativeMessagingHosts/br.ward.nm.json")
        sw = 'chrome.runtime.sendNativeMessage("br.ward.nm", {v: 1, op: "estado"}, () => {});'
    else:
        wardd_bin, nm_bin = sys.argv[5:7]
        fx = json.load(open(f"{raiz}/dev/fixtures/entradas.json"))
        ward_id = [ln.split('"')[1] for ln in open(f"{raiz}/crates/ward-nm/src/lib.rs")
                   if ln.startswith("pub const ID_EXTENSAO")][0]
        rt = os.environ["XDG_RUNTIME_DIR"]
        sockdir = f"{rt}/ward-politica-chrome-{os.getpid()}"
    if cenario == "roubo" and fora:
        shutil.copyfile(f"{raiz}/.dev/cofre-teste.ward", f"{t}/cofre-teste.ward")
        open(f"{t}/cofre-teste.estado", "w").write("ward-estado 1\nvista 0\nqueimada 0\n")
        env_wardd = {"HOME": t, "PATH": "/usr/bin:/bin", "XDG_RUNTIME_DIR": rt, "LANG": "C.UTF-8",
                     "WARD_DEV_DIR": t, "WARD_DEV_SOCKET_DIR": sockdir, "WARD_DEV_EXTENSAO": "chrome",
                     "RUST_LOG": "info"}
        # Fase 44c: a janela de teste confirma a origem `http` (o usuário que confirma).
        if len(sys.argv) > 7:
            os.makedirs(f"{t}/logs", mode=0o700, exist_ok=True)
            shutil.copy2(sys.argv[7], f"{t}/wardd-aprovador-teste")
            open(f"{t}/aprovador.modo", "w").write("confirmar\n")
            env_wardd["WARD_DEV_APP"] = f"{t}/wardd-aprovador-teste"
        wardd = subprocess.Popen(
            [wardd_bin], stdin=subprocess.DEVNULL, stdout=open(f"{t}/wardd.log", "w"), stderr=subprocess.STDOUT,
            env=env_wardd)
        for _ in range(200):
            if os.path.exists(f"{sockdir}/wardd.sock"):
                break
            time.sleep(0.05)
        s = socket.socket(socket.AF_UNIX)
        s.connect(f"{sockdir}/wardd.sock")
        b = json.dumps({"v": 1, "op": "destravar", "dados": {
            "senha": base64.b64encode(fx["senha_mestra"].encode()).decode()}}).encode()
        s.sendall(struct.pack(">I", len(b)) + b)
        n = struct.unpack(">I", s.recv(4))[0]
        r = b""
        while len(r) < n:
            r += s.recv(n - len(r))
        s.close()
        if json.loads(r).get("tipo") != "ok":
            sys.exit("o wardd de teste não destravou")
    if cenario == "roubo":
        # O host do atacante (o `hospedeiro.py` da auditoria): marca que rodou, cria o
        # ajudante com os pipes e faz exec do ward-nm de verdade (mesmo pid, pai = Chrome).
        atacante = f"{t}/atacante.py"
        open(atacante, "w").write(r'''#!/usr/bin/python3 -I
import json, os, struct
NM, SAIDA, ID, MARCA = os.environ["ATQ_NM"], os.environ["ATQ_SAIDA"], os.environ["ATQ_ID"], os.environ["ATQ_MARCA"]
open(MARCA, "w").close()
a_r, a_w = os.pipe()
b_r, b_w = os.pipe()
if os.fork() == 0:
    os.close(a_r); os.close(b_w)
    os.setsid()
    ia, ib = os.fdopen(a_w, "wb", 0), os.fdopen(b_r, "rb", 0)
    def troca(m):
        b = json.dumps(m).encode()
        ia.write(struct.pack("=I", len(b)) + b)
        n = struct.unpack("=I", ib.read(4))[0]
        return json.loads(ib.read(n))
    res = {}
    for origem in ("http://localhost:8081", "http://localhost:8083", "http://localhost:8084"):
        s = troca({"v": 1, "op": "sugerir", "dados": {"origem": origem}})
        res["sugerir " + origem] = s
        d = s.get("dados") if isinstance(s.get("dados"), dict) else {}
        for item in d.get("itens", []) or []:
            nome = item.get("nome")
            res["preencher " + str(nome)] = troca({"v": 1, "op": "preencher", "dados": {"entrada": nome, "origem": origem + "/"}})
    open(SAIDA, "w").write(json.dumps(res))
    os._exit(0)
os.close(a_w); os.close(b_r)
os.dup2(a_r, 0); os.dup2(b_w, 1); os.close(a_r); os.close(b_w)
os.execv(NM, ["ward-nm", f"chrome-extension://{ID}/"])
''')
        os.chmod(atacante, 0o700)
        if not fora:
            # Manifesto do sistema: marca que rodou e faz exec do ward-nm de teste.
            sistema = f"{t}/host-sistema.sh"
            open(sistema, "w").write(f"#!/bin/sh\n: > '{t}/sistema-rodou'\nexec '{nm_bin}' \"$@\"\n")
            os.chmod(sistema, 0o700)
            manifesto(sistema, f"{nmh}/br.ward.nm.json")
        manifesto(atacante, f"{perfil}/NativeMessagingHosts/br.ward.nm.json")
        env_chrome.update({"WARD_DEV_SOCKET_DIR": sockdir, "ATQ_NM": nm_bin, "ATQ_SAIDA": f"{t}/roubado.json",
                           "ATQ_ID": ward_id, "ATQ_MARCA": f"{t}/atacante-rodou"})
        sw = 'const p = chrome.runtime.connectNative("br.ward.nm");'

    json.dump({"manifest_version": 3, "name": "teste-politica", "version": "1",
               "key": base64.b64encode(der).decode(), "permissions": ["nativeMessaging"],
               "background": {"service_worker": "sw.js"}}, open(f"{ext}/manifest.json", "w"))
    open(f"{ext}/sw.js", "w").write(sw)

    r_cmd, w_cmd = os.pipe()
    r_res, w_res = os.pipe()

    def prep():
        os.dup2(r_cmd, 3)
        os.dup2(w_res, 4)

    ch = subprocess.Popen(
        [CHROME, "--headless=new", f"--user-data-dir={perfil}", "--remote-debugging-pipe",
         "--enable-unsafe-extension-debugging", "--no-first-run", "--no-default-browser-check",
         "--disable-gpu", "--disable-sync", "--disable-background-networking", "about:blank"],
        preexec_fn=prep, pass_fds=(3, 4), env=env_chrome,
        stdout=subprocess.DEVNULL, stderr=open(f"{t}/chrome.log", "w"))
    os.close(r_cmd)
    os.close(w_res)
    try:
        os.write(w_cmd, json.dumps({"id": 1, "method": "Extensions.loadUnpacked",
                                    "params": {"path": ext}}).encode() + b"\0")
        if cenario == "precedencia":
            alvos = [f"{t}/recebido-sistema", f"{t}/recebido-usuario"]
        else:
            alvos = [f"{t}/roubado.json"]
        fim = time.time() + 25
        while time.time() < fim and not any(os.path.exists(a) for a in alvos):
            time.sleep(0.2)
        # Mais um pouco: o outro host também teria tempo de ser chamado.
        time.sleep(3 if cenario == "precedencia" else 1)
    finally:
        ch.terminate()
        try:
            ch.wait(timeout=15)
        except subprocess.TimeoutExpired:
            ch.kill()
            ch.wait()
        if wardd is not None:
            wardd.terminate()
            try:
                wardd.wait(timeout=15)
            except subprocess.TimeoutExpired:
                wardd.kill()
                wardd.wait()
            shutil.rmtree(sockdir, ignore_errors=True)

    if cenario == "precedencia":
        print(json.dumps({q: os.path.exists(f"{t}/recebido-{q}") for q in ("sistema", "usuario")}))
        return
    respostas, com_segredo = {}, 0
    if os.path.exists(f"{t}/roubado.json"):
        try:
            roubo = json.load(open(f"{t}/roubado.json"))
        except ValueError:
            roubo = {}
        for pedido, resp in roubo.items():
            respostas[pedido] = resp.get("tipo") if isinstance(resp, dict) else "?"
            dados = resp.get("dados") if isinstance(resp, dict) else None
            if (isinstance(resp, dict) and resp.get("tipo") == "preenchimento" and isinstance(dados, dict)
                    and (dados.get("senha") or dados.get("totp"))):
                com_segredo += 1
        # O que o ataque tirou (falso, D15) não fica no disco depois do resumo.
        os.remove(f"{t}/roubado.json")
    print(json.dumps({"atacante_rodou": os.path.exists(f"{t}/atacante-rodou"),
                      "sistema_rodou": os.path.exists(f"{t}/sistema-rodou"),
                      "respostas": respostas, "com_segredo": com_segredo}))


if __name__ == "__main__":
    main()
