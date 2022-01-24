use std::path::Path;

use pandark_types::{CrawlError, FetchArtifact, Result, TransportProvenance};
use yydb::{Connection, OpenFlags};

use crate::keyspace;

const RESPONSE_CACHE_SCHEMA_VERSION: u32 = 1;
const RESPONSE_CACHE_SCHEMA: &str = "table PandarkCache { @@key: utf8, value: utf8 }";

/// YYDB-backed response cache. Metadata and inline bodies are stored as one record.
pub struct YydbResponseCache {
    connection: Connection,
}

impl YydbResponseCache {
    /// Open a WAL-backed YYDB response cache.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open_with_flags(path, OpenFlags::wal())
            .map_err(|error| CrawlError::InvalidInput(format!("open YYDB cache: {error}")))?;
        connection
            .ensure_schema(RESPONSE_CACHE_SCHEMA_VERSION, RESPONSE_CACHE_SCHEMA)
            .map_err(|error| {
                CrawlError::InvalidInput(format!("initialize YYDB cache: {error}"))
            })?;
        Ok(Self { connection })
    }

    /// Read an artifact and mark it as a cache hit.
    pub fn get(&self, cache_key: &str) -> Result<Option<FetchArtifact>> {
        let key = keyspace::response_cache(cache_key);
        let Some(bytes) = self.connection.get(&key).map_err(map_error)? else {
            return Ok(None);
        };
        let mut artifact: FetchArtifact = serde_json::from_slice(&bytes).map_err(|error| {
            CrawlError::InvalidInput(format!("decode YYDB cache entry: {error}"))
        })?;
        artifact.transport_provenance = TransportProvenance::CacheHit;
        Ok(Some(artifact))
    }

    /// Store an artifact in YYDB and return its cache key.
    pub fn put(&self, artifact: &FetchArtifact) -> Result<String> {
        let key = keyspace::response_cache(&artifact.cache_key);
        let bytes = serde_json::to_vec(artifact).map_err(|error| {
            CrawlError::InvalidInput(format!("encode YYDB cache entry: {error}"))
        })?;
        self.connection.put(key, bytes).map_err(map_error)?;
        Ok(artifact.cache_key.clone())
    }

    /// Flush the YYDB write-ahead log.
    pub fn checkpoint(&self) -> Result<()> {
        self.connection.checkpoint().map_err(map_error)
    }
}

fn map_error(error: yydb::Error) -> CrawlError {
    CrawlError::InvalidInput(format!("YYDB cache error: {error}"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use pandark_types::{BodyStorage, FetchArtifact, TransportProvenance};
    use url::Url;

    use super::YydbResponseCache;

    #[test]
    fn round_trips_artifact() {
        let path = std::env::temp_dir().join(format!(
            "pandark-store-cache-{}.yydb",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let cache = YydbResponseCache::open(&path).expect("cache");
        let artifact = FetchArtifact {
            request_url: Url::parse("https://example.com/").unwrap(),
            final_url: Url::parse("https://example.com/").unwrap(),
            status: 200,
            response_headers: Default::default(),
            content_type: Some("text/html".into()),
            body: BodyStorage::inline(b"ok"),
            redirect_chain: Vec::new(),
            retrieved_at_epoch: 1,
            transport_provenance: TransportProvenance::Network,
            cache_key: "https://example.com/".into(),
        };
        cache.put(&artifact).expect("put");
        let hit = cache.get(&artifact.cache_key).expect("get").expect("hit");
        assert_eq!(hit.body_bytes(), Some(b"ok".as_slice()));
        let _ = std::fs::remove_file(path);
    }
}
