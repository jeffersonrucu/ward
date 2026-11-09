// Textos do "Restaurar backup" (Fase 47, V7.7). As versões chegam do lado Rust só com o
// nome do arquivo e o instante (segundos UTC, tirados do nome; D17, Fase 45). A data vai
// para o fuso local aqui. Nada daqui guarda valor digitado.

const DIA_MS = 86_400_000;

function doisDigitos(n) {
  return String(n).padStart(2, '0');
}

/** O instante (segundos UTC) como `Date`, ou `null` se não for um número inteiro. */
function comoData(instante) {
  const s = Number(instante);
  if (!Number.isSafeInteger(s)) return null;
  return new Date(s * 1000);
}

/** "08/11/2026", no fuso local. */
export function diaDaVersao(instante) {
  const d = comoData(instante);
  if (!d) return '';
  return `${doisDigitos(d.getDate())}/${doisDigitos(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** "08/11/2026 às 14:32", no fuso local. */
export function dataDaVersao(instante) {
  const d = comoData(instante);
  if (!d) return '';
  return `${diaDaVersao(instante)} às ${doisDigitos(d.getHours())}:${doisDigitos(d.getMinutes())}`;
}

/** "agora", "há 2 min", "hoje", "ontem", "há 3 dias", pelos dias do calendário local. */
export function quandoFoi(instante, agora = new Date()) {
  const d = comoData(instante);
  if (!d) return '';
  const minutos = Math.floor((agora.getTime() - d.getTime()) / 60_000);
  if (minutos < 1) return 'agora';
  if (minutos < 60) return `há ${minutos} min`;
  const meiaNoite = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const dias = Math.round((meiaNoite(agora) - meiaNoite(d)) / DIA_MS);
  if (dias <= 0) return 'hoje';
  if (dias === 1) return 'ontem';
  return `há ${dias} dias`;
}

/**
 * Linha de baixo do item (Fase 49a). O protocolo traz o nome de cada versão do backup
 * local e duas marcas do daemon (sem tamanho, origem nem geração). A data vem do nome
 * do arquivo, que qualquer programa seu pode escolher; o daemon confere a ordem real
 * das gravações e marca a versão que é mais antiga do que a data diz.
 */
export function detalheDaVersao(v) {
  if (v?.conferida !== true) return 'Cópia local · de outra senha mestra';
  if (v?.fora_de_ordem === true) {
    return 'Cópia local · gravação mais antiga do que a data diz';
  }
  return 'Cópia local';
}

/** Legenda da lista: quantas versões e como a rotação funciona (D17). */
export function legendaDasVersoes(quantas) {
  const n = Number(quantas) || 0;
  const total = n === 1 ? '1 versão' : `${n} versões`;
  return `${total}, da mais nova para a mais antiga. Cada mudança no cofre gera uma versão; a mais antiga sai quando entra uma nova.`;
}

/** Aviso do que acontece ao restaurar (D17, D39). */
export function avisoDaTroca(instante) {
  return (
    'Antes da troca, o cofre atual é guardado como backup. Depois, valem a senha mestra e o ' +
    `PIN desta versão, e as entradas criadas depois de ${dataDaVersao(instante)} ficam só no ` +
    'backup. O tempo de trava e o backup externo continuam os de agora. A versão volta como ' +
    'uma gravação nova do cofre.'
  );
}

/** Toast depois de restaurar. */
export function mensagemRestaurado(instante, entradas) {
  const n = Number(entradas) || 0;
  const quantas = n === 1 ? '1 entrada' : `${n} entradas`;
  return `Versão de ${dataDaVersao(instante)} restaurada. O cofre tem ${quantas}.`;
}
