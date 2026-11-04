//! End-to-end against fake Stremio addons and a fake TorBox (no network).
use companion_core::{addons, jobs, playlist, torbox::TorBox};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn manifest(s: &MockServer, base: &str, id: &str, prefixes: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(format!("{base}/manifest.json")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id, "name": id, "version": "1.0.0", "types": ["movie", "series"],
            "idPrefixes": prefixes, "resources": ["stream"]})))
        .mount(s)
        .await;
}

#[tokio::test]
async fn multi_addon_fetch_dedupes_and_isolates_failures() {
    let s = MockServer::start().await;
    for (b, id, p) in [("/a", "addon.a", json!(["tt"])), ("/b", "addon.b", json!(["tt"])), ("/c", "addon.c", json!(["tt"])), ("/d", "addon.d", json!(["kitsu"]))] {
        manifest(&s, b, id, p).await;
    }
    let st = |hash: &str, seeds: u32| json!({"streams":[{"name":"[TB+] X 4K","description":format!("Show.S01E02.2160p.mkv 👤 {seeds}"),"infoHash":hash}]});
    Mock::given(method("GET")).and(path("/a/stream/series/tt1:1:2.json")).respond_with(ResponseTemplate::new(200).set_body_json(st("AAA", 5))).mount(&s).await;
    Mock::given(method("GET")).and(path("/b/stream/series/tt1:1:2.json")).respond_with(ResponseTemplate::new(200).set_body_json(st("aaa", 99))).mount(&s).await;
    Mock::given(method("GET")).and(path("/c/stream/series/tt1:1:2.json")).respond_with(ResponseTemplate::new(500)).mount(&s).await;

    let c = addons::http_client();
    let mut list = Vec::new();
    for b in ["a", "b", "c", "d"] {
        list.push(addons::add_addon(&c, &format!("{}/{b}/manifest.json", s.uri())).await.unwrap());
    }
    list[1].name = "Second".into();
    let r = addons::fetch_streams(&c, &list, "series", "tt1:1:2", Some(1), Some(2)).await;
    assert_eq!(r.streams.len(), 1, "same hash from two addons collapses");
    assert_eq!(r.streams[0].seeders, Some(99), "keeps the better copy");
    assert_eq!(r.streams[0].addon_name, "Second");
    assert_eq!((r.streams[0].season, r.streams[0].episode), (Some(1), Some(2)));
    assert_eq!(r.errors.len(), 1, "only addon c fails; d is skipped by idPrefixes");
    assert_eq!(r.errors[0].addon_name, "addon.c");
}

#[tokio::test]
async fn disabled_addons_are_skipped_and_bad_manifest_rejected() {
    let s = MockServer::start().await;
    manifest(&s, "/a", "addon.a", json!(["tt"])).await;
    Mock::given(method("GET")).and(path("/bad/manifest.json")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"hello": 1}))).mount(&s).await;
    let c = addons::http_client();
    let mut a = addons::add_addon(&c, &format!("{}/a", s.uri())).await.unwrap();
    assert!(addons::add_addon(&c, &format!("{}/bad", s.uri())).await.is_err());
    a.enabled = false;
    let r = addons::fetch_streams(&c, &[a], "movie", "tt9", None, None).await;
    assert!(r.streams.is_empty() && r.errors.is_empty());
}

#[tokio::test]
async fn catalog_search_merges() {
    let s = MockServer::start().await;
    Mock::given(method("GET")).and(path("/m/manifest.json")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id":"m","name":"M","resources":["catalog"],"types":["movie"],
        "catalogs":[{"type":"movie","id":"top","extra":[{"name":"search"}]}]}))).mount(&s).await;
    Mock::given(method("GET")).and(path("/m/catalog/movie/top/search=the%20thing.json")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "metas":[{"imdb_id":"tt9","name":"The Thing","releaseInfo":"1982"},{"id":"tt9","name":"dup"}]}))).mount(&s).await;
    let c = addons::http_client();
    let a = addons::add_addon(&c, &format!("{}/m", s.uri())).await.unwrap();
    let r = addons::search(&c, &[a], "movie", "the thing").await;
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["id"], "tt9");
}

#[tokio::test]
async fn send_job_builds_playlist_from_torbox() {
    let s = MockServer::start().await;
    Mock::given(method("POST")).and(path("/tb/torrents/createtorrent")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{"torrent_id":7}}))).mount(&s).await;
    Mock::given(method("GET")).and(path("/tb/torrents/mylist")).and(query_param("id", "7")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":{
        "id":7,"download_finished":true,"files":[
          {"id":1,"name":"Show/S01E01.mkv","short_name":"S01E01.mkv","size":5},
          {"id":2,"name":"Show/S01E02.mkv","short_name":"S01E02.mkv","size":6}]}}))).mount(&s).await;
    for f in ["1", "2"] {
        Mock::given(method("GET")).and(path("/tb/torrents/requestdl")).and(query_param("file_id", f)).and(query_param("token", "KEY"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":true,"data":format!("https://dl.example/7/{f}.mkv")}))).mount(&s).await;
    }
    let tb = TorBox::new(&format!("{}/tb", s.uri()), "KEY");
    let dir = tempfile::tempdir().unwrap();
    let items = vec![
        jobs::SendItem { title: "Ep2".into(), magnet: Some("magnet:?xt=urn:btih:aaa".into()), url: None, torrent_id: None, season: Some(1), episode: Some(2), add_only: false, all_files: false },
        jobs::SendItem { title: "Direct".into(), magnet: None, url: Some("https://cdn.example/m.mkv".into()), torrent_id: None, season: None, episode: None, add_only: false, all_files: false },
    ];
    let upd = |_: &jobs::Job| {};
    let j = jobs::run_send(&tb, dir.path(), items, "My Show", false, jobs::Opts::default(), &upd).await;
    assert!(j.finished && j.rows.iter().all(|r| r.state == "done"), "{:?}", j.rows);
    let file = j.playlist.unwrap();
    assert_eq!(file, "My_Show.m3u8");
    let e = playlist::read(dir.path(), &file).unwrap();
    assert_eq!(e[0].url, "https://dl.example/7/2.mkv");
    assert_eq!(e[1].url, "https://cdn.example/m.mkv");
    // whole-torrent mode + append
    let all = vec![jobs::SendItem { title: "S01".into(), magnet: None, url: None, torrent_id: Some(7), season: None, episode: None, add_only: false, all_files: true }];
    let j2 = jobs::run_send(&tb, dir.path(), all, "My Show", true, jobs::Opts::default(), &upd).await;
    assert_eq!(playlist::read(dir.path(), &j2.playlist.unwrap()).unwrap().len(), 4);
}

#[tokio::test]
async fn torbox_error_is_reported_per_row() {
    let s = MockServer::start().await;
    Mock::given(method("POST")).and(path("/tb/torrents/createtorrent")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"success":false,"detail":"plan limit"}))).mount(&s).await;
    let tb = TorBox::new(&format!("{}/tb", s.uri()), "KEY");
    let dir = tempfile::tempdir().unwrap();
    let it = jobs::SendItem { title: "x".into(), magnet: Some("magnet:?xt=urn:btih:z".into()), url: None, torrent_id: None, season: None, episode: None, add_only: false, all_files: false };
    let j = jobs::run_send(&tb, dir.path(), vec![it], "p", false, jobs::Opts::default(), &|_| {}).await;
    assert_eq!(j.rows[0].state, "error");
    assert!(j.rows[0].detail.contains("plan limit"));
    assert!(j.playlist.is_none());
}
