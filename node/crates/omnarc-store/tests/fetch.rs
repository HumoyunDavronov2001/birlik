//! One device adds a model file; another fetches one window of its layers over QUIC on this machine, gets a file
//! that holds exactly that window, and resumes a fetch that was cut short.

use std::path::PathBuf;

use omnarc_network::protocol::Router;
use omnarc_network::{EndpointAddr, SecretKey, TransportAddr, bind_with};
use omnarc_store::testing::{model, model_with};
use omnarc_store::{BLOBS_ALPN, ModelStore, Window};

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnarc-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_device_fetches_only_its_window() {
    let (a, b) = (dir("a"), dir("b"));
    let file = a.join("Tiny-8L.gguf");
    model(&file, 8);

    // the first device adds the file where it lies and serves its store
    let store_a = ModelStore::open(&a.join("models")).await.unwrap();
    let added = store_a.add(&file, |_| {}).await.unwrap();
    assert_eq!(added.name, "Tiny-8L");
    assert_eq!(added.layout.layers, 8);
    let endpoint_a = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
    let router = Router::builder(endpoint_a.clone()).accept(BLOBS_ALPN, store_a.protocol()).spawn();
    let addr_a =
        EndpointAddr::from_parts(endpoint_a.id(), endpoint_a.bound_sockets().into_iter().map(TransportAddr::Ip));

    // the second device fetches the layers 2:5 and nothing else
    let store_b = ModelStore::open(&b.join("models")).await.unwrap();
    let endpoint_b = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
    let connection = endpoint_b.connect(addr_a, BLOBS_ALPN).await.unwrap();
    let window: Window = "2:5".parse().unwrap();
    let mut arrived = 0;
    let fetched = store_b.fetch(&connection, Some("Tiny-8L"), added.hash, window, |n| arrived = n).await.unwrap();
    assert_eq!(fetched.layout, added.layout);
    assert!((3 * (64 << 10)..4 * (64 << 10)).contains(&arrived), "only three layers travel, not {arrived} bytes");
    assert_eq!(store_b.missing(&fetched, window).await.unwrap(), 0);
    assert!(store_b.missing(&fetched, "0:2".parse().unwrap()).await.unwrap() > 0);

    // the engine's file: the model's full length, the window in place, zeros elsewhere
    let out = b.join("window.gguf");
    store_b.export(&fetched, window, &out).await.unwrap();
    let original = std::fs::read(&file).unwrap();
    let exported = std::fs::read(&out).unwrap();
    assert_eq!(exported.len(), original.len());
    for t in &fetched.header.tensors {
        let range = usize::try_from(t.range.start).unwrap()..usize::try_from(t.range.end).unwrap();
        let wanted = ["blk.2.", "blk.3.", "blk.4."].iter().any(|p| t.name.starts_with(p));
        if wanted {
            assert_eq!(exported[range.clone()], original[range], "{}", t.name);
        } else {
            assert!(exported[range].iter().all(|&b| b == 0), "{} should be a hole", t.name);
        }
    }
    let header = usize::try_from(fetched.header.data_offset).unwrap();
    assert_eq!(exported[..header], original[..header]);
    // the store's file of the part and the engine's file take only the window's space: on Windows they are sparse
    for path in [store_b.part_path(added.hash), out.clone()] {
        let file = std::fs::File::open(&path).unwrap();
        assert!(omnarc_store::sparse::is_sparse(&file).unwrap(), "{} is not sparse", path.display());
    }

    // a second fetch of the same window moves nothing; the next window brings only its own layers
    let mut again = 0;
    store_b.fetch(&connection, Some("Tiny-8L"), added.hash, window, |n| again = n).await.unwrap();
    assert_eq!(again, 0);
    assert!(store_b.export(&fetched, "5:9".parse().unwrap(), &out).await.is_err(), "not fetched yet");
    let mut rest = 0;
    store_b.fetch(&connection, Some("Tiny-8L"), added.hash, "5:9".parse().unwrap(), |n| rest = n).await.unwrap();
    assert!((4 * (64 << 10)..5 * (64 << 10)).contains(&rest), "{rest}");

    // the second device lists what it holds, under one name: the one given last
    let list = store_b.list().await.unwrap();
    assert_eq!(list, vec![("Tiny-8L".to_owned(), added.hash)]);
    store_b.fetch(&connection, None, added.hash, window, |_| {}).await.unwrap();
    assert_eq!(store_b.list().await.unwrap(), list, "a fetch without a name keeps the name it has");
    store_b.fetch(&connection, Some("Tiny"), added.hash, window, |_| {}).await.unwrap();
    assert_eq!(store_b.list().await.unwrap(), vec![("Tiny".to_owned(), added.hash)]);
    assert_eq!(store_b.model(&added.hash.to_hex()).await.unwrap().layout, added.layout);
    assert_eq!(store_b.model("Tiny").await.unwrap().hash, added.hash);

    router.shutdown().await.unwrap();
    endpoint_b.close().await;
    store_a.shutdown().await;
    store_b.shutdown().await;
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_fetch_cut_short_resumes_where_it_stopped() {
    let (a, b) = (dir("resume-a"), dir("resume-b"));
    let file = a.join("Big.gguf");
    // 8 layers of 4 MiB
    model_with(&file, 8, 4 << 20);
    let store_a = ModelStore::open(&a.join("models")).await.unwrap();
    let added = store_a.add(&file, |_| {}).await.unwrap();
    let endpoint_a = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
    let router = Router::builder(endpoint_a.clone()).accept(BLOBS_ALPN, store_a.protocol()).spawn();
    let addr_a =
        EndpointAddr::from_parts(endpoint_a.id(), endpoint_a.bound_sockets().into_iter().map(TransportAddr::Ip));

    let store_b = ModelStore::open(&b.join("models")).await.unwrap();
    let endpoint_b = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
    let window = added.layout.all();
    let whole = added.layout.bytes(window).unwrap();

    // the first fetch is dropped once a quarter of the model has arrived
    let connection = endpoint_b.connect(addr_a.clone(), BLOBS_ALPN).await.unwrap();
    let (cut, cut_now) = tokio::sync::oneshot::channel::<()>();
    let mut cut = Some(cut);
    let mut first = 0;
    let fetch = store_b.fetch(&connection, None, added.hash, window, |n| {
        first = n;
        if n > whole / 4 {
            cut.take().map(|cut| cut.send(()));
        }
    });
    tokio::select! {
        result = fetch => panic!("the fetch was not cut short: {result:?}"),
        _ = cut_now => {}
    }
    connection.close(0u32.into(), b"cut");
    let missing = {
        let model = store_b.model(&added.hash.to_hex()).await.unwrap();
        store_b.missing(&model, window).await.unwrap()
    };
    assert!(missing > 0 && missing < whole, "{missing} of {whole} bytes missing after the cut");

    // the next fetch brings only what is missing
    let connection = endpoint_b.connect(addr_a, BLOBS_ALPN).await.unwrap();
    let mut second = 0;
    let model = store_b.fetch(&connection, None, added.hash, window, |n| second = n).await.unwrap();
    assert_eq!(store_b.missing(&model, window).await.unwrap(), 0);
    assert!(second < whole - whole / 4, "{second} bytes travelled again of {whole}");

    let out = b.join("whole.gguf");
    store_b.export(&model, window, &out).await.unwrap();
    assert!(std::fs::read(&out).unwrap() == std::fs::read(&file).unwrap(), "the model arrived whole and unchanged");

    router.shutdown().await.unwrap();
    endpoint_b.close().await;
    store_a.shutdown().await;
    store_b.shutdown().await;
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}
