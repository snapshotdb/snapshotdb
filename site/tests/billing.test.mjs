import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { paymentEntitlement, validWebhook } from '../app/console/razorpay.ts';
import { tenantFor, sameOrigin } from '../app/console/hosted.ts';

const event = () => ({ event:'subscription.charged', created_at:1700000000,
  payload:{subscription:{entity:{id:'sub_123',plan_id:'plan_pro',quantity:1,notes:{tenant:'github-42'},customer_id:'cust_123',current_start:1700000000,current_end:1702592000}},
    payment:{entity:{status:'captured',currency:'USD',amount:15000}}} });

test('only captured full-price subscription payments grant Pro',()=>{
  assert.equal(paymentEntitlement(event(),'event1','plan_pro').tenant,'github-42');
  for(const name of ['subscription.authenticated','subscription.activated','subscription.pending','subscription.cancelled']) {
    const e=event();e.event=name;assert.equal(paymentEntitlement(e,'event1','plan_pro'),null);
  }
  const e=event();e.payload.payment.entity.amount=100;
  assert.equal(paymentEntitlement(e,'event1','plan_pro'),null);
  assert.equal(paymentEntitlement(event(),'event1','plan_other'),null);
});
test('webhook signature binds the raw body and rejects tampering',()=>{
  const body=JSON.stringify(event());const secret='test-webhook-secret';
  const signature=crypto.createHmac('sha256',secret).update(body).digest('hex');
  assert.ok(validWebhook(body,signature,secret));
  assert.equal(validWebhook(body+' ',signature,secret),false);
  assert.equal(validWebhook(body,'z'.repeat(64),secret),false);
  assert.equal(validWebhook(body,signature,''),false);
});
test('hosted identity survives GitHub username changes; old sessions must sign in again',()=>{
  assert.equal(tenantFor({id:'42',login:'old-name'}),tenantFor({id:'42',login:'new-name'}));
  assert.throws(()=>tenantFor({login:'old-session'}),/Sign out/);
});
test('billing mutations require the website origin',()=>{
  assert.ok(sameOrigin(new Request('https://www.snapshotdb.io/api/billing/checkout',{headers:{origin:'https://www.snapshotdb.io'}})));
  assert.equal(sameOrigin(new Request('https://www.snapshotdb.io/api/billing/checkout',{headers:{origin:'https://evil.example'}})),false);
  assert.equal(sameOrigin(new Request('https://www.snapshotdb.io/api/billing/checkout')),false);
});
