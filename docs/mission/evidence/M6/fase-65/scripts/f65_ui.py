#!/usr/bin/python3 -I
"""Fase 65 (m2, Bateria E): capturas das telas do app pela ponte de teste e conferências
de UI (V3.2, V3.3, V3.4, V6.8, V6.9 em parte; marca "fora de ordem" dos backups, achado da
Fase 64). Só valores falsos (D15). Nunca imprime segredo: a varredura do DOM conta os
padrões de .dev/variations.txt e só diz quantos achou.

Uso: printf '%s\\n' "$URL" | f65_ui.py <raiz> <pasta do socket> <site-packages> <saída>
"""
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT, SOCKDIR, PW, OUT = sys.argv[1:5]
sys.path.insert(0, PW)
from playwright.sync_api import sync_playwright  # noqa: E402

URL = sys.stdin.readline().strip()
assert URL.startswith("http://127.0.0.1:") and "#token=" in URL
DEV = ROOT + "/.dev"
WARD = ROOT + "/target/debug/ward"
MARK = DEV + "/f65-mark"
# A senha e o PIN de teste (D15) vêm de arquivos 0600 em .dev/, nunca daqui.
PASSWORD = open(DEV + "/f65-password", encoding="utf-8").read().strip()
PIN = open(DEV + "/f65-pin", encoding="utf-8").read().strip()
WRONG_PIN = "924681"
os.makedirs(OUT, exist_ok=True)
with open(DEV + "/variations.txt", "rb") as f:
    PATTERNS = [p for p in f.read().split(b"\n") if p]
ENV = {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/nonexistent"), "LANG": "C.UTF-8",
       "WARD_DEV_SOCKET_DIR": SOCKDIR}
results = []


def check(name, ok, detail=""):
    results.append({"checagem": name, "ok": bool(ok), "detalhe": detail})
    print(("ok - " if ok else "FALHOU - ") + name + (f" ({detail})" if detail else ""), flush=True)


def wait_for(cond, timeout=30.0, step=0.25):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            v = cond()
        except Exception:
            v = None
        if v:
            return v
        time.sleep(step)
    return None


def secrets_in(page):
    """Quantos padrões de .dev/variations.txt aparecem no DOM, nos valores dos campos e no
    armazenamento da página (só a contagem sai daqui)."""
    blob = page.content().encode()
    blob += json.dumps(page.evaluate(
        "() => [Array.from(document.querySelectorAll('input,textarea')).map(e => e.value),"
        " JSON.stringify(localStorage), JSON.stringify(sessionStorage),"
        " Array.from(document.querySelectorAll('[title]')).map(e => e.title)]")).encode()
    return sum(1 for p in PATTERNS if p in blob)


def shot(page, name, full=False):
    page.screenshot(path=f"{OUT}/{name}.png", full_page=full)
    n = secrets_in(page)
    check(f"{name}: DOM, campos, title e armazenamento sem padrão (V3.4)", n == 0, f"{n} padrão(ões)")


def step(name, fn):
    try:
        fn()
    except Exception as e:
        check(f"passo {name} sem exceção", False, f"{type(e).__name__}: {str(e)[:240]}")


def run_in_background(entry, stdin=subprocess.DEVNULL):
    if os.path.exists(MARK):
        os.unlink(MARK)
    return subprocess.Popen([WARD, "run", entry, "--", "touch", MARK], env=ENV, cwd=ROOT,
                            stdin=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def result(proc, timeout=60):
    try:
        _, err = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
        _, err = proc.communicate()
    return proc.returncode, err.decode(errors="replace").strip()


def text(p):
    return p.inner_text("body")


def request_in_window(apr, entry, timeout=40.0):
    return wait_for(lambda: entry in text(apr) and apr.locator("#approval-deny").count() and text(apr), timeout)


def no_request(apr):
    return "Nenhum pedido de aprovação esperando." in text(apr)


def deny(apr):
    apr.click("#approval-deny")
    wait_for(lambda: no_request(apr), 15)


def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path="/opt/google/chrome/chrome", headless=True)
        ctx = browser.new_context(viewport={"width": 1280, "height": 800}, color_scheme="light")
        page = ctx.new_page()
        console = []
        page.on("console", lambda m: console.append(m.type))
        page.goto(URL)
        page.wait_for_selector("#master-password", timeout=20000)

        def unlock_screens():
            time.sleep(0.5)
            focused = page.evaluate("() => document.activeElement && document.activeElement.id")
            check("destravar: foco inicial no campo senha (V3.3)", focused == "master-password", str(focused))
            shot(page, "destravar-padrao")
            page.fill("#master-password", "senha-errada-qa")
            page.press("#master-password", "Enter")
            wait_for(lambda: "incorreta" in text(page), 30)
            time.sleep(0.3)
            shot(page, "destravar-senha-errada")
            check("destravar: erro 'Senha mestra incorreta.'", "Senha mestra incorreta." in text(page))
            wait_for(lambda: page.locator("#master-password").is_enabled(), 30)
            time.sleep(2.5)
            page.fill("#master-password", PASSWORD)
            page.press("#master-password", "Enter")
            page.wait_for_selector("[data-entry]", timeout=60000)
            time.sleep(0.5)
        step("destravar", unlock_screens)

        def credentials():
            n = page.locator("[data-entry]").count()
            check("credenciais: 22 entradas visíveis", n == 22, str(n))
            check("credenciais: resumo '22 entradas · 4 grupos'", "22 entradas · 4 grupos" in text(page))
            bg = page.evaluate("() => getComputedStyle(document.body).backgroundColor")
            check("tema escuro com o sistema claro (sem toggle)", bg == "rgb(16, 17, 20)", bg)
            shot(page, "credenciais-cheia")
            page.fill("#search", "pg")
            time.sleep(0.5)
            check("busca 'pg' filtra 4 itens", page.locator("[data-entry]").count() == 4,
                  str(page.locator("[data-entry]").count()))
            shot(page, "credenciais-busca")
            page.fill("#search", "zzzz-nada")
            time.sleep(0.5)
            check("busca sem resultado: 0 itens", page.locator("[data-entry]").count() == 0)
            shot(page, "credenciais-sem-resultado")
            page.fill("#search", "")
            time.sleep(0.3)
            page.get_by_role("button", name="Filtrar").click()
            time.sleep(0.5)
            shot(page, "credenciais-filtros")
            page.keyboard.press("Escape")
            time.sleep(0.3)
            check("Esc fecha os filtros (V3.3)", page.locator("[role=dialog]").count() == 0
                  and "Limpar" not in text(page), "")
        step("credenciais", credentials)

        def detail_and_level():
            page.locator('[data-entry="acme-pg-dev"]').first.click()
            page.wait_for_selector('h2:has-text("acme-pg-dev")', timeout=10000)
            time.sleep(0.4)
            shot(page, "credenciais-detalhe")
            page.get_by_role("button", name="Mudar nível").click()
            time.sleep(0.5)
            shot(page, "nivel-escolher")
            page.keyboard.press("Escape")
            time.sleep(0.4)
            check("Esc fecha o diálogo de nível (V3.3)", "Nível de acesso" not in text(page)
                  or page.locator("[role=dialog]").count() == 0)
        step("detalhe e nível", detail_and_level)

        def new_credential():
            page.get_by_role("button", name="Nova credencial").click()
            time.sleep(0.5)
            shot(page, "nova-credencial")
            page.keyboard.press("Escape")
            time.sleep(0.3)
        step("nova credencial", new_credential)

        def generator():
            page.get_by_role("link", name="Gerador de senha").or_(
                page.get_by_role("button", name="Gerador de senha")).first.click()
            time.sleep(0.6)
            page.screenshot(path=f"{OUT}/gerador-avulso.png")
            check("gerador: tela aberta", "Gerador de senha" in text(page))
        step("gerador", generator)

        def settings():
            page.get_by_role("link", name="Configurações").or_(
                page.get_by_role("button", name="Configurações")).first.click()
            page.wait_for_selector("#settings-title", timeout=10000)
            time.sleep(0.5)
            shot(page, "configuracoes-geral")
        step("configurações", settings)

        # --- Backups: versões reais e cópias plantadas (achado da Fase 64) -------------
        def backups():
            values = page.evaluate(
                "() => Array.from(document.querySelectorAll('#config-lock-timeout option')).map(o => o.value)")
            current = page.eval_on_selector("#config-lock-timeout", "e => e.value")
            seq = [v for v in values if v != current][:2] + [current]
            for v in seq:
                page.select_option("#config-lock-timeout", v)
                time.sleep(1.5)
            names = sorted(n for n in os.listdir(DEV + "/backups") if re.fullmatch(r"ward-\d{8}-\d{6}(-\d+)?\.ward", n))
            check("backup local: 3 versões novas depois de 3 mudanças", len(names) >= 3, ", ".join(names))

            def shift(name, seconds):
                m = re.fullmatch(r"ward-(\d{8})-(\d{6})(?:-\d+)?\.ward", name)
                t = datetime.datetime.strptime(m.group(1) + m.group(2), "%Y%m%d%H%M%S")
                return (t + datetime.timedelta(seconds=seconds)).strftime("ward-%Y%m%d-%H%M%S.ward")

            oldest, newest = names[0], names[-1]
            a = shift(newest, 60)   # cópia da mais velha com nome mais novo
            b = shift(oldest, -60)  # cópia da mais nova com nome mais velho
            for src, dst in ((oldest, a), (newest, b)):
                shutil.copyfile(f"{DEV}/backups/{src}", f"{DEV}/backups/{dst}")
                os.chmod(f"{DEV}/backups/{dst}", 0o600)
            page.get_by_role("button", name="Restaurar backup").click()
            time.sleep(1.5)
            t = text(page)
            shot(page, "restaurar-backup-fora-de-ordem")
            items = page.evaluate(
                "() => Array.from(document.querySelectorAll('[role=dialog] li, [role=dialog] [role=option],"
                " [role=dialog] [role=radio], [role=dialog] label')).map(e => e.innerText.replace(/\\s+/g, ' '))")
            with open(f"{OUT}/restaurar-backup-itens.json", "w") as f:
                json.dump({"reais": names, "plantadas": {a: f"cópia de {oldest}", b: f"cópia de {newest}"},
                           "itens": items}, f, ensure_ascii=False, indent=1)
            marks = t.count("gravação mais antiga do que a data diz")
            check("backup: a marca 'gravação mais antiga do que a data diz' aparece no app", marks >= 1, f"{marks} item(ns)")
            page.keyboard.press("Escape")
            time.sleep(0.4)
        step("backups", backups)

        # --- Janela de aprovação ------------------------------------------------------
        apr = ctx.new_page()
        apr.set_viewport_size({"width": 560, "height": 760})
        apr.goto(URL + "&screen=approval")
        apr.wait_for_load_state("networkidle")

        def approval_empty():
            check("aprovação sem pedido", wait_for(lambda: no_request(apr), 10))
            shot(apr, "aprovacao-sem-pedido")
        step("aprovação sem pedido", approval_empty)

        def approval_normal():
            proc = run_in_background("acme-pg-aprovacao")
            t = request_in_window(apr, "acme-pg-aprovacao")
            check("aprovação stage: entrada, ambiente e comando", bool(t and "stage" in t and "touch" in t))
            time.sleep(0.5)
            focused = apr.evaluate("() => document.activeElement && document.activeElement.id")
            check("aprovação: foco inicial em Negar (V3.3)", focused == "approval-deny", str(focused))
            for name in ("Negar", "Permitir uma vez", "Permitir por 1 hora", "Permitir sempre"):
                check(f"aprovação: botão {name}", apr.get_by_role("button", name=name, exact=True).count() == 1)
            shot(apr, "aprovacao-normal")
            shot(apr, "aprovacao-normal-inteira", full=True)
            apr.keyboard.press("Escape")
            code, err = result(proc)
            check("Esc na janela nega: CLI sai ≠ 0 e a marca não existe (V3.3)",
                  code != 0 and not os.path.exists(MARK), f"saída={code} {err[:100]}")
            if not no_request(apr):
                deny(apr)
            page.bring_to_front()
            time.sleep(0.5)
            page.screenshot(path=f"{OUT}/pos-negar.png")
            apr.bring_to_front()
        step("aprovação normal", approval_normal)

        def approval_prod_and_wrong_pin():
            proc = run_in_background("acme-pg-prod")
            t = request_in_window(apr, "acme-pg-prod")
            check("aprovação prod: ambiente prod em destaque", bool(t and "prod" in t))
            time.sleep(0.5)
            shot(apr, "aprovacao-prod")
            shot(apr, "aprovacao-prod-inteira", full=True)
            apr.fill("#approval-pin", WRONG_PIN)
            apr.get_by_role("button", name="Permitir uma vez", exact=True).click()
            wait_for(lambda: "PIN" in text(apr) and ("incorreto" in text(apr).lower() or "errado" in text(apr).lower()), 10)
            time.sleep(0.4)
            shot(apr, "aprovacao-pin-errado", full=True)
            check("aprovação: PIN errado mostra erro", "incorreto" in text(apr).lower() or "errado" in text(apr).lower())
            deny(apr)
            code, _ = result(proc)
            check("aprovação prod negada: CLI ≠ 0 e sem marca", code != 0 and not os.path.exists(MARK), f"saída={code}")
        step("aprovação prod e PIN errado", approval_prod_and_wrong_pin)

        def approval_stdin_pipe():
            r, w = os.pipe()
            proc = run_in_background("acme-pg-aprovacao", stdin=r)
            os.close(r)
            request_in_window(apr, "acme-pg-aprovacao")
            time.sleep(0.5)
            apr.fill("#approval-pin", PIN)
            time.sleep(0.3)
            disabled = apr.get_by_role("button", name="Permitir uma vez", exact=True).is_disabled()
            check("entrada padrão em pipe: Permitir só depois da confirmação a mais (V6.8, D68)", disabled)
            apr.fill("#approval-pin", "")
            shot(apr, "aprovacao-orfao", full=True)
            deny(apr)
            os.close(w)
            code, _ = result(proc)
            check("pipe negado: CLI ≠ 0 e sem marca", code != 0 and not os.path.exists(MARK), f"saída={code}")
        step("aprovação com pipe", approval_stdin_pipe)

        def approval_expiring():
            proc = run_in_background("acme-pg-aprovacao")
            request_in_window(apr, "acme-pg-aprovacao")
            ok = wait_for(lambda: "O pedido expira em" in text(apr), 130, 1)
            time.sleep(0.5)
            shot(apr, "aprovacao-expirando", full=True)
            check("aprovação: aviso 'O pedido expira em N s' nos últimos 20 s", bool(ok))
            code, err = result(proc, 60)
            check("sem resposta em 2 min: a CLI sai com 'expirou' (V6.8)",
                  code != 0 and "expir" in err and not os.path.exists(MARK), f"saída={code} {err[:100]}")
            time.sleep(1)
            apr.screenshot(path=f"{OUT}/aprovacao-depois-de-expirar.png", full_page=True)
        step("aprovação expirando", approval_expiring)

        check("console do app sem erro", "error" not in console, ",".join(sorted(set(console))))
        browser.close()

    with open(OUT + "/qa-ui.json", "w") as f:
        json.dump(results, f, ensure_ascii=False, indent=1)
    bad = [r for r in results if not r["ok"]]
    print(f"f65-ui: {len(results) - len(bad)} de {len(results)} checagens ok")
    return 1 if bad else 0


sys.exit(main())
