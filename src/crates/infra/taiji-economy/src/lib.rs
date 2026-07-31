//! taiji-economy — LVPA 经济系统核心 crate。
//!
//! 七个子系统：Token（灵力）/Stone（灵石）/Exchange（兑换）/Subsidy（补贴）/Market（坊市）/RealmGate（飞升）/Bankruptcy（入不敷出）

pub mod error;
pub mod token;
pub mod stone;
pub mod exchange;
pub mod subsidy;
pub mod market;
pub mod realm_gate;
pub mod repository;
pub mod bankruptcy;

pub use error::EconomyError;
pub use token::{TokenManager, InMemoryTokenManager};
pub use stone::{StoneManager, InMemoryStoneManager};
pub use exchange::{ExchangeService, ExchangeServiceImpl, ExchangeRate};
pub use subsidy::{SubsidyService, SubsidyServiceImpl, SubsidyConfig};
pub use market::{MarketService, MarketServiceImpl, MarketListing, MarketFilter, MarketSortBy, RoyaltyRecord};
pub use realm_gate::{RealmGateService, RealmGateServiceImpl, DomainId, RealmDomainMapping};
pub use repository::{EconomyRepository, InMemoryEconomyRepository};
