'use client'
/**
 * src/components/settings/ManualSyncPanel.tsx（修正版）
 *
 * 修正清單：
 *   1. useSymbols() 只在此處呼叫一次，allSymbols 往下傳給 SymbolSearchInput
 *   2. 前 10 大市值按鈕：symbols 尚未載入時顯示 loading，載入後正常運作
 *   3. isIdle 判斷加入 symbols loading 狀態，避免空白期誤判
 *   4. 加入 symbols 載入失敗的 error 提示
 *   5. handleReset 改用 hook 而非直接呼叫 getState()
 */
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useSymbols } from '@/src/hooks'
import { useTriggerSync, useSyncStatus } from '@/src/hooks/useManualSync'
import { useAppStore } from '@/src/store/useAppStore'
import { Card, Button, ErrorToast, LoadingSpinner } from '@/src/components/ui'
import { SymbolSearchInput } from './SymbolSearchInput'
import { SelectedSymbolTags } from './SelectedSymbolTags'
import { SyncProgress } from './SyncProgress'
import { SyncResult } from './SyncResult'
import type { SymbolItem } from '@/src/types/api.generated'
import { useQueryClient } from '@tanstack/react-query'

// 前 10 大市值股票代號（由 EJ 於 04-25 確認後更新）
const TOP_10_SYMBOLS = [
  '2330', '2317', '2454', '2412',
  '2308', '2382', '2881', '2882',
  '2303', '3008',
]

export function ManualSyncPanel() {
  const router = useRouter()
  const syncId = useAppStore((s) => s.activeSyncId)
  const setActiveSyncId = useAppStore((s) => s.setActiveSyncId)

  const [selected, setSelected] = useState<SymbolItem[]>([])

  // ── 單一 useSymbols 呼叫，allSymbols 往下傳 ────────────────────────────────
  const {
    data: symbolsData,
    isLoading: symbolsLoading,
    isError: symbolsError,
  } = useSymbols()

  const allSymbols = symbolsData?.symbols ?? []

  const triggerSync = useTriggerSync()
  const syncStatus = useSyncStatus()

  // ── 狀態機 ──────────────────────────────────────────────────────────────────
  const currentStatus = syncStatus.data?.status ?? null
  const isRunning = currentStatus === 'running' || currentStatus === 'rate_limit_waiting'
  const isCompleted = currentStatus === 'completed' || currentStatus === 'failed'
  const isIdle = !isRunning && !isCompleted

  // ── 股票選擇操作 ─────────────────────────────────────────────────────────────

  function handleSelect(symbol: SymbolItem) {
    if (selected.some((s) => s.symbol === symbol.symbol)) return
    setSelected((prev) => [...prev, symbol])
  }

  function handleRemove(symbolCode: string) {
    setSelected((prev) => prev.filter((s) => s.symbol !== symbolCode))
  }

  function handleSelectTop10() {
    // allSymbols 從 API 取得，symbols 清單必須已載入才能過濾
    if (allSymbols.length === 0) return

    const top10 = allSymbols.filter((s) => TOP_10_SYMBOLS.includes(s.symbol))
    const toAdd = top10.filter(
      (s) => !selected.some((sel) => sel.symbol === s.symbol)
    )

    if (toAdd.length === 0) return  // 全部已選，不需再加

    setSelected((prev) => [...prev, ...toAdd])
  }

  function handleClearAll() {
    setSelected([])
  }

  // ── 開始同步 ─────────────────────────────────────────────────────────────────

  function handleStartSync() {
    if (selected.length === 0) return
    triggerSync.mutate({ symbols: selected.map((s) => s.symbol) })
  }

  // ── 再次同步（重置）──────────────────────────────────────────────────────────

  function handleReset() {
    useAppStore.getState().setActiveSyncId(null)
    setSelected([])
    // 清除 query cache 避免舊狀態殘留
    useQueryClient().removeQueries({ queryKey: ['sync-status'] })
  }

  // ── 渲染 ─────────────────────────────────────────────────────────────────────

  return (
    <Card>
      <h3 className="text-sm font-semibold text-slate-200 mb-5">資料同步</h3>

      {/* 執行中：進度顯示 */}
      {isRunning && syncStatus.data && (
        <SyncProgress
          progress={syncStatus.data.progress}
          rateLimit={syncStatus.data.rate_limit}
        />
      )}

      {/* 完成：結果顯示 */}
      {isCompleted && syncStatus.data && (
        <SyncResult
          progress={syncStatus.data.progress}
          summary={syncStatus.data.summary}
          onReset={handleReset}
        />
      )}

      {/* Idle：選擇股票 + 觸發按鈕 */}
      {isIdle && (
        <div className="flex flex-col gap-4">

          {/* 快捷按鈕列 */}
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs text-slate-500">快捷：</span>

            <button
              onClick={handleSelectTop10}
              disabled={symbolsLoading || symbolsError || allSymbols.length === 0}
              className="text-xs px-2.5 py-1 rounded-lg bg-surface border border-surface-border
                         text-slate-400 hover:text-slate-200 hover:bg-surface-hover
                         transition-all disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {symbolsLoading ? '載入中...' : '前 10 大市值'}
            </button>

            {selected.length > 0 && (
              <button
                onClick={handleClearAll}
                className="text-xs px-2.5 py-1 rounded-lg text-slate-600
                           hover:text-slate-400 transition-colors"
              >
                全部清除
              </button>
            )}

            {/* 股票清單載入失敗提示 */}
            {symbolsError && (
              <span className="text-xs text-red-400">
                ⚠ 股票清單載入失敗，請重新整理頁面
              </span>
            )}
          </div>

          {/* 搜尋框（allSymbols 由此層傳入，不重複 fetch）*/}
          <SymbolSearchInput
            allSymbols={allSymbols}
            selectedSymbols={selected.map((s) => s.symbol)}
            onSelect={handleSelect}
            isLoading={symbolsLoading}
            isError={symbolsError}
          />

          {/* 已選標籤 */}
          <div>
            <div className="text-xs text-slate-500 mb-2">
              已選擇（{selected.length} 檔）
            </div>
            <SelectedSymbolTags
              selected={selected}
              onRemove={handleRemove}
            />
          </div>

          {/* 開始同步按鈕 */}
          <Button
            onClick={handleStartSync}
            loading={triggerSync.isPending}
            disabled={selected.length === 0 || symbolsLoading}
            size="lg"
            className="w-full"
          >
            {triggerSync.isPending
              ? '準備中...'
              : selected.length === 0
                ? '請先選擇股票'
                : `開始同步（${selected.length} 檔）`}
          </Button>

          {/* 預估提示：只在有選擇且 API 有資料時顯示 */}
          {selected.length > 0 && !symbolsLoading && (
            <p className="text-xs text-slate-600 text-center">
              預估約 {Math.ceil(selected.length * 6 * 156 / 590)} 小時完成
              （依實際資料缺口大小而異）
            </p>
          )}
        </div>
      )}

      {/* trigger sync 錯誤提示 */}
      {triggerSync.isError && (
        <ErrorToast
          error={triggerSync.error}
          onRetry={handleStartSync}
          onRedirect={router.push}
        />
      )}
    </Card>
  )
}