'use client'
/**
 * src/components/settings/SymbolSearchInput.tsx（修正版）
 *
 * 修正：
 *   - 不再自行呼叫 useSymbols()，改由父層（ManualSyncPanel）傳入 allSymbols
 *   - 避免雙重 fetch 與快取不同步問題
 *   - 加入 loading / error 狀態顯示
 */
import { useState, useRef, useEffect } from 'react'
import { clsx } from 'clsx'
import type { SymbolItem } from '@/src/types/api.generated'

interface SymbolSearchInputProps {
  /** 父層傳入的完整股票清單（來自 useSymbols）*/
  allSymbols: SymbolItem[]
  selectedSymbols: string[]
  onSelect: (symbol: SymbolItem) => void
  disabled?: boolean
  isLoading?: boolean
  isError?: boolean
}

const MAX_DROPDOWN_ITEMS = 10

export function SymbolSearchInput({
  allSymbols,
  selectedSymbols,
  onSelect,
  disabled = false,
  isLoading = false,
  isError = false,
}: SymbolSearchInputProps) {
  const [query, setQuery] = useState('')
  const [isOpen, setIsOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  // 本地過濾（直接用父層傳入的 allSymbols）
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

  // 決定 placeholder 文字
  const placeholder = isLoading
    ? '股票清單載入中...'
    : isError
      ? '股票清單載入失敗，請重新整理'
      : allSymbols.length === 0
        ? '尚無股票資料'
        : '輸入股票代號或名稱...'

  const isDisabled = disabled || isLoading || isError || allSymbols.length === 0

  return (
    <div ref={containerRef} className="relative">
      <div className={clsx(
        'flex items-center gap-2 bg-surface border rounded-lg px-3 py-2 transition-all',
        isOpen && !isDisabled
          ? 'border-brand-500/50 ring-2 ring-brand-500/20'
          : 'border-surface-border',
        isDisabled && 'opacity-50 cursor-not-allowed',
      )}>
        {/* 載入中 spinner / 搜尋 icon */}
        {isLoading ? (
          <span className="w-4 h-4 border-2 border-slate-600 border-t-slate-400 rounded-full animate-spin shrink-0" />
        ) : (
          <span className="text-slate-500 text-sm shrink-0">🔍</span>
        )}

        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => handleInputChange(e.target.value)}
          onFocus={() => query.trim().length > 0 && setIsOpen(true)}
          placeholder={placeholder}
          disabled={isDisabled}
          className="flex-1 bg-transparent text-sm text-slate-200 placeholder-slate-600 focus:outline-none disabled:cursor-not-allowed"
        />

        {query.length > 0 && !isDisabled && (
          <button
            onClick={() => { setQuery(''); setIsOpen(false) }}
            className="text-slate-600 hover:text-slate-400 text-lg leading-none shrink-0"
          >
            ×
          </button>
        )}
      </div>

      {/* 下拉候選清單 */}
      {isOpen && !isDisabled && filtered.length > 0 && (
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
      {isOpen && !isDisabled && query.trim().length > 0 && filtered.length === 0 && (
        <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-surface-card border border-surface-border rounded-xl shadow-2xl px-4 py-3 animate-fade-in">
          <p className="text-sm text-slate-500">找不到「{query}」相關股票</p>
        </div>
      )}
    </div>
  )
}