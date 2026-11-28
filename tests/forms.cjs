// Exercise form state through the same controls a user operates.
const assert = require('node:assert/strict');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const base = process.env.BUILDER_URL || 'http://127.0.0.1:8003';
const methane = `Methane
  MolAR

  1  0  0  0  0  0            999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
M  END
$$$$
`;
async function components(page) {
    await page.locator('#ligand-files').setInputFiles({
        name: 'records.sdf', mimeType: 'chemical/x-mdl-sdfile', buffer: Buffer.from(methane.repeat(2)),
    });
    const list = page.locator('#load-ligands');
    const all = list.getByLabel('All', {exact: true});
    await list.getByLabel('mol 1', {exact: true}).uncheck();
    assert.equal(await all.evaluate(el => el.indeterminate), true);
    await all.check();
    await page.locator('#ligand-residues-tab').focus();
    await page.keyboard.press('End');
    const panel = page.locator('#ligand-custom-panel');
    await panel.waitFor({state:'visible'});
    assert.equal(await page.locator('#ligand-custom-tab').evaluate(el => el === document.activeElement), true);
    const input = panel.locator('.selection-entry input');
    const model = panel.locator('select');
    await input.fill('name C');
    await model.selectOption({index:1});
    assert.equal(await input.inputValue(), '');
    await model.selectOption({index:0});
    assert.equal(await input.inputValue(), 'name C');
    await input.press('Enter');
    await input.fill('name C');
    await input.press('Enter');
    assert.equal(await panel.locator('li').count(), 1);
    await panel.locator('li button').click();
    await page.locator('#partition-system').click();
    await page.locator('#page-ligands').waitFor({state:'visible'});
    await page.locator('[data-page=load]').click();
    await page.getByRole('button',{name:'Remove records.sdf'}).click();
    await page.locator('#load-options').waitFor({state:'hidden'});
    assert.equal(await page.locator('#load-ready').isVisible(), false);
    assert.equal(await page.locator('[data-page=ligands]').getAttribute('href'), null);
}
(async()=>{
    const browser=await chromium.launch({executablePath:process.env.CHROMIUM_EXECUTABLE, args:['--enable-unsafe-swiftshader','--use-angle=swiftshader']});
    try {
        const page=await browser.newPage();
        const errors=[];
        page.on('pageerror',e=>errors.push(e.message));
        await page.goto(base);
        await page.waitForFunction(()=>document.querySelector('#add-lipid:enabled'));
        await components(page);
        assert.deepEqual(errors,[]);
        console.log('Components form passed');
    } finally {await browser.close();}
})().catch(err=>{console.error(err);process.exitCode=1;});
