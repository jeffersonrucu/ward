//! `wardd-aprovador-teste`: janela de aprovação de teste das suítes de ponta a ponta do M6
//! (Fase 44c; D8, D9, D63). Só existe com a feature `test-mode` (`required-features`),
//! nunca no release.
//!
//! O daemon de teste a abre pelo `abrir_app` quando um pedido precisa da janela (com
//! `WARD_DEV_APP` apontando para uma cópia dela dentro do repositório): ela recebe o
//! papel app (D63), conecta ao socket de teste (`$WARD_DEV_SOCKET_DIR/wardd.sock`, ou
//! `/run/user/<uid>/ward-teste/wardd.sock`) e responde cada `pedido_de_aprovacao` com o
//! que a suíte escreveu em `$WARD_DEV_DIR/aprovador.modo` (lido a cada pedido):
//! - `confirmar`, `negar`, `uma_vez <PIN>` (o PIN do cofre de teste),
//!   `uma_vez_ciente <PIN>` (Fase 54b: o mesmo com a caixa da entrada padrão marcada), ou
//! - `nada` (não responde: o pedido expira, ou o cliente desiste).
//!
//! Imprime, uma linha por evento, só o que a suíte confere (nenhum valor do cofre: o
//! pedido de aprovação não tem nenhum): `papel=<papel>`,
//! `pedido operacao=<op> entradas=<a,b> origem=<origem> confirmacao=<0|1> decisao=<d>`
//! (com ` entrada_oculta=1` antes do `decisao` quando a entrada padrão é oculta) e
//! `resposta=<tipo ou código>`. A saída vai para `$WARD_DEV_DIR/logs/<binário>.log`
//! (o lançador do daemon de teste). Executada à mão, ela recebe o papel agente, o daemon
//! recusa o `responder_aprovacao` e ela sai.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use ward_core::entrada::Segredo;
use ward_core::protocolo::{self, Decisao, Papel, Pedido, Resposta};

fn socket() -> Option<PathBuf> {
    match std::env::var_os("WARD_DEV_SOCKET_DIR") {
        Some(d) => Some(PathBuf::from(d).join("wardd.sock")),
        None => Some(PathBuf::from(format!(
            "/run/user/{}/ward-teste/wardd.sock",
            ward_platform::uid_efetivo()
        ))),
    }
}

fn mandar(s: &mut UnixStream, p: &Pedido) -> bool {
    let Ok(corpo) = protocolo::codificar_pedido(Papel::App, p) else {
        return false;
    };
    let Ok(n) = u32::try_from(corpo.len()) else {
        return false;
    };
    s.write_all(&n.to_be_bytes())
        .and_then(|()| s.write_all(&corpo))
        .is_ok()
}

fn ler(s: &mut UnixStream) -> Option<Vec<u8>> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).ok()?;
    let tamanho = usize::try_from(u32::from_be_bytes(n)).ok()?;
    let mut corpo = vec![0u8; tamanho.min(1 << 20)];
    s.read_exact(&mut corpo).ok()?;
    Some(corpo)
}

/// A decisão do arquivo de modo (a suíte troca entre um caso e outro): decisão, PIN e a
/// caixa da entrada padrão (Fase 54b).
fn modo() -> Option<(Decisao, Option<Segredo>, bool)> {
    let dev = PathBuf::from(std::env::var_os("WARD_DEV_DIR")?);
    let texto = std::fs::read_to_string(dev.join("aprovador.modo")).ok()?;
    let linha = texto.lines().next()?.trim();
    match linha.split_once(' ') {
        Some((m @ ("uma_vez" | "uma_vez_ciente"), pin)) => Some((
            Decisao::PermitirUmaVez,
            Some(Segredo::de_bytes(pin.trim().as_bytes().to_vec())),
            m == "uma_vez_ciente",
        )),
        _ => match linha {
            "confirmar" => Some((Decisao::Confirmar, None, false)),
            "negar" => Some((Decisao::Negar, None, false)),
            _ => None,
        },
    }
}

fn main() -> ExitCode {
    let Some(caminho) = socket() else {
        return ExitCode::FAILURE;
    };
    let Ok(mut s) = UnixStream::connect(&caminho) else {
        println!("erro=conectar");
        return ExitCode::FAILURE;
    };
    if !mandar(&mut s, &Pedido::Estado) {
        return ExitCode::FAILURE;
    }
    while let Some(corpo) = ler(&mut s) {
        match protocolo::decodificar_resposta(Papel::App, &corpo) {
            Ok(Resposta::Estado { papel, .. }) => println!("papel={}", papel.nome()),
            Ok(Resposta::PedidoDeAprovacao(p)) => {
                let escolha = modo();
                println!(
                    "pedido operacao={} entradas={} origem={} confirmacao={}{} decisao={}",
                    p.operacao.nome(),
                    p.entradas.join(","),
                    if p.origem.is_empty() { "-" } else { &p.origem },
                    u8::from(p.confirmacao),
                    if p.entrada_oculta {
                        " entrada_oculta=1"
                    } else {
                        ""
                    },
                    escolha.as_ref().map_or("nada", |(d, _, _)| d.nome()),
                );
                if let Some((decisao, pin, entrada_confirmada)) = escolha {
                    let pedido = Pedido::ResponderAprovacao {
                        pedido: p.id,
                        decisao,
                        pin,
                        entrada_confirmada,
                    };
                    if !mandar(&mut s, &pedido) {
                        break;
                    }
                }
            }
            Ok(Resposta::PedidoDeDestrave(_)) => println!("destrave"),
            Ok(Resposta::Erro(e)) => println!("resposta={}", e.codigo()),
            Ok(r) => println!("resposta={}", r.tipo().nome()),
            Err(_) => println!("resposta=invalida"),
        }
        let _ = std::io::stdout().flush();
    }
    println!("fim=1");
    ExitCode::SUCCESS
}
