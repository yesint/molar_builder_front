"""Check the browser payment contract without a compute or payment server."""
import argparse
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os
import signal
import subprocess
import tempfile
import threading


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", default="chromium-browser")
    parser.add_argument("--page", choices=["payment", "membrane", "protein", "glycans", "viewer"], default="payment")
    parser.add_argument("--builder", default=os.environ.get("BILAYER_BUILDER"), help="Native builder for glycan browser checks")
    args = parser.parse_args()
    if args.page == "glycans" and not args.builder:
        parser.error("--page glycans needs --builder or BILAYER_BUILDER")
    done = threading.Event()
    reports = []

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *a, **kw):
            super().__init__(*a, directory=str(Path(__file__).resolve().parents[1]), **kw)

        def log_message(self, *args):
            pass

        def do_GET(self):
            if args.page in ("payment", "membrane", "protein", "glycans") and self.path == "/api/membrane/health":
                health = {"ready": True, "compute_timeout_seconds": 600}
                if args.page == "payment":
                    health.update(payment_mode="paypal-sandbox", amount={"currency_code": "USD", "value": "1.00"})
                data = json.dumps(health).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            super().do_GET()

        def do_POST(self):
            if args.page == "glycans" and self.path == "/api/glycan":
                with tempfile.TemporaryDirectory(prefix="glycan-browser-") as directory:
                    request, result = Path(directory) / "request.json", Path(directory) / "result.json"
                    request.write_bytes(self.rfile.read(int(self.headers["Content-Length"])))
                    run = subprocess.run([args.builder, "glycan", str(request), "--out", str(result)], capture_output=True, timeout=30)
                    self.send_response(422 if run.returncode else 200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": run.stderr.decode()}).encode() if run.returncode else result.read_bytes())
                return
            reports.append(self.rfile.read(min(int(self.headers.get("Content-Length", "0")), 10000)).decode())
            self.send_response(204)
            self.end_headers()
            done.set()

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="molar-front-tests-") as directory:
            with (Path(directory) / "browser.log").open("w+") as log:
                process = subprocess.Popen([
                    args.browser, "--headless", "--disable-gpu", "--no-first-run",
                    "--disable-background-networking", "--enable-logging=stderr",
                    f"--user-data-dir={directory}/profile", f"http://127.0.0.1:{server.server_port}/tests/{args.page}.html",
                ], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    if not done.wait(45) or not reports[-1].startswith("PASS:"):
                        log.seek(0)
                        raise RuntimeError(str(reports) + "\n" + log.read()[-5000:])
                    print(reports[-1])
                finally:
                    try:
                        os.killpg(process.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
