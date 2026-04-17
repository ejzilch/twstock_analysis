/**
 * Unified fetch wrapper.
 * - Injects X-API-KEY header on every request
 * - Parses error_code from non-2xx responses
 * - Throws ApiErrorException for downstream error-handler to process
 */
import type { ApiError, ErrorCode } from '@/types/api.generated'

export class ApiErrorException extends Error {
    constructor(
        public readonly errorCode: ErrorCode,
        public readonly apiError: ApiError,
        public readonly httpStatus: number,
    ) {
        super(apiError.message)
        this.name = 'ApiErrorException'
    }
}

function getApiKey(): string {
    if (typeof window !== 'undefined') {
        return localStorage.getItem('ai_bridge_api_key') ?? process.env.NEXT_PUBLIC_API_KEY ?? ''
    }
    return process.env.NEXT_PUBLIC_API_KEY ?? ''
}

const BASE = process.env.NEXT_PUBLIC_API_BASE_URL ?? ''

export async function apiClient<T>(
    path: string,
    init: RequestInit = {},
): Promise<T> {
    const url = `${BASE}${path}`
    const headers: HeadersInit = {
        'Content-Type': 'application/json',
        'X-API-KEY': getApiKey(),
        ...(init.headers ?? {}),
    }

    const response = await fetch(url, { ...init, headers })

    if (!response.ok) {
        let apiError: ApiError
        try {
            apiError = await response.json()
        } catch {
            apiError = {
                error_code: 'INDICATOR_COMPUTE_FAILED',
                message: `HTTP ${response.status}`,
                fallback_available: false,
                timestamp_ms: Date.now(),
                request_id: null,
            }
        }
        throw new ApiErrorException(apiError.error_code, apiError, response.status)
    }

    return response.json() as Promise<T>
}

export function buildQueryString(params: Record<string, string | number | boolean | undefined>): string {
    const qs = new URLSearchParams()
    for (const [key, value] of Object.entries(params)) {
        if (value !== undefined) qs.set(key, String(value))
    }
    const str = qs.toString()
    return str ? `?${str}` : ''
}