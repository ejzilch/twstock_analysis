import type { CandleItem } from '@/src/types/api.generated'
import { Time } from 'lightweight-charts'

// ── Date / Time ───────────────────────────────────────────────────────────────

export function formatTimestamp(ms: number): string {
    return new Intl.DateTimeFormat('zh-TW', {
        year: 'numeric', month: '2-digit', day: '2-digit',
        hour: '2-digit', minute: '2-digit',
        timeZone: 'Asia/Taipei',
    }).format(new Date(ms))
}

export function formatDate(ms: number): string {
    return new Intl.DateTimeFormat('zh-TW', {
        year: 'numeric', month: '2-digit', day: '2-digit',
        timeZone: 'Asia/Taipei',
    }).format(new Date(ms))
}

export function isMarketOpen(): boolean {
    const now = new Date()
    const taipei = new Date(now.toLocaleString('en-US', { timeZone: 'Asia/Taipei' }))
    const day = taipei.getDay()
    const hour = taipei.getHours()
    const min = taipei.getMinutes()
    const totalMin = hour * 60 + min
    const isWeekday = day >= 1 && day <= 5
    // 09:00 ~ 13:30 Taiwan time
    return isWeekday && totalMin >= 540 && totalMin <= 810
}

// ── Number Formatting ─────────────────────────────────────────────────────────

export function formatPrice(value: number): string {
    return new Intl.NumberFormat('zh-TW', {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
    }).format(value)
}

export function formatPercent(value: number, decimals = 2): string {
    const sign = value >= 0 ? '+' : ''
    return `${sign}${(value * 100).toFixed(decimals)}%`
}

export function formatVolume(volume: number): string {
    if (volume >= 1_000_000) return `${(volume / 1_000_000).toFixed(1)}M`
    if (volume >= 1_000) return `${(volume / 1_000).toFixed(0)}K`
    return String(volume)
}

export function formatCapital(value: number): string {
    return new Intl.NumberFormat('zh-TW', {
        style: 'currency',
        currency: 'TWD',
        minimumFractionDigits: 0,
    }).format(value)
}

// ── TradingView Format Conversion ─────────────────────────────────────────────

export interface TradingViewCandle {
    time: Time   // seconds (UTCTimestamp)
    open: number
    high: number
    low: number
    close: number
}

export interface TradingViewVolume {
    time: Time
    value: number
    color: string
}

/** Convert API candle (ms) to TradingView format (seconds) */
export function toTradingViewCandle(candle: CandleItem): TradingViewCandle {
    return {
        time: (candle.timestamp_ms / 1000) as Time,
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
    }
}

export function toTradingViewVolume(candle: CandleItem): TradingViewVolume {
    return {
        time: (candle.timestamp_ms / 1000) as Time,
        value: candle.volume,
        color: candle.close >= candle.open ? 'rgba(16,185,129,0.4)' : 'rgba(239,68,68,0.4)',
    }
}

// ── ID Generation ─────────────────────────────────────────────────────────────

export function generateRequestId(): string {
    const date = new Date().toISOString().slice(0, 10).replace(/-/g, '')
    const rand = Math.random().toString(36).slice(2, 8).toUpperCase()
    return `req-${date}-${rand}`
}