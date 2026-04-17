import type { Metadata } from 'next'
import type { ReactNode } from 'react'
import Link from 'next/link'
import { AppQueryClientProvider } from '@/providers/QueryClientProvider'
import { ColorModeToggle } from '@/components/mode/ColorModeToggle'
import './globals.css'

export const metadata: Metadata = {
    title: 'AI Bridge — 股票分析與交易信號系統',
    description: '準確、穩定、高效的股票分析與交易信號系統',
}

export default function RootLayout({ children }: { children: ReactNode }) {
    return (
        <html lang="zh-TW">
            <body className="bg-[#0B0F19] text-slate-200 antialiased selection:bg-blue-500/30">
                <AppQueryClientProvider>
                    <div className="flex h-screen overflow-hidden">
                        <Sidebar />
                        {/* 主內容區域：加上微小的內陰影區分邊界 */}
                        <main className="flex-1 overflow-y-auto bg-surface shadow-[inset_1px_0_0_0_rgba(255,255,255,0.05)]">
                            {children}
                        </main>
                    </div>
                </AppQueryClientProvider>
            </body>
        </html>
    )
}

function Sidebar() {
    const navItems = [
        { href: '/dashboard', icon: '◈', label: 'Dashboard' },
        { href: '/stocks', icon: '◫', label: '股票總覽' },
        { href: '/backtest', icon: '◷', label: '策略回測' },
        { href: '/settings', icon: '⚙', label: '系統設定' },
    ]

    return (
        <aside className="w-64 shrink-0 bg-surface-card border-r border-surface-border flex flex-col z-10">
            {/* Logo 區塊 */}
            <div className="h-20 px-6 flex items-center gap-3 border-b border-surface-border/60">
                <div className="w-9 h-9 rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 flex items-center justify-center shadow-lg shadow-blue-500/20">
                    <span className="text-white font-bold text-lg leading-none tracking-tighter">AI</span>
                </div>
                <div className="flex flex-col justify-center">
                    <span className="text-[10px] text-slate-400 font-mono tracking-widest uppercase mb-0.5 leading-none">Bridge System</span>
                    <span className="text-sm font-bold text-slate-100 tracking-wide leading-none">量化交易信號</span>
                </div>
            </div>

            {/* 導覽選單 */}
            <nav className="flex-1 px-4 py-6 flex flex-col gap-1.5 overflow-y-auto">
                <div className="text-xs font-semibold text-slate-500 mb-2 px-2 uppercase tracking-wider">主選單</div>
                {navItems.map((item) => (
                    <Link
                        key={item.href}
                        href={item.href}
                        className="flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm font-medium text-slate-400 hover:text-white hover:bg-slate-800/50 hover:shadow-sm transition-all duration-200 group relative"
                    >
                        <span className="text-lg opacity-70 group-hover:opacity-100 group-hover:scale-110 transition-transform duration-200">
                            {item.icon}
                        </span>
                        {item.label}

                        {/* Hover 時右側的小箭頭提示 (可選) */}
                        <span className="ml-auto opacity-0 -translate-x-2 group-hover:opacity-100 group-hover:translate-x-0 transition-all duration-200 text-slate-500">
                            →
                        </span>
                    </Link>
                ))}
            </nav>

            <div className="px-4 pb-4">
                <ColorModeToggle />
            </div>

            {/* 底部系統狀態 */}
            <div className="px-5 py-5 border-t border-surface-border/60 bg-surface-card/50">
                <div className="flex items-center gap-3 px-2">
                    <div className="relative flex h-2.5 w-2.5">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                        <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-emerald-500"></span>
                    </div>
                    <div className="flex flex-col">
                        <span className="text-xs font-medium text-slate-300">API 已連線</span>
                        <span className="text-[10px] text-slate-500 font-mono mt-0.5">Latency: 12ms</span>
                    </div>
                </div>
            </div>
        </aside>
    )
}