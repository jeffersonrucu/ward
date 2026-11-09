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

/** Os avisos que o lado Rust manda pelo canal (D65 a), um por janela. */
export const AVISOS = Object.freeze(['pedido-de-destrave', 'pedido-de-aprovacao', 'estado-mudou']);

/**
 * Avisos do lado Rust (D65 a): `aoChegar()` é chamado quando chega o `aviso` — na
 * janela principal, `pedido-de-destrave` (Fase 31); na de aprovação,
 * `pedido-de-aprovacao` (Fase 36); na principal também `estado-mudou` (Fase 39: o cofre
 * travou pela bandeja, sozinho ou pela CLI). O aviso não traz dado; quem recebe consulta. No app,
 * pelo canal do Tauri (`ouvir_avisos`, um por janela); na ponte de teste, que não tem
 * canal, por consulta periódica. Devolve a função que para de ouvir.
 * @param {() => Promise<void> | void} aoChegar
 * @param {string | string[]} [aviso]
 * @returns {Promise<() => void>}
 */
export async function ouvirAvisos(aoChegar, aviso = ['pedido-de-destrave', 'estado-mudou']) {
  const avisos = Array.isArray(aviso) ? aviso : [aviso];
  if (avisos.length === 0 || !avisos.every((a) => AVISOS.includes(a))) {
    throw falha('aviso-desconhecido', 'aviso desconhecido');
  }
  if (import.meta.env.MODE === 'teste' && !('__TAURI_INTERNALS__' in window)) {
    const { ouvirPorConsulta } = await import('./ponte-teste.js');
    return ouvirPorConsulta(aoChegar);
  }
  let ativo = true;
  const canal = new Channel();
  canal.onmessage = (m) => {
    if (ativo && m && avisos.includes(m.aviso)) aoChegar();
  };
  try {
    await invoke('ouvir_avisos', { canal });
  } catch {
    // Sem o canal, a tela ainda vê o pedido ao consultar de novo.
  }
  return () => {
    ativo = false;
  };
}
