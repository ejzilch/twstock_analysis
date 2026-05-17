'use client'
import Link from 'next/link'
import { usePathname } from 'next/navigation'
import { clsx } from 'clsx'

type NavItem = {
    href?: string
    label: string
    icon?: string
    status?: 'existing' | 'planned'
    children?: NavItem[]
}

type NavGroup = {
    title: string
    items: NavItem[]
}

const NAV_GROUPS: NavGroup[] = [
    {
        title: 'TW Stocks analysis',
        items: [
            { href: '/dashboard', icon: '◈', label: 'Dashboard（總覽）', status: 'existing' },
            { href: '/stocks', icon: '≡', label: '自選股票', status: 'existing' },
            {
                href: '/backtest',
                icon: '◷',
                label: '回測',
                status: 'existing',
                children: [
                    { href: '/backtest', label: '個股回測（現有）', status: 'existing' },
                    { label: '全量回測（待開發）', status: 'planned' },
                ],
            },
            { href: '/stocks', icon: '▤', label: '股票總覽（現有）', status: 'existing' },
        ],
    },
    {
        title: '設定',
        items: [
            { href: '/settings', icon: '⚙', label: '設定（現有）', status: 'existing' },
        ],
    },
]

function StatusPill({ status }: { status?: 'existing' | 'planned' }) {
    if (!status) return null

    return (
        <span
            className={clsx(
                'ml-auto rounded px-1.5 py-0.5 text-[10px] leading-none',
                status === 'existing'
                    ? 'bg-emerald-500/10 text-emerald-300'
                    : 'bg-amber-500/10 text-amber-300',
            )}
        >
            {status === 'existing' ? '現有' : '待開發'}
        </span>
    )
}

export function SidebarNav() {
    const pathname = usePathname()

    return (
        <nav className="flex-1 overflow-y-auto px-3 py-4">
            <div className="flex flex-col gap-5">
                {NAV_GROUPS.map((group) => (
                    <section key={group.title} className="space-y-2">
                        <h3 className="px-2 text-[11px] tracking-wider uppercase text-slate-500">
                            {group.title}
                        </h3>

                        <div className="flex flex-col gap-1">
                            {group.items.map((item) => {
                                const isActive = item.href
                                    ? pathname === item.href || pathname.startsWith(item.href + '/')
                                    : false

                                return (
                                    <div key={item.label} className="space-y-1">
                                        {item.href ? (
                                            <Link
                                                href={item.href}
                                                className={clsx(
                                                    'flex items-center gap-3 rounded-lg border px-3 py-2.5 text-sm transition-all group',
                                                    isActive
                                                        ? 'bg-brand-600/15 text-brand-300 border-brand-500/20'
                                                        : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover border-transparent',
                                                )}
                                            >
                                                <span
                                                    className={clsx(
                                                        'text-base transition-opacity',
                                                        isActive ? 'opacity-100' : 'opacity-60 group-hover:opacity-100',
                                                    )}
                                                >
                                                    {item.icon ?? '•'}
                                                </span>
                                                <span className={isActive ? 'font-medium' : ''}>{item.label}</span>
                                                <StatusPill status={item.status} />
                                            </Link>
                                        ) : (
                                            <div className="flex items-center gap-3 rounded-lg border border-transparent px-3 py-2.5 text-sm text-slate-500">
                                                <span className="text-base opacity-60">{item.icon ?? '•'}</span>
                                                <span>{item.label}</span>
                                                <StatusPill status={item.status} />
                                            </div>
                                        )}

                                        {item.children && item.children.length > 0 && (
                                            <div className="ml-8 flex flex-col gap-1">
                                                {item.children.map((child) => {
                                                    const isChildActive = child.href
                                                        ? pathname === child.href || pathname.startsWith(child.href + '/')
                                                        : false
                                                    if (child.href) {
                                                        return (
                                                            <Link
                                                                key={child.label}
                                                                href={child.href}
                                                                className={clsx(
                                                                    'flex items-center gap-2 rounded-md border px-2.5 py-1.5 text-xs transition-all',
                                                                    isChildActive
                                                                        ? 'bg-brand-600/10 text-brand-300 border-brand-500/20'
                                                                        : 'text-slate-400 hover:text-slate-200 hover:bg-surface-hover border-transparent',
                                                                )}
                                                            >
                                                                <span>{child.label}</span>
                                                                <StatusPill status={child.status} />
                                                            </Link>
                                                        )
                                                    }

                                                    return (
                                                        <div
                                                            key={child.label}
                                                            className="flex items-center gap-2 rounded-md border border-transparent px-2.5 py-1.5 text-xs text-slate-500"
                                                        >
                                                            <span>{child.label}</span>
                                                            <StatusPill status={child.status} />
                                                        </div>
                                                    )
                                                })}
                                            </div>
                                        )}
                                    </div>
                                )
                            })}
                        </div>
                    </section>
                ))}
            </div>
        </nav>
    )
}
