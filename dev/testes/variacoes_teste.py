#!/usr/bin/python3 -IB
"""Testes do `dev/variacoes.py` (V2.52, mais as variações de D11 de V2.29; Fase 19d).

Uso (da raiz): python3 -I -B dev/testes/variacoes_teste.py  (`make teste-dev-cofre` roda).

Só a biblioteca padrão. Os valores são falsos e montados aqui, não lidos das fixtures. As
formas esperadas estão escritas à mão (ou com `base64`/`urllib` da biblioteca padrão),
não calculadas pelo `variacoes.py`. As mensagens de falha dizem o caso, nunca o valor
nem a lista inteira.
"""

from __future__ import annotations

import base64
import importlib.util
import re
import sys
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
FONTE = RAIZ / "dev" / "variacoes.py"


def carregar():
    spec = importlib.util.spec_from_file_location("variacoes", FONTE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


V = carregar()


def pad(valor: str | bytes, usuario: str | None = None) -> set[bytes]:
    v = valor.encode("utf-8") if isinstance(valor, str) else valor
    return V.variacoes(v, usuario)


def esc(forma: str) -> bytes:
    """Forma esperada com `~` no lugar da barra invertida (o texto do teste fica sem
    sequências de escape que um editor ou ferramenta possa decodificar)."""
    return forma.replace("~", "\\").encode("ascii")


SENHA_GO = "falso-Ab3$k&<9>Zq!"
ACENTO = "senha-a" + chr(0xE7) + chr(0xE3) + "o-01"
EMOJI = "falso-" + chr(0x1F600) + "-01"
LS = "falso" + chr(0x2028) + "linha01"
PS = "falso" + chr(0x2029) + "linha01"


class D37FormasEscapadas(unittest.TestCase):
    """V2.52: JSON do Go, `ensure_ascii` do Python e aspas duplas do shell."""

    def tem(self, valor, forma: bytes, caso: str) -> None:
        self.assertTrue(forma in pad(valor), f"falta a forma: {caso}")

    def test_esc_monta_a_barra(self):
        self.assertEqual(esc("a~u0026"), bytes([0x61, 0x5C, 0x75, 0x30, 0x30, 0x32, 0x36]))

    def test_json_do_go(self):
        self.tem(SENHA_GO, esc("falso-Ab3$k~u0026~u003c9~u003eZq!"), "JSON do Go")

    def test_aspas_duplas_do_shell(self):
        self.tem(SENHA_GO, esc("falso-Ab3~$k&<9>Zq!"), "aspas duplas do shell")
        self.tem('falso-`a"b~c$d'.replace("~", "\\"), esc('falso-~`a~"b~~c~$d'), "shell com ` \" \\ e $")

    def test_ensure_ascii_minusculo(self):
        self.tem(ACENTO, esc("senha-a~u00e7~u00e3o-01"), "ensure_ascii")

    def test_ensure_ascii_com_par_substituto(self):
        self.tem(EMOJI, esc("falso-~ud83d~ude00-01"), "par substituto")

    def test_u2028_e_u2029(self):
        self.tem(LS, esc("falso~u2028linha01"), "U+2028")
        self.tem(PS, esc("falso~u2029linha01"), "U+2029")

    def test_hex_maiusculo_fica_de_fora(self):
        # RA3: o hex em maiúsculas (~u00E7) não é gerado, como na máscara.
        p = pad(ACENTO)
        self.assertFalse(esc("senha-a~u00E7~u00E3o-01") in p, "hex maiúsculo")
        self.assertFalse(any(re.search(rb"\x5cu[0-9a-f]*[A-F]", x) for x in p), "hex maiúsculo")

    def test_so_quando_difere_da_literal(self):
        for v in ("falso-simples-01", "falso_token_github_01", "falso-WP-senha-02"):
            self.assertEqual(V.escapadas(v.encode()), set(), f"forma nova sem caractere: {v[:6]}")
        # Nenhum padrão com barra invertida para quem não tem nada a escapar.
        p = pad("falso-simples-01")
        self.assertFalse(any(b"\x5c" in x for x in p), "padrão escapado sem motivo")
        # E a literal não muda: o conjunto é o mesmo de antes de D37 (codificações de D11).
        self.assertTrue(b"falso-simples-01" in p, "literal")

    def test_formas_tambem_por_linha(self):
        p = pad("falso-linha-um-01\n" + SENHA_GO + "\n")
        self.assertTrue(esc("falso-Ab3$k~u0026~u003c9~u003eZq!") in p, "JSON do Go da linha")
        self.assertTrue(esc("falso-Ab3~$k&<9>Zq!") in p, "shell da linha")
        valor = 'export FALSO_SENHA="' + ACENTO + '"\n'
        self.assertTrue(esc("senha-a~u00e7~u00e3o-01") in pad(valor), "ensure_ascii do valor da linha")

    def test_inteiro_de_varias_linhas_em_json(self):
        # O JSON do Go do segredo inteiro (com quebra) é uma linha só e entra.
        p = pad("falso-<um>-linha\nfalso-dois-linha\n")
        self.assertTrue(esc("falso-~u003cum~u003e-linha~nfalso-dois-linha~n") in p, "Go do inteiro")


class D36PassadaPorLinha(unittest.TestCase):
    """V2.52: linhas aparadas e valores de `CHAVE=valor`, JSON e YAML."""

    CHAVE = (
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

    def test_linhas_aparadas(self):
        p = pad(self.CHAVE)
        for linha in (
            b"-----BEGIN CHAVE FALSA DE TESTE-----",
            b"b3BlbnNzaC1rZXktdjEAAAAAfalsofalso01",
            b"AAAAC3NzaC1lZDI1NTE5AAAAIfalsofalso02",
            b"-----END CHAVE FALSA DE TESTE-----",
        ):
            self.assertTrue(linha in p, "linha aparada da chave")
        self.assertFalse(b"curta" in p, "linha curta virou padrão")
        self.assertFalse(any(b"\r" in x or b"\n" in x for x in p), "padrão com quebra")

    def test_json_de_varias_linhas(self):
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

    def test_chave_igual(self):
        p = pad("FALSO_A=falso-anexo-01\nexport FALSO_B='falso-anexo-02'\nPORT=8080\n")
        for x in (b"falso-anexo-01", b"falso-anexo-02", b"'falso-anexo-02'", b"FALSO_A=falso-anexo-01"):
            self.assertTrue(x in p, "valor de CHAVE=valor")
        self.assertFalse(b"8080" in p, "valor curto de CHAVE=valor (D34)")

    def test_url_e_host_porta_nao_viram_yaml(self):
        valores = V.valores_da_linha(b"https://x.example:8080/falso-caminho")
        self.assertEqual(valores, [], "URL lida como chave: valor")
        self.assertEqual(V.valores_da_linha(b"host:falso-porta-01"), [], "host:porta lido como YAML")

    def test_minimo_em_caracteres(self):
        # 5 caracteres e 7 bytes: abaixo do mínimo de D34/D36, como na máscara.
        self.assertEqual(V.valores_da_linha("X: ação1".encode()), [], "mínimo contado em bytes")
        self.assertEqual(V.linhas_de_anexo("çãçãç\n".encode()), [], "linha curta em caracteres")

    def test_segredo_de_uma_linha_nao_passa_por_linha(self):
        # `"a": "b"` num segredo de uma linha só não é partido (D36 é para várias linhas).
        p = pad('"falso": "falso-valor-01"')
        self.assertFalse(b"falso-valor-01" in p, "segredo de uma linha partido")


class D11DeAntes(unittest.TestCase):
    """V2.29: as variações de D11 continuam (esperadas calculadas com a biblioteca padrão)."""

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
            "campos": {"motor": "postgres", "usuario": "app", "host": "127.0.0.1", "porta": "55432", "base": "app_teste"},
            "segredos": {"senha": "falso-PG:s3nha/01@#"},
        }
        urls = V.database_url(e)
        self.assertTrue(b"postgresql://app:falso-PG%3As3nha%2F01%40%23@127.0.0.1:55432/app_teste" in urls, "DATABASE_URL")

    def test_base64_dentro_de_maior(self):
        p = pad("falso_token_github_01")
        maior = base64.b64encode(b"x-access-token:falso_token_github_01")
        self.assertTrue(any(x in maior for x in p if len(x) >= 8), "miolo do base64")

    def test_json_escape_e_php(self):
        p = pad('falso/"aspas"-01')
        self.assertTrue(rb'falso/\"aspas\"-01' in p, "escape JSON")
        self.assertTrue(rb'falso\/\"aspas\"-01' in p, "escape do PHP")

    def test_lista_sem_padrao_curto_nem_quebra(self):
        with self.assertRaises(V.Recusa):
            V.conferir_padroes({b"falso"})
        with self.assertRaises(V.Recusa):
            V.conferir_padroes({b"falso-\nlinha-01"})
        todos = set()
        for s in (SENHA_GO, ACENTO, EMOJI, LS, PS,
                  D36PassadaPorLinha.CHAVE, D36PassadaPorLinha.JSON, D36PassadaPorLinha.YAML):
            todos |= pad(s, "app")
        lista = V.conferir_padroes(todos)
        self.assertEqual(lista, sorted(lista), "lista fora de ordem (não determinística)")

    def test_saida_fora_de_dev_recusada(self):
        for caminho in ("/tmp/variacoes.txt", "../variacoes.txt", "dev/variacoes.txt"):
            with self.assertRaises(V.Recusa):
                V.validar_saida(caminho)


class Independencia(unittest.TestCase):
    """O oráculo não usa o código da máscara (V2.52)."""

    def test_nao_chama_o_ward_core(self):
        fonte = FONTE.read_text(encoding="utf-8")
        for proibido in ("import subprocess", "subprocess.", "os.system", "cargo ", "ward_core", "ward-core/src"):
            self.assertNotIn(proibido, fonte, "variacoes.py depende do código da máscara")


if __name__ == "__main__":
    resultado = unittest.main(argv=[sys.argv[0]], exit=False, verbosity=1).result
    sys.exit(0 if resultado.wasSuccessful() else 1)
