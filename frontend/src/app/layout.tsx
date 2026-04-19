import type { Metadata } from 'next'
import type { ReactNode } from 'react'
import { AppQueryClientProvider } from '@/providers/QueryClientProvider'
import { SidebarNav } from '@/components/ui/SidebarNav'
import { ColorModeToggle } from '@/components/mode/ColorModeToggle'

import './globals.css'

export const metadata: Metadata = {
    title: 'AI Bridge — 股票分析與交易信號系統',
    description: '準確、穩定、高效的股票分析與交易信號系統',
}

export default function RootLayout({ children }: { children: ReactNode }) {
    return (
        <html lang="zh-TW">
            <body className="bg-surface text-slate-200 antialiased">
                <AppQueryClientProvider>
                    <div className="flex h-screen overflow-hidden">
                        <Sidebar />
                        <main className="flex-1 overflow-y-auto">
                            {children}
                        </main>
                    </div>
                </AppQueryClientProvider>
            </body>
        </html>
    )
}

function Sidebar() {
    return (
        <aside className="w-64 shrink-0 bg-surface-card border-r border-surface-border flex flex-col z-10">
            <div className="px-5 py-5 border-b border-surface-border">
                <div className="text-xs text-slate-500 tracking-widest uppercase mb-0.5">AI Bridge</div>
                <div className="text-base font-semibold text-slate-100">交易信號系統</div>
            </div>
            {/* SidebarNav is a client component that reads pathname for active state */}
            <SidebarNav />

            <div className="px-4 pb-4">
                <ColorModeToggle />
            </div>

            <div className="px-5 py-4 border-t border-surface-border">
                <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse-slow" />
                    <span className="text-xs text-slate-500">系統運行中</span>
                </div>
            </div>

        </aside>
    )
}
