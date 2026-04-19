import { clsx } from 'clsx'
import type { SignalItem, } from '@/src/types/api.generated'
import { ReliabilityBadge, Card } from '@/src/components/ui'
import { formatPrice, formatTimestamp } from '@/src/lib/utils'
import { useSignalTheme } from '@/src/hooks'

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