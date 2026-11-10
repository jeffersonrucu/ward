// Local rules of the Settings screen (Phase 38). The PIN and master password rules are
// the first-use ones (first-use screen module); the settings module of the Rust side
// (`app/core`) checks everything again before asking the daemon. Nothing here keeps a
// typed value.

// Mirror of the minimum and maximum lock minutes of the `ward_core` limits (D45).
export const MIN_LOCK_MINUTES = 1;
export const MAX_LOCK_MINUTES = 1440;

// The design's select (Settings / General, FigCraft 15313) says "at most 4 h": the
// options go up to 4 hours, the D30 default.
export const LOCK_OPTIONS = Object.freeze([15, 30, 60, 120, 240]);

/** Label of a number of minutes as the select shows it (minutes, hours, or "1 h 30 min"). */
export function minutesLabel(minutes) {
  const m = Number(minutes);
  if (m < 60) return `${m} ${m === 1 ? 'minuto' : 'minutos'}`;
  const h = Math.floor(m / 60);
  const rest = m % 60;
  if (rest) return `${h} h ${rest} min`;
  return `${h} ${h === 1 ? 'hora' : 'horas'}`;
}

/**
 * Options of the lock select. A stored value outside the list (another client, or an
 * old vault) shows up too, so the select does not lie about what is stored.
 */
export function lockOptions(current) {
  const list = [...LOCK_OPTIONS];
  const m = Number(current);
  if (Number.isInteger(m) && m >= MIN_LOCK_MINUTES && m <= MAX_LOCK_MINUTES && !list.includes(m)) {
    list.push(m);
    list.sort((a, b) => a - b);
  }
  return list.map((v) => ({ value: String(v), label: minutesLabel(v) }));
}

// External backup (Phase 48, V7.9). The USB drive label comes from udisks2 (untrusted):
// the Rust side already turned the invisible characters into escapes (D51); here it is
// only text.

/** USB drive name for the screen: the label, or a fixed text when there is no label. */
export function usbDriveName(p) {
  const r = typeof p?.rotulo === 'string' ? p.rotulo : '';
  return r || 'Pendrive sem rótulo';
}

/** "mounted" / "connected, but not mounted", for the list of the choice. */
export function situationInList(p) {
  return p?.montado ? 'montado' : 'conectado, mas não montado';
}

/**
 * Texts of the card of the chosen USB drive: name, situation (after the UUID), the
 * status line and whether it is mounted (the copy only goes to a mounted drive; Ward
 * does not mount it).
 */
export function usbDriveCard(external) {
  const p = external?.pendrive ?? null;
  if (p?.montado) {
    return {
      name: usbDriveName(p),
      situation: 'conectado e montado',
      status: 'O Ward copia as versões que faltam para ward-backup/, até 30.',
      mounted: true,
    };
  }
  if (p) {
    return {
      name: usbDriveName(p),
      situation: 'conectado, mas não montado',
      status: 'Abra o pendrive no gerenciador de arquivos para montá-lo e receber a cópia.',
      mounted: false,
    };
  }
  if (external && external.pendrives_visiveis === false) {
    return {
      name: 'Pendrive escolhido',
      situation: 'não deu para ver os pendrives agora',
      status: 'O udisks não respondeu. A cópia volta quando ele responder.',
      mounted: false,
    };
  }
  return {
    name: 'Pendrive escolhido',
    situation: 'não conectado',
    status: 'A cópia acontece quando ele for conectado e montado.',
    mounted: false,
  };
}

/**
 * Phase 49a: the local backup folder does not open or is not safe (it has to be a
 * directory of the user only, 0700). The daemon writes the vault anyway, but with no new
 * version.
 */
export const LOCAL_BACKUP_WARNING =
  'A pasta de backups não é segura (tem de ser só sua, sem acesso para outros usuários), ' +
  'então as mudanças estão sendo gravadas sem backup. Confira as permissões de ' +
  '~/.local/share/ward/backups/ (chmod 700).';
