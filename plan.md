# Dashboard UI 改版追蹤計畫（plan.md）

## 1. 目標與範圍
- 目標：針對本次 Dashboard UI 調整建立可持續追蹤的實作計畫，並以 Phase 1/2/3 管理進度。
- 範圍：只包含本次前端 UI 改版相關工作，不延伸到 `docs/` 既有文件任務，也不包含 Rust 後端改動。
- 原則：以既有 `layout state 型別 + persist schema` 為基線，後續 phase 直接接上，不重複設計。

## 2. 已完成基線（State / Schema）
- [x] 已定稿 `DashboardLayoutState` / `DashboardRightWidgetLayout`（`frontend/src/types/api.types.ts`）。
- [x] 已定稿 `AppStorePersistedStateV2`。
- [x] 已定稿 `persist version = 2` 與 `migrate/sanitize` 機制。
- [x] 已提供 `useAppStore` 的 dashboard actions（split/range/left/right widget controls）（`frontend/src/store/useAppStore.ts`）。
- [x] 基線結論：後續只做 UI 行為與互動接線，不再重做 state schema 決策。

## 3. Phase 1 Checklist（骨架 + splitter + stat-row，無拖曳）
- 狀態：`Completed`
- [x] Sidebar 分組完成：`TW Stocks analysis`、`設定`。
- [x] Dashboard 主內容改為三段：`topbar` / `stat-row` / `split-container`。
- [x] `split-container` 完成左右分欄，加入可拖拉 splitter，左右寬度即時更新。
- [x] `stat-row` 依 indicator visibility 規則顯示資料（含開關啟用時才顯示的欄位）。
- [x] 明確不實作任何拖曳排序（left/right 皆不做拖曳）。
- [ ] 手機版/窄螢幕有可用的降級排版（不破版）。

## 4. Phase 2 Checklist（col-left 拖曳排序 + 顯示控制）
- 狀態：`Completed`
- [x] `col-left` 元件 registry 化：`candles` / `rsi` / `macd` / `institutionalNetFlow`。
- [x] 支援垂直拖曳排序（僅 left 區）。
- [x] `col-left` header 提供顯示/隱藏控制。
- [x] 行為正確映射到 `leftPanelOrder` / `leftPanelVisible`。
- [x] 順序與顯示設定可在重新整理後維持。

## 5. Phase 3 Checklist（col-right grid 拖曳縮放 + 版面持久化）
- 狀態：`In Progress`
- [x] `col-right` 支援 grid preset：`1x1` / `2x2` / `3x3` / `4x4`。
- [x] 右區元件支援拖曳、縮放、顯示/隱藏。
- [x] 行為正確映射到 `rightGridPreset` / `rightWidgets`。
- [x] 元件不超界（位置/尺寸受欄數與最小尺寸限制）。
- [ ] 完整持久化驗證：reload 後還原配置。

## 6. 驗收條件 + 風險 + 變更紀錄模板
### 驗收條件（每個 Phase 都要勾選）
- [ ] 視覺驗收：桌機/行動版顯示正常。
- [ ] 互動驗收：splitter / 拖曳 / 顯示切換符合預期。
- [ ] 狀態驗收：`useAppStore` 對應欄位正確更新。
- [ ] 持久化驗收：重新整理後設定可恢復。

### 最低驗收案例
- [ ] Phase 1：splitter 連動寬度、`stat-row` 數值與顯示規則正確。
- [ ] Phase 2：left 順序改變立即反映，reload 後維持。
- [ ] Phase 3：right grid 拖曳縮放不超界，preset 切換合理，reload 後維持。

### 主要風險
- 互動衝突風險：splitter 拖拉與圖表拖拉事件可能互相干擾。
- 持久化一致性風險：舊資料或非法資料造成 UI 異常，需要持續依賴 sanitize。
- 響應式風險：桌機優先實作後，行動版可能出現操作可用性下降。

### 變更紀錄模板
| 日期 | Phase | 變更內容 | 影響範圍 | 負責人 | 備註 |
|---|---|---|---|---|---|
| YYYY-MM-DD | Phase 1/2/3 | 變更摘要 | 例如：dashboard/layout/store |  |  |

## 實作順序（固定）
- `Phase 1 -> Phase 2 -> Phase 3`
- 不平行開發，以降低回歸風險與整合衝突。
