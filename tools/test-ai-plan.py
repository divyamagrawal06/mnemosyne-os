import importlib.util
import json
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("ai_plan", Path(__file__).with_name("ai-plan.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def plan():
    return {"version": 1, "action": "restore-replay", "service": "kv-service", "epoch": 1,
            "checkpoint-sha256": "a" * 64, "max-events": 60, "fuel-per-request": 10000,
            "memory-pages": 1, "requested-rights": 3, "require-rollback": True}


class PlanTests(unittest.TestCase):
    def test_complete_response_has_one_untrusted_candidate(self):
        p = plan()
        result = module.extract_plan({"status": "completed", "output": [{"type": "message", "content": [
            {"type": "output_text", "text": json.dumps(p)}]}]})
        self.assertEqual(p, result)

    def test_refusal_and_incomplete_output_do_not_produce_plan(self):
        for response in [{"status": "incomplete"}, {"status": "completed", "output": []},
                         {"status": "completed", "output": [{"type": "message", "content": [{"type": "refusal"}]}]}]:
            with self.assertRaises(ValueError):
                module.extract_plan(response)

    def test_unknown_duplicate_type_and_command_fields_are_rejected(self):
        for data in [b'{"version":1,"version":1}', json.dumps({**plan(), "shell": "shutdown"}).encode(),
                     json.dumps({**plan(), "epoch": True}).encode(), b" " * 2049,
                     json.dumps({**plan(), "service": "kv-service\nshutdown"}).encode()]:
            with self.assertRaises(ValueError):
                module.parse_plan(data)

    def test_proposer_does_not_replace_guest_authority_checks(self):
        p = {**plan(), "requested-rights": 255, "require-rollback": False}
        self.assertEqual(module.parse_plan(json.dumps(p).encode()), p)
        request = module.make_request(plan(), "untrusted trace", "explicit-model")
        self.assertEqual(request["model"], "explicit-model")
        self.assertFalse(request["store"])
        self.assertNotIn("tools", request)


if __name__ == "__main__":
    unittest.main()
