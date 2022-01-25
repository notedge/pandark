// gate: G-PANDARK-1
// fixture: pandark.cache.robots_ttl

use std::thread;
use std::time::Duration;

use pandark_store::{MemoryStore, Store, StoreConfig, YydbStore};

const SITE_KEY: &str = "example.com";
const ROBOTS_BODY: &[u8] = b"User-agent: *\nDisallow: /private";

fn assert_robots_ttl<S: Store>(make_store: impl FnOnce() -> S) {
    let mut store = make_store();
    store
        .put_robots_cache(SITE_KEY, ROBOTS_BODY, Duration::from_millis(50))
        .expect("put robots cache");
    assert_eq!(
        store
            .get_robots_cache(SITE_KEY)
            .expect("get robots cache")
            .as_deref(),
        Some(ROBOTS_BODY)
    );
    thread::sleep(Duration::from_millis(60));
    assert!(
        store
            .get_robots_cache(SITE_KEY)
            .expect("get robots cache after ttl")
            .is_none(),
        "expired robots cache must miss"
    );
}

#[test]
fn g_pandark_cache_robots_ttl_memory() {
    assert_robots_ttl(|| MemoryStore::open(StoreConfig::memory(1)).expect("open"));
}

#[cfg(feature = "yydb")]
mod yydb_helpers {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn temp_db_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("pandark-store-{label}-{nonce}.yydb"))
    }

    pub fn cleanup_db(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(format!("{}-wal", path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    }
}

#[cfg(feature = "yydb")]
#[test]
fn g_pandark_cache_robots_ttl_yydb() {
    let path = yydb_helpers::temp_db_path("robots-ttl");
    assert_robots_ttl(|| YydbStore::open(StoreConfig::yydb(&path, 1)).expect("open"));
    yydb_helpers::cleanup_db(&path);
}
