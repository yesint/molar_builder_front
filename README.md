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

The step views live in `src/app/`. The viewer controller in `src/viewer.rs` owns the canvas, the molecular scene, and ligand edit events. Trunk generates the JavaScript needed to load the module and call browser APIs.

```sh
cargo install leptosfmt --locked
leptosfmt src
cargo fmt
leptosfmt --check src
cargo fmt --check
cargo clippy --locked --all-targets --target wasm32-unknown-unknown -- -D warnings
```

The molecular viewer is distributed under the Artistic License 2.0. [MolAR viewer](https://github.com/yesint/molar_vis)

The browser regression test submits a solvated POPC membrane and a dry methane molecule to a local Rust server. It checks leaflet counts, stage confirmation, ligand editing, final rendering, and ZIP downloads. With Playwright and Chromium installed, run `node tests/workflow.cjs`. Set `BUILDER_URL` to use a local server on another port. Set `REFERENCE_URL` to the running HTML/JS version to compare the membrane submission screen in both applications.

Set `WEBGPU=1` to run the browser checks with WebGPU. The test reads the rendered frame before and after a ligand visibility change, checks that selecting the ligands restores the image, and records GPU validation errors.
