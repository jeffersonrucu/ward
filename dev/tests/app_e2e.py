#!/usr/bin/python3 -IB
"""Ponta a ponta do app pela ponte de teste (Fase 39c; V5.10, V5.16, V5.24; D2, D3, S10).

Roda dentro de `dev/tests/app-e2e.sh` (`make test-app-e2e`), que já subiu um `wardd` de
teste só dele (socket em `WARD_DEV_SOCKET_DIR`) com a ponte aberta pelo daemon e o cofre
de teste recém-criado. A URL da ponte (com o token) chega pela entrada padrão, nunca em
argv. O Chrome é o instalado, headless, com perfil temporário do Playwright; nada do
perfil do usuário é tocado.

Passos (cada `ward` é a CLI de teste de verdade, como o agente a chamaria):
- V5.10: Novo vazio com "Salvar" desabilitado; com o nome, habilita e salvar mostra o erro
  no campo obrigatório que falta.
- V5.16: `ward run acme-pg-aprovacao -- touch .dev/e2e-mark` abre a janela de aprovação
  com a entrada, o ambiente e o comando. Negar → a CLI sai ≠ 0 e a marca não existe;
  "Fechar e negar" → igual; "Permitir uma vez" com o PIN → executa, e o pedido seguinte
  abre a janela de novo; "Permitir por 1 hora" → o pedido seguinte não abre janela;
  `ward travar` e destravar → abre de novo; "Permitir sempre" → executa, a entrada passa
  a "Livre" no app e o pedido seguinte roda sem janela.
- V5.24: "Conectar" em `acme-ssh-salto` (com aprovação) abre o "terminal" do repositório
  (`dev/tests/terminal-test.py`) com `ward ssh --token-fd` de verdade contra o
  `ssh-salto` (2224): conecta sem janela nem PIN. Ataques pelo próprio terminal, que tem o
  token: varrer `/proc/*/cmdline` e `environ` (0 achados); reusar o token (vale o nível:
  a janela abre e o QA nega); usar depois de 30 s (idem); usar o token de
  `acme-ssh-salto` em `acme-ssh-interno-aprov`, que salta por ele (idem).

Grava `qa.json` e as capturas em `docs/mission/evidence/test-app-e2e/`. Sai 1 se algum
passo falhar. Nunca imprime a URL, o token da ponte nem valor do cofre.
"""

import json
import os
import subprocess
import sys
import time

if len(sys.argv) != 4:
    print("uso: app_e2e.py <raiz> <WARD_DEV_SOCKET_DIR> <site-packages do Playwright> < url", file=sys.stderr)
    sys.exit(2)
RAIZ, SOCKDIR, PW = sys.argv[1:4]
sys.path.insert(0, PW)
from playwright.sync_api import sync_playwright  # noqa: E402

URL = sys.stdin.readline().strip()
assert URL.startswith("http://127.0.0.1:") and "#token=" in URL, "URL da ponte fora da forma"
DEV = RAIZ + "/.dev"
WARD = RAIZ + "/target/debug/ward"
MARCA = DEV + "/e2e-mark"
SENHA = "falso-mestra-ward-01"  # D15 (falsa)
PIN = "135790"  # D15 (falso)
EVID = RAIZ + "/docs/mission/evidence/test-app-e2e"
LOG_TERMINAL = DEV + "/logs/terminal-teste.log"
os.makedirs(EVID, exist_ok=True)

ENV = {
    "PATH": "/usr/bin:/bin",
    "HOME": os.environ.get("HOME", "/nonexistent"),
    "LANG": "C.UTF-8",
    "WARD_DEV_SOCKET_DIR": SOCKDIR,
}

resultados = []


def checar(nome, ok, detalhe=""):
    resultados.append({"checagem": nome, "ok": bool(ok), "detalhe": detalhe})
    print(("ok - " if ok else "FALHOU - ") + nome + (f" ({detalhe})" if detalhe and not ok else ""))


def ward(*args, prazo=180):
    return subprocess.run([WARD, *args], env=ENV, cwd=RAIZ, capture_output=True, timeout=prazo)


def run_em_fundo():
    if os.path.exists(MARCA):
        os.unlink(MARCA)
    return subprocess.Popen([WARD, "run", "acme-pg-aprovacao", "--", "touch", MARCA], env=ENV, cwd=RAIZ,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def esperar(cond, prazo=30.0, passo=0.25):
    fim = time.monotonic() + prazo
    while time.monotonic() < fim:
        v = cond()
        if v:
            return v
        time.sleep(passo)
    return None


def texto(p):
    return p.inner_text("body")


def pedido_na_janela(apr, entrada, prazo=40.0):
    """Espera a janela de aprovação mostrar um pedido de `entrada`."""
    def tem():
        try:
            t = texto(apr)
        except Exception:
            return None
        return t if entrada in t and "Negar" in t and apr.locator("#aprovacao-negar").count() else None
    return esperar(tem, prazo)


def sem_pedido(apr):
    return "Nenhum pedido de aprovação esperando." in texto(apr)


def responder(apr, botao, pin=None):
    if pin is not None:
        apr.fill("#aprovacao-pin", pin)
    if botao == "negar":
        apr.click("#aprovacao-negar")
    elif botao == "fechar":
        apr.get_by_role("button", name="Fechar e negar").click()
    else:
        apr.get_by_role("button", name=botao, exact=True).click()
    # O próximo pedido só depois de este sair da janela (senão o clique seguinte cai no
    # pedido velho, já respondido).
    esperar(lambda: sem_pedido(apr), 15)


def fim_do_run(proc, prazo=60):
    try:
        out, err = proc.communicate(timeout=prazo)
    except subprocess.TimeoutExpired:
        proc.kill()
        out, err = proc.communicate()
    return proc.returncode, err.decode(errors="replace")


def recarregar(pag):
    """A ponte tira o token da URL depois de ler: recarregar perderia o token. Abre de
    novo a URL de abertura (passando por about:blank para não virar só troca de hash)."""
    pag.goto("about:blank")
    pag.goto(URL)


def destravar(pag):
    pag.wait_for_selector("#senha-mestra", timeout=20000)
    pag.fill("#senha-mestra", SENHA)
    pag.press("#senha-mestra", "Enter")
    pag.wait_for_selector("[data-entrada]", timeout=60000)


def abrir_entrada(pag, nome):
    pag.locator(f'[data-entrada="{nome}"]').first.click()
    pag.wait_for_selector(f'h2:has-text("{nome}")', timeout=10000)


def linhas_terminal():
    try:
        with open(LOG_TERMINAL) as f:
            return f.read().splitlines()
    except FileNotFoundError:
        return []


def modo_terminal(modo):
    fd = os.open(DEV + "/terminal-teste.modo", os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(modo + "\n")


def conectar(pag):
    abrir_entrada(pag, "acme-ssh-salto")
    pag.get_by_role("button", name="Editar").click()
    pag.get_by_role("button", name="Conectar").click()


def novas_linhas(antes, prazo, cond):
    def ver():
        n = linhas_terminal()[antes:]
        return n if cond(n) else None
    return esperar(ver, prazo) or linhas_terminal()[antes:]


PAGINAS = []


def main():
    with sync_playwright() as p:
        nav = p.chromium.launch(executable_path="/opt/google/chrome/chrome", headless=True)
        ctx = nav.new_context(viewport={"width": 1280, "height": 800})
        try:
            passos(ctx)
        except Exception as e:  # o passo que quebrou vira uma falha, com captura
            checar("passo sem exceção", False, f"{type(e).__name__}: {str(e)[:300]}")
            for i, pg in enumerate(PAGINAS):
                try:
                    pg.screenshot(path=f"{EVID}/falha-{i}.png")
                except Exception:
                    pass
        nav.close()

    with open(EVID + "/qa.json", "w") as f:
        json.dump(resultados, f, ensure_ascii=False, indent=1)
    falhas = [r for r in resultados if not r["ok"]]
    print(f"app-e2e: {len(resultados) - len(falhas)} de {len(resultados)} checagens ok")
    return 1 if falhas else 0


def passos(ctx):
    if True:
        pag = ctx.new_page()
        PAGINAS.append(pag)
        pag.goto(URL)
        destravar(pag)
        checar("destravar pelo app com a senha de teste", True)
        apr = ctx.new_page()
        PAGINAS.append(apr)
        apr.set_viewport_size({"width": 560, "height": 760})
        apr.goto(URL + "&tela=aprovacao")
        apr.wait_for_load_state("networkidle")
        checar("janela de aprovação aberta sem pedido", esperar(lambda: sem_pedido(apr), 10))

        # --- V5.10: Salvar desabilitado no Novo vazio ----------------------------------
        pag.get_by_role("button", name="Nova credencial").click()
        pag.locator("button:has-text(\"URL, usuário, senha e TOTP\")").click()
        salvar = pag.get_by_role("button", name="Salvar", exact=True)
        salvar.wait_for(timeout=10000)
        checar("V5.10: Novo vazio com Salvar desabilitado", salvar.is_disabled())
        pag.screenshot(path=EVID + "/formulario-novo-vazio.png")
        pag.fill("#form-nome", "acme-e2e-novo")
        checar("V5.10: com o nome preenchido, Salvar habilita", esperar(lambda: salvar.is_enabled(), 5))
        salvar.click()
        erro_campo = esperar(lambda: "obrigat" in texto(pag).lower() or "preencha" in texto(pag).lower(), 10)
        checar("V5.10: salvar sem grupo e ambiente mostra o erro no campo", erro_campo)
        pag.screenshot(path=EVID + "/formulario-novo-erro-no-campo.png")
        pag.get_by_role("button", name="Cancelar").first.click()

        # --- V5.16: Negar e Fechar ---------------------------------------------------------
        for como in ("negar", "fechar"):
            proc = run_em_fundo()
            t = pedido_na_janela(apr, "acme-pg-aprovacao")
            checar(f"V5.16 ({como}): a janela mostra entrada, ambiente e comando",
                   t and "stage" in t and "touch" in t and MARCA in t, (t or "")[:200])
            if como == "negar":
                apr.screenshot(path=EVID + "/aprovacao-ward-run-real.png")
            responder(apr, como)
            codigo, err = fim_do_run(proc)
            checar(f"V5.16 ({como}): a CLI sai ≠ 0 e a marca não existe",
                   codigo != 0 and not os.path.exists(MARCA), f"saida={codigo} {err.strip()[:120]}")

        # --- Permitir uma vez --------------------------------------------------------------
        proc = run_em_fundo()
        pedido_na_janela(apr, "acme-pg-aprovacao")
        responder(apr, "Permitir uma vez", PIN)
        codigo, err = fim_do_run(proc)
        checar("V5.16 (uma vez): executa com o PIN", codigo == 0 and os.path.exists(MARCA),
               f"saida={codigo} {err.strip()[:120]}")
        proc = run_em_fundo()
        checar("V5.16 (uma vez): o pedido seguinte abre a janela de novo",
               pedido_na_janela(apr, "acme-pg-aprovacao"))
        responder(apr, "negar")
        fim_do_run(proc)

        # --- Permitir por 1 hora ------------------------------------------------------------
        proc = run_em_fundo()
        pedido_na_janela(apr, "acme-pg-aprovacao")
        responder(apr, "Permitir por 1 hora", PIN)
        codigo, err = fim_do_run(proc)
        checar("V5.16 (1 hora): executa com o PIN", codigo == 0 and os.path.exists(MARCA), f"saida={codigo}")
        proc = run_em_fundo()
        codigo, err = fim_do_run(proc, prazo=30)
        checar("V5.16 (1 hora): o pedido seguinte roda sem janela",
               codigo == 0 and os.path.exists(MARCA) and sem_pedido(apr), f"saida={codigo} {err.strip()[:120]}")
        r = ward("travar")
        checar("V5.16 (1 hora): ward travar", r.returncode == 0, r.stderr.decode(errors="replace")[:120])
        recarregar(pag)
        destravar(pag)
        proc = run_em_fundo()
        checar("V5.16 (1 hora): depois de travar e destravar, a janela abre de novo",
               pedido_na_janela(apr, "acme-pg-aprovacao"))
        responder(apr, "negar")
        codigo, _ = fim_do_run(proc)
        checar("V5.16 (1 hora): negado depois de travar", codigo != 0 and not os.path.exists(MARCA))

        # --- V5.24: Conectar ---------------------------------------------------------------
        modo_terminal("normal")
        antes = len(linhas_terminal())
        conectar(pag)
        n = novas_linhas(antes, 90, lambda n: any(" uso=1 " in x for x in n))
        uso = next((x for x in n if " uso=1 " in x), "")
        checar("V5.24: Conectar abre o terminal com ward ssh e conecta sem janela nem PIN",
               "saida=0" in uso and "conectou=sim" in uso and sem_pedido(apr), uso)
        pag.screenshot(path=EVID + "/conectar-acme-ssh-salto.png")
        pag.get_by_role("button", name="Cancelar").first.click()

        # Reuso: o mesmo token duas vezes (e a varredura do /proc).
        modo_terminal("reuso")
        antes = len(linhas_terminal())
        conectar(pag)
        n = novas_linhas(antes, 90, lambda n: any(" uso=1 " in x for x in n))
        proc_l = next((x for x in n if " proc=" in x), "")
        checar("V5.24 (ataque): o token não aparece em /proc/*/cmdline nem environ",
               proc_l.split("proc=")[-1].startswith("0/"), proc_l)
        uso1 = next((x for x in n if " uso=1 " in x), "")
        checar("V5.24 (reuso): o primeiro uso conecta", "saida=0" in uso1 and "conectou=sim" in uso1, uso1)
        t = pedido_na_janela(apr, "acme-ssh-salto", prazo=60)
        checar("V5.24 (ataque): reusar o token vale o nível (a janela abre)", t)
        if t:
            apr.screenshot(path=EVID + "/conectar-reuso-pede-aprovacao.png")
            responder(apr, "negar")
        n = novas_linhas(antes, 60, lambda n: any(" uso=2 " in x for x in n))
        uso2 = next((x for x in n if " uso=2 " in x), "")
        checar("V5.24 (ataque): reuso negado, não conecta", "conectou=nao" in uso2 and "saida=0" not in uso2, uso2)
        pag.get_by_role("button", name="Cancelar").first.click()

        # Token de acme-ssh-salto em acme-ssh-interno-aprov (que salta pelo acme-ssh-salto).
        modo_terminal("outra-entrada")
        antes = len(linhas_terminal())
        conectar(pag)
        t = pedido_na_janela(apr, "acme-ssh-", prazo=60)
        checar("V5.24 (ataque): o token de acme-ssh-salto em acme-ssh-interno-aprov vale o nível", t,
               (t or "")[:200])
        if t:
            apr.screenshot(path=EVID + "/conectar-outra-entrada-pede-aprovacao.png")
        if t:
            responder(apr, "negar")
        n = novas_linhas(antes, 60, lambda n: any(" uso=1 " in x for x in n))
        uso = next((x for x in n if " uso=1 " in x), "")
        checar("V5.24 (ataque): outra entrada negada", "conectou=nao" in uso and "saida=0" not in uso, uso)
        pag.get_by_role("button", name="Cancelar").first.click()

        # Depois de 30 s.
        modo_terminal("expirado")
        antes = len(linhas_terminal())
        conectar(pag)
        t = pedido_na_janela(apr, "acme-ssh-salto", prazo=90)
        checar("V5.24 (ataque): usar o token depois de 30 s vale o nível", t)
        if t:
            responder(apr, "negar")
        n = novas_linhas(antes, 60, lambda n: any(" uso=1 " in x for x in n))
        uso = next((x for x in n if " uso=1 " in x), "")
        checar("V5.24 (ataque): token vencido negado", "conectou=nao" in uso and "saida=0" not in uso, uso)
        pag.get_by_role("button", name="Cancelar").first.click()
        modo_terminal("normal")

        # --- Permitir sempre ----------------------------------------------------------------
        proc = run_em_fundo()
        pedido_na_janela(apr, "acme-pg-aprovacao")
        responder(apr, "Permitir sempre", PIN)
        codigo, err = fim_do_run(proc)
        checar("V5.16 (sempre): executa com o PIN", codigo == 0 and os.path.exists(MARCA), f"saida={codigo}")
        recarregar(pag)
        pag.wait_for_selector("[data-entrada]", timeout=30000)
        abrir_entrada(pag, "acme-pg-aprovacao")
        cartao = esperar(lambda: "Livre" in texto(pag), 10)
        checar("V5.16 (sempre): a entrada passa a Livre no app", cartao)
        pag.screenshot(path=EVID + "/sempre-entrada-livre.png")
        proc = run_em_fundo()
        codigo, _ = fim_do_run(proc, prazo=30)
        checar("V5.16 (sempre): o pedido seguinte roda sem janela",
               codigo == 0 and os.path.exists(MARCA) and sem_pedido(apr), f"saida={codigo}")


if __name__ == "__main__":
    sys.exit(main())
