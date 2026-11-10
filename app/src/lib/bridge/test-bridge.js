// Transporte da ponte de teste (D7). Só entra no build `--mode teste` (dist-teste/).
//
// O `ward-test-bridge` serve esta interface e abre o Chrome em
// `127.0.0.1:<porta>/#token=<aleatório>` (mesma origem). O token fica só nesta variável de módulo
// (nunca no armazenamento do navegador, em cookie ou na URL depois de lido) e vai no cabeçalho
// `Authorization` de cada chamada.

let token = null;
let tela = null;

function lerToken() {
  if (token !== null) return token;
  // `&tela=<nome>` (opcional, só no modo de teste): abre direto numa tela para o QA no
  // Chrome, antes de as telas de destravar existirem. Sai da URL junto com o token.
  const m = /^#token=([0-9a-f]{64})(?:&tela=([a-z-]{1,32}))?$/.exec(window.location.hash);
  token = m ? m[1] : '';
  tela = m && m[2] ? m[2] : null;
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

/** Tela pedida na URL de abertura (`&tela=gerador`), ou `null`. Só no modo de teste. */
export function requestedScreen() {
  lerToken();
  return tela;
}

/**
 * A ponte não tem o canal de avisos do Tauri: consulta a cada 2 s, uma consulta por vez
 * (a anterior precisa terminar), e para quando a tela sai. Só no modo de teste.
 * @param {() => Promise<void> | void} aoChegar
 */
export function ouvirPorConsulta(aoChegar) {
  let ocupado = false;
  const id = setInterval(async () => {
    if (ocupado) return;
    ocupado = true;
    try {
      await aoChegar();
    } finally {
      ocupado = false;
    }
  }, 2000);
  return () => clearInterval(id);
}
