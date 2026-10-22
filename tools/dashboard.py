"""Loopback-only guest control and evidence dashboard; Python standard library, no web install.

The browser can submit only allowlisted guest commands or fixed host demo suites.
Host paths, shell commands and process arguments remain server-controlled. Run with Python 3.10+.
"""
import argparse
import importlib.util
from copy import deepcopy
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys
import threading
import time
from urllib.parse import urlsplit
import uuid
import webbrowser

ROOT = Path(__file__).resolve().parent.parent
_guest_spec = importlib.util.spec_from_file_location('mnemosyne_guest', ROOT / 'tools' / 'guest-controller.py')
_guest_module = importlib.util.module_from_spec(_guest_spec)
_guest_spec.loader.exec_module(_guest_module)
GuestController = _guest_module.GuestController

LOG_LIMIT = 131_072
STEP_TIMEOUT = 240
SUITES = {"full": ("boot", "test", "demo"), "host": ("test", "demo"),
          "boot": ("boot",), "test": ("test",), "demo": ("demo",)}
LABELS = {"boot": "Kernel boot", "test": "Capability & replay tests", "demo": "Checkpoint & replay"}
BOOT_EVENTS = {"boot", "memory", "frames_ok", "capabilities_ok", "paging_ok",
               "exception_ok", "timer_ok", "boot_ok"}
REPLAY_CHECKS = ("state_equal", "effects_equal", "missing_event_rejected",
                 "reordered_event_rejected", "trailing_event_rejected", "corrupted_checkpoint_rejected")


def now():
    return datetime.now(timezone.utc).isoformat()


def parse_evidence(kind, output):
    """Parse the current invocation's stdout, never a possibly stale build file."""
    if kind == "boot":
        events = []
        for line in output.splitlines():
            if line.startswith('{"event":'):
                event = json.loads(line)
                if isinstance(event, dict):
                    events.append(event)
        if not BOOT_EVENTS <= {e.get("event") for e in events}:
            raise ValueError("Kernel completion events are missing. Inspect the console.")
        if not any(e.get("event") == "paging_ok" and e.get("fault_probes") == 3 for e in events):
            raise ValueError("CPU protection probes did not complete.")
        return {"events": events}
    if kind == "test":
        summaries = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", output)
        cases, group = [], "Other"
        for line in output.splitlines():
            if "Running unittests" in line:
                group = ("Capabilities" if "mnemosyne_capability" in line else
                         "Filesystem" if "mnemosyne_chronicle_fs" in line else
                         "Recovery policy" if "mnemosyne_recovery_policy" in line else "Replay & runtime")
            match = re.match(r"test (\S+) \.\.\. (ok|FAILED|ignored)\s*$", line)
            if match:
                cases.append({"name": match[1].removeprefix("tests::"), "status": match[2], "group": group})
        if not summaries or not cases:
            raise ValueError("No complete test results were found. Inspect the console.")
        counts = [sum(int(s[i]) for s in summaries) for i in range(3)]
        return {"passed": counts[0], "failed": counts[1], "ignored": counts[2], "cases": cases}
    decoder = json.JSONDecoder()
    for match in re.finditer(r"^\{", output, re.MULTILINE):
        try:
            report, _ = decoder.raw_decode(output[match.start():])
        except json.JSONDecodeError:
            continue
        if isinstance(report, dict) and "replayed_requests" in report:
            if any(report.get(key) is not True for key in ("passed", *REPLAY_CHECKS)):
                raise ValueError("The replay report contains failed or missing checks.")
            for key in ("final_state_sha256", "replay_state_sha256", "checkpoint_sha256", "module_sha256"):
                if not re.fullmatch(r"[0-9a-f]{64}", str(report.get(key, ""))):
                    raise ValueError("Replay digest is missing or malformed.")
            if report["final_state_sha256"] != report["replay_state_sha256"]:
                raise ValueError("Replay state digests differ.")
            return report
    raise ValueError("No replay report was emitted by this run.")


def new_stage(kind):
    return {"kind": kind, "label": LABELS[kind], "status": "pending", "command": f"python tools/dev.py {kind}",
            "log": "", "log_truncated": False, "result": None, "error": None,
            "started_at": None, "finished_at": None, "duration_seconds": None, "exit_code": None}


def saved_evidence(root):
    folder = root / "presentation" / "evidence"
    try:
        manifest = json.loads((folder / "manifest.json").read_text(encoding="utf-8"))
        data = {}
        for name in ("boot.log", "tests.log", "replay-demo.json"):
            raw = (folder / name).read_bytes()
            # Git checkouts can use CRLF. The manifest explicitly hashes LF text.
            normalized = raw.replace(b"\r\n", b"\n").rstrip(b"\n") + b"\n"
            if hashlib.sha256(normalized).hexdigest() != manifest["files"][name]["sha256"]:
                raise ValueError(f"Saved evidence checksum mismatch: {name}")
            data[name] = normalized.decode("utf-8")
        stages = []
        for kind, file in (("boot", "boot.log"), ("test", "tests.log"), ("demo", "replay-demo.json")):
            stage = new_stage(kind)
            stage.update(status="passed", log=data[file], result=parse_evidence(kind, data[file]), exit_code=0)
            stages.append(stage)
        return {"id": "saved", "source": "recorded", "suite": "full", "status": "passed", "stages": stages,
                "started_at": manifest["captured_at_utc"], "finished_at": manifest["captured_at_utc"],
                "git_head": manifest["code_commit"], "dirty": False, "error": None, "stop_requested": False}
    except (OSError, KeyError, ValueError, TypeError) as error:
        return {"id": "saved", "source": "recorded", "status": "unavailable", "stages": [], "error": str(error)}


def terminate_tree(process):
    if process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                       capture_output=True, timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
    else:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if process.poll() is None:
        process.kill()


def execute(kind, append, root):
    environment = dict(os.environ, PYTHONUNBUFFERED="1", PYTHONIOENCODING="utf-8", CARGO_TERM_COLOR="never")
    options = {"creationflags": subprocess.CREATE_NO_WINDOW} if os.name == "nt" else {"start_new_session": True}
    with subprocess.Popen([sys.executable, "-u", "tools/dev.py", kind], cwd=root, env=environment,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                          encoding="utf-8", errors="replace", **options) as process:
        def read_output():
            for line in process.stdout:
                append(line)
        reader = threading.Thread(target=read_output, daemon=True)
        reader.start()
        try:
            code = process.wait(timeout=STEP_TIMEOUT)
        except subprocess.TimeoutExpired:
            terminate_tree(process)
            process.wait(timeout=10)
            raise RuntimeError(f"Step exceeded {STEP_TIMEOUT} seconds and was stopped. QEMU also has a 25-second guest limit.") from None
        finally:
            reader.join(timeout=3)
        return code


class RunManager:
    def __init__(self, root=ROOT, runner=execute):
        self.root, self.runner = Path(root), runner
        self.lock = threading.RLock()
        self.worker = None
        self.latest = None
        self.saved = saved_evidence(self.root)
        self.storage = self.root / "build" / "dashboard"
        self.token = secrets.token_urlsafe(32)
        self.persistence_error = None
        try:
            previous = json.loads((self.storage / "latest.json").read_text(encoding="utf-8"))
            valid_stages = isinstance(previous, dict) and isinstance(previous.get("stages"), list) and 1 <= len(previous["stages"]) <= 3 and all(
                isinstance(stage, dict) and stage.get("kind") in LABELS
                and stage.get("status") in {"pending", "running", "passed", "failed", "skipped", "interrupted"}
                for stage in previous["stages"])
            if (valid_stages and previous.get("schema") == 1
                    and previous.get("status") in {"running", "passed", "failed", "stopped", "interrupted"}
                    and re.fullmatch(r"[0-9a-f]{12}", str(previous.get("id", "")))):
                self.latest = previous
                if previous.get("status") == "running":
                    previous.update(status="interrupted", finished_at=now(), error="The runner stopped before this run finished. Start a new run.")
                    for stage in previous["stages"]:
                        if stage["status"] in ("pending", "running"):
                            stage.update(status="interrupted", result=None)
        except (OSError, ValueError, TypeError, KeyError):
            pass

    def snapshot(self):
        with self.lock:
            return deepcopy({"saved": self.saved, "latest": self.latest,
                             "busy": self.latest is not None and self.latest["status"] == "running",
                             "token": self.token, "persistence_error": self.persistence_error})

    def _persist(self):
        try:
            self.storage.mkdir(parents=True, exist_ok=True)
            temporary = self.storage / "latest.tmp"
            temporary.write_text(json.dumps(self.latest, indent=2) + "\n", encoding="utf-8")
            temporary.replace(self.storage / "latest.json")
            self.persistence_error = None
        except OSError as error:
            self.persistence_error = f"Could not save run record: {error}"

    def start(self, suite):
        if suite not in SUITES:
            raise ValueError("Unknown demo suite.")
        with self.lock:
            if self.latest and self.latest["status"] == "running":
                raise RuntimeError("A run is already active. Wait for it to finish.")
            try:
                commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.root, text=True, timeout=5, stderr=subprocess.DEVNULL).strip()
                dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=self.root, text=True, timeout=5, stderr=subprocess.DEVNULL).strip())
            except (OSError, subprocess.SubprocessError):
                commit, dirty = "unknown", None
            self.latest = {"schema": 1, "id": uuid.uuid4().hex[:12], "source": "live", "suite": suite,
                           "status": "running", "started_at": now(), "finished_at": None,
                           "git_head": commit, "dirty": dirty, "stages": [new_stage(k) for k in SUITES[suite]],
                           "stop_requested": False, "error": None}
            self._persist()
            self.worker = threading.Thread(target=self._work, name="demo-runner", daemon=False)
            self.worker.start()
            return self.latest["id"]

    def stop(self, run_id):
        with self.lock:
            if not self.latest or self.latest["id"] != run_id or self.latest["status"] != "running":
                raise RuntimeError("That run is no longer active.")
            self.latest["stop_requested"] = True

    def _work(self):
        try:
            for stage in self.latest["stages"]:
                with self.lock:
                    if self.latest["stop_requested"]:
                        break
                    stage.update(status="running", started_at=now())
                    self._persist()
                started = time.monotonic()

                def append(text):
                    with self.lock:
                        combined = stage["log"] + text
                        stage["log_truncated"] |= len(combined) > LOG_LIMIT
                        stage["log"] = combined[-LOG_LIMIT:]

                try:
                    code = self.runner(stage["kind"], append, self.root)
                    with self.lock:
                        stage["exit_code"] = code
                        # Failed commands never inherit old evidence or a passing status.
                        if code != 0:
                            raise RuntimeError(f"Command exited with status {code}. Inspect the console and retry.")
                        result = parse_evidence(stage["kind"], stage["log"])
                        if stage["kind"] == "test" and result["failed"]:
                            raise RuntimeError("The test output contains failures.")
                        stage.update(status="passed", result=result)
                except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
                    with self.lock:
                        stage.update(status="failed", error=str(error), result=None)
                        self.latest["error"] = str(error)
                finally:
                    with self.lock:
                        stage.update(finished_at=now(), duration_seconds=round(time.monotonic() - started, 3))
                        self._persist()
                if stage["status"] == "failed":
                    break
        except Exception as error:
            # Unexpected runner failures must become a terminal state, never an eternal spinner.
            with self.lock:
                self.latest["error"] = f"Runner error: {error}"
                for stage in self.latest["stages"]:
                    if stage["status"] == "running":
                        stage.update(status="failed", error=str(error), result=None)
        finally:
            with self.lock:
                stages = self.latest["stages"]
                for stage in stages:
                    if stage["status"] == "pending":
                        stage["status"] = "skipped"
                passed = all(s["status"] == "passed" for s in stages)
                self.latest.update(status="passed" if passed else "failed" if self.latest["error"] else "stopped", finished_at=now())
                self._persist()


ASSETS = {"/": ("ui/index.html", "text/html; charset=utf-8"),
          "/evidence.html": ("ui/evidence.html", "text/html; charset=utf-8"),
          "/guest-app.js": ("ui/guest-app.js", "text/javascript; charset=utf-8"),
          "/guest-style.css": ("ui/guest-style.css", "text/css; charset=utf-8"),
          "/app.js": ("ui/app.js", "text/javascript; charset=utf-8"),
          "/style.css": ("ui/style.css", "text/css; charset=utf-8"),
          "/favicon.svg": ("ui/favicon.svg", "image/svg+xml"),
          "/files/pitch.pdf": ("presentation/output/mnemosyne-os-pitch.pdf", "application/pdf"),
          "/files/pitch.pptx": ("presentation/output/mnemosyne-os-pitch.pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
          "/files/guide.md": ("presentation/demo-runbook.md", "text/plain; charset=utf-8")}


class DashboardHandler(BaseHTTPRequestHandler):
    server_version = "MnemosyneLocal/1"

    def setup(self):
        super().setup()
        self.connection.settimeout(10)

    def log_message(self, format, *args):
        # Routine one-second state polling need not fill the presenter's terminal.
        if args and str(args[1] if len(args) > 1 else "") not in ("200", "202"):
            super().log_message(format, *args)

    def send(self, status, data, content_type="application/json; charset=utf-8", attachment=None):
        payload = data if isinstance(data, bytes) else json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(payload)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
        if attachment:
            self.send_header("Content-Disposition", f'attachment; filename="{attachment}"')
        self.end_headers()
        try:
            self.wfile.write(payload)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def trusted_request(self, mutation=False):
        port = self.server.server_port
        allowed_hosts = {f"127.0.0.1:{port}", f"localhost:{port}"}
        host = self.headers.get("Host", "")
        if host not in allowed_hosts:
            self.send(403, {"error": "This dashboard accepts only its loopback host."})
            return False
        origin = self.headers.get("Origin")
        if (origin and origin != f"http://{host}") or self.headers.get("Sec-Fetch-Site") == "cross-site":
            self.send(403, {"error": "Cross-origin requests are not allowed."})
            return False
        if mutation:
            supplied = self.headers.get("X-Mnemosyne-Token", "")
            if origin != f"http://{host}" or not secrets.compare_digest(supplied.encode("utf-8"), self.server.manager.token.encode("utf-8")):
                self.send(403, {"error": "Refresh the dashboard before starting another run."})
                return False
        return True

    def do_GET(self):
        if not self.trusted_request():
            return
        path = urlsplit(self.path).path
        manager = self.server.manager
        if path in ("/api/guest", "/api/guest/export"):
            state = self.server.guest.snapshot()
            if not path.endswith("export"):
                state["token"] = manager.token
            self.send(200, state, attachment="mnemosyne-guest.json" if path.endswith("export") else None)
        elif path == "/api/state":
            self.send(200, manager.snapshot())
        elif path in ("/api/export/latest", "/api/export/saved"):
            source = path.rsplit("/", 1)[1]
            result = manager.snapshot()[source]
            if result is None:
                self.send(404, {"error": "No live run has been recorded yet."})
            else:
                self.send(200, result, attachment=f"mnemosyne-{source}.json")
        elif path in ASSETS:
            relative, content_type = ASSETS[path]
            try:
                data = (manager.root / relative).read_bytes()
            except OSError:
                self.send(404, {"error": "This presentation file is not available in this checkout."})
                return
            self.send(200, data, content_type)
        else:
            self.send(404, {"error": "Unknown route."})

    def do_POST(self):
        if not self.trusted_request(mutation=True):
            return
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if self.headers.get("Content-Type") != "application/json" or not 0 < size <= 1024:
                raise ValueError("Expected a small JSON request.")
            body = json.loads(self.rfile.read(size))
            if not isinstance(body, dict):
                raise ValueError("Expected a JSON object.")
            if self.path in ("/api/guest/start", "/api/guest/stop") and not body:
                if self.path.endswith("start"):
                    self.server.guest.start()
                else:
                    self.server.guest.stop()
                self.send(202, self.server.guest.snapshot())
            elif self.path == "/api/guest/command" and set(body) == {"command"}:
                self.server.guest.command(body["command"])
                self.send(202, self.server.guest.snapshot())
            elif self.path == "/api/runs" and set(body) == {"suite"} and isinstance(body["suite"], str):
                run_id = self.server.manager.start(body["suite"])
                self.send(202, {"id": run_id})
            elif self.path == "/api/stop" and set(body) == {"id"} and isinstance(body["id"], str):
                self.server.manager.stop(body["id"])
                self.send(202, {"message": "The current step will finish; remaining steps will be skipped."})
            else:
                raise ValueError("Unknown action or unsupported arguments.")
        except (ValueError, UnicodeError) as error:
            self.send(400, {"error": str(error)})
        except RuntimeError as error:
            self.send(409, {"error": str(error)})
        except (OSError, subprocess.SubprocessError) as error:
            self.send(503, {"error": str(error)})


class DashboardServer(ThreadingHTTPServer):
    def server_close(self):
        if hasattr(self, "guest"):
            self.guest.close()
        super().server_close()


def make_server(port=8765, manager=None, guest=None, guest_timeout=180):
    server = DashboardServer(("127.0.0.1", port), DashboardHandler)
    server.manager = manager or RunManager()
    server.guest = guest or GuestController(server.manager.root, timeout=guest_timeout)
    return server


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--guest-timeout", type=int, default=180, help="maximum guest boot or command seconds (10–600)")
    parser.add_argument("--open", action="store_true", help="open the dashboard in your default browser")
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error("--port must be between 1 and 65535")
    if not 10 <= args.guest_timeout <= 600:
        parser.error("--guest-timeout must be between 10 and 600")
    server = make_server(args.port, guest_timeout=args.guest_timeout)
    if hasattr(signal, "SIGTERM"):
        def terminate_dashboard(signum, frame):
            raise KeyboardInterrupt
        signal.signal(signal.SIGTERM, terminate_dashboard)
    url = f"http://127.0.0.1:{server.server_port}"
    print(f"MnemosyneOS dashboard: {url}\nLocal host UI. Press Ctrl+C to stop.\n", flush=True)
    if args.open:
        webbrowser.open(url)
    try:
        server.serve_forever(poll_interval=0.2)
    except KeyboardInterrupt:
        snapshot = server.manager.snapshot()
        if snapshot["busy"]:
            server.manager.stop(snapshot["latest"]["id"])
            print("Finishing the active bounded step before exiting.", flush=True)
    finally:
        server.server_close()
        if server.manager.worker:
            server.manager.worker.join()


if __name__ == "__main__":
    try:
        main()
    except OSError as error:
        print(f"Could not start local dashboard: {error}\nTry another --port if the address is in use.", file=sys.stderr)
        raise SystemExit(1)
