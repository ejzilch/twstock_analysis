import { clsx } from 'clsx'
import type { BacktestResponse } from '@/src/types/api.generated'
import { formatCapital, formatPercent } from '@/src/lib/utils'
import { Card } from '@/src/components/ui'
import { MetricCard } from '@/src/components/backtest'
import { useAppStore } from '@/src/store/useAppStore'

interface BacktestResultProps { result: BacktestResponse }

export function BacktestResult({ result }: BacktestResultProps) {
    const { metrics } = result
    const colorMode = useAppStore((s) => s.colorMode)
    const pnl = result.final_capital - result.initial_capital
    const pnlPositive = pnl >= 0

    // TW：賺錢紅字；US：賺錢綠字
    const profitColor = colorMode === 'TW' ? 'text-red-400' : 'text-emerald-400'
    const lossColor = colorMode === 'TW' ? 'text-emerald-400' : 'text-red-400'
    const pnlColor = pnlPositive ? profitColor : lossColor

    return (
        <Card>
            <div className="flex items-center justify-between mb-5" >
                <div>
                    <h3 className="text-sm font-semibold text-slate-200" > {result.strategy_name} </h3>
                    < p className="text-xs text-slate-500 mt-0.5" > {result.symbol} </p>
                </div>
                < div className={clsx('text-right', pnlColor)} >
                    <div className="text-xl font-bold font-mono" > {formatCapital(result.final_capital)} </div>
                    < div className="text-xs opacity-80" >
                        {pnlPositive ? '+' : ''}{formatCapital(pnl)}
                    </div>
                </div>
            </div>

            < div className="grid grid-cols-2 gap-3 mb-3" >
                <MetricCard
                    label="年化報酬"
                    value={formatPercent(metrics.annual_return)}
                    positive={metrics.annual_return >= 0}
                />
                <MetricCard
                    label="夏普比率"
                    value={metrics.sharpe_ratio.toFixed(2)}
                    positive={metrics.sharpe_ratio >= 1}
                />
                <MetricCard
                    label="最大回撤"
                    value={formatPercent(- metrics.max_drawdown)
                    }
                    positive={false}
                />
                <MetricCard
                    label="獲利因子"
                    value={metrics.profit_factor.toFixed(2)}
                    positive={metrics.profit_factor >= 1.5}
                />
            </div>

            < div className="grid grid-cols-3 gap-3" >
                <MetricCard label="總交易次數" value={String(metrics.total_trades)} />
                <MetricCard
                    label="勝率"
                    value={formatPercent(metrics.win_rate)}
                    positive={metrics.win_rate >= 0.5}
                />
                <MetricCard
                    label="盈虧交易"
                    value={`${metrics.winning_trades}W / ${metrics.losing_trades}L`}
                />
            </div>
        </Card>
    )
}