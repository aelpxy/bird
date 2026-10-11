import { Download, Maximize2, Minimize2, Pause, Play, Regex, Search, Trash2, WrapText, Clock } from 'lucide-react'
import type { RefObject } from 'react'
import { focusRing } from '@/components/ui/control-styles'
import { IconButton } from '@/components/ui/icon-button'
import type { LogLevel } from '../level'
import type { LogFilters, StreamFilter } from '../filters'
import { RANGES, type LogRange } from '../ranges'

type Toggle = { on: boolean; set: (on: boolean) => void }

type LogToolbarProps = {
  searchRef: RefObject<HTMLInputElement | null>
  filters: LogFilters
  onFilters: (filters: LogFilters) => void
  range: { value: LogRange; set: (range: LogRange) => void }
  machines: { id: string; color: string }[]
  matches: number | null
  error?: string
  paused: Toggle & { missed: number }
  wrap: Toggle
  showTime: Toggle
  fullscreen: Toggle
  onClear: () => void
  onDownload: () => void
}

const segmentClass = (active: boolean) =>
  `px-2 py-1 font-mono text-2xs tracking-[0.06em] uppercase transition-colors ${active ? 'bg-raised text-fg' : 'text-fg-faint hover:text-fg'} ${focusRing}`

function Segmented<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: { value: T; label: string }[]; onChange: (value: T) => void }) {
  return (
    <div role="radiogroup" aria-label={label} className="flex border border-line">
      {options.map((option) => (
        <button key={option.value} type="button" role="radio" aria-checked={value === option.value} onClick={() => onChange(option.value)} className={segmentClass(value === option.value)}>
          {option.label}
        </button>
      ))}
    </div>
  )
}

export function LogToolbar({ searchRef, filters, onFilters, range, machines, matches, error, paused, wrap, showTime, fullscreen, onClear, onDownload }: LogToolbarProps) {
  const set = (patch: Partial<LogFilters>) => onFilters({ ...filters, ...patch })
  return (
    <div className="flex flex-col border-b border-line">
      <div className="flex flex-wrap items-center gap-2 px-2 py-2">
        <Segmented<LogRange> label="Time range" value={range.value} onChange={range.set} options={RANGES.map(({ value, label }) => ({ value, label }))} />
        <label className="relative min-w-52 flex-1">
          <span className="sr-only">Search the logs</span>
          <Search aria-hidden className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-fg-faint" />
          <input
            ref={searchRef}
            type="search"
            value={filters.query}
            onChange={(event) => set({ query: event.target.value })}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                set({ query: '' })
                event.currentTarget.blur()
              }
            }}
            placeholder={filters.regex ? 'Regular expression  ( / )' : 'Search, or field:value  ( / )'}
            aria-invalid={error ? true : undefined}
            className="h-7 w-full border border-line bg-sunken pr-16 pl-8 font-mono text-xs text-fg placeholder:text-fg-faint focus:border-fg-muted focus:outline-none aria-invalid:border-bad"
          />
          <span className="absolute top-1/2 right-1 flex -translate-y-1/2 items-center gap-1">
            {matches !== null && <span className="tabular font-mono text-2xs text-fg-faint">{matches}</span>}
            <IconButton label="Use a regular expression" aria-pressed={filters.regex} onClick={() => set({ regex: !filters.regex })} className={`size-5 ${filters.regex ? 'bg-raised text-fg' : ''}`}>
              <Regex aria-hidden className="size-3.5" />
            </IconButton>
          </span>
        </label>
        <Segmented<StreamFilter>
          label="Stream"
          value={filters.stream}
          onChange={(stream) => set({ stream })}
          options={[
            { value: 'all', label: 'All' },
            { value: 'stdout', label: 'Out' },
            { value: 'stderr', label: 'Err' },
          ]}
        />
        <Segmented<LogLevel | 'all'>
          label="Lowest level"
          value={filters.minLevel}
          onChange={(minLevel) => set({ minLevel })}
          options={[
            { value: 'all', label: 'Any' },
            { value: 'warn', label: 'Warn+' },
            { value: 'error', label: 'Error' },
          ]}
        />
        <div className="ml-auto flex items-center gap-0.5">
          <IconButton label={paused.on ? `Resume${paused.missed ? `, ${paused.missed} new lines` : ''}` : 'Pause'} aria-pressed={paused.on} onClick={() => paused.set(!paused.on)}>
            {paused.on ? <Play aria-hidden className="size-3.5" /> : <Pause aria-hidden className="size-3.5" />}
          </IconButton>
          <IconButton label="Show times" aria-pressed={showTime.on} onClick={() => showTime.set(!showTime.on)} className={showTime.on ? 'text-fg' : ''}>
            <Clock aria-hidden className="size-3.5" />
          </IconButton>
          <IconButton label="Wrap long lines" aria-pressed={wrap.on} onClick={() => wrap.set(!wrap.on)} className={wrap.on ? 'text-fg' : ''}>
            <WrapText aria-hidden className="size-3.5" />
          </IconButton>
          <IconButton label="Download what is shown" onClick={onDownload}>
            <Download aria-hidden className="size-3.5" />
          </IconButton>
          <IconButton label="Clear" onClick={onClear}>
            <Trash2 aria-hidden className="size-3.5" />
          </IconButton>
          <IconButton label={fullscreen.on ? 'Exit full screen' : 'Full screen'} aria-pressed={fullscreen.on} onClick={() => fullscreen.set(!fullscreen.on)}>
            {fullscreen.on ? <Minimize2 aria-hidden className="size-3.5" /> : <Maximize2 aria-hidden className="size-3.5" />}
          </IconButton>
        </div>
      </div>
      {machines.length > 1 && (
        <div className="flex flex-wrap items-center gap-1 border-t border-line px-2 py-1.5" role="group" aria-label="Machines">
          {machines.map((machine) => {
            const shown = !filters.hiddenMachines.has(machine.id)
            return (
              <button
                key={machine.id}
                type="button"
                aria-pressed={shown}
                onClick={() => {
                  const hidden = new Set(filters.hiddenMachines)
                  if (shown) hidden.add(machine.id)
                  else hidden.delete(machine.id)
                  set({ hiddenMachines: hidden })
                }}
                className={`flex items-center gap-1.5 border px-1.5 py-0.5 font-mono text-2xs transition-colors ${shown ? 'border-line-strong text-fg' : 'border-line text-fg-faint line-through'} ${focusRing}`}
              >
                <span aria-hidden className={`size-1.5 bg-current ${machine.color}`} />
                {machine.id.slice(0, 6)}
              </button>
            )
          })}
        </div>
      )}
      {error && <p className="border-t border-line px-3 py-1.5 font-mono text-2xs text-bad">Pattern: {error}</p>}
    </div>
  )
}
