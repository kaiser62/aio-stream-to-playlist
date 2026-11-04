#!/usr/bin/env python3
"""AIOStreams -> filter/correct -> TorBox -> .m3u8 playlist (IINA / mpv / VLC).

Run:  python3 app.py     then open http://127.0.0.1:8765
Stdlib only. Config lives in config.json (created from the Settings box in the UI).
"""
import json, os, re, sys, time, uuid, threading, hashlib, hmac, mimetypes
import urllib.request, urllib.parse, urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
FROZEN = bool(getattr(sys, "frozen", False))  # PyInstaller build
BUNDLE = getattr(sys, "_MEIPASS", HERE)
DATA_DIR = os.environ.get("DATA_DIR") or (os.path.expanduser("~/.aio-stream-to-playlist") if FROZEN else HERE)
os.makedirs(DATA_DIR, exist_ok=True)
try:  # frozen builds ship no system CA bundle; certifi fixes CERTIFICATE_VERIFY_FAILED
    import ssl, certifi
    SSL_CTX = ssl.create_default_context(cafile=certifi.where())
except Exception:
    SSL_CTX = None
DEBUG = os.environ.get("DEBUG", "").lower() in ("1", "true", "yes", "on")


def redact(text):
    """Hide secrets (TorBox token, PIN token, keys) from log lines."""
    return re.sub(r"(?i)\b(token|k|key|pin|api_key)=[^&\s\"']+", r"\1=***", str(text))


def dbg(*a):
    if DEBUG:
        print(time.strftime("%H:%M:%S"), "[debug]", redact(" ".join(str(x) for x in a)), flush=True)

CONFIG_PATH = os.path.join(DATA_DIR, "config.json")
PLAYLIST_DIR = os.path.join(DATA_DIR, "playlists")
TORBOX = os.environ.get("TORBOX_API", "https://api.torbox.app/v1/api")
CINEMETA = os.environ.get("CINEMETA_API", "https://v3-cinemeta.strem.io")
VIDEO_EXT = (".mkv", ".mp4", ".avi", ".m4v", ".mov", ".ts", ".webm", ".wmv")
STATIC_DIR = os.path.join(BUNDLE, "static")
os.makedirs(PLAYLIST_DIR, exist_ok=True)


# ---------------------------------------------------------------- config
def load_config():
    try:
        with open(CONFIG_PATH) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def save_config(cfg):
    cur = load_config()
    if cfg.get("clear_pin"):
        cur.pop("pin", None)
    for k in ("manifest_url", "torbox_api_key", "pin"):
        v = (cfg.get(k) or "").strip()
        if v:  # empty = keep existing (so the key is never echoed back to the UI)
            cur[k] = v
    with open(CONFIG_PATH, "w") as f:
        json.dump(cur, f, indent=2)
    os.chmod(CONFIG_PATH, 0o600)
    return cur


# ---------------------------------------------------------------- http helpers
def http(url, method="GET", headers=None, data=None, timeout=45):
    req = urllib.request.Request(url, data=data, method=method, headers=headers or {})
    req.add_header("User-Agent", "aio-stream-to-playlist/1.0")
    t0 = time.time()
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=SSL_CTX) as r:
            raw = r.read()
            dbg(f"upstream {method} {url} -> {r.status} {len(raw)}B {int((time.time() - t0) * 1000)}ms")
            return json.loads(raw.decode("utf-8", "replace") or "{}")
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", "replace")[:300]
        dbg(f"upstream {method} {url} -> HTTP {e.code} {body}")
        raise RuntimeError(f"HTTP {e.code} from {urllib.parse.urlparse(url).netloc}: {body}")
    except urllib.error.URLError as e:
        dbg(f"upstream {method} {url} -> network error {e.reason}")
        raise RuntimeError(f"Network error: {e.reason}")


def multipart(fields):
    boundary = uuid.uuid4().hex
    body = b""
    for k, v in fields.items():
        body += (f'--{boundary}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n').encode()
    body += f"--{boundary}--\r\n".encode()
    return body, f"multipart/form-data; boundary={boundary}"


# ---------------------------------------------------------------- Cinemeta search
def search_titles(query, kind):
    q = urllib.parse.quote(query)
    j = http(f"{CINEMETA}/catalog/{kind}/top/search={q}.json")
    return [{"id": m.get("imdb_id") or m.get("id"), "name": m.get("name"),
             "year": m.get("releaseInfo") or m.get("year"), "poster": m.get("poster")}
            for m in j.get("metas", [])[:12]]


# ---------------------------------------------------------------- AIOStreams parsing
RES_RE = re.compile(r"(2160p|4k|uhd|1440p|1080p|720p|576p|480p|360p)", re.I)
RES_RANK = {"2160p": 2160, "4k": 2160, "uhd": 2160, "1440p": 1440, "1080p": 1080,
            "720p": 720, "576p": 576, "480p": 480, "360p": 360}
SIZE_RE = re.compile(r"(\d+(?:[.,]\d+)?)\s?(GB|GiB|MB|MiB)", re.I)
SEED_RE = re.compile(r"(?:👤|seeders?:?)\s*(\d+)", re.I)
BAD_RE = re.compile(r"\b(cam|camrip|hdcam|ts|telesync|tc|hdts|scr|screener|r5)\b", re.I)
EP_RE = re.compile(r"[Ss](\d{1,2})[ ._-]?[Ee](\d{1,3})")
EP_ALT = re.compile(r"\b(\d{1,2})x(\d{2,3})\b")


CODEC = [("AV1", r"\bav1\b"), ("HEVC", r"\b(hevc|x265|h\.?265)\b"), ("AVC", r"\b(avc|x264|h\.?264)\b")]
HDR = [("DV", r"\b(dv|dolby[ .-]?vision)\b"), ("HDR10+", r"hdr10\+"), ("HDR10", r"hdr10\b"), ("HDR", r"\bhdr\b")]
AUDIO = [("Atmos", r"atmos"), ("TrueHD", r"truehd"), ("DTS-HD", r"dts[ .-]?hd"), ("DTS", r"\bdts\b"),
         ("DD+", r"(ddp|dd\+|e-?ac-?3)"), ("DD", r"\b(dd|ac-?3)\b"), ("AAC", r"\baac\b")]
QUALITY = [("REMUX", r"remux"), ("BluRay", r"blu-?ray|bdrip|brrip"), ("WEB-DL", r"web[ .-]?dl"),
           ("WEBRip", r"web[ .-]?rip"), ("HDTV", r"hdtv")]


LANG_RE = re.compile(r"\b(multi|dual|english|french|german|spanish|italian|japanese|korean|hindi|russian|portuguese)\b", re.I)


def _first(table, text):
    for label, rx in table:
        if re.search(rx, text, re.I):
            return label
    return ""


def clean(text):
    return re.sub(r"[\U0001F000-\U0001FFFF☀-➿️]+", " ", text or "")


def parse_stream(s, kind_hint=None):
    """Normalise one Stremio stream object from AIOStreams."""
    hints = s.get("behaviorHints") or {}
    name = s.get("name") or ""
    desc = s.get("description") or s.get("title") or ""
    blob = f"{name}\n{desc}\n{hints.get('filename', '')}"
    filename = hints.get("filename") or ""
    if not filename:
        for line in desc.splitlines():
            c = clean(line).strip()
            if re.search(r"\.(mkv|mp4|avi|m4v)$", c, re.I) or re.search(r"[Ss]\d+[Ee]\d+|\b(19|20)\d\d\b", c):
                filename = c
                break
    res = RES_RE.search(blob)
    resolution = res.group(1).lower() if res else ""
    if resolution in ("4k", "uhd"):
        resolution = "2160p"
    size = hints.get("videoSize")
    if not size:
        m = SIZE_RE.search(blob)
        if m:
            n = float(m.group(1).replace(",", "."))
            size = int(n * (1024 ** 3 if m.group(2).lower().startswith("g") else 1024 ** 2))
    seeders = None
    m = SEED_RE.search(blob)
    if m:
        seeders = int(m.group(1))
    cached = bool(re.search(r"\[[A-Z]{2,4}\+\]|⚡", name)) or (s.get("url") and not s.get("infoHash"))
    ep = None
    for r in (EP_RE, EP_ALT):
        m = r.search(filename or blob)
        if m:
            ep = (int(m.group(1)), int(m.group(2)))
            break
    info_hash = (s.get("infoHash") or "").lower() or None
    magnet = None
    if info_hash:
        trackers = [x[8:] for x in (s.get("sources") or []) if isinstance(x, str) and x.startswith("tracker:")]
        magnet = f"magnet:?xt=urn:btih:{info_hash}&dn={urllib.parse.quote(filename or 'stream')}"
        for t in trackers[:12]:
            magnet += "&tr=" + urllib.parse.quote(t, safe="")
    return {
        "title": filename or clean(name).strip() or "unknown",
        "source": clean(name).strip(),
        "resolution": resolution,
        "res_rank": RES_RANK.get(resolution, 0),
        "size": size or 0,
        "seeders": seeders,
        "cached": bool(cached),
        "bad": bool(BAD_RE.search(blob)),
        "codec": _first(CODEC, blob),
        "hdr": _first(HDR, blob),
        "audio": _first(AUDIO, blob),
        "quality": _first(QUALITY, blob),
        "langs": sorted({x.lower() for x in LANG_RE.findall(blob)}),
        "season": ep[0] if ep else None,
        "episode": ep[1] if ep else None,
        "infoHash": info_hash,
        "magnet": magnet,
        "url": s.get("url"),
        "file_idx": s.get("fileIdx"),
    }


def correct_and_dedupe(parsed):
    """Drop unusable rows, dedupe on infoHash / url, keep the best (cached, most seeders)."""
    seen, out = {}, []
    for p in parsed:
        key = p["infoHash"] or p["url"]
        if not key:
            continue
        prev = seen.get(key)
        score = (p["cached"], p["seeders"] or 0, p["size"])
        if prev is None or score > prev[0]:
            seen[key] = (score, p)
    out = [v[1] for v in seen.values()]
    return out


def base_from_manifest(manifest_url):
    u = manifest_url.strip()
    u = re.sub(r"^stremio://", "https://", u)
    return re.sub(r"/manifest\.json.*$", "", u)


def fetch_streams(cfg, kind, imdb, season=None, episode=None):
    if not cfg.get("manifest_url"):
        raise RuntimeError("Set your AIOStreams manifest URL in Settings first.")
    sid = imdb if kind == "movie" else f"{imdb}:{season}:{episode}"
    url = f"{base_from_manifest(cfg['manifest_url'])}/stream/{kind}/{urllib.parse.quote(sid, safe=':')}.json"
    j = http(url, timeout=90)
    return correct_and_dedupe([parse_stream(s) for s in j.get("streams", [])])


# ---------------------------------------------------------------- TorBox
def tb(cfg, path, method="GET", fields=None, query=None):
    key = cfg.get("torbox_api_key")
    if not key:
        raise RuntimeError("Set your TorBox API key in Settings first.")
    url = f"{TORBOX}{path}"
    if query:
        url += "?" + urllib.parse.urlencode(query)
    headers = {"Authorization": f"Bearer {key}"}
    data = None
    if fields is not None:
        data, headers["Content-Type"] = multipart(fields)
    j = http(url, method=method, headers=headers, data=data)
    if j.get("success") is False:
        raise RuntimeError(f"TorBox: {j.get('detail') or j.get('error')}")
    return j.get("data")


def torbox_add(cfg, magnet):
    d = tb(cfg, "/torrents/createtorrent", "POST", {"magnet": magnet, "seed": "1", "allow_zip": "false"})
    return d.get("torrent_id") or d.get("queued_id")


def torbox_info(cfg, torrent_id):
    return tb(cfg, "/torrents/mylist", query={"id": torrent_id, "bypass_cache": "true"})


def pick_file(files, season=None, episode=None):
    vids = [f for f in files if f.get("name", "").lower().endswith(VIDEO_EXT)]
    if not vids:
        return None
    if season is not None and episode is not None:
        for f in vids:
            nm = f["name"]
            for r in (EP_RE, EP_ALT):
                m = r.search(nm)
                if m and (int(m.group(1)), int(m.group(2))) == (season, episode):
                    return f
    return max(vids, key=lambda f: f.get("size", 0))


def torbox_link(cfg, torrent_id, file_id):
    return tb(cfg, "/torrents/requestdl", query={
        "token": cfg["torbox_api_key"], "torrent_id": torrent_id, "file_id": file_id, "redirect": "false"})


# ---------------------------------------------------------------- playlist
def build_m3u8(entries):
    lines = ["#EXTM3U"]
    for e in entries:
        lines.append(f"#EXTINF:-1,{e['title'].replace(chr(10), ' ')}")
        lines.append(e["url"])
    return "\n".join(lines) + "\n"


def safe_name(n):
    n = re.sub(r"[^A-Za-z0-9._ -]+", "", n or "playlist").strip().replace(" ", "_")
    return (n or "playlist")[:80]





# ---------------------------------------------------------------- playlist files
def pl_path(fn):
    fn = os.path.basename(fn)
    if not fn.endswith(".m3u8"):
        fn += ".m3u8"
    return os.path.join(PLAYLIST_DIR, fn)


def read_playlist(fn):
    out, title = [], None
    with open(pl_path(fn), encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line.startswith("#EXTINF:"):
                title = line.split(",", 1)[-1]
            elif line and not line.startswith("#"):
                out.append({"title": title or line.rsplit("/", 1)[-1], "url": line})
                title = None
    return out


def write_playlist(fn, entries):
    with open(pl_path(fn), "w", encoding="utf-8") as f:
        f.write(build_m3u8(entries))


def list_playlists():
    rows = []
    for fn in sorted(os.listdir(PLAYLIST_DIR)):
        if fn.endswith(".m3u8"):
            p = os.path.join(PLAYLIST_DIR, fn)
            rows.append({"file": fn, "name": fn[:-5], "count": len(read_playlist(fn)),
                         "mtime": int(os.path.getmtime(p))})
    return sorted(rows, key=lambda r: -r["mtime"])


# ---------------------------------------------------------------- jobs
JOBS = {}


def resolve_item(cfg, it, row):
    """-> list of {title,url}. Handles direct URLs, magnets, and existing TorBox torrents."""
    if it.get("url") and not it.get("magnet") and not it.get("torrent_id"):
        return [{"title": it["title"], "url": it["url"]}]
    row["state"] = "adding"
    tid = it.get("torrent_id") or torbox_add(cfg, it["magnet"])
    row["state"] = "waiting"
    deadline = time.time() + int(it.get("timeout", 600))
    while True:
        info = torbox_info(cfg, tid)
        if isinstance(info, list):
            info = info[0] if info else None
        if info and (info.get("download_finished") or info.get("download_present") or info.get("cached")):
            break
        if info:
            row["detail"] = f"{info.get('download_state', '')} {round((info.get('progress') or 0) * 100)}%"
        if it.get("add_only"):
            row["detail"] = "queued on TorBox"
            return []
        if time.time() > deadline:
            raise RuntimeError("timed out waiting for TorBox")
        time.sleep(4)
    files = info.get("files") or []
    if it.get("all_files"):
        vids = sorted([f for f in files if f.get("name", "").lower().endswith(VIDEO_EXT)], key=lambda f: f["name"])
        if not vids:
            raise RuntimeError("no video files")
        return [{"title": f.get("short_name") or f["name"].rsplit("/", 1)[-1],
                 "url": torbox_link(load_config(), tid, f["id"])} for f in vids]
    f = pick_file(files, it.get("season"), it.get("episode"))
    if not f:
        raise RuntimeError("no video file in torrent")
    return [{"title": it["title"], "url": torbox_link(cfg, tid, f["id"])}]


def run_send_job(job_id, cfg, items, name, append=False):
    job = JOBS[job_id]
    job["rows"] = [{"title": it.get("title", "?"), "state": "queued", "detail": ""} for it in items]
    entries = read_playlist(safe_name(name)) if append and os.path.isfile(pl_path(safe_name(name))) else []
    for it, row in zip(items, job["rows"]):
        try:
            entries += resolve_item(cfg, it, row)
            if row["state"] != "error":
                row["state"] = "done"
        except Exception as e:
            row["state"], row["detail"] = "error", str(e)
            dbg(f"job {job_id} item {it.get('title')!r} failed: {e}")
    if entries:
        fn = safe_name(name) + ".m3u8"
        write_playlist(fn, entries)
        job["playlist"] = fn
    job["finished"] = True


def torbox_library(cfg):
    out = []
    for t in tb(cfg, "/torrents/mylist", query={"bypass_cache": "true"}) or []:
        vids = [f for f in t.get("files", []) if f.get("name", "").lower().endswith(VIDEO_EXT)]
        out.append({"id": t["id"], "name": t.get("name"), "size": t.get("size"),
                    "progress": t.get("progress"), "state": t.get("download_state"),
                    "ready": bool(t.get("download_finished") or t.get("download_present")),
                    "videos": len(vids)})
    return out


# ---------------------------------------------------------------- auth
def token_for(pin):
    return hmac.new(b"aio-playlist", pin.encode(), hashlib.sha256).hexdigest()[:32]


# ---------------------------------------------------------------- server
class H(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        if DEBUG:
            dbg(f"{self.client_address[0]} {fmt % args}")

    def _send(self, code, body, ctype="application/json", extra=None):
        b = body if isinstance(body, bytes) else body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.send_header("Cache-Control", "no-store")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(b)

    def _json(self, obj, code=200, extra=None):
        self._send(code, json.dumps(obj), extra=extra)

    def authed(self, query):
        pin = load_config().get("pin")
        if not pin:
            return True
        want = token_for(pin)
        cookie = self.headers.get("Cookie", "")
        m = re.search(r"(?:^|;\s*)auth=([0-9a-f]+)", cookie)
        got = (m.group(1) if m else None) or (query.get("k") or [None])[0]
        return bool(got) and hmac.compare_digest(got, want)

    def _static(self, rel):
        rel = rel.lstrip("/") or "index.html"
        full = os.path.normpath(os.path.join(STATIC_DIR, rel))
        if not full.startswith(STATIC_DIR + os.sep) or not os.path.isfile(full):
            return self._send(404, "not found", "text/plain")
        ctype = mimetypes.guess_type(full)[0] or "application/octet-stream"
        if full.endswith(".webmanifest"):
            ctype = "application/manifest+json"
        with open(full, "rb") as f:
            self._send(200, f.read(), ctype)

    def do_GET(self):
        u = urllib.parse.urlparse(self.path)
        p, q = u.path, urllib.parse.parse_qs(u.query)
        if p == "/api/status":  # public: tells the UI whether to show the PIN screen
            c = load_config()
            return self._json({"pin_required": bool(c.get("pin")), "authed": self.authed(q),
                               "configured": bool(c.get("manifest_url") and c.get("torbox_api_key")),
                               "token": token_for(c["pin"]) if c.get("pin") and self.authed(q) else ""})
        if p.startswith("/api/") or p.startswith("/playlist/"):
            if not self.authed(q):
                return self._json({"error": "auth"}, 401)
        if p == "/api/config":
            c = load_config()
            return self._json({"manifest_url": c.get("manifest_url", ""), "has_key": bool(c.get("torbox_api_key")),
                               "has_pin": bool(c.get("pin"))})
        if p.startswith("/api/job/"):
            j = JOBS.get(p.rsplit("/", 1)[1])
            return self._json(j or {"rows": [], "finished": True}, 200 if j else 404)
        if p == "/api/playlists":
            return self._json({"playlists": list_playlists()})
        if p == "/api/playlist":
            try:
                return self._json({"entries": read_playlist(q["file"][0])})
            except OSError:
                return self._json({"error": "not found"}, 404)
        if p == "/api/torbox/library":
            try:
                return self._json({"items": torbox_library(load_config())})
            except Exception as e:
                return self._json({"error": str(e)})
        if p.startswith("/playlist/"):
            fn = os.path.basename(urllib.parse.unquote(p))
            path = pl_path(fn)
            if fn.endswith(".m3u8") and os.path.isfile(path):
                with open(path, "rb") as f:
                    return self._send(200, f.read(), "application/vnd.apple.mpegurl",
                                      {"Content-Disposition": f'inline; filename="{fn}"'})
            return self._send(404, "not found", "text/plain")
        self._static(p)

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        try:
            body = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            return self._json({"error": "bad json"}, 400)
        p = urllib.parse.urlparse(self.path).path
        if p == "/api/login":
            pin = load_config().get("pin")
            if pin and hmac.compare_digest(str(body.get("pin", "")), pin):
                t = token_for(pin)
                return self._json({"ok": True, "token": t},
                                  extra={"Set-Cookie": f"auth={t}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000"})
            time.sleep(1)
            return self._json({"error": "wrong PIN"}, 401)
        if not self.authed({}):
            return self._json({"error": "auth"}, 401)
        try:
            if p == "/api/config":
                save_config(body)
                return self._json({"ok": True})
            if p == "/api/search":
                return self._json({"results": search_titles(body["query"], body.get("kind", "movie"))})
            if p == "/api/fetch":
                return self._json(self.fetch(body))
            if p == "/api/send":
                jid = uuid.uuid4().hex[:8]
                JOBS[jid] = {"rows": [], "finished": False, "playlist": None}
                threading.Thread(target=run_send_job, daemon=True, args=(
                    jid, load_config(), body["items"], body.get("name", "playlist"), bool(body.get("append")))).start()
                return self._json({"job": jid})
            if p == "/api/playlist/save":  # reorder / remove / rename entries
                name = safe_name(body["name"])
                write_playlist(name, [{"title": e["title"], "url": e["url"]} for e in body["entries"]])
                if body.get("old") and safe_name(body["old"]) != name:
                    try: os.remove(pl_path(safe_name(body["old"])))
                    except OSError: pass
                return self._json({"ok": True, "file": name + ".m3u8"})
            if p == "/api/playlist/delete":
                os.remove(pl_path(body["file"]))
                return self._json({"ok": True})
            if p == "/api/open":  # play on the machine running the server (macOS)
                path = pl_path(body["file"])
                app = {"IINA": "IINA", "mpv": "mpv", "VLC": "VLC"}.get(body.get("app"))
                if not os.path.isfile(path) or not app:
                    return self._json({"error": "bad request"}, 400)
                import subprocess
                subprocess.Popen(["open", "-a", app, path])
                return self._json({"ok": True})
        except Exception as e:
            if DEBUG:
                import traceback
                dbg(f"error in POST {p}:\n{traceback.format_exc()}")
            return self._json({"error": str(e)})
        self._json({"error": "not found"}, 404)

    def fetch(self, b):
        cfg = load_config()
        kind, imdb = b["kind"], b["imdb"]
        streams, errors = [], []
        if kind == "movie":
            streams = fetch_streams(cfg, "movie", imdb)
        else:
            for ep in range(b["ep1"], max(b["ep1"], b["ep2"]) + 1):
                try:
                    got = fetch_streams(cfg, "series", imdb, b["season"], ep)
                    for s in got:
                        s["season"], s["episode"] = b["season"], ep
                    streams += got
                except Exception as e:
                    errors.append(f"E{ep}: {e}")
        return {"streams": streams, "errors": errors}


def main():
    host = os.environ.get("HOST", "127.0.0.1")
    port = int(os.environ.get("PORT", 8790))
    if host not in ("127.0.0.1", "localhost", "::1") and not load_config().get("pin"):
        sys.exit("Refusing to listen on the network without a PIN (your TorBox links would be open to the LAN).\n"
                 "Run once on localhost, set a PIN in Settings, then restart with HOST=0.0.0.0.")
    srv = ThreadingHTTPServer((host, port), H)
    if DEBUG:
        dbg(f"python {sys.version.split()[0]} frozen={FROZEN} data_dir={DATA_DIR} static={STATIC_DIR} "
            f"static_ok={os.path.isdir(STATIC_DIR)} host={host} port={port}")
        c = load_config()
        dbg(f"config: manifest={'set' if c.get('manifest_url') else 'missing'} torbox_key={'set' if c.get('torbox_api_key') else 'missing'} has_pin={'yes' if c.get('pin') else 'no'}")
    print(f"Open http://127.0.0.1:{port}" + (f"  (LAN: http://<this-computer-ip>:{port})" if host == "0.0.0.0" else ""), flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
