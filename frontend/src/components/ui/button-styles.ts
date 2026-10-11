import { focusRing } from './control-styles'

export type Variant = 'primary' | 'secondary' | 'ghost' | 'danger'

const variants: Record<Variant, string> = {
  primary: 'border border-fg bg-fg text-canvas hover:not-data-disabled:bg-fg-muted hover:not-data-disabled:border-fg-muted',
  secondary:
    'border border-line bg-surface text-fg hover:not-data-disabled:border-line-strong hover:not-data-disabled:bg-raised',
  ghost: 'border border-transparent text-fg-muted hover:not-data-disabled:bg-raised hover:not-data-disabled:text-fg',
  danger: 'border border-bad/60 bg-bad/10 text-bad hover:not-data-disabled:bg-bad hover:not-data-disabled:text-canvas',
}

export const sizes = {
  md: 'h-9 px-3.5 text-sm',
  sm: 'h-7 px-2.5 text-ui',
} as const

// for links that should look like buttons, like a download
export function buttonClass(variant: Variant = 'primary', size: keyof typeof sizes = 'md'): string {
  return `pressable inline-flex shrink-0 items-center justify-center gap-1.5 rounded-sm font-medium whitespace-nowrap select-none ${sizes[size]} ${focusRing} ${variants[variant]}`
}
