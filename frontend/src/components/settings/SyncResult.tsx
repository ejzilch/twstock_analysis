'use client'
/**
 * src/components/settings/SyncResult.tsx（更新版）
 *
 * 新功能：
 *   1. 失敗股票獨立顯示區塊，附「選取重新同步」快捷按鈕
 *   2. 已完成清單分頁（每頁 10 筆）
 *   3. 跳過（skipped）股票摺疊顯示
 */
import { useState } from 'react'
import { clsx } from 'clsx'
import { Pagination } from '@/src/components/ui'
import type { SymbolProgress, SyncSummary } from '@/src/types/api.types'

interface SyncResultProps {
  progress: SymbolProgress[]
  summary: SyncSummary
  onReset: () => void
  /** 點擊「重新同步失敗股票」時的回呼，傳入失敗的 symbol 代號陣列 */
  onRetryFailed?: (symbols: string[]) => void
}

const PAGE_SIZE = 10

export function SyncResult({ progress, summary, onReset, onRetryFailed }: SyncResultProps) {
  const [completedPage, setCompletedPage] = useState(0)
  const [failedPage, setFailedPage] = useState(0)
  const [skippedPage, setSkippedPage] = useState(0)
  const [showSkipped, setShowSkipped] = useState(false)

  const failedList = progress.filter((p) => p.status === 'failed')
  const completedList = progress.filter((p) => p.status === 'completed')
  const skippedList = progress.filter((p) => p.status === 'skipped')

  const hasFailures = failedList.length > 0
  const allSkipped = summary.total_inserted === 0 && summary.total_failed === 0 && summary.total_skipped > 0
  const noDataSynced = summary.total_inserted === 0 && summary.total_failed === 0 && summary.total_skipped === 0

  // ── 分頁邏輯（只對 completedList 分頁）──────────────────────────────────────
  const completedTotalPages = Math.ceil(completedList.length / PAGE_SIZE)
  const pagedCompleted = completedList.slice(completedPage * PAGE_SIZE, (completedPage + 1) * PAGE_SIZE)
  const failedTotalPages = Math.ceil(failedList.length / PAGE_SIZE)
  const pagedFailed = failedList.slice(failedPage * PAGE_SIZE, (failedPage + 1) * PAGE_SIZE)
  const skippedTotalPages = Math.ceil(skippedList.length / PAGE_SIZE)
  const pagedSkipped = skippedList.slice(skippedPage * PAGE_SIZE, (skippedPage + 1) * PAGE_SIZE)

  // ── 失敗重試 ─────────────────────────────────────────────────────────────────
  function handleRetryFailed() {
    if (!onRetryFailed || failedList.length === 0) return
    onRetryFailed(failedList.map((p) => p.symbol))
  }

  return (
    <div className={clsx(
      'border rounded-xl overflow-hidden',
      hasFailures ? 'border-red-500/30' :
        allSkipped ? 'border-slate-500/30' :
          noDataSynced ? 'border-amber-500/30' : 'border-emerald-500/30',
    )}>
      {/* ── Header ────────────────────────────────────────────────────────────── */}
      <div className={clsx(
        'flex items-center justify-between px-4 py-3 border-b',
        hasFailures ? 'bg-red-500/5 border-red-500/20' :
          allSkipped ? 'bg-slate-500/5 border-slate-500/20' :
            noDataSynced ? 'bg-amber-500/5 border-amber-500/20' :
              'bg-emerald-500/5 border-emerald-500/20',
      )}>
        <div className="flex items-center gap-2">
          <span className="text-base">
            {hasFailures ? '⚠️' : allSkipped ? 'ℹ️' : noDataSynced ? '⚠️' : '✅'}
          </span>
          <span className={clsx(
            'text-sm font-medium',
            hasFailures ? 'text-red-300' :
              allSkipped ? 'text-slate-300' :
                noDataSynced ? 'text-amber-300' : 'text-emerald-300',
          )}>
            {hasFailures
              ? `同步完成（${failedList.length} 檔失敗）`
              : allSkipped ? '無需補資料'
                : noDataSynced ? '同步完成（未取得資料）'
                  : '同步完成'}
          </span>
        </div>
      </div>

      {/* ── 失敗股票區塊 ─────────────────────────────────────────────────────── */}
      {hasFailures && (
        <div className="px-4 py-3 border-b border-red-500/15 bg-red-500/5">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-semibold text-red-400 flex items-center gap-1.5">
              <span>✗</span> 失敗股票（{failedList.length} 檔）
            </span>
            {onRetryFailed && (
              <button
                onClick={handleRetryFailed}
                className="text-xs px-2.5 py-1 rounded-lg bg-red-500/15 border border-red-500/30
                           text-red-300 hover:bg-red-500/25 hover:text-red-200 transition-all"
              >
                選取並重新同步
              </button>
            )}
          </div>

          {/* 失敗股票標籤群 */}
          <div className="flex flex-wrap gap-1.5 mt-2">
            {pagedFailed.map((p) => (
              <div
                key={p.symbol}
                className="flex items-center gap-1 px-2 py-0.5 rounded-md
                           bg-red-500/10 border border-red-500/20 text-xs"
              >
                <span className="font-mono text-red-300">{p.symbol}</span>
                {p.name && <span className="text-red-400/70">{p.name}</span>}
              </div>
            ))}
          </div>

          {/* 失敗明細（每個 symbol 各自的缺口狀態）*/}
          <div className="mt-3 space-y-1">
            {pagedFailed.map((p) => (
              <SymbolResultRow key={p.symbol} progress={p} compact />
            ))}
          </div>
          {failedTotalPages > 1 && (
            <Pagination
              page={failedPage}
              totalPages={failedTotalPages}
              onPageChange={setFailedPage}
            />
          )}
        </div>
      )}

      {/* ── 完成股票區塊（分頁）────────────────────────────────────────────── */}
      {completedList.length > 0 && (
        <div className="px-4 py-3 border-b border-surface-border">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-semibold text-emerald-400 flex items-center gap-1.5">
              <span>✓</span> 完成股票（{completedList.length} 檔）
            </span>
            {completedTotalPages > 1 && (
              <span className="text-xs text-slate-500">
                第 {completedPage + 1} / {completedTotalPages} 頁
              </span>
            )}
          </div>

          <div className="divide-y divide-surface-border">
            {pagedCompleted.map((p) => (
              <SymbolResultRow key={p.symbol} progress={p} />
            ))}
          </div>

          {/* 分頁控制 */}
          {completedTotalPages > 1 && (
            <Pagination
              page={completedPage}
              totalPages={completedTotalPages}
              onPageChange={setCompletedPage}
            />
          )}
        </div>
      )}

      {/* ── 跳過股票（摺疊）────────────────────────────────────────────────── */}
      {skippedList.length > 0 && (
        <div className="px-4 py-2.5 border-b border-surface-border">
          <button
            onClick={() => setShowSkipped((v) => !v)}
            className="flex items-center gap-2 text-xs text-slate-500 hover:text-slate-400 transition-colors w-full"
          >
            <span>{showSkipped ? '▾' : '▸'}</span>
            <span>跳過股票（{skippedList.length} 檔，資料已是最新）</span>
          </button>
          {showSkipped && (
            <div className="mt-2 flex flex-wrap gap-1.5">
              {pagedSkipped.map((p) => (
                <span
                  key={p.symbol}
                  className="px-2 py-0.5 rounded-md bg-surface border border-surface-border
                             text-xs font-mono text-slate-600"
                >
                  {p.symbol}
                </span>
              ))}
            </div>
          )}
          {showSkipped && skippedTotalPages > 1 && (
            <Pagination
              page={skippedPage}
              totalPages={skippedTotalPages}
              onPageChange={setSkippedPage}
            />
          )}
        </div>
      )}

      {/* ── Summary Footer ───────────────────────────────────────────────────── */}
      <div className="px-4 py-3 bg-surface-card/50">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4 text-xs text-slate-400 flex-wrap">
            <span>
              合計新增
              <span className="text-slate-200 font-mono font-medium ml-1">
                {summary.total_inserted.toLocaleString()}
              </span> 筆
            </span>
            {summary.total_skipped > 0 && (
              <span>
                跳過
                <span className="text-slate-400 font-mono ml-1">
                  {summary.total_skipped.toLocaleString()}
                </span> 筆
              </span>
            )}
            {summary.total_failed > 0 && (
              <span>
                失敗
                <span className="text-red-400 font-mono ml-1">
                  {summary.total_failed.toLocaleString()}
                </span> 筆
              </span>
            )}
            {noDataSynced && (
              <span className="text-amber-300">
                未新增任何資料，請檢查股票代碼、日期區間或 FinMind Token
              </span>
            )}
          </div>
          <button
            onClick={onReset}
            className="text-xs text-brand-400 hover:text-brand-300 transition-colors shrink-0 ml-4"
          >
            完成並返回
          </button>
        </div>
      </div>
    </div>
  )
}

// ── 單一股票結果列 ────────────────────────────────────────────────────────────

function SymbolResultRow({
  progress,
  compact = false,
}: {
  progress: SymbolProgress
  compact?: boolean
}) {
  const label = progress.name
    ? `${progress.symbol} ${progress.name}`
    : progress.symbol

  const gapADone = progress.gap_a?.completed
  const gapBDone = progress.gap_b?.completed
  const gapAInserted = progress.gap_a?.inserted ?? 0
  const gapBInserted = progress.gap_b?.inserted ?? 0
  const gapASkipped = progress.gap_a?.skipped ?? 0
  const gapBSkipped = progress.gap_b?.skipped ?? 0

  if (compact) {
    // 失敗區塊：精簡版（只顯示 symbol + 哪段失敗）
    return (
      <div className="flex items-center gap-2 text-xs text-slate-500 py-0.5">
        <span className="font-mono text-red-400 w-12 shrink-0">{progress.symbol}</span>
        <span className="text-slate-600">
          {[
            progress.gap_a && !gapADone && '歷史段',
            progress.gap_b && !gapBDone && '近期段',
          ]
            .filter(Boolean)
            .join(' + ') || '無資料回傳'}
          {' '}失敗
        </span>
      </div>
    )
  }

  return (
    <div className="py-2.5 flex flex-col gap-1">
      <span className="text-xs font-medium text-slate-300">{label}</span>

      {/* Gap A */}
      {progress.gap_a ? (
        <div className="flex items-center gap-3 text-xs text-slate-500 pl-2">
          <span className="w-20 shrink-0">歷史段</span>
          {gapADone ? (
            <>
              <span>新增 <span className="text-slate-300">{gapAInserted.toLocaleString()}</span> 筆</span>
              {gapASkipped > 0 && <span>跳過 {gapASkipped.toLocaleString()} 筆</span>}
            </>
          ) : (
            <span className="text-red-400">失敗</span>
          )}
        </div>
      ) : (
        <div className="flex items-center gap-3 text-xs text-slate-600 pl-2">
          <span className="w-20 shrink-0">歷史段</span>
          <span>已是最早資料，跳過</span>
        </div>
      )}

      {/* Gap B */}
      {progress.gap_b ? (
        <div className="flex items-center gap-3 text-xs text-slate-500 pl-2">
          <span className="w-20 shrink-0">近期段</span>
          {gapBDone ? (
            <>
              <span>新增 <span className="text-slate-300">{gapBInserted.toLocaleString()}</span> 筆</span>
              {gapBSkipped > 0 && <span>跳過 {gapBSkipped.toLocaleString()} 筆</span>}
            </>
          ) : (
            <span className="text-red-400">失敗</span>
          )}
        </div>
      ) : (
        <div className="flex items-center gap-3 text-xs text-slate-600 pl-2">
          <span className="w-20 shrink-0">近期段</span>
          <span>已是最新資料，跳過</span>
        </div>
      )}
    </div>
  )
}