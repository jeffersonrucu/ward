"""Phase 63 (V2.7): reads every readable region of /proc/<pid>/mem (root with
CAP_SYS_PTRACE, only inside the disposable container) and counts how many patterns of
the variations list appear. Prints only counts, never a pattern or a byte of memory.

Usage: python3 -I manual_dump.py <pid> <variations.txt>
"""

import sys

CHUNK = 1 << 20


def patterns(path):
    with open(path, "rb") as f:
        out = [line.rstrip(b"\n") for line in f]
    return [p for p in out if p]


def regions(pid):
    with open(f"/proc/{pid}/maps") as f:
        for line in f:
            parts = line.split()
            perms = parts[1]
            if not perms.startswith("r"):
                continue
            path = parts[5] if len(parts) > 5 else ""
            if path in ("[vvar]", "[vsyscall]", "[vvar_vclock]"):
                continue
            start, end = (int(x, 16) for x in parts[0].split("-"))
            yield start, end


def main():
    pid = int(sys.argv[1])
    pats = patterns(sys.argv[2])
    longest = max(len(p) for p in pats)
    hits = set()
    nregions = 0
    nbytes = 0
    with open(f"/proc/{pid}/mem", "rb", buffering=0) as mem:
        for start, end in regions(pid):
            nregions += 1
            tail = b""
            pos = start
            while pos < end:
                size = min(CHUNK, end - pos)
                try:
                    mem.seek(pos)
                    data = mem.read(size)
                except OSError:
                    data = b""
                pos += size
                if not data:
                    tail = b""
                    continue
                nbytes += len(data)
                buf = tail + data
                if buf.count(0) != len(buf):
                    for i, p in enumerate(pats):
                        if i not in hits and p in buf:
                            hits.add(i)
                tail = buf[-(longest - 1):] if longest > 1 else b""
    print(f"regiões={nregions} bytes={nbytes} padrões_achados={len(hits)}")


if __name__ == "__main__":
    main()
