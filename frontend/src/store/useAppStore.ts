'use client'
/**
 * Zustand global UI state store.
 * RULE: Only UI state lives here. Server data (candles/signals/symbols) belongs in React Query.
 */
import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type {
    AppState,
    AppStorePersistedState,
    DashboardIndicatorId,
    DashboardLayoutState,
    DashboardLeftPanelId,
    DashboardRange,
    DashboardRightGridPreset,
    DashboardRightWidgetId,
    DashboardRightWidgetLayout,
} from '@/src/types/api.types'

const APP_STORE_PERSIST_VERSION = 2

const VALID_INTERVALS = new Set(['1m', '5m', '15m', '1h', '4h', '1d'])
const DASHBOARD_LEFT_PANEL_IDS: DashboardLeftPanelId[] = ['candles', 'rsi', 'macd', 'institutionalNetFlow']
const DASHBOARD_INDICATOR_IDS: DashboardIndicatorId[] = ['ma5', 'ma20', 'ma50', 'bollinger', 'rsi', 'macd']
const DASHBOARD_RIGHT_WIDGET_IDS: DashboardRightWidgetId[] = ['aiPrediction', 'shareholdingRatio', 'monthlyRevenue', 'peAnalysis', 'signalList']
const DASHBOARD_GRID_COLUMNS: Record<DashboardRightGridPreset, number> = {
    '1x1': 1,
    '2x2': 2,
    '3x3': 3,
    '4x4': 4,
}

const DEFAULT_DASHBOARD_LAYOUT: DashboardLayoutState = {
    splitRatio: 0.72,
    selectedRange: 'max',
    leftPanelOrder: ['candles', 'rsi', 'macd', 'institutionalNetFlow'],
    leftPanelVisible: {
        candles: true,
        rsi: true,
        macd: true,
        institutionalNetFlow: false,
    },
    indicatorVisible: {
        ma5: true,
        ma20: true,
        ma50: true,
        bollinger: true,
        rsi: true,
        macd: true,
    },
    rightGridPreset: '2x2',
    rightWidgets: [
        { id: 'aiPrediction', visible: true, x: 0, y: 0, w: 2, h: 1, minW: 1, minH: 1 },
        { id: 'shareholdingRatio', visible: true, x: 0, y: 1, w: 1, h: 1, minW: 1, minH: 1 },
        { id: 'monthlyRevenue', visible: true, x: 1, y: 1, w: 1, h: 1, minW: 1, minH: 1 },
        { id: 'peAnalysis', visible: true, x: 0, y: 2, w: 2, h: 1, minW: 1, minH: 1 },
        { id: 'signalList', visible: true, x: 0, y: 3, w: 2, h: 2, minW: 1, minH: 1 },
    ],
}

const DEFAULT_PERSISTED_STATE: AppStorePersistedState = {
    activeSyncId: null,
    selectedSymbol: '2330',
    selectedInterval: '1h',
    isEcoModeEnabled: true,
    apiKey: '',
    colorMode: 'TW',
    dashboardLayout: cloneDashboardLayout(DEFAULT_DASHBOARD_LAYOUT),
}

function cloneDashboardLayout(layout: DashboardLayoutState): DashboardLayoutState {
    return {
        ...layout,
        leftPanelOrder: [...layout.leftPanelOrder],
        leftPanelVisible: { ...layout.leftPanelVisible },
        indicatorVisible: { ...layout.indicatorVisible },
        rightWidgets: layout.rightWidgets.map((widget) => ({ ...widget })),
    }
}

function createDefaultPersistedState(): AppStorePersistedState {
    return {
        ...DEFAULT_PERSISTED_STATE,
        dashboardLayout: cloneDashboardLayout(DEFAULT_DASHBOARD_LAYOUT),
    }
}

function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value))
}

function toInt(value: unknown, fallback: number): number {
    if (typeof value !== 'number' || Number.isNaN(value)) return fallback
    return Math.trunc(value)
}

function isGridPreset(value: unknown): value is DashboardRightGridPreset {
    return value === '1x1' || value === '2x2' || value === '3x3' || value === '4x4'
}

function sanitizeRange(value: unknown): DashboardRange {
    if (value === 'max') return 'max'
    if (typeof value === 'number' && Number.isFinite(value) && value > 0) return Math.trunc(value)
    return DEFAULT_DASHBOARD_LAYOUT.selectedRange
}

function sanitizeSplitRatio(value: unknown): number {
    if (typeof value !== 'number' || Number.isNaN(value)) return DEFAULT_DASHBOARD_LAYOUT.splitRatio
    return clamp(value, 0.35, 0.85)
}

function sanitizeLeftPanelOrder(value: unknown): DashboardLeftPanelId[] {
    if (!Array.isArray(value)) return [...DASHBOARD_LEFT_PANEL_IDS]

    const allowed = new Set<DashboardLeftPanelId>(DASHBOARD_LEFT_PANEL_IDS)
    const unique: DashboardLeftPanelId[] = []

    for (const item of value) {
        if (typeof item !== 'string') continue
        if (!allowed.has(item as DashboardLeftPanelId)) continue
        const panelId = item as DashboardLeftPanelId
        if (!unique.includes(panelId)) unique.push(panelId)
    }

    for (const panelId of DASHBOARD_LEFT_PANEL_IDS) {
        if (!unique.includes(panelId)) unique.push(panelId)
    }

    return unique
}

function sanitizeBoolRecord<T extends string>(
    ids: readonly T[],
    value: unknown,
    fallback: Record<T, boolean>,
): Record<T, boolean> {
    const source = typeof value === 'object' && value != null
        ? (value as Partial<Record<T, unknown>>)
        : ({} as Partial<Record<T, unknown>>)

    const next = {} as Record<T, boolean>
    for (const id of ids) {
        next[id] = typeof source[id] === 'boolean' ? Boolean(source[id]) : fallback[id]
    }
    return next
}

function sanitizeRightWidgets(
    value: unknown,
    preset: DashboardRightGridPreset,
): DashboardRightWidgetLayout[] {
    const columns = DASHBOARD_GRID_COLUMNS[preset]
    const defaults = DEFAULT_DASHBOARD_LAYOUT.rightWidgets.reduce((acc, widget) => {
        acc[widget.id] = widget
        return acc
    }, {} as Record<DashboardRightWidgetId, DashboardRightWidgetLayout>)

    const sourceMap = new Map<DashboardRightWidgetId, Partial<DashboardRightWidgetLayout>>()

    if (Array.isArray(value)) {
        for (const item of value) {
            if (typeof item !== 'object' || item == null) continue
            const maybeId = (item as Partial<DashboardRightWidgetLayout>).id
            if (!maybeId || !DASHBOARD_RIGHT_WIDGET_IDS.includes(maybeId)) continue
            sourceMap.set(maybeId, item as Partial<DashboardRightWidgetLayout>)
        }
    }

    return DASHBOARD_RIGHT_WIDGET_IDS.map((id) => {
        const base = defaults[id]
        const source = sourceMap.get(id)

        if (!source) {
            const w = clamp(base.w, base.minW, columns)
            return {
                ...base,
                w,
                x: clamp(base.x, 0, Math.max(0, columns - w)),
            }
        }

        const minW = clamp(toInt(source.minW, base.minW), 1, columns)
        const minH = clamp(toInt(source.minH, base.minH), 1, 12)
        const w = clamp(toInt(source.w, base.w), minW, columns)
        const h = clamp(toInt(source.h, base.h), minH, 12)
        const x = clamp(toInt(source.x, base.x), 0, Math.max(0, columns - w))
        const y = clamp(toInt(source.y, base.y), 0, 99)

        return {
            id,
            visible: typeof source.visible === 'boolean' ? source.visible : base.visible,
            x,
            y,
            w,
            h,
            minW,
            minH,
        }
    })
}

function sanitizeDashboardLayout(value: unknown): DashboardLayoutState {
    if (typeof value !== 'object' || value == null) {
        return cloneDashboardLayout(DEFAULT_DASHBOARD_LAYOUT)
    }

    const source = value as Partial<DashboardLayoutState>
    const rightGridPreset = isGridPreset(source.rightGridPreset)
        ? source.rightGridPreset
        : DEFAULT_DASHBOARD_LAYOUT.rightGridPreset

    return {
        splitRatio: sanitizeSplitRatio(source.splitRatio),
        selectedRange: sanitizeRange(source.selectedRange),
        leftPanelOrder: sanitizeLeftPanelOrder(source.leftPanelOrder),
        leftPanelVisible: sanitizeBoolRecord(
            DASHBOARD_LEFT_PANEL_IDS,
            source.leftPanelVisible,
            DEFAULT_DASHBOARD_LAYOUT.leftPanelVisible,
        ),
        indicatorVisible: sanitizeBoolRecord(
            DASHBOARD_INDICATOR_IDS,
            source.indicatorVisible,
            DEFAULT_DASHBOARD_LAYOUT.indicatorVisible,
        ),
        rightGridPreset,
        rightWidgets: sanitizeRightWidgets(source.rightWidgets, rightGridPreset),
    }
}

function sanitizePersistedState(value: unknown): AppStorePersistedState {
    if (typeof value !== 'object' || value == null) {
        return createDefaultPersistedState()
    }

    const source = value as Partial<AppStorePersistedState>

    return {
        activeSyncId:
            source.activeSyncId == null || typeof source.activeSyncId === 'string'
                ? source.activeSyncId ?? null
                : DEFAULT_PERSISTED_STATE.activeSyncId,
        selectedSymbol:
            typeof source.selectedSymbol === 'string' && source.selectedSymbol.trim().length > 0
                ? source.selectedSymbol.trim()
                : DEFAULT_PERSISTED_STATE.selectedSymbol,
        selectedInterval:
            typeof source.selectedInterval === 'string' && VALID_INTERVALS.has(source.selectedInterval)
                ? source.selectedInterval
                : DEFAULT_PERSISTED_STATE.selectedInterval,
        isEcoModeEnabled:
            typeof source.isEcoModeEnabled === 'boolean'
                ? source.isEcoModeEnabled
                : DEFAULT_PERSISTED_STATE.isEcoModeEnabled,
        apiKey: typeof source.apiKey === 'string' ? source.apiKey : DEFAULT_PERSISTED_STATE.apiKey,
        colorMode: source.colorMode === 'US' ? 'US' : 'TW',
        dashboardLayout: sanitizeDashboardLayout(source.dashboardLayout),
    }
}

export const useAppStore = create<AppState>()(
    persist(
        (set) => ({
            ...createDefaultPersistedState(),

            setActiveSyncId: (id) => set({ activeSyncId: id }),
            setSelectedSymbol: (symbol) => set({ selectedSymbol: symbol }),
            setSelectedInterval: (interval) => set({ selectedInterval: interval }),
            toggleEcoMode: () => set((s) => ({ isEcoModeEnabled: !s.isEcoModeEnabled })),
            setApiKey: (key) => {
                if (typeof window !== 'undefined') localStorage.setItem('ai_bridge_api_key', key)
                set({ apiKey: key })
            },
            toggleColorMode: () => set((s) => ({ colorMode: s.colorMode === 'TW' ? 'US' : 'TW' })),

            setDashboardSplitRatio: (ratio) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    splitRatio: sanitizeSplitRatio(ratio),
                },
            })),
            setDashboardSelectedRange: (range) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    selectedRange: sanitizeRange(range),
                },
            })),
            setDashboardLeftPanelOrder: (order) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    leftPanelOrder: sanitizeLeftPanelOrder(order),
                },
            })),
            setDashboardLeftPanelVisible: (panelId, visible) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    leftPanelVisible: {
                        ...s.dashboardLayout.leftPanelVisible,
                        [panelId]: visible,
                    },
                },
            })),
            setDashboardIndicatorVisible: (indicatorId, visible) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    indicatorVisible: {
                        ...s.dashboardLayout.indicatorVisible,
                        [indicatorId]: visible,
                    },
                },
            })),
            setDashboardRightGridPreset: (preset) => set((s) => ({
                dashboardLayout: sanitizeDashboardLayout({
                    ...s.dashboardLayout,
                    rightGridPreset: preset,
                }),
            })),
            setDashboardRightWidgets: (widgets) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    rightWidgets: sanitizeRightWidgets(widgets, s.dashboardLayout.rightGridPreset),
                },
            })),
            upsertDashboardRightWidget: (widget) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    rightWidgets: sanitizeRightWidgets(
                        [...s.dashboardLayout.rightWidgets.filter((w) => w.id !== widget.id), widget],
                        s.dashboardLayout.rightGridPreset,
                    ),
                },
            })),
            setDashboardRightWidgetVisible: (widgetId, visible) => set((s) => ({
                dashboardLayout: {
                    ...s.dashboardLayout,
                    rightWidgets: s.dashboardLayout.rightWidgets.map((widget) =>
                        widget.id === widgetId ? { ...widget, visible } : widget,
                    ),
                },
            })),
            resetDashboardLayout: () => set({ dashboardLayout: cloneDashboardLayout(DEFAULT_DASHBOARD_LAYOUT) }),

            // schedule state: backend is source of truth (not persisted)
            scheduleEnabled: false,
            scheduleTime: '02:00',
            scheduleLoaded: false,
            setSchedule: (enabled, time) =>
                set({ scheduleEnabled: enabled, scheduleTime: time, scheduleLoaded: true }),
            setScheduleEnabled: (enabled) => set({ scheduleEnabled: enabled }),
            setScheduleTime: (time) => set({ scheduleTime: time }),
        }),
        {
            name: 'ai-bridge-ui',
            version: APP_STORE_PERSIST_VERSION,
            migrate: (persistedState, version) => {
                if (version < APP_STORE_PERSIST_VERSION) {
                    return sanitizePersistedState(persistedState)
                }
                return sanitizePersistedState(persistedState)
            },
            partialize: (s): AppStorePersistedState => ({
                activeSyncId: s.activeSyncId,
                selectedSymbol: s.selectedSymbol,
                selectedInterval: s.selectedInterval,
                isEcoModeEnabled: s.isEcoModeEnabled,
                apiKey: s.apiKey,
                colorMode: s.colorMode,
                dashboardLayout: cloneDashboardLayout(s.dashboardLayout),
            }),
        },
    ),
)
