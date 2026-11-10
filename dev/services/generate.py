#!/usr/bin/python3 -IB
"""Gera em .dev/services/ as chaves e senhas FALSAS dos serviços de teste (Fase 12).

Uso (da raiz, ou de qualquer diretório):
    python3 -I -B dev/services/generate.py                      # make dev-services
    python3 -I -B dev/services/generate.py --rotate-host-key ssh-senha

- Senhas das contas de teste: lidas de dev/fixtures/entries.json (valores de D15).
- Senhas internas (MariaDB): aleatórias (`secrets`), nunca escritas no repositório.
- Chave ed25519 do `deploy` com a passphrase de D15, respondida por um pty (nunca em
  argv); chaves de host de cada SSH sem passphrase.
- `.dev/services/` é 0700: é a barreira para os outros usuários da máquina. Os
  arquivos lidos dentro do contêiner por usuário não root (postgres, mariadb,
  www-data) ficam 0644 dentro dele; o resto fica 0600.
- Idempotente: o que já existe não é refeito (as chaves de host ficam estáveis), mas
  as senhas que vêm das fixtures são reescritas para acompanhar as fixtures.

Nunca lê nada fora do repositório.
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

# PATH fixo (Fase 12b): o ssh-keygen chamado aqui é o do sistema, nunca um plantado
# no PATH do usuário (~/.local/bin etc.).
os.environ["PATH"] = "/usr/bin:/bin"

RAIZ = Path(__file__).resolve().parents[2]
FIXTURES = RAIZ / "dev" / "fixtures" / "entries.json"
DESTINO = RAIZ / ".dev" / "services"
SEGREDOS = DESTINO / "secrets"
CHAVES = DESTINO / "keys"
HOSTS = DESTINO / "ssh-host"
SERVICOS_SSH = ("ssh-senha", "ssh-chave", "ssh-salto", "ssh-interno")
# Como o cliente de teste chega em cada um (known_hosts de .dev/services/known_hosts).
NOME_NO_KNOWN_HOSTS = {
    "ssh-senha": "[127.0.0.1]:2222",
    "ssh-chave": "[127.0.0.1]:2223",
    "ssh-salto": "[127.0.0.1]:2224",
    "ssh-interno": "ssh-interno",
}
CHAVE_DEPLOY = CHAVES / "acme-deploy_ed25519"


def falhar(msg: str) -> None:
    print(f"dev-services: {msg}", file=sys.stderr)
    sys.exit(1)


def gravar(caminho: Path, conteudo: bytes, modo: int) -> None:
    """Grava de forma atômica, já com o modo final (nunca passa por modo aberto)."""
    temp = caminho.with_name(f".{caminho.name}.tmp")
    if temp.exists():
        temp.unlink()
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, modo)
    try:
        os.write(fd, conteudo)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.chmod(temp, modo)
    os.replace(temp, caminho)


def diretorio(caminho: Path) -> None:
    caminho.mkdir(mode=0o700, parents=True, exist_ok=True)
    if caminho.is_symlink():
        falhar(f"{caminho} é link simbólico; recusado")
    os.chmod(caminho, 0o700)


def carregar_fixtures() -> dict[str, dict]:
    dados = json.loads(FIXTURES.read_text(encoding="utf-8"))
    return {e["nome"]: e for e in dados["entradas"]}


def segredo(entradas: dict[str, dict], nome: str, campo: str) -> str:
    try:
        return entradas[nome]["segredos"][campo]
    except KeyError:
        falhar(f"fixture sem {nome}.segredos.{campo}")
        raise  # inalcançável


def ssh_keygen_sem_passphrase(caminho: Path, comentario: str) -> None:
    subprocess.run(
        ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-C", comentario, "-f", str(caminho)],
        check=True,
        stdin=subprocess.DEVNULL,
    )


def ssh_keygen_com_passphrase(caminho: Path, comentario: str, passphrase: str) -> None:
    """Responde as perguntas do ssh-keygen por um pty: a passphrase não vai em argv."""
    pid, fd = pty.fork()
    if pid == 0:  # filho
        os.execvp("ssh-keygen", ["ssh-keygen", "-q", "-t", "ed25519", "-C", comentario, "-f", str(caminho)])
    respostas = 0
    lido = b""
    limite = time.monotonic() + 30
    try:
        while time.monotonic() < limite:
            try:
                pedaco = os.read(fd, 1024)
            except OSError:
                break
            if not pedaco:
                break
            lido += pedaco
            # "Enter passphrase ..." e "Enter same passphrase again:"
            while b"passphrase" in lido and lido.rstrip().endswith(b":"):
                os.write(fd, passphrase.encode() + b"\n")
                respostas += 1
                lido = b""
    finally:
        os.close(fd)
        _, status = os.waitpid(pid, 0)
    if os.waitstatus_to_exitcode(status) != 0 or respostas != 2:
        falhar("ssh-keygen da chave do deploy falhou")


def gerar_chave_deploy(entradas: dict[str, dict]) -> None:
    if CHAVE_DEPLOY.exists():
        return
    passphrase = segredo(entradas, "acme-ssh-chave-prod", "passphrase")
    ssh_keygen_com_passphrase(CHAVE_DEPLOY, "ward-teste-acme-deploy", passphrase)
    os.chmod(CHAVE_DEPLOY, 0o600)
    os.chmod(CHAVE_DEPLOY.with_suffix(".pub"), 0o644)


def gerar_chave_host(servico: str, trocar: bool) -> None:
    pasta = HOSTS / servico
    diretorio(pasta)
    chave = pasta / "ssh_host_ed25519_key"
    if chave.exists() and not trocar:
        return
    for c in (chave, chave.with_name(chave.name + ".pub")):
        if c.exists():
            c.unlink()
    ssh_keygen_sem_passphrase(chave, f"ward-teste-host-{servico}")
    os.chmod(chave, 0o600)


def gravar_known_hosts() -> None:
    linhas = []
    for servico in SERVICOS_SSH:
        pub = (HOSTS / servico / "ssh_host_ed25519_key.pub").read_text().split()
        linhas.append(f"{NOME_NO_KNOWN_HOSTS[servico]} {pub[0]} {pub[1]}\n")
    gravar(DESTINO / "known_hosts", "".join(linhas).encode(), 0o600)


def gravar_segredos(entradas: dict[str, dict]) -> None:
    diretorio(SEGREDOS)
    # (arquivo, valor, modo). 0644 só para quem é lido por usuário não root no contêiner.
    fixos = [
        ("pg-app", segredo(entradas, "acme-pg-dev", "senha"), 0o644),  # postgres (uid 70)
        ("ssh-deploy", segredo(entradas, "acme-ssh-senha-stage", "senha"), 0o600),
        ("ssh-sftpuser", segredo(entradas, "beta-sftp-dev", "senha"), 0o600),
        ("ssh-salto", segredo(entradas, "acme-ssh-salto", "senha"), 0o600),
        ("chave-passphrase", segredo(entradas, "acme-ssh-chave-prod", "passphrase"), 0o600),
        ("wp-admin", segredo(entradas, "acme-wp-dev", "senha"), 0o644),  # wp-cli (www-data)
        ("wp-editor", segredo(entradas, "acme-wp-prod", "senha"), 0o644),
    ]
    for nome, valor, modo in fixos:
        gravar(SEGREDOS / nome, valor.encode(), modo)

    # Senhas internas da MariaDB: aleatórias e estáveis (o WordPress as lê como www-data).
    for nome in ("mariadb-root", "mariadb-wp"):
        caminho = SEGREDOS / nome
        if not caminho.exists():
            gravar(caminho, secrets.token_urlsafe(32).encode(), 0o644)
        else:
            os.chmod(caminho, 0o644)

    # Contas do serviço `totp` (8083): as dos logins do WordPress, com a semente TOTP.
    contas = []
    for nome in ("acme-wp-dev", "acme-wp-prod"):
        e = entradas[nome]
        contas.append(
            {
                "usuario": e["campos"]["usuario"],
                "senha": e["segredos"]["senha"],
                "totp_semente": e["segredos"]["totp_semente"],
            }
        )
    gravar(SEGREDOS / "totp-contas.json", json.dumps(contas).encode(), 0o644)  # nobody


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--rotate-host-key", choices=SERVICOS_SSH, help="gera nova chave de host (V4.28)")
    args = p.parse_args()

    for prog in ("ssh-keygen",):
        if shutil.which(prog) is None:
            falhar(f"{prog} não encontrado")
    os.umask(0o077)
    diretorio(RAIZ / ".dev")
    diretorio(DESTINO)
    diretorio(CHAVES)
    diretorio(HOSTS)
    diretorio(DESTINO / "client-home")  # HOME falso dos clientes de teste

    entradas = carregar_fixtures()
    gravar_segredos(entradas)
    gerar_chave_deploy(entradas)
    for servico in SERVICOS_SSH:
        gerar_chave_host(servico, trocar=(args.rotate_host_key == servico))
    gravar_known_hosts()

    if args.rotate_host_key:
        print(f"dev-services: chave de host de {args.rotate_host_key} trocada; "
              f"reinicie com: docker compose -f dev/compose.yml restart {args.rotate_host_key}")
    print(f"dev-services: pronto em {DESTINO.relative_to(RAIZ)}/")


if __name__ == "__main__":
    main()
