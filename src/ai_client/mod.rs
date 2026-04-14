/// Python AI Service HTTP 客戶端模組
///
/// client:        AiServiceClient，封裝 BridgeError 轉換與序列化格式選擇
/// serialization: JSON / MsgPack 自動選擇，對應 ARCH_DESIGN.md 序列化格式策略
pub mod client;
pub mod serialization;

pub use client::{AiServiceClient, PredictRequest, PredictResponse};
pub use serialization::SerializationFormat;
