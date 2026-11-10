//! `wardd-app-teste`: app de teste mínimo dos testes do `wardd` (D63, Fase 23a). Só
//! existe com a feature `test-mode` (`required-features`), nunca no release.
//!
//! Conecta ao daemon de teste (`$WARD_DEV_SOCKET_DIR/wardd.sock`, ou
//! `/run/user/<uid>/ward-teste/wardd.sock`), pede `estado` e `revelar` e imprime, uma
//! por linha, só o que o teste confere (sem valor nenhum):
//! - `papel=<agente|app|extensao>`, da primeira resposta;
//! - `revelar=<tipo ou código do erro>`;
//! - `ambiente=<nomes das variáveis, em ordem>`;
//! - `argumentos=<n>`;
//! - D65 b e c (Fase 22c, V3.39 e V3.40), só quando o papel é app, contra o cofre de
//!   teste de `crates/ward-core/src/corpo/cofre_teste.rs` (o teste grava o mesmo):
//!   `revelar_pg`, `revelar_bloqueada` e `copiar_pg` (`igual` quando o valor bate com o
//!   da entrada, `diferente`, ou o tipo/código), `totp` (`igual` quando bate com o
//!   código local do mesmo período), `campo_inexistente` (o código), `lista` (`resumo`
//!   quando todo item traz resumo e nenhum segredo, ou o motivo), e por fim `travar` e
//!   `revelar_travado`. Nenhum valor sai: só a comparação;
//! - `fim=1`.
//!
//! Aberto pelo `abrir_app` do daemon de teste (com `WARD_DEV_APP` apontando para ele) a
//! saída vai para `.dev/logs/wardd-app-teste.log`; executado à mão, para o terminal.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ward_core::entrada::Entrada;
use ward_core::protocolo::{self, Alvo, Papel, Pedido, Resposta};
use ward_core::totp::Totp;

// O cofre de teste (só valores falsos) pela API pública, como o daemon falso do app.
use ward_core as wc;
#[path = "../../ward-core/src/corpo/cofre_teste.rs"]
mod cofre_teste;

fn pedir(s: &mut UnixStream, p: &Pedido) -> Option<Vec<u8>> {
    let corpo = protocolo::codificar_pedido(Papel::App, p).ok()?;
    s.write_all(&u32::try_from(corpo.len()).ok()?.to_be_bytes())
        .and_then(|()| s.write_all(&corpo))
        .ok()?;
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let tamanho = usize::try_from(u32::from_be_bytes(n)).ok()?;
    let mut corpo = vec![0u8; tamanho.min(1 << 20)];
    s.read_exact(&mut corpo).ok()?;
    Some(corpo)
}

/// O papel vem do `estado`; o resto da leitura é a do agente (aceita tudo que o daemon
/// manda a qualquer papel) para o tipo do `revelar`.
fn descrever(corpo: &[u8]) -> String {
    for papel in [Papel::App, Papel::Agente] {
        match protocolo::decodificar_resposta(papel, corpo) {
            Ok(Resposta::Erro(e)) => return e.codigo().to_owned(),
            Ok(r) => return r.tipo().nome().to_owned(),
            Err(_) => {}
        }
    }
    "invalida".to_owned()
}

/// O segredo da entrada do cofre de teste (para comparar, nunca para imprimir).
fn esperado<'a>(entradas: &'a [Entrada], nome: &str, alvo: &Alvo) -> Option<&'a [u8]> {
    let e = entradas.iter().find(|e| e.nome() == nome)?;
    alvo.segredo_em(e).map(|s| s.expor())
}

/// `igual`/`diferente` para um `segredo`; o tipo ou o código do erro para o resto.
fn comparar(corpo: Option<&[u8]>, esperado: Option<&[u8]>) -> String {
    let Some(corpo) = corpo else {
        return String::new();
    };
    match protocolo::decodificar_resposta(Papel::App, corpo) {
        Ok(Resposta::Segredo(s)) if Some(s.expor()) == esperado => "igual".to_owned(),
        Ok(Resposta::Segredo(_)) => "diferente".to_owned(),
        _ => descrever(corpo),
    }
}

fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
    !agulha.is_empty() && palheiro.windows(agulha.len()).any(|w| w == agulha)
}

/// D65 b e c contra o cofre de teste: o que o app recebe, comparado sem imprimir.
fn valores_do_app(s: &mut UnixStream) {
    let entradas = cofre_teste::cofre_de_teste();
    let senha = || Alvo::Campo("senha".into());
    for (rotulo, revelar, nome) in [
        ("revelar_pg", true, "acme-pg-dev"),
        ("revelar_bloqueada", true, "gama-bloqueada"),
        ("copiar_pg", false, "acme-pg-dev"),
    ] {
        let p = if revelar {
            Pedido::Revelar {
                entrada: nome.into(),
                alvo: senha(),
            }
        } else {
            Pedido::Copiar {
                entrada: nome.into(),
                alvo: senha(),
            }
        };
        let r = pedir(s, &p);
        println!(
            "{rotulo}={}",
            comparar(r.as_deref(), esperado(&entradas, nome, &senha()))
        );
    }
    let semente = esperado(
        &entradas,
        "acme-totp-dev",
        &Alvo::Campo("totp_semente".into()),
    )
    .map(|v| wc::entrada::Segredo::de_bytes(v.to_vec()));
    let local = || {
        semente
            .as_ref()
            .and_then(|s| Totp::de_semente(s).ok())
            .and_then(|t| t.codigo_agora().ok())
            .map(|c| c.codigo().to_owned())
    };
    let antes = local();
    let r = pedir(
        s,
        &Pedido::Totp {
            entrada: "acme-totp-dev".into(),
        },
    );
    let depois = local();
    let totp = match r
        .as_deref()
        .map(|c| protocolo::decodificar_resposta(Papel::App, c))
    {
        Some(Ok(Resposta::CodigoTotp { codigo, restante })) => {
            let bate = [&antes, &depois]
                .iter()
                .any(|l| l.as_deref() == Some(codigo.as_str()));
            if bate && (1..=30).contains(&restante) {
                "igual".to_owned()
            } else {
                "diferente".to_owned()
            }
        }
        _ => r.as_deref().map(descrever).unwrap_or_default(),
    };
    println!("totp={totp}");
    let r = pedir(
        s,
        &Pedido::Revelar {
            entrada: "acme-pg-dev".into(),
            alvo: Alvo::Campo("chave_privada".into()),
        },
    );
    println!(
        "campo_inexistente={}",
        r.as_deref().map(descrever).unwrap_or_default()
    );
    let r = pedir(s, &Pedido::Lista);
    let lista = match r
        .as_deref()
        .map(|c| (c, protocolo::decodificar_resposta(Papel::App, c)))
    {
        Some((c, Ok(Resposta::Lista(itens)))) => {
            if itens.len() != entradas.len() {
                "contagem".to_owned()
            } else if !itens
                .iter()
                .all(|i| i.resumo.is_some() && i.nivel.is_some())
            {
                "sem_resumo".to_owned()
            } else if cofre_teste::SEGREDOS
                .iter()
                .any(|v| contem(c, v.as_bytes()))
            {
                "com_segredo".to_owned()
            } else {
                "resumo".to_owned()
            }
        }
        _ => r.as_deref().map(descrever).unwrap_or_default(),
    };
    println!("lista={lista}");
    let r = pedir(s, &Pedido::Travar);
    println!("travar={}", r.as_deref().map(descrever).unwrap_or_default());
    let r = pedir(
        s,
        &Pedido::Revelar {
            entrada: "acme-pg-dev".into(),
            alvo: senha(),
        },
    );
    println!(
        "revelar_travado={}",
        r.as_deref().map(descrever).unwrap_or_default()
    );
}

fn main() -> ExitCode {
    let mut nomes: Vec<String> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    nomes.sort();
    println!("ambiente={}", nomes.join(","));
    println!(
        "argumentos={}",
        std::env::args_os().count().saturating_sub(1)
    );
    let dir = std::env::var_os("WARD_DEV_SOCKET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "/run/user/{}/ward-teste",
                ward_platform::uid_efetivo()
            ))
        });
    let mut s = match UnixStream::connect(dir.join("wardd.sock")) {
        Ok(s) => s,
        Err(e) => {
            println!("connect={}", e.raw_os_error().unwrap_or(-1));
            return ExitCode::FAILURE;
        }
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let papel = pedir(&mut s, &Pedido::Estado).and_then(|c| {
        Papel::TODOS
            .into_iter()
            .find_map(|p| match protocolo::decodificar_resposta(p, &c) {
                Ok(Resposta::Estado { papel, .. }) => Some(papel),
                _ => None,
            })
    });
    match papel {
        Some(p) => println!("papel={}", p.nome()),
        None => println!("papel=?"),
    }
    let r = pedir(
        &mut s,
        &Pedido::Revelar {
            entrada: "acme-pg-dev".into(),
            alvo: Alvo::Campo("senha".into()),
        },
    );
    println!(
        "revelar={}",
        r.as_deref().map(descrever).unwrap_or_default()
    );
    // Só o app de teste aberto pelo daemon passa daqui: executado à mão (agente), um
    // `travar` mexeria no daemon de quem testa.
    if papel == Some(Papel::App) {
        valores_do_app(&mut s);
    }
    println!("fim=1");
    let _ = std::io::stdout().flush();
    ExitCode::SUCCESS
}
