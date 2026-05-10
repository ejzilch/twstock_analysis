'use client'
import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { useSymbols } from '@/src/hooks'
import { StockTable } from '@/src/components/stocks'
import { LoadingSpinner, ErrorToast } from '@/src/components/ui'

export default function StocksPage() {
    const router = useRouter()
    // includeInactive 由 StockTable 內部的 toggle 控制顯示，
    // 但 fetch 需要在此層提前決定——因為 StockTable 目前收 symbols prop。
    // 方案：讓 StockTable 自己持有 includeInactive state 並通知此層，
    // 或是提升 state 到此層。這裡選擇提升，讓 fetch 跟著 toggle 走。
    const [includeInactive, setIncludeInactive] = useState(false)

    const { data, isLoading, isError, error, refetch } = useSymbols({ includeInactive })

    return (
        <div className="flex flex-col h-full">
            <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
                <div>
                    <h1 className="text-base font-semibold text-slate-100">股票總覽</h1>
                    <p className="text-xs text-slate-500 mt-0.5">
                        {data ? `共 ${data.count} 檔股票` : '動態載入中...'}
                    </p>
                </div>
                {data?.last_synced_ms && (
                    <span className="ml-auto text-xs text-slate-600">
                        最後同步：{new Date(data.last_synced_ms).toLocaleString('zh-TW', { timeZone: 'Asia/Taipei' })}
                    </span>
                )}
            </header>

            <div className="flex-1 overflow-y-auto px-6 py-5">
                {isLoading && (
                    <div className="flex items-center justify-center h-64">
                        <LoadingSpinner size="lg" label="載入股票清單..." />
                    </div>
                )}

                {isError && (
                    <>
                        <div className="flex items-center justify-center h-64">
                            <p className="text-sm text-slate-500">無法載入股票清單</p>
                        </div>
                        <ErrorToast error={error} onRetry={refetch} onRedirect={router.push} />
                    </>
                )}

                {data && (
                    <StockTable
                        symbols={data.symbols}
                        includeInactive={includeInactive}
                        onToggleInactive={() => setIncludeInactive((v) => !v)}
                    />
                )}
            </div>
        </div>
    )
}