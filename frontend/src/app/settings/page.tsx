'use client'
import { ApiKeyForm, PreferenceForm } from '@/components/settings'

export default function SettingsPage() {
    return (
        <div className="flex flex-col h-full">
            <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
                <div>
                    <h1 className="text-base font-semibold text-slate-100">設定</h1>
                    <p className="text-xs text-slate-500 mt-0.5">API Key 與顯示偏好管理</p>
                </div>
            </header>

            <div className="flex-1 overflow-y-auto px-6 py-5">
                <div className="max-w-xl mx-auto flex flex-col gap-5">
                    <ApiKeyForm />
                    <PreferenceForm />

                    {/* Version info */}
                    <div className="text-center text-xs text-slate-600 py-2">
                        AI Bridge Frontend · v0.1.0 · API Contract v2.2
                    </div>
                </div>
            </div>
        </div>
    )
}