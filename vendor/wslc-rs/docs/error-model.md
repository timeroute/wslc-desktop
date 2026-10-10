# 错误模型设计

## 1. 目标

Rust API 应该让用户能同时获得：

- 友好的错误类型；
- 原始 HRESULT；
- SDK 返回的 error message；
- 输入参数上下文；
- 可匹配的 WSLC 专有错误。

## 2. Error enum

```rust
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(&'static str),

    #[error("WSLC SDK not found: {0}")]
    SdkNotFound(String),

    #[error("missing WSL components: {0:?}")]
    MissingComponents(ComponentFlags),

    #[error("COM initialization failed: HRESULT=0x{code:08x}, {message}")]
    ComInitialization {
        code: i32,
        message: String,
    },

    #[error("WSLC call failed: HRESULT=0x{code:08x}, {message}")]
    HResult {
        code: i32,
        message: String,
    },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("string contains interior NUL: {0}")]
    Nul(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 conversion error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}
```

## 3. WSLC 专有错误码

底层 C API 定义了若干 WSLC 专有 HRESULT，例如：

```text
WSLC_E_IMAGE_NOT_FOUND
WSLC_E_CONTAINER_PREFIX_AMBIGUOUS
WSLC_E_CONTAINER_NOT_FOUND
WSLC_E_CONTAINER_NOT_RUNNING
WSLC_E_CONTAINER_IS_RUNNING
WSLC_E_INVALID_SESSION_NAME
WSLC_E_SDK_UPDATE_NEEDED
WSLC_E_REGISTRY_BLOCKED_BY_POLICY
WSLC_E_VOLUME_NOT_AVAILABLE
WSLC_E_SESSION_NOT_FOUND
```

建议提供辅助方法：

```rust
impl Error {
    pub fn wslc_kind(&self) -> Option<WslcErrorKind>;
    pub fn hresult(&self) -> Option<i32>;
}
```

```rust
pub enum WslcErrorKind {
    ImageNotFound,
    ContainerPrefixAmbiguous,
    ContainerNotFound,
    ContainerNotRunning,
    ContainerIsRunning,
    SessionReserved,
    InvalidSessionName,
    NetworkNotFound,
    SdkUpdateNeeded,
    RegistryBlockedByPolicy,
    VolumeNotAvailable,
    SessionNotFound,
    Unknown(i32),
}
```

## 4. 保留原始信息

不要把错误过度抽象成：

```rust
Error::ContainerNotFound
```

而丢失原始 message。更好的方式：

```rust
Error::HResult {
    code,
    message,
}
```

再通过：

```rust
err.wslc_kind() == Some(WslcErrorKind::ContainerNotFound)
```

判断类别。

## 5. 输入校验错误

Rust 层应尽早捕获：

- 空 session name；
- 非法 storage path；
- image name 空字符串；
- argv 为空；
- 字符串包含 `\0`；
- container path 不是绝对路径；
- port 为 0；
- memory_mb 太小或太大；
- timeout 不可转换成 u32 milliseconds。

不要把明显的输入错误直接传给 C API。
