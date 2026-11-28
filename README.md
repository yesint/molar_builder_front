# MolAR Builder for the browser

This branch contains the Leptos frontend for MolAR Builder. It lets you select proteins and ligands, edit ligand geometry, configure a lipid bilayer and solvent, and download a GROMACS system. The molecular viewer runs in the same WebAssembly module as the application. Leptos calls the Rust viewer API directly.

## Build and run

Install the Rust WebAssembly target and Trunk, then build the browser assets.

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
trunk build --release --locked
```

The build writes the HTML, style sheet, WebAssembly module, generated browser loader and lipid catalogue to `dist/`. The Rust backend can serve this directory. For interactive frontend development, start the local Rust backend on port 8003, then run the following command in this checkout.

```sh
trunk serve --port 8080
```

The deployed frontend uses the payment mode and price reported by the backend. Local builds run without payment. The sandbox and production services use their respective PayPal checkouts.

## Source and checks

Each of the six pages is a Leptos component in `src/app/`. The pages own their input values, validation, dialogs, and event handlers. The application keeps them mounted during navigation so that an unfinished edit survives a visit to another page.

| Page | Component responsibilities |
| --- | --- |
| Components | Read structure and ligand files, track selections per model, update the preview, and partition the selected molecules. |
| Protein | Validate mutations, PTMs, disulfide bonds, and orientation. The glycan editor owns its graph, edit history, and IUPAC input. |
| Ligands | Filter the partitioned ligands, edit the selected molecule, save its geometry, and download its parameters. |
| Membrane | Validate composition, estimate leaflet counts, and use the same estimates for the table and both charts. |
| Solvent | Manage hydration, salt, and neutralization settings, including drafts kept while solvent is disabled. |
| Submit | Review the plan, handle local and paid submissions, display build progress, and save the resulting archive. |

`src/app/workspace.rs` owns navigation and shared dialogs. The shared build plan is defined in `src/model.rs`. The viewer controller in `src/viewer.rs` updates one molecular scene and preserves the same canvas as the user moves between pages. Trunk generates the JavaScript needed to load the module and call browser APIs.

```sh
cargo install leptosfmt --locked
leptosfmt src
cargo fmt
leptosfmt --check src
cargo fmt --check
cargo clippy --locked --all-targets --target wasm32-unknown-unknown -- -D warnings
```

To request the same checks on GitHub, push the Leptos commit with a fresh tag whose name starts with `leptos-check-`. For example, run `git push origin HEAD:refs/tags/leptos-check-1` from the Leptos checkout.

The molecular viewer is distributed under the Artistic License 2.0. [MolAR viewer](https://github.com/yesint/molar_vis)

The browser regression test submits a solvated POPC membrane and a dry methane molecule to a local Rust server. It checks leaflet counts, stage confirmation, ligand editing, final rendering, and ZIP downloads. With Playwright and Chromium installed, run `node tests/workflow.cjs`. Set `BUILDER_URL` to use a local server on another port. Set `REFERENCE_URL` to the running HTML/JS version to compare the membrane submission screen in both applications.

Set `WEBGPU=1` to run the browser checks with WebGPU. The test reads the rendered frame before and after a ligand visibility change, checks that selecting the ligands restores the image, and records GPU validation errors.

Run `node tests/forms.cjs` to exercise the controls across all six pages. Run `node tests/checkout.cjs` to check stage help, checkout pricing, cancellation, and submission after payment confirmation with simulated PayPal endpoints. Run the browser suites one at a time when they share a local backend because the server limits concurrent operations.
