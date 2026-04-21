/// src/api/router.rs（新增段落）
///
/// 本檔案為現有 router.rs 的補充，
/// 將以下路由合併進現有 router 建立函數。
///
/// 在現有 Router 建立處加入：
///
/// ```rust
/// use crate::api::handlers::admin_sync::{
///     trigger_manual_sync,
///     get_sync_status,
///     get_sync_status_by_id,
/// };
///
/// // 加入現有的 Router::new() 鏈中：
/// .route("/api/v1/admin/sync",           post(trigger_manual_sync))
/// .route("/api/v1/admin/sync/status",    get(get_sync_status))
/// .route("/api/v1/admin/sync/status/:sync_id", get(get_sync_status_by_id))
/// ```
///
/// AppState 需新增 AdminSyncAppState 的欄位：
///
/// ```rust
/// pub struct AppState {
///     // ...現有欄位...
///     pub admin_sync: Arc<AdminSyncAppState>,
/// }
/// ```
///
/// main.rs 初始化時：
///
/// ```rust
/// let admin_sync_state = Arc::new(AdminSyncAppState {
///     db_pool:      db_pool.clone(),
///     http_client:  http_client.clone(),
///     rate_limiter: rate_limiter.clone(),   // 與排程共用同一個實例
///     redis:        Arc::new(Mutex::new(redis_conn)),
/// });
/// ```

// 本檔案僅為文件說明，實際程式碼請合併進 src/api/router.rs
