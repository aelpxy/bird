import type { ReactNode } from 'react'
import { eyebrowClass } from './control-styles'

// a set of radio cards: for choices that each need a sentence of explanation
export function ChoiceGroup({ legend, children, columns = 'auto' }: { legend: string; children: ReactNode; columns?: 'auto' | 'stack' }) {
  return (
    <fieldset>
      <legend className={`${eyebrowClass} mb-2`}>{legend}</legend>
      <div className={columns === 'stack' ? 'flex flex-col gap-2' : 'grid gap-2 sm:grid-cols-[repeat(auto-fit,minmax(10rem,1fr))]'}>{children}</div>
    </fieldset>
  )
}

type ChoiceCardProps = {
  name: string
  label: string
  description: ReactNode
  checked: boolean
  disabled?: boolean
  icon?: ReactNode
  onSelect: () => void
}

export function ChoiceCard({ name, label, description, checked, disabled = false, icon, onSelect }: ChoiceCardProps) {
  return (
    <label
      className={`pressable relative flex items-start gap-3 border p-3 focus-within:outline focus-within:outline-1 focus-within:outline-offset-2 focus-within:outline-fg ${checked ? 'border-fg bg-raised' : 'border-line bg-surface'} ${disabled ? 'opacity-50' : 'cursor-pointer hover:border-line-strong'}`}
    >
      <input type="radio" name={name} checked={checked} disabled={disabled} onChange={onSelect} className="sr-only" />
      {icon && <span className="mt-0.5 text-fg-muted">{icon}</span>}
      <span className="min-w-0">
        <span className="block text-sm font-medium text-fg">{label}</span>
        <span className="mt-0.5 block text-xs text-fg-faint">{description}</span>
      </span>
      {checked && <span aria-hidden className="absolute top-0 right-0 size-2 bg-fg" />}
    </label>
  )
}
