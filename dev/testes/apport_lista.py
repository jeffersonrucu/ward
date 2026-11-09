#!/usr/bin/python3 -I
"""Fase 51c (S2, S8): a lista do apport instalada pelo Ward, lida pelo código do apport.

Uso: apport_lista.py <pasta da lista> <ignorado>... -- <não ignorado>...

Usa o `Report.check_ignored()` do próprio apport (o mesmo do `/usr/share/apport/apport`
no crash) com a pasta da lista apontada para <pasta> (a do DESTDIR ou a do sistema do
contêiner). Cada caminho antes de `--` tem de ser ignorado (sem relatório em /var/crash)
e cada um depois, não (controle: a lista não cala tudo). A lista do usuário
(~/.apport-ignore.xml) fica de fora: o teste não lê o home. Sai 0 se tudo confere; sem
o módulo do apport, sai 3.
"""

import sys
import xml.dom.minidom


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 3 or "--" not in args:
        print(__doc__, file=sys.stderr)
        return 2
    corte = args.index("--")
    pasta, ignorados, outros = args[0], args[1:corte], args[corte + 1 :]
    try:
        import apport.report as relatorio  # noqa: PLC0415 (o apport pode faltar)
    except ImportError:
        print("sem o módulo do apport", file=sys.stderr)
        return 3
    # Só a pasta do teste: a lista de permitidos e o nome antigo vazios.
    vazia = "/nonexistent/ward-apport-vazia"
    relatorio._DENYLIST_DIR = pasta
    relatorio._LEGACY_DENYLIST_DIR = vazia
    relatorio._ALLOWLIST_DIR = vazia
    relatorio._LEGACY_ALLOWLIST_DIR = vazia
    relatorio.Report._get_ignore_dom = lambda _self: xml.dom.minidom.parseString(
        "<apport/>"
    )
    falhas = 0
    for caminho, esperado in [(c, True) for c in ignorados] + [(c, False) for c in outros]:
        r = relatorio.Report("Crash")
        r["ExecutablePath"] = caminho
        real = r.check_ignored()
        estado = "ok" if real == esperado else "FALHOU"
        if real != esperado:
            falhas += 1
        print(f"{estado}: {caminho} {'ignorado' if real else 'não ignorado'}")
    return 1 if falhas else 0


if __name__ == "__main__":
    sys.exit(main())
