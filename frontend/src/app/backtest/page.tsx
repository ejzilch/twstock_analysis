'use client'
import { useRouter } from 'next/navigation'
import { useBacktest } from '@/hooks'
import { StrategyForm, BacktestResult } from '@/components/backtest'
import { BacktestChart } from '@/components/backtest/Backtestchart'
import { ErrorToast, Card, LoadingSpinner } from '@/components/ui'

export default function BacktestPage() {
    const router = useRouter()
    const backtest = useBacktest()

    return (
        <div className="flex flex-col h-full">
            <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
                <div>
                    <h1 className="text-base font-semibold text-slate-100">回測</h1>
                    <p className="text-xs text-slate-500 mt-0.5">策略歷史績效模擬</p>
                </div>
            </header>

            <div className="flex-1 overflow-y-auto px-6 py-5">
                <div className="max-w-3xl mx-auto flex flex-col gap-5">
                    {/* 注意事項 */}
                    <div className="flex items-start gap-3 px-4 py-3 bg-brand-900/30 border border-brand-500/20 rounded-xl text-xs text-brand-300">
                        <span className="mt-0.5 shrink-0">ℹ</span>
                        <span>
                            所有技術指標由 Rust 引擎統一計算，確保回測結果與實盤信號完全一致。
                            策略邏輯在 Python AI Service 執行，不自行計算指標。
                        </span>
                    </div>

                    {/* 策略設定表單 */}
                    <StrategyForm
                        onSubmit={(params) => backtest.mutate(params)}
                        isLoading={backtest.isPending}
                    />

                    {/* 執行中狀態 */}
                    {backtest.isPending && (
                        <Card className="flex items-center justify-center py-12">
                            <LoadingSpinner size="lg" label="回測執行中，請稍候..." />
                        </Card>
                    )}

                    {/* 回測結果 */}
                    {backtest.data && !backtest.isPending && (
                        <>
                            <BacktestResult result={backtest.data} />
                            <BacktestChart
                                symbol={backtest.data.symbol}
                                from_ms={backtest.data.from_ms}
                                to_ms={backtest.data.to_ms}
                            />
                        </>
                    )}

                    {/* 錯誤處理 */}
                    {backtest.isError && (
                        <ErrorToast
                            error={backtest.error}
                            onRetry={() => backtest.reset()}
                            onRedirect={router.push}
                        />
                    )}
                </div>
            </div>
        </div>
    )
}