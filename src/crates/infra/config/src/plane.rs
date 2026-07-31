//! 天书阁 — 配置平面枚举。
//!
//! 三平面优先级链：Env > File > Db > Default。
//!
//! 设计参考：gbrain `src/core/config.ts:28-399` 三平面配置模式，Rust 翻译实现，非 Cargo 依赖。

use serde::{Deserialize, Serialize};

/// 配置平面——表示配置值的来源层级。
///
/// 优先级（高→低）：
/// - `Env`：环境变量（最高优先级，`LVPA_*` 前缀）
/// - `File`：配置文件（`~/.taiji-quant/config.toml`）
/// - `Db`：数据库持久化配置（延迟实现）
/// - `Default`：硬编码默认值（最低优先级）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigPlane {
    /// 环境变量平面（最高优先级）。
    Env,
    /// 配置文件平面。
    File,
    /// 数据库平面（延迟实现）。
    Db,
    /// 默认值平面（最低优先级）。
    Default,
}

impl ConfigPlane {
    /// 返回平面的优先级顺序索引（0 = 最高）。
    pub fn priority(&self) -> u8 {
        match self {
            ConfigPlane::Env => 0,
            ConfigPlane::File => 1,
            ConfigPlane::Db => 2,
            ConfigPlane::Default => 3,
        }
    }

    /// 返回所有平面，按优先级从高到低排列。
    pub fn all_sorted() -> &'static [ConfigPlane] {
        &[ConfigPlane::Env, ConfigPlane::File, ConfigPlane::Db, ConfigPlane::Default]
    }
}

impl std::fmt::Display for ConfigPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigPlane::Env => write!(f, "env"),
            ConfigPlane::File => write!(f, "file"),
            ConfigPlane::Db => write!(f, "db"),
            ConfigPlane::Default => write!(f, "default"),
        }
    }
}
