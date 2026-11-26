"""Build the browser viewer from the pinned molar_vis commit."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


BUNDLE = Path(__file__).resolve().parent / "vendor/molar_vis"
ASSETS = ("molar_vis.js", "molar_vis_bg.wasm", "molar_vis.d.ts")


def run(*args, cwd=None):
    subprocess.run(args, cwd=cwd, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkout", type=Path, help="Read the pinned commit from a local Git checkout")
    parser.add_argument("--check", action="store_true", help="Verify bundled file hashes without building")
    args = parser.parse_args()
    manifest = BUNDLE / "source.json"
    source = json.loads(manifest.read_text())
    if args.check:
        for name in (*ASSETS, "LICENSE"):
            actual = hashlib.sha256((BUNDLE / name).read_bytes()).hexdigest()
            if actual != source["sha256"][name]:
                raise SystemExit(f"Bundle hash mismatch: {name}")
        print(f"Verified molar_vis bundle from {source['commit']}")
        return

    with tempfile.TemporaryDirectory(prefix="param-gen-viewer-build-") as temporary:
        root = Path(temporary)
        checkout = root / "source"
        if args.checkout:
            run("git", "clone", "--shared", "--no-checkout", str(args.checkout.resolve()), str(checkout))
            run("git", "checkout", "--detach", source["commit"], cwd=checkout)
        else:
            run("git", "init", str(checkout))
            run("git", "fetch", "--depth=1", source["repository"], source["commit"], cwd=checkout)
            run("git", "checkout", "--detach", "FETCH_HEAD", cwd=checkout)
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
        if revision != source["commit"]:
            raise SystemExit("The checkout is not at the pinned molar_vis commit.")
        package = root / "package"
        run("wasm-pack", "build", "crates/molar_vis_js", "--target", "web", "--release",
            *([] if source.get("wasm_opt", True) else ["--no-opt"]),
            "--out-dir", str(package), "--", "--locked", "--target-dir", str(root / "target"), cwd=checkout)
        for name in ASSETS:
            shutil.copyfile(package / name, BUNDLE / name)
        shutil.copyfile(checkout / "LICENSE", BUNDLE / "LICENSE")
        source["sha256"] = {
            name: hashlib.sha256((BUNDLE / name).read_bytes()).hexdigest()
            for name in (*ASSETS, "LICENSE")
        }
        manifest.write_text(json.dumps(source, indent=2) + "\n")
        print(f"Built molar_vis bundle from {revision}")


if __name__ == "__main__":
    main()
