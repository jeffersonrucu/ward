#!/usr/bin/python3 -IB
"""Tests of `dev/variations.py` (V2.52, V2.57, V3.29, V3.31 and V3.32, plus the D11
variations of V2.29; Phases 19d, 19e and 22a).

Usage (from the root): python3 -I -B dev/tests/variations_test.py  (`make test-dev-vault`
runs it).

Standard library only. The values are fake and built here, not read from the fixtures.
The expected forms are written by hand (or with `base64`/`urllib` from the standard
library), not computed by `variations.py`. Failure messages name the case, never the
value or the whole list.
"""

from __future__ import annotations

import base64
import importlib.util
import re
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "dev" / "variations.py"


def load():
    spec = importlib.util.spec_from_file_location("variacoes", SOURCE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


V = load()


def pad(value: str | bytes, username: str | None = None) -> set[bytes]:
    v = value.encode("utf-8") if isinstance(value, str) else value
    return V.variations(v, username)


def esc(form: str) -> bytes:
    """Expected form with `~` in place of the backslash (the test text has no escape
    sequences that an editor or tool could decode)."""
    return form.replace("~", "\\").encode("ascii")


GO_PASSWORD = "falso-Ab3$k&<9>Zq!"
ACCENTED = "senha-a" + chr(0xE7) + chr(0xE3) + "o-01"
EMOJI = "falso-" + chr(0x1F600) + "-01"
LS = "falso" + chr(0x2028) + "linha01"
PS = "falso" + chr(0x2029) + "linha01"


class D37EscapedForms(unittest.TestCase):
    """V2.52: Go JSON, Python's `ensure_ascii` and shell double quotes."""

    def has_form(self, value, form: bytes, label: str) -> None:
        self.assertTrue(form in pad(value), f"falta a forma: {label}")

    def test_esc_builds_the_backslash(self):
        self.assertEqual(esc("a~u0026"), bytes([0x61, 0x5C, 0x75, 0x30, 0x30, 0x32, 0x36]))

    def test_json_do_go(self):
        self.has_form(GO_PASSWORD, esc("falso-Ab3$k~u0026~u003c9~u003eZq!"), "JSON do Go")

    def test_shell_double_quotes(self):
        self.has_form(GO_PASSWORD, esc("falso-Ab3~$k&<9>Zq!"), "aspas duplas do shell")
        self.has_form('falso-`a"b~c$d'.replace("~", "\\"), esc('falso-~`a~"b~~c~$d'), "shell com ` \" \\ e $")

    def test_ensure_ascii_lowercase(self):
        self.has_form(ACCENTED, esc("senha-a~u00e7~u00e3o-01"), "ensure_ascii")

    def test_ensure_ascii_with_surrogate_pair(self):
        self.has_form(EMOJI, esc("falso-~ud83d~ude00-01"), "par substituto")

    def test_u2028_e_u2029(self):
        self.has_form(LS, esc("falso~u2028linha01"), "U+2028")
        self.has_form(PS, esc("falso~u2029linha01"), "U+2029")

    def test_uppercase_hex_left_out(self):
        # RA3: upper case hex (~u00E7) is not generated, as in the mask.
        p = pad(ACCENTED)
        self.assertFalse(esc("senha-a~u00E7~u00E3o-01") in p, "hex maiúsculo")
        self.assertFalse(any(re.search(rb"\x5cu[0-9a-f]*[A-F]", x) for x in p), "hex maiúsculo")

    def test_only_when_different_from_literal(self):
        for v in ("falso-simples-01", "falso_token_github_01", "falso-WP-senha-02"):
            self.assertEqual(V.escaped_forms(v.encode()), set(), f"forma nova sem caractere: {v[:6]}")
        # No pattern with a backslash for a value with nothing to escape.
        p = pad("falso-simples-01")
        self.assertFalse(any(b"\x5c" in x for x in p), "padrão escapado sem motivo")
        # And the literal does not change: the set is the same as before D37 (the D11
        # encodings).
        self.assertTrue(b"falso-simples-01" in p, "literal")

    def test_forms_also_per_line(self):
        p = pad("falso-linha-um-01\n" + GO_PASSWORD + "\n")
        self.assertTrue(esc("falso-Ab3$k~u0026~u003c9~u003eZq!") in p, "JSON do Go da linha")
        self.assertTrue(esc("falso-Ab3~$k&<9>Zq!") in p, "shell da linha")
        value = 'export FALSO_SENHA="' + ACCENTED + '"\n'
        self.assertTrue(esc("senha-a~u00e7~u00e3o-01") in pad(value), "ensure_ascii do valor da linha")

    def test_whole_multi_line_in_json(self):
        # The Go JSON of the whole secret (with a newline) is a single line and goes in.
        p = pad("falso-<um>-linha\nfalso-dois-linha\n")
        self.assertTrue(esc("falso-~u003cum~u003e-linha~nfalso-dois-linha~n") in p, "Go do inteiro")


class D36PerLinePass(unittest.TestCase):
    """V2.52: trimmed lines and values of `KEY=value`, JSON and YAML."""

    KEY = (
        "-----BEGIN CHAVE FALSA DE TESTE-----\r\n"
        "   b3BlbnNzaC1rZXktdjEAAAAAfalsofalso01   \r\n"
        "AAAAC3NzaC1lZDI1NTE5AAAAIfalsofalso02\r\n"
        "curta\r\n"
        "-----END CHAVE FALSA DE TESTE-----\r\n"
    )
    JSON = (
        "{\n"
        '  "type": "service_account",\n'
        '  "private_key": "falso-chave-de-conta-de-servico-0123456789",\n'
        '  "client_email": "falso@x.example"\n'
        "}\n"
    )
    YAML = "users:\n- name: falso\n  user:\n    token: falso-token-kube-01\n    senha: 'falso-yaml-02'\n"

    def test_trimmed_lines(self):
        p = pad(self.KEY)
        for line in (
            b"-----BEGIN CHAVE FALSA DE TESTE-----",
            b"b3BlbnNzaC1rZXktdjEAAAAAfalsofalso01",
            b"AAAAC3NzaC1lZDI1NTE5AAAAIfalsofalso02",
            b"-----END CHAVE FALSA DE TESTE-----",
        ):
            self.assertTrue(line in p, "linha aparada da chave")
        self.assertFalse(b"curta" in p, "linha curta virou padrão")
        self.assertFalse(any(b"\r" in x or b"\n" in x for x in p), "padrão com quebra")

    def test_multi_line_json(self):
        p = pad(self.JSON)
        self.assertTrue(b"falso-chave-de-conta-de-servico-0123456789" in p, "valor JSON sem aspas")
        self.assertTrue(b'"falso-chave-de-conta-de-servico-0123456789"' in p, "valor JSON com aspas")
        self.assertTrue(b"falso@x.example" in p, "outro valor JSON")
        self.assertTrue(b'"service_account"' in p, "valor JSON com aspas (2)")

    def test_yaml(self):
        p = pad(self.YAML)
        self.assertTrue(b"falso-token-kube-01" in p, "valor YAML")
        self.assertTrue(b"falso-yaml-02" in p, "valor YAML sem aspas")
        self.assertTrue(b"'falso-yaml-02'" in p, "valor YAML com aspas")
        self.assertFalse(b"falso" in p, "valor com menos de 6 caracteres virou padrão")

    def test_key_equals(self):
        p = pad("FALSO_A=falso-anexo-01\nexport FALSO_B='falso-anexo-02'\nPORT=8080\n")
        for x in (b"falso-anexo-01", b"falso-anexo-02", b"'falso-anexo-02'", b"FALSO_A=falso-anexo-01"):
            self.assertTrue(x in p, "valor de CHAVE=valor")
        self.assertFalse(b"8080" in p, "valor curto de CHAVE=valor (D34)")

    def test_url_and_host_port_are_not_yaml(self):
        values = V.line_values(b"https://x.example:8080/falso-caminho")
        self.assertEqual(values, [], "URL lida como chave: valor")
        self.assertEqual(V.line_values(b"host:falso-porta-01"), [], "host:porta lido como YAML")

    def test_minimum_in_characters(self):
        # 5 characters and 7 bytes: below the minimum of D34/D36, as in the mask.
        self.assertEqual(V.line_values("X: ação1".encode()), [], "mínimo contado em bytes")
        self.assertEqual(V.attachment_lines("çãçãç\n".encode()), [], "linha curta em caracteres")

    def test_single_line_secret_skips_per_line(self):
        # `"a": "b"` in a single-line secret is not split (D36 is for multiple lines).
        p = pad('"falso": "falso-valor-01"')
        self.assertFalse(b"falso-valor-01" in p, "segredo de uma linha partido")


class D11FromBefore(unittest.TestCase):
    """V2.29: the D11 variations remain (expected ones computed with the standard
    library)."""

    def test_url_base64_basic(self):
        s = "falso-PG:s3nha/01@#"
        p = pad(s, "app")
        for x in (
            b"falso-PG%3As3nha%2F01%40%23",
            b"falso-PG%3as3nha%2f01%40%23",
            base64.b64encode(s.encode()),
            base64.b64encode(s.encode()).rstrip(b"="),
            base64.urlsafe_b64encode(s.encode()),
            base64.b64encode(b"app:" + s.encode()),
            s.encode(),
        ):
            self.assertTrue(x in p, "variação de D11")

    def test_database_url(self):
        e = {
            "fields": {"engine": "postgres", "username": "app", "host": "127.0.0.1", "port": "55432", "db_name": "app_teste"},
            "secrets": {"password": "falso-PG:s3nha/01@#"},
        }
        urls = V.database_url(e)
        self.assertTrue(b"postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste" in urls, "DATABASE_URL")

    def test_base64_inside_larger(self):
        p = pad("falso_token_github_01")
        larger = base64.b64encode(b"x-access-token:falso_token_github_01")
        self.assertTrue(any(x in larger for x in p if len(x) >= 8), "miolo do base64")

    def test_json_escape_e_php(self):
        p = pad('falso/"aspas"-01')
        self.assertTrue(rb'falso/\"aspas\"-01' in p, "escape JSON")
        self.assertTrue(rb'falso\/\"aspas\"-01' in p, "escape do PHP")

    def test_list_without_short_pattern_or_newline(self):
        with self.assertRaises(V.Refusal):
            V.verify_patterns({b"falso"})
        with self.assertRaises(V.Refusal):
            V.verify_patterns({b"falso-\nlinha-01"})
        every = set()
        for s in (GO_PASSWORD, ACCENTED, EMOJI, LS, PS,
                  D36PerLinePass.KEY, D36PerLinePass.JSON, D36PerLinePass.YAML):
            every |= pad(s, "app")
        ordered = V.verify_patterns(every)
        self.assertEqual(ordered, sorted(ordered), "lista fora de ordem (não determinística)")

    def test_output_outside_dev_refused(self):
        for path in ("/tmp/variations.txt", "../variations.txt", "dev/variations.txt"):
            with self.assertRaises(V.Refusal):
                V.validate_output(path)


def esc8(form: str) -> bytes:
    """Like `esc`, but the form may have a non-ASCII character (kept as UTF-8)."""
    return form.replace("~", "\\").encode("utf-8")


C = chr(0xE7)  # c with cedilla
# A line with `"`, `'`, `/`, c with cedilla, `$`, `&`, `<`, `>` and a space; and an
# ASCII-only one for %q.
LINE = 'falso-"q\'/' + C + "$&<x> 01"
LINE_ASCII = "falso-\"q' $&<x>#01"


class D54SingleTable(unittest.TestCase):
    """V2.57: the line of a multi-line secret gets the text forms and the tab fields, and
    gets no base64 or cores; new forms (PHP, single quotes, `printf %q`)."""

    def multi_line(self) -> set[bytes]:
        return pad("falso-linha-um-01\n" + LINE + "\n" + LINE_ASCII + "\n")

    def test_line_gets_the_text_forms(self):
        p = self.multi_line()
        expected_forms = {
            "JSON padrão": 'falso-~"q\'/' + C + "$&<x> 01",
            "JSON com ~/": 'falso-~"q\'~/' + C + "$&<x> 01",
            "Go": 'falso-~"q\'/' + C + "$~u0026~u003cx~u003e 01",
            "ensure_ascii": 'falso-~"q\'/~u00e7$&<x> 01',
            "PHP": 'falso-~"q\'~/~u00e7$&<x> 01',
            "aspas duplas": 'falso-~"q\'/' + C + "~$&<x> 01",
            "aspas simples": "falso-\"q'~''/" + C + "$&<x> 01",
            "printf %q": "falso-~\"q~'~ ~$~&~<x~>#01",
        }
        for label, form in expected_forms.items():
            self.assertTrue(esc8(form) in p, f"falta a forma da linha: {label}")
        self.assertTrue(LINE.encode() in p and LINE_ASCII.encode() in p, "a linha literal")

    def test_line_gets_no_base64_or_cores(self):
        p = self.multi_line()
        for line in (LINE, LINE_ASCII):
            b = line.encode()
            forbidden_forms = set()
            for k in (0, 1, 2):
                for encode in (base64.b64encode, base64.urlsafe_b64encode):
                    forbidden_forms.add(encode(b"\0" * k + b))
            self.assertFalse(any(len(x) >= 8 and any(x in q for q in forbidden_forms) for x in p),
                             "base64 ou miolo de linha solta (RA3)")

    def test_printf_q_so_em_ascii(self):
        self.assertIsNone(V.printf_q(LINE), "printf %q fora do ASCII (vai para RA3)")
        self.assertIsNone(V.printf_q("falso-ctl" + chr(0x1B) + "-01"), "printf %q com controle")
        self.assertEqual(V.escaped_forms(b"falso-simples-01"), set(), "forma sem caractere")

    def test_php_only_with_slash_and_non_ascii(self):
        self.assertIsNone(V.json_php("falso/simples/01"), "PHP sem caractere fora do ASCII")
        self.assertIsNone(V.json_php("falso-a" + C + "o-01"), "PHP sem barra")
        self.assertEqual(V.json_php("falso/a" + C + "/01"), "falso~/a~u00e7~/01".replace("~", "\\"))

    def test_single_quotes_only_with_quote(self):
        self.assertIsNone(V.single_quoted("falso-simples-01"), "aspas simples sem aspa")
        self.assertEqual(V.single_quoted("falso-it's-01x"), "falso-it'~''s-01x".replace("~", "\\"))

    def test_tab_fields(self):
        p = pad("nome\tfalso-campo-tab-01\tcurto\n'falso-tsv-aspas-02'\tx\n")
        self.assertTrue(b"falso-campo-tab-01" in p, "campo de tab")
        self.assertTrue(b"falso-tsv-aspas-02" in p, "campo de tab sem aspas")
        # The field is a line value: a whole secret, with base64.
        self.assertTrue(base64.b64encode(b"falso-campo-tab-01") in p, "base64 do campo de tab")
        self.assertFalse(b"curto" in p, "campo curto virou padrão")

    def test_new_line_parser(self):
        value = (
            "senha2 : falso-yaml-esp-02\n"
            "senha:\tfalso-yaml-tab-01\n"
            '"k":"falso-json-colado-03"\n'
            '  "private key": "falso-valor-com-espaco-na-chave-01",\n'
            '  "x/y": "falso-valor-barra-chave-02"\n'
        )
        p = pad(value)
        for x in (
            b"falso-yaml-esp-02",
            b"falso-yaml-tab-01",
            b"falso-json-colado-03",
            b"falso-valor-com-espaco-na-chave-01",
            b"falso-valor-barra-chave-02",
        ):
            self.assertTrue(x in p, "valor de linha (D54)")
            self.assertTrue(base64.b64encode(x) in p, "valor de linha como segredo inteiro")

    def test_whole_attachment_without_core_or_json(self):
        attachment = b"FALSO_ENV=falso-anexo-01\nPORT=8080\n"
        p = V.variations(attachment, None, attachment=True)
        self.assertTrue(base64.b64encode(attachment) in p, "base64 do anexo inteiro")
        self.assertFalse(any(rb"falso-anexo-01\nPORT" in x for x in p), "JSON do anexo inteiro")
        self.assertTrue(b"falso-anexo-01" in p and b"FALSO_ENV=falso-anexo-01" in p, "linha e valor")

    def test_printf_q_and_single_quotes_match_bash(self):
        bash = "/bin/bash"
        if not Path(bash).exists():
            self.skipTest("sem bash")
        values = []
        for i in range(0x20, 0x7F):
            c = chr(i)
            values += ["a" + c + "b", c + "b", "a" + c, "a=" + c + "b", "a:" + c + "b"]
        r = subprocess.run(
            [bash, "--norc", "--noprofile", "-c",
             'while IFS= read -r v; do printf "%q\\n" "$v"; done <<<"$VALORES"'],
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C.UTF-8", "VALORES": "\n".join(values)},
            capture_output=True, check=True,
        )
        from_bash = r.stdout.decode().split("\n")[:-1]
        self.assertEqual(len(from_bash), len(values))
        for v, expected in zip(values, from_bash):
            self.assertEqual(V.printf_q(v) or v, expected, f"printf %q do caractere {ord(v[-2] if len(v) > 1 else v[0])}")
        r = subprocess.run(
            [bash, "--norc", "--noprofile", "-c", 'echo "${V@Q}"'],
            env={"PATH": "/usr/bin:/bin", "V": "falso-it's-01x"}, capture_output=True, check=True,
        )
        self.assertEqual(r.stdout.decode().strip(), "'" + V.single_quoted("falso-it's-01x") + "'")


class D58D61(unittest.TestCase):
    """V3.29 (Go with `ensure_ascii`, D58), V3.31 (JSON on the line) and V3.32 (YAML,
    dotenv and tab fields of a line that is not text), D61."""

    def test_go_with_ensure_ascii_whole(self):
        v = "falso-Ab3$k&<9>Zq!'/" + C + "-01"
        self.assertTrue(esc8("falso-Ab3$k~u0026~u003c9~u003eZq!'/~u00e7-01") in pad(v), "D58 inteiro")

    def test_go_with_ensure_ascii_only_with_both(self):
        self.assertIsNone(V.json_go_ascii("falso-Ab3$k&<9>Zq!"), "D58 sem caractere fora do ASCII")
        self.assertIsNone(V.json_go_ascii("falso-a" + C + "o-01"), "D58 sem &<>")
        self.assertEqual(V.json_go_ascii("a&" + C), "a~u0026~u00e7".replace("~", "\\"))

    def test_go_with_ensure_ascii_per_line_and_line_value(self):
        line = "linha-tres-Ab3$k&<9>Zq!-a" + C + "o"
        p = pad("falso-linha-um-01\n" + line + "\nTOKEN=falso-tok&" + C + "-02\n")
        self.assertTrue(esc8("linha-tres-Ab3$k~u0026~u003c9~u003eZq!-a~u00e7o") in p, "D58 por linha")
        self.assertTrue(esc8("falso-tok~u0026~u00e7-02") in p, "D58 no valor de linha")

    def test_json_on_one_line(self):
        attachment = b'{"installed":{"client_id":"x.apps","client_secret":"falso-GOCSPX-minificado-01"}}\n'
        p = V.variations(attachment, None, attachment=True)
        self.assertTrue(b"falso-GOCSPX-minificado-01" in p, "anexo JSON minificado")
        self.assertTrue(base64.b64encode(b"falso-GOCSPX-minificado-01") in p, "como segredo inteiro")
        self.assertTrue(b"x.apps" in p, "valor de 6 caracteres também")
        p = pad('{"user":"x","token":"falso-json-uma-linha-01"}')
        self.assertTrue(b"falso-json-uma-linha-01" in p, "segredo de uma linha que começa por {")
        p = pad('{\n  "db": {"senha": "falso-json-aninhado-02"},\n  "a": "x", "b": "falso-json-duas-chaves-03"\n}')
        self.assertTrue(b"falso-json-aninhado-02" in p, "objeto aninhado na linha")
        self.assertTrue(b"falso-json-duas-chaves-03" in p, "duas chaves na linha")

    def test_json_decoded(self):
        bs = "\\"
        p = pad('{"k": "falso-' + bs + "u00e7" + bs + "/x" + bs + '"y-01", "z": "falso-par-'
                + bs + "ud83d" + bs + 'ude00-02"}')
        self.assertTrue(("falso-" + C + '/x"y-01').encode() in p, "escapes do JSON decodificados")
        self.assertTrue(("falso-par-" + chr(0x1F600) + "-02").encode() in p, "par substituto")

    def test_invalid_json_escape_discards(self):
        bs = "\\"
        p = pad('{"k": "falso-invalido-' + bs + 'x-01", "s": "falso-sozinho-' + bs + 'ud800-02"}')
        self.assertFalse(any(b"falso-invalido-x-01" in x for x in p), "escape inválido")
        self.assertFalse(any(x.startswith(b"falso-sozinho-") for x in p), "substituto sozinho")

    def test_json_array_and_flow_stay_in_ra3(self):
        self.assertEqual(V.json_values(b'["falso-elemento-01", "falso-elemento-02"]'), [])
        self.assertEqual(V.json_values(b"{k: falso-flow-yaml-01}"), [])

    def test_yaml_e_dotenv(self):
        value = (
            "senha: falso-yaml-coment-01 # comentario\n"
            "private key: falso-yaml-chave-espaco-02\n"
            "aws/secret: falso-yaml-chave-barra-03\n"
            'SENHA_DB="falso-env-coment-05" # dotenv\n'
            "tok: 'falso-it''s-yaml-06'\n"
            "outro: 'falso-it''s-yaml-07' # c\n"
        )
        p = pad(value)
        for x in (b"falso-yaml-coment-01", b"falso-yaml-chave-espaco-02", b"falso-yaml-chave-barra-03",
                  b"falso-env-coment-05", b"falso-it's-yaml-06", b"falso-it's-yaml-07"):
            self.assertTrue(x in p, "valor de YAML ou dotenv (D61)")
            self.assertTrue(base64.b64encode(x) in p, "como segredo inteiro")

    def test_composite_key_skips_url_and_comment(self):
        self.assertEqual(V.line_values(b"https://host.invalid:8080/x"), [])
        self.assertEqual(V.line_values(b"# nota: falso-comentado-01"), [])
        self.assertEqual(V.line_values(b"{k: falso-flow-yaml-01}"), [])

    def test_tab_fields_of_binary_line(self):
        attachment = (b"SENHA=falso-latin\xe7-01\tTOKEN=falso-campo-tab-02\n"
                 b"nome\xe7\tfalso-campo-tsv-03\tfalso-campo-tsv-04\n")
        p = V.variations(attachment, None, attachment=True)
        for x in (b"TOKEN=falso-campo-tab-02", b"falso-campo-tsv-03", b"falso-campo-tsv-04"):
            self.assertTrue(x in p, "campo de tab de linha binária pelo literal")
        self.assertTrue(b"TOKEN%3Dfalso-campo-tab-02" in p, "e pelo percent")
        self.assertFalse(base64.b64encode(b"falso-campo-tsv-04") in p, "sem base64 (D61)")
        self.assertFalse(b"falso-campo-tab-02" in p, "valor depois do = do campo binário (RA3)")


class Independence(unittest.TestCase):
    """The oracle does not use the mask code (V2.52)."""

    def test_does_not_call_the_core_crate(self):
        source = SOURCE.read_text(encoding="utf-8")
        for forbidden in ("import subprocess", "subprocess.", "os.system", "cargo ", "ward_core", "ward-core/src"):
            self.assertNotIn(forbidden, source, "variations.py depende do código da máscara")


if __name__ == "__main__":
    result = unittest.main(argv=[sys.argv[0]], exit=False, verbosity=1).result
    sys.exit(0 if result.wasSuccessful() else 1)
