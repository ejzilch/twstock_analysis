/// 系統核心領域模型（Domain Models）
///
/// 子模組職責：
/// - candle:      Candle（內部 K 線）、IndicatorValue、MacdValue
/// - indicators:  ComputeIndicatorsRequest、ComputeIndicatorsResponse、IndicatorConfig
/// - symbol:      SymbolMeta
///
/// 注意：此層為純領域模型，禁止引入 HTTP、DB、外部 API 相關依賴。
pub mod candle;
pub mod enums;
pub mod indicators;
pub mod symbol;

pub use enums::*;
// 重新匯出核心領域模型，方便其他模組以 crate::models::Candle 引用
pub use candle::{Candle, IndicatorValue, MacdValue};
pub use indicators::{
    BollingerConfig, ComputeIndicatorsRequest, ComputeIndicatorsResponse, IndicatorConfig,
};
pub use symbol::SymbolMeta;
