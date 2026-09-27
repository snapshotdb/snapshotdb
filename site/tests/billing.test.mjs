import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { checkoutUrl, subscriptionEntitlement, validWebhook } from '../app/console/dodo.ts';
import { tenantFor, sameOrigin } from '../app/console/hosted.ts';

const event = (type='subscription.active', data={}) => ({ business_id:'bus_1', type, timestamp:'2026-09-23T10:00:00Z',
  data:{ payload_type:'Subscription', subscription_id:'sub_123', product_id:'pdt_pro', status:'active', quantity:1, currency:'USD',
    recurring_pre_tax_amount:15000, cancel_at_next_billing_date:false, metadata:{tenant:'github-42'}, customer:{customer_id:'cus_1'},
    previous_billing_date:'2026-09-23T10:00:00Z', next_billing_date:'2026-10-23T10:00:00Z', ...data } });

test('activation and renewal at full price grant the charged period',()=>{
  for(const type of ['subscription.active','subscription.renewed']){
    const e=subscriptionEntitlement(event(type),'msg_1','pdt_pro');
    assert.equal(e.tenant,'github-42');
    assert.deepEqual(e.body,{event_id:'msg_1',event_time:1790157600,event_time_ms:1790157600000,customer:'cus_1',subscription:'sub_123',period_start:1790157600,period_end:1792749600,paid:true});
  }
});
test('events that do not pay never grant Pro',()=>{
  for(const type of ['subscription.on_hold','subscription.plan_changed','subscription.updated','payment.succeeded'])assert.equal(subscriptionEntitlement(event(type),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event('subscription.active',{recurring_pre_tax_amount:100}),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event('subscription.active',{currency:'INR'}),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event('subscription.active',{quantity:2}),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event('subscription.active',{status:'pending'}),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event('subscription.active',{recurring_pre_tax_amount:undefined}),'m','pdt_pro'),null);
  assert.equal(subscriptionEntitlement(event(),'m','pdt_other'),null);
});
test('immediate cancellation, failure and expiry end access; scheduled cancellation does not',()=>{
  for(const type of ['subscription.failed','subscription.expired','subscription.cancelled'])assert.equal(subscriptionEntitlement(event(type,{status:'cancelled'}),'m','pdt_pro').body.paid,false);
  // Seen from Dodo on a declined first payment: no usable billing period, still must end access.
  assert.deepEqual(subscriptionEntitlement(event('subscription.failed',{status:'failed',next_billing_date:'2026-09-23T10:00:00Z'}),'m','pdt_pro').body,
    {event_id:'m',event_time:1790157600,event_time_ms:1790157600000,customer:'cus_1',subscription:'sub_123',paid:false});
  assert.equal(subscriptionEntitlement(event('subscription.cancelled',{cancel_at_next_billing_date:true}),'m','pdt_pro'),null);
});
test('event order retains milliseconds and checkout correlation comes from signed metadata',()=>{
  const e=event('subscription.active',{metadata:{tenant:'github-42',checkout_nonce:'a'.repeat(32)}});
  e.timestamp='2026-09-23T10:00:00.123Z';
  const first=subscriptionEntitlement(e,'first','pdt_pro');
  e.timestamp='2026-09-23T10:00:00.987Z';
  const second=subscriptionEntitlement(e,'second','pdt_pro');
  assert.equal(first.body.event_time_ms,1790157600123);
  assert.equal(second.body.event_time_ms,1790157600987);
  assert.equal(first.body.checkout_nonce,'a'.repeat(32));
  assert.equal(subscriptionEntitlement(event(),'legacy','pdt_pro').body.checkout_nonce,undefined);
});
test('a paid event without a trustworthy tenant or period is rejected, not granted',()=>{
  assert.throws(()=>subscriptionEntitlement(event('subscription.active',{metadata:{}}),'m','pdt_pro'),/identity/);
  assert.throws(()=>subscriptionEntitlement(event('subscription.active',{metadata:{tenant:'github-../x'}}),'m','pdt_pro'),/identity/);
  assert.throws(()=>subscriptionEntitlement(event('subscription.active',{next_billing_date:'2026-09-01T00:00:00Z'}),'m','pdt_pro'),/period/);
});

const secret='whsec_'+Buffer.from('dodo-test-webhook-key-0123456789').toString('base64');
const sign=(id,ts,body,key=secret)=>'v1,'+crypto.createHmac('sha256',Buffer.from(key.slice(6),'base64')).update(`${id}.${ts}.${body}`).digest('base64');
const headers=(id,ts,sig)=>new Headers({'webhook-id':id,'webhook-timestamp':String(ts),'webhook-signature':sig});
test('Standard Webhooks signature binds id, timestamp and raw body',()=>{
  const body=JSON.stringify(event());const now=1790157600;
  assert.ok(validWebhook(body,headers('msg_1',now,sign('msg_1',now,body)),secret,now));
  assert.ok(validWebhook(body,headers('msg_1',now,'v1,bm9wZQ== '+sign('msg_1',now,body)),secret,now),'any listed signature may match');
  assert.equal(validWebhook(body+' ',headers('msg_1',now,sign('msg_1',now,body)),secret,now),false);
  assert.equal(validWebhook(body,headers('msg_2',now,sign('msg_1',now,body)),secret,now),false,'id is signed');
  assert.equal(validWebhook(body,headers('msg_1',now,sign('msg_1',now,body)),secret,now+301),false,'replays go stale');
  assert.equal(validWebhook(body,headers('msg_1',now,sign('msg_1',now,body,'whsec_'+Buffer.from('other').toString('base64'))),secret,now),false);
  assert.equal(validWebhook(body,headers('msg_1',now,sign('msg_1',now,body)),'',now),false);
});
test('only Dodo checkout pages are handed to the browser',()=>{
  assert.equal(checkoutUrl('https://test.checkout.dodopayments.com/session/cks_1'),'https://test.checkout.dodopayments.com/session/cks_1');
  for(const bad of ['http://checkout.dodopayments.com/x','https://checkout.dodopayments.com.evil.example/x','https://evil.example/x'])assert.throws(()=>checkoutUrl(bad));
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
