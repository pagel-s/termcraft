const $ = id => document.getElementById(id);
const canvas = $('game');
const context = canvas.getContext('2d', { alpha: false });
const CELL_W = 8, CELL_H = 16, COLUMNS = 200, ROWS = 60;
const SAVE_KEY = 'lantern-run-v1';
let worker, paused = true, started = false, busy = false, inFlight = false;
let previousCells, lastFrame = 0, sequence = 1, database, lockHeld = false;
let savePromise, aim, lastAim = '', lastDrag = '', buttons = 0;
const pending = new Map(), keys = new Set();
canvas.width = COLUMNS * CELL_W;
canvas.height = ROWS * CELL_H;
context.font = '13px monospace';
context.textBaseline = 'top';
context.imageSmoothingEnabled = false;

function notice(message) { $('notice').textContent = message; $('notice').hidden = !message; }
function errorText(error) { return String(error?.message ?? error); }
function databaseOpen() {
  if (database) return database;
  database = new Promise((resolve, reject) => {
    const request = indexedDB.open('termcraft-browser', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('saves');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('Close other Termcraft tabs to enable saving.'));
  });
  return database;
}
async function storedSave() {
  const db = await databaseOpen();
  return new Promise((resolve, reject) => {
    const transaction = db.transaction('saves', 'readonly');
    const request = transaction.objectStore('saves').get(SAVE_KEY);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
async function storeSave(snapshot) {
  const db = await databaseOpen();
  return new Promise((resolve, reject) => {
    const transaction = db.transaction('saves', 'readwrite');
    transaction.objectStore('saves').put({ snapshot, savedAt: Date.now() }, SAVE_KEY);
    transaction.oncomplete = resolve;
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error ?? new Error('Save transaction aborted.'));
  });
}
async function claimSave() {
  if (lockHeld) return;
  if (!navigator.locks) throw new Error('This browser cannot safely lock a local save. Please use a current desktop browser over HTTPS.');
  await new Promise((resolve, reject) => {
    navigator.locks.request('termcraft-browser-lantern-run', { ifAvailable: true }, async lock => {
      if (!lock) { reject(new Error('Another Termcraft tab is using this save. Close that tab, then press Play again.')); return; }
      lockHeld = true;
      resolve();
      await new Promise(() => {}); // Released automatically when this document closes.
    }).catch(reject);
  });
}
function send(type, data = {}) { worker?.postMessage({ type, ...data }); }
function request(type, data = {}) {
  return new Promise((resolve, reject) => {
    if (!worker) { reject(new Error('The game has not started.')); return; }
    const id = sequence++;
    pending.set(id, { resolve, reject });
    worker.postMessage({ id, type, ...data });
  });
}
function colour(value) { return `#${value.toString(16).padStart(6, '0')}`; }
const quadrants = new Map([[0x2596,4],[0x2597,8],[0x2598,1],[0x2599,13],[0x259a,9],[0x259b,7],[0x259c,11],[0x259d,2],[0x259e,6],[0x259f,14]]);
function paint(cells) {
  if (cells.length !== COLUMNS * ROWS * 3) throw new Error('Invalid game frame.');
  for (let p = 0, cell = 0; p < cells.length; p += 3, cell++) {
    if (previousCells && cells[p] === previousCells[p] && cells[p+1] === previousCells[p+1] && cells[p+2] === previousCells[p+2]) continue;
    const x = cell % COLUMNS * CELL_W, y = Math.floor(cell / COLUMNS) * CELL_H;
    const character = cells[p];
    context.fillStyle = colour(cells[p+2]);
    context.fillRect(x,y,CELL_W,CELL_H);
    if (character === 32) continue;
    context.fillStyle = colour(cells[p+1]);
    if (character === 0x2580) context.fillRect(x,y,CELL_W,CELL_H/2);
    else if (character === 0x2584) context.fillRect(x,y+CELL_H/2,CELL_W,CELL_H/2);
    else if (character === 0x2588) context.fillRect(x,y,CELL_W,CELL_H);
    else if (character === 0x258c) context.fillRect(x,y,CELL_W/2,CELL_H);
    else if (character === 0x2590) context.fillRect(x+CELL_W/2,y,CELL_W/2,CELL_H);
    else if (quadrants.has(character)) {
      const bits = quadrants.get(character);
      for (let q=0;q<4;q++) if (bits & (1<<q)) context.fillRect(x+(q%2)*CELL_W/2,y+Math.floor(q/2)*CELL_H/2,CELL_W/2,CELL_H/2);
    } else context.fillText(String.fromCodePoint(character),x,y+1,CELL_W);
  }
  previousCells = cells;
}
function releaseControls() {
  keys.clear(); buttons = 0; aim = undefined; lastDrag = '';
  send('release');
}
function cover(title, message) {
  $('cover').hidden = false;
  $('cover').querySelector('h1').textContent = title;
  $('loading').textContent = message;
}
async function save() {
  if (!started || !worker) return;
  if (savePromise) return savePromise;
  const current = worker;
  savePromise = (async () => {
    $('save-status').textContent = 'Saving…';
    const snapshot = await request('save');
    if (worker !== current) return snapshot;
    await storeSave(snapshot);
    $('save-status').textContent = 'Saved on this browser';
    return snapshot;
  })().catch(error => {
    $('save-status').textContent = 'Save failed — export a backup';
    notice(`Your previous saved game is retained. ${errorText(error)} Use Export save to keep this session.`);
    throw error;
  }).finally(() => { savePromise = undefined; });
  return savePromise;
}
function pause() {
  if (!started || paused) return;
  paused = true; releaseControls();
  $('pause').textContent = 'Resume';
  $('play').textContent = 'Resume adventure →';
  cover('Take a breath.', 'Your adventure stays on this browser.');
  save().catch(() => {});
}
function resume() {
  paused = false; lastFrame = 0; releaseControls();
  $('cover').hidden = true; $('pause').textContent = 'Pause';
  canvas.focus({ preventScroll: true });
}
function frame(now) {
  requestAnimationFrame(frame);
  if (paused || !started || inFlight || !worker) return;
  if (aim) {
    const tag = `${aim.x},${aim.y}`;
    if (tag !== lastAim) { send('aim',aim); lastAim = tag; }
  }
  const elapsed = lastFrame ? Math.min(now - lastFrame,250) : 0;
  lastFrame = now; inFlight = true;
  request('frame',{ elapsed }).catch(fatal).finally(() => { inFlight = false; });
}
function fatal(error) {
  paused = true; started = false;
  keys.clear(); buttons = 0;
  worker?.terminate(); worker = undefined;
  const abandoned = [...pending.values()]; pending.clear();
  for (const entry of abandoned) entry.reject(error);
  $('pause').disabled = true; $('export').disabled = true;
  $('play').disabled = false; $('play').textContent = 'Try again →';
  cover('Something interrupted play.', 'Your last completed save is retained.');
  notice(`${errorText(error)} You can retry or download the native game above.`);
}
async function boot(snapshot) {
  if (busy) return;
  busy = true; paused = true;
  $('play').disabled = true; $('loading').textContent = 'Loading the Rust game engine…';
  notice('');
  try {
    await claimSave();
    if (savePromise) await savePromise;
    started = false;
    worker?.terminate();
    for (const entry of pending.values()) entry.reject(new Error('Game restarted.'));
    pending.clear(); previousCells = undefined; inFlight = false;
    const current = new Worker(new URL('./worker.js',import.meta.url), { type: 'module' });
    worker = current;
    current.onmessage = ({data}) => {
      if (worker !== current) return;
      const entry = pending.get(data.id);
      if (entry) pending.delete(data.id);
      if (data.type === 'error') { if (entry) entry.reject(new Error(data.message)); else fatal(new Error(data.message)); return; }
      if (data.type === 'frame') {
        paint(data.cells);
        $('objective').textContent = data.status.objective ?? 'Keep exploring.';
        canvas.dataset.playerX = data.status.x;
        canvas.dataset.playerY = data.status.y;
        canvas.dataset.health = data.status.health;
        canvas.dataset.inventoryOpen = data.status.inventory_open;
        canvas.dataset.ready = 'true';
      }
      entry?.resolve(data.result);
    };
    current.onerror = event => {
      const error = new Error(event.message || 'The browser could not load the game engine.');
      for (const entry of pending.values()) entry.reject(error);
      pending.clear(); fatal(error);
    };
    await request('start',{columns:COLUMNS,rows:ROWS,saved:snapshot ?? null});
    await request('frame',{elapsed:0});
    started = true;
    $('pause').disabled = false; $('export').disabled = false;
    $('play').disabled = false;
    resume();
    await save().catch(() => {});
  } catch (error) {
    fatal(error);
  } finally { busy = false; $('play').disabled = false; }
}
$('play').onclick = async () => {
  if (started) { resume(); return; }
  try { await boot((await storedSave())?.snapshot); }
  catch (error) { notice(`Cannot read browser storage: ${errorText(error)}. Your save has not been replaced.`); }
};
$('pause').onclick = () => paused ? resume() : pause();
$('fullscreen').onclick = async () => {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await $('game-shell').requestFullscreen();
    if (started && !paused) canvas.focus({preventScroll:true});
  } catch { notice('Fullscreen is unavailable here. Open the game in its own tab using the link on the guide page.'); }
};
document.addEventListener('fullscreenchange', () => {
  $('fullscreen').textContent = document.fullscreenElement ? 'Exit fullscreen ⛶' : 'Fullscreen ⛶';
  releaseControls();
});
$('export').onclick = async () => {
  try {
    const snapshot = await request('save');
    const url = URL.createObjectURL(new Blob([snapshot],{type:'application/json'}));
    const link = document.createElement('a'); link.href = url; link.download = 'termcraft-lantern-run.json'; link.click();
    setTimeout(() => URL.revokeObjectURL(url),1000);
  } catch (error) { notice(`Could not export: ${errorText(error)}`); }
};
$('import').onchange = async event => {
  const file = event.target.files[0]; if (!file) return;
  pause();
  try {
    if (file.size > 128*1024*1024) throw new Error('Save file exceeds the browser limit.');
    if (!confirm('Import this browser save and replace the current adventure? Export your current save first if you want to keep both.')) return;
    const snapshot = await file.text();
    await boot(snapshot);
  } catch (error) { notice(`Could not import: ${errorText(error)} Your previous saved game is retained.`); }
  finally { event.target.value = ''; }
};
$('restart').onclick = async () => {
  pause();
  if (!confirm('Start a fresh Lantern Run? This replaces the current browser adventure. Export a backup first if you want to keep it.')) return;
  await boot();
};
const gameKeys = new Set(['KeyA','KeyD','KeyW','KeyE','KeyV','KeyO','KeyR','KeyF','KeyQ','ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Space','ShiftLeft','ShiftRight','Enter','Delete','Escape','Tab','PageUp','PageDown','F4','Equal','Minus','NumpadAdd','NumpadSubtract',...Array.from({length:9},(_,i)=>`Digit${i+1}`)]);
canvas.addEventListener('keydown',event => {
  if (event.code === 'Escape' && document.fullscreenElement) {
    event.preventDefault();
    pause();
    document.exitFullscreen().catch(() => {});
    return;
  }
  if (event.ctrlKey || event.metaKey || event.altKey || !gameKeys.has(event.code)) return;
  if (event.code === 'Tab' && canvas.dataset.inventoryOpen !== 'true') return;
  event.preventDefault();
  if (paused || event.repeat || keys.has(event.code)) return;
  keys.add(event.code);
  request('key',{code:event.code,down:true,shift:event.shiftKey}).then(wantsPause => {if(wantsPause)pause();}).catch(fatal);
});
window.addEventListener('keyup',event => {
  if (!keys.has(event.code)) return;
  keys.delete(event.code); send('key',{code:event.code,down:false,shift:event.shiftKey});
});
function position(event) {
  const rect = canvas.getBoundingClientRect();
  return {x:Math.max(0,Math.min(COLUMNS-1,Math.floor((event.clientX-rect.left)/rect.width*COLUMNS))),y:Math.max(0,Math.min(ROWS-1,Math.floor((event.clientY-rect.top)/rect.height*ROWS)))};
}
canvas.addEventListener('pointerdown',event => {
  if (paused || !started || (event.button!==0 && event.button!==2)) return;
  event.preventDefault(); canvas.focus({preventScroll:true});canvas.setPointerCapture(event.pointerId);
  buttons = event.buttons; aim = position(event);
  send('mouse',{...aim,button:event.button,phase:1,shift:event.shiftKey});
});
canvas.addEventListener('pointermove',event => {
  if (paused || !started) return;
  aim=position(event);
  const tag=`${aim.x},${aim.y}`;
  if (buttons && tag!==lastDrag) {lastDrag=tag;send('mouse',{...aim,button:(buttons&2)?2:0,phase:2,shift:event.shiftKey});}
});
canvas.addEventListener('pointerup',event => {
  if (!started) return;
  buttons=event.buttons;lastDrag='';send('mouse',{...position(event),button:event.button,phase:0,shift:event.shiftKey});
});
canvas.addEventListener('pointercancel',releaseControls);
canvas.addEventListener('contextmenu',event => event.preventDefault());
canvas.addEventListener('wheel',event => {if(!paused&&started){event.preventDefault();send('wheel',{down:event.deltaY>0});}},{passive:false});
canvas.addEventListener('blur',releaseControls);
window.addEventListener('blur',pause);
document.addEventListener('visibilitychange',() => {if(document.hidden)pause();});
setInterval(() => {if(started&&!paused)save().catch(()=>{});},15000);
storedSave().then(saved => {if(saved)$('play').textContent='Continue adventure →';}).catch(() => {});
requestAnimationFrame(frame);
