'use client'
import { useEffect } from 'react'
import { Card } from '@/src/components/ui'
import { apiClient } from '@/src/lib/api-client'
import { useAppStore } from '@/src/store/useAppStore'

interface DailyScheduleConfig { enabled: boolean; time: string }

export function DailySchedulePanel() {
    const enabled = useAppStore((s) => s.scheduleEnabled)
    const time = useAppStore((s) => s.scheduleTime)
    const loaded = useAppStore((s) => s.scheduleLoaded)
    const setSchedule = useAppStore((s) => s.setSchedule)
    const setScheduleEnabled = useAppStore((s) => s.setScheduleEnabled)
    const setScheduleTime = useAppStore((s) => s.setScheduleTime)

    // 只在第一次（store 尚未載入過）才 fetch
    useEffect(() => {
        if (loaded) return
        apiClient<DailyScheduleConfig>('/api/v1/admin/sync/schedule')
            .then((res) => setSchedule(res.enabled, res.time || '02:00'))
            .catch(() => setSchedule(false, '02:00'))
    }, [loaded])

    // 狀態變更時 POST（跳過初始化）
    useEffect(() => {
        if (!loaded) return
        apiClient('/api/v1/admin/sync/schedule', {
            method: 'POST',
            body: JSON.stringify({ enabled, time }),
        }).catch(() => { })
    }, [enabled, time, loaded])

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
                        onClick={() => setScheduleEnabled(!enabled)}
                        disabled={!loaded}
                        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${!loaded
                                ? 'bg-surface-border opacity-40 cursor-not-allowed'  // loading 狀態，不顯示任何傾向
                                : enabled
                                    ? 'bg-brand-600'
                                    : 'bg-surface-border'
                            }`}
                    >
                        <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${enabled ? 'translate-x-6' : 'translate-x-1'
                            }`} />
                    </button>
                </div>

                <label className="block">
                    <span className="text-xs text-slate-400">每日啟動時間</span>
                    <input
                        type="time"
                        value={time}
                        onChange={(e) => setScheduleTime(e.target.value)}
                        disabled={!enabled || !loaded}
                        className="mt-1 w-full px-2 py-1.5 rounded bg-surface border border-surface-border text-sm text-slate-200 disabled:opacity-50"
                    />
                </label>
            </div>
        </Card>
    )
}