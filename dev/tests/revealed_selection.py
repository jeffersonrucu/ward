#!/usr/bin/python3 -IB
"""Phase 39g (S2, S12; 2nd audit of M5): the revealed value does not reach the
clipboard by selection, Ctrl+A, Ctrl+C or drag, in the real WebKitGTK.

Called by `dev/tests/revealed-selection.sh` (`make test-revealed-selection`), which has
already started a test `wardd` of its own with the bridge (D63) and passes the bridge URL
(with the token) on standard input, never in argv.

Everything runs on a **private** X server (Xwayland in rootful mode, with an MIT cookie
in `.dev/revealed-selection/`, no TCP): its PRIMARY and CLIPBOARD are not the ones of the
user's session, which this test never reads or writes. An Xwayland window shows on the
screen for about 20 s. WebKitGTK 4.1 (the same as the app's, through python3-gi) opens
the test interface through the bridge, with fake HOME and XDG and an ephemeral context;
unlocks with the test password, opens `acme-pg-dev` and clicks the reveal button.
Another X process (the "other program") reads both selections and only says whether each
needle is there; no value is printed.

First, the self-test: the server refuses a client without the cookie; a `<span>` with
the value as text (the old component, the audit's attack) when selected goes to
PRIMARY, and the same with Ctrl+C to CLIPBOARD (the reader finds it). Then, with the
current component, none of the attacks below may put the value in either of them, and
the attack's effect is verified (the selection changed) so it does not pass for
nothing:
- selection of the field by script (`addRange`);
- real Ctrl+A (GDK keyboard event), then Ctrl+C;
- mouse drag (GDK events) from the label to the end of the field; triple click;
- selection by script that crosses the field, and `execCommand('copy')` on it;
- the page's `innerText` without the value;
- the field still shows the value (`::before`), and the hide button removes it from the
  document.

Exits 1 if something fails; 2 if Xwayland, xauth or python3-gi's WebKitGTK is missing.
"""

import json
import os
import secrets
import shutil
import subprocess
import sys
import threading
import time

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
WORK_DIR = os.path.join(ROOT, ".dev", "revealed-selection")
PASSWORD = "falso-mestra-ward-01"  # D15 (fake)
ENTRY = "acme-pg-dev"
CHILD_DEADLINE = 150


# --------------------------------------------------------------------------- reader
def read_mode():
    """The "other program": reads PRIMARY and CLIPBOARD of the private X and says, for
    each needle (hex on standard input, one per line), whether it is there. Never prints
    the content."""
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    from gi.repository import Gdk, Gtk

    needles = [bytes.fromhex(l.strip()).decode() for l in sys.stdin if l.strip()]
    out = {}
    for name, sel in (("primary", Gdk.SELECTION_PRIMARY), ("clipboard", Gdk.SELECTION_CLIPBOARD)):
        t = Gtk.Clipboard.get(sel).wait_for_text() or ""
        out[name] = [a in t for a in needles]
    print(json.dumps(out), flush=True)
    return 0


# --------------------------------------------------------------------------- WebKit
def webkit_mode():
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("Gdk", "3.0")
    gi.require_version("WebKit2", "4.1")
    from gi.repository import Gdk, GLib, Gtk, WebKit2

    url = sys.stdin.readline().strip()
    if not (url.startswith("http://127.0.0.1:") and "#token=" in url):
        print("FALHA URL da ponte fora da forma", flush=True)
        return 1

    window = Gtk.Window()
    window.set_default_size(1280, 800)
    context = WebKit2.WebContext.new_ephemeral()
    wv = WebKit2.WebView.new_with_context(context)
    # Only so the test can put a neutral value in the selections between one attack and
    # the next (the app does not turn this on; the user's Ctrl+C does not depend on it).
    wv.get_settings().set_javascript_can_access_clipboard(True)
    window.add(wv)
    window.show_all()
    wv.load_uri(url)

    failures = []

    def ok(msg):
        print(f"OK   {msg}", flush=True)

    def fail(msg):
        failures.append(msg)
        print(f"FALHA {msg}", flush=True)

    def on_main(f):
        """Runs `f` on the GTK thread and returns the result (waits)."""
        done = threading.Event()
        box = {}

        def run():
            try:
                box["v"] = f()
            except Exception as e:  # noqa: BLE001
                box["e"] = e
            done.set()
            return False

        GLib.idle_add(run)
        if not done.wait(30):
            raise RuntimeError("a thread do GTK não respondeu")
        if "e" in box:
            raise box["e"]
        return box.get("v")

    def js(code):
        """Evaluates `code` (an expression; the result goes through JSON) on the page."""
        done = threading.Event()
        box = {}

        def end(_wv, res):
            try:
                v = wv.evaluate_javascript_finish(res)
                box["v"] = json.loads(v.to_json(0) or "null")
            except Exception as e:  # noqa: BLE001
                box["e"] = str(e).split("\n")[0][:200]
            done.set()

        script = f"JSON.parse(JSON.stringify((() => {{ {code} }})() ?? null))"
        GLib.idle_add(lambda: (wv.evaluate_javascript(script, -1, None, None, None, end), False)[1])
        if not done.wait(30):
            raise RuntimeError("o javascript não respondeu")
        if "e" in box:
            raise RuntimeError(f"javascript: {box['e']}")
        return box["v"]

    def wait_for(code, deadline=30.0):
        end = time.monotonic() + deadline
        while time.monotonic() < end:
            try:
                if js(code):
                    return True
            except RuntimeError:
                pass
            time.sleep(0.25)
        return False

    def read_selections(*needles):
        """PRIMARY and CLIPBOARD as seen by another X process."""
        time.sleep(0.6)
        r = subprocess.run(
            [sys.executable, "-I", "-B", os.path.abspath(__file__), "--read"],
            input="".join(a.encode().hex() + "\n" for a in needles),
            capture_output=True,
            text=True,
            timeout=20,
            env=os.environ.copy(),
        )
        if r.returncode != 0:
            raise RuntimeError("o leitor das seleções falhou")
        return json.loads(r.stdout)

    # Real GDK events (the path of the user's keyboard and mouse).
    def key_press(kv, mods):
        def f():
            win = wv.get_window()
            disp = Gdk.Display.get_default()
            keyboard = disp.get_default_seat().get_keyboard()
            _, keycodes = Gdk.Keymap.get_for_display(disp).get_entries_for_keyval(kv)
            for event_type in (Gdk.EventType.KEY_PRESS, Gdk.EventType.KEY_RELEASE):
                e = Gdk.Event.new(event_type)
                e.key.window = win
                e.key.send_event = 0
                e.key.time = Gdk.CURRENT_TIME
                e.key.state = mods
                e.key.keyval = kv
                e.key.hardware_keycode = keycodes[0].keycode
                e.set_device(keyboard)
                wv.event(e)

        on_main(f)

    def mouse(event_type, x, y, button_held=False):
        def f():
            win = wv.get_window()
            pointer = Gdk.Display.get_default().get_default_seat().get_pointer()
            e = Gdk.Event.new(event_type)
            if event_type == Gdk.EventType.MOTION_NOTIFY:
                e.motion.window = win
                e.motion.x, e.motion.y = x, y
                e.motion.state = Gdk.ModifierType.BUTTON1_MASK if button_held else 0
            else:
                e.button.window = win
                e.button.x, e.button.y = x, y
                e.button.button = 1
            e.set_device(pointer)
            wv.event(e)

        on_main(f)

    def click(x, y, times=1):
        event_types = [Gdk.EventType.BUTTON_PRESS, Gdk.EventType.DOUBLE_BUTTON_PRESS, Gdk.EventType.TRIPLE_BUTTON_PRESS]
        for i in range(times):
            mouse(Gdk.EventType.BUTTON_PRESS, x, y)
            if i > 0:
                mouse(event_types[i], x, y)
            mouse(Gdk.EventType.BUTTON_RELEASE, x, y)

    CTRL = Gdk.ModifierType.CONTROL_MASK

    def worker():
        try:
            steps()
        except Exception as e:  # noqa: BLE001
            fail(f"erro no teste: {str(e)[:200]}")
        GLib.idle_add(Gtk.main_quit)

    def steps():
        if not wait_for("return !!document.querySelector('#master-password')", 40):
            fail("a tela Destravar não abriu pela ponte")
            return
        js(
            "const e = document.querySelector('#master-password');"
            "const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;"
            f"set.call(e, {json.dumps(PASSWORD)}); e.dispatchEvent(new Event('input', {{bubbles: true}}));"
            "e.form ? e.form.requestSubmit() : e.dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter', bubbles: true}));"
            "return true"
        )
        if not wait_for(f"return !!document.querySelector('[data-entry=\"{ENTRY}\"]')", 60):
            fail("destravar pela ponte com a senha de teste")
            return
        ok("destravado pela ponte com a senha de teste")
        js(f"document.querySelector('[data-entry=\"{ENTRY}\"]').click(); return true")
        if not wait_for("return !!document.querySelector('button[aria-label^=\"Revelar \"]')", 15):
            fail(f"o detalhe de {ENTRY} não mostrou o botão Revelar")
            return

        def reveal():
            # The value may be in the attribute (current component) or in the text (the
            # old one, so the test can show the attack against it).
            find_js = "const e = document.querySelector('.revealed'); return !!(e && (e.dataset.value || e.textContent))"
            if js(find_js):
                return True
            js("document.querySelector('button[aria-label^=\"Revelar \"]').click(); return true")
            return wait_for(find_js, 15)

        if not reveal():
            fail("o valor não foi revelado")
            return
        value = js("const e = document.querySelector('.revealed'); return e.dataset.value || e.textContent")
        if not isinstance(value, str) or len(value) < 6:
            fail("valor revelado fora da forma")
            return
        if js("return document.querySelector('.revealed').textContent === ''"):
            ok("valor revelado só no atributo do campo, sem nó de texto (não impresso)")
        else:
            fail("o valor revelado é um nó de texto (selecionável)")

        # Controls: a plain text on the page (proves the attack had an effect) and a
        # neutral one to put in both selections before each attack. Through the CSSOM and
        # the DOM, without inline HTML or style (the interface's CSP refuses them).
        control = "falso-39g-controle-" + secrets.token_hex(4)
        neutral = "falso-39g-neutro-" + secrets.token_hex(4)
        js(
            "for (const [id, t] of [['t39g-controle', %s], ['t39g-neutro', %s]]) {"
            " const p = document.createElement('p'); p.id = id; p.textContent = t;"
            " document.querySelector('main, body').appendChild(p); }"
            " return true" % (json.dumps(control), json.dumps(neutral))
        )

        def neutralize():
            js(
                "const r = document.createRange(); r.selectNodeContents(document.getElementById('t39g-neutro'));"
                "const s = getSelection(); s.removeAllRanges(); s.addRange(r);"
                "document.execCommand('copy'); return true"
            )
            v = read_selections(neutral, value)
            if not (v["primary"][0] and v["clipboard"][0]) or v["primary"][1] or v["clipboard"][1]:
                raise RuntimeError("não deu para pôr o valor neutro nas seleções")
            on_main(lambda: wv.grab_focus())

        # ---------------------------------------------------------------- self-test
        # The old component (value as a selectable text node): the audit's attack must be
        # found by the reader.
        neutralize()
        js(
            "const s = document.createElement('span'); s.id = 't39g-antigo';"
            f"s.textContent = {json.dumps(value)}; document.body.appendChild(s);"
            "const r = document.createRange(); r.selectNodeContents(s);"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        v = read_selections(value)
        if v["primary"][0]:
            ok("autoteste: valor como texto selecionado vai à PRIMARY (o leitor acha)")
        else:
            fail("autoteste: o leitor não achou o valor selecionado na PRIMARY")
        key_press(Gdk.KEY_c, CTRL)
        v = read_selections(value)
        if v["clipboard"][0]:
            ok("autoteste: Ctrl+C (evento do GDK) leva o valor como texto à CLIPBOARD")
        else:
            fail("autoteste: Ctrl+C do GDK não levou o texto à CLIPBOARD")
        js("document.getElementById('t39g-antigo').remove(); getSelection().removeAllRanges(); return true")

        # ---------------------------------------------------------------- attacks
        def no_value(name, proof=None):
            """No selection has the value; `proof(v)` says whether the attack had an effect."""
            v = read_selections(value, control, neutral)
            if v["primary"][0] or v["clipboard"][0]:
                where = " e ".join(n for n in ("primary", "clipboard") if v[n][0]).upper()
                fail(f"{name}: o valor revelado foi para a {where}")
                return
            if proof is not None and not proof(v):
                fail(f"{name}: o ataque não teve efeito (o teste não prova nada)")
                return
            ok(f"{name}: o valor não vai à PRIMARY nem à CLIPBOARD")

        def box(sel):
            return js(
                f"const e = document.querySelector({json.dumps(sel)}); if (!e) return null;"
                " const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]"
            )

        # 1. Selection of the field by script.
        reveal()
        neutralize()
        js(
            "const r = document.createRange(); r.selectNodeContents(document.querySelector('.revealed'));"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        no_value("seleção do campo por script")

        # 2. Real Ctrl+A and Ctrl+C.
        reveal()
        neutralize()
        key_press(Gdk.KEY_a, CTRL)
        no_value("Ctrl+A", proof=lambda v: v["primary"][1])
        key_press(Gdk.KEY_c, CTRL)
        no_value("Ctrl+A e Ctrl+C", proof=lambda v: v["clipboard"][1])

        # 3. Mouse drag from the label to the end of the field, and triple click on the
        # field.
        reveal()
        neutralize()
        label = box(".revealed")
        field = js(
            "const e = document.querySelector('.revealed').closest('.field');"
            " const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom]"
        )
        if not label or not field:
            fail("arrasto: não achei o campo revelado na tela")
        else:
            x0, y0 = field[0] + 2, field[1] + 4
            x1, y1 = label[2] - 2, (label[1] + label[3]) / 2
            mouse(Gdk.EventType.BUTTON_PRESS, x0, y0)
            drag_steps = 24
            for i in range(1, drag_steps + 1):
                mouse(
                    Gdk.EventType.MOTION_NOTIFY,
                    x0 + (x1 - x0) * i / drag_steps,
                    y0 + (y1 - y0) * i / drag_steps,
                    button_held=True,
                )
            mouse(Gdk.EventType.BUTTON_RELEASE, x1, y1)
            no_value("arrasto do mouse pelo campo", proof=lambda v: not v["primary"][2])
            reveal()
            neutralize()
            r = box(".revealed")
            click((r[0] + r[2]) / 2, (r[1] + r[3]) / 2, times=3)
            no_value("clique triplo no campo")
            key_press(Gdk.KEY_c, CTRL)
            no_value("clique triplo e Ctrl+C")

        # 4. Selection that crosses the field (from before it to the control) and a copy
        # of it.
        reveal()
        neutralize()
        js(
            "const r = document.createRange();"
            "r.setStartBefore(document.querySelector('.revealed').closest('.field'));"
            "r.setEndAfter(document.getElementById('t39g-controle'));"
            "getSelection().removeAllRanges(); getSelection().addRange(r); return true"
        )
        no_value("seleção que atravessa o campo", proof=lambda v: v["primary"][1])
        js("document.execCommand('copy'); return true")
        no_value("cópia da seleção que atravessa o campo", proof=lambda v: v["clipboard"][1])

        # 5. Serialization of the page and what the field shows.
        reveal()
        if js(f"return document.body.innerText.includes({json.dumps(value)})"):
            fail("innerText da página tem o valor revelado")
        else:
            ok("innerText da página sem o valor revelado")
        before_content = js(
            "return getComputedStyle(document.querySelector('.revealed'), '::before').content"
        )
        if before_content == json.dumps(value):
            ok("o campo continua mostrando o valor (::before)")
        else:
            fail("o campo revelado não mostra o valor")
        js("document.querySelector('button[aria-label^=\"Ocultar \"]').click(); return true")
        if wait_for("return !document.querySelector('.revealed')", 10) and not js(
            f"return document.documentElement.outerHTML.includes({json.dumps(value)})"
        ):
            ok("Ocultar tira o valor do documento")
        else:
            fail("depois de Ocultar o valor continua no documento")
        js(
            "for (const id of ['t39g-controle', 't39g-neutro']) document.getElementById(id)?.remove();"
            "getSelection().removeAllRanges(); return true"
        )

    threading.Thread(target=worker, daemon=True).start()
    Gtk.main()
    return 1 if failures else 0


# --------------------------------------------------------------------------- orchestration
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
    shutil.rmtree(WORK_DIR, ignore_errors=True)
    os.makedirs(WORK_DIR, mode=0o700)
    home = os.path.join(WORK_DIR, "home")
    rt = os.path.join(WORK_DIR, "rt")
    for p in (home, rt):
        os.makedirs(p, mode=0o700)
    auth = os.path.join(WORK_DIR, "xauth")
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
        number = b""
        end = time.monotonic() + 20
        while not number.endswith(b"\n") and time.monotonic() < end:
            c = os.read(xr, 1)
            if not c:
                break
            number += c
        os.close(xr)
        if not number.strip().isdigit():
            print("FALHA o Xwayland privado não subiu", flush=True)
            return 1
        disp = ":" + number.decode().strip()
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
        # Self-test of the private X: a client without the cookie is refused.
        no_cookie = subprocess.run(
            [sys.executable, "-I", "-B", "-c",
             "import gi, sys; gi.require_version('Gdk', '3.0'); from gi.repository import Gdk;"
             f" sys.exit(0 if Gdk.Display.open({disp!r}) is None else 1)"],
            env={**env, "XAUTHORITY": os.path.join(WORK_DIR, "nenhum")},
            capture_output=True,
        )
        if no_cookie.returncode != 0:
            print("FALHA o X privado aceitou cliente sem o cookie", flush=True)
            return 1
        print("OK   X privado (sem TCP) recusa cliente sem o cookie", flush=True)

        child = subprocess.run(
            [sys.executable, "-I", "-B", os.path.abspath(__file__), "--webkit"],
            input=url + "\n",
            text=True,
            env=env,
            timeout=CHILD_DEADLINE,
            stderr=subprocess.DEVNULL,
        )
        rc = child.returncode
    except subprocess.TimeoutExpired:
        print("FALHA o WebKit não terminou no prazo", flush=True)
        rc = 1
    finally:
        xwayland.terminate()
        try:
            xwayland.wait(10)
        except subprocess.TimeoutExpired:
            xwayland.kill()
        shutil.rmtree(WORK_DIR, ignore_errors=True)
    print("selecao-revelado: " + ("ok" if rc == 0 else "FALHOU"), flush=True)
    return rc


if __name__ == "__main__":
    if sys.argv[1:] == ["--read"]:
        sys.exit(read_mode())
    if sys.argv[1:] == ["--webkit"]:
        sys.exit(webkit_mode())
    sys.exit(main())
