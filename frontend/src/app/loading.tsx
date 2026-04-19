export default function GlobalLoading() {
    return (
        <div className="flex items-center justify-center h-full min-h-[400px]">
            <div className="flex flex-col items-center gap-3">
                <div className="w-8 h-8 border-2 border-brand-600/30 border-t-brand-500 rounded-full animate-spin" />
                <p className="text-sm text-slate-500">載入中...</p>
            </div>
        </div>
    )
}