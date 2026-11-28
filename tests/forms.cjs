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
    assert.equal(await panel.locator('li span').innerText(), 'mol 1');
    const saved = panel.getByRole('textbox', {name: 'records.sdf, mol 1, saved ligand selection 1'});
    await saved.fill('  name C  ');
    await saved.press('Enter');
    assert.equal(await saved.inputValue(), 'name C');
    await panel.locator('li button').click();
    assert.equal(await input.evaluate(el => el === document.activeElement), true);
    await page.locator('#partition-system').click();
    await page.locator('#page-ligands').waitFor({state:'visible'});
    const filter = page.locator('#ligand-resname-search');
    await filter.fill('missing');
    assert.equal(await page.locator('#prepared-ligands button').count(), 0);
    await filter.fill('mol 2');
    await page.locator('#prepared-ligands button').click();
    await page.locator('#ligand-viewer[data-state=rendered]').waitFor();
    await page.locator('#edit-ligand').click();
    await page.getByRole('button',{name:'Done',exact:true}).click();
    await page.locator('#parameterize-ligand').click();
    await page.locator('#page-membrane:visible').waitFor();
    assert.equal(await page.locator('#ligand-ready').isVisible(),true);
    await page.locator('[data-page=load]').click();
    await page.getByRole('button',{name:'Remove records.sdf'}).click();
    await page.locator('#load-options').waitFor({state:'hidden'});
    assert.equal(await page.locator('#load-ready').isVisible(), false);
    assert.equal(await page.locator('[data-page=ligands]').getAttribute('href'), null);
}
async function protein(page) {
    const structure = [
        'ATOM      1  N   THR A   1       0.000   0.000   0.000  1.00  0.00           N',
        'ATOM      2  CA  THR A   1       1.450   0.000   0.000  1.00  0.00           C',
        'ATOM      3  C   THR A   1       1.900   1.410   0.000  1.00  0.00           C',
        'ATOM      4  O   THR A   1       1.290   2.380   0.000  1.00  0.00           O',
        'ATOM      5  CB  THR A   1       1.910  -0.750   1.250  1.00  0.00           C',
        'ATOM      6  OG1 THR A   1       3.300  -0.750   1.250  1.00  0.00           O',
        'ATOM      7  CG2 THR A   1       1.190  -2.030   1.500  1.00  0.00           C',
        'HETATM    8  O   HOH W   2       7.000   3.000   5.000  1.00  0.00           O',
        'HETATM    9  H1  HOH W   2       7.957   3.000   5.000  1.00  0.00           H',
        'HETATM   10  H2  HOH W   2       6.760   3.927   5.000  1.00  0.00           H',
        'HETATM   11  O   HOH W   3      10.000   3.000   5.000  1.00  0.00           O',
        'HETATM   12  H1  HOH W   3      10.957   3.000   5.000  1.00  0.00           H',
        'HETATM   13  H2  HOH W   3       9.760   3.927   5.000  1.00  0.00           H',
        'END',
    ].join('\n');
    await page.locator('#structure-file').setInputFiles({name:'site.pdb',mimeType:'chemical/x-pdb',buffer:Buffer.from(structure)});
    await page.locator('#ligand-residues-tab').click();
    await page.locator('#load-ligands').getByLabel(/HOH, water/).check();
    await page.locator('#partition-system').click();
    await page.locator('#page-proteins:visible').waitFor();
    await page.locator('#add-mutation').click();
    await page.locator('#mutation-target').selectOption('LYS');
    await page.locator('#mutation-form button[type=submit]').click();
    await page.locator('#add-ptm').click();
    await page.locator('#ptm-type').selectOption('lysine_myristoylation');
    await page.locator('#ptm-add').click();
    assert.match(await page.locator('#protein-ptms').innerText(), /LYS A:1, Lys myristoylation/);
    await page.locator('#protein-mutations button').click();
    assert.match(await page.locator('#protein-error').innerText(), /Remove the incompatible PTM/);
    assert.match(await page.locator('#protein-mutations').innerText(), /→ LYS/);
    await page.locator('#protein-ptms button').click();
    await page.locator('#protein-mutations button').click();
    assert.equal(await page.locator('#protein-mutations button').count(), 0);
    await page.locator('#add-mutation').click();
    await page.locator('#mutation-target').selectOption('SER');
    await page.locator('#mutation-form button[type=submit]').click();
    assert.match(await page.locator('#protein-mutations').innerText(), /THR A:1 → SER/);
    await page.locator('#add-ptm').click();
    await page.locator('#ptm-type').selectOption('glycosylation');
    await page.locator('#ptm-search').fill('no matching residue');
    assert.equal(await page.locator('#ptm-add').isEnabled(),false);
    await page.locator('#ptm-search').fill('SER A:1');
    await page.locator('#ptm-add').click();
    await page.locator('.glycan-item .glycan-buttons button').first().click();
    await page.locator('#glycan-apply:enabled').waitFor();
    await page.locator('#glycan-iupac').fill('Gal(b1-4)GlcNAc');
    await page.waitForFunction(()=>document.querySelectorAll('#glycan-tree [data-node-index]').length===2);
    await page.locator('#glycan-undo').click();
    await page.waitForFunction(()=>document.querySelectorAll('#glycan-tree [data-node-index]').length===1);
    await page.locator('#glycan-add').click();
    await page.waitForFunction(()=>document.querySelectorAll('#glycan-tree [data-node-index]').length===2);
    await page.locator('#glycan-apply').click();
    const sequence = 'Man(b1-6)Man(b1-4)[Man(b1-6)]Ido(b1-3)[Man(b1-4)Man(b1-4)][Man(b1-3)Man(b1-6)]GlcNAc(b1-4)GlcNAc';
    await page.locator('.glycan-item').getByText('Edit…').click();
    await page.locator('#glycan-apply:enabled').waitFor();
    await page.locator('#glycan-iupac').fill(sequence);
    await page.waitForFunction(() => document.querySelectorAll('#glycan-tree [data-node-index]').length === 10);
    await page.locator('#glycan-apply').click();
    const row = page.locator('.glycan-item');
    assert.equal(await row.locator('code').textContent(), sequence);
    const viewport = page.viewportSize();
    for (const width of [1280, 390]) {
        await page.setViewportSize({width, height: 920});
        assert.equal(await row.evaluate(el => {
            const code = el.querySelector('code');
            return getComputedStyle(code).textOverflow === 'ellipsis' &&
                code.scrollWidth > code.clientWidth && code.scrollHeight === code.clientHeight &&
                [...el.querySelectorAll('button')].every(b => b.getBoundingClientRect().right <= el.getBoundingClientRect().right + 1);
        }), true);
    }
    await page.setViewportSize(viewport);
    await page.locator('#orient-protein').uncheck();
    await page.locator('#build-protein').click();
    await page.locator('#page-ligands:visible').waitFor();
    assert.equal(await page.locator('#ligand-viewer').isVisible(), false);
    await page.locator('#parameterize-ligand').click();
    await page.locator('#page-membrane:visible').waitFor();
    assert.match(await page.locator('#submit-summary').textContent(), /Parameterize ligands \(HOH ×2\)/);
    assert.equal(await page.locator('#protein-ready').isVisible(),true);
}
async function disulfides(page) {
    await page.goto(base);
    const records = [];
    for (const [residue, offset] of [[1, 0], [2, 2.04]]) {
        for (const [name, x, y] of [['N', -2.1, 2.2], ['CA', -1.3, 1.5],
            ['C', -1.1, 2.9], ['O', -0.3, 3.5], ['CB', -0.5, 1], ['SG', 0, 0]]) {
            const index = String(records.length + 1).padStart(5);
            const xyz = [x + offset, y, 0].map(n => n.toFixed(3).padStart(8)).join('');
            records.push(`ATOM  ${index}  ${name.padEnd(3)} CYS A${String(residue).padStart(4)}    ${xyz}  1.00  0.00           ${name[0]}`);
        }
    }
    await page.locator('#structure-file').setInputFiles({
        name: 'cystine.pdb', mimeType: 'chemical/x-pdb', buffer: Buffer.from(records.concat('END').join('\n')),
    });
    await page.locator('#partition-system').click();
    const bond = page.locator('#protein-ss input');
    assert.equal(await bond.isChecked(), true);
    await bond.uncheck();
    await page.locator('#add-ss').click();
    const first = page.locator('#ss-first');
    const second = page.locator('#ss-second');
    await first.selectOption({index: 1});
    await second.selectOption({index: 1});
    await page.locator('#ss-form button[type=submit]').click();
    assert.match(await page.locator('#ss-error').innerText(), /two different cysteines/);
    await second.selectOption({index: 0});
    await page.locator('#ss-form button[type=submit]').click();
    assert.equal(await bond.isChecked(), true);
    await bond.uncheck();
    await page.locator('#add-ptm').click();
    await page.locator('#ptm-type').selectOption('palmitoylation');
    await page.locator('#ptm-add').click();
    assert.equal(await bond.isDisabled(), true);
    await page.locator('#protein-ptms button').click();
    assert.equal(await bond.isEnabled(), true);
}
async function membrane(page) {
    await page.locator('#page-membrane:visible').waitFor();
    await page.locator('#add-lipid').click();
    await page.locator('#lipid-search').fill('no-such-lipid');
    assert.equal(await page.locator('#no-lipids').isVisible(), true);
    await page.locator('#lipid-search').fill('POPC');
    await page.locator('#picker-lipids [data-lipid=POPC]').click();
    await page.locator('#POPC-inner').fill('1');
    await page.locator('#POPC-outer').fill('1');
    await page.locator('#build:enabled').waitFor();
    assert.equal(await page.locator('#POPC-inner-count').textContent(), '(46)');
    assert.equal(await page.locator('#outer-pie').getAttribute('data-total'), '46');
    await page.locator('#per-membrane').check();
    assert.equal(await page.locator('#POPC-inner-normalized').textContent(), '50%');
    await page.locator('#cell-size').fill('3.01');
    assert.equal(await page.locator('#build').isEnabled(),false);
    await page.locator('#cell-size').fill('6');
    await page.locator('#add-membrane').uncheck();
    await page.locator('#no-membrane-actions:visible').waitFor();
    await page.locator('#add-membrane').check();
    assert.equal(await page.locator('#POPC-inner').inputValue(), '1');
    await page.locator('#build').click();
    await page.locator('#page-solvent:visible').waitFor();
}
async function solvent(page) {
    const proceed = page.locator('#continue-solvent');
    const concentration = page.locator('#salt-concentration');
    await concentration.fill('-0.1');
    await proceed.click();
    assert.equal(await page.locator('#page-solvent').isVisible(),true);
    await concentration.fill('0.23');
    await page.locator('#assembly-salt').selectOption('KCl');
    await page.locator('#neutralize-system').uncheck();
    await page.locator('#add-solvent').uncheck();
    await page.locator('#solvent-parameters').waitFor({state:'hidden'});
    await page.locator('#add-solvent').check();
    assert.equal(await concentration.inputValue(),'0.23');
    await proceed.click();
    await page.locator('#page-submit:visible').waitFor();
    assert.match(await page.locator('#submit-summary').innerText(),/0.23 mol.L KCl/);
    await page.locator('[data-page=solvent]').click();
    await page.locator('#add-solvent').uncheck();
    await proceed.click();
    assert.doesNotMatch(await page.locator('#submit-summary').innerText(),/Add solvent and ions/);
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
        await protein(page);
        await membrane(page);
        await solvent(page);
        await disulfides(page);
        assert.deepEqual(errors,[]);
        console.log('Passed Components, Protein, Ligands, Membrane and Solvent forms and their Submit summary.');
    } finally {await browser.close();}
})().catch(err=>{console.error(err);process.exitCode=1;});
