import { useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { Scope } from '@/lib/api/paths'
import { stripAnsi } from '@/lib/ansi'
import { filterLines, prepare, type LogFilters, type ShownLine } from '../filters'
import { MACHINE_COLORS } from '../machine-colors'
import { minutesOf, type LogRange } from '../ranges'
import { type LogLine, type StreamState, useLogStream } from '../use-log-stream'
import { LogDetails } from './log-details'
import { LogHistogram } from './log-histogram'
import { LogLineRow } from './log-line'
import { LogToolbar } from './log-toolbar'

const streamBadges: Record<StreamState, { label: string; tone: 'ok' | 'warn' | 'neutral' }> = {
  connecting: { label: 'Connecting', tone: 'neutral' },
  live: { label: 'Live', tone: 'ok' },
  reconnecting: { label: 'Reconnecting', tone: 'warn' },
}

type Props = { scope: Scope; service: string; query: string; range: LogRange; onQuery: (query: string) => void; onRange: (range: LogRange) => void }

export function LogViewer({ scope, service, query, range, onQuery, onRange }: Props) {
  const { lines, state, clear } = useLogStream(scope, service, minutesOf(range))
  const [paused, setPaused] = useState<LogLine[] | null>(null)
  const [filters, setFilters] = useState<Omit<LogFilters, 'query'>>({ regex: false, stream: 'all', minLevel: 'all', hiddenMachines: new Set() })
  const [wrap, setWrap] = useState(true)
  const [showTime, setShowTime] = useState(true)
  const [fullscreen, setFullscreen] = useState(false)
  const [atBottom, setAtBottom] = useState(true)
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set())
  const scroller = useRef<HTMLDivElement>(null)
  const search = useRef<HTMLInputElement>(null)

  // detecting levels, fields and stripping colors is done once per line, not on every filter change
  const prepared = useMemo(() => (paused ?? lines).map(prepare), [paused, lines])
  // the chart shows what the query matches over the whole time, the list only the picked stretch
  const matched = useMemo(() => filterLines(prepared, { ...filters, query, window: undefined }), [prepared, filters, query])
  const shown = useMemo(() => {
    const { window } = filters
    return window ? matched.lines.filter((line) => line.at != null && line.at >= window.from && line.at <= window.to) : matched.lines
  }, [matched, filters])
  const machines = useMemo(() => {
    const ids = [...new Set(prepared.map((line) => line.machine))].sort()
    return ids.map((id, index) => ({ id, color: MACHINE_COLORS[index % MACHINE_COLORS.length] }))
  }, [prepared])
  const colorOf = useMemo(() => new Map(machines.map((machine) => [machine.id, machine.color])), [machines])
  const missed = paused ? lines.length - paused.length : 0

  // the virtualizer re-renders on scroll itself, so the compiler skipping this component costs nothing
  // oxlint-disable-next-line react/incompatible-library
  const virtualizer = useVirtualizer({
    count: shown.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => 20,
    overscan: 30,
    getItemKey: (index) => shown[index]?.id ?? index,
  })

  // stays pinned to the newest line until the reader scrolls up
  useEffect(() => {
    if (atBottom && shown.length > 0) virtualizer.scrollToIndex(shown.length - 1, { align: 'end' })
  }, [shown.length, atBottom, virtualizer])

  // "/" jumps to search; Escape leaves full screen
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const typing = event.target instanceof HTMLElement && /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName)
      if (event.key === '/' && !typing) {
        event.preventDefault()
        search.current?.focus()
      } else if (event.key === 'Escape' && fullscreen && !typing) setFullscreen(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [fullscreen])

  function toggle(id: number) {
    setExpanded((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  function addTerm(term: string) {
    onQuery(query.trim() ? `${query.trim()} ${term}` : term)
  }

  function download(lines: ShownLine[]) {
    const text = lines.map((line) => `${line.at ? new Date(line.at).toISOString() : '-'} ${line.machine.slice(0, 12)} ${line.stream} ${stripAnsi(line.text)}`).join('\n')
    const url = URL.createObjectURL(new Blob([`${text}\n`], { type: 'text/plain' }))
    const link = Object.assign(document.createElement('a'), { href: url, download: `${service}-${new Date().toISOString().slice(0, 19).replace(/:/g, '')}.log` })
    link.click()
    URL.revokeObjectURL(url)
  }

  const filtered = Boolean(query) || filters.stream !== 'all' || filters.minLevel !== 'all' || filters.hiddenMachines.size > 0 || Boolean(filters.window)
  const badge = streamBadges[state]
  return (
    <div className={fullscreen ? 'fixed inset-0 z-40 flex flex-col bg-canvas p-3' : 'flex flex-col'}>
      <div className={`flex flex-col border border-line bg-surface ${fullscreen ? 'min-h-0 flex-1' : ''}`}>
        <LogToolbar
          searchRef={search}
          filters={{ ...filters, query }}
          onFilters={({ query: next, ...rest }) => {
            setFilters(rest)
            if (next !== query) onQuery(next)
          }}
          range={{ value: range, set: onRange }}
          machines={machines}
          matches={query ? shown.length : null}
          error={matched.error}
          paused={{ on: paused !== null, set: (on) => setPaused(on ? lines : null), missed }}
          wrap={{ on: wrap, set: setWrap }}
          showTime={{ on: showTime, set: setShowTime }}
          fullscreen={{ on: fullscreen, set: setFullscreen }}
          onClear={() => {
            clear()
            setPaused(null)
          }}
          onDownload={() => download(shown)}
        />
        <LogHistogram lines={matched.lines} window={filters.window} onWindow={(window) => setFilters({ ...filters, window })} />
        <div className={`relative ${fullscreen ? 'min-h-0 flex-1' : ''}`}>
          <div
            ref={scroller}
            onScroll={() => {
              const element = scroller.current
              if (element) setAtBottom(element.scrollHeight - element.scrollTop - element.clientHeight < 24)
            }}
            role="log"
            aria-live="off"
            aria-label={`Logs of ${service}`}
            tabIndex={0}
            className={`overflow-auto bg-sunken py-1.5 font-mono text-xs leading-5 text-fg focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-fg-muted ${fullscreen ? 'h-full' : 'h-[min(38rem,65dvh)]'}`}
          >
            {shown.length === 0 ? (
              <p className="px-4 py-2 text-fg-faint">{filtered ? 'No lines match the filters.' : state === 'live' ? 'Waiting for output…' : 'Connecting to the log stream…'}</p>
            ) : (
              <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((item) => {
                  const line = shown[item.index]
                  return (
                    <div key={item.key} ref={virtualizer.measureElement} data-index={item.index} className="absolute top-0 left-0 w-full" style={{ transform: `translateY(${item.start}px)` }}>
                      <LogLineRow
                        line={line}
                        machineColor={colorOf.get(line.machine) ?? 'text-info'}
                        matcher={matched.matcher}
                        wrap={wrap}
                        showTime={showTime}
                        expanded={expanded.has(line.id)}
                        onToggle={toggle}
                      />
                      {expanded.has(line.id) && <LogDetails line={line} onAddTerm={addTerm} />}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
          {!atBottom && (
            <Button size="sm" className="absolute right-4 bottom-4" onClick={() => setAtBottom(true)}>
              <ArrowDown aria-hidden className="size-3.5" />
              Latest
            </Button>
          )}
        </div>
        <div className="flex items-center justify-between gap-3 border-t border-line px-3 py-1.5 font-mono text-2xs text-fg-faint">
          <span className="tabular">
            {shown.length === prepared.length ? `${prepared.length} lines` : `${shown.length} of ${prepared.length} lines`}
            {machines.length > 1 && ` · ${machines.length} machines`}
          </span>
          <Badge tone={paused ? 'neutral' : badge.tone} dot pulse={!paused && state !== 'live'}>
            {paused ? `Paused${missed ? ` · ${missed} new` : ''}` : badge.label}
          </Badge>
        </div>
      </div>
    </div>
  )
}
