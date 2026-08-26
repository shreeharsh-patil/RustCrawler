use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResultStoreError {
    #[error("Object not found at location: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[async_trait]
pub trait ResultStore: Send + Sync {
    async fn put(
        &self,
        job_id: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String, ResultStoreError>;

    async fn get(&self, location: &str) -> Result<Vec<u8>, ResultStoreError>;
    async fn list_chunk_locations(&self, job_id: &str) -> Result<Vec<String>, ResultStoreError>;
}

#[derive(Clone, Default)]
pub struct LocalResultStore {
    // location -> (content_type, data)
    storage: Arc<DashMap<String, (String, Vec<u8>)>>,
    // job_id -> list of locations
    job_indices: Arc<DashMap<String, Vec<String>>>,
}

impl LocalResultStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ResultStore for LocalResultStore {
    async fn put(
        &self,
        job_id: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String, ResultStoreError> {
        let clean_key = key.trim_start_matches('/');
        let location = format!("jobs/{job_id}/{clean_key}");

        self.storage
            .insert(location.clone(), (content_type.to_string(), data));
        let mut entry = self.job_indices.entry(job_id.to_string()).or_default();
        if !entry.contains(&location) {
            entry.push(location.clone());
        }

        Ok(location)
    }

    async fn get(&self, location: &str) -> Result<Vec<u8>, ResultStoreError> {
        if let Some(entry) = self.storage.get(location) {
            Ok(entry.1.clone())
        } else {
            Err(ResultStoreError::NotFound(location.to_string()))
        }
    }

    async fn list_chunk_locations(&self, job_id: &str) -> Result<Vec<String>, ResultStoreError> {
        Ok(self
            .job_indices
            .get(job_id)
            .map(|r| r.clone())
            .unwrap_or_default())
    }
}

pub struct S3ResultStore {
    fallback: LocalResultStore,
    bucket: String,
    endpoint: Option<String>,
}

impl S3ResultStore {
    pub fn new(bucket: String, endpoint: Option<String>) -> Self {
        Self {
            fallback: LocalResultStore::new(),
            bucket,
            endpoint,
        }
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }
}

#[async_trait]
impl ResultStore for S3ResultStore {
    async fn put(
        &self,
        job_id: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String, ResultStoreError> {
        self.fallback.put(job_id, key, data, content_type).await
    }

    async fn get(&self, location: &str) -> Result<Vec<u8>, ResultStoreError> {
        self.fallback.get(location).await
    }

    async fn list_chunk_locations(&self, job_id: &str) -> Result<Vec<String>, ResultStoreError> {
        self.fallback.list_chunk_locations(job_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_result_store_put_get_list() {
        let store = LocalResultStore::new();
        let job_id = "job_test_result";

        let loc1 = store
            .put(
                job_id,
                "pages/hash1.json",
                b"{\"page\":1}".to_vec(),
                "application/json",
            )
            .await
            .unwrap();

        let loc2 = store
            .put(
                job_id,
                "pages/hash2.json",
                b"{\"page\":2}".to_vec(),
                "application/json",
            )
            .await
            .unwrap();

        let data1 = store.get(&loc1).await.unwrap();
        assert_eq!(data1, b"{\"page\":1}");

        let data2 = store.get(&loc2).await.unwrap();
        assert_eq!(data2, b"{\"page\":2}");

        let locations = store.list_chunk_locations(job_id).await.unwrap();
        assert_eq!(locations.len(), 2);
        assert!(locations.contains(&loc1));
        assert!(locations.contains(&loc2));
    }
}
