// ── 固定指標顏色 ──────────────────────────────────────────────────────────────
export const INDICATOR_COLORS = {
    ma5: '#2196F3',
    ma20: '#FF9800',
    ma50: '#9C27B0',
    bollUpper: '#F44336',
    bollMid: '#78909C',
    bollLower: '#4CAF50',
    rsi: '#00BCD4',
} as const

// ── K 線漲跌顏色（依 colorMode 切換）─────────────────────────────────────────
// TW 模式：紅漲綠跌（台灣慣例）
// US 模式：綠漲紅跌（歐美慣例）
export type ColorMode = 'TW' | 'US'

export interface CandleColorSet {
    up: string   // 上漲實體 + 影線
    down: string   // 下跌實體 + 影線
    upVolume: string   // 上漲成交量
    downVolume: string  // 下跌成交量
}

const TW_COLORS: CandleColorSet = {
    up: '#ef4444',
    down: '#10b981',
    upVolume: 'rgba(239,68,68,0.4)',
    downVolume: 'rgba(16,185,129,0.4)',
}

const US_COLORS: CandleColorSet = {
    up: '#10b981',
    down: '#ef4444',
    upVolume: 'rgba(16,185,129,0.4)',
    downVolume: 'rgba(239,68,68,0.4)',
}

export function getCandleColors(colorMode: ColorMode): CandleColorSet {
    return colorMode === 'TW' ? TW_COLORS : US_COLORS
}

export interface MacdRsiColorSet {
    netBuy: string
    netSell: string
    macdLine: string
    signal: string
    histPos: string
    histNeg: string
}

export function getMacdRsiColors(colorMode: ColorMode): MacdRsiColorSet {
    if (colorMode === 'TW') {
        return {
            netBuy: '#ef4444',
            netSell: '#10b981',
            macdLine: '#ef4444',   // 紅漲 快線紅
            signal: '#10b981',   // 綠跌 慢線綠
            histPos: 'rgba(239,68,68,0.6)',
            histNeg: 'rgba(16,185,129,0.6)',
        }
    }
    return {
        netBuy: '#10b981',
        netSell: '#ef4444',
        macdLine: '#10b981',   // 綠漲 快線綠
        signal: '#ef4444',   // 紅跌 慢線紅
        histPos: 'rgba(16,185,129,0.6)',
        histNeg: 'rgba(239,68,68,0.6)',
    }
}

// ── 圖表背景 / 格線顏色 ───────────────────────────────────────────────────────
export const CHART_THEME = {
    background: '#161b27',
    textColor: '#94a3b8',
    gridLine: '#1e2a3a',
    borderColor: '#1e2a3a',
} as const

// ── 訊號 BUY / SELL 顏色 ──────────────────────────────────────────────────────
export const SIGNAL_TYPE = {
    buy: '#4CAF50',
    sell: '#F44336',
} as const