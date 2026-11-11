// Phase 65b: unit test of the detail rules for the TOTP seed (app/src/screens/credentials.js),
// without a browser. The detail draws every secret but the seed, shows the code when the
// entry has a seed, and never asks to reveal or copy the seed (also against odd field
// values a hostile caller could hand it).
// Usage: node dev/tests/credentials_totp.mjs (from the repository root).

import assert from 'node:assert/strict';
import { TOTP_SEED, detailSecrets, hasTotp, detailField } from '../../app/src/screens/credentials.js';

const login = {
  name: 'acme-totp-dev',
  secrets: [
    { id: 'password', label: 'Senha' },
    { id: 'totp_seed', label: 'Semente TOTP' },
  ],
};
const database = { name: 'acme-pg-dev', secrets: [{ id: 'password', label: 'Senha' }] };

let n = 0;
function check(name, f) {
  f();
  n++;
  console.log(`ok   ${name}`);
}

check('the seed id is the protocol one', () => assert.equal(TOTP_SEED, 'totp_seed'));
check('detail of a login with TOTP draws only the password', () =>
  assert.deepEqual(detailSecrets(login).map((s) => s.id), ['password']));
check('login with TOTP shows the code', () => assert.equal(hasTotp(login), true));
check('entry without a seed: no code, secrets as they are', () => {
  assert.equal(hasTotp(database), false);
  assert.deepEqual(detailSecrets(database), database.secrets);
});
check('no detail (none selected or locked): nothing', () => {
  assert.deepEqual(detailSecrets(null), []);
  assert.equal(hasTotp(undefined), false);
  assert.deepEqual(detailSecrets({}), []);
});
check('detailSecrets does not change the detail (the form still sees the seed)', () => {
  detailSecrets(login);
  assert.equal(login.secrets.length, 2);
});
check('attack: the detail never asks to reveal or copy the seed', () => {
  assert.equal(detailField('totp_seed'), false);
  for (const f of ['', null, undefined, 42, {}, ['totp_seed'], { toString: () => 'totp_seed' }]) {
    assert.equal(detailField(f), false, `field ${String(f)}`);
  }
});
check('the other fields of the detail still go', () => {
  for (const f of ['password', 'username', 'url', 'EXTRA_TOKEN']) assert.equal(detailField(f), true);
});

console.log(`credentials_totp: ${n} ok`);
