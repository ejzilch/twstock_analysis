'use client'
import { useState } from 'react'
import { clsx } from 'clsx'
import BacktestDateRangePicker from '@/components/ui/BacktestDateRangePicker'
import type { BacktestResponse } from '@/types/api.generated'
import { Card, Button, Input, Select } from '@/components/ui'
import { formatCapital, formatPercent } from '@/lib/utils'
import { useSymbols } from '@/hooks'
import { DateValueType } from "react-tailwindcss-datepicker"


// 載入我們自建的 Wrapper，而不是直接載入套件


// ── StrategyForm ──────────────────────────────────────────────────────────────

interface StrategyFormProps {
    onSubmit: (params: {
        symbol: string
        strategy_name: string
        from_ms: number
        to_ms: number
        initial_capital: number
        position_size_percent: number
    }) => void
    isLoading: boolean
}

const STRATEGIES = [
    { value: 'trend_follow_v1', label: '趨勢跟隨 v1' },
    { value: 'mean_reversion_v1', label: '均值回歸 v1' },
    { value: 'breakout_v1', label: '突破策略 v1' },
]

export function StrategyForm({ onSubmit, isLoading }: StrategyFormProps) {
    const { data: symbolsData } = useSymbols()
    const [symbol, setSymbol] = useState('2330')
    const [strategy, setStrategy] = useState('trend_follow_v1')
    const [capital, setCapital] = useState('100000')
    const [positionSize, setPositionSize] = useState('100')

    const [dateRange, setDateRange] = useState<DateValueType>({
        startDate: new Date(2025, 0, 1),
        endDate: new Date(2025, 11, 1)
    })

    const symbolOptions = (symbolsData?.symbols ?? []).map((s) => ({
        value: s.symbol,
        label: `${s.symbol} ${s.name}`,
    }))

    function handleSubmit() {
        const fromMs = dateRange?.startDate?.getTime() ?? new Date(2025, 0, 1).getTime()
        const toMs = dateRange?.endDate?.getTime() ?? new Date(2025, 11, 1).getTime()
        onSubmit({
            symbol,
            strategy_name: strategy,
            from_ms: fromMs,
            to_ms: toMs,
            initial_capital: parseFloat(capital),
            position_size_percent: parseInt(positionSize, 10),
        })
    }

    return (
        <Card>
            <h3 className="text-sm font-semibold text-slate-200 mb-4">策略參數設定</h3>
            <div className="grid grid-cols-2 gap-4">
                <Select
                    label="股票"
                    value={symbol}
                    onChange={setSymbol}
                    options={symbolOptions.length > 0 ? symbolOptions : [{ value: '2330', label: '2330 台積電' }]}
                />
                <Select
                    label="策略"
                    value={strategy}
                    onChange={setStrategy}
                    options={STRATEGIES}
                />
                <div className="col-span-2 flex flex-col gap-1">
                    <label className="text-sm text-slate-400">回測區間</label>
                    <BacktestDateRangePicker
                        onChange={({ from_ms, to_ms }) => {
                            // 存到 state 或直接丟 API
                        }}
                    />
                </div>
                <Input label="初始資金 (TWD)" value={capital} onChange={setCapital} placeholder="100000" />
                <Input label="倉位比例 (%)" value={positionSize} onChange={setPositionSize} placeholder="100" />
            </div>
            <div className="mt-5">
                <Button onClick={handleSubmit} loading={isLoading} className="w-full" size="lg">
                    {isLoading ? '回測執行中...' : '執行回測'}
                </Button>
            </div>
        </Card>
    )
}

// ── BacktestResult ────────────────────────────────────────────────────────────

interface BacktestResultProps { result: BacktestResponse }

interface MetricCardProps { label: string; value: string; sub?: string; positive?: boolean }

function MetricCard({ label, value, sub, positive }: MetricCardProps) {
    return (
        <div className="bg-surface border border-surface-border rounded-lg p-4">
            <div className="text-xs text-slate-500 uppercase tracking-wider mb-1.5">{label}</div>
            <div className={clsx(
                'text-2xl font-bold font-mono',
                positive === true ? 'text-emerald-400' :
                    positive === false ? 'text-red-400' : 'text-slate-200',
            )}>
                {value}
            </div>
            {sub && <div className="text-xs text-slate-500 mt-1">{sub}</div>}
        </div>
    )
}

export function BacktestResult({ result }: BacktestResultProps) {
    const { metrics } = result
    const pnl = result.final_capital - result.initial_capital
    const pnlPositive = pnl >= 0

    return (
        <Card>
            <div className="flex items-center justify-between mb-5">
                <div>
                    <h3 className="text-sm font-semibold text-slate-200">{result.strategy_name}</h3>
                    <p className="text-xs text-slate-500 mt-0.5">{result.symbol}</p>
                </div>
                <div className={clsx('text-right', pnlPositive ? 'text-emerald-400' : 'text-red-400')}>
                    <div className="text-xl font-bold font-mono">{formatCapital(result.final_capital)}</div>
                    <div className="text-xs opacity-80">
                        {pnlPositive ? '+' : ''}{formatCapital(pnl)}
                    </div>
                </div>
            </div>

            <div className="grid grid-cols-2 gap-3 mb-3">
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
                    value={formatPercent(-metrics.max_drawdown)}
                    positive={false}
                />
                <MetricCard
                    label="獲利因子"
                    value={metrics.profit_factor.toFixed(2)}
                    positive={metrics.profit_factor >= 1.5}
                />
            </div>

            <div className="grid grid-cols-3 gap-3">
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