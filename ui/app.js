'use strict';
const invoke = (c, a) => window.__TAURI__.core.invoke(c, a);
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
      return {error: 'unknown call ' + p};
    } catch (e) { return {error: String(e)}; }
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
    if (v === 'addons') this.loadAddons();
    if (v === 'playlists') this.loadPlaylists();
    if (v === 'settings') this.loadSettings();
    window.scrollTo(0, 0);
  },

  async init() {
    $('#app').hidden = false;
    this.bind();
    invoke('addons_seed').catch(() => {});
    const c = await invoke('config_get').catch(() => ({}));
    const h = location.hash.slice(1);
    this.show(['find', 'results', 'addons', 'library', 'playlists', 'settings'].includes(h) ? h : (c.has_key ? 'find' : 'settings'));
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
    $('#ad-add').onclick = () => this.addAddon();
    $('#ad-url').onkeydown = e => { if (e.key === 'Enter') this.addAddon(); };
    $('#ad-refresh').onclick = async () => { for (const a of this.addons || []) await invoke('addons_refresh', {id: a.id}).catch(() => {}); this.loadAddons(); };
    $('#ad-export').onclick = () => this.exportAddons();
    $('#ad-import').onclick = () => this.importAddons();
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
    for (const [key, field] of [['addon', 'addon_name'], ['codec', 'codec'], ['hdr', 'hdr'], ['audio', 'audio'], ['quality', 'quality']]) {
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
        ${s.addon_name ? `<span class="b">${esc(s.addon_name)}</span>` : ''}
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
  playerButtons(file) {
    const wrap = document.createElement('div'); wrap.className = 'pl-actions';
    const desktop = PLAT === 'mac' || PLAT === 'windows' || PLAT === 'other';
    let h = '<button class="btn" data-export>Export to Downloads</button><button class="btn" data-copy>Copy playlist</button>';
    if (PLAT === 'mac') h += '<button class="btn primary" data-play="IINA">Open in IINA</button><button class="btn" data-play="VLC">VLC</button><button class="btn" data-play="mpv">mpv</button>';
    else if (desktop) h += '<button class="btn primary" data-play="">Open in default player</button>';
    wrap.innerHTML = h;
    wrap.querySelector('[data-copy]').onclick = async () => { try { await navigator.clipboard.writeText(await invoke('playlist_text', {file})); this.toast('Playlist copied'); } catch (e) { this.toast(String(e)); } };
    wrap.querySelector('[data-export]').onclick = async () => { try { this.toast('Saved: ' + await invoke('playlist_export', {file}), 4000); } catch (e) { this.toast(String(e), 4000); } };
    $$('[data-play]', wrap).forEach(b => b.onclick = async () => { try { await invoke('open_playlist', {file, player: b.dataset.play || null}); } catch (e) { this.toast(String(e), 4000); } });
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

  /* ---------- ADDONS ---------- */
  async loadAddons() {
    this.addons = await invoke('addons_list');
    const L = this.addons;
    $('#ad-list').innerHTML = L.map((a, i) => `<div class="item" data-id="${esc(a.id)}">
      ${a.logo ? `<img src="${esc(a.logo)}" alt="" width="40" height="40" style="border-radius:8px;object-fit:cover">` : ''}
      <div class="body"><div class="t">${esc(a.name)} <span class="mut">v${esc(a.version)}</span></div>
       <div class="src">${esc(a.description).slice(0, 140)}</div>
       <div class="badges">
        <span class="b ${a.last_ok === false ? '' : a.last_ok ? 'ok' : ''}">${a.last_ok === false ? '✕ ' + esc(a.last_error || 'error') : a.last_ok ? '● ok' + (a.latency_ms != null ? ' ' + a.latency_ms + 'ms' : '') : 'untested'}</span>
        ${a.resources.map(r => `<span class="b">${esc(r)}</span>`).join('')}${a.types.map(t => `<span class="b">${esc(t)}</span>`).join('')}
        <span class="b">${esc(a.url_masked)}</span><span class="b">timeout ${a.timeout_secs}s</span></div>
       <div class="pl-actions">
        <button class="btn sm" data-a="toggle">${a.enabled ? 'Disable' : 'Enable'}</button>
        <button class="btn sm" data-a="rename">Rename</button><button class="btn sm" data-a="timeout">Timeout</button>
        <button class="btn sm" data-a="refresh">Test</button>
        ${a.configurable ? '<button class="btn sm" data-a="configure">Configure</button>' : ''}
        <button class="btn sm" data-a="copy">Copy URL</button>
        <button class="btn sm" data-a="up" ${i ? '' : 'disabled'}>↑</button><button class="btn sm" data-a="down" ${i < L.length - 1 ? '' : 'disabled'}>↓</button>
        <button class="btn sm danger" data-a="remove">Remove</button></div></div></div>`).join('') || '<p class="mut">No addons yet. Paste an addon URL above.</p>';
    $$('#ad-list [data-a]').forEach(b => b.onclick = () => this.addonAction(b.closest('.item').dataset.id, b.dataset.a));
  },
  async addAddon() {
    const url = $('#ad-url').value.trim(); if (!url) return;
    $('#ad-msg').textContent = 'Adding…';
    try { await invoke('addons_add', {url}); $('#ad-url').value = ''; $('#ad-msg').textContent = ''; this.loadAddons(); }
    catch (e) { $('#ad-msg').innerHTML = `<span class="bad">${esc(String(e))}</span>`; }
  },
  async addonAction(id, act) {
    const a = this.addons.find(x => x.id === id);
    try {
      if (act === 'toggle') await invoke('addons_update', {id, enabled: !a.enabled});
      else if (act === 'rename') { const n = prompt('Name', a.name); if (n) await invoke('addons_update', {id, name: n}); }
      else if (act === 'timeout') { const t = prompt('Timeout (seconds, 3-120)', a.timeout_secs); if (t) await invoke('addons_update', {id, timeoutSecs: +t}); }
      else if (act === 'refresh') await invoke('addons_refresh', {id});
      else if (act === 'configure') await invoke('addon_configure', {id});
      else if (act === 'copy') { await navigator.clipboard.writeText(await invoke('addon_url', {id})); this.toast('Addon URL copied (keep it private)'); }
      else if (act === 'remove') { if (!confirm('Remove ' + a.name + '?')) return; await invoke('addons_remove', {id}); }
      else if (act === 'up' || act === 'down') {
        const ids = this.addons.map(x => x.id), i = ids.indexOf(id), j = act === 'up' ? i - 1 : i + 1;
        [ids[i], ids[j]] = [ids[j], ids[i]]; await invoke('addons_reorder', {ids});
      }
    } catch (e) { this.toast(String(e), 4000); }
    this.loadAddons();
  },
  async exportAddons() {
    if (!confirm('The export contains full addon URLs, which may include private settings. Copy to clipboard?')) return;
    const l = await invoke('addons_export');
    try { await navigator.clipboard.writeText(l.map(x => x.url).join('\n')); this.toast('Copied ' + l.length + ' URLs'); } catch (e) { this.toast(String(e)); }
  },
  async importAddons() {
    const t = prompt('Paste addon URLs, one per line'); if (!t) return;
    const r = await invoke('addons_import', {urls: t.split(/\s+/)});
    this.toast(`Added ${r.added}` + (r.errors.length ? `, ${r.errors.length} failed: ${r.errors[0]}` : ''), 5000); this.loadAddons();
  },

  /* ---------- SETTINGS ---------- */
  async loadSettings() {
    const c = await invoke('config_get');
    $('#s-key').placeholder = c.has_key ? '•••••• saved (leave blank to keep)' : 'paste your TorBox API key';
  },
  async saveSettings() {
    const r = await this.api('/api/config', {torbox_key: $('#s-key').value});
    $('#s-key').value = ''; $('#s-msg').textContent = r.error || 'Saved ✓'; this.loadSettings();
  },
};
App.init();
