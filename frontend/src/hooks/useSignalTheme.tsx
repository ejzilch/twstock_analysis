import { useAppStore } from '@/src/store/useAppStore'

export function useSignalTheme() {
    const colorMode = useAppStore((s) => s.colorMode)

    const getTheme = (type: 'BUY' | 'SELL') => {
        // 判斷這個情境下是否應該顯示紅色
        const isRed =
            (colorMode === 'TW' && type === 'BUY') ||
            (colorMode === 'US' && type === 'SELL')

        if (isRed) {
            return {
                bg: 'bg-red-500/15',
                bgLight: 'bg-red-500/5', // PredictionPanel 的淺色背景
                text: 'text-red-400',
                border: 'border-red-500/20',
                bar: 'bg-red-500',
            }
        } else {
            return {
                bg: 'bg-emerald-500/15',
                bgLight: 'bg-emerald-500/5', // PredictionPanel 的淺色背景
                text: 'text-emerald-400',
                border: 'border-emerald-500/20',
                bar: 'bg-emerald-500',
            }
        }
    }

    return getTheme
}