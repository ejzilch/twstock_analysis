'use client'
import { useRouter } from 'next/navigation'
import { useBacktest } from '@/src/hooks'
import { StrategyForm, BacktestResult } from '@/src/components/backtest'
import { BacktestChart } from '@/src/components/backtest/BacktestChart'
import { ErrorToast, Card, LoadingSpinner } from '@/src/components/ui'

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
                {/* 注意事項 */}
                <div className="flex items-start gap-3 px-4 py-3 bg-brand-900/50 border border-brand-500/20 rounded-xl  text-brand-300">
                    <span>ℹ️</span>
                    <span>
                        所有技術指標由 Rust 引擎統一計算，確保回測結果與實盤信號完全一致。
                        策略邏輯在 Python AI Service 執行，不自行計算指標。
                    </span>
                </div>
            </header>

            <div className="flex-1 overflow-y-auto px-6 py-5">
                <div className="flex gap-5 items-start">
                    {/* 左側：策略設定 + 回測結果 */}
                    <div className="w-120 shrink-0 flex flex-col gap-5">
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

                        {/* 回測結果指標 */}
                        {backtest.data && !backtest.isPending && (
                            <BacktestResult result={backtest.data} />
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

                    {/* 右側：回測期間 K 線 */}
                    <div className="flex-1 min-w-0">
                        {backtest.data && !backtest.isPending ? (
                            <BacktestChart
                                symbol={backtest.data.symbol}
                                strategyName={backtest.data.strategy_name}
                                from_ms={backtest.data.from_ms}
                                to_ms={backtest.data.to_ms}
                                exitFilterPct={backtest.data.exit_filter_pct}
                            />
                        ) : (
                            <Card className="flex items-center justify-center h-full min-h-[400px]">
                                <p className="text-sm text-slate-500">執行回測後顯示 K 線圖</p>
                            </Card>
                        )}
                    </div>
                </div>
            </div>
        </div>
    )
}