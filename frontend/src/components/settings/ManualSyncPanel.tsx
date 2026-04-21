'use client'
/**
 * src/components/settings/ManualSyncPanel.tsx
 *
 * 手動同步主面板，組合所有子元件。
 *
 * 狀態機：
 *   idle → running / rate_limit_waiting → completed / failed → idle
 */
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { clsx } from 'clsx'
import { useSymbols } from '@/src/hooks'
import { useTriggerSync, useSyncStatus } from '@/src/hooks/useManualSync'
import { useAppStore } from '@/src/store/useAppStore'
import { Card, Button, ErrorToast } from '@/src/components/ui'
import { SymbolSearchInput } from './SymbolSearchInput'
import { SelectedSymbolTags } from './SelectedSymbolTags'
import { SyncProgress } from './SyncProgress'
import { SyncResult } from './SyncResult'
import type { SymbolItem } from '@/src/types/api.generated'

// 前 10 大市值股票（固定清單，由 EJ 於 04-25 審核後更新）
const TOP_10_SYMBOLS = [
  '2330', '2317', '2454', '2412',
  '2308', '2382', '2881', '2882',
  '2303', '3008',
]

export function ManualSyncPanel() {
  const router = useRouter()
  const syncId = useAppStore((s) => s.activeSyncId)

  const [selected, setSelected] = useState<SymbolItem[]>([])

  const { data: symbolsData } = useSymbols()
  const triggerSync = useTriggerSync()
  const syncStatus = useSyncStatus()

  // 目前的同步狀態
  const currentStatus = syncStatus.data?.status ?? null
  const isRunning = currentStatus === 'running' || currentStatus === 'rate_limit_waiting'
  const isCompleted = currentStatus === 'completed' || currentStatus === 'failed'
  const isIdle = !syncId && !isRunning && !isCompleted

  // ── 股票選擇操作 ────────────────────────────────────────────────────────────

  function handleSelect(symbol: SymbolItem) {
    if (selected.some((s) => s.symbol === symbol.symbol)) return
    setSelected((prev) => [...prev, symbol])
  }

  function handleRemove(symbolCode: string) {
    setSelected((prev) => prev.filter((s) => s.symbol !== symbolCode))
  }

  function handleSelectTop10() {
    const allSymbols = symbolsData?.symbols ?? []
    const top10 = allSymbols.filter((s) => TOP_10_SYMBOLS.includes(s.symbol))
    // 不重複加入已選的
    const toAdd = top10.filter((s) => !selected.some((sel) => sel.symbol === s.symbol))
    setSelected((prev) => [...prev, ...toAdd])
  }

  function handleClearAll() {
    setSelected([])
  }

  // ── 開始同步 ────────────────────────────────────────────────────────────────

  function handleStartSync() {
    if (selected.length === 0) return
    triggerSync.mutate({ symbols: selected.map((s) => s.symbol) })
  }

  // ── 再次同步（重置狀態）────────────────────────────────────────────────────

  function handleReset() {
    useAppStore.getState().setActiveSyncId(null)
    setSelected([])
  }

  // ── 渲染 ────────────────────────────────────────────────────────────────────

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
          {/* 快捷按鈕 */}
          <div className="flex items-center gap-2">
            <span className="text-xs text-slate-500">快捷：</span>
            <button
              onClick={handleSelectTop10}
              className="text-xs px-2.5 py-1 rounded-lg bg-surface border border-surface-border text-slate-400 hover:text-slate-200 hover:bg-surface-hover transition-all"
            >
              前 10 大市值
            </button>
            {selected.length > 0 && (
              <button
                onClick={handleClearAll}
                className="text-xs px-2.5 py-1 rounded-lg text-slate-600 hover:text-slate-400 transition-colors"
              >
                全部清除
              </button>
            )}
          </div>

          {/* 搜尋框 */}
          <SymbolSearchInput
            selectedSymbols={selected.map((s) => s.symbol)}
            onSelect={handleSelect}
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
            disabled={selected.length === 0}
            size="lg"
            className="w-full"
          >
            {triggerSync.isPending ? '準備中...' : '開始同步'}
          </Button>

          {/* 預估提示 */}
          {selected.length > 0 && (
            <p className="text-xs text-slate-600 text-center">
              預估約 {Math.ceil(selected.length * 6 * 156 / 590)} 小時完成
              （依實際缺口大小而異）
            </p>
          )}
        </div>
      )}

      {/* 錯誤提示 */}
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
