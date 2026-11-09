// Transporte da ponte de teste (D7). Só entra no build `--mode teste` (dist-teste/).
//
// O `ward-ponte-teste` serve esta interface e abre o Chrome em
// `127.0.0.1:<porta>/#token=<aleatório>` (mesma origem). O token fica só nesta variável de módulo
// (nunca no armazenamento do navegador, em cookie ou na URL depois de lido) e vai no cabeçalho
// `Authorization` de cada chamada.

let token = null;

function lerToken() {
  if (token !== null) return token;
  const m = /^#token=([0-9a-f]{64})$/.exec(window.location.hash);
  token = m ? m[1] : '';
  // Tira o token da barra de endereço e do histórico.
  window.history.replaceState(null, '', window.location.pathname);
  return token;
}

export function criarTransporte() {
  lerToken();
  return async (comando, args) => {
    let resposta;
    try {
      resposta = await fetch(`/ponte/${comando}`, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${token}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(args ?? {}),
        credentials: 'omit',
        cache: 'no-store',
        redirect: 'error',
        referrerPolicy: 'no-referrer',
      });
    } catch {
      throw { codigo: 'ponte-indisponivel', mensagem: 'a ponte de teste não respondeu' };
    }
    let corpo;
    try {
      corpo = await resposta.json();
    } catch {
      throw { codigo: 'ponte-invalida', mensagem: 'resposta inválida da ponte de teste' };
    }
    if (corpo && corpo.ok === true) return corpo.valor ?? null;
    throw {
      codigo: String(corpo?.codigo ?? 'falha'),
      mensagem: String(corpo?.mensagem ?? 'falha na ponte de teste'),
    };
  };
}
