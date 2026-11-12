#!/usr/bin/python3 -IB
"""Phase 65a (m2; handoff of Phase 65, problem 1): guard of the suites that open Chrome.

On a host with Ward installed, `/etc/opt/chrome/policies/managed/ward.json` force-installs
the real extension in EVERY Chrome profile (temporary test profiles included), and
`/etc/opt/chrome/native-messaging-hosts/br.ward.nm.json` sends any extension with the
Ward id to `/usr/local/bin/ward-nm`, that is, to the installed `wardd` and the real
vault. A test Chrome that loads an extension (or gets the forced one) on such a host can
take fake usernames and passwords to the installed daemon, or ask it for real ones. So
every suite that opens Chrome calls this guard first and refuses to run when the Chrome it
is about to open would see any of this.

Usage (from the root):
  python3 -I -B dev/tests/chrome_guard.py check [--root <dir>]
      Exits 0 when Chrome would see nothing of Ward, 3 with the list of findings
      (what was found and where; never the content), 2 on a usage error.
  python3 -I -B dev/tests/chrome_guard.py --self-test
      Plants each form of the policy and of the host manifest in a copy of the tree under
      `.dev/chrome-guard.*` (including a copy of the host's own `ward.json`, when there is
      one and it can be read) and requires each one to be found; a clean tree and an
      unrelated policy must pass. Exits 0 only if every case behaves.

What counts as a finding (fail closed: what cannot be read is a finding too):
- in `etc/opt/chrome/policies/{managed,recommended}/` and
  `etc/chromium/policies/{managed,recommended}/`: a file named `ward.json`; a file whose
  bytes carry the Ward extension id (ExtensionSettings, ExtensionInstallForcelist,
  ExtensionInstallAllowlist... in any form, even invalid JSON); a JSON with
  `NativeMessagingUserLevelHosts` (Ward's policy: it makes Chrome ignore the profile
  manifest and use the system one); a file or folder that cannot be read;
- in `etc/opt/chrome/native-messaging-hosts/` and `etc/chromium/native-messaging-hosts/`:
  `br.ward.nm.json`; any manifest that lets the Ward id in (`allowed_origins`) or points
  to a `ward` program; a manifest that cannot be read.
The Ward id is read from `crates/ward-nm/src/lib.rs` (`EXTENSION_ID`), the single source.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import stat
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NM_SOURCE = ROOT / "crates" / "ward-nm" / "src" / "lib.rs"
HOST_NAME = "br.ward.nm"
POLICY_DIRS = (
    "etc/opt/chrome/policies/managed",
    "etc/opt/chrome/policies/recommended",
    "etc/chromium/policies/managed",
    "etc/chromium/policies/recommended",
)
HOST_DIRS = (
    "etc/opt/chrome/native-messaging-hosts",
    "etc/chromium/native-messaging-hosts",
)
MAX_FILE = 4 * 1024 * 1024
REFUSED = 3


def ward_id() -> str:
    src = NM_SOURCE.read_text(encoding="utf-8")
    m = re.search(r'pub const EXTENSION_ID: &str = "([a-p]{32})";', src)
    if not m:
        print("chrome-guard: EXTENSION_ID não encontrado em crates/ward-nm/src/lib.rs", file=sys.stderr)
        sys.exit(2)
    return m.group(1)


def read_limited(path: Path) -> bytes:
    # Follows a link on purpose: Chrome follows it too.
    with open(path, "rb") as f:
        data = f.read(MAX_FILE + 1)
    if len(data) > MAX_FILE:
        raise OSError("arquivo grande demais")
    return data


def entries(d: Path, findings: list[str], shown: str):
    try:
        st = os.stat(d)
    except FileNotFoundError:
        return []
    except OSError as e:
        findings.append(f"{shown}: não deu para ler a pasta ({e.strerror})")
        return []
    if not stat.S_ISDIR(st.st_mode):
        findings.append(f"{shown}: existe e não é pasta")
        return []
    try:
        return sorted(os.listdir(d))
    except OSError as e:
        findings.append(f"{shown}: não deu para listar ({e.strerror})")
        return []


def check(root: Path, wid: str) -> list[str]:
    findings: list[str] = []
    wid_b = wid.encode()
    for rel in POLICY_DIRS:
        d = root / rel
        for name in entries(d, findings, "/" + rel):
            shown = f"/{rel}/{name}"
            if name == "ward.json":
                findings.append(f"{shown}: política do Ward instalada")
            try:
                data = read_limited(d / name)
            except IsADirectoryError:
                continue
            except OSError as e:
                findings.append(f"{shown}: não deu para ler ({e.strerror or e})")
                continue
            if wid_b in data:
                findings.append(f"{shown}: cita o id da extensão do Ward")
            try:
                doc = json.loads(data)
            except (ValueError, UnicodeDecodeError):
                continue
            if isinstance(doc, dict) and "NativeMessagingUserLevelHosts" in doc:
                findings.append(f"{shown}: NativeMessagingUserLevelHosts (política do Ward)")
    for rel in HOST_DIRS:
        d = root / rel
        for name in entries(d, findings, "/" + rel):
            shown = f"/{rel}/{name}"
            if name == HOST_NAME + ".json":
                findings.append(f"{shown}: manifesto de native messaging do Ward")
            try:
                data = read_limited(d / name)
            except IsADirectoryError:
                continue
            except OSError as e:
                findings.append(f"{shown}: não deu para ler ({e.strerror or e})")
                continue
            if wid_b in data:
                findings.append(f"{shown}: deixa entrar o id da extensão do Ward")
            try:
                doc = json.loads(data)
            except (ValueError, UnicodeDecodeError):
                continue
            if isinstance(doc, dict):
                prog = doc.get("path")
                if isinstance(prog, str) and re.search(r"(^|/)ward[^/]*$", prog):
                    findings.append(f"{shown}: aponta para um programa do Ward")
                if doc.get("name") == HOST_NAME:
                    findings.append(f"{shown}: host {HOST_NAME}")
    # The same finding can come twice (name and content): one line each.
    return list(dict.fromkeys(findings))


def run_check(root: Path) -> int:
    findings = check(root, ward_id())
    if not findings:
        print("chrome-guard: ok, o Chrome não vê política nem host do Ward")
        return 0
    print("chrome-guard: RECUSADO. Este Chrome veria o Ward instalado:", file=sys.stderr)
    for f in findings:
        print(f"  - {f}", file=sys.stderr)
    print("  Uma extensão de teste (ou a forçada pela política) chegaria ao /usr/local/bin/ward-nm,\n"
          "  ao wardd instalado e ao cofre real. Rode a suíte no contêiner (make test-nm,\n"
          "  test-extension e test-login já rodam lá) ou numa máquina sem o Ward instalado.\n"
          "  Nunca apague nem mude a política para rodar o teste.", file=sys.stderr)
    return REFUSED


def self_test() -> int:
    wid = ward_id()
    dev = ROOT / ".dev"
    dev.mkdir(mode=0o700, exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix="chrome-guard.", dir=dev))
    failures = 0

    def case(name: str, expect_refused: bool, plant) -> None:
        nonlocal failures
        r = base / name.replace(" ", "-")
        r.mkdir()
        plant(r)
        got = check(r, wid)
        verdict = ("recusado" if got else "liberado")
        expected = ("recusar" if expect_refused else "liberar")
        if bool(got) == expect_refused:
            print("ok    " + name + ": " + verdict)
        else:
            failures += 1
            print("FALHA " + name + ": esperado " + expected + ", deu " + (str(got) if got else verdict))

    def put(r: Path, rel: str, content: bytes, mode: int = 0o644) -> Path:
        p = r / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(content)
        os.chmod(p, mode)
        return p

    ward_policy = json.dumps({
        "ExtensionSettings": {wid: {
            "installation_mode": "force_installed",
            "update_url": "file:///usr/local/share/ward/extension/update.xml"}},
        "NativeMessagingUserLevelHosts": False,
        "PasswordManagerEnabled": False,
    }).encode()
    host_policy = Path("/etc/opt/chrome/policies/managed/ward.json")
    try:
        real = read_limited(host_policy) if host_policy.is_file() else None
    except OSError:
        real = None
    managed = "etc/opt/chrome/policies/managed"

    case("árvore limpa", False, lambda r: None)
    case("política alheia", False, lambda r: put(r, f"{managed}/empresa.json", b'{"HomepageLocation": "https://example.org"}'))
    case("manifesto alheio", False, lambda r: put(r, "etc/opt/chrome/native-messaging-hosts/org.exemplo.json",
         json.dumps({"name": "org.exemplo", "path": "/usr/bin/exemplo", "type": "stdio",
                     "allowed_origins": ["chrome-extension://abcdefghijklmnopabcdefghijklmnop/"]}).encode()))
    if real is not None:
        case("cópia da política do host (ward.json)", True, lambda r: put(r, f"{managed}/ward.json", real))
        case("cópia da política do host com outro nome", True, lambda r: put(r, f"{managed}/00-empresa.json", real))
    else:
        print("aviso a política do host não existe ou não pode ser lida: só as geradas")
    case("ward.json gerado", True, lambda r: put(r, f"{managed}/ward.json", ward_policy))
    case("ExtensionSettings com outro nome", True, lambda r: put(r, f"{managed}/z.json", ward_policy))
    case("ward.json vazio", True, lambda r: put(r, f"{managed}/ward.json", b"{}"))
    case("ExtensionInstallForcelist", True, lambda r: put(r, f"{managed}/lista.json",
         json.dumps({"ExtensionInstallForcelist": [f"{wid};file:///x/update.xml"]}).encode()))
    case("só NativeMessagingUserLevelHosts", True, lambda r: put(r, f"{managed}/nm.json",
         b'{"NativeMessagingUserLevelHosts": false}'))
    case("JSON inválido com o id", True, lambda r: put(r, f"{managed}/quebrado.json", b"{" + wid.encode()))
    case("pasta recommended", True, lambda r: put(r, "etc/opt/chrome/policies/recommended/ward.json", ward_policy))
    case("Chromium", True, lambda r: put(r, "etc/chromium/policies/managed/x.json", ward_policy))

    def link(r: Path) -> None:
        target = put(r, "fora/politica.json", ward_policy)
        (r / managed).mkdir(parents=True)
        os.symlink(target, r / managed / "link.json")
    case("política por link", True, link)
    if os.geteuid() != 0:
        case("política ilegível", True, lambda r: put(r, f"{managed}/fechada.json", b"{}", 0o000))

        def closed_dir(r: Path) -> None:
            put(r, f"{managed}/a.json", b"{}")
            os.chmod(r / managed, 0o000)
        case("pasta de política ilegível", True, closed_dir)
    case("manifesto br.ward.nm.json", True, lambda r: put(r, "etc/opt/chrome/native-messaging-hosts/br.ward.nm.json",
         json.dumps({"name": HOST_NAME, "path": "/usr/local/bin/ward-nm", "type": "stdio",
                     "allowed_origins": [f"chrome-extension://{wid}/"]}).encode()))
    case("manifesto com outro nome e o id", True, lambda r: put(r, "etc/opt/chrome/native-messaging-hosts/x.json",
         json.dumps({"name": "x", "path": "/usr/bin/x", "type": "stdio",
                     "allowed_origins": [f"chrome-extension://{wid}/"]}).encode()))
    case("manifesto que aponta para o ward-nm", True, lambda r: put(r, "etc/chromium/native-messaging-hosts/y.json",
         json.dumps({"name": "y", "path": "/usr/local/bin/ward-nm", "type": "stdio", "allowed_origins": []}).encode()))

    # Restores the permissions before removing.
    # (top-down: each folder is opened before the walk goes into it)
    for dirpath, dirnames, _ in os.walk(base):
        for d in dirnames:
            p = os.path.join(dirpath, d)
            if not os.path.islink(p):
                os.chmod(p, 0o700)
    shutil.rmtree(base)
    if failures:
        print(f"chrome-guard --self-test: {failures} caso(s) com falha", file=sys.stderr)
        return 1
    print("chrome-guard --self-test: tudo certo")
    return 0


def main(argv: list[str]) -> int:
    if argv == ["--self-test"]:
        return self_test()
    if argv and argv[0] == "check":
        root = Path("/")
        rest = argv[1:]
        if rest[:1] == ["--root"] and len(rest) == 2:
            root = Path(rest[1])
        elif rest:
            print("uso: chrome_guard.py check [--root <pasta>] | --self-test", file=sys.stderr)
            return 2
        return run_check(root)
    print("uso: chrome_guard.py check [--root <pasta>] | --self-test", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
