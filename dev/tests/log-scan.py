#!/usr/bin/python3 -IB
"""V5.27 (S13, Fase 39): nenhum padrão de `.dev/variations.txt` nos logs do app, da ponte,
do daemon de teste e no console da webview gravado pelo QA.

Uso (da raiz): python3 -I -B dev/tests/log-scan.py [arquivo ou pasta ...]
(`make test-app-logs`). Sem argumentos, varre `.dev/logs/` inteira.

- Só arquivos dentro de `.dev/` do repositório (resolvidos, sem seguir link para fora) e
  só arquivos comuns: um link ou um caminho fora de `.dev/` é recusado (sai 2).
- A lista de padrões nunca é impressa, nem o trecho achado: a saída diz só o arquivo e
  quantos padrões casaram. Padrão com menos de 6 bytes é recusado (a lista do
  `dev/variations.py` não tem nenhum; um curto daria falso positivo).
- Sai 0 limpo, 1 com achado, 2 com erro de uso (sem lista, nada para varrer, caminho
  recusado).
- `--self-test` confere a varredura com um log falso plantado (o ataque tem de ser
  achado) e um limpo, em `.dev/log-scan-self-test/`.
"""

from __future__ import annotations

import os
import stat
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
DEV = RAIZ / ".dev"
LISTA = DEV / "variations.txt"
MIN_PADRAO = 6
MAX_ARQUIVO = 256 * 1024 * 1024


def falhar(msg: str, codigo: int = 2) -> None:
    print(f"varredura-logs: {msg}", file=sys.stderr)
    sys.exit(codigo)


def padroes(lista: Path) -> list[bytes]:
    try:
        st = os.lstat(lista)
    except FileNotFoundError:
        falhar("falta .dev/variations.txt: rode 'make dev-variations'")
    if not stat.S_ISREG(st.st_mode):
        falhar("a lista de padrões não é arquivo comum")
    if st.st_mode & 0o077:
        falhar("a lista de padrões está aberta a grupo ou outros")
    v = [p for p in lista.read_bytes().split(b"\n") if p]
    if not v:
        falhar("a lista de padrões está vazia")
    if any(len(p) < MIN_PADRAO for p in v):
        falhar("a lista tem padrão curto demais")
    return v


def dentro_de_dev(p: Path) -> bool:
    try:
        p.resolve(strict=True).relative_to(DEV.resolve(strict=True))
        return True
    except (ValueError, FileNotFoundError):
        return False


def arquivos(alvos: list[Path]) -> list[Path]:
    achados: list[Path] = []
    for a in alvos:
        if not a.is_absolute():
            a = RAIZ / a
        if os.path.islink(a) or not dentro_de_dev(a):
            falhar(f"recusado (fora de .dev/ ou link): {a.name}")
        if a.is_dir():
            for raiz, pastas, nomes in os.walk(a, followlinks=False):
                pastas[:] = [d for d in pastas if not os.path.islink(os.path.join(raiz, d))]
                for n in sorted(nomes):
                    f = Path(raiz) / n
                    if stat.S_ISREG(os.lstat(f).st_mode):
                        achados.append(f)
        elif stat.S_ISREG(os.lstat(a).st_mode):
            achados.append(a)
    return achados


def varrer(lista: list[bytes], alvos: list[Path]) -> int:
    fs = arquivos(alvos)
    if not fs:
        falhar("nada para varrer")
    sujos = 0
    for f in fs:
        if os.path.getsize(f) > MAX_ARQUIVO:
            falhar(f"arquivo grande demais: {f.relative_to(RAIZ)}")
        corpo = f.read_bytes()
        n = sum(1 for p in lista if p in corpo)
        rel = f.relative_to(RAIZ)
        if n:
            sujos += 1
            print(f"SEGREDO  {rel}: {n} padrão(ões) de .dev/variations.txt")
        else:
            print(f"limpo    {rel} ({len(corpo)} bytes)")
    print(f"{len(fs)} arquivo(s), {sujos} com segredo")
    return 1 if sujos else 0


def autoteste(lista: list[bytes]) -> int:
    pasta = DEV / "log-scan-self-test"
    pasta.mkdir(mode=0o700, exist_ok=True)
    limpo = pasta / "limpo.log"
    sujo = pasta / "sujo.log"
    limpo.write_bytes(b"ward-app: sem bandeja (teste)\n")
    # O ataque: um log que vazou um valor da lista no meio de outra linha.
    sujo.write_bytes(b"[console] valor=" + lista[len(lista) // 2] + b" fim\n")
    erros = 0
    if varrer(lista, [limpo]) != 0:
        print("FALHOU: log limpo acusado")
        erros += 1
    if varrer(lista, [sujo]) != 1:
        print("FALHOU: log com segredo não acusado")
        erros += 1
    sujo.unlink()
    limpo.unlink()
    pasta.rmdir()
    print("autoteste ok" if not erros else "autoteste FALHOU")
    return 1 if erros else 0


def main() -> int:
    args = sys.argv[1:]
    lista = padroes(LISTA)
    if args == ["--self-test"]:
        return autoteste(lista)
    alvos = [Path(a) for a in args] or [DEV / "logs"]
    return varrer(lista, alvos)


if __name__ == "__main__":
    sys.exit(main())
