// Texts of the restore backup dialog (Phase 47, V7.7). The versions come from the Rust
// side with only the file name and the instant (UTC seconds, taken from the name; D17,
// Phase 45). The date goes to the local time zone here. Nothing here keeps a typed
// value.

const DAY_MS = 86_400_000;

function twoDigits(n) {
  return String(n).padStart(2, '0');
}

/** The instant (UTC seconds) as a `Date`, or `null` if it is not an integer. */
function asDate(instant) {
  const s = Number(instant);
  if (!Number.isSafeInteger(s)) return null;
  return new Date(s * 1000);
}

/** "08/11/2026", in the local time zone. */
export function versionDay(instant) {
  const d = asDate(instant);
  if (!d) return '';
  return `${twoDigits(d.getDate())}/${twoDigits(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** "08/11/2026 at 14:32" (in pt-BR), in the local time zone. */
export function versionDate(instant) {
  const d = asDate(instant);
  if (!d) return '';
  return `${versionDay(instant)} às ${twoDigits(d.getHours())}:${twoDigits(d.getMinutes())}`;
}

/** "now", "2 min ago", "today", "yesterday", "3 days ago" (in pt-BR), by the days of
 * the local calendar. */
export function howLongAgo(instant, now = new Date()) {
  const d = asDate(instant);
  if (!d) return '';
  const minutes = Math.floor((now.getTime() - d.getTime()) / 60_000);
  if (minutes < 1) return 'agora';
  if (minutes < 60) return `há ${minutes} min`;
  const midnight = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((midnight(now) - midnight(d)) / DAY_MS);
  if (days <= 0) return 'hoje';
  if (days === 1) return 'ontem';
  return `há ${days} dias`;
}

/**
 * Bottom line of the item (Phase 49a). The protocol brings the name of each version of
 * the local backup and two marks from the daemon (no size, origin or generation). The
 * date comes from the file name, which any program of yours can choose; the daemon
 * checks the real order of the writes and marks the version that is older than the date
 * says.
 */
export function versionDetail(v) {
  if (v?.conferida !== true) return 'Cópia local · de outra senha mestra';
  if (v?.fora_de_ordem === true) {
    return 'Cópia local · gravação mais antiga do que a data diz';
  }
  return 'Cópia local';
}

/** Caption of the list: how many versions and how the rotation works (D17). */
export function versionsCaption(count) {
  const n = Number(count) || 0;
  const total = n === 1 ? '1 versão' : `${n} versões`;
  return `${total}, da mais nova para a mais antiga. Cada mudança no cofre gera uma versão; a mais antiga sai quando entra uma nova.`;
}

/** Warning of what happens on restoring (D17, D39). */
export function swapWarning(instant) {
  return (
    'Antes da troca, o cofre atual é guardado como backup. Depois, valem a senha mestra e o ' +
    `PIN desta versão, e as entradas criadas depois de ${versionDate(instant)} ficam só no ` +
    'backup. O tempo de trava e o backup externo continuam os de agora. A versão volta como ' +
    'uma gravação nova do cofre.'
  );
}

/** Toast after restoring. */
export function restoredMessage(instant, entries) {
  const n = Number(entries) || 0;
  const count = n === 1 ? '1 entrada' : `${n} entradas`;
  return `Versão de ${versionDate(instant)} restaurada. O cofre tem ${count}.`;
}
