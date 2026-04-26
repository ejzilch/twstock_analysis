'use client'
import { useState, useEffect, useMemo } from 'react'
import { clsx } from 'clsx'
import type { CrosshairData, ChartSyncHandle } from '@/src/hooks/useChartSync'
import { INDICATOR_COLORS } from '@/src/constants/chartColors'
import type { ColorMode } from '@/src/constants/chartColors'

// ── Shared helpers ────────────────────────────────────────────────────────────

function fmt(n: number | null | undefined, decimals = 2): string {
    if (n == null) return '—'
    return n.toLocaleString('zh-TW', { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
}

function fmtVolume(n: number | null | undefined): string {
    if (n == null) return '—'
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(2) + 'M'
    if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
    return n.toFixed(0)
}

function fmtTime(ms: number | null): string {
    if (ms == null) return '—'
    const d = new Date(ms)
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

interface FieldProps { label: string; value: string; color?: string }

function Field({ label, value, color }: FieldProps) {
    return (
        <div className="flex items-center justify-between gap-3">
            <span className="text-slate-500 text-[10px] shrink-0">{label}</span>
            <span className="font-mono font-semibold text-[11px] tabular-nums" style={color ? { color } : undefined}>
                {value}
            </span>
        </div>
    )
}

function Divider() {
    return <div className="border-t border-slate-700/60 my-1" />
}

// ── Floating panel shell ──────────────────────────────────────────────────────

interface FloatPanelProps {
    x: number
    y: number
    containerWidth: number
    containerHeight: number
    children: React.ReactNode
}

const PANEL_W = 188
const OFFSET_X = 14
const OFFSET_Y = 14

function FloatPanel({ x, y, containerWidth, containerHeight, children }: FloatPanelProps) {
    // 靠右超出時往左翻
    const left = x + OFFSET_X + PANEL_W > containerWidth
        ? x - PANEL_W - OFFSET_X
        : x + OFFSET_X
    // 靠下超出時往上翻
    const top = y + OFFSET_Y + 300 > containerHeight
        ? y - OFFSET_Y - 8
        : y + OFFSET_Y

    return (
        <div
            className="absolute z-50 pointer-events-none select-none"
            style={{ left, top, width: PANEL_W }}
        >
            <div className="rounded-lg border border-slate-700/80 bg-slate-900/95 backdrop-blur-sm shadow-2xl shadow-black/60 px-2.5 py-2 text-[11px]">
                {children}
            </div>
        </div>
    )
}

// ── Hook ──────────────────────────────────────────────────────────────────────

function useCrosshairData(sync: ChartSyncHandle | undefined): CrosshairData | null {
    const [data, setData] = useState<CrosshairData | null>(null)
    useEffect(() => {
        if (!sync) return
        return sync.subscribeCrosshairData(setData)
    }, [sync])
    return data
}

// ── CandleTooltip ─────────────────────────────────────────────────────────────

interface CandleTooltipProps {
    sync: ChartSyncHandle | undefined
    colorMode?: ColorMode
    visibleIndicators?: Set<string>
    mousePos: { x: number; y: number } | null
    containerSize: { width: number; height: number }
}

export function CandleTooltip({ sync, colorMode = 'TW', visibleIndicators, mousePos, containerSize }: CandleTooltipProps) {
    const data = useCrosshairData(sync)
    const show = (key: string) => !visibleIndicators || visibleIndicators.has(key)

    const upColor = colorMode === 'TW' ? '#ef4444' : '#10b981'
    const downColor = colorMode === 'TW' ? '#10b981' : '#ef4444'

    const closeColor = useMemo(() => {
        if (!data || data.open == null || data.close == null) return undefined
        return data.close >= data.open ? upColor : downColor
    }, [data, upColor, downColor])

    const changePct = useMemo(() => {
        if (!data || data.open == null || data.close == null || data.open === 0) return null
        return ((data.close - data.open) / data.open) * 100
    }, [data])

    if (!data || !mousePos) return null

    const ma5 = data.indicators?.['ma5'] as number | undefined
    const ma20 = data.indicators?.['ma20'] as number | undefined
    const ma50 = data.indicators?.['ma50'] as number | undefined
    const boll = data.indicators?.['bollinger'] as { upper: number; middle: number; lower: number } | undefined

    return (
        <FloatPanel x={0} y={0} containerWidth={containerSize.width} containerHeight={containerSize.height}>
            {/* Time */}
            <div className="text-slate-400 text-[10px] font-mono mb-1.5 whitespace-nowrap">
                {fmtTime(data.timestamp_ms)}
            </div>

            {/* OHLCV */}
            <Field label="開盤 (Open)" value={fmt(data.open)} />
            <Field label="最高 (Highest)" value={fmt(data.high)} />
            <Field label="最低 (Lowest)" value={fmt(data.low)} />
            <Field label="收盤 (Close)" value={fmt(data.close)} color={closeColor} />
            {changePct != null && (
                <Field
                    label="漲跌"
                    value={`${changePct >= 0 ? '+' : ''}${changePct.toFixed(2)}%`}
                    color={changePct >= 0 ? upColor : downColor}
                />
            )}
            {data.volume != null && (
                <Field label="量 V" value={fmtVolume(data.volume)} />
            )}

            {/* MA */}
            {(show('ma5') && ma5 != null) || (show('ma20') && ma20 != null) || (show('ma50') && ma50 != null) ? (
                <>
                    <Divider />
                    {show('ma5') && ma5 != null && <Field label="MA5" value={fmt(ma5)} color={INDICATOR_COLORS.ma5} />}
                    {show('ma20') && ma20 != null && <Field label="MA20" value={fmt(ma20)} color={INDICATOR_COLORS.ma20} />}
                    {show('ma50') && ma50 != null && <Field label="MA50" value={fmt(ma50)} color={INDICATOR_COLORS.ma50} />}
                </>
            ) : null}

            {/* Bollinger */}
            {show('bollinger') && boll != null && (
                <>
                    <Divider />
                    <Field label="BB ↑" value={fmt(boll.upper)} color={INDICATOR_COLORS.bollUpper} />
                    <Field label="BB —" value={fmt(boll.middle)} color={INDICATOR_COLORS.bollMid} />
                    <Field label="BB ↓" value={fmt(boll.lower)} color={INDICATOR_COLORS.bollLower} />
                </>
            )}
        </FloatPanel>
    )
}

// ── RsiTooltip ────────────────────────────────────────────────────────────────

interface RsiTooltipProps {
    sync: ChartSyncHandle | undefined
    mousePos: { x: number; y: number } | null
    containerSize: { width: number; height: number }
}

export function RsiTooltip({ sync, mousePos, containerSize }: RsiTooltipProps) {
    const data = useCrosshairData(sync)
    const rsi = data?.indicators?.['rsi14'] as number | undefined

    if (rsi == null || !mousePos) return null

    const color =
        rsi >= 70 ? INDICATOR_COLORS.macdLine
            : rsi <= 30 ? INDICATOR_COLORS.signal
                : INDICATOR_COLORS.rsi

    const label = rsi >= 70 ? '超買' : rsi <= 30 ? '超賣' : null

    return (
        <FloatPanel x={mousePos.x} y={mousePos.y} containerWidth={containerSize.width} containerHeight={containerSize.height}>
            <div className="text-slate-500 text-[10px] mb-1.5 uppercase tracking-wider">RSI (14)</div>
            <div className="flex items-center justify-between gap-2">
                <span className="font-mono font-bold text-sm tabular-nums" style={{ color }}>{rsi.toFixed(2)}</span>
                {label && (
                    <span className="text-[9px] px-1.5 py-0.5 rounded font-semibold"
                        style={{ color, background: `${color}22` }}>
                        {label}
                    </span>
                )}
            </div>
        </FloatPanel>
    )
}

// ── MacdTooltip ───────────────────────────────────────────────────────────────

interface MacdTooltipProps {
    sync: ChartSyncHandle | undefined
    mousePos: { x: number; y: number } | null
    containerSize: { width: number; height: number }
}

export function MacdTooltip({ sync, mousePos, containerSize }: MacdTooltipProps) {
    const data = useCrosshairData(sync)
    const macd = data?.indicators?.['macd'] as
        | { macd_line: number; signal_line: number; histogram: number }
        | undefined

    if (!macd || !mousePos) return null

    const oscColor = macd.histogram >= 0 ? '#4ade80' : '#f87171'

    return (
        <FloatPanel x={mousePos.x} y={mousePos.y} containerWidth={containerSize.width} containerHeight={containerSize.height}>
            <div className="text-slate-500 text-[10px] mb-1.5 uppercase tracking-wider">MACD (12,26,9)</div>
            <Field label="DIF" value={fmt(macd.macd_line, 4)} color={INDICATOR_COLORS.macdLine} />
            <Field label="DEA" value={fmt(macd.signal_line, 4)} color={INDICATOR_COLORS.signal} />
            <Field label="OSC" value={fmt(macd.histogram, 4)} color={oscColor} />
        </FloatPanel>
    )
}