'use strict';

// This surface renders only state returned by the live guest runner.
let guest = null;
let token = '';
let connected = false;
let pending = false;
let polling = false;
let revision = 0;
let activeTab = 'files';
const history = [];
let historyIndex = 0;
const rendered = new Map();
const $ = (id) => document.getElementById(id);
const escapeHtml = (value) => String(value ?? '').replace(/[&<>"']/g, (character) => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[character]));
const safeName = (value) => /^[A-Za-z0-9][A-Za-z0-9._-]{0,47}$/.test(value);
const text = (value) => escapeHtml(value);

async function api(path, body) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), path.endsWith('/stop') ? 30000 : 12000);
  try {
    const response = await fetch(path, {
      method: body === undefined ? 'GET' : 'POST',
      headers: body === undefined ? {} : {'Content-Type':'application/json', 'X-Mnemosyne-Token':token},
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: controller.signal,
      cache: 'no-store',
    });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || `Host returned HTTP ${response.status}.`);
    return result;
  } catch (error) {
    if (error.name === 'AbortError') throw new Error('The host did not respond within 12 seconds. Reconnecting automatically; inspect the console before retrying a command.');
    throw error;
  } finally { clearTimeout(timeout); }
}

function notice(message) {
  $('notice').textContent = message || '';
  $('notice').hidden = !message;
}

function replaceIfChanged(id, html) {
  if (rendered.get(id) === html) return;
  const element = $(id);
  const focused = document.activeElement;
  const focusCommand = element.contains(focused) ? focused.dataset.command : null;
  const scrollTop = element.scrollTop;
  element.innerHTML = html;
  element.scrollTop = scrollTop;
  rendered.set(id, html);
  if (focusCommand) {
    const replacement = [...element.querySelectorAll('[data-command]')].find((button) => button.dataset.command === focusCommand);
    replacement?.focus({preventScroll:true});
  }
}

function renderCollections() {
  const files = Array.isArray(guest?.files) ? guest.files : [];
  replaceIfChanged('files-list', files.length ? files.map((file) => `<div class="data-row"><span><code>${text(file.name)}</code> <small>${text(file.bytes)} B</small></span><span class="row-actions"><button data-command="cat ${text(file.name)}" aria-label="Read ${text(file.name)}">Read</button><button data-edit="${text(file.name)}" aria-label="Edit ${text(file.name)}">Edit</button></span></div>`).join('') : `<p class="empty">${guest?.status === 'stopped' ? 'Start the OS to inspect its filesystem.' : 'No files reported by the guest. Write a file below, or refresh the listing.'}</p>`);
  const snapshots = Array.isArray(guest?.snapshots) ? guest.snapshots : [];
  replaceIfChanged('snapshots-list', snapshots.length ? snapshots.map((snapshot) => `<div class="data-row"><span><strong>${text(snapshot.label)}</strong><br><small>Generation ${text(snapshot.generation)} · ${text(snapshot.bytes)} B</small></span><span class="row-actions"><button data-command="restore ${text(snapshot.generation)}" aria-label="Restore snapshot ${text(snapshot.label)}, generation ${text(snapshot.generation)}">Restore</button></span></div>`).join('') : '<p class="empty">No snapshots reported. Save one after writing a file.</p>');
  const tasks = Array.isArray(guest?.tasks) ? guest.tasks : [];
  replaceIfChanged('tasks-list', tasks.length ? `<table><thead><tr><th>Task</th><th>State</th><th>Switches</th><th>Faults</th><th>Epoch</th></tr></thead><tbody>${tasks.map((task) => `<tr><td>${text(task.name)}<br><small>PID ${text(task.pid)}</small></td><td>${text(task.state)}</td><td title="Preemptions: ${text(task.preemptions)}">${text(task.switches)}</td><td>${text(task.faults)}</td><td>${text(task.epoch)}</td></tr>`).join('')}</tbody></table>` : '<p class="empty">No task records yet. Start the OS, then refresh.</p>');
  const events = Array.isArray(guest?.events) ? guest.events : [];
  const recent = events.slice(-80).reverse();
  // Retain expanded disclosures across polls and append new records only when changed.
  const eventSignature = JSON.stringify(recent);
  if (rendered.get('event-signature') !== eventSignature) {
    const open = new Set([...$('events-list').querySelectorAll('details[open]')].map((item) => item.dataset.record));
    $('events-list').innerHTML = recent.length ? recent.map((event, index) => {
      const record = String(events.length - index);
      return `<details class="event-item" data-record="${record}" ${open.has(record) ? 'open' : ''}><summary>${text(record)}. ${text(event.event || event.type || 'Guest record')}</summary><pre>${text(JSON.stringify(event, null, 2))}</pre></details>`;
    }).join('') : '<p class="empty">Guest events will appear after boot.</p>';
    rendered.set('event-signature', eventSignature);
  }
  const samples = events.filter((event) => event.event === 'benchmark-sample');
  replaceIfChanged('benchmarks-list', samples.length ? `<h3 class="section-title">Guest benchmark samples</h3><p class="hint">Elapsed ticks are guest timer observations, at 10 ms resolution. Zero ticks means below one tick, not zero execution time.</p><div class="data-region"><table><thead><tr><th>Benchmark</th><th>Sample</th><th>Operations</th><th>Ticks</th></tr></thead><tbody>${samples.slice(-100).map((sample) => `<tr><td>${text(sample.benchmark)}</td><td>${text(sample.sample)}</td><td>${text(sample.operations)}</td><td>${text(sample['elapsed-ticks'] ?? sample.elapsed_ticks)}</td></tr>`).join('')}</tbody></table></div>` : '');
}

function render() {
  const status = guest?.status || 'unknown';
  const busy = pending || Boolean(guest?.busy) || ['starting','stopping','busy'].includes(status);
  const ready = connected && !busy && status === 'ready';
  $('connection').textContent = connected ? 'Host connected' : 'Host disconnected';
  $('connection-dot').classList.toggle('online', connected);
  $('guest-status').textContent = connected ? ({stopped:'OS stopped', starting:'Booting OS', stopping:'Stopping OS', ready:'OS ready', busy:'Guest working', error:'Guest error'}[status] || status) : 'Disconnected';
  $('guest-status').className = `status-pill ${status}`;
  $('busy-label').textContent = pending ? 'Sending request…' : busy ? 'Waiting for guest…' : '';
  $('start').disabled = !connected || busy || !['stopped','error'].includes(status);
  // Stop stays available while a guest command runs so an unresponsive guest can be terminated.
  $('stop').disabled = !connected || pending || ['stopped','stopping','unknown'].includes(status);
  for (const button of document.querySelectorAll('[data-command], [data-edit], #file-form button, #snapshot-form button, #plan-form button, #candidate-form button, #send')) button.disabled = !ready;
  $('command').disabled = !ready;
  $('last-command').textContent = guest?.last_command ? `Last command: ${guest.last_command}` : 'No command sent';
  const log = guest?.log || (status === 'stopped' ? 'The OS is stopped. Select Start OS to boot the kernel.' : 'Waiting for guest serial output…');
  const displayed = `${guest?.log_truncated ? '[Earlier serial output truncated by the host runner]\n' : ''}${log}`;
  if ($('serial-log').textContent !== displayed) {
    $('serial-log').textContent = displayed;
    if ($('follow').checked) $('serial-log').scrollTop = $('serial-log').scrollHeight;
  }
  const result = guest?.last_result;
  $('result-status').textContent = busy ? 'Guest operation in progress. Serial output is live.' : result ? `${result.ok ? 'Completed' : 'Command failed'}: ${result.command}` : status === 'ready' ? 'Guest ready. Choose an action or enter a command.' : 'Commands become available when the guest is ready.';
  $('result-status').classList.toggle('failed', Boolean(result && !result.ok));
  if (guest?.error) notice(guest.error);
  renderCollections();
  // Freshly inserted row controls need the same state as existing controls.
  for (const button of document.querySelectorAll('[data-command], [data-edit]')) button.disabled = !ready;
}

async function poll() {
  if (polling || pending) return;
  polling = true;
  const observedRevision = revision;
  try {
    const update = await api('/api/guest');
    if (observedRevision !== revision) return;
    guest = update;
    token = update.token || token;
    if (!connected) notice('');
    connected = true;
    render();
  } catch (error) {
    if (observedRevision !== revision) return;
    connected = false;
    notice(`${error.message} Check that the dashboard host is still running.`);
    render();
  } finally { polling = false; }
}

async function mutate(action, body = {}) {
  if (pending || !connected) return false;
  pending = true;
  revision += 1;
  notice('');
  render();
  try {
    guest = await api(`/api/guest/${action}`, body);
    token = guest.token || token;
    return true;
  } catch (error) {
    notice(error.message);
    return false;
  } finally {
    pending = false;
    render();
    // Reconcile accepted operations and errors with authoritative host state.
    setTimeout(poll, 150);
  }
}

async function command(value) {
  const commandText = value.trim();
  if (!commandText || commandText.length > 480 || !/^[\x20-\x7e]+$/.test(commandText)) {
    notice('Use a single ASCII command of at most 480 characters, including its filename and contents.');
    return false;
  }
  if (!connected || pending || guest?.busy || guest?.status !== 'ready') return false;
  if (history.at(-1) !== commandText) history.push(commandText);
  historyIndex = history.length;
  return mutate('command', {command:commandText});
}

$('start').addEventListener('click', () => mutate('start'));
$('stop').addEventListener('click', () => mutate('stop'));
$('command-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const input = $('command');
  const accepted = await command(input.value);
  if (accepted) input.value = '';
});
$('command').addEventListener('keydown', (event) => {
  if (!['ArrowUp','ArrowDown'].includes(event.key) || !history.length) return;
  event.preventDefault();
  historyIndex = Math.min(history.length, Math.max(0, historyIndex + (event.key === 'ArrowUp' ? -1 : 1)));
  $('command').value = history[historyIndex] || '';
});
$('file-form').addEventListener('submit', (event) => {
  event.preventDefault();
  const name = $('file-name').value.trim();
  const contents = $('file-content').value;
  if (!safeName(name) || !/^[\x20-\x7e]+$/.test(contents)) return notice('Use a filename with letters, numbers, dots, underscores or hyphens, and single-line ASCII contents.');
  command(`write ${name} ${contents}`);
});
$('snapshot-form').addEventListener('submit', (event) => {
  event.preventDefault();
  const label = $('snapshot-label').value.trim();
  if (!safeName(label)) return notice('Use a snapshot label with letters, numbers, dots, underscores or hyphens.');
  command(`snapshot ${label}`);
});
$('plan-form').addEventListener('submit', (event) => {
  event.preventDefault();
  const name = $('plan-name').value.trim();
  if (!safeName(name)) return notice('Enter a guest plan filename.');
  command(`verify-plan ${name}`);
});
$('candidate-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const name = $('candidate-name').value.trim();
  if (!safeName(name)) return notice('Use a filename that starts with a letter or number.');
  let contents = $('candidate-content').value.trim();
  // Compact valid formatted JSON for the guest's single-line serial shell.
  try { contents = JSON.stringify(JSON.parse(contents)); } catch { /* The verifier also accepts malformed candidates for rejection tests. */ }
  const accepted = await command(`write ${name} ${contents}`);
  if (accepted) $('plan-name').value = name;
});
document.addEventListener('click', (event) => {
  const tab = event.target.closest('[data-tab]');
  if (tab) {
    activeTab = tab.dataset.tab;
    document.querySelectorAll('[data-tab]').forEach((button) => button === tab ? button.setAttribute('aria-current','page') : button.removeAttribute('aria-current'));
    document.querySelectorAll('.view').forEach((view) => { view.hidden = view.id !== `view-${activeTab}`; });
    return;
  }
  const edit = event.target.closest('[data-edit]');
  if (edit && !edit.disabled) {
    $('file-name').value = edit.dataset.edit;
    $('file-content').value = '';
    $('file-content').focus();
    notice(`Editing ${edit.dataset.edit}. Read the file in the serial console first if you need its current contents. Writing replaces the file.`);
    return;
  }
  const action = event.target.closest('[data-command]');
  if (action && !action.disabled) command(action.dataset.command);
});
$('follow').addEventListener('change', () => { if ($('follow').checked) $('serial-log').scrollTop = $('serial-log').scrollHeight; });
$('download-log').addEventListener('click', () => {
  const url = URL.createObjectURL(new Blob([guest?.log || ''], {type:'text/plain;charset=utf-8'}));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'mnemosyne-guest-serial.log';
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});

async function initialize() {
  try {
    const bootstrap = await api('/api/state');
    token = bootstrap.token || '';
  } catch (error) { notice(error.message); }
  await poll();
  setInterval(poll, 900);
}
render();
initialize();
