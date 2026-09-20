import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { encodeSession, verifySession } from '../app/console/session.ts';

process.env.AUTH_SECRET = 'test-only-session-secret-at-least-32-characters';
const user = { login: 'audit-user', provider: 'github' };
const sign = (value) => {
  const payload = Buffer.from(JSON.stringify(value)).toString('base64url');
  return payload + '.' + crypto.createHmac('sha256', process.env.AUTH_SECRET).update(payload).digest('base64url');
};
test('session round trip and tampering rejection', () => {
  const token = encodeSession(user);
  assert.equal(verifySession(token)?.login, user.login);
  assert.equal(verifySession('x' + token), null);
  assert.equal(verifySession(token.split('.')[0] + '.' + 'é'.repeat(43)), null);
  assert.equal(verifySession(undefined), null);
});
test('expired, legacy, and malformed signed identities are rejected', () => {
  const exp = Math.floor(Date.now() / 1000) + 60;
  for (const value of [user, { ...user, exp: 1 }, { ...user, exp, login: 1 }, { ...user, exp, provider: 'other' }, null]) {
    assert.equal(verifySession(sign(value)), null);
  }
});
test('missing production secret fails closed', () => {
  const secret = process.env.AUTH_SECRET;
  const token = encodeSession(user);
  delete process.env.AUTH_SECRET;
  assert.equal(verifySession(token), null);
  assert.throws(() => encodeSession(user), /AUTH_SECRET/);
  process.env.AUTH_SECRET = secret;
});
