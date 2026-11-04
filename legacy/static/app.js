'use strict';
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'}[c]));
const gb = b => b ? (b / 1073741824).toFixed(2) + ' GB' : '—';
const UA = navigator.userAgent;
const PLAT = /iPhone|iPad|iPod/.test(UA) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1) ? 'ios'
  : /Android/.test(UA) ? 'android' : /Win/.test(UA) ? 'windows' : /Mac/.test(UA) ? 'mac' : 'other';

const App = {
  kind: 'movie', title: null, streams: [], view: [], token: '', sel: new Set(), chips: {},

  async api(path, body) {
    const opt = body === undefined ? {} : {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)};
    const r = await fetch(path, opt);
    if (r.status === 401) { this.showLogin(); throw new Error('Locked'); }
    return r.json();
  },
  toast(msg, ms = 2600) {
    const t = $('#toast'); t.textContent = msg; t.hidden = false;
    clearTimeout(this._t); this._t = setTimeout(() => t.hidden = true, ms);
  },
  show(v) {
    $$('main > section').forEach(s => s.hidden = s.id !== 'v-' + v);
    $$('#nav button').forEach(b => b.classList.toggle('on', b.dataset.v === v));
    try { history.replaceState(null, '', '#' + v); } catch (e) {}
    if (v === 'library') this.loadLibrary();
    if (v === 'playlists') this.loadPlaylists();
    if (v === 'settings') this.loadSettings();
    window.scrollTo(0, 0);
  },
  showLogin() { $('#login').hidden = false; $('#app').hidden = true; },

  async init() {
    if ('serviceWorker' in navigator) navigator.serviceWorker.register('/sw.js').catch(() => {});
    const st = await fetch('/api/status').then(r => r.json());
    if (st.pin_required && !st.authed) return this.showLogin();
    this.token = st.token || '';
    $('#login').hidden = true; $('#app').hidden = false;
    this.bind();
    const h = location.hash.slice(1);
    this.show(['find', 'results', 'library', 'playlists', 'settings'].includes(h) ? h : (st.configured ? 'find' : 'settings'));
  },
  async login(e) {
    e.preventDefault();
    const r = await fetch('/api/login', {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({pin: $('#pin').value})});
    if (r.ok) location.reload(); else $('#loginerr').textContent = 'Wrong PIN';
  },

  bind() {
    $$('#nav button').forEach(b => b.onclick = () => this.show(b.dataset.v));
    $$('#kindseg button').forEach(b => b.onclick = () => {
      this.kind = b.dataset.k;
      $$('#kindseg button').forEach(x => x.classList.toggle('on', x === b));
      $('#epcard').hidden = this.kind !== 'series';
      $('#hits').innerHTML = ''; this.title = null;
    });
    $('#qgo').onclick = () => this.search();
    $('#q').onkeydown = e => { if (e.key === 'Enter') this.search(); };
    $('#fetchgo').onclick = () => this.fetchStreams();
    $('#fbtn').onclick = () => { const f = $('#filters'); f.hidden = !f.hidden; };
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
    $('#s-clearpin').onclick = async () => { await this.api('/api/config', {clear_pin: true}); this.toast('PIN removed'); this.loadSettings(); };
  },

  /* ---------- FIND ---------- */
  async search() {
    const q = $('#q').value.trim(); if (!q) return;
    $('#hits').innerHTML = '<p class="mut">Searching…</p>';
    const r = await this.api('/api/search', {query: q, kind: this.kind});
    if (r.error) return $('#hits').innerHTML = `<p class="bad">${esc(r.error)}</p>`;
    $('#hits').innerHTML = r.results.map((h, i) => `<div class="hit" data-i="${i}">
      ${h.poster ? `<img loading="lazy" src="${esc(h.poster)}" alt="">` : '<img alt="">'}
      <div><div>${esc(h.name)}</div><div class="mut">${esc(h.year || '')} · ${esc(h.id)}</div></div></div>`).join('') || '<p class="mut">No matches.</p>';
    $$('#hits .hit').forEach(el => el.onclick = () => {
      const h = r.results[+el.dataset.i]; this.title = h; $('#imdb').value = h.id;
      $('#plname').value = h.name; $('#hits').innerHTML = `<div class="hit picked"><div><b>${esc(h.name)}</b> <span class="mut">${esc(h.year || '')} · ${esc(h.id)}</span></div></div>`;
    });
  },
  async fetchStreams() {
    const id = $('#imdb').value.trim();
    if (!/^tt\d+$/.test(id)) return $('#fstatus').textContent = 'Pick a title or enter an IMDb id like tt1375666.';
    $('#fstatus').textContent = 'Fetching from AIOStreams… (can take a while for many episodes)';
    $('#fetchgo').disabled = true;
    try {
      const r = await this.api('/api/fetch', {kind: this.kind, imdb: id, season: +$('#season').value, ep1: +$('#ep1').value, ep2: +$('#ep2').value});
      if (r.error) return $('#fstatus').innerHTML = `<span class="bad">${esc(r.error)}</span>`;
      this.streams = r.streams.map((s, i) => ({...s, _id: i}));
      this.errors = r.errors || [];
      this.sel.clear(); this.buildChips(); this.applyFilters(true);
      $('#fstatus').textContent = `${this.streams.length} streams fetched.`;
      this.show('results');
    } finally { $('#fetchgo').disabled = false; }
  },

  /* ---------- FILTERS ---------- */
  buildChips() {
    this.chips = {};
    for (const [key, field] of [['codec', 'codec'], ['hdr', 'hdr'], ['audio', 'audio'], ['quality', 'quality']]) {
      const vals = [...new Set(this.streams.map(s => s[field]).filter(Boolean))].sort();
      const box = $('#c-' + key);
      $$('.chip', box).forEach(c => c.remove());
      this.chips[key] = new Set();
      vals.forEach(v => {
        const c = document.createElement('button'); c.className = 'chip'; c.textContent = v;
        c.onclick = () => { const s = this.chips[key]; s.has(v) ? s.delete(v) : s.add(v); c.classList.toggle('on'); this.applyFilters(); };
        box.append(c);
      });
      box.hidden = !vals.length;
    }
  },
  resetFilters() {
    $('#f-res').value = 1080; $('#f-max').value = 0; $('#f-min').value = 0; $('#f-seeds').value = 0;
    $('#f-ex').value = 'cam,telesync,hdts'; $('#f-in').value = ''; $('#f-cached').checked = true; $('#f-best').checked = true;
    $$('.chip.on').forEach(c => c.classList.remove('on')); Object.values(this.chips).forEach(s => s.clear());
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
        chipOk('codec', s.codec) && chipOk('hdr', s.hdr) && chipOk('audio', s.audio) && chipOk('quality', s.quality);
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
    $('#rstatus').textContent = total ? `${this.view.length} of ${total} shown` + (this.errors?.length ? ` · ${this.errors.length} episode errors: ${this.errors.slice(0, 2).join('; ')}` : '') : 'Nothing fetched yet — use Find.';
    $('#rlist').innerHTML = this.view.map(s => `<label class="item ${this.sel.has(s._id) ? 'sel' : ''}">
      <input type="checkbox" data-id="${s._id}" ${this.sel.has(s._id) ? 'checked' : ''}>
      <div class="body"><div class="t">${esc(s.title)}</div>
       <div class="src">${esc(s.source)}</div>
       <div class="badges">
        ${s.season != null ? `<span class="b">S${s.season}E${s.episode}</span>` : ''}
        ${s.resolution ? `<span class="b res">${esc(s.resolution)}</span>` : ''}
        ${[s.quality, s.codec, s.hdr, s.audio].filter(Boolean).map(x => `<span class="b">${esc(x)}</span>`).join('')}
        ${s.size ? `<span class="b">${gb(s.size)}</span>` : ''}
        ${s.seeders != null ? `<span class="b">${s.seeders} seeds</span>` : ''}
        ${s.langs?.length ? `<span class="b">${esc(s.langs.join(', '))}</span>` : ''}
        ${s.cached ? '<span class="b ok">cached</span>' : ''}
       </div></div></label>`).join('');
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
    const {job} = await this.api('/api/send', {items, name: $('#plname').value || 'playlist', append: $('#o-append').checked});
    this.poll(job, $('#job'));
  },
  async poll(id, box) {
    const j = await this.api('/api/job/' + id);
    box.innerHTML = '<div class="card">' + j.rows.map(r => `<div class="pl-entry"><span>${esc(r.title)}</span>
      <span class="state-${esc(r.state)}" style="flex:none">${esc(r.state)}</span><span class="mut" style="flex:none;font-size:12px">${esc(r.detail)}</span></div>`).join('') + '</div>';
    if (!j.finished) return setTimeout(() => this.poll(id, box), 2000);
    if (j.playlist) { box.append(this.playerButtons(j.playlist)); this.toast('Playlist ready'); }
  },

  /* ---------- players ---------- */
  plUrl(file) { return location.origin + '/playlist/' + encodeURIComponent(file) + (this.token ? '?k=' + this.token : ''); },
  playerButtons(file) {
    const u = this.plUrl(file), e = encodeURIComponent(u), wrap = document.createElement('div');
    wrap.className = 'pl-actions';
    const a = (label, href, cls = '', dl) => `<a class="btn ${cls}" href="${href}" ${dl ? 'download' : ''}>${label}</a>`;
    const rec = {ios: 'vlc', android: 'vlc', mac: 'iina', windows: 'dl', other: 'dl'}[PLAT];
    let h = '';
    h += a('Download .m3u8', esc(u), rec === 'dl' ? 'primary' : '', true);
    if (PLAT === 'mac') h += a('Open in IINA', `iina://weblink?url=${e}`, 'primary');
    if (PLAT === 'ios') h += a('Open in VLC', `vlc-x-callback://x-callback-url/stream?url=${e}`, 'primary') + a('Open in Infuse', `infuse://x-callback-url/play?url=${e}`);
    if (PLAT === 'android') h += a('Open in VLC', `vlc://${esc(u)}`, 'primary');
    if (PLAT === 'mac' || PLAT === 'windows') h += a('VLC', `vlc://${esc(u)}`);
    h += `<button class="btn" data-copy>Copy link</button>`;
    h += `<button class="btn" data-srv="IINA">Play on server: IINA</button><button class="btn" data-srv="mpv">mpv</button><button class="btn" data-srv="VLC">VLC</button>`;
    wrap.innerHTML = h;
    wrap.querySelector('[data-copy]').onclick = async () => { try { await navigator.clipboard.writeText(u); this.toast('Link copied'); } catch { prompt('Copy:', u); } };
    $$('[data-srv]', wrap).forEach(b => b.onclick = async () => { const r = await this.api('/api/open', {file, app: b.dataset.srv}); this.toast(r.error || 'Opened on the server computer'); });
    return wrap;
  },

  /* ---------- LIBRARY ---------- */
  async loadLibrary() {
    $('#liblist').innerHTML = '<p class="mut">Loading…</p>';
    const r = await this.api('/api/torbox/library');
    if (r.error) return $('#liblist').innerHTML = `<p class="bad">${esc(r.error)}</p>`;
    this.lib = r.items; this.libSel = new Set(); this.renderLibrary();
  },
  renderLibrary() {
    const q = $('#libq').value.toLowerCase();
    const items = (this.lib || []).filter(i => (i.name || '').toLowerCase().includes(q));
    $('#liblist').innerHTML = items.map(i => `<label class="item"><input type="checkbox" data-id="${i.id}" ${this.libSel.has(i.id) ? 'checked' : ''} ${i.ready ? '' : 'disabled'}>
      <div class="body"><div class="t">${esc(i.name)}</div><div class="badges"><span class="b">${gb(i.size)}</span><span class="b">${i.videos} video file${i.videos === 1 ? '' : 's'}</span>
      ${i.ready ? '<span class="b ok">ready</span>' : `<span class="b">${esc(i.state || '')} ${Math.round((i.progress || 0) * 100)}%</span>`}</div>
      ${i.ready ? '' : `<div class="pbar"><i style="width:${Math.round((i.progress || 0) * 100)}%"></i></div>`}</div></label>`).join('') || '<p class="mut">Nothing here.</p>';
    $$('#liblist input').forEach(c => c.onchange = () => { c.checked ? this.libSel.add(+c.dataset.id) : this.libSel.delete(+c.dataset.id); });
  },
  async buildFromLibrary() {
    const items = (this.lib || []).filter(i => this.libSel.has(i.id)).map(i => ({title: i.name, torrent_id: i.id, all_files: true}));
    if (!items.length) return this.toast('Select at least one');
    const {job} = await this.api('/api/send', {items, name: $('#libname').value || 'torbox', append: false});
    this.poll(job, $('#libjob'));
  },

  /* ---------- PLAYLISTS ---------- */
  async loadPlaylists() {
    $('#pledit').hidden = true;
    const r = await this.api('/api/playlists');
    $('#pllist').innerHTML = r.playlists.map(p => `<div class="item"><div class="body"><div class="t">${esc(p.name)}</div>
      <div class="src">${p.count} item${p.count === 1 ? '' : 's'} · ${new Date(p.mtime * 1000).toLocaleString()}</div></div>
      <button class="btn sm" data-open="${esc(p.file)}">Open</button></div>`).join('') || '<p class="mut">No playlists yet.</p>';
    $$('#pllist [data-open]').forEach(b => b.onclick = () => this.editPlaylist(b.dataset.open));
  },
  async editPlaylist(file) {
    const r = await this.api('/api/playlist?file=' + encodeURIComponent(file));
    if (r.error) return this.toast(r.error);
    let entries = r.entries; const old = file.replace(/\.m3u8$/, ''); const box = $('#pledit'); box.hidden = false;
    const draw = () => {
      box.innerHTML = `<div class="row"><input id="pe-name" class="grow" value="${esc(old)}"><button class="btn primary" id="pe-save">Save</button>
        <button class="btn danger" id="pe-del">Delete</button></div><div id="pe-actions"></div>` +
        entries.map((e, i) => `<div class="pl-entry"><span>${esc(e.title)}</span>
          <button class="btn sm" data-up="${i}" ${i ? '' : 'disabled'}>↑</button><button class="btn sm" data-dn="${i}" ${i < entries.length - 1 ? '' : 'disabled'}>↓</button>
          <button class="btn sm danger" data-rm="${i}">✕</button></div>`).join('');
      $('#pe-actions').replaceWith(Object.assign(this.playerButtons(file), {id: 'pe-actions'}));
      $$('[data-up]', box).forEach(b => b.onclick = () => { const i = +b.dataset.up; [entries[i - 1], entries[i]] = [entries[i], entries[i - 1]]; draw(); });
      $$('[data-dn]', box).forEach(b => b.onclick = () => { const i = +b.dataset.dn; [entries[i + 1], entries[i]] = [entries[i], entries[i + 1]]; draw(); });
      $$('[data-rm]', box).forEach(b => b.onclick = () => { entries.splice(+b.dataset.rm, 1); draw(); });
      $('#pe-save').onclick = async () => { const n = $('#pe-name').value || old; await this.api('/api/playlist/save', {name: n, old, entries}); this.toast('Saved'); this.loadPlaylists(); };
      $('#pe-del').onclick = async () => { if (confirm('Delete this playlist?')) { await this.api('/api/playlist/delete', {file}); this.loadPlaylists(); } };
    };
    draw(); box.scrollIntoView({behavior: 'smooth'});
  },

  /* ---------- SETTINGS ---------- */
  async loadSettings() {
    const c = await this.api('/api/config');
    $('#s-manifest').value = c.manifest_url || '';
    $('#s-key').placeholder = c.has_key ? '•••••• saved (leave blank to keep)' : 'paste your TorBox API key';
    $('#s-pin').placeholder = c.has_pin ? '•••• saved (leave blank to keep)' : 'optional, required for network access';
    $('#s-here').textContent = `You are on ${PLAT} at ${location.origin}`;
  },
  async saveSettings() {
    const pin = $('#s-pin').value;
    const r = await this.api('/api/config', {manifest_url: $('#s-manifest').value, torbox_api_key: $('#s-key').value, pin});
    if (pin) {  // a new PIN would lock this very browser out: log in with it straight away
      const l = await fetch('/api/login', {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify({pin})}).then(x => x.json());
      this.token = l.token || '';
    }
    $('#s-key').value = ''; $('#s-pin').value = '';
    $('#s-msg').textContent = r.ok ? 'Saved ✓' : (r.error || 'Failed'); this.loadSettings();
  },
};
App.init();
