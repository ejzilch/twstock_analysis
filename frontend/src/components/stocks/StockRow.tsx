import { clsx } from 'clsx'
import type { SymbolItem } from '@/src/types/api.types'

interface StockRowProps { symbol: SymbolItem }

export function StockRow({ symbol }: StockRowProps) {
    return (
        <tr className="hover:bg-surface-hover transition-colors">
            <td className="px-5 py-3.5 font-mono font-medium text-slate-200">{symbol.symbol}</td>
            <td className="px-5 py-3.5 text-slate-300">{symbol.name}</td>
            <td className="px-5 py-3.5">
                <span className={clsx(
                    'inline-flex px-2 py-0.5 rounded text-xs font-medium',
                    symbol.exchange === 'TWSE'
                        ? 'bg-brand-500/15 text-brand-300'
                        : 'bg-purple-500/15 text-purple-300',
                )}>
                    {symbol.exchange}
                </span>
            </td>
            <td className="px-5 py-3.5 text-xs text-slate-500">{symbol.data_source}</td>
            <td className="px-5 py-3.5">
                <span className={clsx(
                    'inline-flex items-center gap-1.5 text-xs',
                    symbol.is_active ? 'text-emerald-400' : 'text-slate-500',
                )}>
                    <span className={clsx('w-1.5 h-1.5 rounded-full', symbol.is_active ? 'bg-emerald-400' : 'bg-slate-600')} />
                    {symbol.is_active ? '活躍' : '下市'}
                </span>
            </td>
        </tr>
    )
}