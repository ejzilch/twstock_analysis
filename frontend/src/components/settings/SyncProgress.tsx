'use client'
/**
 * src/components/settings/SyncProgress.tsx
 *
 * 同步執行中的進度顯示元件。
 * 已完成（completed / failed / skipped）的 symbol 從列表移除，
 * header 顯示「剩餘 N / 共 M 檔」。
 */
import { useState, useEffect } from 'react'
import { clsx } from 'clsx'
import { Pagination } from '@/src/components/ui'
import type { RateLimitInfo, SymbolProgress, GapProgress, SymbolSyncStatus } from '@/src/types/api.types'

interface SyncProgressProps {
  progress: SymbolProgress[]
  rateLimit: RateLimitInfo
}

const DONE_STATUSES: SymbolSyncStatus[] = ['completed', 'failed', 'skipped']

export function SyncProgress({ progress, rateLimit }: SyncProgressProps) {
  const [collapsed, setCollapsed] = useState(false)
  const [progressPage, setProgressPage] = useState(0)
  const PROGRESS_PAGE_SIZE = 5

  const usedPct = Math.round((rateLimit.used_this_hour / rateLimit.limit_per_hour) * 100)
  const remaining = Math.max(rateLimit.used_this_hour, 0)

  const total = progress.length
  const done = progress.filter((p) => DONE_STATUSES.includes(p.status))
  const remaining_symbols = progress.filter((p) => !DONE_STATUSES.includes(p.status))
  const doneCount = done.length
  const remainingCount = remaining_symbols.length

  const totalProgressPages = Math.ceil(remaining_symbols.length / PROGRESS_PAGE_SIZE)
  const pagedSymbols = remaining_symbols.slice(
    progressPage * PROGRESS_PAGE_SIZE,
    (progressPage + 1) * PROGRESS_PAGE_SIZE,
  )

  useEffect(() => {
    const totalPages = Math.ceil(remainingCount / PROGRESS_PAGE_SIZE)
    if (progressPage >= totalPages && totalPages > 0) {
      setProgressPage(totalPages - 1)
    }
  }, [remainingCount])

  return (
    <div className="bg-surface border border-surface-border rounded-xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-surface-border">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-brand-400 animate-pulse" />
          <span className="text-sm font-medium text-slate-200">同步執行中</span>
          {total > 0 && (
            <span className="text-xs text-slate-500">
              — 已完成 <span className="text-slate-300 font-mono">{doneCount}</span>
              {' '}/ 共 <span className="text-slate-300 font-mono">{total}</span> 檔
              {remainingCount > 0 && (
                <>，剩餘 <span className="text-brand-300 font-mono">{remainingCount}</span> 檔</>
              )}
            </span>
          )}
        </div>
        <button
          onClick={() => setCollapsed((c) => !c)}
          className="text-xs text-slate-500 hover:text-slate-300 transition-colors"
        >
          {collapsed ? '展開' : '收合'}
        </button>
      </div>

      {!collapsed && (
        <div className="px-4 py-4 flex flex-col gap-4">
          {/* Rate limit bar */}
          <div>
            <div className="flex items-center justify-between text-xs mb-1.5">
              <span className="text-slate-400">API 剩餘次數</span>
              <span className={clsx(
                'font-mono',
                usedPct >= 90 ? 'text-red-400' :
                  usedPct >= 70 ? 'text-amber-400' : 'text-slate-300',
              )}>
                {remaining} / {rateLimit.limit_per_hour} 次
              </span>
            </div>
            <div className="h-2 bg-surface-border rounded-full overflow-hidden">
              <div
                className={clsx(
                  'h-full rounded-full transition-all duration-500',
                  usedPct >= 90 ? 'bg-red-500' :
                    usedPct >= 70 ? 'bg-amber-500' : 'bg-brand-500',
                )}
                style={{ width: `${Math.min(usedPct, 100)}%` }}
              />
            </div>
          </div>

          {/* Rate limit waiting banner */}
          {rateLimit.is_waiting && rateLimit.resume_at_ms && (
            <RateLimitWaitBanner
              resumeAtMs={rateLimit.resume_at_ms}
              usedThisHour={rateLimit.used_this_hour}
              limitPerHour={rateLimit.limit_per_hour}
            />
          )}

          {/* 已完成摘要（只顯示計數，不展開明細） */}
          {doneCount > 0 && (
            <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-emerald-500/5 border border-emerald-500/15 text-xs text-slate-500">
              <span className="text-emerald-400">✓</span>
              已完成 {doneCount} 檔，明細將於同步結束後顯示
            </div>
          )}

          {/* 進行中 / 等待中的 symbol 列表 */}
          {remainingCount > 0 ? (
            <div className="flex flex-col gap-3">
              {pagedSymbols.map((p) => (
                <SymbolProgressRow key={p.symbol} progress={p} />
              ))}
              {totalProgressPages > 1 && (
                <Pagination
                  page={progressPage}
                  totalPages={totalProgressPages}
                  onPageChange={setProgressPage}
                />
              )}
            </div>
          ) : (
            <p className="text-xs text-slate-600 text-center py-2">
              所有股票均已處理完成，等待最終狀態確認…
            </p>
          )}

          {/* Warning */}
          {!rateLimit.is_waiting && (
            <p className="text-xs text-amber-500/80 flex items-center gap-1.5">
              <span>⚠</span>
              Rate limit 後自動繼續，請勿關閉視窗
            </p>
          )}
        </div>
      )}
    </div>
  )
}

// ── Rate limit 等待倒數 ───────────────────────────────────────────────────────

function RateLimitWaitBanner({
  resumeAtMs,
  usedThisHour,
  limitPerHour,
}: {
  resumeAtMs: number
  usedThisHour: number
  limitPerHour: number
}) {
  const [remaining, setRemaining] = useState(calcRemaining(resumeAtMs))

  useEffect(() => {
    const timer = setInterval(() => {
      setRemaining(calcRemaining(resumeAtMs))
    }, 1000)
    return () => clearInterval(timer)
  }, [resumeAtMs])

  return (
    <div className="flex items-center gap-2 px-3 py-2.5 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-400">
      <span className="text-base">⏳</span>
      <div>
        <div className="font-medium">達到 FinMind 每小時上限（{usedThisHour.toLocaleString()} / {limitPerHour.toLocaleString()} 次）</div>
        <div className="opacity-80 mt-0.5">
          將於 <span className="font-mono font-medium">{remaining}</span> 後自動繼續，無需任何操作
        </div>
      </div>
    </div>
  )
}

function calcRemaining(resumeAtMs: number): string {
  const diff = Math.max(0, resumeAtMs - Date.now())
  const mins = Math.floor(diff / 60_000)
  const secs = Math.floor((diff % 60_000) / 1000)
  return `${mins} 分 ${String(secs).padStart(2, '0')} 秒`
}

// ── 單一股票進度列 ────────────────────────────────────────────────────────────

function SymbolProgressRow({ progress }: { progress: SymbolProgress }) {
  const label = progress.name
    ? `${progress.symbol} ${progress.name}`
    : progress.symbol

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-slate-300">{label}</span>
        <StatusPill status={progress.status} />
      </div>

      {progress.status === 'pending' && (
        <p className="text-xs text-slate-600">等待中...</p>
      )}

      {progress.gap_a && (
        <GapProgressBar label="歷史段（缺口 A）" gap={progress.gap_a} />
      )}
      {progress.gap_b && (
        <GapProgressBar label="近期段（缺口 B）" gap={progress.gap_b} />
      )}
    </div>
  )
}

function GapProgressBar({ label, gap }: { label: string; gap: GapProgress }) {
  const from = new Date(gap.from_ms).toLocaleDateString('zh-TW')
  const to = new Date(gap.to_ms).toLocaleDateString('zh-TW')

  if (gap.completed) {
    return (
      <div className="flex items-center gap-2 text-xs">
        <span className="text-slate-500 w-28 shrink-0">{label}</span>
        <span className="text-emerald-400">完成 ✅</span>
        <span className="text-slate-600">+{gap.inserted.toLocaleString()} 筆</span>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-center justify-between text-xs">
        <span className="text-slate-500">{label}</span>
        <span className="text-slate-600 font-mono">{from} ～ {to}</span>
      </div>
      <div className="h-1.5 bg-surface-border rounded-full overflow-hidden">
        <div className="h-full bg-brand-500 rounded-full animate-pulse-slow w-1/3" />
      </div>
    </div>
  )
}

function StatusPill({ status }: { status: SymbolSyncStatus }) {
  const config = {
    pending: { label: '等待中', className: 'bg-slate-500/15 text-slate-400' },
    running: { label: '執行中', className: 'bg-brand-500/15 text-brand-300' },
    completed: { label: '完成', className: 'bg-emerald-500/15 text-emerald-400' },
    failed: { label: '失敗', className: 'bg-red-500/15 text-red-400' },
    skipped: { label: '跳過', className: 'bg-slate-500/15 text-slate-500' },
  }[status]

  return (
    <span className={clsx('text-xs px-2 py-0.5 rounded-full', config.className)}>
      {config.label}
    </span>
  )
}