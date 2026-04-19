'use client'
/**
 * Zustand global UI state store.
 * RULE: Only UI state lives here. Server data (candles/signals/symbols) belongs in React Query.
 */
import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { AppState } from '@/src/types/app'

export const useAppStore = create<AppState>()(
    persist(
        (set) => ({
            selectedSymbol: '2330',
            selectedInterval: '1h',
            isEcoModeEnabled: true,
            apiKey: '',

            setSelectedSymbol: (symbol) => set({ selectedSymbol: symbol }),
            setSelectedInterval: (interval) => set({ selectedInterval: interval }),
            toggleEcoMode: () => set((s) => ({ isEcoModeEnabled: !s.isEcoModeEnabled })),
            setApiKey: (key) => {
                if (typeof window !== 'undefined') localStorage.setItem('ai_bridge_api_key', key)
                set({ apiKey: key })
            },
            colorMode: 'TW',
            toggleColorMode: () => set((s) => ({ colorMode: s.colorMode === 'TW' ? 'US' : 'TW' })),
        }),
        {
            name: 'ai-bridge-ui',
            partialize: (s) => ({
                selectedSymbol: s.selectedSymbol,
                selectedInterval: s.selectedInterval,
                isEcoModeEnabled: s.isEcoModeEnabled,
                apiKey: s.apiKey,
                colorMode: s.colorMode,
            }),
        },
    ),
)