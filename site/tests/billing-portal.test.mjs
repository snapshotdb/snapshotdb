import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createBillingPortal, portalUrl } from '../app/console/billing-portal.ts';

test('portal redirects must stay on HTTPS Dodo-owned hosts without embedded credentials',()=>{
  assert.equal(portalUrl('https://customer.dodopayments.com/session/test'),'https://customer.dodopayments.com/session/test');
  for(const bad of ['javascript:alert(1)','https://dodopayments.com.evil.test/a','https://evildodopayments.com/a','http://customer.dodopayments.com/a','https://user@customer.dodopayments.com/a','https://customer.dodopayments.com:8443/a'])assert.throws(()=>portalUrl(bad));
});

test('portal uses the saved customer, creates no email, and works without a paid entitlement',async t=>{
  t.mock.method(globalThis,'fetch',async(url,init)=>{
    const u=new URL(url);
    assert.equal(u.origin,'https://test.dodopayments.com');
    assert.equal(u.pathname,'/customers/cus_saved/customer-portal/session');
    assert.equal(u.searchParams.get('send_email'),'false');
    assert.equal(u.searchParams.get('return_url'),'https://snapshotdb.io/console?billing=return');
    assert.equal(init.method,'POST');
    assert.equal(init.headers.Authorization,'Bearer test-only-key');
    return Response.json({link:'https://customer.dodopayments.com/session/test'});
  });
  const oldKey=process.env.DODO_PAYMENTS_API_KEY,oldEnv=process.env.DODO_PAYMENTS_ENVIRONMENT;
  process.env.DODO_PAYMENTS_API_KEY='test-only-key';process.env.DODO_PAYMENTS_ENVIRONMENT='test_mode';
  try{
    assert.equal(await createBillingPortal('cus_saved','https://snapshotdb.io/console?billing=return'),'https://customer.dodopayments.com/session/test');
    for(const customer of ['',undefined,'cus_../../other'])await assert.rejects(createBillingPortal(customer,'https://snapshotdb.io/console'),/customer/);
    assert.equal(globalThis.fetch.mock.callCount(),1);
  }finally{
    if(oldKey===undefined)delete process.env.DODO_PAYMENTS_API_KEY;else process.env.DODO_PAYMENTS_API_KEY=oldKey;
    if(oldEnv===undefined)delete process.env.DODO_PAYMENTS_ENVIRONMENT;else process.env.DODO_PAYMENTS_ENVIRONMENT=oldEnv;
  }
});
