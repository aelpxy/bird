import { useState, type PointerEvent } from 'react'
import { eyebrowClass } from '@/components/ui/control-styles'

const WIDTH = 480
const HEIGHT = 120
const PAD_TOP = 8
const short = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', hour12: false })
const long = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false })

export type ChartPoint = { at: number; value: number; peak?: number }
export type Marker = { at: number; label: string }

type Props = {
  title: string
  points: ChartPoint[]
  // the whole range in unix seconds, so a service that started recently does not fill the width
  from: number
  to: number
  step: number
  limit?: number
  markers?: Marker[]
  format: (value: number) => string
}

// one measure over time: the average as a line, the peak faintly above it, deploys as ticks
export function MetricChart({ title, points, from, to, step, limit, markers = [], format }: Props) {
  const [hover, setHover] = useState<number | null>(null)
  const clock = to - from > 86_400 ? long : short
  const top = Math.max(limit ?? 0, 1, ...points.map((point) => Math.max(point.value, point.peak ?? 0)))
  const x = (at: number) => ((at - from) / Math.max(1, to - from)) * WIDTH
  const y = (value: number) => PAD_TOP + (1 - value / top) * (HEIGHT - PAD_TOP)
  const line = path(points, step, (point) => point.value, x, y)
  const peak = points.some((point) => point.peak !== undefined && point.peak > point.value) ? path(points, step, (point) => point.peak ?? point.value, x, y) : ''
  const latest = points.at(-1)
  const highest = Math.max(0, ...points.map((point) => point.peak ?? point.value))
  const hovered = hover === null ? undefined : points[hover]
  const shownMarkers = markers.filter((marker) => marker.at >= from && marker.at <= to)

  function onMove(event: PointerEvent<SVGSVGElement>) {
    if (points.length === 0) return
    const box = event.currentTarget.getBoundingClientRect()
    const at = from + ((event.clientX - box.left) / box.width) * (to - from)
    let nearest = 0
    for (let index = 1; index < points.length; index += 1) {
      if (Math.abs(points[index].at - at) < Math.abs(points[nearest].at - at)) nearest = index
    }
    setHover(nearest)
  }

  return (
    <figure className="flex min-w-0 flex-col gap-2">
      <figcaption className="flex items-baseline justify-between gap-3">
        <span className={eyebrowClass}>{title}</span>
        <span className="tabular truncate font-mono text-xs text-fg-muted">
          {hovered ? (
            <>
              <span className="text-fg">{format(hovered.value)}</span>
              {hovered.peak !== undefined && hovered.peak > hovered.value && ` · peak ${format(hovered.peak)}`} · {clock.format(hovered.at * 1000)}
            </>
          ) : latest ? (
            <>
              <span className="text-fg">{format(latest.value)}</span> latest · peak {format(highest)}
            </>
          ) : (
            'No data yet'
          )}
        </span>
      </figcaption>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={latest ? `${title}: latest ${format(latest.value)}, peak ${format(highest)}${limit ? `, limit ${format(limit)}` : ''}` : `${title}: no data yet`}
        className="h-28 w-full touch-none overflow-visible"
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        {limit !== undefined && <line x1={0} x2={WIDTH} y1={y(limit)} y2={y(limit)} className="stroke-line-strong" strokeDasharray="3 3" vectorEffect="non-scaling-stroke" />}
        <line x1={0} x2={WIDTH} y1={HEIGHT} y2={HEIGHT} className="stroke-line" vectorEffect="non-scaling-stroke" />
        {shownMarkers.map((marker) => (
          <line key={marker.at} x1={x(marker.at)} x2={x(marker.at)} y1={0} y2={HEIGHT} className="stroke-fg-faint" strokeDasharray="1 3" vectorEffect="non-scaling-stroke">
            <title>{`${marker.label} · ${clock.format(marker.at * 1000)}`}</title>
          </line>
        ))}
        {peak && <path d={peak} fill="none" className="stroke-fg-faint" strokeWidth={1} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />}
        <path d={line} fill="none" className="stroke-fg" strokeWidth={2} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
        {hovered && <line x1={x(hovered.at)} x2={x(hovered.at)} y1={0} y2={HEIGHT} className="stroke-fg-muted" vectorEffect="non-scaling-stroke" />}
      </svg>
      <div className="flex justify-between gap-3 font-mono text-2xs text-fg-faint tabular">
        <span>{clock.format(from * 1000)}</span>
        {limit !== undefined && <span>limit {format(limit)}</span>}
        <span>now</span>
      </div>
    </figure>
  )
}

// a gap of more than two steps, like birdd being down, breaks the line instead of bridging it
function path(points: ChartPoint[], step: number, value: (point: ChartPoint) => number, x: (at: number) => number, y: (value: number) => number): string {
  return points
    .map((point, index) => {
      const joined = index > 0 && point.at - points[index - 1].at <= step * 2
      return `${joined ? 'L' : 'M'}${x(point.at).toFixed(1)},${y(value(point)).toFixed(1)}`
    })
    .join(' ')
}
