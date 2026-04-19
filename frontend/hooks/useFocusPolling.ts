'use client'
/**
 * useFocusPolling — manages refetchInterval based on window focus duration.
 * SPEC: pause polling when tab loses focus for > 5 minutes; resume immediately on focus restore.
 */
import { useState, useEffect, useRef } from 'react'
import { isMarketOpen } from '@/lib/utils'
import { useAppStore } from '@/store/useAppStore'

const NORMAL_INTERVAL_MS = 30_000         // 30 seconds during market hours
const ECO_INTERVAL_MS = 5 * 60_000     // 5 minutes after market close
const BACKGROUND_PAUSE_MS = 5 * 60_000     // pause after 5 min in background

export function useFocusPolling(): number | false {
    const isEco = useAppStore((s) => s.isEcoModeEnabled)
    const [paused, setPaused] = useState(false)
    const blurTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

    useEffect(() => {
        function handleBlur() {
            // Start 5-minute timer; if still blurred after that, pause polling
            blurTimerRef.current = setTimeout(() => {
                setPaused(true)
            }, BACKGROUND_PAUSE_MS)
        }

        function handleFocus() {
            // Cancel any pending blur timer and immediately resume
            if (blurTimerRef.current) {
                clearTimeout(blurTimerRef.current)
                blurTimerRef.current = null
            }
            setPaused(false)
        }

        window.addEventListener('blur', handleBlur)
        window.addEventListener('focus', handleFocus)

        return () => {
            window.removeEventListener('blur', handleBlur)
            window.removeEventListener('focus', handleFocus)
            if (blurTimerRef.current) clearTimeout(blurTimerRef.current)
        }
    }, [])

    if (paused) return false

    const marketOpen = isMarketOpen()
    if (!marketOpen && isEco) return ECO_INTERVAL_MS
    return NORMAL_INTERVAL_MS
}