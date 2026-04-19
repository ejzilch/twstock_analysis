
'use client'
import { useState } from 'react'
import { Card, Input, Button } from '@/src/components/ui'
import { useAppStore } from '@/src/store/useAppStore'

export function ApiKeyForm() {
    const storedKey = useAppStore((s) => s.apiKey)
    const setApiKey = useAppStore((s) => s.setApiKey)
    const [key, setKey] = useState(storedKey)
    const [saved, setSaved] = useState(false)

    function handleSave() {
        setApiKey(key)
        setSaved(true)
        setTimeout(() => setSaved(false), 2000)
    }

    return (
        <Card>
            <h3 className="text-sm font-semibold text-slate-200 mb-4">API Key 管理</h3>
            <p className="text-xs text-slate-500 mb-4">
                API Key 儲存於本機 localStorage，不會上傳至伺服器。每次請求會自動帶入 X-API-KEY header。
            </p>
            <div className="flex gap-3">
                <Input
                    value={key}
                    onChange={setKey}
                    type="password"
                    placeholder="輸入你的 API Key..."
                    className="flex-1"
                />
                <Button onClick={handleSave} variant={saved ? 'secondary' : 'primary'}>
                    {saved ? '✓ 已儲存' : '儲存'}
                </Button>
            </div>
        </Card>
    )
}