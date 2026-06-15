use async_trait::async_trait;
use dms_core::CoreResult;

/// 已存储 blob 的信息。
#[derive(Debug, Clone)]
pub struct BlobInfo {
    /// 内容的 SHA-256（十六进制，64 字符）——既是寻址 key，也用于完整性校验。
    pub sha256: String,
    /// 字节数。
    pub size: u64,
}

/// 内容寻址 blob 存储端口（sha256 寻址 + 去重）。
///
/// 实现按内容 sha256 寻址、散列分片落盘，同内容只存一份。文件系统/对象存储(S3/MinIO)
/// 等后端可互换，由组合根注入。业务模块（如文件管理）在其上记录元数据(project/目录/名)。
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// 存入内容，返回 sha256 与大小（同内容已存在则直接返回、不重复写）。
    async fn put(&self, data: &[u8]) -> CoreResult<BlobInfo>;
    /// 按 sha256 读取内容。
    async fn get(&self, sha256: &str) -> CoreResult<Vec<u8>>;
    /// 是否已存在该内容。
    async fn exists(&self, sha256: &str) -> CoreResult<bool>;
}
