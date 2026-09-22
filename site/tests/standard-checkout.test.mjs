import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { validPaymentSignature, checkoutPrice, providerError } from '../app/console/standard-checkout.ts';

test('payment signature binds order and payment; rejects tampering and malformed signatures',()=>{
  const secret='test-only-secret';
  const signature=crypto.createHmac('sha256',secret).update('order_123|pay_456').digest('hex');
  assert.ok(validPaymentSignature('order_123','pay_456',signature,secret));
  assert.equal(validPaymentSignature('order_other','pay_456',signature,secret),false);
  assert.equal(validPaymentSignature('order_123','pay_other',signature,secret),false);
  for(const invalid of ['', 'a', 'g'.repeat(64)])assert.equal(validPaymentSignature('order_123','pay_456',invalid,secret),false);
  assert.equal(validPaymentSignature('order_123','pay_456',signature,''),false);
});
test('server owns the test price and rejects invalid amounts and live keys',()=>{
  const keys=['RAZORPAY_KEY_ID','RAZORPAY_CHECKOUT_AMOUNT','RAZORPAY_CHECKOUT_CURRENCY'];
  const previous=keys.map(k=>process.env[k]);
  try{
    process.env.RAZORPAY_KEY_ID='rzp_test_example';process.env.RAZORPAY_CHECKOUT_CURRENCY='INR';
    process.env.RAZORPAY_CHECKOUT_AMOUNT='100';assert.deepEqual(checkoutPrice(),{amount:100,currency:'INR'});
    for(const amount of ['99','-1','1.5','NaN']){process.env.RAZORPAY_CHECKOUT_AMOUNT=amount;assert.throws(checkoutPrice);}
    process.env.RAZORPAY_CHECKOUT_AMOUNT='100';process.env.RAZORPAY_KEY_ID='rzp_live_example';assert.throws(checkoutPrice,/test-mode/);
  }finally{keys.forEach((k,i)=>{if(previous[i]===undefined)delete process.env[k];else process.env[k]=previous[i];});}
});
test('provider errors distinguish authentication without exposing provider details',async()=>{
  assert.equal(providerError({statusCode:401}).status,401);
  const response=providerError({statusCode:400,error:{description:'private detail'}});
  assert.equal(response.status,500);assert.doesNotMatch(await response.text(),/private detail/);
});
