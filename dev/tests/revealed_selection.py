#!/usr/bin/python3 -IB
"""Fase 39g (S2, S12; 2ª auditoria do M5): o valor revelado não vai à área de
transferência por seleção, Ctrl+A, Ctrl+C nem arrasto, no WebKitGTK de verdade.

Chamado por `dev/tests/revealed-selection.sh` (`make test-revealed-selection`), que já
subiu um `wardd` de teste só dele com a ponte (D63) e passa a URL da ponte (com o token)
pela entrada padrão, nunca em argv.

Tudo roda num servidor X **privado** (Xwayland em modo rootful, com cookie MIT em
`.dev/revealed-selection/`, sem TCP): a PRIMARY e a CLIPBOARD dele não são as da sessão
do usuário, que este teste nunca lê nem escreve. Uma janela do Xwayland aparece na tela
por uns 20 s. O WebKitGTK 4.1 (o mesmo do app, por python3-gi) abre a interface de
teste pela ponte, com HOME e XDG falsos e contexto efêmero; destrava com a senha de
teste, abre `acme-pg-dev` e clica em "Revelar". Outro processo X (o "outro programa")
lê as duas seleções e só diz se cada agulha está lá; nenhum valor é impresso.

Antes, o autoteste: o servidor recusa cliente sem cookie; um `<span>` com o valor como
texto (o componente antigo, ataque da auditoria) selecionado vai à PRIMARY, e o mesmo
com Ctrl+C à CLIPBOARD (o leitor acha). Depois, com o componente atual, cada ataque
abaixo não pode pôr o valor em nenhuma das duas, e o efeito do ataque é conferido (a
seleção mudou) para ele não passar à toa:
- seleção do campo por script (`addRange`);
- Ctrl+A de verdade (evento de teclado do GDK), depois Ctrl+C;
- arrasto do mouse (eventos do GDK) do rótulo até o fim do campo; clique triplo;
- seleção por script que atravessa o campo, e `execCommand('copy')` sobre ela;
- `innerText` da página sem o valor;
- o campo continua mostrando o valor (`::before`), e "Ocultar" o tira do documento.

Sai 1 se algo falhar; 2 se faltar Xwayland, xauth ou o WebKitGTK do python3-gi.
"""

import json
import os
import secrets
import shutil
import subprocess
import sys
import threading
import time

RAIZ = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
PASTA = os.path.join(RAIZ, ".dev", "revealed-selection")
SENHA = "falso-mestra-ward-01"  # D15 (falsa)
ENTRADA = "acme-pg-dev"
PRAZO_FILHO = 150


# --------------------------------------------------------------------------- leitor
def modo_ler():
    """O "outro programa": lê PRIMARY e CLIPBOARD do X privado e diz, para cada agulha
    (hex na entrada padrão, uma por linha), se ela está lá. Nunca imprime o conteúdo."""
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk, Gtk

    agulhas = [bytes.fromhex(l.strip()).decode() for l in sys.stdin if l.strip()]
    saida = {}
    for nome, sel in (("primary", Gdk.SELECTION_PRIMARY), ("clipboard", Gdk.SELECTION_CLIPBOARD)):
        t = Gtk.Clipboard.get(sel).wait_for_text() or ""
        saida[nome] = [a in t for a in agulhas]
    print(json.dumps(saida), flush=True)
    return 0


# --------------------------------------------------------------------------- WebKit
def modo_webkit():
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    gi.require_version("WebKit2", "4.1")
    from gi.repository import Gdk, GLib, Gtk, WebKit2

    url = sys.stdin.readline().strip()
    if not (url.startswith("http://127.0.0.1:") and "#token=" in url):
        print("FALHA URL da ponte fora da forma", flush=True)
        return 1

    janela = Gtk.Window()
    janela.set_default_size(1280, 800)
    contexto = WebKit2.WebContext.new_ephemeral()
    wv = WebKit2.WebView.new_with_context(contexto)
    # Só para o teste pôr um valor neutro nas seleções entre um ataque e outro (o app
    # não liga isto; Ctrl+C do usuário não depende disto).
    wv.get_settings().set_javascript_can_access_clipboard(True)
    janela.add(wv)
    janela.show_all()
    wv.load_uri(url)

    falhas = []

    def ok(msg):
        print(f"OK   {msg}", flush=True)

    def falha(msg):
        falhas.append(msg)
        print(f"FALHA {msg}", flush=True)

    def no_principal(f):
        """Roda `f` na thread do GTK e devolve o resultado (espera)."""
        pronto = threading.Event()
        caixa = {}

        def rodar():
            try:
                caixa["v"] = f()
            except Exception as e:  # noqa: BLE001
                caixa["e"] = e
            pronto.set()
            return False

        GLib.idle_add(rodar)
        if not pronto.wait(30):
            raise RuntimeError("a thread do GTK não respondeu")
        if "e" in caixa:
            raise caixa["e"]
        return caixa.get("v")

    def js(codigo):
        """Avalia `codigo` (uma expressão; o resultado vai por JSON) na página."""
        pronto = threading.Event()
        caixa = {}

        def fim(_wv, res):
            try:
                v = wv.evaluate_javascript_finish(res)
                caixa["v"] = json.loads(v.to_json(0) or "null")
            except Exception as e:  # noqa: BLE001
                caixa["e"] = str(e).split("\n")[0][:200]
            pronto.set()

        script = f"JSON.parse(JSON.stringify((() => {{ {codigo} }})() ?? null))"
        GLib.idle_add(lambda: (wv.evaluate_javascript(script, -1, None, None, None, fim), False)[1])
        if not pronto.wait(30):
            raise RuntimeError("o javascript não respondeu")
        if "e" in caixa:
            raise RuntimeError(f"javascript: {caixa['e']}")
        return caixa["v"]

    def esperar(codigo, prazo=30.0):
        fim = time.monotonic() + prazo
        while time.monotonic() < fim:
            try:
                if js(codigo):
                    return True
            except RuntimeError:
                pass
            time.sleep(0.25)
        return False

    def ler(*agulhas):
        """PRIMARY e CLIPBOARD vistas por outro processo X."""
        time.sleep(0.6)
        r = subprocess.run(
            [sys.executable, "-I", "-B", os.path.abspath(__file__), "--ler"],
            input="".join(a.encode().hex() + "\n" for a in agulhas),
            capture_output=True,
            text=True,
            timeout=20,
            env=os.environ.copy(),
        )
        if r.returncode != 0:
            raise RuntimeError("o leitor das seleções falhou")
        return json.loads(r.stdout)

    # Eventos do GDK de verdade (o caminho do teclado e do mouse do usuário).
    def tecla(kv, mods):
        def f():
            win = wv.get_window()
            disp = Gdk.Display.get_default()
            teclado = disp.get_default_seat().get_keyboard()
            _, chaves = Gdk.Keymap.get_for_display(disp).get_entries_for_keyval(kv)
            for tipo in (Gdk.EventType.KEY_PRESS, Gdk.EventType.KEY_RELEASE):
                e = Gdk.Event.new(tipo)
                e.key.window = win
                e.key.send_event = 0
                e.key.time = Gdk.CURRENT_TIME
                e.key.state = mods
                e.key.keyval = kv
                e.key.hardware_keycode = chaves[0].keycode
                e.set_device(teclado)
                wv.event(e)

        no_principal(f)

    def mouse(tipo, x, y, botao_preso=False):
        def f():
            win = wv.get_window()
            ponteiro = Gdk.Display.get_default().get_default_seat().get_pointer()
            e = Gdk.Event.new(tipo)
            if tipo == Gdk.EventType.MOTION_NOTIFY:
                e.motion.window = win
                e.motion.x, e.motion.y = x, y
                e.motion.state = Gdk.ModifierType.BUTTON1_MASK if botao_preso else 0
            else:
                e.button.window = win
                e.button.x, e.button.y = x, y
                e.button.button = 1
            e.set_device(ponteiro)
            wv.event(e)

        no_principal(f)

    def clique(x, y, vezes=1):
        tipos = [Gdk.EventType.BUTTON_PRESS, Gdk.EventType.DOUBLE_BUTTON_PRESS, Gdk.EventType.TRIPLE_BUTTON_PRESS]
        for i in range(vezes):
            mouse(Gdk.EventType.BUTTON_PRESS, x, y)
            if i > 0:
                mouse(tipos[i], x, y)
            mouse(Gdk.EventType.BUTTON_RELEASE, x, y)

    CTRL = Gdk.ModifierType.CONTROL_MASK

    def principal():
        try:
            passos()
        except Exception as e:  # noqa: BLE001
            falha(f"erro no teste: {str(e)[:200]}")
        GLib.idle_add(Gtk.main_quit)

    def passos():
        if not esperar("return !!document.querySelector('#master-password')", 40):
            falha("a tela Destravar não abriu pela ponte")
            return
        js(
            "const e = document.querySelector('#master-password');"
            "const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
            f"set.call(e, {json.dumps(SENHA)}); e.dispatchEvent(new Event('input', {{bubbles: true}}));"
            "e.form ? e.form.requestSubmit() : e.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));"
            "return true"
        )
        if not esperar(f"return !!document.querySelector('[data-entry=\"{ENTRADA}\"]')", 60):
            falha("destravar pela ponte com a senha de teste")
            return
        ok("destravado pela ponte com a senha de teste")
        js(f"document.querySelector('[data-entry=\"{ENTRADA}\"]').click(); return true")
        if not esperar("return !!document.querySelector('button[aria-label^=\"Revelar \"]')", 15):
            falha(f"o detalhe de {ENTRADA} não mostrou o botão Revelar")
            return

        def revelar():
            # O valor pode estar no atributo (componente atual) ou no texto (o antigo,
            # para o teste mostrar o ataque contra ele).
            achar = "const e = document.querySelector('.revealed'); return !!(e && (e.dataset.value || e.textContent))"
            if js(achar):
                return True
            js("document.querySelector('button[aria-label^=\"Revelar \"]').click(); return true")
            return esperar(achar, 15)

        if not revelar():
            falha("o valor não foi revelado")
            return
        valor = js("const e = document.querySelector('.revealed'); return e.dataset.value || e.textContent")
        if not isinstance(valor, str) or len(valor) < 6:
            falha("valor revelado fora da forma")
            return
        if js("return document.querySelector('.revealed').textContent === ''"):
            ok("valor revelado só no atributo do campo, sem nó de texto (não impresso)")
        else:
            falha("o valor revelado é um nó de texto (selecionável)")

        # Controles: um texto comum na página (prova que o ataque teve efeito) e um
        # neutro para pôr nas duas seleções antes de cada ataque. Pelo CSSOM e pelo DOM,
        # sem HTML nem estilo inline (a CSP da interface os recusa).
        controle = "falso-39g-controle-" + secrets.token_hex(4)
        neutro = "falso-39g-neutro-" + secrets.token_hex(4)
        js(
            "for (const [id, t] of [['t39g-controle', %s], ['t39g-neutro', %s]]) {"
            " const p = document.createElement('p'); p.id = id; p.textContent = t;"
            " document.querySelector('main, body').appendChild(p); }"
            " return true" % (json.dumps(controle), json.dumps(neutro))
        )

        def neutralizar():
            js(
                "const r = document.createRange(); r.selectNodeContents(document.getElementById('t39g-neutro'));"
                "const s = getSelection(); s.removeAllRanges(); s.addRange(r);"
                "document.execCommand('copy'); return true"
            )
            v = ler(neutro, valor)
            if not (v["primary"][0] and v["clipboard"][0]) or v["primary"][1] or v["clipboard"][1]:
                raise RuntimeError("não deu para pôr o valor neutro nas seleções")
            no_principal(lambda: wv.grab_focus())

        # ---------------------------------------------------------------- autoteste
        # O componente antigo (valor como nó de texto selecionável): o ataque da
        # auditoria tem de ser achado pelo leitor.
        neutralizar()
        js(
            "const s = document.createElement('span'); s.id = 't39g-antigo';"
            f"s.textContent = {json.dumps(valor)}; document.body.appendChild(s);"
            "const r = document.createRange(); r.selectNodeContents(s);"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        v = ler(valor)
        if v["primary"][0]:
            ok("autoteste: valor como texto selecionado vai à PRIMARY (o leitor acha)")
        else:
            falha("autoteste: o leitor não achou o valor selecionado na PRIMARY")
        tecla(Gdk.KEY_c, CTRL)
        v = ler(valor)
        if v["clipboard"][0]:
            ok("autoteste: Ctrl+C (evento do GDK) leva o valor como texto à CLIPBOARD")
        else:
            falha("autoteste: Ctrl+C do GDK não levou o texto à CLIPBOARD")
        js("document.getElementById('t39g-antigo').remove(); getSelection().removeAllRanges(); return true")

        # ---------------------------------------------------------------- ataques
        def sem_valor(nome, prova=None):
            """Nenhuma seleção tem o valor; `prova(v)` diz se o ataque teve efeito."""
            v = ler(valor, controle, neutro)
            if v["primary"][0] or v["clipboard"][0]:
                onde = " e ".join(n for n in ("primary", "clipboard") if v[n][0]).upper()
                falha(f"{nome}: o valor revelado foi para a {onde}")
                return
            if prova is not None and not prova(v):
                falha(f"{nome}: o ataque não teve efeito (o teste não prova nada)")
                return
            ok(f"{nome}: o valor não vai à PRIMARY nem à CLIPBOARD")

        def caixa(sel):
            return js(
                f"const e = document.querySelector({json.dumps(sel)}); if (!e) return null;"
                " const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]"
            )

        # 1. Seleção do campo por script.
        revelar()
        neutralizar()
        js(
            "const r = document.createRange(); r.selectNodeContents(document.querySelector('.revealed'));"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        sem_valor("seleção do campo por script")

        # 2. Ctrl+A e Ctrl+C de verdade.
        revelar()
        neutralizar()
        tecla(Gdk.KEY_a, CTRL)
        sem_valor("Ctrl+A", prova=lambda v: v["primary"][1])
        tecla(Gdk.KEY_c, CTRL)
        sem_valor("Ctrl+A e Ctrl+C", prova=lambda v: v["clipboard"][1])

        # 3. Arrasto do mouse do rótulo até o fim do campo, e clique triplo no campo.
        revelar()
        neutralizar()
        rotulo = caixa(".revealed")
        campo = js(
            "const e = document.querySelector('.revealed').closest('.field');"
            " const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]"
        )
        if not rotulo or not campo:
            falha("arrasto: não achei o campo revelado na tela")
        else:
            x0, y0 = campo[0] + 2, campo[1] + 4
            x1, y1 = rotulo[2] - 2, (rotulo[1] + rotulo[3]) / 2
            mouse(Gdk.EventType.BUTTON_PRESS, x0, y0)
            passos_arrasto = 24
            for i in range(1, passos_arrasto + 1):
                mouse(
                    Gdk.EventType.MOTION_NOTIFY,
                    x0 + (x1 - x0) * i / passos_arrasto,
                    y0 + (y1 - y0) * i / passos_arrasto,
                    botao_preso=True,
                )
            mouse(Gdk.EventType.BUTTON_RELEASE, x1, y1)
            sem_valor("arrasto do mouse pelo campo", prova=lambda v: not v["primary"][2])
            revelar()
            neutralizar()
            r = caixa(".revealed")
            clique((r[0] + r[2]) / 2, (r[1] + r[3]) / 2, vezes=3)
            sem_valor("clique triplo no campo")
            tecla(Gdk.KEY_c, CTRL)
            sem_valor("clique triplo e Ctrl+C")

        # 4. Seleção que atravessa o campo (de antes dele até o controle) e cópia dela.
        revelar()
        neutralizar()
        js(
            "const r = document.createRange();"
            "r.setStartBefore(document.querySelector('.revealed').closest('.field'));"
            "r.setEndAfter(document.getElementById('t39g-controle'));"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        sem_valor("seleção que atravessa o campo", prova=lambda v: v["primary"][1])
        js("document.execCommand('copy'); return true")
        sem_valor("cópia da seleção que atravessa o campo", prova=lambda v: v["clipboard"][1])

        # 5. Serialização da página e o que o campo mostra.
        revelar()
        if js(f"return document.body.innerText.includes({json.dumps(valor)})"):
            falha("innerText da página tem o valor revelado")
        else:
            ok("innerText da página sem o valor revelado")
        antes = js(
            "return getComputedStyle(document.querySelector('.revealed'), '::before').content"
        )
        if antes == json.dumps(valor):
            ok("o campo continua mostrando o valor (::before)")
        else:
            falha("o campo revelado não mostra o valor")
        js("document.querySelector('button[aria-label^=\"Ocultar \"]').click(); return true")
        if esperar("return !document.querySelector('.revealed')", 10) and not js(
            f"return document.documentElement.outerHTML.includes({json.dumps(valor)})"
        ):
            ok("Ocultar tira o valor do documento")
        else:
            falha("depois de Ocultar o valor continua no documento")
        js(
            "for (const id of ['t39g-controle', 't39g-neutro']) document.getElementById(id)?.remove();"
            "getSelection().removeAllRanges(); return true"
        )

    threading.Thread(target=principal, daemon=True).start()
    Gtk.main()
    return 1 if falhas else 0


# --------------------------------------------------------------------------- orquestra
def main():
    for f in ("/usr/bin/Xwayland", "/usr/bin/xauth", "/usr/bin/mcookie"):
        if not os.access(f, os.X_OK):
            print(f"selecao-revelado: falta {f}", file=sys.stderr)
            return 2
    r = subprocess.run(
        [sys.executable, "-I", "-B", "-c", "import gi; gi.require_version('WebKit2', '4.1'); from gi.repository import WebKit2"],
        capture_output=True,
    )
    if r.returncode != 0:
        print("selecao-revelado: falta o WebKitGTK 4.1 do python3-gi (gir1.2-webkit2-4.1)", file=sys.stderr)
        return 2
    if not os.environ.get("WAYLAND_DISPLAY"):
        print("selecao-revelado: precisa da sessão Wayland (o Xwayland privado abre uma janela nela)", file=sys.stderr)
        return 2

    url = sys.stdin.readline().strip()
    shutil.rmtree(PASTA, ignore_errors=True)
    os.makedirs(PASTA, mode=0o700)
    home = os.path.join(PASTA, "home")
    rt = os.path.join(PASTA, "rt")
    for p in (home, rt):
        os.makedirs(p, mode=0o700)
    auth = os.path.join(PASTA, "xauth")
    cookie = subprocess.run(["/usr/bin/mcookie"], capture_output=True, text=True, check=True).stdout.strip()
    base = {"PATH": "/usr/bin:/bin", "HOME": home, "LANG": "C.UTF-8"}
    os.close(os.open(auth, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600))
    subprocess.run(["/usr/bin/xauth", "-q", "-f", auth, "add", ":0", ".", cookie], check=True, env=base)

    xr, xw = os.pipe()
    xwayland = subprocess.Popen(
        ["/usr/bin/Xwayland", "-displayfd", str(xw), "-geometry", "1280x800", "-nolisten", "tcp",
         "-auth", auth, "-noreset"],
        pass_fds=(xw,),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env={**base, "XDG_RUNTIME_DIR": os.environ["XDG_RUNTIME_DIR"],
             "WAYLAND_DISPLAY": os.environ["WAYLAND_DISPLAY"]},
    )
    os.close(xw)
    rc = 1
    try:
        numero = b""
        fim = time.monotonic() + 20
        while not numero.endswith(b"\n") and time.monotonic() < fim:
            c = os.read(xr, 1)
            if not c:
                break
            numero += c
        os.close(xr)
        if not numero.strip().isdigit():
            print("FALHA o Xwayland privado não subiu", flush=True)
            return 1
        disp = ":" + numero.decode().strip()
        subprocess.run(["/usr/bin/xauth", "-q", "-f", auth, "add", disp, ".", cookie], check=True, env=base)

        env = {
            **base,
            "DISPLAY": disp,
            "XAUTHORITY": auth,
            "GDK_BACKEND": "x11",
            "XDG_RUNTIME_DIR": rt,
            "XDG_CONFIG_HOME": os.path.join(home, ".config"),
            "XDG_CACHE_HOME": os.path.join(home, ".cache"),
            "XDG_DATA_HOME": os.path.join(home, ".local", "share"),
            "NO_AT_BRIDGE": "1",
            "GTK_A11Y": "none",
            "DBUS_SESSION_BUS_ADDRESS": "unix:path=" + os.path.join(rt, "sem-dbus"),
        }
        # Autoteste do X privado: cliente sem o cookie é recusado.
        sem = subprocess.run(
            [sys.executable, "-I", "-B", "-c",
             "import gi, sys; gi.require_version('Gdk', '3.0'); from gi.repository import Gdk;"
             f" sys.exit(0 if Gdk.Display.open({disp!r}) is None else 1)"],
            env={**env, "XAUTHORITY": os.path.join(PASTA, "nenhum")},
            capture_output=True,
        )
        if sem.returncode != 0:
            print("FALHA o X privado aceitou cliente sem o cookie", flush=True)
            return 1
        print("OK   X privado (sem TCP) recusa cliente sem o cookie", flush=True)

        filho = subprocess.run(
            [sys.executable, "-I", "-B", os.path.abspath(__file__), "--webkit"],
            input=url + "\n",
            text=True,
            env=env,
            timeout=PRAZO_FILHO,
            stderr=subprocess.DEVNULL,
        )
        rc = filho.returncode
    except subprocess.TimeoutExpired:
        print("FALHA o WebKit não terminou no prazo", flush=True)
        rc = 1
    finally:
        xwayland.terminate()
        try:
            xwayland.wait(10)
        except subprocess.TimeoutExpired:
            xwayland.kill()
        shutil.rmtree(PASTA, ignore_errors=True)
    print("selecao-revelado: " + ("ok" if rc == 0 else "FALHOU"), flush=True)
    return rc


if __name__ == "__main__":
    if sys.argv[1:] == ["--ler"]:
        sys.exit(modo_ler())
    if sys.argv[1:] == ["--webkit"]:
        sys.exit(modo_webkit())
    sys.exit(main())
