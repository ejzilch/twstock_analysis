import type { SignalItem } from '@/src/types/api.generated'
import { ReliabilityBadge, Card } from '@/src/components/ui'
import { useSignalTheme } from '@/src/hooks/useSignalTheme'

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