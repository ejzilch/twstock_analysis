'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { clsx } from 'clsx'

const NAV_ITEMS = [
    { href: '/dashboard', icon: '◈', label: 'Dashboard' },
    { href: '/stocks', icon: '≡', label: '股票總覽' },
    { href: '/backtest', icon: '◷', label: '回測' },
    { href: '/settings', icon: '⚙', label: '設定' },
]

export function SidebarNav() {
    const pathname = usePathname()

    return (

        <nav className="flex-1 px-3 py-4 flex flex-col gap-1">
            {NAV_ITEMS.map((item) => {
                const isActive = pathname === item.href || pathname.startsWith(item.href + '/')
                return (
                    <Link
                        key={item.href}
                        href={item.href}
                        className={clsx(
                            'flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-all group',
                            isActive
                                ? 'bg-brand-600/15 text-brand-300 border border-brand-500/20'
                                : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover border border-transparent',
                        )}
                    >
                        <span className={clsx(
                            'text-base transition-opacity',
                            isActive ? 'opacity-100' : 'opacity-60 group-hover:opacity-100',
                        )}>
                            {item.icon}
                        </span>
                        <span className={isActive ? 'font-medium' : ''}>{item.label}</span>
                        {isActive && (
                            <span className="ml-auto w-1.5 h-1.5 rounded-full bg-brand-400" />
                        )}
                    </Link>
                )
            })}
        </nav>

    )
}