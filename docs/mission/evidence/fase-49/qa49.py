"""QA da Fase 49 (V7.10): kit de emergência pela ponte, Chrome headless 1280x800.

Uso: qa49.py <saida-da-ponte> <chrome> <pasta-evidencia> <variacoes.txt|-> <rotulo> [pendrive-esperado]
`pendrive-esperado` (opcional): texto que o item "Pendrive" da folha precisa conter.
O PDF é gerado em memória (page.pdf sem path) e só o texto extraído é conferido; nada
dele é gravado no repositório.
"""
import json
import re
import subprocess
import sys
import time

sys.path.insert(0, "/tmp/claude-1000/-home-jeffersonoliveira-projetos-cofre/d2e230bd-a662-43be-afff-2572da5a4e80/scratchpad/f49/site")
from playwright.sync_api import sync_playwright  # noqa: E402

saida_ponte, chrome, pasta, variacoes, rotulo = sys.argv[1:6]
pendrive_esperado = sys.argv[6] if len(sys.argv) > 6 else "backup externo desligado"
url = None
for _ in range(600):
    m = re.search(r"http://127\.0\.0\.1:\d+/#token=[0-9a-f]{64}", open(saida_ponte).read())
    if m:
        url = m.group(0)
        break
    time.sleep(0.5)
assert url, "ponte não subiu"

# Segredos a procurar: a senha mestra e o PIN de teste (D15), mais a lista de variações
# (quando houver). Nunca impressos: só a contagem de achados.
padroes = ["falso-mestra-ward-01", "135790"]
if variacoes != "-":
    padroes += [l.rstrip("\n") for l in open(variacoes, encoding="utf-8", errors="replace") if l.strip()]


def achados(texto):
    return sum(1 for p in padroes if p and p in texto)


r = {}
with sync_playwright() as p:
    nav = p.chromium.launch(executable_path=chrome, headless=True)
    ctx = nav.new_context(viewport={"width": 1280, "height": 800})
    pedidos, ruins, console = [], [], []
    ctx.on("request", lambda q: pedidos.append(q.url))
    favicons = []
    ctx.on("response", lambda q: q.status >= 400 and (favicons if "favicon" in q.url else ruins).append((q.status, q.url)))
    pg = ctx.new_page()
    pg.on("console", lambda m: m.type == "error" and console.append(f"{m.text} @ {m.location.get('url', '').split('#')[0]}"))
    pg.goto(url + "&tela=configuracoes")
    pg.wait_for_selector("#config-kit")
    pg.wait_for_timeout(500)
    pg.evaluate("() => document.fonts.ready")
    botao = pg.locator("button[aria-label='Imprimir kit de emergência']")
    r["botao_habilitado"] = botao.is_enabled()
    pg.screenshot(path=f"{pasta}/configuracoes-kit-habilitado-{rotulo}.png")

    # window.print contado (no Chrome headless ele não abre janela).
    pg.evaluate("() => { window.__impressoes = 0; window.print = () => { window.__impressoes++; }; }")
    botao.click()
    pg.wait_for_selector("dialog.kit[open]")
    pg.wait_for_selector("[data-teste=kit-pendrive] .valor:not(:has-text('Lendo'))")
    pg.evaluate("() => document.fonts.ready")
    pg.wait_for_timeout(300)
    r["dialogo_no_body"] = pg.evaluate("() => document.querySelector('dialog.kit').parentElement === document.body")
    folha = pg.locator("[data-teste=folha-kit]")
    caixa = folha.bounding_box()
    r["folha_794x1123"] = round(caixa["width"]) == 794 and round(caixa["height"]) == 1123
    texto = folha.inner_text()
    r["tem_cofre"] = "~/.local/share/ward/" in texto
    r["tem_backups"] = "~/.local/share/ward/backups/" in texto
    r["pendrive"] = pg.inner_text("[data-teste=kit-pendrive]")
    r["pendrive_esperado"] = pendrive_esperado in r["pendrive"]
    r["senha_linhas_em_branco"] = pg.evaluate(
        "() => [...document.querySelectorAll('.folha .linha')].map(l => l.textContent.trim().length)"
    ) == [0, 0]
    r["sem_campo_na_folha"] = pg.locator("[data-teste=folha-kit] input, [data-teste=folha-kit] textarea, [data-teste=folha-kit] [contenteditable]").count() == 0
    r["tem_data"] = re.search(r"Impresso em \d{2}/\d{2}/\d{4}", texto) is not None
    pg.screenshot(path=f"{pasta}/kit-de-emergencia-1280x800-{rotulo}.png")
    folha.screenshot(path=f"{pasta}/kit-de-emergencia-folha-{rotulo}.png")

    pg.click("button[aria-label='Imprimir a folha']")
    r["window_print_chamado"] = pg.evaluate("() => window.__impressoes") == 1

    html = pg.content()
    r["dom_achados"] = achados(html) + achados(pg.evaluate("() => document.body.innerText"))

    # Ataque: uma isca com a senha mestra de teste fora da folha (no #app, atrás do
    # kit). A impressão só pode levar a folha: a isca não pode sair no PDF.
    pg.evaluate("""() => { const d = document.createElement('div'); d.id = 'isca';
      d.textContent = 'isca ' + 'falso-mestra-' + 'ward-01'; document.getElementById('app').appendChild(d); }""")
    # Controle: a busca acha a isca no DOM (então "0 achados" quer dizer algo).
    r["controle_busca_acha_isca_no_dom"] = achados(pg.content()) >= 1
    # PDF como a impressão o veria (media print), só em memória.
    pdf = pg.pdf(format="A4", print_background=True, prefer_css_page_size=True)
    txt = subprocess.run(["pdftotext", "-layout", "-", "-"], input=pdf, capture_output=True, check=True).stdout.decode()
    paginas = subprocess.run(["pdfinfo", "-"], input=pdf, capture_output=True).stdout.decode()
    m = re.search(r"Pages:\s+(\d+)", paginas)
    r["pdf_paginas"] = int(m.group(1)) if m else None
    r["pdf_uma_pagina"] = r["pdf_paginas"] == 1
    r["pdf_so_folha"] = "Kit de emergência" in txt and "Confira a folha" not in txt and "Acesso" not in txt
    r["pdf_tem_pastas"] = "~/.local/share/ward/backups/" in txt
    r["pdf_achados"] = achados(txt)
    r["isca_fora_do_pdf"] = "isca" not in txt
    pg.evaluate("() => document.getElementById('isca').remove()")
    # Imagem da página impressa para conferência (render do PDF pelo pdftoppm).
    subprocess.run(["pdftoppm", "-png", "-r", "96", "-singlefile", "-", f"{pasta}/kit-impresso-pdf-{rotulo}"], input=pdf, check=False)

    # Esc fecha, o diálogo sai e nada fica no armazenamento nem na URL.
    pg.keyboard.press("Escape")
    pg.wait_for_timeout(300)
    r["esc_fecha"] = pg.locator("dialog.kit[open]").count() == 0
    st = pg.evaluate("() => [localStorage.length, sessionStorage.length, location.href]")
    r["sem_storage"] = st[0] == 0 and st[1] == 0
    r["url_limpa"] = "token" not in st[2] and "kit" not in st[2]
    # Reabrir funciona de novo.
    botao.click()
    pg.wait_for_selector("dialog.kit[open]")
    pg.click("text=Fechar")
    pg.wait_for_timeout(300)
    r["fechar_fecha"] = pg.locator("dialog.kit[open]").count() == 0

    r["so_pedidos_locais"] = all(u.startswith("http://127.0.0.1:") for u in pedidos)
    r["sem_http_erro"] = not ruins
    # O 404 do /favicon.ico aparece no console como recurso que não carregou.
    # O 404 do /favicon.ico (a ponte não serve ícone; o Chrome pede duas vezes, uma
    # pela página e uma pelo PDF) é o único erro descontado.
    console = [c for c in console if not c.endswith("/favicon.ico")]
    r["console_erros"] = console
    r["console_sem_erro"] = not console
    nav.close()

r["padroes_procurados"] = len(padroes)
ok = (
    all(v for k, v in r.items() if isinstance(v, bool))
    and r["dom_achados"] == 0
    and r["pdf_achados"] == 0
)
r["ok"] = ok
print(json.dumps(r, ensure_ascii=False, indent=2))
sys.exit(0 if ok else 1)
