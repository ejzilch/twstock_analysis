/// API 層的請求與回應資料結構
///
/// request:  查詢參數與 POST body 的反序列化結構
/// response: 對外回傳的序列化結構，從 domain model 轉換而來
pub mod enums;
pub mod request;
pub mod response;

pub use enums::*;
pub use response::ErrorResponse;
