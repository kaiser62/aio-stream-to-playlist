/* Player window logic: renders mpv state (streamed from Rust as `player://prop` events) and
   turns IINA-style keys, mouse, wheel and sliders into player commands.
   `?mock=1` runs a fake timeline in a normal browser for design checks. */
(() => {
  'use strict';
  const MOCK = new URLSearchParams(location.search).has('mock');
  const EMBED = new URLSearchParams(location.search).has('embed');
  if (EMBED) document.documentElement.classList.add('embed');
  const T = window.__TAURI__ || (EMBED && window.parent && window.parent.__TAURI__) || undefined;
  const $ = (s, r = document) => r.querySelector(s);
  const el = (tag, cls, html) => { const e = document.createElement(tag); if (cls) e.className = cls; if (html != null) e.innerHTML = html; return e; };
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

  // ------------------------------------------------------------------ icons (inline, SF-Symbols-like)
  const I = (d, vb = '0 0 24 24') => `<svg viewBox="${vb}" aria-hidden="true">${d}</svg>`;
  const ICON = {
    play: I('<path d="M8 5.6v12.8a1 1 0 0 0 1.52.85l10.4-6.4a1 1 0 0 0 0-1.7L9.52 4.75A1 1 0 0 0 8 5.6z"/>'),
    pause: I('<rect x="6" y="5" width="4.2" height="14" rx="1.3"/><rect x="13.8" y="5" width="4.2" height="14" rx="1.3"/>'),
    prev: I('<rect x="5" y="5.5" width="2.4" height="13" rx="1"/><path d="M19 6.3v11.4a.9.9 0 0 1-1.4.75L9.9 12.75a.9.9 0 0 1 0-1.5l7.7-5.7A.9.9 0 0 1 19 6.3z"/>'),
    next: I('<rect x="16.6" y="5.5" width="2.4" height="13" rx="1"/><path d="M5 6.3v11.4a.9.9 0 0 0 1.4.75l7.7-5.7a.9.9 0 0 0 0-1.5L6.4 5.55A.9.9 0 0 0 5 6.3z"/>'),
    back: I('<path d="M12 4.5a8 8 0 1 1-7.6 5.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><path d="M4 3.6v5.2h5.2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><text x="12" y="15.4" font-size="7.4" font-weight="700" text-anchor="middle" fill="currentColor" font-family="system-ui">10</text>'),
    fwd: I('<path d="M12 4.5a8 8 0 1 0 7.6 5.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><path d="M20 3.6v5.2h-5.2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/><text x="12" y="15.4" font-size="7.4" font-weight="700" text-anchor="middle" fill="currentColor" font-family="system-ui">10</text>'),
    vol0: I('<path d="M4 9.5v5h3.6l4.6 3.8a.7.7 0 0 0 1.1-.55V6.25a.7.7 0 0 0-1.1-.55L7.6 9.5z"/><path d="M16.5 9.5l4 5M20.5 9.5l-4 5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>'),
    vol1: I('<path d="M4 9.5v5h3.6l4.6 3.8a.7.7 0 0 0 1.1-.55V6.25a.7.7 0 0 0-1.1-.55L7.6 9.5z"/><path d="M16 9.4a3.8 3.8 0 0 1 0 5.2" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>'),
    vol2: I('<path d="M4 9.5v5h3.6l4.6 3.8a.7.7 0 0 0 1.1-.55V6.25a.7.7 0 0 0-1.1-.55L7.6 9.5z"/><path d="M16 9.4a3.8 3.8 0 0 1 0 5.2M18.6 7a7.4 7.4 0 0 1 0 10" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>'),
    list: I('<circle cx="5" cy="7" r="1.4"/><circle cx="5" cy="12" r="1.4"/><circle cx="5" cy="17" r="1.4"/><path d="M9.5 7h10M9.5 12h10M9.5 17h10" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/>'),
    sliders: I('<path d="M4 8h9M17 8h3M4 16h3M11 16h9" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/><circle cx="15" cy="8" r="2.3" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="9" cy="16" r="2.3" fill="none" stroke="currentColor" stroke-width="1.8"/>'),
    pin: I('<path d="M9 4h6l-.8 5.2 2.8 2.6v1.2h-4.4V20h-1.2v-6.999H7v-1.2l2.8-2.6z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>'),
    cur: I('<path d="M8 5.6v12.8a1 1 0 0 0 1.52.85l10.4-6.4a1 1 0 0 0 0-1.7L9.52 4.75A1 1 0 0 0 8 5.6z"/>'),
    speed: I('<path d="M13 5l7 7-7 7M4 5l7 7-7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>'),
    check: I('<path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/>'),
  };
  document.querySelectorAll('[data-ic]').forEach((b) => { b.innerHTML = ICON[b.dataset.ic] || ''; });

  // ------------------------------------------------------------------ backend bridge
  let mockState = null;
  async function invoke(cmd, args) {
    if (MOCK) return mockInvoke(cmd, args || {});
    return T.core.invoke(cmd, args || {});
  }
  const P = {}; // latest mpv properties
  const exec = (...args) => invoke('player_exec', { args }).catch(() => {});
  const setProp = (prop, value) => invoke('player_set', { prop, value }).catch(() => {});
  const cmd = (name, arg) => invoke('player_cmd', { cmd: name, arg: arg == null ? null : arg }).catch(() => {});
  const win = () => (!EMBED && T && T.window && T.window.getCurrentWindow ? T.window.getCurrentWindow() : null);

  // ------------------------------------------------------------------ formatting
  function fmt(t, force) {
    if (t == null || !isFinite(t)) return '--:--';
    t = Math.max(0, Math.floor(t));
    const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), s = t % 60;
    const mm = (h || force) ? String(m).padStart(2, '0') : String(m);
    return (h ? h + ':' : '') + mm + ':' + String(s).padStart(2, '0');
  }

  // ------------------------------------------------------------------ OSD (top-left, like IINA)
  const osdBox = $('#osd');
  const osdTimers = new Map();
  function osd(key, text, { icon, bar, sub } = {}) {
    let item = osdBox.querySelector(`[data-k="${key}"]`);
    if (!item) { item = el('div', 'osd-item vfx'); item.dataset.k = key; osdBox.appendChild(item); }
    item.classList.remove('out');
    item.innerHTML = (icon ? ICON[icon] || '' : '') + `<span>${esc(text)}</span>` +
      (sub ? `<span class="osd-sub">${esc(sub)}</span>` : '') +
      (bar != null ? `<span class="osd-bar"><i style="width:${clamp(bar, 0, 100)}%"></i></span>` : '');
    clearTimeout(osdTimers.get(key));
    osdTimers.set(key, setTimeout(() => {
      item.classList.add('out');
      setTimeout(() => item.remove(), 360);
    }, 1500));
    while (osdBox.children.length > 3) osdBox.firstChild.remove();
  }
  function flash(icon) {
    const f = $('#flash');
    f.innerHTML = ICON[icon] || '';
    f.classList.remove('go'); void f.offsetWidth; f.classList.add('go');
  }

  // ------------------------------------------------------------------ sliders
  function slider(root, { value, onInput, onCommit, tip }) {
    const track = $('.track', root);
    let drag = false;
    const frac = (e) => { const r = track.getBoundingClientRect(); return clamp((e.clientX - r.left) / r.width, 0, 1); };
    root.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      drag = true; root.classList.add('drag'); root.setPointerCapture(e.pointerId);
      onInput(frac(e), true);
    });
    root.addEventListener('pointermove', (e) => {
      if (tip) tip(e, frac(e));
      if (drag) onInput(frac(e), true);
    });
    const end = (e) => {
      if (!drag) return;
      drag = false; root.classList.remove('drag');
      (onCommit || onInput)(frac(e), false);
    };
    root.addEventListener('pointerup', end);
    root.addEventListener('pointercancel', () => { drag = false; root.classList.remove('drag'); });
    return { get dragging() { return drag; }, set(f) { f = clamp(f, 0, 1); $('.fill', root).style.width = f * 100 + '%'; const k = $('.knob', root); if (k) k.style.left = f * 100 + '%'; } };
  }

  // seek slider
  let showRemaining = true;
  let seekLast = 0;
  const seek = slider($('#seek'), {
    onInput: (f, live) => {
      const d = P.duration; if (!d) return;
      const t = f * d;
      $('#cur').textContent = fmt(t);
      seek.set(f);
      const now = performance.now();
      if (!live || now - seekLast > 90) { seekLast = now; cmd('seek_abs', t); }
    },
    tip: (e, f) => {
      const d = P.duration || 0; const tip = $('#seek .tip');
      tip.style.left = f * 100 + '%';
      $('span', tip).textContent = fmt(f * d);
      const ch = chapterAt(f * d);
      $('small', tip).textContent = ch ? ch.title : '';
      $('small', tip).style.display = ch ? '' : 'none';
    },
  });
  $('#remain').addEventListener('click', () => { showRemaining = !showRemaining; renderTime(); });

  // volume slider (0–100 in the UI; mpv allows up to volume-max)
  const vol = slider($('#vol'), {
    onInput: (f) => { const v = Math.round(f * 100); setProp('volume', v); P.volume = v; renderVolume(); osdVolume(); },
  });
  $('#mute').addEventListener('click', () => cmd('toggle_mute'));

  function chapterAt(t) {
    const ch = P['chapter-list'] || []; let cur = null;
    for (const c of ch) if (c.time <= t) cur = c;
    return cur;
  }

  // ------------------------------------------------------------------ rendering
  function renderPlay() {
    $('#play').innerHTML = P.pause ? ICON.play : ICON.pause;
    document.body.classList.toggle('paused', !!P.pause);
  }
  function renderTime() {
    const t = P['time-pos'] || 0, d = P.duration || 0;
    if (!seek.dragging) {
      $('#cur').textContent = fmt(t);
      seek.set(d ? t / d : 0);
    }
    $('#remain').textContent = showRemaining ? '-' + fmt(Math.max(0, d - t)) : fmt(d);
  }
  function renderBuffer() {
    const d = P.duration || 0; const bt = P['demuxer-cache-time'];
    $('#seek .buffer').style.width = d && bt ? clamp(bt / d, 0, 1) * 100 + '%' : '0%';
    document.body.classList.toggle('buffering', !!P['paused-for-cache']);
  }
  function renderTicks() {
    const box = $('#seek .ticks'); box.innerHTML = '';
    const d = P.duration || 0; if (!d) return;
    for (const c of (P['chapter-list'] || [])) if (c.time > 0.5) { const i = el('i'); i.style.left = (c.time / d) * 100 + '%'; box.appendChild(i); }
  }
  function renderLoop() {
    const a = P['ab-loop-a'], b = P['ab-loop-b'], d = P.duration || 0; const l = $('#seek .loop');
    if (typeof a === 'number' && d) { l.hidden = false; l.style.left = (a / d) * 100 + '%'; l.style.width = ((typeof b === 'number' ? b : a) - a) / d * 100 + '%'; if (typeof b !== 'number') l.style.width = '2px'; }
    else l.hidden = true;
  }
  function renderVolume() {
    const v = P.volume == null ? 100 : P.volume, m = P.mute;
    $('#mute').innerHTML = m || v === 0 ? ICON.vol0 : v < 34 ? ICON.vol1 : ICON.vol2;
    vol.set(m ? 0 : v / 100);
  }
  function renderSpeed() {
    const s = P.speed || 1; const pill = $('#speedpill');
    pill.textContent = (Math.round(s * 100) / 100) + 'x'; pill.classList.toggle('on', Math.abs(s - 1) > 0.001);
  }
  function renderTitle() {
    const t = P['media-title'] || P.filename || '';
    $('#title').textContent = t;
    const w = win(); if (w && w.setTitle) w.setTitle(t || 'Player').catch(() => {});
  }
  function renderNav() {
    const pl = P.playlist || [], pos = P['playlist-pos'];
    $('#prev').disabled = !(pl.length > 1); $('#next').disabled = !(pl.length > 1);
    $('#prev').style.opacity = $('#next').style.opacity = pl.length > 1 ? '' : '.35';
  }

  // ------------------------------------------------------------------ OSD helpers
  function osdVolume() { const v = P.volume == null ? 100 : P.volume; osdKey('vol', P.mute ? 'Muted' : 'Volume ' + Math.round(v), { icon: P.mute || v === 0 ? 'vol0' : v < 34 ? 'vol1' : 'vol2', bar: v / (P['volume-max'] || 100) * 100 }); }
  const osdKey = osd;

  // ------------------------------------------------------------------ property intake
  const quiet = new Set(['time-pos', 'duration', 'demuxer-cache-time', 'estimated-vf-fps', 'core-idle', 'seeking', 'video-params', 'hwdec-current', 'eof-reached', 'paused-for-cache', 'file-loaded', 'path', 'filename', 'media-title', 'playlist', 'playlist-pos', 'track-list', 'chapter-list', 'chapter', 'ab-loop-a', 'ab-loop-b']);
  let ready = false;
  // Transparent (shows mpv underneath) only once a real video frame exists; black before, so nothing behind the app leaks through.
  function videoOn() {
    const on = !!(P.path && P['video-params']);
    document.body.classList.toggle('video-on', on);
    if (EMBED && videoOn.last !== on) { videoOn.last = on; try { parent.postMessage({ t: 'video-on', on }, '*'); } catch (e) {} }
  }
  function apply(name, v, fromSnapshot) {
    const old = P[name]; P[name] = v;
    switch (name) {
      case 'pause': renderPlay(); if (ready && !fromSnapshot && old !== v) flash(v ? 'pause' : 'play'); if (ready && !fromSnapshot && old !== v) osd('pause', v ? 'Paused' : 'Playing', { icon: v ? 'pause' : 'play' }); break;
      case 'time-pos': case 'duration': renderTime(); if (name === 'duration') { renderTicks(); renderLoop(); } break;
      case 'demuxer-cache-time': case 'paused-for-cache': renderBuffer(); break;
      case 'volume': case 'mute': renderVolume(); if (ready && !fromSnapshot && old !== v && !$('#vol').classList.contains('drag')) osdVolume(); break;
      case 'speed': renderSpeed(); if (ready && !fromSnapshot && old !== v) osd('speed', (Math.round(v * 100) / 100) + 'x', { icon: 'speed', sub: 'Speed' }); break;
      case 'path': document.body.classList.toggle('idle', !v); videoOn(); if (v && !fromSnapshot) applyFx(); break;
      case 'video-params': videoOn(); break;
      case 'media-title': case 'filename': renderTitle(); break;
      case 'chapter-list': renderTicks(); renderSidebar(); break;
      case 'ab-loop-a': case 'ab-loop-b': renderLoop(); if (ready && !fromSnapshot && name === 'ab-loop-a' && typeof v === 'number') osd('ab', 'A–B loop: ' + (typeof P['ab-loop-b'] === 'number' ? 'on' : 'A set')); if (ready && !fromSnapshot && name === 'ab-loop-b' && v == null) osd('ab', 'A–B loop off'); break;
      case 'playlist': case 'playlist-pos': renderNav(); renderSidebar(); break;
      case 'track-list': case 'sid': case 'aid': renderSidebar(); if (ready && !fromSnapshot && (name === 'sid' || name === 'aid') && old !== v) osdTrack(name); break;
      case 'sub-delay': if (ready && !fromSnapshot && old !== v) osd('subd', 'Subtitle delay ' + (v * 1000).toFixed(0) + ' ms'); break;
      case 'audio-delay': if (ready && !fromSnapshot && old !== v) osd('audd', 'Audio delay ' + (v * 1000).toFixed(0) + ' ms'); break;
      default: if (sidebarMode === 'settings') renderSidebar();
    }
  }
  function osdTrack(name) {
    const type = name === 'sid' ? 'sub' : 'audio'; const id = P[name];
    const t = (P['track-list'] || []).find((x) => x.type === type && x.id === id);
    osd(name, (type === 'sub' ? 'Subtitles: ' : 'Audio: ') + (id === false || id === 'no' || !t ? 'Off' : trackLabel(t)));
  }
  function trackLabel(t) { return [t.title, t.lang && t.lang.toUpperCase(), (!t.title && !t.lang) ? 'Track ' + t.id : ''].filter(Boolean).join(' · ') + (t.codec ? '' : ''); }

  // ------------------------------------------------------------------ sidebar
  let sidebarMode = null; // 'playlist' | 'settings'
  const tabState = { playlist: 'playlist', settings: 'video' };
  const TABS = { playlist: [['playlist', 'Playlist'], ['chapters', 'Chapters']], settings: [['video', 'Video'], ['audio', 'Audio'], ['sub', 'Subtitles']] };
  function toggleSidebar(mode) {
    const open = sidebarMode === mode;
    sidebarMode = open ? null : mode;
    document.body.classList.toggle('sidebar-open', !!sidebarMode);
    $('#btn-playlist').setAttribute('aria-pressed', sidebarMode === 'playlist');
    $('#btn-settings').setAttribute('aria-pressed', sidebarMode === 'settings');
    if (sidebarMode) renderSidebar();
  }
  function renderSidebar() {
    if (!sidebarMode) return;
    const tabs = $('#sb-tabs'); tabs.innerHTML = '';
    for (const [id, label] of TABS[sidebarMode]) {
      const b = el('button', tabState[sidebarMode] === id ? 'on' : '', esc(label));
      b.onclick = () => { tabState[sidebarMode] = id; renderSidebar(); };
      tabs.appendChild(b);
    }
    const body = $('#sb-body'); const keep = body.scrollTop; body.innerHTML = '';
    const tab = tabState[sidebarMode];
    if (tab === 'playlist') sbPlaylist(body); else if (tab === 'chapters') sbChapters(body);
    else if (tab === 'video') sbVideo(body); else if (tab === 'audio') sbAudio(body); else sbSub(body);
    body.scrollTop = keep;
  }
  function sbPlaylist(body) {
    const pl = P.playlist || [];
    if (!pl.length) { body.appendChild(el('div', 'sb-empty', 'The playlist is empty.')); return; }
    pl.forEach((it, i) => {
      const cur = it.current || i === P['playlist-pos'];
      const name = it.title || (it.filename || '').split('/').pop() || 'Item ' + (i + 1);
      const row = el('div', 'li' + (cur ? ' cur' : ''), (cur ? `<span class="cur-ic">${ICON.cur}</span>` : `<span class="idx">${i + 1}</span>`) + `<span class="t">${esc(name)}</span>`);
      row.ondblclick = () => exec('playlist-play-index', i);
      row.onclick = () => exec('playlist-play-index', i);
      body.appendChild(row);
    });
  }
  function sbChapters(body) {
    const ch = P['chapter-list'] || [];
    if (!ch.length) { body.appendChild(el('div', 'sb-empty', 'This file has no chapters.')); return; }
    const t = P['time-pos'] || 0;
    const curIdx = ch.reduce((a, c, i) => (c.time <= t ? i : a), -1);
    ch.forEach((c, i) => {
      const row = el('div', 'li' + (i === curIdx ? ' cur' : ''), `<span class="idx">${i + 1}</span><span class="t">${esc(c.title || 'Chapter ' + (i + 1))}</span><span class="d">${fmt(c.time)}</span>`);
      row.onclick = () => cmd('seek_abs', c.time);
      body.appendChild(row);
    });
  }
  function secHead(body, text) { const s = el('div', 'qs-sec'); s.appendChild(el('h4', '', esc(text))); body.appendChild(s); return s; }
  function sliderRow(parent, label, { min, max, step, get, set, fmtv }) {
    const row = el('div', 'qs-row'); row.appendChild(el('label', '', esc(label)));
    const s = el('div', 'slider'); s.innerHTML = '<div class="track"><div class="fill"></div><div class="knob"></div></div>';
    const val = el('span', 'val'); row.appendChild(s); row.appendChild(val); parent.appendChild(row);
    const show = (v) => { val.textContent = fmtv ? fmtv(v) : v; };
    const cur = get(); const f = (cur - min) / (max - min);
    const api = slider(s, { onInput: (fr) => { let v = min + fr * (max - min); v = Math.round(v / step) * step; v = +v.toFixed(3); set(v); show(v); api.set((v - min) / (max - min)); } });
    api.set(clamp(f, 0, 1)); show(cur);
    const reset = el('button', 'chip', 'Reset'); reset.style.cssText = 'height:22px;padding:0 7px;font-size:11px'; reset.onclick = () => { const d = label === 'Speed' ? 1 : (label === 'Scale' ? 1 : 0); set(d); renderSidebar(); }; row.appendChild(reset);
  }
  function chips(parent, items, current, onPick) {
    const c = el('div', 'chips');
    for (const [label, value] of items) { const b = el('button', 'chip' + (String(current) === String(value) ? ' on' : ''), esc(label)); b.onclick = () => { onPick(value); setTimeout(renderSidebar, 60); }; c.appendChild(b); }
    parent.appendChild(c);
  }
  /* Video effects: Anime4K upscaling presets + mpv motion smoothing (frame blending/interpolation) + deband. Persisted per device. */
  const FX_DEF = { a4k: 'off', smooth: 'off', deband: false, scale: 'default' };
  let FX = Object.assign({}, FX_DEF);
  try { Object.assign(FX, JSON.parse(localStorage.getItem('player.fx') || '{}')); } catch (e) {}
  const SMOOTH = { off: [['interpolation', false], ['video-sync', 'audio'], ['tscale', 'oversample']],
    light: [['video-sync', 'display-resample'], ['tscale', 'oversample'], ['interpolation', true]],
    blend: [['video-sync', 'display-resample'], ['tscale', 'linear'], ['interpolation', true]] };
  const saveFx = () => { try { localStorage.setItem('player.fx', JSON.stringify(FX)); } catch (e) {} };
  function applyFx(only) {
    if (!only || only === 'a4k') invoke('player_filter', { preset: FX.a4k }).catch(() => {});
    if (!only || only === 'smooth') for (const [k, v] of SMOOTH[FX.smooth] || SMOOTH.off) setProp(k, v);
    if (!only || only === 'deband') setProp('deband', !!FX.deband);
    if (!only || only === 'scale') { const m = { default: ['bilinear', 'bilinear'], sharp: ['ewa_lanczossharp', 'mitchell'], spline: ['spline36', 'mitchell'] }[FX.scale] || []; if (m.length) { setProp('scale', m[0]); setProp('dscale', m[1]); } }
  }
  const setFx = (k, v) => { FX[k] = v; saveFx(); applyFx(k); };
  function sbVideo(body) {
    let s = secHead(body, 'Playback');
    sliderRow(s, 'Speed', { min: 0.25, max: 4, step: 0.05, get: () => P.speed || 1, set: (v) => { P.speed = v; setProp('speed', v); renderSpeed(); }, fmtv: (v) => v + 'x' });
    s = secHead(body, 'Upscaling (Anime4K)');
    chips(s, [['Off', 'off'], ['A · HQ', 'a-hq'], ['B · HQ', 'b-hq'], ['C · HQ', 'c-hq'], ['A · Fast', 'a-fast'], ['B · Fast', 'b-fast'], ['C · Fast', 'c-fast']], FX.a4k, (v) => setFx('a4k', v));
    s.appendChild(el('div', 'sb-empty sb-hint', 'A: clean anime / B: compressed / C: upscale + denoise. Fast presets suit weaker GPUs.'));
    s = secHead(body, 'Motion smoothing');
    chips(s, [['Off', 'off'], ['Light', 'light'], ['Blend', 'blend']], FX.smooth, (v) => setFx('smooth', v));
    s.appendChild(el('div', 'sb-empty sb-hint', 'Frame interpolation in mpv. Motion-compensated (SVP/RIFE) needs VapourSynth.'));
    s = secHead(body, 'Scaler');
    chips(s, [['Default', 'default'], ['Sharp', 'sharp'], ['Spline36', 'spline']], FX.scale, (v) => setFx('scale', v));
    const drow = el('div', 'qs-row'); drow.appendChild(el('label', '', 'Deband'));
    const dsw = el('button', 'switch'); dsw.setAttribute('role', 'switch'); dsw.setAttribute('aria-checked', !!FX.deband);
    dsw.onclick = () => { setFx('deband', !FX.deband); setTimeout(renderSidebar, 60); };
    drow.appendChild(dsw); s.appendChild(drow);
    s = secHead(body, 'Aspect ratio');
    chips(s, [['Default', '-1'], ['4:3', '4:3'], ['16:9', '16:9'], ['16:10', '16:10'], ['21:9', '2.35:1'], ['5:4', '5:4']], P['video-aspect-override'] == null ? '-1' : P['video-aspect-override'], (v) => setProp('video-aspect-override', v));
    s = secHead(body, 'Rotation');
    chips(s, [['0°', 0], ['90°', 90], ['180°', 180], ['270°', 270]], P['video-rotate'] || 0, (v) => setProp('video-rotate', v));
    s = secHead(body, 'Decoding');
    const row = el('div', 'qs-row'); row.appendChild(el('label', '', 'Hardware'));
    const sw = el('button', 'switch'); const on = P.hwdec && P.hwdec !== 'no';
    sw.setAttribute('role', 'switch'); sw.setAttribute('aria-checked', !!on);
    sw.onclick = () => { setProp('hwdec', on ? 'no' : 'auto-safe'); setTimeout(renderSidebar, 80); };
    row.appendChild(sw); const hc = P['hwdec-current']; if (hc && hc !== 'no') row.appendChild(el('span', 'val', esc(hc)));
    s.appendChild(row);
    s = secHead(body, 'Color');
    for (const [k, label] of [['brightness', 'Brightness'], ['contrast', 'Contrast'], ['saturation', 'Saturation'], ['gamma', 'Gamma'], ['hue', 'Hue']]) {
      sliderRow(s, label, { min: -100, max: 100, step: 1, get: () => P[k] || 0, set: (v) => { P[k] = v; setProp(k, v); } });
    }
  }
  function trackList(body, type, current, onPick, none) {
    const tr = (P['track-list'] || []).filter((t) => t.type === type);
    if (none) { const r = el('div', 'radio' + (current === false || current === 'no' || current == null ? ' on' : ''), '<span class="dot"></span><span>None</span>'); r.onclick = () => onPick('no'); body.appendChild(r); }
    if (!tr.length && !none) body.appendChild(el('div', 'sb-empty', 'No tracks'));
    for (const t of tr) {
      const r = el('div', 'radio' + (t.selected || t.id === current ? ' on' : ''), `<span class="dot"></span><span>${esc(trackLabel(t))}</span><span class="sub">${esc(t.codec || '')}</span>`);
      r.onclick = () => onPick(t.id); body.appendChild(r);
    }
  }
  function sbAudio(body) {
    let s = secHead(body, 'Audio track'); trackList(s, 'audio', P.aid, (id) => setProp('aid', id), false);
    s = secHead(body, 'Levels');
    sliderRow(s, 'Volume', { min: 0, max: 130, step: 1, get: () => P.volume == null ? 100 : P.volume, set: (v) => { P.volume = v; setProp('volume', v); renderVolume(); } });
    sliderRow(s, 'Delay', { min: -5, max: 5, step: 0.1, get: () => P['audio-delay'] || 0, set: (v) => setProp('audio-delay', v), fmtv: (v) => v + 's' });
  }
  function sbSub(body) {
    let s = secHead(body, 'Subtitle track'); trackList(s, 'sub', P.sid, (id) => setProp('sid', id), true);
    s = secHead(body, 'Adjust');
    sliderRow(s, 'Delay', { min: -10, max: 10, step: 0.1, get: () => P['sub-delay'] || 0, set: (v) => setProp('sub-delay', v), fmtv: (v) => v + 's' });
    sliderRow(s, 'Scale', { min: 0.5, max: 3, step: 0.05, get: () => P['sub-scale'] || 1, set: (v) => setProp('sub-scale', v), fmtv: (v) => v + 'x' });
    sliderRow(s, 'Position', { min: 0, max: 100, step: 1, get: () => P['sub-pos'] == null ? 100 : P['sub-pos'], set: (v) => setProp('sub-pos', v) });
  }

  // ------------------------------------------------------------------ buttons
  $('#play').onclick = () => cmd('toggle_pause');
  $('#back').onclick = () => cmd('seek', -10);
  $('#fwd').onclick = () => cmd('seek', 10);
  $('#prev').onclick = () => cmd('prev');
  $('#next').onclick = () => cmd('next');
  $('#speedpill').onclick = () => { setProp('speed', 1); };
  $('#btn-playlist').onclick = () => toggleSidebar('playlist');
  $('#btn-settings').onclick = () => toggleSidebar('settings');
  let pinned = false;
  $('#pin').onclick = () => { pinned = !pinned; $('#pin').setAttribute('aria-pressed', pinned); const w = win(); if (w) w.setAlwaysOnTop(pinned).catch(() => {}); osd('pin', pinned ? 'Floating on top' : 'Not floating'); };

  // ------------------------------------------------------------------ fullscreen
  let fs = false;
  async function toggleFullscreen() {
    if (EMBED) { parent.postMessage({ t: 'player-fs' }, '*'); return; }
    const w = win();
    if (!w) { fs = !fs; osd('fs', fs ? 'Fullscreen' : 'Windowed'); return; }
    try { fs = !(await w.isFullscreen()); await w.setFullscreen(fs); } catch (_) {}
  }

  // ------------------------------------------------------------------ stage mouse (IINA defaults)
  let clickTimer = null;
  $('#stage').addEventListener('click', () => {
    if (ctx.classList.contains('open')) { closeCtx(); return; }
    clearTimeout(clickTimer);
    clickTimer = setTimeout(() => cmd('toggle_pause'), 230);
  });
  $('#stage').addEventListener('dblclick', () => { clearTimeout(clickTimer); toggleFullscreen(); });
  let wheelAcc = 0, wheelX = 0;
  $('#stage').addEventListener('wheel', (e) => {
    e.preventDefault();
    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) { wheelX += e.deltaX; if (Math.abs(wheelX) > 40) { cmd('seek', wheelX > 0 ? 5 : -5); wheelX = 0; } }
    else { wheelAcc += e.deltaY; if (Math.abs(wheelAcc) > 6) { cmd('volume_add', wheelAcc < 0 ? 2 : -2); wheelAcc = 0; } }
  }, { passive: false });

  // ------------------------------------------------------------------ context menu
  const ctx = $('#ctx');
  function closeCtx() { ctx.classList.remove('open'); }
  document.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    const items = [
      [P.pause ? 'Play' : 'Pause', 'Space', () => cmd('toggle_pause')],
      ['Fullscreen', 'F', toggleFullscreen],
      null,
      ['Next in playlist', '>', () => cmd('next')],
      ['Previous in playlist', '<', () => cmd('prev')],
      null,
      ['Playlist', '', () => toggleSidebar('playlist')],
      ['Quick settings', '', () => toggleSidebar('settings')],
      ['Screenshot', '⌘S', () => { cmd('screenshot'); osd('shot', 'Screenshot taken'); }],
      ['Playback stats', 'I', () => cmd('stats')],
      null,
      ['Close player', 'Q', closePlayer],
    ];
    ctx.innerHTML = '';
    for (const it of items) {
      if (!it) { ctx.appendChild(el('hr')); continue; }
      const b = el('button', '', `<span>${esc(it[0])}</span><kbd>${esc(it[1])}</kbd>`);
      b.onclick = () => { closeCtx(); it[2](); };
      ctx.appendChild(b);
    }
    ctx.classList.add('open');
    const r = ctx.getBoundingClientRect();
    ctx.style.left = Math.min(e.clientX, innerWidth - r.width - 8) + 'px';
    ctx.style.top = Math.min(e.clientY, innerHeight - r.height - 8) + 'px';
  });
  document.addEventListener('pointerdown', (e) => { if (!ctx.contains(e.target)) closeCtx(); }, true);

  // ------------------------------------------------------------------ chrome auto-hide
  let hideTimer = null;
  function poke() {
    document.body.classList.remove('ui-hidden');
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      const over = document.querySelector('#osc:hover, #titlebar:hover, #sidebar:hover, #ctx.open');
      if (!over && !P.pause) document.body.classList.add('ui-hidden');
      else if (over) poke();
    }, 2600);
  }
  document.addEventListener('mousemove', poke);
  document.addEventListener('pointerdown', poke);
  document.addEventListener('keydown', poke);

  // ------------------------------------------------------------------ keyboard (key map comes from Rust)
  const DOMKEY = { ' ': 'SPACE', ArrowLeft: 'LEFT', ArrowRight: 'RIGHT', ArrowUp: 'UP', ArrowDown: 'DOWN', Backspace: 'BS', Enter: 'ENTER', PageDown: 'PGDWN', PageUp: 'PGUP' };
  let keymap = new Map();
  function keyName(e) {
    let k = DOMKEY[e.key] || e.key; const mods = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.metaKey) mods.push('Meta');
    if (e.shiftKey && k.length > 1) mods.push('Shift');
    return mods.concat(k).join('+');
  }
  async function loadKeys() {
    try { const list = await invoke('player_keys'); keymap = new Map(list.map((k) => [k.key, k])); } catch (_) { /* fallback below */ }
    if (!keymap.size) [['SPACE', 'toggle_pause'], ['LEFT', 'seek', -5], ['RIGHT', 'seek', 5], ['UP', 'volume_add', 5], ['DOWN', 'volume_add', -5], ['m', 'toggle_mute'], ['f', 'fullscreen']].forEach(([key, c, a]) => keymap.set(key, { key, cmd: c, arg: a == null ? null : a }));
  }
  function closePlayer() { invoke('player_close').catch(() => {}); const w = win(); if (w) w.close().catch(() => {}); else if (EMBED) resetIdle(); }
  document.addEventListener('keydown', (e) => {
    if (e.target && /input|textarea/i.test(e.target.tagName)) return;
    if (e.key === 'Escape') { e.preventDefault(); if (ctx.classList.contains('open')) closeCtx(); else if (fs) toggleFullscreen(); else if (sidebarMode) toggleSidebar(sidebarMode); return; }
    if (e.key === 'Tab') { e.preventDefault(); toggleSidebar('playlist'); return; }
    const k = keymap.get(keyName(e));
    if (!k) return;
    e.preventDefault();
    if (k.cmd === 'fullscreen') return toggleFullscreen();
    if (k.cmd === 'stop') return closePlayer();
    if (k.cmd === 'screenshot') osd('shot', 'Screenshot taken');
    if (k.cmd === 'seek') osd('seek', (k.arg > 0 ? '+' : '') + k.arg + ' s', { icon: k.arg > 0 ? 'fwd' : 'back' });
    cmd(k.cmd, k.arg);
  });

  function resetIdle() {
    for (const k of Object.keys(P)) delete P[k];
    document.body.classList.remove('video-on'); document.body.classList.add('idle');
    renderPlay(); renderTime(); renderVolume(); renderSpeed(); renderTitle(); renderNav(); renderTicks();
    if (sidebarMode) toggleSidebar(sidebarMode);
  }
  window.addEventListener('message', (e) => { if (e.data && e.data.t === 'reset') { delete P.path; delete P['video-params']; videoOn(); }
    if (e.data && e.data.t === 'fs-state') { fs = !!e.data.on; osd('fs', fs ? 'Fullscreen' : 'Windowed'); } });

  // ------------------------------------------------------------------ events from Rust
  async function boot() {
    await loadKeys();
    try { const snap = await invoke('player_props'); for (const [k, v] of Object.entries(snap || {})) apply(k, v, true); } catch (_) {}
    renderPlay(); renderTime(); renderVolume(); renderSpeed(); renderTitle(); renderNav();
    document.body.classList.toggle('idle', !P.path);
    videoOn();
    ready = true; poke();
    if (T && T.event) {
      T.event.listen('player://prop', (e) => apply(e.payload.name, e.payload.value, false));
      T.event.listen('player://closed', () => { const w = win(); if (w) w.close().catch(() => {}); else resetIdle(); });
    }
    if (MOCK) startMock();
  }

  // ------------------------------------------------------------------ mock (browser design checks)
  function mockInvoke(c, a) {
    const s = mockState || (mockState = {});
    if (c === 'player_keys') return Promise.resolve(KEYS_FALLBACK);
    if (c === 'player_props') return Promise.resolve(s.props || {});
    if (c === 'player_set') { mockSet(a.prop, a.value); return Promise.resolve(); }
    if (c === 'player_exec') { const [n, x] = a.args; if (n === 'seek') mockSet('time-pos', x); if (n === 'playlist-play-index') { mockSet('playlist-pos', x); mockSet('time-pos', 0); } return Promise.resolve(); }
    if (c === 'player_cmd') {
      const m = { toggle_pause: () => mockSet('pause', !P.pause), seek: () => mockSet('time-pos', clamp((P['time-pos'] || 0) + a.arg, 0, P.duration)), seek_abs: () => mockSet('time-pos', a.arg), volume_add: () => mockSet('volume', clamp((P.volume || 0) + a.arg, 0, 130)), toggle_mute: () => mockSet('mute', !P.mute), speed_mul: () => mockSet('speed', +((P.speed || 1) * a.arg).toFixed(2)), speed_reset: () => mockSet('speed', 1), next: () => mockSet('playlist-pos', ((P['playlist-pos'] || 0) + 1) % 3), prev: () => mockSet('playlist-pos', ((P['playlist-pos'] || 0) + 2) % 3) };
      (m[a.cmd] || (() => {}))(); return Promise.resolve();
    }
    return Promise.resolve();
  }
  function mockSet(k, v) { P[k] = v; mockState.props[k] = v; apply(k, v, false); }
  const KEYS_FALLBACK = [['SPACE', 'toggle_pause'], ['LEFT', 'seek', -5], ['RIGHT', 'seek', 5], ['Shift+LEFT', 'seek', -60], ['Shift+RIGHT', 'seek', 60], ['UP', 'volume_add', 5], ['DOWN', 'volume_add', -5], ['m', 'toggle_mute'], ['[', 'speed_mul', 0.9091], [']', 'speed_mul', 1.1], ['BS', 'speed_reset'], ['f', 'fullscreen'], ['>', 'next'], ['<', 'prev'], ['q', 'stop']].map(([key, cmd, arg]) => ({ key, cmd, arg: arg == null ? null : arg, desc: '' }));
  function startMock() {
    document.body.classList.add('mock');
    const q = new URLSearchParams(location.search);
    const base = {
      'media-title': 'Severance S02E04 · Woe\'s Hollow', duration: 3412, 'time-pos': 1130, pause: q.has('paused'), volume: 70, mute: false, speed: 1,
      'demuxer-cache-time': 1700, 'playlist-pos': 1, hwdec: 'auto-safe', 'hwdec-current': 'videotoolbox',
      playlist: [{ title: 'Severance S02E03 · Who Is Alive?' }, { title: 'Severance S02E04 · Woe\'s Hollow', current: true }, { title: 'Severance S02E05 · Trojan\'s Horse' }],
      'chapter-list': [{ title: 'Cold open', time: 0 }, { title: 'Credits', time: 120 }, { title: 'Part one', time: 340 }, { title: 'Part two', time: 1500 }, { title: 'Finale', time: 2700 }],
      'track-list': [{ type: 'video', id: 1, codec: 'hevc', selected: true }, { type: 'audio', id: 1, lang: 'eng', title: 'Surround 5.1', codec: 'eac3', selected: true }, { type: 'audio', id: 2, lang: 'jpn', codec: 'aac' }, { type: 'sub', id: 1, lang: 'eng', title: 'Full', codec: 'subrip' }, { type: 'sub', id: 2, lang: 'fre', codec: 'subrip' }],
      aid: 1, sid: false, 'sub-delay': 0, 'audio-delay': 0, 'sub-scale': 1, 'sub-pos': 100,
    };
    mockState = { props: {} };
    for (const [k, v] of Object.entries(base)) { P[k] = v; mockState.props[k] = v; }
    Object.keys(base).forEach((k) => apply(k, base[k], true));
    renderPlay(); renderTime(); renderVolume(); renderSpeed(); renderTitle(); renderNav(); renderTicks();
    if (q.has('sidebar')) toggleSidebar(q.get('sidebar') || 'playlist');
    if (q.has('tab')) { tabState[sidebarMode || 'settings'] = q.get('tab'); renderSidebar(); }
    setInterval(() => { if (!P.pause) mockSet('time-pos', Math.min(P.duration, (P['time-pos'] || 0) + 0.1 * (P.speed || 1))); }, 100);
  }

  boot();
})();
