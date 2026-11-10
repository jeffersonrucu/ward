// The 9 entry types (scope, the ready-made types section): label of the type badge
// and icon.
// The key is the protocol identifier (the entry type name in `ward-core`), the same one
// the daemon sends in the list and in the detail.
export const TYPES = Object.freeze({
  'login': { label: 'login', icon: 'type-login' },
  'banco': { label: 'banco', icon: 'type-database' },
  'servidor': { label: 'servidor', icon: 'type-server' },
  'api': { label: 'api', icon: 'type-api' },
  'aws': { label: 'aws', icon: 'type-aws' },
  'smtp': { label: 'smtp', icon: 'type-smtp' },
  'ftp-sftp': { label: 'ftp/sftp', icon: 'type-ftp' },
  'nota-segura': { label: 'nota segura', icon: 'type-note' },
  'generico': { label: 'genérico', icon: 'type-generic' },
});

// Unknown type (daemon newer than the interface): shows as generic.
export function entryType(id) {
  return Object.hasOwn(TYPES, id) ? TYPES[id] : TYPES['generico'];
}
