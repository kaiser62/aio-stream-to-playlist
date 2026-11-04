/* Dev-only: loaded on every start but inert unless the URL has ?mock=1.
   Provides a fake window.__TAURI__.core.invoke with realistic sample data so the UI
   can be viewed in a plain browser. Add &fail=1 to make player_open reject. */
(() => {
  const qs = new URLSearchParams(location.search);
  if (!qs.get('mock') || (window.__TAURI__ && window.__TAURI__.core)) return;
  const GB = 1073741824, wait = ms => new Promise(r => setTimeout(r, ms));
  const poster = (a, b) => 'data:image/svg+xml;utf8,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="200" height="300"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient></defs><rect width="200" height="300" fill="url(#g)"/><circle cx="140" cy="90" r="46" fill="#fff" opacity=".16"/></svg>`);
  const addons = [
    {id: 'a1', name: 'AIOStreams', version: '2.14.0', description: 'One addon to rule them all: aggregates, filters and sorts streams from many sources.', logo: '', enabled: true, last_ok: true, latency_ms: 412, resources: ['stream'], types: ['movie', 'series'], url_masked: 'https://aio.example.app/stremio/••••••••/manifest.json', timeout_secs: 30, configurable: true},
    {id: 'a2', name: 'Torrentio', version: '0.0.15', description: 'Provides torrent streams from scraped torrent providers.', logo: '', enabled: true, last_ok: true, latency_ms: 880, resources: ['stream'], types: ['movie', 'series', 'anime'], url_masked: 'https://torrentio.strem.fun/•••/manifest.json', timeout_secs: 20, configurable: true},
    {id: 'a3', name: 'Comet', version: '1.8.2', description: 'Stremio addon for debrid services with fast cached lookups.', logo: '', enabled: true, last_ok: false, last_error: 'timeout after 20s', resources: ['stream'], types: ['movie', 'series'], url_masked: 'https://comet.example.org/••••/manifest.json', timeout_secs: 20, configurable: false},
    {id: 'a4', name: 'Cinemeta Mirror', version: '3.0.0', description: 'Metadata only. Disabled until needed.', logo: '', enabled: false, last_ok: null, resources: ['catalog', 'meta'], types: ['movie'], url_masked: 'https://v3-cinemeta.strem.io/manifest.json', timeout_secs: 10, configurable: false},
  ];
  const mk = (i, t, o) => Object.assign({title: t, source: 'BluRay.REMUX · ' + o.addon_name, magnet: 'magnet:?xt=urn:btih:' + i, res_rank: 1080, resolution: '1080p', quality: 'WEB-DL', codec: 'x265', hdr: '', audio: 'DDP5.1', size: 6 * GB, seeders: 120, langs: ['EN'], cached: true, bad: false}, o);
  const streams = [
    mk(1, 'Inception.2010.2160p.UHD.BluRay.REMUX.HDR10.HEVC.TrueHD.Atmos-FraMeSToR', {resolution: '4K', res_rank: 2160, quality: 'REMUX', hdr: 'HDR10', audio: 'Atmos', size: 58.4 * GB, seeders: 214, addon_name: 'AIOStreams'}),
    mk(2, 'Inception 2010 2160p WEB-DL DV HDR10+ DDP5.1 Atmos H.265', {resolution: '4K', res_rank: 2160, hdr: 'DV', size: 24.1 * GB, seeders: 88, addon_name: 'Torrentio', cached: false}),
    mk(3, 'Inception.2010.1080p.BluRay.x265.10bit.DTS-HD.MA.5.1-RARBG', {quality: 'BluRay', audio: 'DTS-HD', size: 9.8 * GB, seeders: 530, addon_name: 'AIOStreams', langs: ['EN', 'FR']}),
    mk(4, 'Inception (2010) 1080p BrRip x264 YIFY', {codec: 'x264', quality: 'BrRip', audio: 'AAC', size: 1.9 * GB, seeders: 1021, addon_name: 'Torrentio'}),
    mk(5, 'Inception.2010.1080p.WEB-DL.DDP5.1.H.264-NTb', {codec: 'x264', size: 7.2 * GB, seeders: 64, addon_name: 'Comet'}),
    mk(6, 'Inception.2010.720p.BluRay.x264-SiNNERS', {resolution: '720p', res_rank: 720, codec: 'x264', quality: 'BluRay', size: 4.4 * GB, seeders: 77, addon_name: 'Torrentio'}),
    mk(7, 'Inception.2010.HDCAM.XviD-AAC', {resolution: '480p', res_rank: 480, quality: 'CAM', codec: 'XviD', size: 0.7 * GB, seeders: 12, addon_name: 'Comet', cached: false}),
  ];
  const series = [];
  for (let e = 1; e <= 3; e++) {
    series.push(mk(100 + e, `Severance.S02E0${e}.2160p.ATVP.WEB-DL.DDP5.1.Atmos.DV.H.265`, {season: 2, episode: e, resolution: '4K', res_rank: 2160, hdr: 'DV', size: (11 + e) * GB, seeders: 140 - e * 20, addon_name: 'AIOStreams'}));
    series.push(mk(110 + e, `Severance.S02E0${e}.1080p.WEB-DL.DDP5.1.H.264-NTb`, {season: 2, episode: e, codec: 'x264', size: (3.1 + e / 2) * GB, seeders: 90, addon_name: 'Torrentio'}));
  }
  let playlists = [
    {name: 'Inception', file: 'Inception.m3u8', count: 1, mtime: Date.now() / 1000 - 3600},
    {name: 'Severance S02', file: 'Severance S02.m3u8', count: 3, mtime: Date.now() / 1000 - 86400 * 2},
    {name: 'torbox', file: 'torbox.m3u8', count: 5, mtime: Date.now() / 1000 - 86400 * 9},
  ];
  const entries = {'Inception.m3u8': [{title: 'Inception 2010 2160p REMUX'}], 'Severance S02.m3u8': [1, 2, 3].map(e => ({title: `Severance S02E0${e} 2160p WEB-DL DV`})), 'torbox.m3u8': ['Dune.Part.Two.2024.2160p', 'Arrival.2016.1080p.BluRay', 'Blade.Runner.2049.2017.2160p.UHD', 'Sicario.2015.1080p', 'Heat.1995.REMUX'].map(title => ({title}))};
  let jobTick = 0;
  const h = {
    config_get: () => ({has_key: qs.get('nokey') ? false : true}),
    config_set: () => null, addons_seed: () => null,
    search: ({kind, query}) => kind === 'series'
      ? [{id: 'tt11280740', name: 'Severance', year: '2022', poster: poster('#3b5a63', '#101c20')}]
      : [{id: 'tt1375666', name: 'Inception', year: '2010', poster: poster('#4a5d80', '#0f1522')}, {id: 'tt5295894', name: 'Inception: The Cobol Job', year: '2010'}, {id: 'tt0111161', name: 'The Shawshank Redemption', year: '1994', poster: poster('#8a6a3a', '#20160a')}, {id: 'tt0468569', name: 'The Dark Knight', year: '2008'}],
    fetch_streams: async ({kind}) => { await wait(900); return {streams: kind === 'series' ? series : streams, errors: kind === 'series' ? ['S2E4: no streams'] : []}; },
    send: () => { jobTick = 0; return {job: 'j1'}; },
    job_get: () => { jobTick++; const st = ['queued', 'adding', 'waiting', 'done', 'error'], rows = ['Inception 2010 2160p REMUX', 'Severance S02E01 2160p', 'Dune Part Two 1080p'].map((title, i) => {
      const s = jobTick > 4 ? (i === 2 ? 'error' : 'done') : jobTick > 2 ? (i === 0 ? 'done' : 'waiting') : jobTick > 1 ? 'adding' : 'queued';
      return {title, state: s, detail: s === 'waiting' ? 'downloading 62%' : s === 'done' ? 'added to TorBox' : s === 'error' ? 'torrent not cached' : ''}; });
      return {rows, finished: jobTick > 4, playlist: jobTick > 4 ? 'Inception.m3u8' : null}; },
    library: () => ({items: [
      {id: 1, name: 'Dune.Part.Two.2024.2160p.UHD.BluRay.REMUX', size: 62 * GB, videos: 1, ready: true},
      {id: 2, name: 'Severance.S02.1080p.ATVP.WEB-DL', size: 38 * GB, videos: 10, ready: true},
      {id: 3, name: 'Blade.Runner.2049.2017.2160p', size: 51 * GB, videos: 1, ready: false, state: 'downloading', progress: 0.42},
      {id: 4, name: 'Arrival.2016.1080p.BluRay.x265', size: 8 * GB, videos: 2, ready: true}]}),
    playlists_list: () => playlists, playlist_read: ({file}) => entries[file] || [], playlist_text: () => '#EXTM3U', playlist_export: () => '~/Downloads/x.m3u8',
    playlist_save: ({name}) => name + '.m3u8', playlist_delete: ({file}) => { playlists = playlists.filter(p => p.file !== file); },
    open_playlist: () => null,
    player_open: () => qs.get('fail') ? Promise.reject('Built-in player is not available on this build yet') : null,
    player_cmd: () => null, player_close: () => null, player_play: () => null,
    player_props: () => ({'media-title': 'Severance S02E04', 'time-pos': 1130, duration: 3412, pause: false}),
    player_recent: () => [{title: 'Severance S02E04 · Woe\'s Hollow', url: 'a', pos: 1130, dur: 3412, at: 1}, {title: 'Dune.Part.Two.2024.2160p.REMUX', url: 'b', pos: 0, dur: 9960, at: 1}, {title: 'Arrival.2016.1080p', url: 'c', pos: 5400, dur: 6960, at: 1}],
    logs_get: () => [], logs_clear: () => null, log_push: () => null, window_fullscreen: ({on}) => on,
    title_meta: ({name, series, season, episode}) => ({id: 'tt0', name, year: '2022', rating: '8.7', genres: ['Drama', 'Thriller'], poster: poster('#3b5a63', '#101c20'), background: '', description: 'Mock summary.', ep_title: series ? 'Woe\'s Hollow' : '', ep_thumb: '', ep_overview: ''}),
    player_recent_remove: () => null,
    player_keys: () => [['SPACE', 'Play / pause'], ['← / →', 'Seek 5 s'], ['⇧← / ⇧→', 'Seek 60 s'], ['↑ / ↓', 'Volume'], ['M', 'Mute'], ['[ / ]', 'Speed'], ['F', 'Fullscreen'], ['< / >', 'Previous / next']].map(([key, desc]) => ({key, desc})),
    addons_list: () => addons.map(a => ({...a})),
    addons_add: ({url}) => { if (!/^https?:|^stremio:/.test(url)) throw 'Not a valid addon URL'; return addons[0]; },
    addons_update: ({id, enabled, name, timeoutSecs}) => { const a = addons.find(x => x.id === id); if (enabled !== undefined && enabled !== null) a.enabled = enabled; if (name) a.name = name; if (timeoutSecs) a.timeout_secs = timeoutSecs; },
    addons_remove: ({id}) => { addons.splice(addons.findIndex(x => x.id === id), 1); },
    addons_reorder: ({ids}) => { addons.sort((a, b) => ids.indexOf(a.id) - ids.indexOf(b.id)); },
    addons_refresh: ({id}) => { const a = addons.find(x => x.id === id); a.last_ok = true; a.latency_ms = 300 + Math.round(Math.random() * 500); },
    addon_url: () => 'https://example/manifest.json', addon_configure: () => null,
    addons_export: () => addons.map(a => ({url: a.url_masked})), addons_import: ({urls}) => ({added: urls.length, errors: []}),
  };
  window.__TAURI__ = {core: {invoke: async (c, a) => { await wait(80); if (!(c in h)) throw 'unknown command ' + c; return h[c](a || {}); }}};
})();
