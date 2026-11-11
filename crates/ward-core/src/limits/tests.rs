//! Tests of the limits in a single source (D32, V2.35) and of the mask budget (D29,
//! V2.36).
//!
//! Every number here comes from `crate::limits`: if a constant changes, validation,
//! body reading and budget change together, and these tests still hold at the new
//! limit. Fake values only.

use zeroize::Zeroizing;

use super::*;
use crate::configuracao::{Configuracao, HashPin};
use crate::corpo::{ErroCorpo, desserializar, serializar};
use crate::entrega::{self, Entrega, ErroRecurso, Recursos};
use crate::entry::{
    AttachmentDraft, DataDraft, Draft, Entry, EntryType, ExtraDraft, Secret, TOO_LARGE_FOR_MASK,
    ValidationErrors,
};
use crate::mascara::{self, ConstrutorMascara, ErroMascara};

// ---- Helpers ----

/// Configuration of body v2 (hash of the fake D15 PIN), computed once.
fn cfg() -> &'static Configuracao {
    static C: std::sync::OnceLock<Configuracao> = std::sync::OnceLock::new();
    C.get_or_init(|| Configuracao::nova(HashPin::criar(b"135790").unwrap()))
}

fn t(v: &str) -> String {
    v.to_owned()
}

fn sec(v: Vec<u8>) -> Option<Secret> {
    Some(Secret::from_bytes(v))
}

/// Text of `n` bytes that exercises the mask variations: JSON escape (`"`, `\`, `/`)
/// and percent-encoding in the URL (`@`, `:`, `%`), without NUL or control.
fn hard_text(n: usize) -> Vec<u8> {
    const PATTERN: &[u8] = b"falso\"\\/@:%-k";
    (0..n).map(|i| PATTERN[i % PATTERN.len()]).collect()
}

/// Bytes from 0 to 255 repeated (binary, with line breaks).
fn binary(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 256) as u8).collect()
}

fn fake_key(n: usize) -> Vec<u8> {
    let label = ["OPENSSH", "PRIVATE KEY"].join(" ");
    let mut v = format!("-----BEGIN {label}-----\n").into_bytes();
    let end = format!("\n-----END {label}-----\n").into_bytes();
    let middle = n.saturating_sub(v.len() + end.len());
    v.extend((0..middle).map(|i| if i % 70 == 69 { b'\n' } else { b'f' }));
    v.extend(end);
    v
}

fn draft(name: &str, data: DataDraft) -> Draft {
    Draft {
        name: t(name),
        group: t("zeta"),
        environment: t("stage"),
        project: t("projeto"),
        tags: (1..=MAX_TAGS).map(|n| format!("tag-{n:02}")).collect(),
        data,
        extras: Vec::new(),
        attachments: Vec::new(),
    }
}

/// The large fields of the type (4 KiB secrets, which already generate URL, Basic auth
/// and escapes; the private key of the server and ftp/sftp types at the 64 KiB limit, in
/// several lines, so that the per-line pass of D36 weighs in, V2.43), tunnel reference
/// when `tunnel`.
fn large_data(kind: EntryType, tunnel: Option<&str>) -> DataDraft {
    const S: usize = 4 * 1024;
    match kind {
        EntryType::Login => DataDraft::Login {
            url: format!("https://cheio.invalid/{}", "a".repeat(MAX_URL - 22)),
            username: "u".repeat(MAX_LINE),
            password: sec(hard_text(S)),
            totp_seed: sec(vec![b'A'; MAX_TOTP]),
        },
        EntryType::Database => DataDraft::Database {
            engine: t("redis"),
            host: "h".repeat(MAX_HOST),
            port: t("1"),
            db_name: t("99999"),
            username: "u".repeat(MAX_LINE),
            password: sec(hard_text(S)),
            tunnel: tunnel.map(t).unwrap_or_default(),
        },
        EntryType::Server => DataDraft::Server {
            host: "h".repeat(MAX_HOST),
            port: t("65535"),
            username: "u".repeat(MAX_LINE),
            password: None,
            private_key: sec(fake_key(MAX_SECRET)),
            passphrase: sec(hard_text(S)),
            jump: String::new(),
        },
        EntryType::Api => DataDraft::Api {
            url: format!("https://cheio.invalid/{}", "a".repeat(MAX_URL - 22)),
            token: sec(hard_text(S)),
            variable: t("FALSO_TOKEN"),
        },
        EntryType::Aws => DataDraft::Aws {
            access_key_id: "A".repeat(MAX_ACCESS_KEY),
            secret_access_key: sec(hard_text(S)),
            region: "r".repeat(MAX_REGION),
            role_arn: format!("arn:{}", "a".repeat(MAX_URL - 4)),
            session_token: sec(hard_text(S)),
        },
        EntryType::Smtp => DataDraft::Smtp {
            host: "h".repeat(MAX_HOST),
            port: t("1"),
            username: "u".repeat(MAX_LINE),
            password: sec(hard_text(S)),
            security: t("tls"),
        },
        EntryType::FtpSftp => DataDraft::FtpSftp {
            protocol: t("sftp"),
            host: "h".repeat(MAX_HOST),
            port: t("1"),
            username: "u".repeat(MAX_LINE),
            password: None,
            private_key: sec(fake_key(MAX_SECRET)),
            passphrase: sec(hard_text(S)),
        },
        EntryType::SecureNote => DataDraft::SecureNote {
            text: sec(vec![b'n'; MAX_NOTE]),
        },
        EntryType::Generic => DataDraft::Generic,
    }
}

/// Multi-line secret extra (D36): a service account JSON with lines `"key": "value"`,
/// `key: value` and `KEY=value`, of `n` bytes. Each line and each value become
/// patterns, with the escaped forms of D37 (`$`, `&`, `\`, non-ASCII) and the per-line
/// text forms of D54 (single quotes, `printf %q`, PHP, tab fields, `key : value`) and
/// the candidates of D61 (JSON pairs anywhere in the line, trailing comment in YAML and
/// dotenv, compound YAML key, `''`, Go with `ensure_ascii`).
fn multi_line_json(n: usize) -> Vec<u8> {
    let mut v = b"{\n".to_vec();
    let mut i = 0usize;
    while v.len() + 80 < n {
        let line = match i % 9 {
            0 => format!("  \"campo_{i:05}\": \"falso-valor-$&ç-{i:05}-0123456789\",\n"),
            1 => format!("  chave_{i:05}: falso-yaml-\\-{i:05}-0123456789\n"),
            2 => format!("  CHAVE_{i:05}=falso-env-`<>`-{i:05}-0123456789\n"),
            3 => format!("  \"x/{i:05}\":\"falso-'php'/ação-{i:05}\tcampo-{i:05}\",\n"),
            4 => format!("  chave_{i:05} : falso esp {i:05}\tcampo-de-tab-{i:05}\n"),
            5 => format!(
                "  \"a\": \"x\", \"p_{i:05}\": \"falso-par-\\u00e7-{i:05}\", \"b\": {{\"c\": \"falso-ani-&ç-{i:05}\"}},\n"
            ),
            6 => format!("  senha_{i:05}: falso-yaml-{i:05}-0123 # comentario {i:05}\n"),
            7 => format!("  private key {i:05}: 'falso-it''s-{i:05}-0123' # c\n"),
            _ => format!("  SENHA_{i:05}=\"falso-env-{i:05}-0123456789\" # dotenv\n"),
        };
        v.extend_from_slice(line.as_bytes());
        i += 1;
    }
    v.extend_from_slice(b"}\n");
    v
}

/// Entry of the type with the large fields, attachments adding up to the D29 limit, a
/// multi-line secret extra (D36) and `total` bytes of secret extras (in extras of up to
/// [`MAX_EXTRA_VALUE`], at most [`MAX_EXTRAS`]).
fn full(kind: EntryType, total: usize, tunnel: Option<&str>) -> Draft {
    let mut r = draft(
        &format!("zz-{}", kind.name().replace('/', "-")),
        large_data(kind, tunnel),
    );
    // With the 64 KiB key (cost ~1.6 MiB in the mask), the server and ftp/sftp types do not
    // fit the budget with the attachments at the D29 limit (it was already so before
    // D36: the per-line pass of the key adds only ~64 KiB); their largest accepted entry
    // carries half the attachments.
    let binary_max = if matches!(kind, EntryType::Server | EntryType::FtpSftp) {
        MAX_ATTACHMENTS_TOTAL / 2
    } else {
        MAX_ATTACHMENTS_TOTAL - 4096
    };
    r.attachments = vec![
        AttachmentDraft {
            name: t("binario.bin"),
            content: Secret::from_bytes(binary(binary_max)),
        },
        AttachmentDraft {
            name: t("app.env"),
            content: Secret::from_bytes(
                b"FALSO_ENV=falso-anexo-01\nPORT=8080\n"
                    .iter()
                    .copied()
                    .cycle()
                    .take(4096)
                    .collect(),
            ),
        },
    ];
    r.extras.push(ExtraDraft {
        name: t("json"),
        value: Secret::from_bytes(multi_line_json(4 * 1024)),
        secret: true,
    });
    let mut rest = total;
    let mut i = 1;
    while rest > 0 && i < MAX_EXTRAS {
        let n = rest.min(MAX_EXTRA_VALUE);
        r.extras.push(ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(hard_text(n)),
            secret: true,
        });
        rest -= n;
        i += 1;
    }
    r
}

fn too_large(e: &ValidationErrors) -> bool {
    e.has_reason("entrada", TOO_LARGE_FOR_MASK)
}

/// The largest `total` of extras that validation accepts (binary search) and the entry
/// at it.
fn largest_accepted(kind: EntryType, tunnel: Option<&str>) -> (usize, Entry) {
    let ceiling = MAX_EXTRAS * MAX_EXTRA_VALUE;
    let mut lo = 0usize;
    let mut hi = ceiling;
    let accepts = |n: usize| match full(kind, n, tunnel).validate() {
        Ok(_) => true,
        Err(e) => {
            assert!(too_large(&e), "{kind}: recusada por outro motivo: {e}");
            false
        }
    };
    assert!(accepts(0), "{kind}: nem sem extras cabe");
    if accepts(hi) {
        lo = hi;
    } else {
        // Invariant: `lo` accepts, `hi` refuses.
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if accepts(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    let e = full(kind, lo, tunnel).validate().unwrap();
    (lo, e)
}

/// Fake `ward run` resources (the real ones belong to the daemon).
struct Fakes;

impl Recursos for Fakes {
    fn codigo_totp(&mut self, _: &Secret) -> Result<Zeroizing<String>, ErroRecurso> {
        Ok(Zeroizing::new(t("123456")))
    }
    fn arquivo(&mut self, name: &str, _: &Secret) -> Result<String, ErroRecurso> {
        Ok(format!("/run/user/1000/ward/tmp/1-{name}"))
    }
    fn tunel(&mut self, _: &str, _: &str, _: u16) -> Result<u16, ErroRecurso> {
        Ok(40123)
    }
}

fn delivery_of(e: &Entry) -> Entrega {
    entrega::montar(e, &mut Fakes).unwrap()
}

/// Real sum of the patterns of the `ward run` mask (entries and deliveries).
fn real_cost(entries: &[&Entry], deliveries: &[&Entrega]) -> usize {
    let mut c = ConstrutorMascara::new();
    for e in entries {
        c = c.entrada(e);
    }
    for g in deliveries {
        c = c.entrega(g);
    }
    mascara::somar(&c, usize::MAX).unwrap()
}

// ---- V2.35: a single source ----

#[test]
fn v2_35_validation_and_body_at_the_same_attachment_limit() {
    let generic = |sizes: &[usize]| {
        let mut r = draft("anexos", DataDraft::Generic);
        r.extras.push(ExtraDraft {
            name: t("x"),
            value: Secret::from_text(t("y")),
            secret: false,
        });
        r.attachments = sizes
            .iter()
            .enumerate()
            .map(|(i, &n)| AttachmentDraft {
                name: format!("a{i}.bin"),
                content: Secret::from_bytes(vec![0x5a; n]),
            })
            .collect();
        r
    };
    // At the limit: validation accepts, the body writes and reads.
    let at_limit = generic(&[MAX_ATTACHMENTS_TOTAL - 1, 1]).validate().unwrap();
    let body = serializar(cfg(), 1, core::slice::from_ref(&at_limit)).unwrap();
    assert_eq!(desserializar(&body).map(|(_, _, v)| v.len()), Ok(1));
    // One byte above: validation refuses, and so does the body with that extra byte.
    assert!(generic(&[MAX_ATTACHMENTS_TOTAL, 1]).validate().is_err());
    let mut forged = body.to_vec();
    // The last field of the body is the content of the 2nd attachment (1 byte): grows
    // to 2.
    let n = forged.len();
    forged[n - 5..n - 1].copy_from_slice(&2u32.to_le_bytes());
    forged.push(0x5a);
    assert_eq!(desserializar(&forged).err(), Some(ErroCorpo::Invalido));
    // And the same body without the extra byte opens again (the forger is right).
    forged[n - 5..n - 1].copy_from_slice(&1u32.to_le_bytes());
    forged.pop();
    assert_eq!(desserializar(&forged).map(|(_, _, v)| v.len()), Ok(1));
}

#[test]
fn v2_35_budget_is_the_same_in_validation_and_in_the_mask() {
    // Validation refuses exactly when the cost by the mask pattern generator goes over the
    // budget: near the boundary, on both sides.
    let (n, e) = largest_accepted(EntryType::Api, None);
    let cost = mascara::custo_da_entrada(&e).unwrap();
    assert!(cost <= MASK_BUDGET);
    let r = full(EntryType::Api, n + 1, None);
    assert!(r.validate().is_err_and(|e| too_large(&e)));
    // The validation cost is that of the mask built with the real delivery, or more
    // (the measured tunnel port is the longest).
    let g = delivery_of(&e);
    assert!(real_cost(&[&e], &[&g]) <= cost);
}

// ---- V2.36: no accepted entry is left without `ward run` ----

/// V2.36 and V2.43 (D36, D37): with the per-line pass and the escaped forms, the largest
/// accepted entry of each type with delivery (with the 64 KiB private key in several
/// lines in the server and ftp/sftp types and a multi-line secret extra) still builds the
/// mask, and the cost measured by validation is ≥ the real one.
#[test]
fn v2_36_largest_entry_of_each_type_builds_the_mask() {
    for kind in EntryType::ALL {
        if kind == EntryType::SecureNote {
            continue;
        }
        let (_, e) = largest_accepted(kind, None);
        // The multi-line extra is there, and the key at the D25 limit (V2.43).
        assert!(e.extras().iter().any(|x| x.name() == "json"), "{kind}");
        if matches!(kind, EntryType::Server | EntryType::FtpSftp) {
            let key = e
                .secrets()
                .into_iter()
                .map(|s| s.value.expose().len())
                .max()
                .unwrap();
            assert!(key >= MAX_SECRET - 64, "{kind}: chave de {key} bytes");
        }
        let g = delivery_of(&e);
        let cost = mascara::custo_da_entrada(&e).unwrap();
        assert!(cost <= MASK_BUDGET, "{kind}");
        let real = real_cost(&[&e], &[&g]);
        assert!(real <= cost, "{kind}: {real} > {cost}");
        assert!(real <= PATTERN_LIMIT, "{kind}");
        mascara::Mascara::da_entrada(&e, Some(&g))
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
    }
}

#[test]
fn v2_36_database_and_tunnel_server_pair_builds_the_mask() {
    // The two entries of a combined use (D19), each at the largest accepted size.
    let (_, server) = largest_accepted(EntryType::Server, None);
    let (_, database) = largest_accepted(EntryType::Database, Some(server.name()));
    assert!(database.verify_in_vault([&server]).is_ok());
    let g_database = delivery_of(&database);
    let g_server = delivery_of(&server);
    let real = real_cost(&[&database, &server], &[&g_database, &g_server]);
    assert!(real <= PATTERN_LIMIT, "{real}");
    ConstrutorMascara::new()
        .entrada(&database)
        .entrada(&server)
        .entrega(&g_database)
        .entrega(&g_server)
        .construir()
        .expect("o par cabe na máscara");
    // And the same holds for the jump (server + jump server).
    let (_, jump) = largest_accepted(EntryType::Server, None);
    let g_jump = delivery_of(&jump);
    assert!(real_cost(&[&server, &jump], &[&g_server, &g_jump]) <= PATTERN_LIMIT);
}

#[test]
fn v2_36_cost_refuses_above_the_budget_and_note_stays_out() {
    // Above the budget: the too-large error, without value in the error.
    let mut r = full(EntryType::Generic, 0, None);
    r.extras = (0..MAX_EXTRAS)
        .map(|i| ExtraDraft {
            name: format!("e{i:02}"),
            value: Secret::from_bytes(vec![b'k'; MAX_EXTRA_VALUE]),
            secret: true,
        })
        .collect();
    let errors = r.validate().unwrap_err();
    assert!(too_large(&errors));
    assert!(!format!("{errors} {errors:?}").contains("kkkk"));
    // The secure note has no delivery: cost 0, even when full.
    let note = full(EntryType::SecureNote, MAX_EXTRAS * MAX_EXTRA_VALUE, None)
        .validate()
        .unwrap();
    assert_eq!(mascara::custo_da_entrada(&note), Ok(0));
    assert!(!format!("{}", ErroMascara::GrandeDemais).contains("falso"));
}

/// Text of numbered fake lines separated by `sep`, of `n` bytes.
fn lines_with(sep: u8, n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(n + 32);
    let mut i = 0usize;
    while v.len() < n {
        v.extend_from_slice(format!("falso-linha-{i:05}-0123456789").as_bytes());
        v.push(sep);
        i += 1;
    }
    v.truncate(n);
    v
}

/// The full generic with a secret extra of lines separated by `sep` and `total` bytes
/// of filler extras.
fn with_lines(sep: u8, total: usize) -> Draft {
    let mut r = full(EntryType::Generic, total, None);
    // Without the attachments: the filler (extras) is what takes the entry to the
    // budget.
    r.attachments.clear();
    r.extras.push(ExtraDraft {
        name: t("linhas"),
        value: Secret::from_bytes(lines_with(sep, MAX_EXTRA_VALUE)),
        secret: true,
    });
    r
}

/// V2.43 (D29 with D36): the budget counts the per-line pass. Tab and line break cost
/// the same before D36 (both become 2 bytes in JSON, `%09`/`%0A` in the URL and the
/// same size in base64); with D36 only the break generates the per-line patterns. At
/// the largest filler at which the tab version fits, the version with line breaks goes
/// over the budget only because of D36 and is refused with the D29 message, without
/// value in the error.
#[test]
fn v2_43_entry_that_only_goes_over_the_budget_by_d36_is_refused() {
    let accepts = |n: usize| match with_lines(b'\t', n).validate() {
        Ok(_) => true,
        Err(e) => {
            assert!(too_large(&e), "recusada por outro motivo: {e}");
            false
        }
    };
    let (mut lo, mut hi) = (0usize, MAX_EXTRA_VALUE * 40);
    assert!(accepts(lo) && !accepts(hi));
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if accepts(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let tab = with_lines(b'\t', lo).validate().unwrap();
    let tab_cost = mascara::custo_da_entrada(&tab).unwrap();
    assert!(tab_cost <= MASK_BUDGET);
    let errors = with_lines(b'\n', lo).validate().unwrap_err();
    assert!(too_large(&errors), "{errors}");
    let text = format!("{errors} {errors:?}");
    assert!(!text.contains("falso"), "erro com valor");
    assert!(text.contains("entrada grande demais para a máscara do `ward run`"));
    // With the same line in a smaller chunk, the version with breaks fits and its cost
    // is ≥ the real one (the same pattern generator).
    let small = {
        let mut r = full(EntryType::Generic, 0, None);
        r.extras.push(ExtraDraft {
            name: t("linhas"),
            value: Secret::from_bytes(lines_with(b'\n', 16 * 1024)),
            secret: true,
        });
        r.validate().unwrap()
    };
    let g = delivery_of(&small);
    let cost = mascara::custo_da_entrada(&small).unwrap();
    assert!(real_cost(&[&small], &[&g]) <= cost);
}
