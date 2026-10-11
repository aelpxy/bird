import { useMemo, useState, type PointerEvent } from 'react'
import { buckets, type Bucket } from '../histogram'
import type { LogLevel } from '../level'

const BARS = 80
const HEIGHT = 56
const clock = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false })

type Props = {
  lines: { at?: number | null; level?: LogLevel }[]
  window?: { from: number; to: number }
  onWindow: (window: { from: number; to: number } | undefined) => void
}

// lines per slice of time, errors and warnings on top; dragging across it picks a stretch to read
export function LogHistogram({ lines, window, onWindow }: Props) {
  const bars = useMemo(() => buckets(lines, BARS), [lines])
  const [hover, setHover] = useState<number | null>(null)
  const [drag, setDrag] = useState<{ from: number; to: number } | null>(null)
  if (bars.length === 0) return null
  const tallest = Math.max(1, ...bars.map((bar) => bar.error + bar.warn + bar.other))
  const first = bars[0].start
  const last = bars[bars.length - 1].end

  function indexAt(event: PointerEvent<HTMLDivElement>): number {
    const box = event.currentTarget.getBoundingClientRect()
    return Math.min(BARS - 1, Math.max(0, Math.floor(((event.clientX - box.left) / box.width) * BARS)))
  }

  const shown = drag ?? (window ? { from: Math.max(0, Math.floor(((window.from - first) / (last - first)) * BARS)), to: Math.min(BARS - 1, Math.floor(((window.to - first) / (last - first)) * BARS)) } : null)
  const hovered = hover === null ? undefined : bars[hover]

  return (
    <div className="border-b border-line px-3 pt-2 pb-1.5">
      <div
        role="img"
        aria-label={`${lines.length} lines between ${clock.format(first)} and ${clock.format(last)}. Drag across the chart to read one stretch.`}
        className="relative flex cursor-crosshair touch-none items-end gap-px select-none"
        style={{ height: HEIGHT }}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId)
          const index = indexAt(event)
          setDrag({ from: index, to: index })
        }}
        onPointerMove={(event) => {
          const index = indexAt(event)
          setHover(index)
          if (drag) setDrag({ from: drag.from, to: index })
        }}
        onPointerLeave={() => setHover(null)}
        onPointerUp={() => {
          if (!drag) return
          const low = bars[Math.min(drag.from, drag.to)]
          const high = bars[Math.max(drag.from, drag.to)]
          setDrag(null)
          onWindow({ from: Math.floor(low.start), to: Math.ceil(high.end) })
        }}
      >
        {bars.map((bar, index) => (
          <Bar key={index} bar={bar} tallest={tallest} picked={shown ? index >= Math.min(shown.from, shown.to) && index <= Math.max(shown.from, shown.to) : true} />
        ))}
      </div>
      <div className="mt-1 flex justify-between gap-3 font-mono text-2xs text-fg-faint tabular">
        <span>{clock.format(first)}</span>
        <span aria-live="polite" className="flex items-center gap-2">
          {hovered
            ? `${clock.format(hovered.start)} · ${hovered.error + hovered.warn + hovered.other} lines${hovered.error ? ` · ${hovered.error} errors` : ''}${hovered.warn ? ` · ${hovered.warn} warnings` : ''}`
            : window && (
                <>
                  {clock.format(window.from)} – {clock.format(window.to)}
                  <button type="button" onClick={() => onWindow(undefined)} className="text-fg-muted underline underline-offset-2 hover:text-fg">
                    show all
                  </button>
                </>
              )}
        </span>
        <span>{clock.format(last)}</span>
      </div>
    </div>
  )
}

function Bar({ bar, tallest, picked }: { bar: Bucket; tallest: number; picked: boolean }) {
  const scale = (count: number) => `${(count / tallest) * 100}%`
  return (
    <div className={`flex h-full flex-1 flex-col justify-end ${picked ? '' : 'opacity-25'}`}>
      <div className="bg-bad" style={{ height: scale(bar.error) }} />
      <div className="bg-warn" style={{ height: scale(bar.warn) }} />
      <div className="bg-fg-faint" style={{ height: scale(bar.other) }} />
    </div>
  )
}
