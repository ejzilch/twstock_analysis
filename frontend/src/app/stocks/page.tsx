'use client'
import { useRouter } from 'next/navigation'
import { useSymbols } from '@/hooks'
import { StockTable } from '@/components/stocks'
import { LoadingSpinner, ErrorToast } from '@/components/ui'

export default function StocksPage() {
    const router = useRouter()
    const { data, isLoading, isError, error, refetch } = useSymbols()

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

                {data && <StockTable symbols={data.symbols} />}
            </div>
        </div>
    )
}