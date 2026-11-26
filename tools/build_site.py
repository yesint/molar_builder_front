"""Build a Pages artifact from an explicit list of public browser assets."""
from pathlib import Path
import shutil
import subprocess
import sys
import json

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ["index.html", "style.css", "app.js", "api.js", "config.js", "payment.js",
          "preview.js", "viewer.html", "viewer.js", "viewer.css",
          "vendor/molar_vis/molar_vis.js", "vendor/molar_vis/molar_vis_bg.wasm",
          "vendor/molar_vis/LICENSE", "vendor/molar_vis/source.json"]
ASSETS += ["builder-api.js", "builder-checkout.js", "membrane.html", "membrane.css", "membrane.js", "membrane-composition.js", "protein.js", "glycans.js", "load.js", "assembly.js",
           "membrane-assets/catalog.json"]
ASSETS += [lipid["image"] for lipid in json.loads((ROOT / "membrane-assets/catalog.json").read_text())["lipids"]]


def main():
    subprocess.run([sys.executable, str(ROOT / "build_viewer.py"), "--check"], check=True)
    destination = ROOT / "_site"
    if destination.exists():
        shutil.rmtree(destination)
    for name in ASSETS:
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    (destination / ".nojekyll").touch()
    print(f"Collected {len(ASSETS)} public assets.")


if __name__ == "__main__":
    main()
