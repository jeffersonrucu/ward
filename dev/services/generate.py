#!/usr/bin/python3 -IB
"""Generates in .dev/services/ the FAKE keys and passwords of the test services (Phase 12).

Usage (from the root, or from any directory):
    python3 -I -B dev/services/generate.py                      # make dev-services
    python3 -I -B dev/services/generate.py --rotate-host-key <ssh service>

- Passwords of the test accounts: read from dev/fixtures/entries.json (D15 values).
- Internal passwords (MariaDB): random (`secrets`), never written to the repository.
- ed25519 key of `deploy` with the D15 passphrase, answered through a pty (never in
  argv); host keys of each SSH server without a passphrase.
- `.dev/services/` is 0700: it is the barrier against the other users of the machine.
  The files read inside the container by a non-root user (postgres, mariadb,
  www-data) are 0644 inside it; the rest is 0600.
- Idempotent: what already exists is not redone (the host keys stay stable), but the
  passwords that come from the fixtures are rewritten to follow the fixtures.

Never reads anything outside the repository.
"""

from __future__ import annotations

import argparse
import json
import os
import pty
import secrets
import shutil
import subprocess
import sys
import time
from pathlib import Path

# First line of the original docstring, shown by --help (output text stays pt-BR, m2:DT20).
DESCRIPTION = "Gera em .dev/services/ as chaves e senhas FALSAS dos serviços de teste (Fase 12)."

# Fixed PATH (Phase 12b): the ssh-keygen called here is the system one, never one
# planted in the user's PATH (~/.local/bin etc.).
os.environ["PATH"] = "/usr/bin:/bin"

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "dev" / "fixtures" / "entries.json"
DEST = ROOT / ".dev" / "services"
SECRETS = DEST / "secrets"
KEYS = DEST / "keys"
HOSTS = DEST / "ssh-host"
SSH_SERVICES = ("ssh-senha", "ssh-chave", "ssh-salto", "ssh-interno")
# How the test client reaches each one (known_hosts of .dev/services/known_hosts).
KNOWN_HOSTS_NAME = {
    "ssh-senha": "[127.0.0.1]:2222",
    "ssh-chave": "[127.0.0.1]:2223",
    "ssh-salto": "[127.0.0.1]:2224",
    "ssh-interno": "ssh-interno",
}
DEPLOY_KEY = KEYS / "acme-deploy_ed25519"


def fail(msg: str) -> None:
    print(f"dev-services: {msg}", file=sys.stderr)
    sys.exit(1)


def write_file(path: Path, content: bytes, mode: int) -> None:
    """Writes atomically, already with the final mode (never goes through an open mode)."""
    temp = path.with_name(f".{path.name}.tmp")
    if temp.exists():
        temp.unlink()
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
    try:
        os.write(fd, content)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.chmod(temp, mode)
    os.replace(temp, path)


def ensure_dir(path: Path) -> None:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.is_symlink():
        fail(f"{path} é link simbólico; recusado")
    os.chmod(path, 0o700)


def load_fixtures() -> dict[str, dict]:
    data = json.loads(FIXTURES.read_text(encoding="utf-8"))
    return {e["name"]: e for e in data["entries"]}


def secret(entries: dict[str, dict], name: str, field: str) -> str:
    try:
        return entries[name]["secrets"][field]
    except KeyError:
        fail(f"fixture sem {name}.secrets.{field}")
        raise  # unreachable


def ssh_keygen_without_passphrase(path: Path, comment: str) -> None:
    subprocess.run(
        ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", comment, "-f", str(path)],
        check=True,
        stdin=subprocess.DEVNULL,
    )


def ssh_keygen_with_passphrase(path: Path, comment: str, passphrase: str) -> None:
    """Answers the ssh-keygen prompts through a pty: the passphrase never goes in argv."""
    pid, fd = pty.fork()
    if pid == 0:  # child
        os.execvp("ssh-keygen", ["ssh-keygen", "-q", "-t", "ed25519", "-C", comment, "-f", str(path)])
    answers = 0
    read_buf = b""
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            try:
                chunk = os.read(fd, 1024)
            except OSError:
                break
            if not chunk:
                break
            read_buf += chunk
            # "Enter passphrase ..." and "Enter same passphrase again:"
            while b"passphrase" in read_buf and read_buf.rstrip().endswith(b":"):
                os.write(fd, passphrase.encode() + b"\n")
                answers += 1
                read_buf = b""
    finally:
        os.close(fd)
        _, status = os.waitpid(pid, 0)
    if os.waitstatus_to_exitcode(status) != 0 or answers != 2:
        fail("ssh-keygen da chave do deploy falhou")


def generate_deploy_key(entries: dict[str, dict]) -> None:
    if DEPLOY_KEY.exists():
        return
    passphrase = secret(entries, "acme-ssh-chave-prod", "passphrase")
    ssh_keygen_with_passphrase(DEPLOY_KEY, "ward-teste-acme-deploy", passphrase)
    os.chmod(DEPLOY_KEY, 0o600)
    os.chmod(DEPLOY_KEY.with_suffix(".pub"), 0o644)


def generate_host_key(service: str, rotate: bool) -> None:
    folder = HOSTS / service
    ensure_dir(folder)
    key = folder / "ssh_host_ed25519_key"
    if key.exists() and not rotate:
        return
    for c in (key, key.with_name(key.name + ".pub")):
        if c.exists():
            c.unlink()
    ssh_keygen_without_passphrase(key, f"ward-teste-host-{service}")
    os.chmod(key, 0o600)


def write_known_hosts() -> None:
    lines = []
    for service in SSH_SERVICES:
        pub = (HOSTS / service / "ssh_host_ed25519_key.pub").read_text().split()
        lines.append(f"{KNOWN_HOSTS_NAME[service]} {pub[0]} {pub[1]}\n")
    write_file(DEST / "known_hosts", "".join(lines).encode(), 0o600)


def write_secrets(entries: dict[str, dict]) -> None:
    ensure_dir(SECRETS)
    # (file, value, mode). 0644 only for what a non-root user reads in the container.
    fixed = [
        ("pg-app", secret(entries, "acme-pg-dev", "password"), 0o644),  # postgres (uid 70)
        ("ssh-deploy", secret(entries, "acme-ssh-senha-stage", "password"), 0o600),
        ("ssh-sftpuser", secret(entries, "beta-sftp-dev", "password"), 0o600),
        ("ssh-salto", secret(entries, "acme-ssh-salto", "password"), 0o600),
        ("chave-passphrase", secret(entries, "acme-ssh-chave-prod", "passphrase"), 0o600),
        ("wp-admin", secret(entries, "acme-wp-dev", "password"), 0o644),  # wp-cli (www-data)
        ("wp-editor", secret(entries, "acme-wp-prod", "password"), 0o644),
    ]
    for name, value, mode in fixed:
        write_file(SECRETS / name, value.encode(), mode)

    # Internal MariaDB passwords: random and stable (WordPress reads them as www-data).
    for name in ("mariadb-root", "mariadb-wp"):
        path = SECRETS / name
        if not path.exists():
            write_file(path, secrets.token_urlsafe(32).encode(), 0o644)
        else:
            os.chmod(path, 0o644)

    # Accounts of the `totp` service (8083): those of the WordPress logins, with the TOTP seed.
    accounts = []
    for name in ("acme-wp-dev", "acme-wp-prod"):
        e = entries[name]
        accounts.append(
            {
                "usuario": e["fields"]["username"],
                "senha": e["secrets"]["password"],
                "totp_semente": e["secrets"]["totp_seed"],
            }
        )
    write_file(SECRETS / "totp-contas.json", json.dumps(accounts).encode(), 0o644)  # nobody


def main() -> None:
    p = argparse.ArgumentParser(description=DESCRIPTION)
    p.add_argument("--rotate-host-key", choices=SSH_SERVICES, help="gera nova chave de host (V4.28)")
    args = p.parse_args()

    for prog in ("ssh-keygen",):
        if shutil.which(prog) is None:
            fail(f"{prog} não encontrado")
    os.umask(0o077)
    ensure_dir(ROOT / ".dev")
    ensure_dir(DEST)
    ensure_dir(KEYS)
    ensure_dir(HOSTS)
    ensure_dir(DEST / "client-home")  # fake HOME of the test clients

    entries = load_fixtures()
    write_secrets(entries)
    generate_deploy_key(entries)
    for service in SSH_SERVICES:
        generate_host_key(service, rotate=(args.rotate_host_key == service))
    write_known_hosts()

    if args.rotate_host_key:
        print(f"dev-services: chave de host de {args.rotate_host_key} trocada; "
              f"reinicie com: docker compose -f dev/compose.yml restart {args.rotate_host_key}")
    print(f"dev-services: pronto em {DEST.relative_to(ROOT)}/")


if __name__ == "__main__":
    main()
