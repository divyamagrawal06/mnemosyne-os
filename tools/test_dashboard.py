"""Boundary tests for the local process runner and browser API; no QEMU required."""
from copy import deepcopy
import http.client
import json
from pathlib import Path
import shutil
import tempfile
import threading
import unittest

import dashboard


EVIDENCE = dashboard.ROOT / "presentation" / "evidence"
OUTPUTS = {kind: (EVIDENCE / name).read_text(encoding="utf-8") for kind, name in
           (("boot", "boot.log"), ("test", "tests.log"), ("demo", "replay-demo.json"))}


class EvidenceTests(unittest.TestCase):
    def test_saved_checksums_reject_tampering(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            shutil.copytree(EVIDENCE, root / "presentation/evidence")
            self.assertEqual(dashboard.saved_evidence(root)["status"], "passed")
            (root / "presentation/evidence/boot.log").write_text('faked boot_ok', encoding="utf-8")
            result = dashboard.saved_evidence(root)
            self.assertEqual(result["status"], "unavailable")
            self.assertEqual(result["stages"], [])

    def test_boot_requires_completion_and_protection_events(self):
        for output in ('{"event":"boot_ok"}', OUTPUTS["boot"].replace('"fault_probes":3', '"fault_probes":0')):
            with self.assertRaises(ValueError):
                dashboard.parse_evidence("boot", output)

    def test_replay_requires_checks_and_matching_hashes(self):
        report = json.loads(OUTPUTS["demo"])
        for key, value in (("state_equal", False), ("missing_event_rejected", None),
                           ("replay_state_sha256", "0" * 64)):
            altered = dict(report, **{key: value})
            with self.assertRaises(ValueError):
                dashboard.parse_evidence("demo", json.dumps(altered))

    def test_test_parser_preserves_failed_cases(self):
        output = "Running unittests mnemosyne_capability\ntest tests::reject_stale ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored"
        result = dashboard.parse_evidence("test", output)
        self.assertEqual(result["failed"], 1)
        self.assertEqual(result["cases"][0]["status"], "FAILED")


class ManagerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.release = threading.Event()
        self.entered = threading.Event()
        self.calls = []

        def runner(kind, append, root):
            self.calls.append(kind)
            self.entered.set()
            self.release.wait(timeout=5)
            append(OUTPUTS[kind])
            return 0

        self.manager = dashboard.RunManager(Path(self.temp.name), runner)

    def tearDown(self):
        self.release.set()
        if self.manager.worker:
            self.manager.worker.join(timeout=10)
            self.assertFalse(self.manager.worker.is_alive())
        self.temp.cleanup()

    def test_single_flight_and_stop_at_boundary(self):
        run_id = self.manager.start("full")
        self.assertTrue(self.entered.wait(timeout=5))
        with self.assertRaises(RuntimeError):
            self.manager.start("demo")
        with self.assertRaises(RuntimeError):
            self.manager.stop("stale-id")
        self.manager.stop(run_id)
        self.release.set()
        self.manager.worker.join(timeout=5)
        latest = self.manager.snapshot()["latest"]
        self.assertEqual(self.calls, ["boot"])
        self.assertEqual(latest["status"], "stopped")
        self.assertEqual([s["status"] for s in latest["stages"]], ["passed", "skipped", "skipped"])

    def test_failure_never_reuses_previous_success(self):
        self.release.set()
        self.manager.start("demo")
        self.manager.worker.join(timeout=5)
        self.assertEqual(self.manager.snapshot()["latest"]["status"], "passed")
        def fail(kind, append, root):
            append("A new compiler failure\n")
            return 2
        self.manager.runner = fail
        self.manager.start("full")
        self.manager.worker.join(timeout=5)
        latest = self.manager.snapshot()["latest"]
        self.assertEqual(latest["status"], "failed")
        self.assertTrue(all(s["result"] is None for s in latest["stages"]))
        self.assertEqual(latest["stages"][1]["status"], "skipped")

    def test_zero_exit_without_valid_output_is_failure(self):
        self.manager.runner = lambda kind, append, root: 0
        self.manager.start("demo")
        self.manager.worker.join(timeout=5)
        self.assertEqual(self.manager.snapshot()["latest"]["status"], "failed")

    def test_log_is_bounded_and_marked(self):
        def noisy(kind, append, root):
            append("x" * (dashboard.LOG_LIMIT + 50))
            append("\n" + OUTPUTS[kind])
            return 0
        self.manager.runner = noisy
        self.manager.start("demo")
        self.manager.worker.join(timeout=5)
        stage = self.manager.snapshot()["latest"]["stages"][0]
        self.assertEqual(stage["status"], "passed")
        self.assertTrue(stage["log_truncated"])
        self.assertLessEqual(len(stage["log"]), dashboard.LOG_LIMIT)

    def test_restart_marks_interrupted_run(self):
        self.manager.storage.mkdir(parents=True)
        previous = {"schema": 1, "status": "running", "stages": [dict(dashboard.new_stage("demo"), status="running")]}
        (self.manager.storage / "latest.json").write_text(json.dumps(previous), encoding="utf-8")
        restarted = dashboard.RunManager(Path(self.temp.name))
        latest = restarted.snapshot()["latest"]
        self.assertEqual(latest["status"], "interrupted")
        self.assertEqual(latest["stages"][0]["status"], "interrupted")
        self.assertFalse(restarted.snapshot()["busy"])


class HttpBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.invoked = []
        def runner(kind, append, root):
            self.invoked.append(kind)
            append(OUTPUTS[kind])
            return 0
        self.manager = dashboard.RunManager(Path(self.temp.name), runner)
        self.server = dashboard.make_server(0, self.manager)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.host = f"127.0.0.1:{self.server.server_port}"
        self.headers = {"Content-Type": "application/json", "Origin": f"http://{self.host}", "X-Mnemosyne-Token": self.manager.token}

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.manager.worker:
            self.manager.worker.join(timeout=5)
        self.temp.cleanup()

    def request(self, method, path, data=None, headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.server.server_port, timeout=5)
        connection.request(method, path, json.dumps(data) if data is not None else None, headers or {})
        response = connection.getresponse()
        status, content = response.status, json.loads(response.read())
        connection.close()
        return status, content

    def test_cross_origin_and_tokenless_posts_do_not_execute(self):
        for headers in ({"Content-Type":"application/json"},
                        dict(self.headers, Origin="https://untrusted.example"),
                        dict(self.headers, **{"X-Mnemosyne-Token":"invalid"}),
                        dict(self.headers, **{"Sec-Fetch-Site":"cross-site"})):
            status, _ = self.request("POST", "/api/runs", {"suite":"demo"}, headers)
            self.assertEqual(status, 403)
        self.assertEqual(self.invoked, [])

    def test_rebinding_host_and_traversal_are_denied(self):
        self.assertEqual(self.request("GET", "/api/state", headers={"Host":"untrusted.example"})[0], 403)
        for path in ("/../AGENTS.md", "/%2e%2e/AGENTS.md", "/tools/dev.py"):
            self.assertEqual(self.request("GET", path)[0], 404)

    def test_only_fixed_suite_payload_is_accepted(self):
        for body in ({"suite":"shell"}, {"suite":"demo", "args":["untrusted"]},
                     {"suite":["demo"]}, ["demo"]):
            self.assertEqual(self.request("POST", "/api/runs", body, self.headers)[0], 400)
        self.assertEqual(self.invoked, [])
        self.assertEqual(self.request("POST", "/api/runs", {"suite":"demo"}, self.headers)[0], 202)
        self.manager.worker.join(timeout=5)
        self.assertEqual(self.invoked, ["demo"])
        status, exported = self.request("GET", "/api/export/latest")
        self.assertEqual(status, 200)
        self.assertEqual(exported["status"], "passed")
        self.assertNotIn("token", exported)


if __name__ == "__main__":
    unittest.main()
