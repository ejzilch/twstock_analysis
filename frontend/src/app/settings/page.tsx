'use client'
/**
 * src/app/settings/page.tsx（完整更新版）
 *
 * 新增 ManualSyncPanel，置於 PreferenceForm 下方。
 */
import { useState } from 'react'
import { clsx } from 'clsx'
import { ApiKeyForm, PreferenceForm, DailySchedulePanel } from '@/src/components/settings'
import { ManualSyncPanel } from '@/src/components/settings/ManualSyncPanel'

export default function SettingsPage() {
  const [activeTab, setActiveTab] = useState<'api' | 'pref' | 'sync'>('api')

  const tabs = [
    { key: 'api' as const, label: 'API Key 管理' },
    { key: 'pref' as const, label: '顯示偏好' },
    { key: 'sync' as const, label: '資料同步' },
  ]

  return (
    <div className="flex flex-col h-full">
      <header className="flex items-center gap-4 px-6 py-4 border-b border-surface-border bg-surface-card/50 backdrop-blur-sm sticky top-0 z-10">
        <div>
          <h1 className="text-base font-semibold text-slate-100">設定</h1>
          <p className="text-xs text-slate-500 mt-0.5">API Key、顯示偏好與資料管理</p>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto px-6 py-5">
        <div className="max-w-3xl mx-auto flex flex-col gap-5">
          <div className="flex items-center gap-2 rounded-lg border border-surface-border bg-surface-card/40 p-1">
            {tabs.map((tab) => (
              <button
                key={tab.key}
                onClick={() => setActiveTab(tab.key)}
                className={clsx(
                  'px-3 py-1.5 text-xs rounded-md transition-all',
                  activeTab === tab.key
                    ? 'bg-brand-600 text-white'
                    : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover'
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {activeTab === 'api' && <ApiKeyForm />}
          {activeTab === 'pref' && <PreferenceForm />}
          {activeTab === 'sync' && (
            <div className="space-y-5">
              <DailySchedulePanel />
              <ManualSyncPanel />
            </div>
          )}

          <div className="text-center text-xs text-slate-600 py-2">
            AI Bridge Frontend · v0.1.0 · API Contract v2.2
          </div>
        </div>
      </div>
    </div>
  )
}
