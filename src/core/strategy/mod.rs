/// 交易信號策略模組
///
/// signal_aggregator: 整合指標結果與 AI 預測，產生 TradeSignalResponse。
/// 目前以 oneshot channel 回傳給 REST handler，
/// 預留 broadcast channel 介面供日後 WebSocket 擴充使用。
pub mod signal_aggregator;
pub mod traits;
