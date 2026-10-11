// one look for everything you type into or pick from

export const focusRing =
  'focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-fg'

export const controlClass =
  'w-full rounded-sm border border-line bg-sunken px-3 text-sm text-fg transition-colors placeholder:text-fg-faint hover:border-line-strong focus:border-fg-muted focus:outline-none data-invalid:border-bad disabled:opacity-50 data-disabled:opacity-50 any-pointer-coarse:text-base'

export const inputClass = `${controlClass} h-9`

export const textareaClass = `${controlClass} min-h-20 resize-y py-2 font-mono leading-5`

export const labelClass = 'text-ui font-medium text-fg'

export const hintClass = 'text-xs text-fg-faint'

export const errorTextClass = 'text-xs text-bad'

// small uppercase mono labels: section titles, table headers, metadata keys
export const eyebrowClass = 'font-mono text-2xs font-medium tracking-[0.08em] text-fg-faint uppercase'
