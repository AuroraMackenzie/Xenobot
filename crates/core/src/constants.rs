//! Application constants and configuration defaults.

/// Default database page size for V4 decryption.
pub const DEFAULT_PAGE_SIZE: usize = 4096;

/// Default reserve size for V4 decryption.
pub const DEFAULT_RESERVE_SIZE: usize = 48;

/// PBKDF2 iteration count for V4 key derivation.
pub const V4_PBKDF2_ITERATIONS: u32 = 256_000;

/// Default HTTP server port.
pub const DEFAULT_HTTP_PORT: u16 = 8080;

/// Default WebSocket server port.
pub const DEFAULT_WS_PORT: u16 = 8081;

/// Maximum file size for import (100 MB).
pub const MAX_IMPORT_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Maximum number of concurrent connections.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 100;

/// Default database connection pool size.
pub const DEFAULT_DB_POOL_SIZE: u32 = 10;

/// Default session timeout in seconds.
pub const DEFAULT_SESSION_TIMEOUT: u64 = 3600;

/// Default cache TTL in seconds.
pub const DEFAULT_CACHE_TTL: u64 = 300;
