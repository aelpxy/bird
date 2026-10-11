import type { ReactNode } from 'react'
import { Logo } from '@/components/layout/logo'
import { Eyebrow } from '@/components/ui/eyebrow'

type AuthCardProps = {
  step: string
  title: string
  description: ReactNode
  children: ReactNode
}

export function AuthCard({ step, title, description, children }: AuthCardProps) {
  return (
    <main className="bg-grid flex min-h-dvh flex-col items-center justify-center px-4 py-12">
      <div className="w-full max-w-sm">
        <div className="mb-6 flex items-end justify-between gap-4">
          <div className="flex items-center gap-3">
            <Logo size="lg" />
            <div>
              <p className="font-mono text-sm font-semibold tracking-tight text-fg">bird</p>
              <p className="text-xs text-fg-faint">Agent and human friendly containers as a service.</p>
            </div>
          </div>
        </div>
        <Eyebrow className="mb-2">{step}</Eyebrow>
        <div className="border border-line-strong bg-surface">
          <div className="border-b border-line px-6 py-5">
            <h1 className="text-lg font-semibold tracking-tight text-fg">{title}</h1>
            <p className="mt-1 text-sm text-fg-muted">{description}</p>
          </div>
          <div className="px-6 py-6">{children}</div>
        </div>
      </div>
    </main>
  )
}
