import type { ReactNode } from 'react'

export type Tone = 'neutral' | 'ok' | 'warn' | 'bad' | 'info'

const tones: Record<Tone, { badge: string; dot: string }> = {
  neutral: { badge: 'border-line-strong text-fg-muted', dot: 'bg-fg-faint' },
  ok: { badge: 'border-ok/40 bg-ok/8 text-ok', dot: 'bg-ok' },
  warn: { badge: 'border-warn/40 bg-warn/8 text-warn', dot: 'bg-warn' },
  bad: { badge: 'border-bad/40 bg-bad/8 text-bad', dot: 'bg-bad' },
  info: { badge: 'border-info/40 bg-info/8 text-info', dot: 'bg-info' },
}

type BadgeProps = {
  tone?: Tone
  dot?: boolean
  // a pulsing dot for states that are still changing, like a deploy in progress
  pulse?: boolean
  children: ReactNode
}

export function Badge({ tone = 'neutral', dot = false, pulse = false, children }: BadgeProps) {
  const { badge, dot: dotColor } = tones[tone]
  return (
    <span
      className={`inline-flex h-5 items-center gap-1.5 border px-1.5 font-mono text-2xs font-medium tracking-wide whitespace-nowrap uppercase ${badge}`}
    >
      {dot && <span aria-hidden className={`size-1.5 ${dotColor} ${pulse ? 'animate-pulse' : ''}`} />}
      {children}
    </span>
  )
}
