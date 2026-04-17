'use client'
/**
 * Dashboard business components.
 * May depend on hooks/ and store/, but ui/ and charts/ remain pure.
 */
import { clsx } from 'clsx'
import type { SymbolItem, SignalItem, Interval } from '@/types/api.generated'
import { ReliabilityBadge, Card, Select } from '@/components/ui'
import { formatPrice, formatPercent, formatTimestamp } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'
import { useSymbols } from '@/hooks'
import { useSignalTheme } from '@/src/hooks/useSignalTheme'

// ── SymbolSelector ────────────────────────────────────────────────────────────

export function SymbolSelector() {
    const { data, isLoading } = useSymbols()
    const selected = useAppStore((s) => s.selectedSymbol)
    const setSelected = useAppStore((s) => s.setSelectedSymbol)

    if (isLoading) return (
        <div className="h-9 w-40 bg-surface-card border border-surface-border rounded-lg animate-pulse" />
    )

    const options = (data?.symbols ?? []).map((s: SymbolItem) => ({
        value: s.symbol,
        label: `${s.symbol} ${s.name}`,
    }))

    return (
        <select
            value={selected}
            onChange={(e) => setSelected(e.target.value)}
            className="bg-surface-card border border-surface-border rounded-lg px-3 py-2 text-sm text-slate-200 focus:outline-none focus:ring-2 focus:ring-brand-500/50 min-w-[160px]"
        >
            {options.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
        </select>
    )
}

// ── IntervalSelector ──────────────────────────────────────────────────────────

const INTERVALS: { value: Interval; label: string }[] = [
    { value: '1m', label: '1 分' },
    { value: '5m', label: '5 分' },
    { value: '15m', label: '15 分' },
    { value: '1h', label: '1 時' },
    { value: '4h', label: '4 時' },
    { value: '1d', label: '日線' },
]

export function IntervalSelector() {
    const selected = useAppStore((s) => s.selectedInterval)
    const setSelected = useAppStore((s) => s.setSelectedInterval)

    return (
        <div className="flex items-center gap-1 bg-surface-card border border-surface-border rounded-lg p-1">
            {INTERVALS.map((i) => (
                <button
                    key={i.value}
                    onClick={() => setSelected(i.value)}
                    className={clsx(
                        'px-3 py-1 rounded-md text-xs font-medium transition-all',
                        selected === i.value
                            ? 'bg-brand-600 text-white shadow-sm'
                            : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover',
                    )}
                >
                    {i.label}
                </button>
            ))}
        </div>
    )
}

// ── SignalList ────────────────────────────────────────────────────────────────

interface SignalListProps { signals: SignalItem[] }

export function SignalList({ signals }: SignalListProps) {
    // 只需要呼叫這一行，取得 getTheme 函數
    const getTheme = useSignalTheme()

    if (signals.length === 0) { /* 略 */ }

    return (
        <div className="flex flex-col gap-2">
            {signals.map((signal) => {
                // 直接使用 getTheme，不用再管 colorMode 的邏輯了

                const theme = getTheme(signal.signal_type);
                const buyTheme = getTheme('BUY')
                const sellTheme = getTheme('SELL')

                return (
                    <Card key={signal.id} className="relative">
                        <div className="absolute top-3 right-3">
                            <ReliabilityBadge reliability={signal.reliability} />
                        </div>

                        <div className="flex items-start gap-3 pr-24">
                            {/* 左側 Icon 區塊 */}
                            <div className={clsx(
                                'mt-0.5 w-8 h-8 rounded-lg flex items-center justify-center text-xs font-bold shrink-0 transition-colors duration-300',
                                theme.bg,
                                theme.text
                            )}>
                                {signal.signal_type === 'BUY' ? '▲' : '▼'}
                            </div>

                            <div className="min-w-0">
                                <div className="flex items-baseline gap-2">
                                    {/* BUY / SELL 文字 */}
                                    <span className={clsx(
                                        'text-sm font-semibold transition-colors duration-300',
                                        theme.text
                                    )}>
                                        {signal.signal_type}
                                    </span>


                                    <span className="text-xs text-slate-500">{formatTimestamp(signal.timestamp_ms)}</span>
                                </div>
                                <p className="text-xs text-slate-400 mt-1 truncate">{signal.reason}</p>
                                <div className="flex items-center gap-4 mt-2 text-xs text-slate-500">
                                    <span>進場 <span className="text-slate-300">{formatPrice(signal.entry_price)}</span></span>
                                    <span>目標 <span className={clsx(signal.signal_type === 'BUY' ? buyTheme.text : sellTheme.text)}>{formatPrice(signal.target_price)}</span></span>
                                    <span>停損 <span className={clsx(signal.signal_type === 'BUY' ? sellTheme.text : buyTheme.text)}>{formatPrice(signal.stop_loss)}</span></span>
                                </div>
                            </div>
                        </div>
                        <div className="mt-3 pt-3 border-t border-surface-border">
                            <div className="flex items-center gap-2">
                                <span className="text-xs text-slate-500">AI 信心度</span>
                                <div className="flex-1 h-1.5 bg-surface rounded-full overflow-hidden">
                                    <div
                                        className="h-full bg-brand-500 rounded-full transition-all"
                                        style={{ width: `${signal.confidence * 100}%` }}
                                    />
                                </div>
                                <span className="text-xs text-slate-300 font-mono">{(signal.confidence * 100).toFixed(0)}%</span>
                            </div>
                        </div>
                    </Card>
                )
            })}
        </div>
    )
}

// ── PredictionPanel ───────────────────────────────────────────────────────────

interface PredictionPanelProps { signals: SignalItem[] }

export function PredictionPanel({ signals }: PredictionPanelProps) {
    // 只需要這行
    const getTheme = useSignalTheme()

    const latest = signals[0]
    const upCount = signals.filter((s) => s.signal_type === 'BUY').length
    const downCount = signals.filter((s) => s.signal_type === 'SELL').length
    const total = signals.length || 1
    const avgConf = signals.length > 0
        ? signals.reduce((sum, s) => sum + s.confidence, 0) / signals.length
        : 0

    // 直接向 Hook 拿買跟賣的主題包
    const buyTheme = getTheme('BUY')
    const sellTheme = getTheme('SELL')

    return (
        <Card>
            <h3 className="text-xs font-medium text-slate-400 uppercase tracking-wider mb-4">AI 預測概覽</h3>
            <div className="grid grid-cols-2 gap-3">
                {/* 買進信號區塊 */}
                <div className={`border rounded-lg p-3 transition-colors ${buyTheme.bgLight} ${buyTheme.border}`}>
                    <div className={`text-2xl font-bold ${buyTheme.text}`}>{upCount}</div>
                    <div className="text-xs text-slate-500 mt-0.5">買進信號</div>
                </div>

                {/* 賣出信號區塊 */}
                <div className={`border rounded-lg p-3 transition-colors ${sellTheme.bgLight} ${sellTheme.border}`}>
                    <div className={`text-2xl font-bold ${sellTheme.text}`}>{downCount}</div>
                    <div className="text-xs text-slate-500 mt-0.5">賣出信號</div>
                </div>
            </div>

            <div className="mt-4">
                <div className="flex justify-between text-xs text-slate-500 mb-1.5">
                    <span>多空比</span>
                    <span className="text-slate-300">{((upCount / total) * 100).toFixed(0)}% 多</span>
                </div>
                {/* 多空比進度條：動態替換顏色 */}
                <div className="h-2 bg-surface rounded-full overflow-hidden flex">
                    <div className={`h-full transition-all duration-500 ${buyTheme.bar}`} style={{ width: `${(upCount / total) * 100}%` }} />
                    <div className={`h-full transition-all duration-500 ${sellTheme.bar}`} style={{ width: `${(downCount / total) * 100}%` }} />
                </div>
            </div>

            <div className="mt-4 pt-3 border-t border-surface-border/60 flex items-center justify-between">
                <span className="text-xs text-slate-500">平均信心度</span>
                <span className="text-sm font-mono font-medium text-slate-200">{(avgConf * 100).toFixed(1)}%</span>
            </div>

            {latest && (
                <div className="mt-2 flex items-center justify-between">
                    <span className="text-xs text-slate-500">最新信號來源</span>
                    <ReliabilityBadge reliability={latest.reliability} />
                </div>
            )}
        </Card>
    )
}