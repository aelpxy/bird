import type { ComponentProps, ReactNode } from 'react'
import { focusRing } from './control-styles'

type IconButtonProps = Omit<ComponentProps<'button'>, 'aria-label' | 'children'> & {
  // icon-only buttons need a name for screen readers
  label: string
  tone?: 'default' | 'danger'
  children?: ReactNode
}

export function IconButton({ label, tone = 'default', className = '', type = 'button', ...props }: IconButtonProps) {
  const hover = tone === 'danger' ? 'hover:bg-bad/15 hover:text-bad' : 'hover:bg-raised hover:text-fg'
  return (
    <button
      {...props}
      type={type}
      aria-label={label}
      title={label}
      className={`pressable grid size-7 shrink-0 place-items-center rounded-sm text-fg-faint disabled:opacity-40 ${hover} ${focusRing} ${className}`}
    />
  )
}
