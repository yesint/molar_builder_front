// Run against a local Rust server. Playwright is a test dependency only.
const assert = require('node:assert/strict');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');

const origin = process.env.BUILDER_URL || 'http://127.0.0.1:8003';
const methane = `Methane
  MolAR Builder

  5  4  0  0  0  0            999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    0.6290    0.6290    0.6290 H   0  0  0  0  0  0  0  0  0  0  0  0
   -0.6290   -0.6290    0.6290 H   0  0  0  0  0  0  0  0  0  0  0  0
   -0.6290    0.6290   -0.6290 H   0  0  0  0  0  0  0  0  0  0  0  0
    0.6290   -0.6290   -0.6290 H   0  0  0  0  0  0  0  0  0  0  0  0
  1  2  1  0
  1  3  1  0
  1  4  1  0
  1  5  1  0
M  END
$$$$
`;

async function visit(page, base) {
    await page.goto(base);
    await page.waitForFunction(() => !document.querySelector('#add-lipid').disabled);
}

async function saveResult(page) {
    await page.locator('#assemble-system').click();
    await page.locator('#assembly-download:visible').waitFor({timeout: 1200000});
    await page.locator('#assembly-viewer[data-state="rendered"]').waitFor();
    assert.equal(await page.locator('#assembly-error').isVisible(), false);
    const download = page.waitForEvent('download');
    await page.locator('#assembly-download').click();
    const archive = await download;
    assert.equal(archive.suggestedFilename(), 'assembly.zip');
    assert.equal(await archive.failure(), null);
    await page.locator('#assembly-unsaved-warning').waitFor({state: 'hidden'});
}

async function membrane(page, base) {
    await visit(page, base);
    assert.equal(await page.locator('[data-page=proteins]').getAttribute('href'), null);
    await page.locator('[data-page=submit]').click();
    assert.equal(await page.locator('#assemble-system').isDisabled(), true);
    await page.locator('[data-page=membrane]').click();
    assert.equal(await page.locator('#build').isDisabled(), true);
    await page.locator('#add-lipid').click();
    await page.locator('#lipid-search').fill('POPC');
    await page.locator('[data-lipid=POPC]').click();
    await page.locator('#POPC-inner').fill('1');
    await page.locator('#POPC-outer').fill('1');
    await page.locator('#build:enabled').waitFor();
    assert.equal(await page.locator('#POPC-inner-count').textContent(), '(46)');
    assert.equal(await page.locator('#outer-pie').getAttribute('data-total'), '46');
    await page.locator('#per-membrane').check();
    assert.equal(await page.locator('#POPC-inner-normalized').textContent(), '50%');
    await page.locator('#per-leaflet').check();
    await page.locator('#build').click();
    assert.equal(await page.locator('#waters-per-lipid').inputValue(), '50');
    assert.equal(await page.locator('#water-clearance').inputValue(), '1');
    await page.locator('#continue-solvent').click();
    assert.match(await page.locator('#submit-summary').innerText(), /Build membrane \(POPC\)/);
    assert.equal(await page.locator('#assemble-system').isEnabled(), true);
}

async function ligand(page) {
    await visit(page, origin);
    await page.locator('#ligand-files').setInputFiles({
        name: 'methane.sdf', mimeType: 'chemical/x-mdl-sdfile', buffer: Buffer.from(methane),
    });
    await page.locator('#load-viewer[data-state=rendered]:visible').waitFor();
    await page.getByLabel('mol 1', {exact: true}).uncheck();
    await page.getByLabel('mol 1', {exact: true}).check();
    await page.locator('#partition-system').click();
    await page.locator('#ligand-viewer[data-state=rendered]:visible').waitFor();
    await page.locator('#edit-ligand').click();
    await page.getByRole('button', {name: 'Done', exact: true}).click();
    await page.locator('#parameterize-ligand').click();
    await page.locator('#add-membrane').uncheck();
    await page.locator('#continue-no-membrane').click();
    await page.locator('#add-solvent').uncheck();
    await page.locator('#continue-solvent').click();
    await saveResult(page);
    await page.locator('[data-page=ligands]').click();
    const download = page.waitForEvent('download');
    await page.locator('#ligand-download').click();
    assert.equal(await (await download).failure(), null);
}

(async () => {
    const health = await fetch(new URL('/api/health', origin)).then(r => r.json());
    assert.equal(health.mode, 'local', 'This test submits builds and requires free local mode.');
    const browser = await chromium.launch({
        executablePath: process.env.CHROMIUM_EXECUTABLE,
        args: ['--disable-webgpu', '--disable-blink-features=WebGPU', '--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--disable-gpu-watchdog'],
    });
    try {
        const page = await browser.newPage({viewport: {width: 1440, height: 940}});
        page.setDefaultTimeout(60000);
        const errors = [];
        page.on('pageerror', error => errors.push(error.message));
        await membrane(page, origin);
        if (process.env.REFERENCE_URL) {
            const original = await browser.newPage({viewport: page.viewportSize()});
            await membrane(original, process.env.REFERENCE_URL);
            assert.deepEqual(await page.screenshot(), await original.screenshot());
            await original.close();
        }
        await saveResult(page);
        await ligand(page);
        assert.deepEqual(errors, []);
        console.log('Passed membrane and ligand builds, viewer controls, and archive downloads.');
    } finally {
        await browser.close();
    }
})().catch(error => {console.error(error); process.exitCode = 1;});
