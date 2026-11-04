import json, os, sys, tempfile, threading, unittest, urllib.request, urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

# ---- fake upstream: AIOStreams + TorBox + Cinemeta in one server
class Fake(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _j(self, o):
        b = json.dumps(o).encode(); self.send_response(200)
        self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(b)))
        self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        p = self.path
        if p.startswith("/aio/stream/series/tt1:1:2"):
            return self._j({"streams": [
                {"name": "[TB+] Torrentio 4K", "description": "Show.S01E02.2160p.WEB-DL.HEVC.HDR10.Atmos.mkv\n💾 8.4 GB 👤 120",
                 "infoHash": "AAA", "sources": ["tracker:udp://t.example:80"]},
                {"name": "[TB+] Dup", "description": "Show.S01E02.2160p 💾 8 GB 👤 3", "infoHash": "aaa"},
                {"name": "Cam", "description": "Show.S01E02.1080p.CAM.x264.mkv 1 GB", "infoHash": "bbb"}]})
        if p.startswith("/aio/stream/movie/tt9"):
            return self._j({"streams": [{"name": "[RD] Direct", "description": "Movie.1080p.mkv 4 GB", "url": "https://cdn.example/m.mkv"}]})
        if p.startswith("/cm/catalog/movie/top/search="):
            return self._j({"metas": [{"imdb_id": "tt9", "name": "Movie", "releaseInfo": "2020"}]})
        if p.startswith("/tb/torrents/mylist"):
            t = {"id": 7, "name": "Show S01", "size": 100, "download_finished": True, "download_state": "cached", "progress": 1,
                 "files": [{"id": 1, "name": "Show/S01E01.mkv", "short_name": "S01E01.mkv", "size": 5},
                           {"id": 2, "name": "Show/S01E02.mkv", "short_name": "S01E02.mkv", "size": 6}]}
            return self._j({"success": True, "data": t if "id=7" in p else [t]})
        if p.startswith("/tb/torrents/requestdl"):
            q = dict(x.split("=") for x in p.split("?")[1].split("&"))
            assert q["token"] == "KEY"
            return self._j({"success": True, "data": f"https://dl.example/{q['torrent_id']}/{q['file_id']}.mkv"})
        self.send_response(404); self.end_headers()
    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0); body = self.rfile.read(n)
        assert self.headers["Authorization"] == "Bearer KEY"
        assert b"magnet:?xt=urn:btih:aaa" in body
        self._j({"success": True, "data": {"torrent_id": 7}})

fake = ThreadingHTTPServer(("127.0.0.1", 0), Fake)
threading.Thread(target=fake.serve_forever, daemon=True).start()
FP = fake.server_address[1]
TMP = tempfile.mkdtemp()
os.environ.update(DATA_DIR=TMP, TORBOX_API=f"http://127.0.0.1:{FP}/tb", CINEMETA_API=f"http://127.0.0.1:{FP}/cm")
import app  # noqa: E402


class Unit(unittest.TestCase):
    def test_parse_rich_fields(self):
        s = app.parse_stream({"name": "[TB+] X 4K", "description": "Show.S01E02.2160p.WEB-DL.HEVC.HDR10.Atmos.mkv\n💾 8.4 GB 👤 120", "infoHash": "ABC"})
        self.assertEqual((s["resolution"], s["codec"], s["hdr"], s["audio"], s["quality"]), ("2160p", "HEVC", "HDR10", "Atmos", "WEB-DL"))
        self.assertEqual((s["season"], s["episode"], s["seeders"], s["cached"]), (1, 2, 120, True))
        self.assertTrue(s["magnet"].startswith("magnet:?xt=urn:btih:abc"))

    def test_dedupe_and_drop_unusable(self):
        raw = [app.parse_stream(x) for x in [{"name": "a", "infoHash": "H"}, {"name": "b", "infoHash": "h", "description": "👤 9"}, {"name": "none"}]]
        out = app.correct_and_dedupe(raw)
        self.assertEqual(len(out), 1); self.assertEqual(out[0]["seeders"], 9)

    def test_pick_file_matches_episode(self):
        files = [{"id": 1, "name": "a/S01E01.mkv", "size": 9}, {"id": 2, "name": "a/S01E02.mkv", "size": 1}, {"id": 3, "name": "a/x.nfo", "size": 99}]
        self.assertEqual(app.pick_file(files, 1, 2)["id"], 2)
        self.assertEqual(app.pick_file(files)["id"], 1)

    def test_m3u8_roundtrip(self):
        app.write_playlist("rt", [{"title": "A", "url": "http://x/1"}, {"title": "B", "url": "http://x/2"}])
        self.assertEqual([e["title"] for e in app.read_playlist("rt")], ["A", "B"])

    def test_manifest_base(self):
        self.assertEqual(app.base_from_manifest("stremio://h/a/b/manifest.json"), "https://h/a/b")

    def test_safe_name_blocks_traversal(self):
        self.assertNotIn("/", app.safe_name("../../etc/passwd"))


class E2E(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = app.ThreadingHTTPServer(("127.0.0.1", 0), app.H)
        threading.Thread(target=cls.srv.serve_forever, daemon=True).start()
        cls.base = f"http://127.0.0.1:{cls.srv.server_address[1]}"

    @classmethod
    def tearDownClass(cls): cls.srv.shutdown()

    @staticmethod
    def _parse(raw):
        try: return json.loads(raw or b"{}")
        except ValueError: return {"_raw": raw}

    def req(self, path, body=None, headers=None):
        r = urllib.request.Request(self.base + path, data=None if body is None else json.dumps(body).encode(),
                                   headers={"Content-Type": "application/json", **(headers or {})})
        try:
            with urllib.request.urlopen(r) as resp: return resp.status, self._parse(resp.read()), resp
        except urllib.error.HTTPError as e: return e.code, self._parse(e.read()), e

    def wait_job(self, jid):
        import time
        for _ in range(50):
            _, j, _ = self.req("/api/job/" + jid)
            if j["finished"]: return j
            time.sleep(0.1)
        self.fail("job timeout")

    def test_flow(self):
        st, _, r = self.req("/")
        self.assertEqual(st, 200); self.assertEqual(r.headers["Content-Type"].split(";")[0], "text/html")
        self.assertEqual(self.req("/app.js")[0], 200); self.assertEqual(self.req("/manifest.webmanifest")[0], 200)
        self.assertEqual(self.req("/../app.py")[0], 404)
        self.req("/api/config", {"manifest_url": f"http://127.0.0.1:{FP}/aio/manifest.json", "torbox_api_key": "KEY"})
        self.assertTrue(self.req("/api/status")[1]["configured"])
        self.assertEqual(self.req("/api/search", {"query": "movie", "kind": "movie"})[1]["results"][0]["id"], "tt9")
        # series fetch: dedupe collapses AAA/aaa, CAM flagged
        _, f, _ = self.req("/api/fetch", {"kind": "series", "imdb": "tt1", "season": 1, "ep1": 2, "ep2": 2})
        self.assertEqual(len(f["streams"]), 2); self.assertTrue(any(s["bad"] for s in f["streams"]))
        good = [s for s in f["streams"] if not s["bad"]][0]
        _, j, _ = self.req("/api/send", {"items": [good], "name": "My Show"})
        job = self.wait_job(j["job"]); self.assertEqual(job["rows"][0]["state"], "done", job)
        self.assertEqual(job["playlist"], "My_Show.m3u8")
        # picked E02 file (id 2) from the torrent
        self.assertEqual(app.read_playlist("My_Show")[0]["url"], "https://dl.example/7/2.mkv")
        # direct-url movie, append mode
        _, m, _ = self.req("/api/fetch", {"kind": "movie", "imdb": "tt9"})
        _, j, _ = self.req("/api/send", {"items": m["streams"], "name": "My Show", "append": True})
        self.wait_job(j["job"]); self.assertEqual(len(app.read_playlist("My_Show")), 2)
        # library -> whole-torrent playlist
        _, lib, _ = self.req("/api/torbox/library"); self.assertEqual(lib["items"][0]["videos"], 2)
        _, j, _ = self.req("/api/send", {"items": [{"title": "Show S01", "torrent_id": 7, "all_files": True}], "name": "lib"})
        self.wait_job(j["job"]); self.assertEqual(len(app.read_playlist("lib")), 2)
        # edit/rename/delete
        self.req("/api/playlist/save", {"name": "renamed", "old": "lib", "entries": [{"title": "only", "url": "http://x"}]})
        names = [p["name"] for p in self.req("/api/playlists")[1]["playlists"]]
        self.assertIn("renamed", names); self.assertNotIn("lib", names)
        self.req("/api/playlist/delete", {"file": "renamed.m3u8"})
        self.assertNotIn("renamed", [p["name"] for p in self.req("/api/playlists")[1]["playlists"]])
        st, _, r = self.req("/playlist/My_Show.m3u8")
        self.assertEqual(r.headers["Content-Type"], "application/vnd.apple.mpegurl")

    def test_pin_auth(self):
        self.req("/api/config", {"pin": "1234"})
        try:
            self.assertEqual(self.req("/api/playlists")[0], 401)
            self.assertEqual(self.req("/api/login", {"pin": "0000"})[0], 401)
            st, body, r = self.req("/api/login", {"pin": "1234"})
            self.assertEqual(st, 200)
            cookie = r.headers["Set-Cookie"].split(";")[0]
            self.assertEqual(self.req("/api/playlists", headers={"Cookie": cookie})[0], 200)
            self.assertEqual(self.req("/api/playlists?k=" + body["token"])[0], 200)
            self.assertEqual(self.req("/")[0], 200)  # shell is public so the PIN screen can load
        finally:
            self.req("/api/config", {"clear_pin": True}, headers={"Cookie": cookie})


if __name__ == "__main__":
    unittest.main()
