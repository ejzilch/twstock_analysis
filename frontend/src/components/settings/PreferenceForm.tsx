import { useAppStore } from '@/src/store/useAppStore'
import { Card, Input, Select } from '@/src/components/ui'

export function PreferenceForm() {
    const isEco = useAppStore((s) => s.isEcoModeEnabled)
    const toggleEco = useAppStore((s) => s.toggleEcoMode)
    const interval = useAppStore((s) => s.selectedInterval)
    const setInterval = useAppStore((s) => s.setSelectedInterval)
    const symbol = useAppStore((s) => s.selectedSymbol)
    const setSymbol = useAppStore((s) => s.setSelectedSymbol)

    const INTERVAL_OPTIONS = [
        { value: '1m', label: '1 分鐘' },
        { value: '5m', label: '5 分鐘' },
        { value: '15m', label: '15 分鐘' },
        { value: '1h', label: '1 小時' },
        { value: '4h', label: '4 小時' },
        { value: '1d', label: '日線' },
    ]

    return (
        <Card>
            <h3 className="text-sm font-semibold text-slate-200 mb-5">顯示偏好</h3>
            <div className="flex flex-col gap-5">
                <div className="flex items-center justify-between">
                    <div>
                        <div className="text-sm text-slate-300">收盤後節能模式</div>
                        <div className="text-xs text-slate-500 mt-0.5">13:30 後將輪詢頻率降為 5 分鐘</div>
                    </div>
                    <button
                        onClick={toggleEco}
                        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${isEco ? 'bg-brand-600' : 'bg-surface-border'
                            }`}
                    >
                        <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${isEco ? 'translate-x-6' : 'translate-x-1'
                            }`} />
                    </button>
                </div>

                <Select
                    label="預設 K 線粒度"
                    value={interval}
                    onChange={setInterval}
                    options={INTERVAL_OPTIONS}
                />

                <Input
                    label="預設股票代號"
                    value={symbol}
                    onChange={setSymbol}
                    placeholder="例如：2330"
                />
            </div>
        </Card>
    )
}