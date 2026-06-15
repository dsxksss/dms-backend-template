//! 文件系统内容寻址 blob 存储（sha256 + 散列分片 + 原子写 + 去重）。
//!
//! 路径：`<root>/<sha[0:2]>/<sha[2:4]>/<sha256>`。同内容只存一份（去重）。
//! S3 / MinIO 等后端可另实现 [`BlobStore`] 替换，组合根注入。

use std::path::PathBuf;

use async_trait::async_trait;
use dms_application::port::{BlobInfo, BlobStore};
use dms_core::{CoreError, CoreResult};
use sha2::{Digest, Sha256};

pub struct FilesystemBlobStore {
    root: PathBuf,
}

impl FilesystemBlobStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, sha256: &str) -> PathBuf {
        self.root
            .join(&sha256[0..2])
            .join(&sha256[2..4])
            .join(sha256)
    }

    fn valid_sha(sha256: &str) -> bool {
        sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

#[async_trait]
impl BlobStore for FilesystemBlobStore {
    async fn put(&self, data: &[u8]) -> CoreResult<BlobInfo> {
        let sha256 = hex::encode(Sha256::digest(data));
        let path = self.path_for(&sha256);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| CoreError::internal(format!("blob mkdir: {e}")))?;
            }
            // 原子写：先写临时文件再 rename，避免读到半写内容。
            let tmp = path.with_extension("tmp");
            tokio::fs::write(&tmp, data)
                .await
                .map_err(|e| CoreError::internal(format!("blob write: {e}")))?;
            tokio::fs::rename(&tmp, &path)
                .await
                .map_err(|e| CoreError::internal(format!("blob commit: {e}")))?;
        }
        Ok(BlobInfo {
            sha256,
            size: data.len() as u64,
        })
    }

    async fn get(&self, sha256: &str) -> CoreResult<Vec<u8>> {
        if !Self::valid_sha(sha256) {
            return Err(CoreError::NotFound("blob not found".into()));
        }
        tokio::fs::read(self.path_for(sha256))
            .await
            .map_err(|_| CoreError::NotFound("blob not found".into()))
    }

    async fn exists(&self, sha256: &str) -> CoreResult<bool> {
        if !Self::valid_sha(sha256) {
            return Ok(false);
        }
        Ok(tokio::fs::try_exists(self.path_for(sha256))
            .await
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_get_and_dedup() {
        let dir = std::env::temp_dir().join(format!("dms-blob-{}", std::process::id()));
        let store = FilesystemBlobStore::new(&dir);

        let info = store.put(b"hello dms").await.unwrap();
        assert_eq!(info.size, 9);
        assert_eq!(info.sha256.len(), 64);
        assert!(store.exists(&info.sha256).await.unwrap());
        assert_eq!(store.get(&info.sha256).await.unwrap(), b"hello dms");

        // 同内容再 put → 同 sha、幂等（去重）。
        let info2 = store.put(b"hello dms").await.unwrap();
        assert_eq!(info.sha256, info2.sha256);

        // 非法/未知 sha → NotFound / false。
        assert!(store.get("zz").await.is_err());
        assert!(!store.exists("zz").await.unwrap());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
