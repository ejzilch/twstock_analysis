'use client'
/**
 * src/components/settings/SyncResult.tsx
 *
 * 同步完成後的結果顯示。
 * 純 props，無任何 API 或 store 依賴。
 */
import { clsx } from 'clsx'
import type { SymbolProgress, SyncSummary } from '@/src/types/api.generated'

interface SyncResultProps {
  progress: SymbolProgress[]
  summary: SyncSummary
  onReset: () => void
}

export function SyncResult({ progress, summary, onReset }: SyncResultProps) {
  const hasFailures = summary.total_failed > 0
  const allSkipped = summary.total_inserted === 0 && summary.total_failed === 0 && summary.total_skipped > 0
  const noDataSynced = summary.total_inserted === 0 && summary.total_failed === 0 && summary.total_skipped === 0

  return (
    <div className={clsx(
      'border rounded-xl overflow-hidden',
      hasFailures ? 'border-red-500/30' :
        allSkipped ? 'border-slate-500/30' :
          noDataSynced ? 'border-amber-500/30' : 'border-emerald-500/30',
    )}>
      {/* Header */}
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
              ? '同步完成（含失敗）'
              : allSkipped
                ? '無需補資料'
                : noDataSynced
                  ? '同步完成（未取得資料）'
                  : '同步完成'}
          </span>
        </div>
      </div>

      {/* Per-symbol result */}
      <div className="px-4 py-3 flex flex-col divide-y divide-surface-border">
        {progress.map((p) => (
          <SymbolResultRow key={p.symbol} progress={p} />
        ))}
      </div>

      {/* Summary footer */}
      <div className="px-4 py-3 bg-surface-card/50 border-t border-surface-border">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4 text-xs text-slate-400">
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
            className="text-xs text-brand-400 hover:text-brand-300 transition-colors"
          >
            完成並返回
          </button>
        </div>
      </div>
    </div >
  )
}

// ── 單一股票結果列 ────────────────────────────────────────────────────────────

function SymbolResultRow({ progress }: { progress: SymbolProgress }) {
  const label = progress.name
    ? `${progress.symbol} ${progress.name}`
    : progress.symbol

  const gapADone = progress.gap_a?.completed
  const gapBDone = progress.gap_b?.completed
  const gapAInserted = progress.gap_a?.inserted ?? 0
  const gapBInserted = progress.gap_b?.inserted ?? 0
  const gapASkipped = progress.gap_a?.skipped ?? 0
  const gapBSkipped = progress.gap_b?.skipped ?? 0

  return (
    <div className="py-3 flex flex-col gap-1.5">
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
