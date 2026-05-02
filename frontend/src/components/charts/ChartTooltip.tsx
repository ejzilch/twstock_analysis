'use client'
import { useState, useEffect, useMemo } from 'react'
import type { CrosshairData, ChartSyncHandle } from '@/src/hooks/useChartSync'
import type { ColorMode, ThemedIndicatorColorsSet } from '@/src/constants/chartColors'
import { getCandleColors, getThemedIndicatorColor, BASE_INDICATOR_COLORS } from '@/src/constants/chartColors'

// ── Shared helpers ────────────────────────────────────────────────────────────

function fmt(n: number | null | undefined, decimals = 2): string {
    if (n == null) return '—'
    return n.toLocaleString('zh-TW', { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
}

function fmtVolume(n: number | null | undefined): string {
    if (n == null) return '—'
    // if (n >= 1_000_000) return (n / 1_000_000).toFixed(2) + 'M'
    if (n >= 1_000) return (n / 1_000).toFixed(0) + 'K'
    return n.toFixed(0)
}

function fmtTime(ms: number | null): string {
    if (ms == null) return '—'
    const d = new Date(ms)
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

// ── Field row ─────────────────────────────────────────────────────────────────

interface FieldProps { label: string; value: string; color?: string; mono?: boolean }

function Field({ label, value, color, mono = true }: FieldProps) {
    return (
        <div className="grid grid-cols-[auto_1fr] items-center gap-3 leading-none">
            <span className="text-slate-500 text-[16px] shrink-0 tracking-wide ">{label}</span>
            <span
                className={`text-[16px] tabular-nums font-semibold text-right ${mono ? 'font-mono' : ''}`}
                style={color ? { color } : { color: '#cbd5e1' }}
            >
                {value}
            </span>
        </div>
    )
}

function Divider() {
    return <div className="border-t border-slate-700/50 my-1.5" />
}

// ── Anchor panel — fixed top-left inside a relative container ─────────────────

interface AnchorPanelProps {
    children: React.ReactNode
    /** extra Tailwind classes, e.g. to set width */
    className?: string
    visible: boolean
}

function AnchorPanel({ children, className = '', visible }: AnchorPanelProps) {
    return (
        <div
            className={`
                absolute top-2 left-2 z-40 pointer-events-none select-none
                transition-opacity duration-150
                ${visible ? 'opacity-100' : 'opacity-0'}
                ${className}
            `}
        >
            {/* Glass card */}
            <div
                className="rounded-md px-2.5 py-2 text-[11px] space-y-1"
                style={{
                    background: 'rgba(15, 20, 35, 0.82)',
                    backdropFilter: 'blur(8px)',
                    border: '1px solid rgba(255,255,255,0.06)',
                    boxShadow: '0 4px 24px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.04)',
                    minWidth: 172,
                }}
            >
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

// ── CandleTooltip — 固定在 K 線圖左上 ────────────────────────────────────────

interface CandleTooltipProps {
    sync: ChartSyncHandle | undefined
    colorMode?: ColorMode
    visibleIndicators?: Set<string>
}

export function CandleTooltip({
    sync,
    colorMode = 'TW',
    visibleIndicators,
}: CandleTooltipProps) {
    const data = useCrosshairData(sync)
    const show = (key: string) => !visibleIndicators || visibleIndicators.has(key)

    const candleColor = getCandleColors(colorMode)
    const themedIndicatorColor = getThemedIndicatorColor(colorMode)

    const closeColor = useMemo(() => {
        if (!data || data.open == null || data.close == null || data.prevClose == null) return undefined
        return data.close === data.prevClose ? candleColor.unchanged : data.close > data.prevClose ? candleColor.up : candleColor.down;
    }, [data])

    const upOrDown = useMemo(() => {
        if (!data || data.close == null) return null
        const base = data.prevClose ?? data.open
        if (base == null || base === 0) return null
        return data.close - base
    }, [data])

    const changePct = useMemo(() => {
        if (!data || data.close == null) return null
        const base = data.prevClose ?? data.open
        if (base == null || base === 0) return null
        return ((data.close - base) / base) * 100
    }, [data])

    const ma5 = data?.indicators?.['ma5'] as number | undefined
    const ma20 = data?.indicators?.['ma20'] as number | undefined
    const ma50 = data?.indicators?.['ma50'] as number | undefined
    const boll = data?.indicators?.['bollinger'] as
        | { upper: number; middle: number; lower: number }
        | undefined
    const rsi = data?.indicators?.['rsi14'] as number | undefined
    const macd = data?.indicators?.['macd'] as
        | { macd_line: number; signal_line: number; histogram: number }
        | undefined
    const hasMA = (show('ma5') && ma5 != null) || (show('ma20') && ma20 != null) || (show('ma50') && ma50 != null)
    const hasBoll = show('bollinger') && boll != null

    return (
        <AnchorPanel visible={data != null} className="w-[220px]">
            {/* Timestamp */}
            <div className="text-[10px] font-mono text-slate-500 mb-1.5 whitespace-nowrap">
                {fmtTime(data?.timestamp_ms ?? null)}
            </div>

            {/* OHLCV */}
            <Field label="最高 Highest" value={fmt(data?.high)} />
            <Field label="最低 Lowest" value={fmt(data?.low)} />
            <Field label="開盤 Open" value={fmt(data?.open)} />
            <Field label="收盤 Close" value={fmt(data?.close)} color={closeColor} />
            {upOrDown != null && (
                <Field
                    label="漲跌"
                    value={`${upOrDown >= 0 ? '+' : ''}${upOrDown.toFixed(2)}`}
                    color={upOrDown === 0 ? candleColor.unchanged : upOrDown > 0 ? candleColor.up : candleColor.down}
                />
            )}
            {changePct != null && (
                <Field
                    label="漲跌%"
                    value={`${changePct >= 0 ? '+' : ''}${changePct.toFixed(2)}%`}
                    color={changePct === 0 ? candleColor.unchanged : changePct > 0 ? candleColor.up : candleColor.down}
                />
            )}
            {data?.volume != null && (
                <Field label="成交量 Volume" value={fmtVolume(data.volume)} />
            )}

            {/* MA */}
            {hasMA && (
                <>
                    <Divider />
                    {show('ma5') && ma5 != null && <Field label="MA5" value={fmt(ma5)} color={BASE_INDICATOR_COLORS.ma5} />}
                    {show('ma20') && ma20 != null && <Field label="MA20" value={fmt(ma20)} color={BASE_INDICATOR_COLORS.ma20} />}
                    {show('ma50') && ma50 != null && <Field label="MA50" value={fmt(ma50)} color={BASE_INDICATOR_COLORS.ma50} />}
                </>
            )}

            {/* Bollinger */}
            {hasBoll && (
                <>
                    <Divider />
                    <Field label="BOLL 上軌" value={fmt(boll!.upper)} color={themedIndicatorColor.bollUpper} />
                    <Field label="BOLL 中軌" value={fmt(boll!.middle)} color={BASE_INDICATOR_COLORS.bollMid} />
                    <Field label="BOLL 下軌" value={fmt(boll!.lower)} color={themedIndicatorColor.bollLower} />
                </>
            )}

            {/* RSI / MACD */}
            {(rsi != null || macd != null) && (
                <>
                    <Divider />
                    {rsi != null && <Field label="RSI14" value={fmt(rsi)} color={BASE_INDICATOR_COLORS.rsi} />}
                    {macd != null && (
                        <>
                            <Field label="DIF" value={fmt(macd.macd_line)} color={themedIndicatorColor.macdLine} />
                            <Field label="DEA" value={fmt(macd.signal_line)} color={themedIndicatorColor.signal} />
                            <Field label="OSC" value={fmt(macd.histogram)} color={macd.histogram >= 0 ? themedIndicatorColor.histPos : themedIndicatorColor.histNeg} />
                        </>
                    )}
                </>
            )}
        </AnchorPanel>
    )
}

// ── RsiTooltip — 固定在 RSI 圖左上 ───────────────────────────────────────────

interface RsiTooltipProps {
    sync: ChartSyncHandle | undefined
    mc: ThemedIndicatorColorsSet
}

export function RsiTooltip({ sync, mc }: RsiTooltipProps) {
    const data = useCrosshairData(sync)
    const rsi = data?.indicators?.['rsi14'] as number | undefined

    const color =
        rsi == null ? BASE_INDICATOR_COLORS.rsi
            : rsi >= 70 ? mc.macdLine
                : rsi <= 30 ? mc.signal
                    : BASE_INDICATOR_COLORS.rsi

    const zone =
        rsi == null ? null
            : rsi >= 70 ? '超買'
                : rsi <= 30 ? '超賣'
                    : null

    return (
        <AnchorPanel visible={rsi != null}>
            {/* Header */}
            <div className="flex items-center justify-between mb-1.5 gap-2">
                <span className="text-[10px] uppercase py-1.5 tracking-widest text-slate-500">RSI 14</span>
                {zone && (
                    <span
                        className="text-[16px] px-1.5 py-0.5 rounded font-bold tracking-wide"
                        style={{ color, background: `${color}22` }}
                    >
                        {zone}
                    </span>
                )}
            </div>

            {/* Value */}
            <div className="font-mono font-bold text-lg tabular-nums leading-none" style={{ color }}>
                {rsi?.toFixed(2) ?? '—'}
            </div>

            {/* Visual bar */}
            <div className="mt-2 h-1 rounded-full bg-slate-700/60 overflow-hidden">
                <div
                    className="h-full rounded-full transition-all duration-100"
                    style={{
                        width: `${Math.min(100, Math.max(0, rsi ?? 0))}%`,
                        background: color,
                        opacity: 0.75,
                    }}
                />
            </div>
            <div className="flex justify-between mt-0.5">
                <span className="text-[9px] text-slate-600">0</span>
                <span className="text-[9px] text-slate-600">30</span>
                <span className="text-[9px] text-slate-600">70</span>
                <span className="text-[9px] text-slate-600">100</span>
            </div>
        </AnchorPanel>
    )
}

// ── MacdTooltip — 固定在 MACD 圖左上 ─────────────────────────────────────────

interface MacdTooltipProps {
    sync: ChartSyncHandle | undefined
    mc: ThemedIndicatorColorsSet | undefined
}

export function MacdTooltip({ sync, mc }: MacdTooltipProps) {
    const data = useCrosshairData(sync)

    const macd = data?.indicators?.['macd'] as
        | { macd_line: number; signal_line: number; histogram: number }
        | undefined

    const oscColor = !macd ? '#94a3b8' : macd.histogram >= 0 ? mc?.histPos : mc?.histNeg
    const cross =
        !macd ? null
            : macd.macd_line > macd.signal_line ? { label: '多頭排列', color: mc?.macdLine }
                : { label: '空頭排列', color: mc?.signal }

    return (
        <AnchorPanel visible={macd != null}>
            {/* Header */}
            <div className="flex items-center justify-between mb-1.5 gap-2">
                <span className="text-[10px] uppercase py-1.5 tracking-widest text-slate-500">MACD 12·26·9</span>
                {cross && (
                    <span
                        className="text-[16px] px-1.5 py-0.5 rounded font-bold tracking-wide"
                        style={{ color: cross.color, background: `${cross.color}22` }}
                    >
                        {cross.label}
                    </span>
                )}
            </div>

            <Field label="快速線 DIF" value={fmt(macd?.macd_line, 4)} color={mc?.macdLine} />
            <Field label="慢速線 DEA" value={fmt(macd?.signal_line, 4)} color={mc?.signal} />
            <Field label="柱體值 OSC" value={fmt(macd?.histogram, 4)} color={oscColor} />
        </AnchorPanel>
    )
}