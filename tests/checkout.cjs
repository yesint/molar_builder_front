// Intercept the payment service so this test can run against a free local backend.
const assert = require('node:assert/strict');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const origin = process.env.BUILDER_URL || 'http://127.0.0.1:8003';
const amount = {currency_code: 'USD', value: '1.25'};
const response = value => ({contentType: 'application/json', body: JSON.stringify(value)});

async function checkout(context) {
    const errors = [];
    let buildRequests = 0;
    let paymentChecks = 0;
    let paid = false;
    await context.route('**/api/health', route => route.fulfill(response({mode: 'sandbox', amount})));
    await context.route('**/api/checkout', route => route.fulfill(response({
        order_id: 'TESTORDER12345', amount,
        url: 'https://www.sandbox.paypal.com/checkoutnow?token=TESTORDER12345',
    })));
    await context.route('https://www.sandbox.paypal.com/**', route =>
        route.fulfill({contentType: 'text/html', body: '<title>Test approval</title>'}));
    await context.route('**/api/payment-status', route => {
        paymentChecks++;
        return route.fulfill(response({paid}));
    });
    await context.route('**/api/build', route => {
        buildRequests++;
        return route.fulfill({...response({error: 'Test build reached the server.'}), status: 422});
    });

    const page = await context.newPage();
    page.on('pageerror', error => errors.push(error.message));
    await page.goto(origin + '/#proteins');
    await page.locator('#page-load:visible').waitFor();
    assert.equal(new URL(page.url()).hash, '#load');
    await page.locator('[data-page=membrane]').click();
    await page.waitForFunction(() => document.activeElement.id === 'membrane-heading');
    await page.getByRole('button', {name: 'Help for Membrane'}).click();
    assert.match(await page.locator('#stage-help-text').innerText(), /unselect "Add membrane"/);
    await page.keyboard.press('Escape');
    await page.locator('#open-about').click();
    await page.locator('#about-builder[open]').waitFor();
    assert.equal(await page.locator('#public-beta-terms').textContent(),
        await page.locator('#submission-fee').textContent());
    assert.equal(await page.locator('#public-beta-terms [data-build-price]').textContent(), 'USD 1.25');
    await page.getByRole('button', {name: 'Close', exact: true}).click();
    await page.locator('#add-lipid').click();
    await page.locator('#lipid-search').fill('POPC');
    await page.locator('[data-lipid=POPC]').click();
    await page.locator('#POPC-inner').fill('1');
    await page.locator('#POPC-outer').fill('1');
    await page.locator('#build').click();
    await page.locator('#continue-solvent').click();
    assert.equal(await page.locator('#assemble-system').innerText(), 'Submit and pay USD 1.25 with PayPal');

    const opened = page.waitForEvent('popup');
    await page.locator('#assemble-system').click();
    const popup = await opened;
    await popup.waitForURL('https://www.sandbox.paypal.com/**');
    await page.waitForResponse('**/api/payment-status');
    await popup.goto(origin + '/?checkout=cancel');
    await page.locator('#assembly-error:visible').waitFor();
    assert.match(await page.locator('#assembly-error').innerText(), /cancelled checkout/);
    assert.equal(buildRequests, 0);
    assert.ok(paymentChecks > 0);
    paid = true;
    await page.locator('#assemble-system:enabled').click();
    await page.waitForFunction(() => document.querySelector('#assembly-error').textContent === 'Test build reached the server.');
    assert.equal(await page.locator('#protein-error').textContent(), '');
    assert.equal(buildRequests, 1);
    assert.deepEqual(errors, []);
}
(async () => {
    const browser = await chromium.launch({executablePath: process.env.CHROMIUM_EXECUTABLE});
    try {
        await checkout(await browser.newContext());
        console.log('Passed navigation, stage help, checkout terms, sandbox pricing, PayPal cancellation and paid build handoff.');
    } finally {
        await browser.close();
    }
})().catch(error => {console.error(error); process.exitCode = 1;});
