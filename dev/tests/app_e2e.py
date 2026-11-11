#!/usr/bin/python3 -IB
"""End to end of the app through the test bridge (Phase 39c; V5.10, V5.16, V5.24; D2, D3, S10).

Runs inside `dev/tests/app-e2e.sh` (`make test-app-e2e`), which already started a test
`wardd` of its own (socket in `WARD_DEV_SOCKET_DIR`) with the bridge opened by the daemon
and a newly created test vault. The bridge URL (with the token) comes through standard
input, never in argv. Chrome is the installed one, headless, with a temporary Playwright
profile; nothing of the user's profile is touched.

Steps (each `ward` is the real test CLI, as the agent would call it):
- V5.10: an empty New with the save button disabled; with the name, it enables and saving
  shows the error on the missing required field.
- V5.16: `ward run` on the approval-level Postgres entry with `touch .dev/e2e-mark` opens
  the approval window with the entry, the environment and the command. Deny → the CLI
  exits ≠ 0 and the mark does not exist; close-and-deny → the same; allow once with the
  PIN → runs, and the next request opens the window again; allow for 1 hour → the next
  request does not open a window; locking and unlocking through the CLI → it opens again;
  allow always → runs, the entry turns to the free level in the app and the next request
  runs with no window.
- V5.24: the connect button on the jump host entry (approval level) opens the
  repository's "terminal" (`dev/tests/terminal-test.py`) with a real
  `ward ssh --token-fd` against the jump host service (2224): it connects with no window
  nor PIN. Attacks by the terminal itself, which holds the token: scan `/proc/*/cmdline`
  and `environ` (0 found); reuse the token (the level applies: the window opens and QA
  denies); use it after 30 s (same); use the jump host entry's token on the internal host
  entry with approval, which jumps through it (same).

Writes `qa.json` and the screenshots to `docs/mission/evidence/test-app-e2e/`. Exits 1 if
any step fails. Never prints the URL, the bridge token nor a vault value.
"""

import json
import os
import subprocess
import sys
import time

if len(sys.argv) != 4:
    print("uso: app_e2e.py <raiz> <WARD_DEV_SOCKET_DIR> <site-packages do Playwright> < url", file=sys.stderr)
    sys.exit(2)
ROOT, SOCKDIR, PW = sys.argv[1:4]
sys.path.insert(0, PW)
from playwright.sync_api import sync_playwright  # noqa: E402

URL = sys.stdin.readline().strip()
assert URL.startswith("http://127.0.0.1:") and "#token=" in URL, "URL da ponte fora da forma"
DEV = ROOT + "/.dev"
WARD = ROOT + "/target/debug/ward"
MARK = DEV + "/e2e-mark"
PASSWORD = "falso-mestra-ward-01"  # D15 (fake)
PIN = "135790"  # D15 (fake)
EVIDENCE = ROOT + "/docs/mission/evidence/test-app-e2e"
TERMINAL_LOG = DEV + "/logs/terminal-teste.log"
os.makedirs(EVIDENCE, exist_ok=True)

ENV = {
    "PATH": "/usr/bin:/bin",
    "HOME": os.environ.get("HOME", "/nonexistent"),
    "LANG": "C.UTF-8",
    "WARD_DEV_SOCKET_DIR": SOCKDIR,
}

results = []


def check(name, ok, detail=""):
    results.append({"checagem": name, "ok": bool(ok), "detalhe": detail})
    print(("ok - " if ok else "FALHOU - ") + name + (f" ({detail})" if detail and not ok else ""))


def ward(*args, timeout=180):
    return subprocess.run([WARD, *args], env=ENV, cwd=ROOT, capture_output=True, timeout=timeout)


def run_in_background():
    if os.path.exists(MARK):
        os.unlink(MARK)
    return subprocess.Popen([WARD, "run", "acme-pg-aprovacao", "--", "touch", MARK], env=ENV, cwd=ROOT,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def wait_for(cond, timeout=30.0, step=0.25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        v = cond()
        if v:
            return v
        time.sleep(step)
    return None


def page_text(p):
    return p.inner_text("body")


def request_in_window(apr, entry, timeout=40.0):
    """Waits for the approval window to show a request for `entry`."""
    def has_request():
        try:
            t = page_text(apr)
        except Exception:
            return None
        return t if entry in t and "Negar" in t and apr.locator("#approval-deny").count() else None
    return wait_for(has_request, timeout)


def no_request(apr):
    return "Nenhum pedido de aprovação esperando." in page_text(apr)


def answer(apr, button, pin=None):
    if pin is not None:
        apr.fill("#approval-pin", pin)
    if button == "negar":
        apr.click("#approval-deny")
    elif button == "fechar":
        apr.get_by_role("button", name="Fechar e negar").click()
    else:
        apr.get_by_role("button", name=button, exact=True).click()
    # The next request only after this one leaves the window (otherwise the next click
    # lands on the old, already answered request).
    wait_for(lambda: no_request(apr), 15)


def run_result(proc, timeout=60):
    try:
        out, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
        out, err = proc.communicate()
    return proc.returncode, err.decode(errors="replace")


def reload_page(page):
    """The bridge removes the token from the URL after reading it: reloading would lose the
    token. Opens the opening URL again (through about:blank so it is not just a hash change)."""
    page.goto("about:blank")
    page.goto(URL)


def unlock(page):
    page.wait_for_selector("#master-password", timeout=20000)
    page.fill("#master-password", PASSWORD)
    page.press("#master-password", "Enter")
    page.wait_for_selector("[data-entry]", timeout=60000)


def open_entry(page, name):
    page.locator(f'[data-entry="{name}"]').first.click()
    page.wait_for_selector(f'h2:has-text("{name}")', timeout=10000)


def terminal_lines():
    try:
        with open(TERMINAL_LOG) as f:
            return f.read().splitlines()
    except FileNotFoundError:
        return []


def set_terminal_mode(mode):
    fd = os.open(DEV + "/terminal-teste.modo", os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(mode + "\n")


def connect(page):
    open_entry(page, "acme-ssh-salto")
    page.get_by_role("button", name="Editar").click()
    page.get_by_role("button", name="Conectar").click()


def new_lines(before, timeout, cond):
    def probe():
        n = terminal_lines()[before:]
        return n if cond(n) else None
    return wait_for(probe, timeout) or terminal_lines()[before:]


PAGES = []


def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path="/opt/google/chrome/chrome", headless=True)
        ctx = browser.new_context(viewport={"width": 1280, "height": 800})
        try:
            steps(ctx)
        except Exception as e:  # the step that broke becomes a failure, with a screenshot
            check("passo sem exceção", False, f"{type(e).__name__}: {str(e)[:300]}")
            for i, pg in enumerate(PAGES):
                try:
                    pg.screenshot(path=f"{EVIDENCE}/falha-{i}.png")
                except Exception:
                    pass
        browser.close()

    with open(EVIDENCE + "/qa.json", "w") as f:
        json.dump(results, f, ensure_ascii=False, indent=1)
    failures = [r for r in results if not r["ok"]]
    print(f"app-e2e: {len(results) - len(failures)} de {len(results)} checagens ok")
    return 1 if failures else 0


def steps(ctx):
    if True:
        page = ctx.new_page()
        PAGES.append(page)
        page.goto(URL)
        unlock(page)
        check("destravar pelo app com a senha de teste", True)
        apr = ctx.new_page()
        PAGES.append(apr)
        apr.set_viewport_size({"width": 560, "height": 760})
        apr.goto(URL + "&screen=approval")
        apr.wait_for_load_state("networkidle")
        check("janela de aprovação aberta sem pedido", wait_for(lambda: no_request(apr), 10))

        # --- V5.10: Save disabled on an empty New ---------------------------------------
        page.get_by_role("button", name="Nova credencial").click()
        page.locator("button:has-text(\"URL, usuário, senha e TOTP\")").click()
        save_button = page.get_by_role("button", name="Salvar", exact=True)
        save_button.wait_for(timeout=10000)
        check("V5.10: Novo vazio com Salvar desabilitado", save_button.is_disabled())
        page.screenshot(path=EVIDENCE + "/formulario-novo-vazio.png")
        page.fill("#form-name", "acme-e2e-novo")
        check("V5.10: com o nome preenchido, Salvar habilita", wait_for(lambda: save_button.is_enabled(), 5))
        save_button.click()
        field_error = wait_for(lambda: "obrigat" in page_text(page).lower() or "preencha" in page_text(page).lower(), 10)
        check("V5.10: salvar sem grupo e ambiente mostra o erro no campo", field_error)
        page.screenshot(path=EVIDENCE + "/formulario-novo-erro-no-campo.png")
        page.get_by_role("button", name="Cancelar").first.click()

        # --- V5.16: Deny and Close -----------------------------------------------------------
        for how in ("negar", "fechar"):
            proc = run_in_background()
            t = request_in_window(apr, "acme-pg-aprovacao")
            check(f"V5.16 ({how}): a janela mostra entrada, ambiente e comando",
                  t and "stage" in t and "touch" in t and MARK in t, (t or "")[:200])
            if how == "negar":
                apr.screenshot(path=EVIDENCE + "/aprovacao-ward-run-real.png")
            answer(apr, how)
            code, err = run_result(proc)
            check(f"V5.16 ({how}): a CLI sai ≠ 0 e a marca não existe",
                  code != 0 and not os.path.exists(MARK), f"saida={code} {err.strip()[:120]}")

        # --- Allow once ----------------------------------------------------------------------
        proc = run_in_background()
        request_in_window(apr, "acme-pg-aprovacao")
        answer(apr, "Permitir uma vez", PIN)
        code, err = run_result(proc)
        check("V5.16 (uma vez): executa com o PIN", code == 0 and os.path.exists(MARK),
              f"saida={code} {err.strip()[:120]}")
        proc = run_in_background()
        check("V5.16 (uma vez): o pedido seguinte abre a janela de novo",
              request_in_window(apr, "acme-pg-aprovacao"))
        answer(apr, "negar")
        run_result(proc)

        # --- Allow for 1 hour ------------------------------------------------------------------
        proc = run_in_background()
        request_in_window(apr, "acme-pg-aprovacao")
        answer(apr, "Permitir por 1 hora", PIN)
        code, err = run_result(proc)
        check("V5.16 (1 hora): executa com o PIN", code == 0 and os.path.exists(MARK), f"saida={code}")
        proc = run_in_background()
        code, err = run_result(proc, timeout=30)
        check("V5.16 (1 hora): o pedido seguinte roda sem janela",
              code == 0 and os.path.exists(MARK) and no_request(apr), f"saida={code} {err.strip()[:120]}")
        r = ward("lock")
        check("V5.16 (1 hora): ward lock", r.returncode == 0, r.stderr.decode(errors="replace")[:120])
        reload_page(page)
        unlock(page)
        proc = run_in_background()
        check("V5.16 (1 hora): depois de travar e destravar, a janela abre de novo",
              request_in_window(apr, "acme-pg-aprovacao"))
        answer(apr, "negar")
        code, _ = run_result(proc)
        check("V5.16 (1 hora): negado depois de travar", code != 0 and not os.path.exists(MARK))

        # --- V5.24: Connect ----------------------------------------------------------------
        set_terminal_mode("normal")
        before = len(terminal_lines())
        connect(page)
        n = new_lines(before, 90, lambda n: any(" uso=1 " in x for x in n))
        use_line = next((x for x in n if " uso=1 " in x), "")
        check("V5.24: Conectar abre o terminal com ward ssh e conecta sem janela nem PIN",
              "saida=0" in use_line and "conectou=sim" in use_line and no_request(apr), use_line)
        page.screenshot(path=EVIDENCE + "/conectar-acme-ssh-salto.png")
        page.get_by_role("button", name="Cancelar").first.click()

        # Reuse: the same token twice (and the /proc scan).
        set_terminal_mode("reuso")
        before = len(terminal_lines())
        connect(page)
        n = new_lines(before, 90, lambda n: any(" uso=1 " in x for x in n))
        proc_line = next((x for x in n if " proc=" in x), "")
        check("V5.24 (ataque): o token não aparece em /proc/*/cmdline nem environ",
              proc_line.split("proc=")[-1].startswith("0/"), proc_line)
        use1 = next((x for x in n if " uso=1 " in x), "")
        check("V5.24 (reuso): o primeiro uso conecta", "saida=0" in use1 and "conectou=sim" in use1, use1)
        t = request_in_window(apr, "acme-ssh-salto", timeout=60)
        check("V5.24 (ataque): reusar o token vale o nível (a janela abre)", t)
        if t:
            apr.screenshot(path=EVIDENCE + "/conectar-reuso-pede-aprovacao.png")
            answer(apr, "negar")
        n = new_lines(before, 60, lambda n: any(" uso=2 " in x for x in n))
        use2 = next((x for x in n if " uso=2 " in x), "")
        check("V5.24 (ataque): reuso negado, não conecta", "conectou=nao" in use2 and "saida=0" not in use2, use2)
        page.get_by_role("button", name="Cancelar").first.click()

        # The jump host entry's token on the internal host entry with approval (which jumps
        # through the jump host entry).
        set_terminal_mode("outra-entrada")
        before = len(terminal_lines())
        connect(page)
        t = request_in_window(apr, "acme-ssh-", timeout=60)
        check("V5.24 (ataque): o token de acme-ssh-salto em acme-ssh-interno-aprov vale o nível", t,
              (t or "")[:200])
        if t:
            apr.screenshot(path=EVIDENCE + "/conectar-outra-entrada-pede-aprovacao.png")
        if t:
            answer(apr, "negar")
        n = new_lines(before, 60, lambda n: any(" uso=1 " in x for x in n))
        use_line = next((x for x in n if " uso=1 " in x), "")
        check("V5.24 (ataque): outra entrada negada", "conectou=nao" in use_line and "saida=0" not in use_line, use_line)
        page.get_by_role("button", name="Cancelar").first.click()

        # After 30 s.
        set_terminal_mode("expirado")
        before = len(terminal_lines())
        connect(page)
        t = request_in_window(apr, "acme-ssh-salto", timeout=90)
        check("V5.24 (ataque): usar o token depois de 30 s vale o nível", t)
        if t:
            answer(apr, "negar")
        n = new_lines(before, 60, lambda n: any(" uso=1 " in x for x in n))
        use_line = next((x for x in n if " uso=1 " in x), "")
        check("V5.24 (ataque): token vencido negado", "conectou=nao" in use_line and "saida=0" not in use_line, use_line)
        page.get_by_role("button", name="Cancelar").first.click()
        set_terminal_mode("normal")

        # --- Allow always ----------------------------------------------------------------
        proc = run_in_background()
        request_in_window(apr, "acme-pg-aprovacao")
        answer(apr, "Permitir sempre", PIN)
        code, err = run_result(proc)
        check("V5.16 (sempre): executa com o PIN", code == 0 and os.path.exists(MARK), f"saida={code}")
        reload_page(page)
        page.wait_for_selector("[data-entry]", timeout=30000)
        open_entry(page, "acme-pg-aprovacao")
        card = wait_for(lambda: "Livre" in page_text(page), 10)
        check("V5.16 (sempre): a entrada passa a Livre no app", card)
        page.screenshot(path=EVIDENCE + "/sempre-entrada-livre.png")
        proc = run_in_background()
        code, _ = run_result(proc, timeout=30)
        check("V5.16 (sempre): o pedido seguinte roda sem janela",
              code == 0 and os.path.exists(MARK) and no_request(apr), f"saida={code}")


if __name__ == "__main__":
    sys.exit(main())
