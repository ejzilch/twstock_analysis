'use client'
/**
 * src/components/dashboard/SymbolSelector.tsx
 *
 * 與 SymbolSearchInput 結構完全對齊：
 *   - 單一 input 常駐，不做模式切換
 *   - 未輸入時 placeholder 顯示目前選中股票代號 + 名稱
 *   - 輸入時即時過濾，支援代號與中文名稱
 *   - 選取後清空 query，回到 placeholder 顯示狀態
 */
import { useState, useRef, useEffect } from 'react'
import { clsx } from 'clsx'
import { useSymbols } from '@/src/hooks'
import { useAppStore } from '@/src/store/useAppStore'
import type { SymbolItem } from '@/src/types/api.types'

const MAX_DROPDOWN_ITEMS = 10

export function SymbolSelector() {
    const symbol = useAppStore((s) => s.selectedSymbol)
    const setSymbol = useAppStore((s) => s.setSelectedSymbol)

    const { data: symbolsData, isLoading, isError } = useSymbols()
    const allSymbols = symbolsData?.symbols ?? []

    const [query, setQuery] = useState('')
    const [isOpen, setIsOpen] = useState(false)
    const containerRef = useRef<HTMLDivElement>(null)
    const inputRef = useRef<HTMLInputElement>(null)

    // 找到目前選中的股票完整資訊（用於 placeholder）
    const selectedItem = allSymbols.find((s) => s.symbol === symbol)

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
                setQuery('')
            }
        }
        document.addEventListener('mousedown', handleClickOutside)
        return () => document.removeEventListener('mousedown', handleClickOutside)
    }, [])

    function handleSelect(item: SymbolItem) {
        setSymbol(item.symbol)
        setQuery('')
        setIsOpen(false)
        inputRef.current?.focus()
    }

    function handleInputChange(value: string) {
        setQuery(value)
        setIsOpen(value.trim().length > 0)
    }

    function handleManualSubmit() {
        const code = query.trim()
        if (!code) return
        const exact = allSymbols.find((s) => s.symbol === code)
        if (exact) {
            handleSelect(exact)
        } else if (filtered.length > 0) {
            handleSelect(filtered[0])
        }
    }

    // Placeholder：載入中 / 錯誤 / 已選股票 / 預設
    const placeholder = isLoading
        ? '股票清單載入中...'
        : isError
            ? '載入失敗，請輸入股票代號'
            : selectedItem
                ? `${selectedItem.symbol}　${selectedItem.name}`
                : symbol
                    ? symbol
                    : '輸入股票代號或名稱...'

    return (
        <div ref={containerRef} className="relative">
            <div className={clsx(
                'flex items-center gap-2 bg-surface-card border rounded-lg px-3 py-1.5 transition-all min-w-[180px]',
                isOpen
                    ? 'border-brand-500/50 ring-2 ring-brand-500/20'
                    : 'border-surface-border',
            )}>
                {/* 載入中 spinner / 搜尋 icon */}
                {isLoading ? (
                    <span className="w-3.5 h-3.5 border-2 border-slate-600 border-t-slate-400 rounded-full animate-spin shrink-0" />
                ) : (
                    <span className="text-slate-500 text-sm shrink-0">🔍</span>
                )}

                <input
                    ref={inputRef}
                    type="text"
                    value={query}
                    onChange={(e) => handleInputChange(e.target.value)}
                    onFocus={() => query.trim().length > 0 && setIsOpen(true)}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter') {
                            e.preventDefault()
                            handleManualSubmit()
                        }
                        if (e.key === 'Escape') {
                            setQuery('')
                            setIsOpen(false)
                            inputRef.current?.blur()
                        }
                    }}
                    placeholder={placeholder}
                    className="flex-1 bg-transparent text-sm text-slate-200 placeholder-slate-500 focus:outline-none"
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
                <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-surface-card border border-surface-border rounded-xl shadow-2xl overflow-hidden animate-fade-in min-w-[240px]">
                    {filtered.map((item) => {
                        const isSelected = item.symbol === symbol
                        return (
                            <button
                                key={item.symbol}
                                onMouseDown={(e) => {
                                    e.preventDefault() // 防止 input blur 觸發 close
                                    handleSelect(item)
                                }}
                                className={clsx(
                                    'w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors',
                                    isSelected
                                        ? 'bg-brand-500/10 cursor-default'
                                        : 'hover:bg-surface-hover',
                                )}
                            >
                                <span className="font-mono text-sm font-medium text-slate-200 w-12 shrink-0">
                                    {item.symbol}
                                </span>
                                <span className="text-sm text-slate-400 flex-1 truncate">
                                    {item.name}
                                </span>
                                <span className={clsx(
                                    'text-xs px-1.5 py-0.5 rounded shrink-0',
                                    item.exchange === 'TWSE'
                                        ? 'bg-brand-500/15 text-brand-300'
                                        : 'bg-purple-500/15 text-purple-300',
                                )}>
                                    {item.exchange}
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
            {isOpen && query.trim().length > 0 && filtered.length === 0 && !isLoading && (
                <div className="absolute top-full left-0 right-0 mt-1 z-50 bg-surface-card border border-surface-border rounded-xl shadow-2xl px-4 py-3 animate-fade-in min-w-[240px]">
                    <p className="text-sm text-slate-500">找不到「{query}」相關股票</p>
                </div>
            )}
        </div>
    )
}