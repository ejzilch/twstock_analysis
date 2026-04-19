/**
 * Business layer derived types.
 * Composed from api.generated.ts via Pick<>/Omit<> — never duplicate interface definitions.
 */
import type {
    CandleItem,
    SignalItem,
    SymbolItem,
    MacdValue,
    BollingerValue,
    ReliabilityLevel,
} from './api.generated'

// Candle with guaranteed indicator shapes for chart rendering
export type CandleWithIndicators = CandleItem & {
    indicators: {
        ma5?: number
        ma20?: number
        ma50?: number
        rsi?: number
        macd?: MacdValue
        bollinger?: BollingerValue
    }
}

// Lightweight symbol for selector UI
export type SymbolOption = Pick<SymbolItem, 'symbol' | 'name' | 'exchange'>

// Signal enriched with display metadata
export type SignalWithDisplay = SignalItem & {
    displayLabel: string
    displayColor: string
}

// Badge configuration derived from reliability
export interface BadgeConfig {
    label: string
    color: 'green' | 'yellow' | 'gray' | 'red'
    bg: string
    text: string
}

export const RELIABILITY_BADGE: Record<ReliabilityLevel, BadgeConfig> = {
    high: { label: 'AI 高信心', color: 'green', bg: 'bg-emerald-500/15', text: 'text-emerald-400' },
    medium: { label: 'AI 中信心', color: 'yellow', bg: 'bg-amber-500/15', text: 'text-amber-400' },
    low: { label: '技術指標', color: 'gray', bg: 'bg-slate-500/15', text: 'text-slate-400' },
    unknown: { label: '信號異常', color: 'red', bg: 'bg-red-500/15', text: 'text-red-400' },
}

// Zustand store shape
export interface AppState {
    selectedSymbol: string
    selectedInterval: string
    isEcoModeEnabled: boolean
    apiKey: string
    colorMode: 'TW' | 'US'
    toggleColorMode: () => void
    setSelectedSymbol: (symbol: string) => void
    setSelectedInterval: (interval: string) => void
    toggleEcoMode: () => void
    setApiKey: (key: string) => void
}