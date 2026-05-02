'use client'
import { useEffect, useState } from 'react'
import { Card } from '@/src/components/ui'
import { apiClient } from '@/src/lib/api-client'

interface DailyScheduleConfig { enabled: boolean; time: string }

export function DailySchedulePanel() {
    const [enabled, setEnabled] = useState<boolean | null>(null)
    const [time, setTime] = useState('02:00')
    const [loading, setLoading] = useState(true)

    useEffect(() => {
        apiClient<DailyScheduleConfig>('/api/v1/admin/sync/schedule')
            .then((res) => {
                setEnabled(res.enabled)
                setTime(res.time || '02:00')
            })
            .catch(() => {
                setEnabled(false)
            })
            .finally(() => setLoading(false))
    }, [])

    useEffect(() => {
        if (loading || enabled === null) return
        apiClient<DailyScheduleConfig>('/api/v1/admin/sync/schedule', {
            method: 'POST',
            body: JSON.stringify({ enabled, time }),
        }).catch(() => { })
    }, [enabled, time, loading])

    return (
        <Card>
            <h3 className="text-sm font-semibold text-slate-200 mb-5">每日排程</h3>
            <div className="space-y-4">
                <div className="flex items-center justify-between">
                    <div>
                        <div className="text-sm text-slate-300">啟用每日自動同步</div>
                        <div className="text-xs text-slate-500 mt-0.5">儲存於後端，server 會依設定時間自動觸發同步</div>
                    </div>
                    <button
                        onClick={() => setEnabled((v) => !v)}
                        disabled={loading || enabled === null}
                        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${(enabled ?? false) ? 'bg-brand-600' : 'bg-surface-border'} ${(loading || enabled === null) ? 'opacity-60 cursor-not-allowed' : ''}`}
                    >
                        <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${(enabled ?? false) ? 'translate-x-6' : 'translate-x-1'}`} />
                    </button>
                </div>

                <label className="block">
                    <span className="text-xs text-slate-400">每日啟動時間</span>
                    <input type="time" value={time} onChange={(e) => setTime(e.target.value)} disabled={!enabled || loading}
                        className="mt-1 w-full px-2 py-1.5 rounded bg-surface border border-surface-border text-sm text-slate-200 disabled:opacity-50" />
                </label>
            </div>
        </Card>
    )
}