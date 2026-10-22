# MnemosyneOS guest workspace

The default page is the live OS workspace. It controls the custom x86-64 guest in QEMU through a Python standard-library server. The browser, server, and guest are separate: opening the page on a Raspberry Pi does not make the kernel a native Pi kernel.

Start the dashboard with `python tools/dashboard.py`, then open <http://127.0.0.1:8765>. The guest runner needs its prebuilt release ISO, QEMU and a writable local data directory; see the [cloud and guest runbook](../docs/cloud-demo.md) for preparation and deployment. No npm runtime, remote fonts, CDN or network dependency is needed for the UI.

1. Select **Start OS**. Commands are disabled until the real serial prompt is ready.
2. **Files** lists guest files. Write a file, select **Read**, save a disk snapshot, change the file, and restore the snapshot. Restore replaces the filesystem state. File contents are single-line printable ASCII; names start with a letter or number and may contain dots, underscores or hyphens.
3. **Tasks** shows records parsed from the guest's actual `ps` output. Refresh to obtain new counters.
4. **Experiments** sends the displayed guest commands. Output appears in **Guest serial console**. Repeated benchmark samples use the guest's coarse 10 ms timer; zero ticks means the work completed below one timer tick, not zero elapsed time.
5. **Recovery** displays the current plan, saves a candidate plan JSON file, and verifies it. Valid formatted JSON is compacted onto one serial command line. The maximum entire command is 480 characters. Inspect the real result before running recovery. Guest event details are raw serial JSON, not invented trace records.
6. **Stop OS** asks an idle guest to shut down. An unresponsive or busy guest can be terminated by the runner. Inspect serial output before repeating a timed-out action.

**Follow output** controls console scrolling, and the command field supports arrow-key history. **Save log** exports serial text; **Export evidence** downloads the current guest state. Polling reconnects after a host interruption, while controls stay disabled until connection and guest readiness return.

The original dashboard remains at [the host evidence archive](http://127.0.0.1:8765/evidence.html). Its Wasmi replay, Cargo tests and saved baseline execute or were captured on the host. They do not establish the corresponding feature runs in the kernel. The archive documentation below describes that earlier evidence workflow.

## Host evidence archive

Current integrated OS demo: [cloud and guest runbook](../docs/cloud-demo.md). The original deck and saved browser evidence retain their earlier baseline.

From the repository root:

```powershell
python tools/dashboard.py --open
```

Open <http://127.0.0.1:8765>. The `--open` flag is optional. Use `--port 8766` if the default port is occupied. Stop the server with Ctrl+C in the terminal that started it. If a run is active, shutdown finishes the current bounded command and skips remaining commands.

Python 3.10 or newer is enough to view saved evidence. Live execution uses the repository's existing Python, Git, pinned Rust toolchain, and Cargo packages. Kernel boot additionally uses the existing QEMU/xorriso environment (Ubuntu WSL on Windows). There are no npm dependencies, CDN assets, remote fonts, model calls, or cloud services. Rehearse once to populate the existing build and dependency caches.

## Presentation route

1. At slide 15 of the deck, switch to the browser. Leave **Full demonstration** selected and click **Run demo**.
2. Watch **Overview** advance through kernel boot, tests, and checkpoint/replay. It changes the selected evidence source to **Latest run**.
3. Open **Kernel** to show real guest events. Explain the three CPU protection probes and the distinction between interrupts and process preemption.
4. Open **Test suite**. Filter by Capabilities or Replay & runtime; search a case such as `stale` or `fuel`.
5. Open **Replay inspector**. Compare the state digests and rejection results. Filter recorded events to Request, Clock, Random, Effect, or Reply, then select an event to inspect its actual payload.
6. Expand **Raw console** if a reviewer wants the command output. Use **Export evidence** to save a JSON record.
7. Open **Project & demo** for the implementation map, pitch deck, PowerPoint, and full narration guide.

Browser zoom around 110-125% can help on a classroom projector. The interface is responsive and keyboard accessible; the source switch and statuses use text as well as color.

## Live, saved, and absent evidence

The original **Saved demonstration** is read from `presentation/evidence`, checked against its checksum manifest, and labeled with its capture time and code commit. It is not replaced by new runs. The original report contains aggregate replay evidence but no event array; the UI explicitly asks for a new replay run to inspect the trace.

**Latest run** belongs to the local dashboard runner. It includes its ID, checkout revision, dirty-working-tree flag, stages, raw output, exit status, and parsed evidence. A stage produces a passing result only if its command succeeds and its current output passes the relevant evidence checks. Failed, skipped, and unstarted stages do not inherit results from older runs or build files.

The latest record is atomically saved to ignored `build/dashboard/latest.json`. Restarting the dashboard retains it. A run left active by a crash is marked interrupted, never resumed or reclassified as successful. Starting another run replaces Latest run; export a result first if you want a separate copy. The original CLI output files under `build/` remain available.

Stage durations are wall time including compilation, image construction, and host scheduling. They are not kernel or Wasm performance benchmarks.

## Runner boundary

The server listens only on `127.0.0.1`. It serves a fixed set of assets, presentation files, and API routes, with no directory browsing or arbitrary file path endpoint. State changes require a matching loopback Host/Origin and a per-process token. The API accepts only named suites and never accepts shell text, flags, environment overrides, or code from the browser. It uses argument-vector subprocesses without a shell.

Each server instance runs one suite at a time. Keep one dashboard instance active and avoid starting a separate CLI build while its suite runs. **Stop after step** requests a boundary stop: the active step finishes, and pending steps are skipped. Each command has a 240-second runner timeout; the guest has its existing 25-second QEMU timeout. Console output is capped at 128 KiB per stage and marked when truncated. No external access or deployment is provided; this is a local development tool.

The Rust experiment exports the real `Vec<Event>` after recording. The frontend renders these records as data, with no fabricated timeline, scrub-to-restore control, or executed AI recovery plan. The fixture's bounded integers are safe for this JavaScript display; this is not a general binary trace format.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| Runner disconnected | Start `python tools/dashboard.py`, check its terminal, and leave the browser open to reconnect. |
| Address already in use | Use the running dashboard or choose another `--port`; avoid simultaneous runs. |
| Kernel fails to start | Inspect Raw console. Check `wsl -l -v` and the QEMU/xorriso prerequisites in the demo guide. Host checks and saved evidence remain available. |
| Step exceeded its time limit | Inspect the log and toolchain/cache prerequisites; retry after resolving the cause. |
| Saved evidence unavailable | Restore the original evidence files and manifest from Git. The UI will not display altered files as validated recorded evidence. Restart the server after restoring them. |
| No trace in Saved demonstration | Run Replay only and inspect Latest run. The original archived summary is intentionally preserved. |
| Run failed | Inspect the failed stage's Raw console. No later stage runs, and no previous success is substituted. |

## Development checks

```powershell
python tools/test-dashboard.py -v
python tools/dev.py test
python tools/dev.py check
node --check ui/app.js
```

The Python tests exercise rejected cross-origin/tokenless requests, host validation, fixed command selection, checksum tampering, single-flight execution, stale stop requests, boundary stopping, interruption recovery, bounded logs, and failure provenance. The Rust tests check that exported observations retain the recorded request/effect/reply relationships.

The frontend uses ordinary HTML, CSS, and JavaScript. Design context lives in `PRODUCT.md` and `DESIGN.md`. Python's `http.server` is deliberately confined to this local development tool; see its [standard-library documentation](https://docs.python.org/3/library/http.server.html). Browser origin semantics follow the [Origin header documentation](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Origin).
