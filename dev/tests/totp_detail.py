#!/usr/bin/python3 -I
"""Phase 65b (user request; S11, D4): the detail of an entry with TOTP shows the
current code, never the seed.

Starts the test bridge with the fake daemon of the app core (`--fake-daemon full`, the
fake entries of the test vault; no `wardd`, no socket) and opens the test build in the
installed Chrome, headless, through Python Playwright (temporary profile, extensions off).
Every call of the interface to the bridge is recorded (command, arguments and reply).

Checks:
- `acme-totp-dev` and `acme-wp-dev` (login with seed): the detail shows the TOTP code
  label, the code in two groups of 3 digits and the countdown going down; the seed label
  is not in the detail; the code comes from the TOTP code command of that entry.
- Copy of the code calls the copy TOTP code command of that entry (answered here by the
  test, so the session clipboard is not touched).
- `acme-pg-dev` (no seed): no code and no TOTP code request.
- Attack: every button of the detail is clicked (reveal and copy of the password
  included, copy answered by the test); no call ever names the seed field, and neither
  the seed (and its forms) nor its label shows in any reply or in the page.
- Control (the detector sees the attack): the edit form still has the seed, and
  revealing it there makes the same detector find the seed in the reply and in the page;
  back to the detail, the page has no seed again.

Usage: PW_PYTHONPATH=<site-packages> python3 -I dev/tests/totp_detail.py \\
    <ward-test-bridge> <test build dir>
Exit: 0 all ok, 1 a check failed, 2 usage or setup.
"""

import json
import os
import re
import subprocess
import sys
import time

CHROME = "/opt/google/chrome/chrome"
SEED = "JBSWY3DPEHPK3PXP"
# Forms of the seed that a leak could take: as saved, lowercase, in groups of 4, the
# otpauth URI and the decoded bytes ("Hello!" + 0xdeadbeef).
SEED_FORMS = [
    SEED,
    SEED.lower(),
    "JBSW Y3DP EHPK 3PXP",
    "jbsw y3dp ehpk 3pxp",
    "secret=" + SEED,
    "Hello!",
]
SEED_LABEL = "Semente TOTP"
TOTP_ENTRIES = ["acme-totp-dev", "acme-wp-dev"]
NO_TOTP = "acme-pg-dev"

failures = []


def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + (f" ({detail})" if detail and not ok else ""))
    if not ok:
        failures.append(name)


def visible(page):
    """HTML, text and the values of every field of the page (a revealed value can sit in a
    field value, which the HTML does not carry)."""
    return page.content() + page.evaluate(
        "() => document.body.innerText + '\\n' + [...document.querySelectorAll('input, textarea')]"
        ".map((e) => e.value).join('\\n')"
    )


def leaks(text):
    low = text.lower()
    return [f for f in SEED_FORMS if f.lower() in low]


def main():
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-4], file=sys.stderr)
        return 2
    bridge_bin, dist = sys.argv[1], sys.argv[2]
    pw = os.environ.get("PW_PYTHONPATH", "")
    if not (pw.startswith("/") and os.path.isdir(os.path.join(pw, "playwright"))):
        print("totp-detail: PW_PYTHONPATH=<site-packages com o playwright> (absoluto)", file=sys.stderr)
        return 2
    if not os.access(CHROME, os.X_OK):
        print("totp-detail: falta o Chrome em /opt/google/chrome", file=sys.stderr)
        return 2
    if not (os.access(bridge_bin, os.X_OK) and os.path.isfile(os.path.join(dist, "index.html"))):
        print("totp-detail: falta a ponte de teste ou o app/dist-teste (make app-front)", file=sys.stderr)
        return 2
    sys.path.insert(0, pw)
    from playwright.sync_api import sync_playwright  # noqa: E402

    bridge = subprocess.Popen(
        [bridge_bin, "--dist", dist, "--fake-daemon", "full"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    try:
        url = bridge.stdout.readline().strip()
        if not re.fullmatch(r"http://127\.0\.0\.1:[0-9]+/#token=[0-9a-f]{64}", url):
            print("totp-detail: a ponte não deu a URL", file=sys.stderr)
            return 2
        with sync_playwright() as p:
            browser = p.chromium.launch(
                executable_path=CHROME,
                headless=True,
                args=["--disable-extensions", "--no-first-run", "--disable-sync"],
            )
            try:
                run(browser, url)
            finally:
                browser.close()
    finally:
        bridge.terminate()
        try:
            bridge.wait(timeout=10)
        except subprocess.TimeoutExpired:
            bridge.kill()
            bridge.wait()
    print(f"totp-detail: {len(failures)} falha(s)")
    return 1 if failures else 0


def run(browser, url):
    page = browser.new_page(viewport={"width": 1280, "height": 800})
    calls = []  # (command, arguments, reply text)

    def on_response(resp):
        m = re.search(r"/test-bridge/([a-z_]+)$", resp.url)
        if not m:
            return
        try:
            body = resp.request.post_data or ""
            reply = resp.text()
        except Exception:  # noqa: BLE001 (reply of a routed call)
            reply = ""
        calls.append((m.group(1), body, reply))

    page.on("response", on_response)

    # Copy (field or code) answered here: the session clipboard is never written.
    routed = []

    def fake_copy(route):
        routed.append((route.request.url.rsplit("/", 1)[-1], route.request.post_data or ""))
        route.fulfill(status=200, content_type="application/json", body='{"ok":true,"value":null}')

    page.route(re.compile(r".*/test-bridge/(copy_field|copy_totp_code)$"), fake_copy)

    page.goto(url)
    page.get_by_role("heading", name="Credenciais").wait_for(timeout=15000)
    detail = page.locator("section.detail")

    def open_entry(name):
        page.locator(f'[data-entry="{name}"]').click()
        detail.locator("h2.name", has_text=name).wait_for(timeout=10000)

    def args_of(cmd):
        return [json.loads(b or "{}") for c, b, _ in calls + [(c, b, "") for c, b in routed] if c == cmd]

    def code_checks(entry):
        code = detail.locator('[data-test="codigo-totp"]')
        code.wait_for(timeout=10000)
        page.wait_for_function(
            "() => /^[0-9]{3} [0-9]{3}$/.test(document.querySelector('[data-test=\"codigo-totp\"]')?.textContent?.trim() ?? '')",
            timeout=10000,
        )
        shown = code.inner_text().strip()
        check(f"{entry}: código de 6 dígitos em dois grupos", bool(re.fullmatch(r"[0-9]{3} [0-9]{3}", shown)), shown)
        check(f"{entry}: rótulo Código TOTP", detail.get_by_text("Código TOTP", exact=True).count() == 1)
        check(f"{entry}: sem o rótulo da semente no detalhe", detail.get_by_text(SEED_LABEL).count() == 0)
        check(f"{entry}: o código veio do comando do código TOTP", {"entry": entry} in args_of("totp_code"))
        countdown = detail.locator(".countdown")
        first = int(re.search(r"[0-9]+", countdown.inner_text()).group(0))
        time.sleep(2.2)
        second = int(re.search(r"[0-9]+", countdown.inner_text()).group(0))
        check(f"{entry}: contagem anda", second != first and 1 <= second <= 30, f"{first} → {second}")
        detail.get_by_role("button", name="Copiar código").click()
        page.wait_for_timeout(300)
        check(f"{entry}: Copiar chama copiar o código TOTP", ("copy_totp_code", json.dumps({"entry": entry}, separators=(",", ":"))) in routed, str(routed[-1:]))

    for entry in TOTP_ENTRIES:
        open_entry(entry)
        try:
            code_checks(entry)
        except Exception as e:  # noqa: BLE001 (a missing code is a failure, the attack still runs)
            check(f"{entry}: código TOTP no detalhe", False, type(e).__name__)

        # Attack: every button of the detail but the ones that open other screens.
        buttons = detail.locator("button")
        for i in range(buttons.count()):
            b = buttons.nth(i)
            text = (b.inner_text() or "").strip()
            if text in ("Editar", "Excluir", "Duplicar", "Mudar nível") or not b.is_enabled():
                continue
            b.click()
            page.wait_for_timeout(150)
        page.wait_for_timeout(500)
        check(f"{entry}: página sem a semente depois de todos os botões", not leaks(visible(page)), f"{len(leaks(visible(page)))} forma(s) da semente")

    open_entry(NO_TOTP)
    page.wait_for_timeout(500)
    check(f"{NO_TOTP}: sem código TOTP", detail.locator('[data-test="codigo-totp"]').count() == 0 and detail.get_by_text("Código TOTP").count() == 0)
    check(f"{NO_TOTP}: nenhum pedido de código TOTP", {"entry": NO_TOTP} not in args_of("totp_code"))

    # The whole record of the detail: no call names the seed, no reply carries it.
    named = [(c, b) for c, b, _ in calls if "totp_seed" in b] + [(c, b) for c, b in routed if "totp_seed" in b]
    check("ataque: nenhum pedido do detalhe nomeia a semente", not named, str(named))
    leaked = [(c, len(leaks(r))) for c, _, r in calls if leaks(r)]  # never print the value
    check("ataque: nenhuma resposta ao detalhe traz a semente", not leaked, str(leaked))

    # Control: in the edit form the seed is still there, and revealing it is seen.
    open_entry("acme-totp-dev")
    detail.get_by_role("button", name="Editar").click()
    page.get_by_text(SEED_LABEL, exact=True).first.wait_for(timeout=10000)
    check("controle: a semente aparece no formulário de edição", page.get_by_text(SEED_LABEL, exact=True).count() >= 1)
    before = len(calls)
    page.get_by_role("button", name=f"Revelar {SEED_LABEL}").click()
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline and not any(c == "reveal_field" for c, _, _ in calls[before:]):
        page.wait_for_timeout(100)
    page.wait_for_timeout(300)
    seen = [c for c, b, r in calls[before:] if "totp_seed" in b and leaks(r)]
    check("controle: o detector acha a semente revelada no formulário (pedido e resposta)", seen == ["reveal_field"], str(seen))
    check("controle: o detector acha a semente na página do formulário", bool(leaks(visible(page))))
    page.get_by_role("button", name="Cancelar").click()
    detail.locator("h2.name", has_text="acme-totp-dev").wait_for(timeout=10000)
    page.wait_for_timeout(500)
    check("de volta ao detalhe: página sem a semente", not leaks(visible(page)), f"{len(leaks(visible(page)))} forma(s) da semente")
    check("de volta ao detalhe: sem o rótulo da semente", detail.get_by_text(SEED_LABEL).count() == 0)
    # Screenshot of the detail for the handoff (app side of the FigCraft frame 23159).
    shot = os.environ.get("TOTP_DETAIL_SHOT")
    if shot:
        page.locator("#search").fill("totp")
        page.locator('[data-entry="acme-totp-dev"]').click()
        detail.locator("h2.name", has_text="acme-totp-dev").wait_for(timeout=10000)
        page.wait_for_timeout(1000)
        page.screenshot(path=shot)
    page.close()


if __name__ == "__main__":
    sys.exit(main())
