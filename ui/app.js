'use strict';

const $ = (selector) => document.querySelector(selector);
const escapeHtml = (value) => String(value ?? '').replace(/[&<>"']/g, (character) => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[character]));
const paths = {
  grid: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
  cpu: '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4"/><path d="M10 10h4v4h-4z"/>',
  replay: '<path d="M3 11a9 9 0 1 1 2 7M3 4v7h7"/><path d="M12 7v5l3 2"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  circleCheck: '<circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5m-18 5 9 5 9-5"/>',
  play: '<path d="m8 4 12 8-12 8V4Z"/>',
  download: '<path d="M12 3v12m-4-4 4 4 4-4M4 16v5h16v-5"/>',
  file: '<path d="M14 2H5v20h14V7l-5-5Zm0 0v6h5M8 12h8m-8 4h8"/>',
  terminal: '<path d="m4 6 6 6-6 6m9 0h7"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v.1"/>',
  arrow: '<path d="M4 12h16m-6-6 6 6-6 6"/>',
  cross: '<path d="m6 6 12 12M6 18 18 6"/>',
};
const icon = (name) => `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true">${paths[name] || paths.info}</svg>`;
document.querySelectorAll('[data-icon]').forEach((node) => { node.innerHTML = icon(node.dataset.icon); });

const pages = {
  overview: ['Overview', 'DEMONSTRATION WORKSPACE', 'Service recovery, observed.', 'Boot, verify, and replay the working prototype.'],
  kernel: ['Kernel', 'X86-64 · QEMU GUEST', 'Inside the kernel boot.', 'Inspect the events emitted by the real Rust kernel.'],
  replay: ['Replay inspector', 'WASMI · HOST EXPERIMENT', 'Same history. Same state.', 'Inspect a checkpoint, recorded inputs, and replay validation.'],
  tests: ['Test suite', 'CAPABILITIES · RUNTIME · REPLAY', 'Correctness under pressure.', 'Review the success and failure cases exercised by the host tests.'],
  project: ['Project & demo', 'RESEARCH PROTOTYPE', 'The demo, and what comes next.', 'Presentation materials, implementation scope, and the next acceptance gates.'],
};
let state = null, source = 'saved', view = 'overview', connected = false, submitting = false;
let rendered = '', testQuery = '', testGroup = 'all', traceFilter = 'all', selectedEvent = 0;
let lastAnnounced = '', pollTimer = null;
const statusLabels = {passed:'Passed', failed:'Failed', running:'Running', pending:'Waiting', skipped:'Skipped', stopped:'Stopped', interrupted:'Interrupted', unavailable:'Unavailable', ok:'Passed', recorded:'Recorded'};
function badge(status, label) { return `<span class="badge ${escapeHtml(status)}">${status === 'passed' || status === 'ok' ? icon('check') : ''}${escapeHtml(label || statusLabels[status] || status)}</span>`; }
function selectedRun() { return state?.[source] || null; }
function stageFor(kind) { return selectedRun()?.stages?.find((stage) => stage.kind === kind); }
function resultFor(kind) { const stage = stageFor(kind); return stage?.status === 'passed' ? stage.result : null; }
function formatDate(value) { if (!value) return 'Not started'; const date = new Date(value); return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat(undefined, {month:'short', day:'numeric', year:'numeric', hour:'2-digit', minute:'2-digit'}).format(date); }
function formatNumber(value) { return typeof value === 'number' ? new Intl.NumberFormat().format(value) : 'Unavailable'; }
function showNotice(message, error = false) { const node = $('#notice'); node.hidden = !message; node.textContent = message; node.className = `notice${error ? ' error' : ''}`; }
function empty(title, text, suite = '') { return `<div class="panel empty">${icon('terminal')}<h2>${escapeHtml(title)}</h2><p>${escapeHtml(text)}</p>${suite ? `<button class="button primary" data-run="${suite}">${icon('play')}Run ${suite === 'full' ? 'demo' : suite === 'demo' ? 'replay' : suite === 'test' ? 'tests' : 'kernel'}</button>` : ''}</div>`; }
function panelHeader(title, subtitle = '', right = '') { return `<div class="panel-head"><div><h2>${title}</h2>${subtitle ? `<p>${subtitle}</p>` : ''}</div>${right}</div>`; }
function infoNote(text) { return `<div class="panel-note">${icon('info')}<span>${text}</span></div>`; }

function updateChrome() {
  $('#connection').textContent = connected ? state?.busy ? 'Local runner · active' : 'Local runner connected' : 'Runner disconnected';
  $('#connection').classList.toggle('offline', !connected);
  const busy = state?.busy || submitting;
  $('#run').disabled = !connected || busy;
  $('#suite').disabled = !connected || busy;
  document.querySelectorAll('[data-run]').forEach((button) => { button.disabled = !connected || busy; });
  $('#run').querySelector('span:last-child').textContent = busy ? 'Running…' : 'Run demo';
  $('#stop').hidden = !state?.busy;
  $('#stop').disabled = !connected || !!state?.latest?.stop_requested;
  $('#stop').textContent = state?.latest?.stop_requested ? 'Finishing current step…' : 'Stop after step';
  document.querySelectorAll('[data-source]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.source === source)));
  $('#download').href = `/api/export/${source}`;
  $('#download').hidden = !selectedRun();
  const run = selectedRun();
  if (!run) {
    $('#provenance').innerHTML = `${badge('pending', 'No live run yet')}<span>Choose a suite and run the current checkout.</span>`;
  } else {
    $('#provenance').innerHTML = `${badge(source === 'saved' ? 'recorded' : run.status, source === 'saved' ? 'Saved evidence' : undefined)}
      <span>${source === 'saved' ? 'Captured' : 'Started'} ${escapeHtml(formatDate(run.started_at))}</span>
      ${run.git_head ? `<span class="mono" title="${escapeHtml(run.git_head)}">${escapeHtml(run.git_head.slice(0, 7))}${run.dirty ? ' + working changes' : ''}</span>` : ''}
      <span class="provenance-note">${source === 'saved' ? 'A previous demonstration, not a fresh run.' : `Run ${escapeHtml(run.id)} · ${run.finished_at ? 'Finished ' + escapeHtml(formatDate(run.finished_at)) : 'Results appear as steps finish.'}`}</span>`;
  }
  const [name, eyebrow, title, description] = pages[view];
  $('#breadcrumb').textContent = name; $('#eyebrow').textContent = eyebrow;
  $('#title').textContent = title; $('#description').textContent = description;
  document.title = `MnemosyneOS · ${name}`;
  document.querySelectorAll('[data-nav]').forEach((link) => {
    if (link.dataset.nav === view) link.setAttribute('aria-current', 'page'); else link.removeAttribute('aria-current');
  });
}

function renderConsole(kinds = ['boot', 'test', 'demo']) {
  const stages = (selectedRun()?.stages || []).filter((s) => kinds.includes(s.kind));
  if (!stages.length) return '';
  const log = stages.map((stage) => `> ${stage.command}\n${stage.log_truncated ? '[Earlier console output truncated at the display limit]\n' : ''}${stage.log || 'No output yet.'}${stage.error ? '\nERROR: ' + stage.error : ''}`).join('\n\n');
  return `<details class="console" data-console><summary>${icon('terminal')} Raw console <span>Actual command output</span></summary><pre tabindex="0" aria-label="Command output">${escapeHtml(log)}</pre></details>`;
}

function replayComparison(compact = false) {
  const report = resultFor('demo');
  if (!report) return empty('Replay evidence will appear here', 'Run the replay experiment to compare the original and restored fixture state.', 'demo');
  return `<section class="panel">${panelHeader('Replay state comparison', 'SHA-256 of the complete fixture state', badge('passed', 'Exact match'))}
    <div class="hash-compare"><div class="hash-label">${icon('layers')}Original final state</div><code class="hash-value">${escapeHtml(report.final_state_sha256)}</code>
      <div class="hash-divider"></div><div class="hash-label">${icon('replay')}Restored and replayed state</div><code class="hash-value">${escapeHtml(report.replay_state_sha256)}</code></div>
    <dl class="mini-facts"><div><dt>Requests replayed</dt><dd>${formatNumber(report.replayed_requests)}</dd></div><div><dt>Recorded events</dt><dd>${formatNumber(report.trace_events)}</dd></div><div><dt>Mutable state</dt><dd>${formatNumber(report.snapshot_mutable_state_bytes)} B</dd></div></dl>
    ${compact ? '' : infoNote('Full state copy at a completed-handler boundary. Effects are compared in memory; durable recovery is a later milestone.')}</section>`;
}

function rejectionChecks() {
  const report = resultFor('demo');
  const cases = [
    ['missing_event_rejected', 'Missing input', 'Remove a recorded event'],
    ['reordered_event_rejected', 'Reordered history', 'Swap clock and random events'],
    ['trailing_event_rejected', 'Unconsumed event', 'Append an extra clock event'],
    ['corrupted_checkpoint_rejected', 'Corrupted checkpoint', 'Flip a byte in saved memory'],
  ];
  return `<section class="panel">${panelHeader('Failure checks', 'Actual rejection cases in the replay experiment')}
    <ul class="check-list">${cases.map(([key, title, description]) => `<li>${icon(report?.[key] ? 'circleCheck' : 'info')}<span>${title}<span class="check-detail">${description}</span></span>${badge(report ? report[key] ? 'passed' : 'failed' : 'pending', report ? report[key] ? 'Rejected' : 'Accepted' : 'Not run')}</li>`).join('')}</ul>
    ${infoNote('A failed replay discards pending effects and requires a fresh instance.')}</section>`;
}

function renderOverview() {
  const run = selectedRun();
  if (!run) return empty('Ready for the first live run', 'Run the full demonstration, or choose host checks to skip QEMU. The saved demonstration remains available in the source switch above.', 'full');
  if (run.status === 'unavailable') return empty('Saved evidence is unavailable', run.error, 'full');
  const definitions = [
    ['boot', '01', 'Boot the kernel', 'CPU protection, exceptions, and timer delivery.', 'kernel'],
    ['test', '02', 'Verify the contracts', 'Capability authority and replay failure cases.', 'tests'],
    ['demo', '03', 'Restore & replay', 'Reconstruct state from recorded inputs.', 'replay'],
  ];
  const tests = resultFor('test'), boot = resultFor('boot');
  return `<section class="panel">${panelHeader('The demonstration path', 'Three independently inspectable stages', badge(run.status, source === 'saved' && run.status === 'passed' ? 'Recorded run passed' : undefined))}
    <div class="run-flow">${definitions.map(([kind, n, title, description, href]) => {
      const stage = stageFor(kind), status = stage?.status || 'skipped';
      const detail = kind === 'test' && tests ? `${tests.passed} tests passed` : kind === 'boot' && boot ? `${boot.events.length} serial events` : kind === 'demo' && resultFor('demo') ? `${resultFor('demo').replayed_requests} requests replayed` : stage?.duration_seconds != null ? `${stage.duration_seconds.toFixed(2)} s wall time` : stage ? 'Awaiting completion' : 'Outside this suite';
      return `<div class="flow-step"><span class="step-number ${escapeHtml(status)}">${status === 'passed' ? icon('check') : status === 'failed' ? icon('cross') : n}</span><h3><a href="#${href}">${title} <span aria-hidden="true">↗</span></a></h3><p>${description}</p><div class="step-foot">${badge(status)}<small>${escapeHtml(detail)}</small></div></div>`;
    }).join('')}</div>${infoNote('The kernel boots inside QEMU. Tests and the Wasmi replay experiment execute separately on the host.')}</section>
    ${run.error ? `<div class="notice error section-gap">${escapeHtml(run.error)}</div>` : ''}
    <div class="split">${replayComparison(true)}${rejectionChecks()}</div>
    <div class="scope-strip"><span class="section-label">CURRENT BOUNDARY</span><p><strong>Working:</strong> kernel boot, capability checks, and host replay.<br><strong>Next:</strong> isolated services, persistent snapshots, and policy-checked recovery. <a class="text-link" href="#project">View roadmap ${icon('arrow')}</a></p></div>${renderConsole()}`;
}

function renderKernel() {
  const boot = resultFor('boot');
  const stage = stageFor('boot');
  const descriptions = {boot:'Rust kernel entry', memory:'Boot memory map', frames_ok:'Physical frame allocation', capabilities_ok:'Kernel capability smoke check', paging_ok:'NX, write protection, and unmap faults', exception_ok:'Breakpoint exception delivery', timer_ok:'PIT interrupt delivery', boot_ok:'All guest checks completed'};
  const events = boot ? `<section class="panel">${panelHeader('Serial event sequence', 'Events are shown in the order the guest emitted them.', badge('passed', 'Boot passed'))}<ol class="event-list">${boot.events.map((event, index) => `<li><span class="event-index">${String(index + 1).padStart(2,'0')}</span><div><div class="event-name">${escapeHtml(event.event)}</div><div class="event-fields">${escapeHtml(descriptions[event.event] || '')}${Object.keys(event).length > 1 ? ' · ' + escapeHtml(Object.entries(event).filter(([key]) => key !== 'event').map(([key, value]) => `${key}=${value}`).join(' · ')) : ''}</div></div>${icon('circleCheck')}</li>`).join('')}</ol></section>` : empty(stage?.status === 'running' ? 'Kernel boot is running' : 'No kernel result in this run', stage?.error || 'Select Kernel boot only or run the full demonstration to collect guest events.', 'boot');
  return `<div class="split section-gap">${events}<div><section class="panel">${panelHeader('Guest configuration', 'Bounded local smoke test')}<dl class="facts-list"><div><dt>Architecture</dt><dd class="mono">x86-64 · q35</dd></div><div><dt>Emulation</dt><dd>QEMU / TCG</dd></div><div><dt>CPU / memory</dt><dd>1 vCPU / 256 MiB</dd></div><div><dt>Bootloader</dt><dd>Limine</dd></div><div><dt>Networking</dt><dd>No network device</dd></div><div><dt>Guest timeout</dt><dd>25 seconds</dd></div><div><dt>Expected guest exit</dt><dd class="mono">33 (debug exit)</dd></div></dl>${infoNote('This is the boot address space. Timer delivery does not establish process preemption.')}</section>
    <section class="panel section-gap">${panelHeader('Three protection probes')}<ul class="check-list"><li>${icon('cpu')}Execute from an NX page</li><li>${icon('cpu')}Write to a read-only page</li><li>${icon('cpu')}Read a page after unmapping</li></ul></section></div></div>${renderConsole(['boot'])}`;
}

function traceRows(report) {
  return report.trace.map((event, index) => ({event, index})).filter(({event}) => traceFilter === 'all' || event.event === traceFilter);
}
function renderTrace(report) {
  if (!Array.isArray(report?.trace)) return `<section class="panel section-gap">${panelHeader('Event trace', 'This saved report contains aggregate results only.')}<div class="panel-body"><p class="small muted">Run the current replay experiment to inspect its actual request, clock, random, effect, and reply events. The saved summary is preserved as originally captured.</p><button class="button" data-run="demo">${icon('play')}Run replay with trace</button></div></section>`;
  const rows = traceRows(report);
  if (!rows.some(({index}) => index === selectedEvent)) selectedEvent = rows[0]?.index ?? 0;
  return `<section class="panel section-gap">${panelHeader('Recorded event trace', `${report.trace.length} observed events · Select an event to inspect its payload`, `<div class="trace-filter"><label for="trace-filter" class="sr-only">Filter event type</label><select id="trace-filter">${['all','request','clock','random','emit','reply'].map((value) => `<option value="${value}" ${traceFilter === value ? 'selected' : ''}>${value === 'all' ? 'All event types' : value === 'emit' ? 'Effect (emit)' : value[0].toUpperCase() + value.slice(1)}</option>`).join('')}</select></div>`)}
    <div class="trace-layout"><div class="trace-list" aria-label="Recorded events">${rows.map(({event, index}) => `<button data-event="${index}" aria-pressed="${index === selectedEvent}"><span class="event-index">${String(index + 1).padStart(2,'0')}</span><span>${escapeHtml(event.event)}</span><span>${event.event === 'request' ? 'key ' + escapeHtml(event.request.key) : event.event === 'emit' ? 'version ' + escapeHtml(event.effect.version) : escapeHtml(event.value)}</span></button>`).join('') || '<p class="small muted">No matching events.</p>'}</div><div id="trace-detail" class="trace-detail">${eventDetail(report)}</div></div>${infoNote('These are recorded observations. Selecting an event inspects the trace; it does not execute a recovery action.')}</section>`;
}
function eventDetail(report) { const event = report.trace[selectedEvent]; return event ? `<span class="section-label">EVENT ${String(selectedEvent + 1).padStart(2,'0')}</span><h3>${escapeHtml(event.event)}</h3><pre>${escapeHtml(JSON.stringify(event, null, 2))}</pre>` : '<p class="muted">Select an event.</p>'; }
function renderReplay() {
  const report = resultFor('demo');
  if (!report) return empty(stageFor('demo')?.status === 'running' ? 'The replay experiment is running' : 'No replay result in this run', stageFor('demo')?.error || 'Run replay to record inputs, restore a checkpoint into a new instance, and compare complete fixture state.', 'demo') + renderConsole(['demo']);
  return `<div class="split equal">${replayComparison()}${rejectionChecks()}</div>${renderTrace(report)}
    <details class="console"><summary>${icon('layers')} Checkpoint identity <span>Module, profile, and snapshot binding</span></summary><div class="panel-body section-gap"><p class="small muted">${escapeHtml(report.scope)}</p><div class="hash-label">Execution profile</div><code class="hash-value">${escapeHtml(report.profile)}</code><div class="hash-label">Module SHA-256</div><code class="hash-value">${escapeHtml(report.module_sha256)}</code><div class="hash-label">Checkpoint SHA-256</div><code class="hash-value">${escapeHtml(report.checkpoint_sha256)}</code></div></details>${renderConsole(['demo'])}`;
}

function testRows(tests) {
  const filtered = tests.cases.filter((test) => (testGroup === 'all' || test.group === testGroup) && test.name.toLowerCase().includes(testQuery.toLowerCase()));
  return filtered.length ? filtered.map((test) => `<tr><td class="test-name">${escapeHtml(test.name.replaceAll('_', ' '))}<small class="mono">${escapeHtml(test.name)}</small><small class="test-group-inline">${escapeHtml(test.group)}</small></td><td>${escapeHtml(test.group)}</td><td>${badge(test.status === 'ok' ? 'passed' : test.status === 'FAILED' ? 'failed' : 'skipped')}</td></tr>`).join('') : '<tr><td colspan="3">No tests match this filter.</td></tr>';
}
function renderTests() {
  const tests = resultFor('test');
  if (!tests) return empty(stageFor('test')?.status === 'running' ? 'The host tests are running' : 'No test result in this run', stageFor('test')?.error || 'Run the test suite to inspect capability behavior and replay failure cases.', 'test') + renderConsole(['test']);
  return `<section class="panel">${panelHeader('Host correctness suite', `${tests.passed} passed · ${tests.failed} failed · ${tests.ignored} ignored`, `<div class="test-tools"><label class="sr-only" for="test-search">Search tests</label><input id="test-search" type="search" placeholder="Search test cases…" value="${escapeHtml(testQuery)}"><label class="sr-only" for="test-group">Test group</label><select id="test-group"><option value="all">All groups</option>${['Capabilities','Replay & runtime'].map((group) => `<option value="${escapeHtml(group)}" ${testGroup === group ? 'selected' : ''}>${escapeHtml(group)}</option>`).join('')}</select></div>`)}
    <div class="table-wrap" tabindex="0" role="region" aria-label="Test results"><table class="data-table test-table"><thead><tr><th scope="col">TEST CASE</th><th scope="col">GROUP</th><th scope="col">RESULT</th></tr></thead><tbody id="test-rows">${testRows(tests)}</tbody></table></div>${infoNote('These tests exercise the prototype contracts. They do not constitute a whole-system formal proof.')}</section>${renderConsole(['test'])}`;
}

function renderProject() {
  const milestones = [
    ['Rust microkernel', 'Implemented', 'Boot, memory mapping, CPU faults, and interrupts'],
    ['Capability table', 'Implemented', 'Attenuation, subtree revocation, and stale handles'],
    ['Wasm checkpoint & replay', 'Host prototype', 'Fixed fixture, full state copies, bounded trace'],
    ['Demo dashboard', 'Implemented', 'Local runner, evidence views, and export'],
    ['Processes & IPC', 'Planned', 'Ring 3, preemption, and bounded endpoints'],
    ['ChronicleFS', 'Planned', 'Persistent roots and crash recovery'],
    ['Durable service recovery', 'Planned', 'IPC replay and external-effect protocol'],
    ['AI plan verifier', 'Planned', 'Typed plans, authority checks, and staged execution'],
    ['Performance evaluation', 'Planned', 'Controlled benchmarks and fault campaigns'],
  ];
  return `<section class="panel project-intro"><p class="eyebrow">MNEMOSYNEOS</p><h2>A capability-secure microkernel with deterministic service replay and verified AI-assisted recovery.</h2><p>The proposal describes the intended system. This dashboard demonstrates the kernel and host replay foundations that currently exist. Policy-checked AI recovery remains a research milestone.</p></section>
    <div class="split equal"><section class="panel">${panelHeader('Your presentation route', 'The UI can replace the three terminal demonstration steps.')}<ol class="guide-steps"><li><span><strong>Introduce the research question.</strong><small>Use slides 1–14 to explain the architecture and current scope.</small></span></li><li><span><strong>Switch here at slide 15.</strong><small>Select Full demonstration and click Run demo. Inspect Kernel, Tests, and Replay as each stage finishes.</small></span></li><li><span><strong>Open the replay trace.</strong><small>Compare the state digests, show rejected histories, and select a recorded input.</small></span></li><li><span><strong>Return to the deck.</strong><small>Finish with the evaluation plan and milestones. Use Saved demonstration if a live prerequisite is unavailable.</small></span></li></ol></section>
    <section class="panel">${panelHeader('Presentation materials', 'Local files included with the repository')}<div class="resources"><a href="/files/pitch.pdf" target="_blank" rel="noopener">View pitch deck <span>30 slides ↗</span></a><a href="/files/pitch.pptx" download>Editable PowerPoint <span>PPTX ↓</span></a><a href="/files/guide.md" target="_blank" rel="noopener">Full demo guide <span>Commands + narration ↗</span></a><a href="/api/export/${source}" download>Export selected evidence <span>JSON ↓</span></a></div><div class="panel-body"><span class="section-label">START THE DASHBOARD</span><p class="small section-gap"><code class="inline-code">python tools/dashboard.py --open</code></p><p class="small muted">Local browser UI. Existing Python, Rust, and QEMU tools power the live runs. No web package installation is needed.</p></div></section></div>
    <section class="panel section-gap">${panelHeader('Implementation map', 'Acceptance gates remain visible alongside current capabilities.')}<div class="table-wrap"><table class="data-table"><thead><tr><th>COMPONENT</th><th>STATUS</th><th>SCOPE</th></tr></thead><tbody>${milestones.map(([component,status,scope]) => `<tr><td>${component}</td><td>${badge(status === 'Planned' ? 'pending' : status === 'Host prototype' ? 'recorded' : 'passed',status)}</td><td>${scope}</td></tr>`).join('')}</tbody></table></div></section>`;
}

function render(force = false) {
  updateChrome();
  const signature = JSON.stringify([view, source, selectedRun()]);
  if (!force && signature === rendered) return;
  rendered = signature;
  const openConsole = $('[data-console]')?.open;
  const oldConsole = $('[data-console] pre');
  const oldScroll = oldConsole?.scrollTop || 0;
  const wasAtEnd = oldConsole ? oldConsole.scrollHeight - oldConsole.scrollTop - oldConsole.clientHeight < 50 : false;
  const active = document.activeElement?.id;
  const selection = active === 'test-search' ? [document.activeElement.selectionStart, document.activeElement.selectionEnd] : null;
  const renderers = {overview:renderOverview, kernel:renderKernel, replay:renderReplay, tests:renderTests, project:renderProject};
  $('#content').innerHTML = renderers[view]();
  document.querySelectorAll('[data-run]').forEach((button) => { button.disabled = !connected || !!state?.busy || submitting; });
  if (openConsole && $('[data-console]')) {
    $('[data-console]').open = true;
    const nextConsole = $('[data-console] pre');
    nextConsole.scrollTop = wasAtEnd ? nextConsole.scrollHeight : oldScroll;
  }
  if (active && document.getElementById(active)) {
    document.getElementById(active).focus({preventScroll:true});
    if (selection) document.getElementById(active).setSelectionRange(...selection);
  }
}

async function api(url, body) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 8000);
  try {
    const response = await fetch(url, body ? {method:'POST', headers:{'Content-Type':'application/json', 'X-Mnemosyne-Token':state?.token || ''}, body:JSON.stringify(body), signal:controller.signal} : {signal:controller.signal});
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `Request failed (${response.status}).`);
    return result;
  } finally { clearTimeout(timeout); }
}
async function poll() {
  clearTimeout(pollTimer);
  try {
    const previousConnection = connected;
    state = await api('/api/state'); connected = true;
    if (!previousConnection) showNotice('');
    if (state.persistence_error) showNotice(state.persistence_error, true);
    const announcement = state.latest ? `${state.latest.id}:${state.latest.status}:${state.latest.stages.map((s) => s.status).join(',')}` : '';
    if (announcement !== lastAnnounced) {
      lastAnnounced = announcement;
      $('#announcer').textContent = state.latest ? `Run ${statusLabels[state.latest.status] || state.latest.status}. ${state.latest.stages.filter((s) => s.status === 'passed').length} steps passed.` : '';
    }
    render();
  } catch (error) {
    connected = false;
    showNotice('The local runner is disconnected. Start python tools/dashboard.py, then this page will reconnect. Displayed evidence has not been refreshed.', true);
    updateChrome();
    document.querySelectorAll('[data-run]').forEach((button) => { button.disabled = true; });
  } finally { pollTimer = setTimeout(poll, state?.busy ? 800 : 2500); }
}
async function startRun(suite) {
  if (!connected || state?.busy || submitting) return;
  submitting = true; showNotice(''); render(true);
  try {
    await api('/api/runs', {suite});
    source = 'latest'; rendered = ''; selectedEvent = 0;
    if (view === 'project') location.hash = 'overview';
    await poll();
  } catch (error) { showNotice(error.message, true); }
  finally { submitting = false; render(true); }
}

$('#run').addEventListener('click', () => startRun($('#suite').value));
$('.skip-link').addEventListener('click', (event) => { event.preventDefault(); $('#main').focus(); });
$('#stop').addEventListener('click', async () => {
  if (!state?.latest) return;
  try { const result = await api('/api/stop', {id:state.latest.id}); showNotice(result.message); await poll(); }
  catch (error) { showNotice(error.message, true); }
});
document.querySelectorAll('[data-source]').forEach((button) => button.addEventListener('click', () => { source = button.dataset.source; selectedEvent = 0; render(true); }));
$('#content').addEventListener('click', (event) => {
  const runButton = event.target.closest('[data-run]');
  if (runButton) startRun(runButton.dataset.run);
  const traceButton = event.target.closest('[data-event]');
  if (traceButton) {
    selectedEvent = Number(traceButton.dataset.event);
    document.querySelectorAll('[data-event]').forEach((button) => button.setAttribute('aria-pressed', String(Number(button.dataset.event) === selectedEvent)));
    $('#trace-detail').innerHTML = eventDetail(resultFor('demo'));
  }
});
$('#content').addEventListener('input', (event) => {
  if (event.target.id === 'test-search') { testQuery = event.target.value; $('#test-rows').innerHTML = testRows(resultFor('test')); }
});
$('#content').addEventListener('change', (event) => {
  if (event.target.id === 'test-group') { testGroup = event.target.value; $('#test-rows').innerHTML = testRows(resultFor('test')); }
  if (event.target.id === 'trace-filter') { traceFilter = event.target.value; render(true); }
});
function navigate() { view = Object.hasOwn(pages, location.hash.slice(1)) ? location.hash.slice(1) : 'overview'; if (state) render(true); }
window.addEventListener('hashchange', navigate);
navigate(); poll();
