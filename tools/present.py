"""Guided, honest presentation of the existing kernel and host replay experiment."""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
EVIDENCE = ROOT / "presentation" / "evidence"


def pause(automatic):
    if not automatic:
        input("\nPress Enter to run this step (Ctrl+C to stop). ")


def step(title, explanation, command, automatic, transcript):
    print(f"\n{title}\n{'=' * len(title)}\n{explanation}", flush=True)
    print("\n> " + " ".join(command), flush=True)
    pause(automatic)
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, text=True, encoding="utf-8",
                            errors="replace", stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=240)
    elapsed = time.monotonic() - started
    print(result.stdout, end="", flush=True)
    transcript.append(f"\n{title}\n> {' '.join(command)}\n{result.stdout}")
    return {"title": title, "command": command, "exit_code": result.returncode,
            "elapsed_seconds": round(elapsed, 3), "output": result.stdout}


def recorded(automatic):
    manifest = json.loads((EVIDENCE / "manifest.json").read_text(encoding="utf-8"))
    print("RECORDED EVIDENCE ONLY. No build, test, or VM runs in this mode.")
    print(f"Captured: {manifest['captured_at_utc']}\nCode baseline: {manifest['code_commit']}")
    for title, name in [("Kernel serial output", "boot.log"),
                        ("Host test output", "tests.log"),
                        ("Replay report", "replay-demo.json")]:
        print(f"\n{title}")
        pause(automatic)
        print((EVIDENCE / name).read_text(encoding="utf-8"))
    print("End of recorded evidence. These files do not establish a fresh live result.")


def live(automatic, host_only):
    BUILD.mkdir(parents=True, exist_ok=True)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    print("MnemosyneOS live presentation demo")
    print(f"Repository: {ROOT}\nGit HEAD: {commit}")
    print("The kernel runs in QEMU. The Wasmi experiment runs separately on the host.")
    print("This is the CLI route; tools/dashboard.py provides a host browser UI.")
    print("No OS desktop, ChronicleFS, or AI planner is implemented yet.")
    transcript = []
    report = {"mode": "live-host-only" if host_only else "live", "git_head": commit,
              "started_at_utc": datetime.now(timezone.utc).isoformat(), "steps": [],
              "passed": False}
    commands = []
    if not host_only:
        commands.append(("1. Boot the Rust kernel",
            "Look for paging_ok with three fault probes, capabilities_ok, and boot_ok.\n"
            "This checks the boot address space and CPU faults. Process isolation is later work.",
            [sys.executable, "tools/dev.py", "boot"]))
    else:
        print("Host-only mode: the kernel boot is skipped.")
    commands.extend([
        ("2. Run capability and replay tests",
         "The baseline contains 8 capability tests and 16 replay/runtime tests.\n"
         "Negative tests exercise rejection, stale handles, resource limits, and traps.",
         [sys.executable, "tools/dev.py", "test"]),
        ("3. Restore and replay the Wasm fixture",
         "Compare final_state_sha256 with replay_state_sha256.\n"
         "All rejection flags must be true. Effects stay in memory in this experiment.",
         [sys.executable, "tools/dev.py", "demo"]),
    ])
    try:
        for title, explanation, command in commands:
            result = step(title, explanation, command, automatic, transcript)
            report["steps"].append(result)
            if result["exit_code"] != 0:
                raise RuntimeError(f"{title} failed with exit code {result['exit_code']}")
        replay = json.loads((BUILD / "replay-demo.json").read_text(encoding="utf-8"))
        if replay.get("passed") is not True:
            raise RuntimeError("Replay invariants failed")
        tests = next(s["output"] for s in report["steps"] if "tests" in s["title"])
        report["host_tests_passed"] = sum(map(int, re.findall(r"test result: ok\. (\d+) passed", tests)))
        report["replay"] = replay
        report["passed"] = True
        print(f"\nLIVE DEMO PASSED: {report['host_tests_passed']} host tests and exact fixture replay.")
        print("Scope: current prototype only. Durable storage and policy-checked AI recovery are proposed.")
    finally:
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        (BUILD / "presentation-demo.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (BUILD / "presentation-demo.log").write_text("".join(transcript), encoding="utf-8")
        print(f"\nRun record: {BUILD / 'presentation-demo.json'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--auto", action="store_true", help="run without Enter prompts")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--host-only", action="store_true", help="run tests/replay without QEMU")
    modes.add_argument("--recorded", action="store_true", help="show clearly labeled saved evidence")
    args = parser.parse_args()
    if args.recorded:
        recorded(args.auto)
    else:
        live(args.auto, args.host_only)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nDemo stopped by presenter.", file=sys.stderr)
        raise SystemExit(130)
    except (OSError, ValueError, RuntimeError, StopIteration, subprocess.SubprocessError) as error:
        print(f"\nDemo stopped: {error}", file=sys.stderr)
        raise SystemExit(1)
