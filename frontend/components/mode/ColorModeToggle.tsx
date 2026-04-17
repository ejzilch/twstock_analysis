'use client'

import { useAppStore } from '@/store/useAppStore'

export function ColorModeToggle() {
    const colorMode = useAppStore((s) => s.colorMode)
    const toggleColorMode = useAppStore((s) => s.toggleColorMode)

    return (
        <button
            onClick={toggleColorMode}
            className="flex items-center justify-between w-full px-3 py-2.5 mt-2 rounded-xl text-sm font-medium text-slate-400 bg-surface-hover/50 border border-surface-border hover:text-white hover:bg-slate-800/80 transition-all group"
        >
            <span className="flex items-center gap-2">
                <span className="text-lg opacity-70 group-hover:opacity-100 transition-opacity">🎨</span>
                漲跌配色
            </span>
            <span className="font-mono text-[14px] tracking-wider bg-surface/80 px-2 py-1 rounded-md">
                {colorMode === 'TW' ? (
                    <><span className="text-red-500">紅漲</span> / <span className="text-emerald-500">綠跌</span></>
                ) : (
                    <><span className="text-emerald-500">綠漲</span> / <span className="text-red-500">紅跌</span></>
                )}
            </span>
        </button>
    )
}