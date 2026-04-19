'use client'
import { useEffect } from 'react'
import { Button } from '@/src/components/ui'

interface ErrorPageProps {
    error: Error & { digest?: string }
    reset: () => void
}

export default function GlobalError({ error, reset }: ErrorPageProps) {
    useEffect(() => {
        // Log to server-side monitoring in production
        console.error('[GlobalError]', error)
    }, [error])

    return (
        <div className="flex items-center justify-center h-full min-h-[400px]">
            <div className="text-center max-w-md px-6">
                <div className="w-12 h-12 rounded-full bg-red-500/10 border border-red-500/20 flex items-center justify-center mx-auto mb-4">
                    <span className="text-red-400 text-xl">!</span>
                </div>
                <h2 className="text-base font-semibold text-slate-200 mb-2">頁面發生錯誤</h2>
                <p className="text-sm text-slate-500 mb-5">
                    {error.message ?? '發生未預期的錯誤，請嘗試重新整理'}
                </p>
                <div className="flex items-center justify-center gap-3">
                    <Button onClick={reset} variant="primary">重試</Button>
                    <Button onClick={() => window.location.href = '/dashboard'} variant="secondary">
                        返回 Dashboard
                    </Button>
                </div>
            </div>
        </div>
    )
}