'use client'

import { useState } from 'react'
import { DayPicker, DateRange } from 'react-day-picker'
import 'react-day-picker/dist/style.css'
import { format } from 'date-fns'
import { clsx } from 'clsx'
import './daypicker.css'

type RangeKey = '1M' | '3M' | '6M' | '1Y' | 'YTD' | 'MAX' | 'CUSTOM'

interface Props {
    onChange: (range: { from_ms: number; to_ms: number }) => void
}

const RANGE_OPTIONS: RangeKey[] = ['1M', '3M', '6M', '1Y', 'YTD', 'MAX']

function getRange(key: RangeKey): DateRange {
    const now = new Date()

    switch (key) {
        case '1M': return { from: new Date(now.getFullYear(), now.getMonth() - 1, now.getDate()), to: now }
        case '3M': return { from: new Date(now.getFullYear(), now.getMonth() - 3, now.getDate()), to: now }
        case '6M': return { from: new Date(now.getFullYear(), now.getMonth() - 6, now.getDate()), to: now }
        case '1Y': return { from: new Date(now.getFullYear() - 1, now.getMonth(), now.getDate()), to: now }
        case 'YTD': return { from: new Date(now.getFullYear(), 0, 1), to: now }
        case 'MAX': return { from: new Date(2000, 0, 1), to: now }
        default: return { from: now, to: now }
    }
}

export default function BacktestDateRangePicker({ onChange }: Props) {
    const [active, setActive] = useState<RangeKey>('1Y')
    const [range, setRange] = useState<DateRange | undefined>(getRange('1Y'))

    function applyQuickRange(key: RangeKey) {
        setActive(key)
        const r = getRange(key)
        setRange(r)

        if (r.from && r.to) {
            onChange({
                from_ms: r.from.getTime(),
                to_ms: r.to.getTime()
            })
        }
    }

    function handleSelect(r: DateRange | undefined) {
        setActive('CUSTOM')
        setRange(r)

        if (r?.from && r?.to) {
            onChange({
                from_ms: r.from.getTime(),
                to_ms: r.to.getTime()
            })
        }
    }

    return (
        <div className="flex flex-col gap-3">

            {/* 快捷區間 */}
            <div className="flex gap-2 flex-wrap">
                {RANGE_OPTIONS.map((key) => (
                    <button
                        key={key}
                        onClick={() => applyQuickRange(key)}
                        className={clsx(
                            'px-3 py-1.5 text-xs rounded-lg border transition',
                            active === key
                                ? 'bg-brand-600 text-white border-brand-500'
                                : 'bg-surface border-surface-border text-slate-400 hover:text-slate-200'
                        )}
                    >
                        {key}
                    </button>
                ))}

                <button
                    onClick={() => setActive('CUSTOM')}
                    className={clsx(
                        'px-3 py-1.5 text-xs rounded-lg border',
                        active === 'CUSTOM'
                            ? 'bg-brand-600 text-white border-brand-500'
                            : 'bg-surface border-surface-border text-slate-400'
                    )}
                >
                    自訂
                </button>
            </div>

            {/* 日期顯示 */}
            {range?.from && range?.to && (
                <div className="text-xs text-slate-400">
                    {format(range.from, 'yyyy/MM/dd')} - {format(range.to, 'yyyy/MM/dd')}
                </div>
            )}

            {/* Calendar（不會炸的關鍵：永遠 mounted） */}
            <div className={clsx(active === 'CUSTOM' ? 'block' : 'hidden', 'bg - surface - card border border-surface-border rounded-xl p-4')}>
                <DayPicker
                    mode="range"
                    selected={range}
                    onSelect={handleSelect}
                    numberOfMonths={2}
                />
            </div>
        </div >
    )
}