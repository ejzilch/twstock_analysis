import { clsx } from 'clsx'

export function Pagination({
    page,
    totalPages,
    onPageChange,
}: {
    page: number
    totalPages: number
    onPageChange: (page: number) => void
}) {
    return (
        <div className="flex items-center justify-center gap-1 mt-3">
            <button
                onClick={() => onPageChange(0)}
                disabled={page === 0}
                className="text-xs px-2 py-1 rounded text-slate-500 hover:text-slate-300
                   disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
                «
            </button>
            <button
                onClick={() => onPageChange(Math.max(0, page - 1))}
                disabled={page === 0}
                className="text-xs px-2 py-1 rounded text-slate-500 hover:text-slate-300
                   disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
                ‹
            </button>

            {Array.from({ length: totalPages }, (_, i) => i)
                .filter((i) => Math.abs(i - page) <= 2)
                .map((i) => (
                    <button
                        key={i}
                        onClick={() => onPageChange(i)}
                        className={clsx(
                            'text-xs w-6 h-6 rounded transition-all',
                            i === page
                                ? 'bg-brand-600 text-white'
                                : 'text-slate-500 hover:text-slate-300 hover:bg-surface-hover',
                        )}
                    >
                        {i + 1}
                    </button>
                ))}

            <button
                onClick={() => onPageChange(Math.min(totalPages - 1, page + 1))}
                disabled={page >= totalPages - 1}
                className="text-xs px-2 py-1 rounded text-slate-500 hover:text-slate-300
                   disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
                ›
            </button>
            <button
                onClick={() => onPageChange(totalPages - 1)}
                disabled={page >= totalPages - 1}
                className="text-xs px-2 py-1 rounded text-slate-500 hover:text-slate-300
                   disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
            >
                »
            </button>
        </div>
    )
}