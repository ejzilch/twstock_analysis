'use client'
/**
 * src/components/settings/SyncProgress.tsx
 *
 * 同步執行中的進度顯示元件。
 * 顯示 API 使用量、各股票進度、rate limit 等待狀態。
 */
import { useState, useEffect } from 'react'
import { clsx } from 'clsx'
import type { RateLimitInfo, SymbolProgress } from '@/src/types/api.generated'

interface SyncProgressProps {
  progress: SymbolProgress[]
  rateLimit: RateLimitInfo
}

export function SyncProgress({ progress, rateLimit }: SyncProgressProps) {
  const [collapsed, setCollapsed] = useState(false)

  const usedPct = Math.round((rateLimit.used_this_hour / rateLimit.limit_per_hour) * 100)

  return (
    <div className="bg-surface border border-surface-border rounded-xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-surface-border">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-brand-400 animate-pulse" />
          <span className="text-sm font-medium text-slate-200">同步執行中</span>
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
              <span className="text-slate-400">API 使用量</span>
              <span className={clsx(
                'font-mono',
                usedPct >= 90 ? 'text-red-400' :
                  usedPct >= 70 ? 'text-amber-400' : 'text-slate-300',
              )}>
                {rateLimit.used_this_hour} / {rateLimit.limit_per_hour} 次
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
            <RateLimitWaitBanner resumeAtMs={rateLimit.resume_at_ms} />
          )}

          {/* Per-symbol progress */}
          <div className="flex flex-col gap-3">
            {progress.map((p) => (
              <SymbolProgressRow key={p.symbol} progress={p} />
            ))}
          </div>

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

function RateLimitWaitBanner({ resumeAtMs }: { resumeAtMs: number }) {
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
        <div className="font-medium">達到 FinMind 每小時上限（590 / 600 次）</div>
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

function GapProgressBar({ label, gap }: { label: string; gap: import('@/src/types/api.generated').GapProgress }) {
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

function StatusPill({ status }: { status: import('@/src/types/api.generated').SymbolSyncStatus }) {
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
