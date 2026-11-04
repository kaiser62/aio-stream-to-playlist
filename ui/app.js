'use strict';
const invoke = (c, a) => { try { return window.__TAURI__.core.invoke(c, a); } catch (e) { return Promise.reject(e); } };
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'}[c]));
const gb = b => b ? (b / 1073741824).toFixed(2) + ' GB' : '—';
const ic = (n, cls = '') => `<svg class="ic ${cls}" aria-hidden="true"><use href="#i-${n}"/></svg>`;
const hue = s => { let h = 0; for (const c of String(s || '')) h = (h * 31 + c.charCodeAt(0)) % 360; return h; };
const initial = s => (String(s || '?').replace(/^(the|a|an)\s+/i, '').trim()[0] || '?').toUpperCase();
const stateBox = (icon, title, text, o = {}) => `<div class="state ${o.err ? 'err' : ''}"><div class="art">${ic(icon)}</div><h4>${esc(title)}</h4>${text ? `<p>${esc(text)}</p>` : ''}${o.action ? `<button class="btn ${o.err ? '' : 'primary'}" data-act="${esc(o.action[1])}">${esc(o.action[0])}</button>` : ''}</div>`;
const skeleton = (n, cls) => Array.from({length: n}, () => `<div class="sk ${cls}"></div>`).join('');
const UA = navigator.userAgent;
const PLAT = /iPhone|iPad|iPod/.test(UA) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1) ? 'ios'
  : /Android/.test(UA) ? 'android' : /Win/.test(UA) ? 'windows' : /Mac/.test(UA) ? 'mac' : 'other';

const App = {
  kind: 'movie', title: null, streams: [], view: [], token: '', sel: new Set(), chips: {},

  async api(path, body) {
    const [p, qs] = path.split('?'); const q = new URLSearchParams(qs || '');
    try {
      if (p === '/api/config') {
        if (body === undefined) return await invoke('config_get');
        await invoke('config_set', {torboxKey: body.torbox_key || null}); return {ok: true};
      }
      if (p === '/api/search') return {results: await invoke('search', {kind: body.kind, query: body.query})};
      if (p === '/api/fetch') return await invoke('fetch_streams', {kind: body.kind, imdb: body.imdb, season: body.season, ep1: body.ep1, ep2: body.ep2});
      if (p === '/api/send') return await invoke('send', {items: body.items, name: body.name, append: !!body.append});
      if (p.startsWith('/api/job/')) return await invoke('job_get', {id: p.split('/').pop()});
      if (p === '/api/torbox/library') return await invoke('library');
      if (p === '/api/playlists') return {playlists: await invoke('playlists_list')};
      if (p === '/api/playlist') return {entries: await invoke('playlist_read', {file: q.get('file')})};
      if (p === '/api/playlist/save') return {file: await invoke('playlist_save', {name: body.name, old: body.old || null, entries: body.entries})};
      if (p === '/api/playlist/delete') { await invoke('playlist_delete', {file: body.file}); return {ok: true}; }
      if (p === '/api/player/open') { await invoke('player_open', {name: body.name}); return {ok: true}; }
      if (p === '/api/player/cmd') return {ok: true, result: await invoke('player_cmd', {cmd: body.cmd, arg: body.arg ?? null})};
      return {error: 'unknown call ' + p};
    } catch (e) { return {error: String(e)}; }
  },
  toast(msg, ms = 2600, tone = '') {
    const t = $('#toast'); t.className = 'toast ' + tone;
    t.innerHTML = (tone === 'bad' ? ic('alert') : '') + `<span>${esc(msg)}</span>`; t.hidden = false;
    clearTimeout(this._t); this._t = setTimeout(() => t.hidden = true, ms);
  },
  err(e, ms = 4500) { this.toast(String(e && e.error ? e.error : e), ms, 'bad'); },

  /* promise-based replacement for confirm()/prompt(): works in every webview */
  ask({title, msg = '', ok = 'OK', danger = false, input = null}) {
    return new Promise(res => {
      const m = $('#modal'), inp = $('#m-input'), okb = $('#m-ok'), prev = document.activeElement;
      $('#m-title').textContent = title; $('#m-msg').textContent = msg; okb.textContent = ok;
      okb.classList.toggle('danger-fill', danger);
      inp.hidden = input === null; if (input !== null) { inp.value = input.value ?? ''; inp.placeholder = input.placeholder || ''; inp.setAttribute('aria-label', title); }
      m.hidden = false; (input === null ? okb : inp).focus();
      const done = v => { m.hidden = true; okb.onclick = $('#m-cancel').onclick = m.onkeydown = m.onclick = null; prev && prev.focus && prev.focus(); res(v); };
      okb.onclick = () => done(input === null ? true : inp.value);
      $('#m-cancel').onclick = () => done(input === null ? false : null);
      m.onclick = e => { if (e.target === m) $('#m-cancel').onclick(); };
      m.onkeydown = e => { if (e.key === 'Escape') $('#m-cancel').onclick(); else if (e.key === 'Enter' && input !== null) okb.onclick(); };
    });
  },

  show(v) {
    $$('main > section').forEach(s => s.hidden = s.id !== 'v-' + v);
    $$('#nav button').forEach(b => { const on = b.dataset.v === v; b.classList.toggle('on', on); on ? b.setAttribute('aria-current', 'page') : b.removeAttribute('aria-current'); });
    $('.shell').classList.remove('nav-open'); $('#railtog').setAttribute('aria-expanded', 'false');
    $('#v-results').classList.remove('fopen-m');
    try { history.replaceState(null, '', location.search + '#' + v); } catch (e) {}
    if (v === 'find') this.refreshKeyBanner();
    if (v === 'library') this.loadLibrary();
    if (v === 'addons') this.loadAddons();
    if (v === 'playlists') this.loadPlaylists();
    if (v === 'watching') this.loadWatching();
    if (v === 'player') this.loadPlayer(); else this.syncVideo();
    if (v === 'settings') this.loadSettings();
    window.scrollTo(0, 0);
  },

  setTheme(t) {
    try { t === 'auto' ? localStorage.removeItem('theme') : localStorage.setItem('theme', t); } catch (e) {}
    t === 'auto' ? document.documentElement.removeAttribute('data-theme') : document.documentElement.dataset.theme = t;
    $$('#themeseg button').forEach(b => b.classList.toggle('on', b.dataset.t === t));
  },

  async init() {
    $('#app').hidden = false;
    let th = 'auto'; try { th = localStorage.getItem('theme') || 'auto'; } catch (e) {}
    this.setTheme(th);
    this.bind(); this.bindPlayer();
    if (matchMedia('(min-width:1100px)').matches) $('#v-results').classList.add('fopen');
    invoke('addons_seed').catch(() => {});
    const c = await invoke('config_get').catch(() => ({}));
    this.hasKey = !!c.has_key;
    const h = location.hash.slice(1);
    this.renderResults();
    this.renderHitsEmpty();
    this.show(['find', 'results', 'addons', 'library', 'playlists', 'watching', 'player', 'settings'].includes(h) ? h : (c.has_key ? 'find' : 'settings'));
  },

  async refreshKeyBanner() {
    const c = await invoke('config_get').catch(() => ({}));
    this.hasKey = !!c.has_key; $('#keybanner').hidden = this.hasKey;
  },

  bind() {
    $$('#nav button').forEach(b => b.onclick = () => this.show(b.dataset.v));
    document.addEventListener('click', e => { const g = e.target.closest('[data-goto]'); if (g) this.show(g.dataset.goto); });
    $('#railtog').onclick = () => { const sh = $('.shell'); sh.classList.toggle('nav-open'); $('#railtog').setAttribute('aria-expanded', sh.classList.contains('nav-open')); };
    $('#navscrim').onclick = () => { $('.shell').classList.remove('nav-open'); $('#railtog').setAttribute('aria-expanded', 'false'); };
    document.addEventListener('keydown', e => { if (e.key === 'Escape') { $('.shell').classList.remove('nav-open'); $('#v-results').classList.remove('fopen-m'); if (!matchMedia('(min-width:1100px)').matches) $('#v-results').classList.remove('fopen'); } });
    $$('#kindseg button').forEach(b => b.onclick = () => {
      this.kind = b.dataset.k;
      $$('#kindseg button').forEach(x => x.classList.toggle('on', x === b));
      $('#epcard').hidden = this.kind !== 'series';
      this.renderHitsEmpty(); this.title = null;
    });
    $('#qgo').onclick = () => this.search();
    $('#q').onkeydown = e => { if (e.key === 'Enter') this.search(); };
    $('#imdb').onkeydown = e => { if (e.key === 'Enter') this.fetchStreams(); };
    $('#fetchgo').onclick = () => this.fetchStreams();
    const rs = $('#v-results'), fb = $('#fbtn');
    const setF = on => { rs.classList.toggle('fopen', on); fb.setAttribute('aria-expanded', on); };
    fb.setAttribute('aria-expanded', rs.classList.contains('fopen'));
    fb.onclick = () => setF(!rs.classList.contains('fopen'));
    $('#fclose').onclick = () => setF(false); $('#fscrim').onclick = () => setF(false);
    $('#freset').onclick = () => this.resetFilters();
    ['#sort', '#f-res', '#f-max', '#f-min', '#f-seeds', '#f-ex', '#f-in', '#f-cached', '#f-best']
      .forEach(s => $(s).addEventListener('input', () => this.applyFilters()));
    $('#selall').onclick = () => { this.view.forEach(s => this.sel.add(s._id)); this.renderResults(); };
    $('#selnone').onclick = () => { this.sel.clear(); this.renderResults(); };
    $('#sendgo').onclick = () => this.send();
    $('#libreload').onclick = () => this.loadLibrary();
    $('#libq').oninput = () => this.renderLibrary();
    $('#libgo').onclick = () => this.buildFromLibrary();
    $('#s-save').onclick = () => this.saveSettings();
    $('#s-eye').onclick = () => { const k = $('#s-key'), show = k.type === 'password'; k.type = show ? 'text' : 'password'; $('#s-eye').innerHTML = ic(show ? 'eyeoff' : 'eye'); $('#s-eye').setAttribute('aria-label', show ? 'Hide key' : 'Show key'); };
    $$('#themeseg button').forEach(b => b.onclick = () => this.setTheme(b.dataset.t));
    $$('#subnav button').forEach(b => b.onclick = () => {
      $$('#subnav button').forEach(x => x.classList.toggle('on', x === b));
      $$('.sgroup').forEach(g => g.hidden = g.id !== 's-' + b.dataset.s);
    });
    $('#ad-add').onclick = () => this.addAddon();
    $('#ad-url').onkeydown = e => { if (e.key === 'Enter') this.addAddon(); };
    $('#ad-refresh').onclick = async () => {
      const b = $('#ad-refresh'); b.disabled = true;
      for (const a of this.addons || []) await invoke('addons_refresh', {id: a.id}).catch(() => {});
      b.disabled = false; this.loadAddons();
    };
    $('#ad-export').onclick = () => this.exportAddons();
    $('#ad-import').onclick = () => this.importAddons();
    document.addEventListener('click', e => { const a = e.target.closest('[data-act]'); if (!a) return;
      if (a.dataset.act === 'resetf') this.resetFilters(); else if (a.dataset.act === 'goto-find') this.show('find'); else if (a.dataset.act === 'retry-lib') this.loadLibrary(); });
  },

  /* ---------- FIND ---------- */
  renderHitsEmpty() {
    $('#hits').innerHTML = stateBox('film', this.kind === 'movie' ? 'Find a movie' : 'Find a series', 'Search by title above, or paste an IMDb id below.');
  },
  async search() {
    const q = $('#q').value.trim(); if (!q) return $('#q').focus();
    $('#hits').innerHTML = skeleton(6, 'sk-hit');
    const r = await this.api('/api/search', {query: q, kind: this.kind});
    if (r.error) return $('#hits').innerHTML = stateBox('alert', 'Search failed', r.error, {err: true});
    $('#hits').innerHTML = r.results.map((h, i) => `<button class="hit" data-i="${i}" style="--i:${i};--h:${hue(h.name)}">
      <div class="poster"><b>${esc(initial(h.name))}</b>${h.poster ? `<img loading="lazy" src="${esc(h.poster)}" alt="" onerror="this.remove()">` : ''}</div>
      <div class="hit-t"><b>${esc(h.name)}</b><span>${esc(h.year || '')} · ${esc(h.id)}</span></div></button>`).join('')
      || stateBox('search', 'No matches', 'Try another spelling, or paste the IMDb id directly.');
    $$('#hits .hit').forEach(el => el.onclick = () => {
      const h = r.results[+el.dataset.i]; this.title = h; $('#imdb').value = h.id;
      $('#plname').value = h.name;
      $('#hits').innerHTML = `<div class="picked" style="--h:${hue(h.name)}"><div class="poster"><b>${esc(initial(h.name))}</b>${h.poster ? `<img src="${esc(h.poster)}" alt="" onerror="this.remove()">` : ''}</div>
        <div class="pk-t"><b>${esc(h.name)}</b><span>${esc(h.year || '')} · ${esc(h.id)}</span></div><span class="check">${ic('check')}</span></div>`;
    });
  },
  async fetchStreams() {
    const id = $('#imdb').value.trim(), st = $('#fstatus');
    st.classList.remove('err');
    if (!/^tt\d+$/.test(id)) { st.classList.add('err'); st.textContent = 'Pick a title or enter an IMDb id like tt1375666.'; return $('#imdb').focus(); }
    st.innerHTML = '<span class="spin"></span>Fetching from your addons… (can take a while for many episodes)';
    $('#fetchgo').disabled = true;
    try {
      const r = await this.api('/api/fetch', {kind: this.kind, imdb: id, season: +$('#season').value, ep1: +$('#ep1').value, ep2: +$('#ep2').value});
      if (r.error) { st.classList.add('err'); return st.textContent = r.error; }
      this.streams = r.streams.map((s, i) => ({...s, _id: i}));
      this.errors = r.errors || [];
      this.sel.clear(); this.buildChips(); this.applyFilters(true);
      st.textContent = `${this.streams.length} streams fetched.`;
      this.show('results');
    } finally { $('#fetchgo').disabled = false; }
  },

  /* ---------- FILTERS ---------- */
  buildChips() {
    this.chips = {};
    for (const [key, field] of [['addon', 'addon_name'], ['codec', 'codec'], ['hdr', 'hdr'], ['audio', 'audio'], ['quality', 'quality']]) {
      const vals = [...new Set(this.streams.map(s => s[field]).filter(Boolean))].sort();
      const box = $('#c-' + key);
      $$('.chip', box).forEach(c => c.remove());
      this.chips[key] = new Set();
      vals.forEach(v => {
        const c = document.createElement('button'); c.className = 'chip'; c.textContent = v; c.setAttribute('aria-pressed', 'false');
        c.onclick = () => { const s = this.chips[key]; s.has(v) ? s.delete(v) : s.add(v); c.classList.toggle('on'); c.setAttribute('aria-pressed', s.has(v)); this.applyFilters(); };
        box.append(c);
      });
      box.hidden = !vals.length;
    }
  },
  resetFilters() {
    $('#f-res').value = 1080; $('#f-max').value = 0; $('#f-min').value = 0; $('#f-seeds').value = 0;
    $('#f-ex').value = 'cam,telesync,hdts'; $('#f-in').value = ''; $('#f-cached').checked = true; $('#f-best').checked = true;
    $$('.chip.on').forEach(c => { c.classList.remove('on'); c.setAttribute('aria-pressed', 'false'); }); Object.values(this.chips).forEach(s => s.clear());
    this.applyFilters();
  },
  applyFilters(keepSel) {
    const minRes = +$('#f-res').value, mx = +$('#f-max').value * 1073741824, mn = +$('#f-min').value * 1073741824,
      seeds = +$('#f-seeds').value, ex = $('#f-ex').value.toLowerCase().split(',').map(x => x.trim()).filter(Boolean),
      inc = $('#f-in').value.toLowerCase().trim(), cached = $('#f-cached').checked;
    const chipOk = (key, v) => !this.chips[key] || !this.chips[key].size || this.chips[key].has(v);
    let list = this.streams.filter(s => {
      const t = (s.title + ' ' + s.source).toLowerCase();
      return s.res_rank >= minRes && (!mx || s.size <= mx) && s.size >= mn && (s.seeders ?? 0) >= seeds && !s.bad &&
        !ex.some(w => t.includes(w)) && (!inc || t.includes(inc)) && (!cached || s.cached) &&
        chipOk('addon', s.addon_name) && chipOk('codec', s.codec) && chipOk('hdr', s.hdr) && chipOk('audio', s.audio) && chipOk('quality', s.quality);
    });
    const sorters = {
      best: (a, b) => b.res_rank - a.res_rank || (b.cached - a.cached) || (b.seeders || 0) - (a.seeders || 0),
      res: (a, b) => b.res_rank - a.res_rank || b.size - a.size,
      size: (a, b) => b.size - a.size, sizeasc: (a, b) => a.size - b.size,
      seeds: (a, b) => (b.seeders || 0) - (a.seeders || 0),
    };
    list.sort((a, b) => (a.season || 0) - (b.season || 0) || (a.episode || 0) - (b.episode || 0) || sorters[$('#sort').value](a, b));
    if ($('#f-best').checked) {
      const seen = new Set();
      list = list.filter(s => { const k = (s.season ?? 0) + ':' + (s.episode ?? 0); if (seen.has(k)) return false; seen.add(k); return true; });
    }
    this.view = list;
    if (!keepSel) { const ids = new Set(list.map(s => s._id)); this.sel = new Set([...this.sel].filter(i => ids.has(i))); }
    else this.sel = new Set(list.map(s => s._id));
    const n = [minRes !== 1080, mx, mn, seeds, inc, !cached, ...Object.values(this.chips).map(s => s.size)].filter(Boolean).length;
    $('#fcount').hidden = !n; $('#fcount').textContent = n;
    this.renderResults();
  },

  renderResults() {
    const total = this.streams.length;
    $('#rcount').hidden = !total; $('#rcount').textContent = this.view.length;
    $('#rtitle').textContent = this.title?.name || 'Results';
    const errs = this.errors?.length ? ` · ${this.errors.length} episode error${this.errors.length === 1 ? '' : 's'}: ${this.errors.slice(0, 2).join('; ')}` : '';
    $('#rstatus').textContent = total ? `${this.view.length} of ${total} streams shown${errs}` : 'Nothing fetched yet.';
    $('#fbtn').hidden = $('#sort').closest('label').hidden = !total;
    if (!total) { $('#rlist').innerHTML = stateBox('results', 'No results yet', 'Search for a title and fetch streams to see them here.', {action: ['Go to Search', 'goto-find']}); return this.updateBar(); }
    if (!this.view.length) { $('#rlist').innerHTML = stateBox('filter', 'Nothing matches your filters', 'Loosen the resolution, size or cached-only filters.', {action: ['Reset filters', 'resetf']}); return this.updateBar(); }
    let last = null;
    $('#rlist').innerHTML = this.view.map((s, n) => {
      let head = '';
      if (s.season != null) { const k = s.season + ':' + s.episode; if (k !== last) { head = `<div class="ep-head">Season ${s.season} · Episode ${s.episode}</div>`; last = k; } }
      const rr = s.res_rank >= 2160 ? 'r4k' : s.res_rank === 1080 ? 'r1080' : '';
      return head + `<label class="item ${this.sel.has(s._id) ? 'sel' : ''}" style="--i:${n}">
      <input class="chk" type="checkbox" data-id="${s._id}" ${this.sel.has(s._id) ? 'checked' : ''}><span class="cbx">${'<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5"/></svg>'}</span>
      <div class="body"><div class="t">${esc(s.title)}</div>
       <div class="src">${esc(s.source)}</div>
       <div class="badges">
        ${s.resolution ? `<span class="b res ${rr}">${esc(s.resolution)}</span>` : ''}
        ${s.cached ? `<span class="b ok">${ic('check')}Cached</span>` : ''}
        ${[s.quality, s.codec, s.hdr, s.audio].filter(Boolean).map(x => `<span class="b">${esc(x)}</span>`).join('')}
        ${s.size ? `<span class="b">${gb(s.size)}</span>` : ''}
        ${s.seeders != null ? `<span class="b">${ic('seed')}${s.seeders}</span>` : ''}
        ${s.langs?.length ? `<span class="b">${esc(s.langs.join(', '))}</span>` : ''}
        ${s.addon_name ? `<span class="b addon" style="--h:${hue(s.addon_name)}">${esc(s.addon_name)}</span>` : ''}
       </div></div></label>`;
    }).join('');
    $$('#rlist input').forEach(c => c.onchange = () => {
      const id = +c.dataset.id; c.checked ? this.sel.add(id) : this.sel.delete(id);
      c.closest('.item').classList.toggle('sel', c.checked); this.updateBar();
    });
    this.updateBar();
  },
  updateBar() {
    $('#sendbar').hidden = !this.view.length;
    $('#selinfo').textContent = `${this.sel.size} selected`;
    $('#sendgo').disabled = !this.sel.size;
  },

  /* ---------- SEND / JOBS ---------- */
  async send() {
    const items = this.view.filter(s => this.sel.has(s._id)).map(s => ({
      title: s.title, magnet: s.magnet, url: s.url, season: s.season, episode: s.episode, add_only: $('#o-addonly').checked}));
    if (!items.length) return;
    $('#sendgo').disabled = true;
    const r = await this.api('/api/send', {items, name: $('#plname').value || 'playlist', append: $('#o-append').checked});
    this.updateBar();
    if (r.error) return this.err(r);
    this.poll(r.job, $('#job'));
    $('#job').scrollIntoView({behavior: 'smooth', block: 'nearest'});
  },
  async poll(id, box) {
    const j = await this.api('/api/job/' + id);
    if (j.error) { box.innerHTML = stateBox('alert', 'Could not read job', j.error, {err: true}); return; }
    const tone = st => /done|ok|ready|complete/.test(st) ? 'ok' : /err|fail/.test(st) ? 'bad' : st === 'queued' ? 'q' : 'run';
    const fin = j.rows.filter(r => tone(r.state) !== 'run' && tone(r.state) !== 'q').length, pct = j.rows.length ? Math.round(fin / j.rows.length * 100) : 0;
    box.innerHTML = `<div class="card job"><div class="job-h"><h3>${j.finished ? 'Finished' : 'Sending to TorBox…'}</h3><span>${fin}/${j.rows.length}</span></div>
      <div class="pbar ${j.finished ? 'done' : ''}" role="progressbar" aria-valuenow="${pct}" aria-valuemin="0" aria-valuemax="100"><i style="width:${pct}%"></i></div>` +
      j.rows.map(r => { const t = tone(r.state); return `<div class="jrow"><span class="jdot ${t}">${t === 'ok' ? ic('check') : t === 'bad' ? ic('x') : t === 'run' ? '<span class="spin"></span>' : ''}</span>
      <span class="jt" title="${esc(r.title)}">${esc(r.title)}</span><span class="jd state-${esc(r.state)}">${esc(r.detail || r.state)}</span></div>`; }).join('') + '</div>';
    if (!j.finished) return setTimeout(() => this.poll(id, box), 2000);
    if (j.playlist) { box.firstChild.append(this.playerButtons(j.playlist)); this.toast('Playlist ready'); }
  },

  /* ---------- player tab ---------- */
  async loadWatching() {
    const left = t => { t = Math.max(0, Math.floor(t || 0)); const h = Math.floor(t / 3600), m = Math.floor(t % 3600 / 60); return h ? `${h} h ${m} min` : `${Math.max(1, m)} min`; };
    const clean = raw => {
      let t = String(raw || ''); try { t = decodeURIComponent(t); } catch (e) {}
      t = t.replace(/\.(mkv|mp4|avi|m3u8?)$/i, '');
      const ep = t.match(/S(\d{1,2})E(\d{1,3})/i), yr = t.match(/[.\s_(]((?:19|20)\d{2})[.\s_)]/);
      const name = (t.split(/[.\s_]+(?=S\d{1,2}E\d|(?:19|20)\d{2}[.\s_]|2160p|1080p|720p|480p|UHD|BluRay|WEB)/i)[0] || t).replace(/[._]+/g, ' ').trim();
      return {name, s: ep ? +ep[1] : null, e: ep ? +ep[2] : null, ep: ep ? `S${ep[1].padStart(2, '0')} · E${ep[2].padStart(2, '0')}` : '', year: yr ? yr[1] : ''};
    };
    const hue = s => { let h = 0; for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360; return h; };
    const rec = await invoke('player_recent').catch(() => []);
    const done = r => r.dur > 0 && r.pos / r.dur >= 0.95;
    const card = r => {
      const c = clean(r.title), pct = r.dur > 0 ? Math.min(100, r.pos / r.dur * 100) : 0;
      const sub = r.dur > 0 ? (done(r) ? 'Watched' : left(r.dur - r.pos) + ' left') : (r.pos > 5 ? 'Resume at ' + left(r.pos) : 'Not started');
      return `<article class="wt-card" data-u="${esc(r.url)}" style="--h:${hue(c.name)}"><button class="wt-art" aria-label="Play ${esc(c.name)}"><span class="wt-init">${esc(c.name.slice(0, 2).toUpperCase())}</span>
        <span class="wt-play">${ic('play')}</span>${pct ? `<span class="wt-bar"><i style="width:${pct}%"></i></span>` : ''}</button>
        <button class="icon-btn wt-x" aria-label="Remove from list">${ic('x')}</button>
        <div class="wt-meta"><b title="${esc(c.name)}">${esc(c.name)}</b><span class="wt-line">${esc([c.ep, c.year].filter(Boolean).join(' · '))}</span><small class="wt-eptitle"></small><em>${sub}</em></div></article>`;
    };
    const row = (title, list) => list.length ? `<h3 class="ct pl-h">${title}</h3><div class="wt-grid">${list.map(card).join('')}</div>` : '';
    $('#wt-body').innerHTML = rec.length ? row('Continue watching', rec.filter(r => !done(r))) + row('Watched', rec.filter(done))
      : '<div class="card pad"><p class="mut">Nothing yet. Play something from Playlists or the Player tab and it shows up here with your progress.</p></div>';
    $$('#wt-body .wt-card').forEach(el => {
      const r = rec.find(x => x.url === el.dataset.u);
      el.querySelector('.wt-art').onclick = () => {
        this.show('player');
        (this.resetStage(), invoke('player_play', {entries: [{title: r.title, url: r.url}], index: 0})).then(() => { this.syncVideo(); setTimeout(() => { try { $('#pframe').contentWindow.focus(); } catch (e) {} }, 300); }).catch(e => this.err(e));
      };
      el.querySelector('.wt-x').onclick = async () => { await invoke('player_recent_remove', {url: r.url}).catch(() => {}); this.loadWatching(); };
      this.enrichCard(el, clean(r.title));
    });
  },
  /* Series/movie metadata (poster, episode title, rating, summary) from the user's metadata addon; cached on-device. */
  async enrichCard(el, c) {
    const key = 'meta:' + c.name.toLowerCase() + '|' + (c.s ?? '') + '|' + (c.e ?? '');
    let m = null;
    try { m = JSON.parse(localStorage.getItem(key) || 'null'); } catch (e) {}
    if (!m) {
      m = await invoke('title_meta', {name: c.name, series: c.s != null, season: c.s, episode: c.e}).catch(() => null);
      if (m) try { localStorage.setItem(key, JSON.stringify(m)); } catch (e) {}
    }
    if (!m || !el.isConnected) return;
    const img = m.ep_thumb || m.background || m.poster;
    const art = el.querySelector('.wt-art');
    if (img && /^https?:/.test(img)) { art.style.backgroundImage = `url("${img.replace(/"/g, '%22')}")`; art.classList.add('has-img'); }
    el.querySelector('b').textContent = m.name || c.name;
    const line = [c.ep, String(m.year || '').replace(/[–-]\s*$/, ''), m.rating ? '★ ' + m.rating : '', (m.genres || []).slice(0, 2).join(', ')].filter(Boolean).join(' · ');
    el.querySelector('.wt-line').textContent = line;
    el.querySelector('.wt-eptitle').textContent = m.ep_title || '';
    const tip = [m.ep_title, m.ep_overview || m.description].filter(Boolean).join('\n\n');
    if (tip) el.querySelector('.wt-meta').title = tip;
  },
  async loadPlayer() {
    const fmt = t => { t = Math.max(0, Math.floor(t || 0)); const h = Math.floor(t / 3600), m = Math.floor(t % 3600 / 60), s = t % 60; return (h ? h + ':' + String(m).padStart(2, '0') : m) + ':' + String(s).padStart(2, '0'); };
    const play = async (entries, index = 0) => { try { await (this.resetStage(), invoke('player_play', {entries, index})); this.syncVideo(); setTimeout(() => { try { $('#pframe').contentWindow.focus(); } catch (e) {} }, 300); } catch (e) { this.err(e); } };
    $('#pl-go').onclick = () => { const v = $('#pl-open').value.trim(); if (!v) return; play([{title: v.split('/').pop().split('?')[0] || v, url: v}]); };
    $('#pl-open').onkeydown = e => { if (e.key === 'Enter') $('#pl-go').click(); };
    const keys = await invoke('player_keys').catch(() => []);
    $('#pl-keys').innerHTML = keys.map(k => `<div class="krow"><kbd>${esc(k.key)}</kbd><span>${esc(k.desc)}</span></div>`).join('');
    this.syncVideo();
  },
  resetStage() { try { $('#pframe').contentWindow.postMessage({t: 'reset'}, '*'); } catch (e) {} },
  syncVideo() {
    if (this._vrq) return;
    this._vrq = requestAnimationFrame(() => {
      this._vrq = 0;
      const r = $('#v-player').hidden || !this.vOn ? null : $('#pframe').getBoundingClientRect();
      const W = innerWidth, H = innerHeight;
      document.documentElement.style.setProperty('--hole', r ? `path(evenodd, "M0 0H${W}V${H}H0Z M${r.left} ${r.top}H${r.right}V${r.bottom}H${r.left}Z")` : 'none');
      invoke('player_set_rect', r ? {x: r.left, y: r.top, w: r.width, h: r.height} : {x: 0, y: 0, w: 0, h: 0}).catch(() => {});
    });
  },
  async playerFs() {
    const st = $('#pstage'), on = !st.classList.contains('pfs');
    st.classList.toggle('pfs', on);
    try { await window.__TAURI__.window.getCurrentWindow().setFullscreen(on); } catch (e) {}
    try { $('#pframe').contentWindow.postMessage({t: 'fs-state', on}, '*'); } catch (e) {}
    setTimeout(() => this.syncVideo(), 150); setTimeout(() => this.syncVideo(), 700);
  },
  bindPlayer() {
    new ResizeObserver(() => this.syncVideo()).observe($('#pframe'));
    addEventListener('resize', () => this.syncVideo()); addEventListener('scroll', () => this.syncVideo(), {passive: true});
    addEventListener('message', e => { if (e.data && e.data.t === 'player-fs') this.playerFs(); if (e.data && e.data.t === 'video-on') { this.vOn = !!e.data.on; this.syncVideo(); } });
  },

  /* ---------- players ---------- */
  playerButtons(file) {
    const wrap = document.createElement('div'); wrap.className = 'pl-actions';
    const desktop = PLAT === 'mac' || PLAT === 'windows' || PLAT === 'other';
    let h = `<button class="btn primary" data-app>${ic('play')}Play in app</button>
      <button class="btn" data-export>${ic('download')}Export to Downloads</button><button class="btn" data-copy>${ic('copy')}Copy playlist</button>`;
    if (PLAT === 'mac') h += '<button class="btn" data-play="IINA">Open in IINA</button><button class="btn" data-play="VLC">VLC</button><button class="btn" data-play="mpv">mpv</button>';
    else if (desktop) h += '<button class="btn" data-play="">Open in default player</button>';
    wrap.innerHTML = h;
    wrap.querySelector('[data-app]').onclick = async () => { this.show('player'); this.resetStage(); const r = await this.api('/api/player/open', {name: file}); if (r.error) this.err(r.error); else setTimeout(() => { try { $('#pframe').contentWindow.focus(); } catch (e) {} }, 300); };
    wrap.querySelector('[data-copy]').onclick = async () => { try { await navigator.clipboard.writeText(await invoke('playlist_text', {file})); this.toast('Playlist copied'); } catch (e) { this.err(e); } };
    wrap.querySelector('[data-export]').onclick = async () => { try { this.toast('Saved: ' + await invoke('playlist_export', {file}), 4000); } catch (e) { this.err(e); } };
    $$('[data-play]', wrap).forEach(b => b.onclick = async () => { try { await invoke('open_playlist', {file, player: b.dataset.play || null}); } catch (e) { this.err(e); } });
    return wrap;
  },

  /* ---------- LIBRARY ---------- */
  async loadLibrary() {
    $('#liblist').innerHTML = skeleton(4, 'sk-item');
    const r = await this.api('/api/torbox/library');
    if (r.error) return $('#liblist').innerHTML = stateBox('alert', 'Could not load your library', r.error, {err: true, action: ['Try again', 'retry-lib']});
    this.lib = r.items; this.libSel = new Set(); this.renderLibrary();
  },
  renderLibrary() {
    const q = $('#libq').value.toLowerCase();
    const items = (this.lib || []).filter(i => (i.name || '').toLowerCase().includes(q));
    $('#liblist').innerHTML = items.map((i, n) => `<label class="item ${this.libSel.has(i.id) ? 'sel' : ''}" style="--i:${n}"><input class="chk" type="checkbox" data-id="${i.id}" ${this.libSel.has(i.id) ? 'checked' : ''} ${i.ready ? '' : 'disabled'}><span class="cbx"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7.5"/></svg></span>
      <div class="body"><div class="t">${esc(i.name)}</div><div class="badges"><span class="b">${gb(i.size)}</span><span class="b">${ic('film')}${i.videos} video${i.videos === 1 ? '' : 's'}</span>
      ${i.ready ? `<span class="b ok">${ic('check')}Ready</span>` : `<span class="b">${esc(i.state || '')} ${Math.round((i.progress || 0) * 100)}%</span>`}</div>
      ${i.ready ? '' : `<div class="pbar"><i style="width:${Math.round((i.progress || 0) * 100)}%"></i></div>`}</div></label>`).join('')
      || (this.lib && this.lib.length ? stateBox('search', 'No matches', 'Nothing in your library matches that filter.') : stateBox('box', 'Your TorBox is empty', 'Send something from Search and it will show up here.', {action: ['Go to Search', 'goto-find']}));
    $$('#liblist input').forEach(c => c.onchange = () => { c.checked ? this.libSel.add(+c.dataset.id) : this.libSel.delete(+c.dataset.id); c.closest('.item').classList.toggle('sel', c.checked); });
  },
  async buildFromLibrary() {
    const items = (this.lib || []).filter(i => this.libSel.has(i.id)).map(i => ({title: i.name, torrent_id: i.id, all_files: true}));
    if (!items.length) return this.toast('Select at least one');
    const r = await this.api('/api/send', {items, name: $('#libname').value || 'torbox', append: false});
    if (r.error) return this.err(r);
    this.poll(r.job, $('#libjob'));
  },

  /* ---------- PLAYLISTS ---------- */
  async loadPlaylists() {
    $('#pledit').hidden = true; $('.pl-grid').classList.remove('has-edit');
    $('#pllist').innerHTML = skeleton(3, 'sk-item');
    const r = await this.api('/api/playlists');
    if (r.error) return $('#pllist').innerHTML = stateBox('alert', 'Could not load playlists', r.error, {err: true});
    $('#pllist').innerHTML = r.playlists.map((p, n) => `<button class="item pl-item" data-open="${esc(p.file)}" style="--i:${n}"><span class="art">${ic('playlist')}</span><div class="body"><div class="t">${esc(p.name)}</div>
      <div class="src">${p.count} item${p.count === 1 ? '' : 's'} · ${new Date(p.mtime * 1000).toLocaleDateString(undefined, {month: 'short', day: 'numeric', year: 'numeric'})}</div></div><span class="chev">${ic('chev')}</span></button>`).join('')
      || stateBox('playlist', 'No playlists yet', 'Send streams to TorBox from Search and your playlists will appear here.', {action: ['Go to Search', 'goto-find']});
    $$('#pllist [data-open]').forEach(b => b.onclick = () => this.editPlaylist(b.dataset.open));
  },
  async editPlaylist(file) {
    const r = await this.api('/api/playlist?file=' + encodeURIComponent(file));
    if (r.error) return this.err(r);
    let entries = r.entries; const old = file.replace(/\.m3u8$/, ''); const box = $('#pledit'); box.hidden = false;
    $('.pl-grid').classList.add('has-edit');
    $$('#pllist .pl-item').forEach(b => b.classList.toggle('sel', b.dataset.open === file));
    const draw = () => {
      box.innerHTML = `<div class="editor-h"><input id="pe-name" value="${esc(old)}" aria-label="Playlist name"><button class="btn primary" id="pe-save">Save</button>
        <button class="btn danger" id="pe-del" aria-label="Delete playlist">${ic('trash')}</button></div>
        <div class="mut small" style="margin-top:8px">${entries.length} item${entries.length === 1 ? '' : 's'}</div>
        <div class="pe-list">` +
        (entries.map((e, i) => `<div class="pl-entry"><span class="n">${i + 1}</span><span class="et">${esc(e.title)}</span>
          <button class="icon-btn" data-up="${i}" aria-label="Move up" ${i ? '' : 'disabled'}>${ic('up')}</button><button class="icon-btn" data-dn="${i}" aria-label="Move down" ${i < entries.length - 1 ? '' : 'disabled'}>${ic('down')}</button>
          <button class="icon-btn danger" data-rm="${i}" aria-label="Remove">${ic('x')}</button></div>`).join('') || '<p class="mut small">This playlist is empty.</p>') + '</div><div id="pe-actions"></div>';
      $('#pe-actions').replaceWith(Object.assign(this.playerButtons(file), {id: 'pe-actions'}));
      $$('[data-up]', box).forEach(b => b.onclick = () => { const i = +b.dataset.up; [entries[i - 1], entries[i]] = [entries[i], entries[i - 1]]; draw(); });
      $$('[data-dn]', box).forEach(b => b.onclick = () => { const i = +b.dataset.dn; [entries[i + 1], entries[i]] = [entries[i], entries[i + 1]]; draw(); });
      $$('[data-rm]', box).forEach(b => b.onclick = () => { entries.splice(+b.dataset.rm, 1); draw(); });
      $('#pe-save').onclick = async () => { const n = $('#pe-name').value || old; const x = await this.api('/api/playlist/save', {name: n, old, entries}); if (x.error) return this.err(x); this.toast('Saved'); this.loadPlaylists(); };
      $('#pe-del').onclick = async () => { if (await this.ask({title: 'Delete this playlist?', msg: old + ' will be removed from this device.', ok: 'Delete', danger: true})) { await this.api('/api/playlist/delete', {file}); this.loadPlaylists(); } };
    };
    draw(); if (!matchMedia('(min-width:1000px)').matches) box.scrollIntoView({behavior: 'smooth'});
  },

  /* ---------- ADDONS ---------- */
  async loadAddons() {
    if (!this.addons) $('#ad-list').innerHTML = skeleton(3, 'sk-item');
    try { this.addons = await invoke('addons_list'); } catch (e) { return $('#ad-list').innerHTML = stateBox('alert', 'Could not load addons', String(e), {err: true}); }
    const L = this.addons;
    $('#ad-list').innerHTML = L.map((a, i) => {
      const st = a.last_ok === false ? `<span class="hstat bad" title="${esc(a.last_error || '')}">${esc((a.last_error || 'error').slice(0, 40))}</span>`
        : a.last_ok ? `<span class="hstat ok">Online${a.latency_ms != null ? ' · ' + a.latency_ms + ' ms' : ''}</span>` : '<span class="hstat">Untested</span>';
      return `<div class="card ad ${a.enabled ? '' : 'off'}" data-id="${esc(a.id)}" style="--i:${i};--h:${hue(a.name)}">
      <span class="ad-rank">#${i + 1}</span>
      <div class="ad-main">
       <div class="ad-logo">${esc(initial(a.name))}${a.logo ? `<img src="${esc(a.logo)}" alt="" onerror="this.remove()">` : ''}</div>
       <div class="ad-info"><div class="ad-name">${esc(a.name)} <small>v${esc(a.version)}</small></div>
        <div class="ad-desc">${esc(a.description)}</div>
        <div class="badges">${st}${a.resources.map(r => `<span class="b">${esc(r)}</span>`).join('')}${a.types.map(t => `<span class="b">${esc(t)}</span>`).join('')}<span class="b">${ic('clock')}${a.timeout_secs}s</span></div>
        <div class="ad-url" title="${esc(a.url_masked)}">${esc(a.url_masked)}</div></div>
      </div>
      <div class="ad-foot">
       <label class="tgl on-l" title="${a.enabled ? 'Enabled' : 'Disabled'}"><input type="checkbox" data-a="toggle" ${a.enabled ? 'checked' : ''} aria-label="Enabled"><i class="sw"></i><span style="margin-left:8px">${a.enabled ? 'On' : 'Off'}</span></label>
       <span class="sp"></span>
       <button class="btn sm" data-a="refresh" aria-label="Test">${ic('bolt')}<span>Test</span></button>
       ${a.configurable ? `<button class="btn sm" data-a="configure" aria-label="Configure">${ic('sliders')}<span>Configure</span></button>` : ''}
       <button class="btn sm" data-a="rename" aria-label="Rename">${ic('edit')}<span>Rename</span></button>
       <button class="btn sm" data-a="timeout" aria-label="Timeout">${ic('clock')}<span>Timeout</span></button>
       <button class="btn sm" data-a="copy" aria-label="Copy URL">${ic('copy')}<span>Copy URL</span></button>
       <button class="btn sm" data-a="up" aria-label="Move up" ${i ? '' : 'disabled'}>${ic('up')}</button><button class="btn sm" data-a="down" aria-label="Move down" ${i < L.length - 1 ? '' : 'disabled'}>${ic('down')}</button>
       <button class="btn sm danger" data-a="remove" aria-label="Remove">${ic('trash')}<span>Remove</span></button>
      </div></div>`; }).join('') || stateBox('addons', 'No addons yet', 'Paste a Stremio addon manifest URL above to start finding streams.');
    $$('#ad-list [data-a]').forEach(b => { const f = () => this.addonAction(b.closest('.ad').dataset.id, b.dataset.a); b.type === 'checkbox' ? b.onchange = f : b.onclick = f; });
  },
  async addAddon() {
    const url = $('#ad-url').value.trim(); if (!url) return $('#ad-url').focus();
    const m = $('#ad-msg'); m.classList.remove('err'); m.innerHTML = '<span class="spin"></span>Adding…'; $('#ad-add').disabled = true;
    try { await invoke('addons_add', {url}); $('#ad-url').value = ''; m.textContent = ''; this.loadAddons(); }
    catch (e) { m.classList.add('err'); m.textContent = String(e); }
    $('#ad-add').disabled = false;
  },
  async addonAction(id, act) {
    const a = this.addons.find(x => x.id === id);
    try {
      if (act === 'toggle') await invoke('addons_update', {id, enabled: !a.enabled});
      else if (act === 'rename') { const n = await this.ask({title: 'Rename addon', ok: 'Save', input: {value: a.name}}); if (n) await invoke('addons_update', {id, name: n}); }
      else if (act === 'timeout') { const t = await this.ask({title: 'Timeout', msg: 'Seconds to wait for this addon (3 to 120).', ok: 'Save', input: {value: a.timeout_secs}}); if (t) await invoke('addons_update', {id, timeoutSecs: +t}); }
      else if (act === 'refresh') { this.toast('Testing ' + a.name + '…', 1500); await invoke('addons_refresh', {id}); }
      else if (act === 'configure') await invoke('addon_configure', {id});
      else if (act === 'copy') { await navigator.clipboard.writeText(await invoke('addon_url', {id})); this.toast('Addon URL copied (keep it private)'); }
      else if (act === 'remove') { if (!await this.ask({title: 'Remove ' + a.name + '?', msg: 'You can add it again later with its URL.', ok: 'Remove', danger: true})) return; await invoke('addons_remove', {id}); }
      else if (act === 'up' || act === 'down') {
        const ids = this.addons.map(x => x.id), i = ids.indexOf(id), j = act === 'up' ? i - 1 : i + 1;
        [ids[i], ids[j]] = [ids[j], ids[i]]; await invoke('addons_reorder', {ids});
      }
    } catch (e) { this.err(e); }
    this.loadAddons();
  },
  async exportAddons() {
    if (!await this.ask({title: 'Export addon URLs?', msg: 'The export contains full addon URLs, which may include private settings. Copy to clipboard?', ok: 'Copy'})) return;
    try { const l = await invoke('addons_export'); await navigator.clipboard.writeText(l.map(x => x.url).join('\n')); this.toast('Copied ' + l.length + ' URLs'); } catch (e) { this.err(e); }
  },
  async importAddons() {
    const t = await this.ask({title: 'Import addons', msg: 'Paste addon URLs, separated by spaces or new lines.', ok: 'Import', input: {placeholder: 'https://…/manifest.json'}}); if (!t) return;
    try {
      const r = await invoke('addons_import', {urls: t.split(/\s+/).filter(Boolean)});
      this.toast(`Added ${r.added}` + (r.errors.length ? `, ${r.errors.length} failed: ${r.errors[0]}` : ''), 5000, r.errors.length ? 'bad' : ''); this.loadAddons();
    } catch (e) { this.err(e); }
  },

  /* ---------- SETTINGS ---------- */
  async loadSettings() {
    const c = await invoke('config_get').catch(() => ({}));
    this.hasKey = !!c.has_key;
    $('#s-key').placeholder = c.has_key ? '•••••• saved (leave blank to keep)' : 'paste your TorBox API key';
    const p = $('#s-state'); p.textContent = c.has_key ? 'Connected' : 'Not set'; p.classList.toggle('ok', !!c.has_key);
  },
  async saveSettings() {
    const m = $('#s-msg'); m.classList.remove('err');
    const r = await this.api('/api/config', {torbox_key: $('#s-key').value});
    $('#s-key').value = '';
    if (r.error) { m.classList.add('err'); m.textContent = r.error; } else { m.textContent = 'Saved'; this.toast('API key saved'); }
    this.loadSettings();
  },
};
App.init();
