'use client'
/**
 * src/components/settings/SymbolSearchInput.tsx
 *
 * 股票搜尋輸入框 + 下拉候選清單。
 *
 * 規則：
 *   - 資料來自 useSymbols()（已有 hook），不另打 API
 *   - 前端本地過濾，無額外請求
 *   - 下拉最多顯示 10 筆
 *   - 已選股票顯示打勾，點擊無效果
 */
import { useState, useRef, useEffect } from 'react'
import { clsx } from 'clsx'
import { useSymbols } from '@/src/hooks'
import type { SymbolItem } from '@/src/types/api.generated'

interface SymbolSearchInputProps {
  selectedSymbols: string[]
  onSelect:        (symbol: SymbolItem) => void
  disabled?:       boolean
}

const MAX_DROPDOWN_ITEMS = 10

export function SymbolSearchInput({
  selectedSymbols,
  onSelect,
  disabled = false,
}: SymbolSearchInputProps) {
  const [query,     setQuery]     = useState('')
  const [isOpen,    setIsOpen]    = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const inputRef     = useRef<HTMLInputElement>(null)

  const { data: symbolsData } = useSymbols()
  const allSymbols = symbolsData?.symbols ?? []

  // 本地過濾
  const filtered = query.trim().length === 0
    ? []
    : allSymbols
        .filter((s) =>
          s.symbol.includes(query.trim()) ||
          s.name.includes(query.trim())
        )
        .slice(0, MAX_DROPDOWN_ITEMS)

  // 點擊外部關閉下拉
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  function handleSelect(symbol: SymbolItem) {
    if (selectedSymbols.includes(symbol.symbol)) return
    onSelect(symbol)
    setQuery('')
    setIsOpen(false)
    inputRef.current?.focus()
  }

  function handleInputChange(value: string) {
    setQuery(value)
    setIsOpen(value.trim().length > 0)
  }

  return (
    <div ref={containerRef} className="relative">
      <div className={clsx(
        'flex items-center gap-2 bg-surface border rounded-lg px-3 py-2 transition-all',
        isOpen
          ? 'border-brand-500/50 ring-2 ring-brand-500/20'
          : 'border-surface-border',
        disabled && 'opacity-50 cursor-not-allowed',
      )}>
        <span className="text-slate-500 text-sm shrink-0">🔍</span>
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => handleInputChange(e.target.value)}
          onFocus={() => query.trim().length > 0 && setIsOpen(true)}
          placeholder="輸入股票代號或名稱..."
          disabled={disabled}
          className="flex-1 bg-transparent text-sm text-slate-200 placeholder-slate-600 focus:outline-none"
        />
        {query.length > 0 && (
          <button
            onClick={() => { setQuery(''); setIsOpen(false) }}
            className="text-slate-600 hover:text-slate-400 text-lg leading-none shrink-0"
          >
            ×
          </button>
        )}
      </div>

      {/* 下拉候選清單 */}
      {isOpen && filtered.length > 0 && (
        <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-surface-card border border-surface-border rounded-xl shadow-2xl overflow-hidden animate-fade-in">
          {filtered.map((symbol) => {
            const isSelected = selectedSymbols.includes(symbol.symbol)
            return (
              <button
                key={symbol.symbol}
                onClick={() => handleSelect(symbol)}
                disabled={isSelected}
                className={clsx(
                  'w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors',
                  isSelected
                    ? 'opacity-50 cursor-not-allowed'
                    : 'hover:bg-surface-hover',
                )}
              >
                <span className="font-mono text-sm font-medium text-slate-200 w-12 shrink-0">
                  {symbol.symbol}
                </span>
                <span className="text-sm text-slate-400 flex-1 truncate">
                  {symbol.name}
                </span>
                <span className={clsx(
                  'text-xs px-1.5 py-0.5 rounded shrink-0',
                  symbol.exchange === 'TWSE'
                    ? 'bg-brand-500/15 text-brand-300'
                    : 'bg-purple-500/15 text-purple-300',
                )}>
                  {symbol.exchange}
                </span>
                {isSelected && (
                  <span className="text-emerald-400 text-sm shrink-0">✓</span>
                )}
              </button>
            )
          })}

          {filtered.length === MAX_DROPDOWN_ITEMS && (
            <div className="px-4 py-2 text-xs text-slate-600 border-t border-surface-border">
              顯示前 {MAX_DROPDOWN_ITEMS} 筆，繼續輸入可縮小結果
            </div>
          )}
        </div>
      )}

      {/* 無結果提示 */}
      {isOpen && query.trim().length > 0 && filtered.length === 0 && (
        <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-surface-card border border-surface-border rounded-xl shadow-2xl px-4 py-3 animate-fade-in">
          <p className="text-sm text-slate-500">找不到「{query}」相關股票</p>
        </div>
      )}
    </div>
  )
}
