//! db-store（灵脉）配置
//!
//! 来源: modules/db-store/接口设计.md:80-117 — DbConfig + PRAGMA 参考
//! 来源: modules/db-store/接口设计.md:525-537 — BufferConfig

/// SQLite 数据库连接配置
///
/// WAL 模式默认开启，满足高并发读场景。
/// 测试时使用 `database_path = ":memory:"`。
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// 数据库文件路径（支持 `:memory:`）
    pub database_path: String,
    /// WAL 模式（默认 true）
    pub wal_mode: bool,
    /// 忙等待超时（毫秒，默认 5000）
    pub busy_timeout_ms: u32,
    /// 缓存大小（KB，默认 64000 = 64MB）
    pub cache_size_kb: u32,
    /// 是否开启外键约束（默认 true）
    pub foreign_keys: bool,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            database_path: "lvpa.db".into(),
            wal_mode: true,
            busy_timeout_ms: 5000,
            cache_size_kb: 64000,
            foreign_keys: true,
        }
    }
}

impl DbConfig {
    /// 创建 `:memory:` 数据库配置（测试用）
    pub fn in_memory() -> Self {
        Self {
            database_path: ":memory:".into(),
            ..Self::default()
        }
    }

    /// 生成 SQLite PRAGMA 语句列表
    ///
    /// 来源: modules/db-store/接口设计.md:110-116 — PRAGMA 参考
    pub fn pragmas(&self) -> Vec<String> {
        let mut p = Vec::with_capacity(6);
        if self.wal_mode {
            p.push("PRAGMA journal_mode = WAL;".into());
        }
        p.push(format!("PRAGMA busy_timeout = {};", self.busy_timeout_ms));
        p.push(format!("PRAGMA cache_size = -{};", self.cache_size_kb));
        p.push("PRAGMA synchronous = NORMAL;".into());
        if self.foreign_keys {
            p.push("PRAGMA foreign_keys = ON;".into());
        }
        p.push("PRAGMA temp_store = MEMORY;".into());
        p
    }
}

/// L1 内存环缓冲配置
///
/// 来源: modules/db-store/接口设计.md:524-537 — BufferConfig
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// 每 (symbol, freq) 的最大缓存 K 线条数
    pub max_bars_per_slot: usize,
    /// 是否启用订阅通知
    pub enable_notify: bool,
    /// 落盘批量大小
    pub flush_batch_size: usize,
    /// 落盘间隔（毫秒）
    pub flush_interval_ms: u64,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_bars_per_slot: 10_000,
            enable_notify: true,
            flush_batch_size: 100,
            flush_interval_ms: 1000,
        }
    }
}
