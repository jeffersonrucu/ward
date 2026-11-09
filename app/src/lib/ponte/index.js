// Única porta da interface para o lado Rust. No app, a IPC do Tauri; no Chrome em modo
// de teste (D7), a ponte HTTP do `ward-ponte-teste`. Nenhuma resposta traz segredo
// (o "copiar" é resolvido no Rust, D4), e nada daqui guarda resposta em estado global,
// no armazenamento do navegador ou na URL.
import { Channel, invoke } from '@tauri-apps/api/core';
import { COMANDOS } from './comandos.js';

let transporte = null;

async function obterTransporte() {
  if (transporte) return transporte;
  // `import.meta.env.MODE` é trocado no build: fora do modo de teste este ramo some do
  // pacote e o código da ponte nem entra em `dist/` (conferido por `make app-check`).
  if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
    const { criarTransporte } = await import('./ponte-teste.js');
    transporte = criarTransporte();
  } else {
    transporte = (comando, args) => invoke(comando, args);
  }
  return transporte;
}

/** Erro de comando: `{ codigo, mensagem }`, sem valor de entrada. */
function falha(codigo, mensagem) {
  return Object.freeze({ codigo, mensagem });
}

export async function chamar(comando, args = {}) {
  if (!COMANDOS.includes(comando)) {
    throw falha('comando-desconhecido', 'comando desconhecido');
  }
  const t = await obterTransporte();
  try {
    return await t(comando, args);
  } catch (e) {
    if (e && typeof e.codigo === 'string' && typeof e.mensagem === 'string') {
      throw falha(e.codigo, e.mensagem);
    }
    throw falha('falha', 'não foi possível falar com o app');
  }
}

/**
 * Avisos do lado Rust (Fase 31, D65 a): `aoChegar()` é chamado quando chega um
 * `pedido_de_destrave` (o aviso não traz dado; quem recebe consulta o estado). No app,
 * pelo canal do Tauri (`ouvir_avisos`); na ponte de teste, que não tem canal, por
 * consulta periódica. Devolve a função que para de ouvir.
 * @param {() => Promise<void> | void} aoChegar
 * @returns {Promise<() => void>}
 */
export async function ouvirAvisos(aoChegar) {
  if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
    const { ouvirPorConsulta } = await import('./ponte-teste.js');
    return ouvirPorConsulta(aoChegar);
  }
  let ativo = true;
  const canal = new Channel();
  canal.onmessage = (m) => {
    if (ativo && m && m.aviso === 'pedido-de-destrave') aoChegar();
  };
  try {
    await invoke('ouvir_avisos', { canal });
  } catch {
    // Sem o canal, a tela ainda vê o pedido ao consultar o estado.
  }
  return () => {
    ativo = false;
  };
}
