// Texts of the printed emergency kit (FigCraft 16773, Phase 49; V7.10, D17).
//
// The sheet says where the vault, the local copies and the USB drive are, and leaves
// blank lines for the master password written by hand. No secret enters here: the
// folders are the fixed ones of the installation (D17, D18), and from the daemon comes
// only whether the external backup is on (and, when there is one, the UUID and the
// label of the USB drive, which are not secret). Nothing here keeps a typed value, and
// nothing goes to storage, the URL or a file.

// Folders of the installation (D17, D18): the same ones the local backup section of the
// settings screen shows.
export const VAULT_DIR = '~/.local/share/ward/';
export const BACKUPS_DIR = '~/.local/share/ward/backups/';
// Folder of the copies inside the USB drive (D17: `<mount point>/ward-backup/`).
export const USB_DRIVE_DIR = 'ward-backup/';

export const RESTORE_STEPS = Object.freeze([
  'Instale o Ward e crie ou destrave um cofre.',
  `Se esta máquina parou, copie um arquivo ward-AAAAMMDD-HHMMSS.ward do pendrive para ${BACKUPS_DIR}.`,
  'No app, abra Configurações › Backup local › Restaurar e escolha a versão.',
  'Digite a senha mestra que valia naquela versão.',
]);

// File system UUID in the format of D45 (`[0-9A-Fa-f-]`, 1 to 36, no `-` at the ends).
// The Rust side already checks it; here it is only so nothing else is ever printed as a
// UUID.
const UUID = /^[0-9A-Fa-f](?:[0-9A-Fa-f-]{0,34}[0-9A-Fa-f])?$/;
// USB drive label: the Rust side already neutralized it (D51) and cut it; here it is
// only text, and any control character left turns into a space.
const MAX_LABEL = 64;
const CONTROL_CHARS = /[\u0000-\u001f\u007f-\u009f]/g;

/** "Printed on 08/11/2026" (in pt-BR), in the local date of `now`. */
export function printedDate(now) {
  const d = now instanceof Date && !Number.isNaN(now.getTime()) ? now : new Date();
  const two = (n) => String(n).padStart(2, '0');
  return `Impresso em ${two(d.getDate())}/${two(d.getMonth() + 1)}/${d.getFullYear()}`;
}

function cleanLabel(r) {
  if (typeof r !== 'string') return '';
  return r.replace(CONTROL_CHARS, ' ').trim().slice(0, MAX_LABEL);
}

/**
 * The USB drive item of the backups section. `external` is `{ enabled, uuid?,
 * usbDrive?: { label } }` (only `enabled` when the app does not read the UUID yet).
 */
export function usbDriveLine(external) {
  if (external?.enabled !== true) {
    return {
      label: 'Pendrive · backup externo desligado',
      value: 'Nenhuma cópia em pendrive. Ligue em Configurações › Backup externo em pendrive.',
      mono: false,
    };
  }
  const parts = [];
  const name = cleanLabel(external.usbDrive?.label);
  if (name) parts.push(name);
  const uuid = typeof external.uuid === 'string' && UUID.test(external.uuid) ? external.uuid : '';
  if (uuid) parts.push(`UUID ${uuid}`);
  parts.push(`pasta ${USB_DRIVE_DIR}`);
  return { label: 'Pendrive · copiado sempre que é conectado', value: parts.join(' · '), mono: true };
}
