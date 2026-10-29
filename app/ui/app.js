'use strict';

// LocoNet 6040 Studio: the main window. The layout is edited here and saved, read and sent
// through the commands in src-tauri/src/main.rs.

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;
const dialog = window.__TAURI__.dialog;

const st = {
  layout: null,
  path: null, // where the layout is saved, or null
  dirty: false,
  keyboard: 0, // the Keyboard shown
  pair: 0, // the key pair selected, 0 to 15
  onBus: [], // Keyboards the Nano found, from the last read
  ports: [],
  port: localStorage.getItem('port') || '',
  busy: false,
  settings: false, // the Settings window is open
  status: { text: 'Ready', kind: '' },
};

// ---------- helpers ----------

const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
const kindOf = (key) => (typeof key === 'string' ? key : Object.keys(key)[0]);
const numberOf = (key) => (typeof key === 'string' ? 0 : Object.values(key)[0]);
const makeKey = (kind, number) => (kind === 'nothing' ? 'nothing' : { [kind]: number });
const automaticKey = (kb, pair) => ({ turnout: kb * 16 + pair + 1 });
const effectiveKey = (kb, pair) => (kb.automatic ? automaticKey(kb.number, pair) : kb.keys[pair]);
const fileName = (path) => (path ? path.split(/[\\/]/).pop() : 'Not saved yet');

function keyboard(number = st.keyboard) {
  return st.layout.keyboards.find((k) => k.number === number);
}

function toast(text) {
  const el = document.getElementById('toast');
  el.textContent = text;
  el.classList.add('show');
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove('show'), 2600);
}

function setStatus(text, kind = '') {
  st.status = { text, kind };
  const el = document.querySelector('.status');
  if (el) {
    el.className = `status ${kind}`;
    el.textContent = text;
  }
}

function markDirty(dirty = true) {
  st.dirty = dirty;
  invoke('set_dirty', { dirty });
  const dot = document.querySelector('.top .dirty');
  if (dot) dot.style.display = dirty ? '' : 'none';
}

async function confirmDiscard() {
  if (!st.dirty) return true;
  return dialog.ask('The layout has changes that are not saved. Discard them?', {
    title: 'Unsaved changes',
    kind: 'warning',
    okLabel: 'Discard',
    cancelLabel: 'Keep',
  });
}

function newKeyboard(number) {
  return {
    number,
    automatic: true,
    keys: Array.from({ length: 16 }, (_, pair) => automaticKey(number, pair)),
    name: '',
    key_names: Array(16).fill(''),
  };
}

// ---------- rendering ----------

/// The same turnout as the Intellibox and as Rocrail's MADA, PADA and FADA write it, filled
/// in after each render (the formulas live in l6040-core, rocrail.rs).
async function fillAddresses() {
  const el = document.getElementById('addresses');
  const kb = st.layout && keyboard();
  if (!el || !kb) return;
  const key = effectiveKey(kb, st.pair);
  // For a track section or nothing the table is filled for turnout 1 but kept invisible,
  // so it still takes its room.
  const turnout = kindOf(key) === 'turnout' ? numberOf(key) : 1;
  const a = await invoke('rocrail_addresses', { turnout });
  if (!document.body.contains(el)) return;
  el.innerHTML = `
    <div class="addr-title">This turnout in other programs</div>
    <table class="addr">
      <tr><th>Intellibox, IB-Switch, 6040</th><td colspan="3">turnout <b>${a.turnout}</b></td></tr>
      <tr><th class="h">Rocrail</th><td class="h">Address</td><td class="h">Port</td><td class="h">Gate</td></tr>
      <tr><th>PADA <span class="muted">turnout number</span></th><td><b>0</b></td><td><b>${a.pada_port}</b></td><td></td></tr>
      <tr><th>MADA <span class="muted">decoder + port</span></th><td><b>${a.mada_address}</b></td><td><b>${a.mada_port}</b></td><td></td></tr>
      <tr><th>FADA <span class="muted">outputs from 0</span></th><td><b>${a.fada_address}</b></td><td><b>0</b></td><td><b>0</b></td></tr>
    </table>
    <div class="hint">The three Rocrail rows are the same turnout, written three ways. Fill in one of them in the turnout's Interface tab in Rocrail; PADA is the easiest, because its Port is the turnout number.</div>`;
}

function render() {
  const app = document.getElementById('app');
  app.innerHTML = `
    ${renderTop()}
    <div class="body">
      <main class="main">${renderMain()}</main>
    </div>
    ${st.settings ? renderSettings() : ''}
    <div class="status ${st.status.kind}">${esc(st.status.text)}</div>`;
  fillAddresses();
}

function renderTop() {
  const options = st.ports.length
    ? st.ports.map((p) => `<option value="${esc(p.name)}" ${p.name === st.port ? 'selected' : ''}>${esc(p.label)}</option>`).join('')
    : '<option value="">No USB port found</option>';
  const choose = st.ports.length && !st.port ? '<option value="" selected>Choose the Nano\'s port</option>' : '';
  return `
    <header class="top">
      <img class="mark" src="icon.png" alt="">
      <span class="title">LocoNet 6040 Studio</span>
      <span class="file">${esc(st.layout.name || fileName(st.path))}</span>
      <span class="dirty" style="${st.dirty ? '' : 'display:none'}" title="Not saved"></span>
      <span class="spacer"></span>
      <button class="btn" data-act="settings" title="The layout's name and the settings of the Nano">Settings</button>
      <select class="select port" data-act="port" title="The USB port of the Nano">${choose}${options}</select>
      <button class="btn" data-act="ports" title="Look for USB ports again">Refresh</button>
      <button class="btn" data-act="read" ${st.busy ? 'disabled' : ''}>Read from Nano</button>
      <button class="btn primary" data-act="send" ${st.busy ? 'disabled' : ''}>Send to Nano</button>
    </header>`;
}

/// The Settings window: the layout's name and the settings of the Nano, which most
/// layouts never change.
function renderSettings() {
  return `
    <div class="overlay" data-act="close-settings">
      <div class="sheet" role="dialog" aria-label="Settings">
        <section>
          <h2>Layout</h2>
          <label class="field">Name<input class="input" data-bind="layout-name" value="${esc(st.layout.name)}" placeholder="For example: Adorf station"></label>
        </section>
        ${renderOptions()}
        <div class="sheet-foot"><button class="btn primary" data-act="close-settings">Done</button></div>
      </div>
    </div>`;
}

/// The Keyboards as tabs above the Keyboard picture, with Add for the numbers still free.
function renderTabs() {
  const tabs = [...st.layout.keyboards]
    .sort((a, b) => a.number - b.number)
    .map((kb) => {
      const bus = st.onBus.includes(kb.number) ? '<span class="tab-bus" title="Found on the bus by the Nano"></span>' : '';
      return `
        <button class="tab ${kb.number === st.keyboard ? 'active' : ''}" data-act="pick-kb" data-kb="${kb.number}" title="Keyboard ${kb.number}">
          <span class="tab-num">${kb.number}</span>
          <span class="tab-name">${esc(kb.name || `Keyboard ${kb.number}`)}</span>${bus}
        </button>`;
    })
    .join('');
  const free = [...Array(16).keys()].filter((n) => !keyboard(n));
  const add = free.length
    ? `<span class="tab-add">
         <select class="select" id="add-kb" title="The number set with the switches on the back of the Keyboard">${free.map((n) => `<option value="${n}">Keyboard ${n}</option>`).join('')}</select>
         <button class="btn" data-act="add-kb">+ Add</button>
       </span>`
    : '';
  return `<nav class="tabs">${tabs}${add}</nav>`;
}

function renderOptions() {
  const o = st.layout.options;
  const box = (name, label, hint) => `
    <label class="option">
      <input type="checkbox" data-opt="${name}" ${o[name] ? 'checked' : ''}>
      <span><div class="label">${label}</div><div class="hint">${hint}</div></span>
    </label>`;
  return `
    <section>
      <h2>Settings of the Nano</h2>
      ${box('auto_assign', 'Automatic numbering', 'Keyboards not set up here switch 16 turnouts in a row: Keyboard 0 switches 1 to 16, Keyboard 1 17 to 32.')}
      ${box('hold_without_power', 'No keys without track power', 'A key is not sent while the track power is off.')}
      ${box('stop_go', 'Märklin STOP and GO', 'The STOP and GO lines of the Märklin bus switch the LocoNet track power. Only after measuring them.')}
      ${box('log', 'Log on the Serial Monitor', 'The Nano writes a line for every key and switch command.')}
      ${box('watchdog', 'Restart after a fault', 'The Nano restarts by itself if its program hangs.')}
      <div class="option">
        <span style="width:16px"></span>
        <span>
          <div class="label">Coil time</div>
          <div class="coil"><input class="input" type="number" min="0" max="12750" step="50" data-opt="coil_ms" value="${o.coil_ms}"> ms</div>
          <div class="hint">How long a turnout coil stays on (200 ms as on an IB-Switch). 0: until the key is released.</div>
        </span>
      </div>
    </section>`;
}

function renderMain() {
  const kb = keyboard();
  if (!kb) {
    return `${renderTabs()}<div class="empty">Add a Keyboard with <b>+ Add</b> above, or choose <b>Read from Nano</b> to take over the setup of your Nano.</div>`;
  }
  // As on the real Keyboard 6040: two rows of 8 key pairs, 1 to 8 on top, 9 to 16 below;
  // each pair is a red key with its red LED above it to the right, its number, and a green
  // key. The LED is lit for thrown (red) and off for closed (green); here it is off, as
  // the Studio does not know the turnouts' positions.
  const pairs = [0, 1]
    .map((row) => {
      const cells = [...Array(8).keys()]
        .map((i) => {
          const pair = row * 8 + i;
          const key = effectiveKey(kb, pair);
          const kind = kindOf(key);
          const chip = kind === 'turnout' ? `<span class="chip">${numberOf(key)}</span>`
            : kind === 'sensor' ? `<span class="chip sensor">s${numberOf(key)}</span>`
            : '<span class="chip none">—</span>';
          return `
            <div class="pair ${pair === st.pair ? 'sel' : ''} ${kind === 'nothing' ? 'off' : ''}" data-act="pick-pair" data-pair="${pair}" title="${esc(kb.key_names[pair])}">
              <span class="key-wrap"><span class="led" title="Lit when the turnout is thrown (red)"></span><span class="key red"></span></span>
              <span class="no">${pair + 1}</span>
              <span class="key green"></span>
              ${chip}
              <span class="name">${esc(kb.key_names[pair])}</span>
            </div>`;
        })
        .join('');
      return `<div class="krow">${cells}</div>`;
    })
    .join('');
  const first = kb.number * 16 + 1;
  return `
    ${renderTabs()}
    <div class="kb-head">
      <h1>Keyboard ${kb.number}</h1>
      <label class="field">Name<input class="input" data-bind="kb-name" value="${esc(kb.name)}" placeholder="For example: Adorf station"></label>
      <div class="mode" role="group">
        <button class="${kb.automatic ? 'on' : ''}" data-act="mode" data-auto="1">Automatic</button>
        <button class="${kb.automatic ? '' : 'on'}" data-act="mode" data-auto="0">Own assignment</button>
      </div>
      <button class="btn danger" data-act="remove-kb">Remove</button>
    </div>
    ${kb.automatic ? `<div class="notice">Automatic: this Keyboard switches turnouts ${first} to ${first + 15}. Choose <b>Own assignment</b> to change what its keys do.</div>` : ''}
    <div class="work">
    <div class="k6040">
      <div class="face">
        <span class="screw tl"></span><span class="screw tr"></span><span class="screw bl"></span><span class="screw br"></span>
        <div class="face-top"><span class="window">${esc(kb.name)}</span><span class="face-no">No. ${kb.number}</span></div>
        <div class="pairs">${pairs}</div>
      </div>
      <div class="base"><span>keyboard</span><span class="muted-ink">red: thrown · green: closed</span></div>
    </div>
    ${renderInspector(kb)}
    </div>`;
}

function renderInspector(kb) {
  const pair = st.pair;
  const key = effectiveKey(kb, pair);
  const kind = kindOf(key);
  const disabled = kb.automatic ? 'disabled' : '';
  const radio = (value, label) => `
    <label class="kind ${kind === value ? 'on' : ''}">
      <input type="radio" name="kind" value="${value}" data-act="kind" ${kind === value ? 'checked' : ''} ${disabled}> ${label}
    </label>`;
  const explain = {
    turnout: 'The red key switches the turnout to <b>thrown</b> (curved), the green key to <b>closed</b> (straight). Use the number you would type on the Intellibox.',
    sensor: 'This key pair switches nothing. Its lamps show the track section: <b>red</b> when a train is there, <b>green</b> when it is free.',
    nothing: 'This key pair does nothing.',
  }[kind];
  // Every kind takes the room of a turnout, the largest, so the panel never changes height.
  const number = kind === 'nothing'
    ? '<label class="field reserve">Turnout<input class="input" disabled></label>'
    : `
    <label class="field">${kind === 'turnout' ? 'Turnout (1 to 2048)' : 'Track section (1 to 4096)'}
      <input class="input" type="number" min="1" max="${kind === 'turnout' ? 2048 : 4096}" data-bind="key-number" value="${numberOf(key)}" ${disabled}>
    </label>`;
  return `
    <section class="inspector">
      <h3>Key pair ${pair + 1}</h3>
      <div class="kinds">${radio('turnout', 'Switches a turnout')}${radio('sensor', 'Shows a track section')}${radio('nothing', 'Does nothing')}</div>
      <div class="row">
        ${number}
        <label class="field">Name (only in this app)<input class="input" data-bind="key-name" value="${esc(kb.key_names[pair])}" placeholder="For example: Entry track 1"></label>
      </div>
      <div class="explain">${explain}</div>
      <div class="addresses ${kind === 'turnout' ? '' : 'reserve'}" id="addresses"></div>
    </section>`;
}

// ---------- editing ----------

document.addEventListener('click', async (ev) => {
  const el = ev.target.closest('[data-act]');
  if (!el || el.tagName === 'SELECT' || el.tagName === 'INPUT') return;
  const act = el.dataset.act;
  if (act === 'settings') {
    st.settings = true;
    render();
  } else if (act === 'close-settings') {
    if (el.classList.contains('overlay') && ev.target !== el) return; // a click inside the window
    st.settings = false;
    render();
  } else if (act === 'pick-kb') {
    st.keyboard = Number(el.dataset.kb);
    st.pair = 0;
    render();
  } else if (act === 'pick-pair') {
    st.pair = Number(el.dataset.pair);
    render();
  } else if (act === 'add-kb') {
    const number = Number(document.getElementById('add-kb').value);
    st.layout.keyboards.push(newKeyboard(number));
    st.layout.keyboards.sort((a, b) => a.number - b.number);
    st.keyboard = number;
    markDirty();
    render();
  } else if (act === 'remove-kb') {
    const ok = await dialog.ask(`Remove Keyboard ${st.keyboard} from the layout? On the Nano it goes back to automatic numbering.`, { title: 'Remove Keyboard', kind: 'warning' });
    if (!ok) return;
    st.layout.keyboards = st.layout.keyboards.filter((k) => k.number !== st.keyboard);
    st.keyboard = st.layout.keyboards[0]?.number ?? 0;
    markDirty();
    render();
  } else if (act === 'mode') {
    const kb = keyboard();
    const automatic = el.dataset.auto === '1';
    if (kb.automatic === automatic) return;
    // Own assignment starts from what the Keyboard switched so far.
    if (!automatic) kb.keys = kb.keys.map((_, pair) => effectiveKey(kb, pair));
    kb.automatic = automatic;
    markDirty();
    render();
  } else if (act === 'ports') {
    await refreshPorts();
  } else {
    await command(act);
  }
});

document.addEventListener('change', (ev) => {
  const el = ev.target;
  if (el.dataset.act === 'port') {
    st.port = el.value;
    localStorage.setItem('port', st.port);
  } else if (el.dataset.act === 'kind') {
    const kb = keyboard();
    const old = kb.keys[st.pair];
    const number = numberOf(old) || (el.value === 'turnout' ? kb.number * 16 + st.pair + 1 : 1);
    kb.keys[st.pair] = makeKey(el.value, number);
    markDirty();
    render();
  } else if (el.dataset.opt) {
    const o = st.layout.options;
    if (el.type === 'checkbox') o[el.dataset.opt] = el.checked;
    else {
      const ms = Math.max(0, Math.min(12750, Math.round(Number(el.value) / 50) * 50 || 0));
      o.coil_ms = ms;
      el.value = ms;
    }
    markDirty();
  } else if (el.dataset.bind === 'key-number') {
    const kb = keyboard();
    const kind = kindOf(kb.keys[st.pair]);
    const max = kind === 'turnout' ? 2048 : 4096;
    const number = Math.max(1, Math.min(max, Math.round(Number(el.value)) || 1));
    kb.keys[st.pair] = makeKey(kind, number);
    markDirty();
    render();
  }
});

document.addEventListener('input', (ev) => {
  const el = ev.target;
  if (el.dataset.bind === 'layout-name') {
    st.layout.name = el.value;
    document.querySelector('.top .file').textContent = el.value || fileName(st.path);
    markDirty();
  } else if (el.dataset.bind === 'kb-name') {
    keyboard().name = el.value;
    const tab = document.querySelector(`.tab[data-kb="${st.keyboard}"] .tab-name`);
    if (tab) tab.textContent = el.value || `Keyboard ${st.keyboard}`;
    const label = document.querySelector('.face .window');
    if (label) label.textContent = el.value;
    markDirty();
  } else if (el.dataset.bind === 'key-name') {
    keyboard().key_names[st.pair] = el.value;
    const name = document.querySelector(`.pair[data-pair="${st.pair}"] .name`);
    if (name) name.textContent = el.value;
    markDirty();
  }
});

// ---------- files and the Nano ----------

const LAYOUT_FILTER = [{ name: 'LocoNet 6040 Studio layout', extensions: ['json'] }];
const BACKUP_FILTER = [{ name: 'Backup', extensions: ['txt'] }];

async function save(as) {
  let path = st.path;
  if (as || !path) {
    path = await dialog.save({ defaultPath: `${st.layout.name || 'Keyboards'}.json`, filters: LAYOUT_FILTER });
    if (!path) return false;
  }
  await invoke('save_layout', { path, layout: st.layout });
  st.path = path;
  markDirty(false);
  render();
  toast('Saved');
  return true;
}

/// Takes over a layout read from the Nano or a backup, keeping the names this layout
/// gave its Keyboards and key pairs.
function takeOver(parsed) {
  const names = new Map(st.layout.keyboards.map((k) => [k.number, k]));
  for (const kb of parsed.layout.keyboards) {
    const old = names.get(kb.number);
    if (old) {
      kb.name = old.name;
      kb.key_names = old.key_names;
    }
  }
  parsed.layout.name = st.layout.name;
  parsed.layout.notes = st.layout.notes;
  st.layout = parsed.layout;
  st.onBus = parsed.on_the_bus;
  st.keyboard = st.layout.keyboards[0]?.number ?? 0;
  st.pair = 0;
}

async function command(act) {
  try {
    if (act === 'new') {
      if (!(await confirmDiscard())) return;
      st.layout = await invoke('new_layout');
      st.path = null;
      st.keyboard = 0;
      st.pair = 0;
      st.onBus = [];
      markDirty(false);
      render();
    } else if (act === 'open') {
      if (!(await confirmDiscard())) return;
      const path = await dialog.open({ filters: LAYOUT_FILTER });
      if (!path) return;
      st.layout = await invoke('open_layout', { path });
      st.path = path;
      st.keyboard = st.layout.keyboards[0]?.number ?? 0;
      st.pair = 0;
      st.onBus = [];
      markDirty(false);
      render();
      setStatus(`Opened ${fileName(path)}`);
    } else if (act === 'save') {
      await save(false);
    } else if (act === 'save-as') {
      await save(true);
    } else if (act === 'import-backup') {
      const path = await dialog.open({ filters: BACKUP_FILTER });
      if (!path) return;
      const parsed = await invoke('import_backup', { path });
      takeOver(parsed);
      markDirty();
      render();
      report(parsed, `Imported ${fileName(path)}`);
    } else if (act === 'export-backup') {
      const path = await dialog.save({ defaultPath: `${st.layout.name || 'Keyboards'} backup.txt`, filters: BACKUP_FILTER });
      if (!path) return;
      await invoke('export_backup', { path, layout: st.layout });
      toast('Backup exported');
    } else if (act === 'export-keymap') {
      const path = await dialog.save({ defaultPath: 'KeyMap.h', filters: [{ name: 'KeyMap.h', extensions: ['h'] }] });
      if (!path) return;
      await invoke('export_keymap', { path, layout: st.layout });
      toast('KeyMap.h exported');
    } else if (act === 'read') {
      await readFromNano();
    } else if (act === 'send') {
      await sendToNano();
    }
  } catch (error) {
    setStatus(String(error), 'bad');
    await dialog.message(String(error), { title: 'LocoNet 6040 Studio', kind: 'error' });
  }
}

function report(parsed, done) {
  if (parsed.problems.length) {
    setStatus(`${done}, but ${parsed.problems.length} line(s) could not be read`, 'bad');
    dialog.message(parsed.problems.join('\n'), { title: 'Some lines could not be read', kind: 'warning' });
  } else {
    setStatus(done, 'good');
  }
}

function needPort() {
  if (st.port) return true;
  dialog.message('Connect the Nano with its USB cable, press Refresh and choose its port at the top right.', { title: 'No USB port chosen', kind: 'info' });
  return false;
}

async function busy(text, work) {
  st.busy = true;
  render();
  setStatus(text, 'busy');
  try {
    return await work();
  } finally {
    st.busy = false;
    render();
  }
}

async function readFromNano() {
  if (!needPort()) return;
  if (!(await confirmDiscard())) return;
  const result = await busy('Reading from the Nano… (it restarts when it is connected)', () => invoke('read_from_nano', { port: st.port }));
  takeOver(result.parsed);
  markDirty();
  render();
  report(result.parsed, `Read from the Nano (program version ${result.firmware || 'unknown'})`);
}

async function sendToNano() {
  if (!needPort()) return;
  const problems = await invoke('problems', { layout: st.layout });
  if (problems.length) {
    await dialog.message(problems.join('\n'), { title: 'Please correct this first', kind: 'warning' });
    return;
  }
  const ok = await dialog.ask('Send this setup to the Nano? It replaces the setup on the Nano.', { title: 'Send to Nano', okLabel: 'Send', cancelLabel: 'Cancel' });
  if (!ok) return;
  const stop = await listen('send-progress', (ev) => setStatus(`Sending to the Nano… ${ev.payload.done} of ${ev.payload.total}`, 'busy'));
  try {
    const result = await busy('Connecting to the Nano… (it restarts when it is connected)', () => invoke('send_to_nano', { port: st.port, layout: st.layout }));
    if (result.differences.length) {
      setStatus('Sent, but the Nano differs from the layout', 'bad');
      await dialog.message(result.differences.join('\n'), { title: 'The Nano differs from the layout', kind: 'warning' });
    } else {
      setStatus('Sent to the Nano and checked: the Nano has this setup', 'good');
      toast('Sent and checked');
    }
  } finally {
    stop();
  }
}

async function refreshPorts() {
  st.ports = await invoke('list_ports');
  if (!st.ports.some((p) => p.name === st.port)) st.port = st.ports.find((p) => p.usb)?.name || '';
  render();
}

// ---------- start ----------

listen('menu', (ev) => command(ev.payload));
listen('close-requested', async () => {
  if (await confirmDiscard()) invoke('close_window');
});
document.addEventListener('keydown', (ev) => {
  if (ev.key === 'Escape' && st.settings) {
    st.settings = false;
    render();
  }
});
document.addEventListener('contextmenu', (ev) => {
  if (!ev.target.closest('input')) ev.preventDefault();
});

(async () => {
  st.layout = await invoke('new_layout');
  const path = await invoke('startup_file');
  if (path) {
    try {
      st.layout = await invoke('open_layout', { path });
      st.path = path;
      st.keyboard = st.layout.keyboards[0]?.number ?? 0;
    } catch (error) {
      setStatus(String(error), 'bad');
    }
  }
  st.ports = await invoke('list_ports');
  if (!st.ports.some((p) => p.name === st.port)) st.port = st.ports.find((p) => p.usb)?.name || '';
  render();
})();
