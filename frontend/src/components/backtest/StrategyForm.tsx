'use client'
import { useState } from 'react'
import BacktestDateRangePicker from '@/src/components/features/backtest/BacktestDateRangePicker'
import { Input, Select, Card, Button } from '@/src/components/ui'
import { useSymbols } from '@/src/hooks'
import { DateValueType } from "react-tailwindcss-datepicker"

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
            <h3 className="text-sm font-semibold text-slate-200 mb-4" > 策略參數設定 </h3>
            < div className="grid grid-cols-2 gap-4" >
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
                <div className="col-span-2 flex flex-col gap-1" >
                    <label className="text-sm text-slate-400" > 回測區間 </label>
                    < BacktestDateRangePicker
                        onChange={({ from_ms, to_ms }) => {
                            // 存到 state 或直接丟 API
                        }
                        }
                    />
                </div>
                < Input label="初始資金 (TWD)" value={capital} onChange={setCapital} placeholder="100000" />
                <Input label="倉位比例 (%)" value={positionSize} onChange={setPositionSize} placeholder="100" />
            </div>
            < div className="mt-5" >
                <Button onClick={handleSubmit} loading={isLoading} className="w-full" size="lg" >
                    {isLoading ? '回測執行中...' : '執行回測'}
                </Button>
            </div>
        </Card>
    )
}