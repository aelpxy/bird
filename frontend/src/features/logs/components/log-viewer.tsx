import { ArrowDown } from 'lucide-react'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { Scope } from '@/lib/api/paths'
import { stripAnsi } from '@/lib/ansi'
import { filterLines, prepare, type LogFilters, type ShownLine } from '../filters'
import { type LogLine, type StreamState, useLogStream } from '../use-log-stream'
import { LogLineRow } from './log-line'
import { MACHINE_COLORS } from '../machine-colors'
import { LogToolbar } from './log-toolbar'

const streamBadges: Record<StreamState, { label: string; tone: 'ok' | 'warn' | 'neutral' }> = {
  connecting: { label: 'Connecting', tone: 'neutral' },
  live: { label: 'Live', tone: 'ok' },
  reconnecting: { label: 'Reconnecting', tone: 'warn' },
}

const noFilters: LogFilters = { query: '', regex: false, stream: 'all', minLevel: 'all', hiddenMachines: new Set() }

export function LogViewer({ scope, service }: { scope: Scope; service: string }) {
  const { lines, state, clear } = useLogStream(scope, service)
  const [paused, setPaused] = useState<LogLine[] | null>(null)
  const [filters, setFilters] = useState(noFilters)
  const [wrap, setWrap] = useState(true)
  const [showTime, setShowTime] = useState(true)
  const [fullscreen, setFullscreen] = useState(false)
  const [atBottom, setAtBottom] = useState(true)
  const scroller = useRef<HTMLDivElement>(null)
  const search = useRef<HTMLInputElement>(null)

  // detecting levels and stripping colors is done once per line, not on every filter change
  const prepared = useMemo(() => (paused ?? lines).map(prepare), [paused, lines])
  const result = useMemo(() => filterLines(prepared, filters), [prepared, filters])
  const machines = useMemo(() => {
    const ids = [...new Set(prepared.map((line) => line.machine))].sort()
    return ids.map((id, index) => ({ id, color: MACHINE_COLORS[index % MACHINE_COLORS.length] }))
  }, [prepared])
  const colorOf = useMemo(() => new Map(machines.map((machine) => [machine.id, machine.color])), [machines])
  const missed = paused ? lines.length - paused.length : 0

  // stays pinned to the newest line until the reader scrolls up
  useLayoutEffect(() => {
    const element = scroller.current
    if (element && atBottom) element.scrollTop = element.scrollHeight
  }, [result.lines, atBottom])

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

  function download(shown: ShownLine[]) {
    const text = shown.map((line) => `${line.at ? new Date(line.at).toISOString() : '-'} ${line.machine.slice(0, 12)} ${line.stream} ${stripAnsi(line.text)}`).join('\n')
    const url = URL.createObjectURL(new Blob([`${text}\n`], { type: 'text/plain' }))
    const link = Object.assign(document.createElement('a'), { href: url, download: `${service}-${new Date().toISOString().slice(0, 19).replace(/:/g, '')}.log` })
    link.click()
    URL.revokeObjectURL(url)
  }

  const badge = streamBadges[state]
  return (
    <div className={fullscreen ? 'fixed inset-0 z-40 flex flex-col bg-canvas p-3' : 'flex flex-col'}>
      <div className={`flex flex-col border border-line bg-surface ${fullscreen ? 'min-h-0 flex-1' : ''}`}>
        <LogToolbar
          searchRef={search}
          filters={filters}
          onFilters={setFilters}
          machines={machines}
          matches={filters.query ? result.lines.length : null}
          error={result.error}
          paused={{ on: paused !== null, set: (on) => setPaused(on ? lines : null), missed }}
          wrap={{ on: wrap, set: setWrap }}
          showTime={{ on: showTime, set: setShowTime }}
          fullscreen={{ on: fullscreen, set: setFullscreen }}
          onClear={() => {
            clear()
            setPaused(null)
          }}
          onDownload={() => download(result.lines)}
        />
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
            {result.lines.length === 0 ? (
              <p className="px-4 py-2 text-fg-faint">
                {filters.query || filters.stream !== 'all' || filters.minLevel !== 'all' || filters.hiddenMachines.size > 0
                  ? 'No lines match the filters.'
                  : state === 'live'
                    ? 'Waiting for output…'
                    : 'Connecting to the log stream…'}
              </p>
            ) : (
              result.lines.map((line) => (
                <LogLineRow key={line.id} line={line} machineColor={colorOf.get(line.machine) ?? 'text-info'} matcher={result.matcher} wrap={wrap} showTime={showTime} />
              ))
            )}
          </div>
          {!atBottom && (
            <Button
              size="sm"
              className="absolute right-4 bottom-4"
              onClick={() => {
                const element = scroller.current
                if (element) element.scrollTop = element.scrollHeight
                setAtBottom(true)
              }}
            >
              <ArrowDown aria-hidden className="size-3.5" />
              Latest
            </Button>
          )}
        </div>
        <div className="flex items-center justify-between gap-3 border-t border-line px-3 py-1.5 font-mono text-2xs text-fg-faint">
          <span className="tabular">
            {result.lines.length === prepared.length ? `${prepared.length} lines` : `${result.lines.length} of ${prepared.length} lines`}
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
