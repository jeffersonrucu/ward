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

m2:Phase 66c (findings A1-A3 of the M6 scrutiny), with screenshots:
- m2:V3.2/V3.4 (screens): unlock (default and wrong password), Credentials full, search,
  no result, filters, detail, reveal and hide (the value only in the field while revealed,
  never in a screenshot), level, new credential, the new and the edit form of each of the
  9 types, generator, Settings and its dialogs (change password, change PIN, restore
  backup, emergency kit); with the fake daemon of the bridge (no `wardd`): first use,
  empty Credentials and choose a USB drive. In each screen the DOM, the field values, the
  `title` attributes and the page storage have no pattern of `.dev/variations.txt`.
- m2:V3.3 (keyboard): initial focus on the password and on Deny, the Tab order of the
  shell and of the approval window, visible focus, arrows in the level options, Esc
  closes the filters, the level dialog and the Settings dialogs, and Esc in the approval
  window denies (the CLI exits ≠ 0 and runs nothing).
- m2:V6.8 (`acme-pg-prod`): the window shows the entry, the `prod` highlight, the exact
  command, the directory and the process chain, with the 4 buttons; each of the three
  "Allow" without a PIN only asks for it (the CLI keeps waiting); a wrong PIN shows the
  error; with the standard input in a pipe the three are disabled until the extra
  confirmation, an "Allow" whose confirmation is taken out of the request on the way is
  refused (attack, nothing runs) and with it the command runs; with no answer, the request
  expires in 2 min (the warning of the last seconds) and the CLI says the request expired.
- m2:V6.9 (`acme-pg-dev`): changing the level asks for the PIN (a wrong one changes
  nothing, the right one changes it; back to free); 4 wrong PINs in the level dialog and
  the 5th in the approval window lock the vault (blocked PIN in the window, the CLI
  denied, the app on the unlock screen); then 5 wrong PINs in the level dialog alone lock
  it again.

Writes `qa.json` and the screenshots to `docs/mission/evidence/test-app-e2e/`, or to the
folder in `E2E_EVIDENCE` (absolute, inside `docs/mission/evidence/`). Exits 1 if any step
fails. Never prints the URL, the bridge token nor a vault value; no screenshot shows a
revealed value.
"""

import json
import os
import re
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
BRIDGE = ROOT + "/target/debug/ward-test-bridge"
DIST = ROOT + "/app/dist-teste"
MARK = DEV + "/e2e-mark"
PASSWORD = "falso-mestra-ward-01"  # D15 (fake)
PIN = "135790"  # D15 (fake)
WRONG_PIN = "924681"  # not the test PIN
EVIDENCE_ROOT = ROOT + "/docs/mission/evidence/"
EVIDENCE = os.environ.get("E2E_EVIDENCE") or EVIDENCE_ROOT + "test-app-e2e"
if not (EVIDENCE.startswith(EVIDENCE_ROOT) and ".." not in EVIDENCE.split("/")):
    print("app_e2e: E2E_EVIDENCE fora de docs/mission/evidence/", file=sys.stderr)
    sys.exit(2)
TERMINAL_LOG = DEV + "/logs/terminal-teste.log"
os.makedirs(EVIDENCE, exist_ok=True)
with open(DEV + "/variations.txt", "rb") as f:
    PATTERNS = [p for p in f.read().split(b"\n") if p]

ENV = {
    "PATH": "/usr/bin:/bin",
    "HOME": os.environ.get("HOME", "/nonexistent"),
    "LANG": "C.UTF-8",
    "WARD_DEV_SOCKET_DIR": SOCKDIR,
}

results = []


def check(name, ok, detail=""):
    results.append({"checagem": name, "ok": bool(ok), "detalhe": detail})
    print(("ok - " if ok else "FALHOU - ") + name + (f" ({detail})" if detail and not ok else ""), flush=True)


def ward(*args, timeout=180):
    return subprocess.run([WARD, *args], env=ENV, cwd=ROOT, capture_output=True, timeout=timeout,
                          stdin=subprocess.DEVNULL)


def run_in_background(entry="acme-pg-aprovacao", stdin=subprocess.DEVNULL):
    if os.path.exists(MARK):
        os.unlink(MARK)
    # Phase 65a: standard input closed. Inherited, it was the pipe through which
    # app-e2e.sh passes the URL, and with a pipe D68 holds the buttons until the extra
    # confirmation (V6.8), which only the m2:Phase 66c step tests (with its own pipe).
    return subprocess.Popen([WARD, "run", entry, "--", "touch", MARK], env=ENV, cwd=ROOT,
                            stdin=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def wait_for(cond, timeout=30.0, step=0.25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            v = cond()
        except Exception:  # noqa: BLE001 (a page in transition: try again)
            v = None
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


def unlock_button_ready(page):
    """The unlock form takes a password now: the submit button has the idle label (not
    the sending one nor the D20 countdown), is enabled and the field is editable."""
    button = page.locator('form button[type="submit"]', has_text=re.compile(r"^\s*Destravar\s*$"))
    return (button.count() == 1 and button.first.is_enabled()
            and page.locator("#master-password").is_editable())


def unlock(page):
    try:
        page.wait_for_selector("#master-password", timeout=20000)
    except Exception:
        # Phase 65c: say why, instead of a bare timeout. The bridge error on the unlock
        # screen means a test bundle that does not match the bridge (left from an older
        # build).
        text = page_text(page)
        if "ponte de teste" in text:
            raise RuntimeError("a tela mostra um erro da ponte de teste em vez do destravar "
                               "(app/dist-teste de outro build? rode 'make test-app-e2e')") from None
        raise
    # A wrong password before makes the next try wait (D20): only the button is
    # disabled (with the countdown) and an Enter in the field is dropped; the field
    # stays enabled. Wait for the idle button to come back (m2:Phase 66b).
    if not wait_for(lambda: unlock_button_ready(page), 40):
        raise RuntimeError("o botão 'Destravar' não voltou depois da espera da senha errada")
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


# --- m2:Phase 66c helpers: screenshots with the DOM scan, focus ------------------------------
def secrets_in(page):
    """How many patterns of .dev/variations.txt are in the DOM, the field values, the
    `title` attributes and the page storage (only the count leaves here)."""
    blob = page.content().encode()
    blob += json.dumps(page.evaluate(
        "() => [Array.from(document.querySelectorAll('input,textarea')).map(e => e.value),"
        " JSON.stringify(localStorage), JSON.stringify(sessionStorage),"
        " Array.from(document.querySelectorAll('[title]')).map(e => e.title)]")).encode()
    return sum(1 for p in PATTERNS if p in blob)


def shot(page, name, full=False):
    """Screenshot of a screen without a revealed value, and the m2:V3.4 scan of it."""
    time.sleep(0.3)
    n = secrets_in(page)
    check(f"V3.4 {name}: DOM, campos, title e armazenamento sem padrão", n == 0, f"{n} padrão(ões)")
    if n == 0:  # a screen with a value never becomes evidence
        page.screenshot(path=f"{EVIDENCE}/{name}.png", full_page=full)


def focused(page):
    return page.evaluate(
        "() => { const e = document.activeElement; if (!e || e === document.body) return null;"
        " return {id: e.id || '', tag: e.tagName.toLowerCase(),"
        " name: (e.getAttribute('aria-label') || e.innerText || e.value || '').trim().slice(0, 40),"
        " visible: e.matches(':focus-visible')}; }")


def dialog_open(page):
    return page.evaluate("() => !!document.querySelector('dialog[open]')")


def clear_window(apr):
    """A request left in the window (an earlier step that broke) is denied."""
    if apr.locator("#approval-deny").count():
        apr.click("#approval-deny")
        wait_for(lambda: no_request(apr), 15)


def ensure_unlocked(page):
    if page.locator("#master-password").count():
        unlock(page)


PAGES = []


def main():
    with sync_playwright() as p:
        # Phase 65a: Chrome in the bwrap of chrome-isolated.sh (no Ward policy, host manifest
        # nor /usr/local), with the Playwright `--disable-extensions`; and, before any page,
        # no extension target may exist (the forced one of the installed Ward included).
        browser = p.chromium.launch(executable_path=ROOT + "/dev/tests/chrome-isolated.sh", headless=True)
        cdp = browser.new_browser_cdp_session()
        targets = cdp.send("Target.getTargets")["targetInfos"]
        cdp.detach()
        extensions = [t for t in targets if t.get("url", "").startswith("chrome-extension://")]
        check("Chrome sem nenhuma extensão (guarda da Fase 65a)", not extensions,
              f"{len(extensions)} alvo(s) de extensão")
        if extensions:
            browser.close()
            sys.exit(3)
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
        # m2:Phase 66c: the screens that need a vault state the test daemon does not have.
        block("telas do daemon falso", lambda: fake_daemon_screens(browser))
        browser.close()

    with open(EVIDENCE + "/qa.json", "w") as f:
        json.dump(results, f, ensure_ascii=False, indent=1)
    failures = [r for r in results if not r["ok"]]
    print(f"app-e2e: {len(results) - len(failures)} de {len(results)} checagens ok")
    return 1 if failures else 0


def block(name, fn):
    """m2:Phase 66c: one block of checks; an exception fails it with a screenshot of each
    page, and the next block still runs."""
    try:
        fn()
    except Exception as e:  # noqa: BLE001
        check(f"bloco {name} sem exceção", False, f"{type(e).__name__}: {str(e)[:300]}")
        slug = re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")
        for i, pg in enumerate(PAGES):
            try:
                pg.screenshot(path=f"{EVIDENCE}/falha-{slug}-{i}.png")
            except Exception:
                pass


def steps(ctx):
    page = ctx.new_page()
    PAGES.append(page)
    page.goto(URL)
    # --- m2:V3.2/V3.3: the unlock screen, before the unlock --------------------------------
    block("destravar", lambda: unlock_screens(page))
    unlock(page)
    check("destravar pelo app com a senha de teste", True)
    apr = ctx.new_page()
    PAGES.append(apr)
    apr.set_viewport_size({"width": 560, "height": 760})
    apr.goto(URL + "&screen=approval")
    apr.wait_for_load_state("networkidle")
    check("janela de aprovação aberta sem pedido", wait_for(lambda: no_request(apr), 10))
    shot(apr, "aprovacao-sem-pedido")
    page.bring_to_front()

    # --- m2:V3.2/V3.3/V3.4: the screens of the app -----------------------------------------
    block("telas", lambda: screens(page))
    ensure_unlocked(page)

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
    page.bring_to_front()
    time.sleep(0.5)
    page.screenshot(path=EVIDENCE + "/pos-negar.png")

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

    # --- m2:V6.8 and m2:V6.9 (findings A1 and A2 of the M6 scrutiny) --------------------------
    block("V6.8 janela de aprovação de acme-pg-prod", lambda: approval_prod(page, apr))
    block("V6.8 entrada padrão em pipe", lambda: approval_pipe(page, apr))
    block("V6.8 expira em 2 min", lambda: approval_expires(page, apr))
    block("V6.9 nível com PIN", lambda: level_with_pin(page, apr))
    block("V6.9 cinco PINs errados", lambda: five_wrong_pins(page, apr))


# --- m2:Phase 66c blocks ----------------------------------------------------------------------
def unlock_screens(page):
    page.wait_for_selector("#master-password", timeout=20000)
    time.sleep(0.5)
    f = focused(page)
    check("V3.3 destravar: foco inicial no campo da senha mestra", f and f["id"] == "master-password", str(f))
    shot(page, "destravar-padrao")
    page.fill("#master-password", "senha-errada-qa")
    page.press("#master-password", "Enter")
    check("V3.2 destravar: 'Senha mestra incorreta.'",
          wait_for(lambda: "Senha mestra incorreta." in page_text(page), 30))
    shot(page, "destravar-senha-errada")
    check("V3.2 destravar: o botão 'Destravar' volta depois da espera",
          wait_for(lambda: unlock_button_ready(page), 40))
    f = focused(page)
    check("V3.3 destravar: o foco volta ao campo depois do erro", f and f["id"] == "master-password", str(f))


def screens(page):
    page.bring_to_front()
    # The shell: entries, summary, theme.
    n = page.locator("[data-entry]").count()
    check("V3.2 credenciais: 22 entradas visíveis", n == 22, str(n))
    check("V3.2 credenciais: resumo '22 entradas · 4 grupos'", "22 entradas · 4 grupos" in page_text(page))
    bg = page.evaluate("() => getComputedStyle(document.body).backgroundColor")
    check("V3.2 tema escuro (rgb(16, 17, 20))", bg == "rgb(16, 17, 20)", bg)
    shot(page, "credenciais-cheia")

    # V3.3: the Tab order of the shell, from the start of the page, with visible focus.
    page.evaluate("() => document.activeElement && document.activeElement.blur()")
    order = []
    for _ in range(7):
        page.keyboard.press("Tab")
        order.append(focused(page))
    names = [(o or {}).get("name", "") for o in order]
    with open(EVIDENCE + "/ordem-de-foco-credenciais.json", "w") as f:
        json.dump(order, f, ensure_ascii=False, indent=1)
    nav = [x for x in ("Credenciais", "Gerador de senha", "Configurações") if x in names]
    check("V3.3 Tab: a navegação na ordem Credenciais, Gerador de senha, Configurações",
          nav == ["Credenciais", "Gerador de senha", "Configurações"]
          and names.index("Credenciais") < names.index("Gerador de senha") < names.index("Configurações"),
          str(names))
    check("V3.3 Tab: foco visível em cada parada", all(o and o["visible"] for o in order),
          str([o and o["visible"] for o in order]))
    search_at = next((i for i, o in enumerate(order) if o and o["id"] == "search"), None)
    check("V3.3 Tab: a busca depois da navegação", search_at is not None
          and search_at > names.index("Configurações"), str(names))
    shift = page.evaluate("() => document.activeElement.id")
    page.keyboard.press("Shift+Tab")
    back = focused(page)
    check("V3.3 Shift+Tab volta uma parada", back is not None and order[-2] is not None
          and back["name"] == order[-2]["name"], f"{shift} → {back}")

    # Search, no result, filters (Esc closes).
    page.fill("#search", "pg")
    time.sleep(0.5)
    check("V3.2 busca 'pg' filtra 4 itens", page.locator("[data-entry]").count() == 4,
          str(page.locator("[data-entry]").count()))
    shot(page, "credenciais-busca")
    page.fill("#search", "zzzz-nada")
    time.sleep(0.5)
    check("V3.2 busca sem resultado: 0 itens", page.locator("[data-entry]").count() == 0)
    shot(page, "credenciais-sem-resultado")
    page.fill("#search", "")
    time.sleep(0.3)
    page.get_by_role("button", name="Filtrar").click()
    check("V3.2 filtros abertos", wait_for(lambda: dialog_open(page), 5))
    shot(page, "credenciais-filtros")
    page.keyboard.press("Escape")
    check("V3.3 Esc fecha os filtros", wait_for(lambda: not dialog_open(page), 5))

    # Detail, reveal and hide (V3.4: the value only in the field while revealed).
    open_entry(page, "acme-pg-dev")
    shot(page, "credenciais-detalhe")
    page.get_by_role("button", name="Revelar Senha").first.click()
    check("V3.4 controle: revelar põe o valor na página (o detector vê)",
          wait_for(lambda: secrets_in(page) > 0, 10))
    page.get_by_role("button", name="Ocultar Senha").first.click()
    check("V3.4 ocultar tira o valor do DOM", wait_for(lambda: secrets_in(page) == 0, 10))
    page.locator('[data-entry="acme-pg-tunel"]').first.click()
    page.wait_for_selector('h2:has-text("acme-pg-tunel")', timeout=10000)
    check("V3.4 sair da entrada: nada do valor no DOM", secrets_in(page) == 0)
    open_entry(page, "acme-pg-dev")

    # Level (Esc closes; arrows change the option).
    page.get_by_role("button", name="Mudar nível").click()
    check("V3.2 nível: diálogo aberto", wait_for(lambda: dialog_open(page), 5))
    shot(page, "nivel-escolher")
    page.locator('input[name="nivel-de-acesso"]:checked').focus()
    before = page.evaluate("() => document.querySelector('input[name=\"nivel-de-acesso\"]:checked').value")
    page.keyboard.press("ArrowUp")
    after = page.evaluate("() => document.querySelector('input[name=\"nivel-de-acesso\"]:checked').value")
    check("V3.3 setas mudam a opção de nível", before != after, f"{before} → {after}")
    page.keyboard.press("Escape")
    check("V3.3 Esc fecha o diálogo de nível sem mudar nada",
          wait_for(lambda: not dialog_open(page), 5) and "Livre" in page.locator("section.detail").inner_text())

    # New credential: the type choice and the new form of each type.
    kinds = ["login", "database", "server", "api", "aws", "smtp", "ftp-sftp", "secure-note", "generic"]
    page.get_by_role("button", name="Nova credencial").click()
    page.wait_for_selector('button[data-type="login"]', timeout=10000)
    check("V3.2 nova credencial: os 9 tipos",
          all(page.locator(f'button[data-type="{k}"]').count() == 1 for k in kinds))
    shot(page, "nova-credencial")
    for k in kinds:
        if not page.locator(f'button[data-type="{k}"]').count():
            page.get_by_role("button", name="Nova credencial").click()
            page.wait_for_selector(f'button[data-type="{k}"]', timeout=10000)
        page.locator(f'button[data-type="{k}"]').click()
        page.get_by_role("button", name="Salvar", exact=True).wait_for(timeout=10000)
        shot(page, f"formulario-novo-{k}", full=True)
        page.get_by_role("button", name="Cancelar").first.click()
        wait_for(lambda: page.locator('button[data-type="login"]').count() or page.locator("[data-entry]").count(), 10)
    if page.locator('button[data-type="login"]').count():
        page.get_by_role("button", name="Cancelar").first.click()

    # The edit form of each type (attachments in the server key and the generic one; TOTP
    # in the login): every secret masked, nothing of a value in the page.
    for k, entry in (("login", "acme-wp-dev"), ("database", "acme-pg-dev"), ("server", "acme-ssh-chave-prod"),
                     ("api", "beta-github-api"), ("aws", "beta-aws-dev"), ("smtp", "beta-smtp-stage"),
                     ("ftp-sftp", "beta-sftp-dev"), ("secure-note", "gama-nota"), ("generic", "gama-generico")):
        page.wait_for_selector("[data-entry]", timeout=10000)
        open_entry(page, entry)
        page.locator("section.detail").get_by_role("button", name="Editar").click()
        page.get_by_role("button", name="Salvar alterações", exact=True).wait_for(timeout=10000)
        shot(page, f"formulario-edicao-{k}", full=True)
        page.get_by_role("button", name="Cancelar").first.click()

    # Generator and Settings (and its dialogs, Esc closes each).
    page.get_by_role("button", name="Gerador de senha").first.click()
    check("V3.2 gerador aberto", wait_for(lambda: "Gerador de senha" in page_text(page), 5))
    page.screenshot(path=f"{EVIDENCE}/gerador-avulso.png")  # a random password, not a vault value
    page.get_by_role("button", name="Configurações").first.click()
    page.wait_for_selector("#settings-title", timeout=10000)
    shot(page, "configuracoes-geral", full=True)
    for label, name in (("Trocar senha mestra", "trocar-senha"), ("Trocar PIN de aprovação", "trocar-pin"),
                        ("Restaurar backup", "restaurar-backup"), ("Imprimir kit de emergência", "kit-de-emergencia")):
        page.get_by_role("button", name=label).click()
        check(f"V3.2 {name}: diálogo aberto", wait_for(lambda: dialog_open(page), 10))
        time.sleep(0.5)
        shot(page, name)
        page.keyboard.press("Escape")
        check(f"V3.3 Esc fecha {name}", wait_for(lambda: not dialog_open(page), 5))
    page.get_by_role("button", name="Credenciais").first.click()
    page.wait_for_selector("[data-entry]", timeout=10000)


def approval_prod(page, apr):
    clear_window(apr)
    apr.bring_to_front()
    proc = run_in_background("acme-pg-prod")
    t = request_in_window(apr, "acme-pg-prod")
    check("V6.8 prod: entrada, ambiente prod, comando exato, diretório e cadeia",
          bool(t and "acme-pg-prod" in t and "prod" in t and f'"touch" "{MARK}"' in t and "Quem pediu" in t
               and "Diretório de trabalho" in t and "Credencial de produção." in t), (t or "")[:300])
    for name in ("Negar", "Permitir uma vez", "Permitir por 1 hora", "Permitir sempre"):
        check(f"V6.8 prod: botão {name}", apr.get_by_role("button", name=name, exact=True).count() == 1)
    time.sleep(0.5)
    f = focused(apr)
    check("V3.3 aprovação: foco inicial em Negar", f and f["id"] == "approval-deny", str(f))
    shot(apr, "aprovacao-prod")
    shot(apr, "aprovacao-prod-inteira", full=True)
    apr.keyboard.press("Tab")
    f = focused(apr)
    check("V3.3 aprovação: Tab de Negar vai a Permitir uma vez", f and f["name"] == "Permitir uma vez", str(f))
    apr.focus("#approval-deny")
    apr.keyboard.press("Shift+Tab")
    f = focused(apr)
    check("V3.3 aprovação: Shift+Tab de Negar vai ao PIN", f and f["id"] == "approval-pin", str(f))
    # Each "Allow" without a PIN only asks for it; nothing is answered.
    for name in ("Permitir uma vez", "Permitir por 1 hora", "Permitir sempre"):
        apr.fill("#approval-pin", "")
        apr.get_by_role("button", name=name, exact=True).click()
        asked = wait_for(lambda: "Digite o PIN para permitir." in page_text(apr), 5)
        check(f"V6.8 {name} sem PIN pede o PIN e o pedido continua",
              asked and proc.poll() is None and request_in_window(apr, "acme-pg-prod", 2))
    apr.fill("#approval-pin", WRONG_PIN)
    apr.get_by_role("button", name="Permitir uma vez", exact=True).click()
    check("V6.8 PIN errado: 'PIN incorreto. Nada foi permitido.'",
          wait_for(lambda: "PIN incorreto. Nada foi permitido." in page_text(apr), 10))
    check("V6.8 PIN errado: o pedido continua", proc.poll() is None and not os.path.exists(MARK))
    shot(apr, "aprovacao-pin-errado", full=True)
    # V3.3: Esc denies.
    apr.keyboard.press("Escape")
    code, err = run_result(proc)
    check("V3.3 Esc na janela nega: a CLI sai ≠ 0 e a marca não existe",
          code != 0 and not os.path.exists(MARK), f"saida={code} {err.strip()[:100]}")
    wait_for(lambda: no_request(apr), 15)


def approval_pipe(page, apr):
    clear_window(apr)
    r, w = os.pipe()
    proc = run_in_background("acme-pg-prod", stdin=r)
    os.close(r)
    os.close(w)  # a pipe at its end: still a standard input the window does not show
    t = request_in_window(apr, "acme-pg-prod")
    check("V6.8 pipe: o aviso da entrada padrão", bool(t and "O comando recebe dados pela entrada padrão" in t))
    buttons = [apr.get_by_role("button", name=n, exact=True)
               for n in ("Permitir uma vez", "Permitir por 1 hora", "Permitir sempre")]
    check("V6.8 pipe: os três de permitir desligados sem a confirmação a mais",
          all(b.is_disabled() for b in buttons))
    apr.fill("#approval-pin", PIN)
    check("V6.8 pipe: com o PIN e sem a confirmação, continuam desligados", all(b.is_disabled() for b in buttons))
    apr.fill("#approval-pin", "")
    shot(apr, "aprovacao-orfao", full=True)
    switch = apr.locator('label[for="approval-stdin"]')
    switch.click()
    check("V6.8 pipe: com a confirmação, os três ligam", wait_for(lambda: all(b.is_enabled() for b in buttons), 5))
    shot(apr, "aprovacao-orfao-confirmado", full=True)

    # Attack: the confirmation taken out of the answer on the way (as a page that does
    # not show the switch would send it) is refused, and nothing runs.
    def strip_acknowledgment(route):
        body = json.loads(route.request.post_data or "{}")
        if "acknowledged" in body:
            body["acknowledged"] = False
        route.continue_(post_data=json.dumps(body))

    apr.route(re.compile(r".*/test-bridge/answer_approval$"), strip_acknowledgment)
    apr.fill("#approval-pin", PIN)
    apr.get_by_role("button", name="Permitir uma vez", exact=True).click()
    time.sleep(2)
    apr.unroute(re.compile(r".*/test-bridge/answer_approval$"))
    check("V6.8 ataque: sem a confirmação no pedido, a resposta é recusada e nada roda",
          proc.poll() is None and not os.path.exists(MARK) and request_in_window(apr, "acme-pg-prod", 5))
    if not apr.locator("#approval-stdin:checked").count():
        switch.click()
    apr.fill("#approval-pin", PIN)
    apr.get_by_role("button", name="Permitir uma vez", exact=True).click()
    code, err = run_result(proc)
    check("V6.8 pipe: com a confirmação e o PIN, o comando roda",
          code == 0 and os.path.exists(MARK), f"saida={code} {err.strip()[:100]}")
    wait_for(lambda: no_request(apr), 15)


def approval_expires(page, apr):
    clear_window(apr)
    proc = run_in_background("acme-pg-prod")
    request_in_window(apr, "acme-pg-prod")
    t0 = time.monotonic()
    warned = wait_for(lambda: "O pedido expira em" in page_text(apr), 130, 1)
    check("V6.8 aviso 'O pedido expira em N s' nos últimos segundos", bool(warned))
    shot(apr, "aprovacao-expirando", full=True)
    code, err = run_result(proc, 60)
    took = time.monotonic() - t0
    check("V6.8 sem resposta em 2 min o pedido expira e a CLI diz 'expirado'",
          code != 0 and "expirado" in err and not os.path.exists(MARK) and 100 <= took <= 135,
          f"saida={code} {took:.0f} s {err.strip()[:100]}")
    check("V6.8 a janela fica sem pedido depois de expirar", wait_for(lambda: no_request(apr), 15))


def change_level(page, entry, level_label, pin):
    """Opens the level dialog of `entry`, chooses `level_label` and sends `pin`."""
    page.bring_to_front()
    open_entry(page, entry)
    page.get_by_role("button", name="Mudar nível").click()
    wait_for(lambda: dialog_open(page), 5)
    page.locator("dialog[open] label").filter(has_text=level_label).first.click()
    page.get_by_role("button", name="Continuar").click()
    page.wait_for_selector("#nivel-pin", timeout=5000)
    page.fill("#nivel-pin", pin)
    page.get_by_role("button", name="Confirmar").click()


def level_value(page):
    return page.locator("section.detail").inner_text()


def level_with_pin(page, apr):
    clear_window(apr)
    ensure_unlocked(page)
    page.bring_to_front()
    open_entry(page, "acme-pg-dev")
    check("V6.9 acme-pg-dev começa Livre", "Livre" in level_value(page))
    page.get_by_role("button", name="Mudar nível").click()
    wait_for(lambda: dialog_open(page), 5)
    page.locator("dialog[open] label").filter(has_text="Com aprovação").first.click()
    page.get_by_role("button", name="Continuar").click()
    page.wait_for_selector("#nivel-pin", timeout=5000)
    check("V6.9 mudar o nível pede o PIN", "Confirmar com o PIN" in page_text(page))
    shot(page, "nivel-pin")
    page.fill("#nivel-pin", WRONG_PIN)
    page.get_by_role("button", name="Confirmar").click()
    check("V6.9 PIN errado: 'PIN incorreto. Nada foi alterado.'",
          wait_for(lambda: "PIN incorreto. Nada foi alterado." in page_text(page), 10))
    shot(page, "nivel-pin-errado")
    page.fill("#nivel-pin", PIN)
    page.get_by_role("button", name="Confirmar").click()
    check("V6.9 com o PIN certo, o nível muda para Com aprovação",
          wait_for(lambda: not dialog_open(page) and "Com aprovação" in level_value(page), 10))
    shot(page, "nivel-mudou")
    proc = run_in_background("acme-pg-dev")
    check("V6.9 com aprovação, o ward run de acme-pg-dev abre a janela", request_in_window(apr, "acme-pg-dev"))
    answer(apr, "negar")
    run_result(proc)
    change_level(page, "acme-pg-dev", "Livre", PIN)
    check("V6.9 de volta a Livre com o PIN", wait_for(lambda: not dialog_open(page) and "Livre" in level_value(page), 10))


def wrong_pins_in_level_dialog(page, n):
    """`n` wrong PINs in a row in the level dialog of acme-pg-dev; returns how many were
    refused with the vault still open."""
    page.bring_to_front()
    open_entry(page, "acme-pg-dev")
    page.get_by_role("button", name="Mudar nível").click()
    wait_for(lambda: dialog_open(page), 5)
    page.locator("dialog[open] label").filter(has_text="Bloqueado").first.click()
    page.get_by_role("button", name="Continuar").click()
    page.wait_for_selector("#nivel-pin", timeout=5000)
    refused = 0
    for i in range(n):
        page.fill("#nivel-pin", WRONG_PIN)
        page.get_by_role("button", name="Confirmar").click()
        if i < n - 1 or n < 5:
            if wait_for(lambda: page.locator('dialog[data-step="pin-errado"]').count()
                        and page.locator("#nivel-pin").input_value() == "", 10):
                refused += 1
            time.sleep(0.3)
    return refused


DAEMON_LOG = DEV + "/logs/wardd.log"
PIN_LOCK_LINE = "5 erros de PIN seguidos: cofre travado (S9)"


def daemon_log():
    try:
        with open(DAEMON_LOG, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def five_wrong_pins(page, apr):
    clear_window(apr)
    ensure_unlocked(page)
    locks_before = daemon_log().count(PIN_LOCK_LINE)
    # The count starts at zero: the last PIN sent was the right one (back to free).
    refused = wrong_pins_in_level_dialog(page, 4)
    check("V6.9 4 PINs errados no nível: recusados, o cofre segue aberto",
          refused == 4 and not page.locator("#master-password").count(), f"{refused} recusado(s)")
    page.keyboard.press("Escape")
    wait_for(lambda: not dialog_open(page), 5)
    check("V6.9 o nível não mudou com PIN errado", "Livre" in level_value(page))
    # The 5th, in the approval window.
    proc = run_in_background("acme-pg-prod")
    request_in_window(apr, "acme-pg-prod")
    apr.fill("#approval-pin", WRONG_PIN)
    apr.get_by_role("button", name="Permitir uma vez", exact=True).click()
    blocked = wait_for(lambda: "Cinco erros seguidos de PIN. O cofre foi travado e o pedido, negado." in page_text(apr)
                       and "PIN bloqueado: o cofre foi travado." in page_text(apr), 15)
    check("V6.9 o 5º PIN errado (na janela) trava o cofre: PIN bloqueado na janela", blocked)
    check("V6.9 PIN bloqueado: os três de permitir desligados",
          all(apr.get_by_role("button", name=n, exact=True).is_disabled()
              for n in ("Permitir uma vez", "Permitir por 1 hora", "Permitir sempre")))
    shot(apr, "aprovacao-pin-bloqueado", full=True)
    code, err = run_result(proc)
    check("V6.9 o pedido do 5º erro é negado: CLI ≠ 0 e a marca não existe",
          code != 0 and not os.path.exists(MARK), f"saida={code} {err.strip()[:100]}")
    apr.click("#approval-deny")
    wait_for(lambda: no_request(apr), 15)
    page.bring_to_front()
    reload_page(page)
    check("V6.9 o app mostra o cofre travado (destravar)", wait_for(lambda: page.locator("#master-password").count(), 20))
    shot(page, "travado-pelos-pins")
    unlock(page)
    # Again, 5 wrong PINs in the level dialog alone.
    refused = wrong_pins_in_level_dialog(page, 5)
    check("V6.9 5 PINs errados no nível: o app vai para o destravar (cofre travado)",
          wait_for(lambda: page.locator("#master-password").count(), 20), f"{refused} recusado(s) antes")
    locks = daemon_log().count(PIN_LOCK_LINE)
    check("V6.9 o log do daemon registra as duas travas por PIN (S9)", locks - locks_before == 2,
          f"{locks - locks_before} trava(s)")
    unlock(page)
    check("V6.9 destravar de novo com a senha mestra", page.locator("[data-entry]").count() > 0)


def fake_daemon_screens(browser):
    """First use, empty Credentials and choose a USB drive, with the fake daemon of the
    bridge (`--fake-daemon`; no `wardd`, no socket, fake values only)."""
    for mode, name in (("no-vault", "primeiro-uso"), ("empty", "credenciais-vazia"),
                       ("external-backup-enabled", "escolher-pendrive")):
        bridge = subprocess.Popen([BRIDGE, "--dist", DIST, "--fake-daemon", mode], stdin=subprocess.DEVNULL,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        try:
            url = bridge.stdout.readline().strip()
            check(f"V3.2 {name}: a ponte com o daemon falso deu a URL",
                  re.fullmatch(r"http://127\.0\.0\.1:[0-9]+/#token=[0-9a-f]{64}", url) is not None)
            ctx = browser.new_context(viewport={"width": 1280, "height": 800})
            pg = ctx.new_page()
            pg.goto(url)
            if mode == "no-vault":
                ok = wait_for(lambda: "Criar seu cofre" in page_text(pg), 15)
                check("V3.2 primeiro uso: 'Criar seu cofre'", ok)
                shot(pg, name)
            elif mode == "empty":
                ok = wait_for(lambda: "Crie a primeira entrada em Nova credencial." in page_text(pg), 15)
                check("V3.2 credenciais vazia: o convite para a primeira entrada", ok)
                shot(pg, name)
            else:
                pg.get_by_role("heading", name="Credenciais").wait_for(timeout=15000)
                pg.get_by_role("button", name="Configurações").first.click()
                pg.wait_for_selector("#settings-title", timeout=10000)
                pg.get_by_role("button", name="Trocar pendrive").click()
                ok = wait_for(lambda: dialog_open(pg), 10)
                check("V3.2 escolher pendrive: diálogo aberto", ok)
                shot(pg, name)
                pg.keyboard.press("Escape")
                check("V3.3 Esc fecha escolher pendrive", wait_for(lambda: not dialog_open(pg), 5))
            ctx.close()
        finally:
            bridge.terminate()
            try:
                bridge.wait(timeout=10)
            except subprocess.TimeoutExpired:
                bridge.kill()
                bridge.wait()


if __name__ == "__main__":
    sys.exit(main())
