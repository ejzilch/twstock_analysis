'use client'
/**
 * src/components/settings/SelectedSymbolTags.tsx
 *
 * 已選股票標籤列表，每個標籤含移除按鈕。
 * 純 props，無任何 API 或 store 依賴。
 */
import { clsx } from 'clsx'
import type { SymbolItem } from '@/src/types/api.generated'

interface SelectedSymbolTagsProps {
  selected: SymbolItem[]
  onRemove: (symbol: string) => void
  disabled?: boolean
}

export function SelectedSymbolTags({
  selected,
  onRemove,
  disabled = false,
}: SelectedSymbolTagsProps) {
  if (selected.length === 0) {
    return (
      <p className="text-xs text-slate-600 py-1">尚未選擇任何股票</p>
    )
  }

  return (
    <div className="flex flex-wrap gap-2">
      {selected.map((symbol) => (
        <span
          key={symbol.symbol}
          className={clsx(
            'inline-flex items-center gap-1.5 pl-2.5 pr-1.5 py-1 rounded-lg text-xs font-medium',
            'bg-brand-600/15 text-brand-300 border border-brand-500/20',
            'transition-all',
          )}
        >
          <span className="font-mono">{symbol.symbol}</span>
          <span className="text-brand-400/70">{symbol.name}</span>
          <button
            onClick={() => onRemove(symbol.symbol)}
            disabled={disabled}
            aria-label={`移除 ${symbol.name}`}
            className={clsx(
              'ml-0.5 w-4 h-4 rounded flex items-center justify-center',
              'text-brand-400/60 hover:text-brand-200 hover:bg-brand-500/20',
              'transition-colors leading-none',
              disabled && 'opacity-50 cursor-not-allowed',
            )}
          >
            ×
          </button>
        </span>
      ))}
    </div>
  )
}
