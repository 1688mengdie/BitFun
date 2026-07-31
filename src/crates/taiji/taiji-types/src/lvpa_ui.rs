//! 修仙 UI 基础类型 — 场景/模式/主题 Token/境界色/灵根色。
//!
//! 定义 Phase 6 用户交互层（L3）共享类型，供 Transport 消息路由 + 跨 crate 使用。
//! 参考: 架构总纲 §4a 修仙 UI 设计规范

use serde::{Deserialize, Serialize};

// ============================================================
// 1.1 LvpaSceneId
// ============================================================

/// 修仙场景 ID 枚举 — 对应架构总纲 §4a 定义的 6 个修仙场景。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LvpaSceneId {
    /// 宗门 — 宗门大地图、建筑入口
    Sect,
    /// 工坊 — 工坊看板、工作流进度
    Workshop,
    /// 坊市 — 坊市货架、卡片交易
    Market,
    /// 洞府 — Agent 个人空间、状态面板
    Cave,
    /// 藏经阁 — 知识库、功法查阅
    Library,
    /// 接引台 — 外部接入、API Key 管理
    Gate,
}

impl LvpaSceneId {
    /// 返回场景的中文名称。
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Sect => "宗门",
            Self::Workshop => "工坊",
            Self::Market => "坊市",
            Self::Cave => "洞府",
            Self::Library => "藏经阁",
            Self::Gate => "接引台",
        }
    }

    /// 返回场景的 CSS class 名称。
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Sect => "lvpa-scene-sect",
            Self::Workshop => "lvpa-scene-workshop",
            Self::Market => "lvpa-scene-market",
            Self::Cave => "lvpa-scene-cave",
            Self::Library => "lvpa-scene-library",
            Self::Gate => "lvpa-scene-gate",
        }
    }
}

// ============================================================
// 1.2 LvpaMode
// ============================================================

/// 工作模式 — bitfun 原生模式 / taiji 修仙模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LvpaMode {
    /// BitFun 原生模式（默认）
    #[default]
    Bitfun,
    /// 太极修仙模式（显示 LVPA 场景 + 主题）
    Taiji,
}

// ============================================================
// 1.3 LvpaThemeToken
// ============================================================

/// LVPA 五色 Token — 墨色/朱砂/金线/青玉/云白。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LvpaThemeToken {
    /// 墨色（#1c1c1f）— 主文字/暗色底色
    pub ink: String,
    /// 朱砂（#c8102e）— 强调色/危险/信号
    pub vermillion: String,
    /// 金线（#d4a843）— 高亮/灵石/稀有
    pub gold: String,
    /// 青玉（#7eb09b）— 成功/自然/柔和
    pub jade: String,
    /// 云白（#faf8f0）— 亮色底色/宣纸
    pub cloud: String,
}

impl Default for LvpaThemeToken {
    fn default() -> Self {
        Self {
            ink: "#1c1c1f".into(),
            vermillion: "#c8102e".into(),
            gold: "#d4a843".into(),
            jade: "#7eb09b".into(),
            cloud: "#faf8f0".into(),
        }
    }
}

// ============================================================
// 1.4 RealmColor
// ============================================================

/// 境界色映射 — 7 境界 → 色值。
/// 对应 taiji_types::Realm 的 7 个境界。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum RealmColor {
    /// 炼气 — #8a8a8a
    QiRefining,
    /// 筑基 — #6d8a5e
    Foundation,
    /// 金丹 — #d4a843
    GoldenCore,
    /// 元婴 — #5e8ab5
    NascentSoul,
    /// 化神 — #b55ea8
    DivineTransformation,
    /// 炼虚 — #8a5eb5
    VoidRefining,
    /// 飞升 — #5eb5a8
    Ascension,
}

impl RealmColor {
    /// 返回对应的色值（hex）。
    pub fn hex(&self) -> &'static str {
        match self {
            Self::QiRefining => "#8a8a8a",
            Self::Foundation => "#6d8a5e",
            Self::GoldenCore => "#d4a843",
            Self::NascentSoul => "#5e8ab5",
            Self::DivineTransformation => "#b55ea8",
            Self::VoidRefining => "#8a5eb5",
            Self::Ascension => "#5eb5a8",
        }
    }
}

// ============================================================
// 1.5 SpiritRootColor
// ============================================================

/// 灵根色 — 金木水火土。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpiritRootColor {
    /// 金 — #d4a843
    Metal,
    /// 木 — #7eb09b
    Wood,
    /// 水 — #5e8ab5
    Water,
    /// 火 — #c8102e
    Fire,
    /// 土 — #a0885e
    Earth,
}

impl SpiritRootColor {
    /// 返回对应的色值（hex）。
    pub fn hex(&self) -> &'static str {
        match self {
            Self::Metal => "#d4a843",
            Self::Wood => "#7eb09b",
            Self::Water => "#5e8ab5",
            Self::Fire => "#c8102e",
            Self::Earth => "#a0885e",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== LvpaSceneId =====

    #[test]
    fn test_lvpa_scene_id_display_name() {
        assert_eq!(LvpaSceneId::Sect.display_name(), "宗门");
        assert_eq!(LvpaSceneId::Workshop.display_name(), "工坊");
        assert_eq!(LvpaSceneId::Market.display_name(), "坊市");
        assert_eq!(LvpaSceneId::Cave.display_name(), "洞府");
        assert_eq!(LvpaSceneId::Library.display_name(), "藏经阁");
        assert_eq!(LvpaSceneId::Gate.display_name(), "接引台");
    }

    #[test]
    fn test_lvpa_scene_id_css_class() {
        assert_eq!(LvpaSceneId::Sect.css_class(), "lvpa-scene-sect");
        assert_eq!(LvpaSceneId::Gate.css_class(), "lvpa-scene-gate");
    }

    #[test]
    fn test_lvpa_scene_id_serde() {
        for scene in &[LvpaSceneId::Sect, LvpaSceneId::Workshop, LvpaSceneId::Market,
                       LvpaSceneId::Cave, LvpaSceneId::Library, LvpaSceneId::Gate] {
            let json = serde_json::to_string(scene).unwrap();
            let back: LvpaSceneId = serde_json::from_str(&json).unwrap();
            assert_eq!(*scene, back);
        }
    }

    // ===== LvpaMode =====

    #[test]
    fn test_lvpa_mode_default() {
        assert_eq!(LvpaMode::default(), LvpaMode::Bitfun);
    }

    #[test]
    fn test_lvpa_mode_serde() {
        for mode in &[LvpaMode::Bitfun, LvpaMode::Taiji] {
            let json = serde_json::to_string(mode).unwrap();
            let back: LvpaMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, back);
        }
    }

    // ===== LvpaThemeToken =====

    #[test]
    fn test_lvpa_theme_token_default() {
        let token = LvpaThemeToken::default();
        assert_eq!(token.ink, "#1c1c1f");
        assert_eq!(token.cloud, "#faf8f0");
    }

    #[test]
    fn test_lvpa_theme_token_serde() {
        let token = LvpaThemeToken {
            ink: "#111".into(),
            vermillion: "#222".into(),
            gold: "#333".into(),
            jade: "#444".into(),
            cloud: "#555".into(),
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: LvpaThemeToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ink, "#111");
        assert_eq!(back.gold, "#333");
    }

    // ===== RealmColor =====

    #[test]
    fn test_realm_color_hex() {
        assert_eq!(RealmColor::QiRefining.hex(), "#8a8a8a");
        assert_eq!(RealmColor::GoldenCore.hex(), "#d4a843");
        assert_eq!(RealmColor::Ascension.hex(), "#5eb5a8");
    }

    #[test]
    fn test_realm_color_serde() {
        for c in &[RealmColor::QiRefining, RealmColor::Foundation, RealmColor::GoldenCore,
                   RealmColor::NascentSoul, RealmColor::DivineTransformation,
                   RealmColor::VoidRefining, RealmColor::Ascension] {
            let json = serde_json::to_string(c).unwrap();
            let back: RealmColor = serde_json::from_str(&json).unwrap();
            assert_eq!(*c, back);
        }
    }

    // ===== SpiritRootColor =====

    #[test]
    fn test_spirit_root_color_hex() {
        assert_eq!(SpiritRootColor::Metal.hex(), "#d4a843");
        assert_eq!(SpiritRootColor::Fire.hex(), "#c8102e");
    }

    #[test]
    fn test_spirit_root_color_serde() {
        for c in &[SpiritRootColor::Metal, SpiritRootColor::Wood, SpiritRootColor::Water,
                   SpiritRootColor::Fire, SpiritRootColor::Earth] {
            let json = serde_json::to_string(c).unwrap();
            let back: SpiritRootColor = serde_json::from_str(&json).unwrap();
            assert_eq!(*c, back);
        }
    }
}
