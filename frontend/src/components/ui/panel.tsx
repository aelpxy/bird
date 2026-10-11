import type { ReactNode } from 'react'

type PanelProps = {
  children: ReactNode
  tone?: 'default' | 'danger' | 'notice'
  padded?: boolean
  className?: string
}

const tones = {
  default: 'border-line bg-surface',
  danger: 'border-bad/40 bg-bad/5',
  notice: 'border-warn/40 bg-warn/5',
}

// the one container: a hairline box on the surface color
export function Panel({ children, tone = 'default', padded = true, className = '' }: PanelProps) {
  return <div className={`border ${tones[tone]} ${padded ? 'p-4' : ''} ${className}`}>{children}</div>
}
