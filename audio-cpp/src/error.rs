//! 统一错误类型。

use std::path::PathBuf;

/// audio-cpp 高层 API 的错误类型。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// C ABI 调用失败（返回非 0），`msg` 为 shim 的 `audiocpp_last_error()`。
    #[error("C ABI 调用失败: {0}")]
    Ffi(String),

    /// C++ 侧返回空句柄（如加载模型 / 创建会话失败），`msg` 为错误信息。
    #[error("空句柄: {0}")]
    NullHandle(String),

    /// JSON 解析 / 序列化错误。
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// 字符串或路径中含 NUL 字节，无法转为 C 字符串。
    #[error("参数含 NUL 字节: {0}")]
    Nul(#[from] std::ffi::NulError),

    /// 路径含非法 UTF-8（capi.h 约定所有字符串均为 UTF-8）。
    #[error("路径不是合法 UTF-8: {0}")]
    NonUtf8Path(PathBuf),

    /// 其他运行时错误（如示例中用户自选解码库的失败信息）。
    #[error("{0}")]
    Other(String),
}
