#!/usr/bin/python3 -I
"""Phase 51c (S2, S8): the apport list installed by Ward, read by apport's own code.

Usage: apport_list.py <list folder> <ignored>... -- <not ignored>...

Uses apport's own `Report.check_ignored()` (the same one `/usr/share/apport/apport` runs
on a crash) with the list folder pointed at <folder> (the DESTDIR one or the container
system one). Each path before `--` must be ignored (no report in /var/crash) and each one
after it must not (control: the list does not silence everything). The user's list
(~/.apport-ignore.xml) is left out: the test does not read the home. Exits 0 if
everything matches; without the apport module, exits 3. The usage text it prints is
`USAGE` (pt-BR, unchanged).
"""

import sys
import xml.dom.minidom

USAGE = """Fase 51c (S2, S8): a lista do apport instalada pelo Ward, lida pelo código do apport.

Uso: apport_list.py <pasta da lista> <ignorado>... -- <não ignorado>...

Usa o `Report.check_ignored()` do próprio apport (o mesmo do `/usr/share/apport/apport`
no crash) com a pasta da lista apontada para <pasta> (a do DESTDIR ou a do sistema do
contêiner). Cada caminho antes de `--` tem de ser ignorado (sem relatório em /var/crash)
e cada um depois, não (controle: a lista não cala tudo). A lista do usuário
(~/.apport-ignore.xml) fica de fora: o teste não lê o home. Sai 0 se tudo confere; sem
o módulo do apport, sai 3.
"""


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 3 or "--" not in args:
        print(USAGE, file=sys.stderr)
        return 2
    cut = args.index("--")
    folder, ignored, others = args[0], args[1:cut], args[cut + 1 :]
    try:
        import apport.report as report  # noqa: PLC0415 (apport may be missing)
    except ImportError:
        print("sem o módulo do apport", file=sys.stderr)
        return 3
    # Only the test folder: the allow list and the legacy name empty.
    empty = "/nonexistent/ward-apport-vazia"
    report._DENYLIST_DIR = folder
    report._LEGACY_DENYLIST_DIR = empty
    report._ALLOWLIST_DIR = empty
    report._LEGACY_ALLOWLIST_DIR = empty
    report.Report._get_ignore_dom = lambda _self: xml.dom.minidom.parseString(
        "<apport/>"
    )
    failures = 0
    for path, expected in [(c, True) for c in ignored] + [(c, False) for c in others]:
        r = report.Report("Crash")
        r["ExecutablePath"] = path
        real = r.check_ignored()
        status = "ok" if real == expected else "FALHOU"
        if real != expected:
            failures += 1
        verdict = "ignorado" if real else "não ignorado"
        print(f"{status}: {path} {verdict}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
