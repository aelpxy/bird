import { useState, type PointerEvent } from 'react'
import { eyebrowClass } from '@/components/ui/control-styles'

const WIDTH = 480
const HEIGHT = 120
const PAD_TOP = 8
const clock = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', hour12: false })

type Point = { at: number; value: number }
type Props = { title: string; points: Point[]; limit: number; format: (value: number) => string }

// one measure over time against its limit, read with a crosshair; one series, so no legend
export function UsageChart({ title, points, limit, format }: Props) {
  const [hover, setHover] = useState<number | null>(null)
  if (points.length < 2) return null
  const first = points[0].at
  const last = points[points.length - 1].at
  const top = Math.max(limit, ...points.map((point) => point.value))
  const x = (at: number) => ((at - first) / Math.max(1, last - first)) * WIDTH
  const y = (value: number) => PAD_TOP + (1 - value / top) * (HEIGHT - PAD_TOP)
  const line = points.map((point, index) => `${index === 0 ? 'M' : 'L'}${x(point.at).toFixed(1)},${y(point.value).toFixed(1)}`).join(' ')
  const area = `${line} L${WIDTH},${HEIGHT} L0,${HEIGHT} Z`
  const now = points[points.length - 1]
  const peak = Math.max(...points.map((point) => point.value))
  const hovered = hover === null ? undefined : points[hover]

  function onMove(event: PointerEvent<SVGSVGElement>) {
    const box = event.currentTarget.getBoundingClientRect()
    const at = first + ((event.clientX - box.left) / box.width) * (last - first)
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
        <span className="tabular font-mono text-xs text-fg-muted">
          {hovered ? (
            <>
              <span className="text-fg">{format(hovered.value)}</span> at {clock.format(hovered.at * 1000)}
            </>
          ) : (
            <>
              <span className="text-fg">{format(now.value)}</span> now · peak {format(peak)}
            </>
          )}
        </span>
      </figcaption>
      <svg
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`${title} over the last ${Math.round((last - first) / 60)} minutes: now ${format(now.value)}, peak ${format(peak)}, limit ${format(limit)}`}
        className="h-28 w-full touch-none overflow-visible"
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        <line x1={0} x2={WIDTH} y1={y(limit)} y2={y(limit)} className="stroke-line-strong" strokeDasharray="3 3" vectorEffect="non-scaling-stroke" />
        <line x1={0} x2={WIDTH} y1={HEIGHT} y2={HEIGHT} className="stroke-line" vectorEffect="non-scaling-stroke" />
        <path d={area} className="fill-fg/8" />
        <path d={line} fill="none" className="stroke-fg" strokeWidth={2} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
        {hovered && <line x1={x(hovered.at)} x2={x(hovered.at)} y1={0} y2={HEIGHT} className="stroke-fg-faint" vectorEffect="non-scaling-stroke" />}
      </svg>
      <div className="flex justify-between font-mono text-2xs text-fg-faint tabular">
        <span>{clock.format(first * 1000)}</span>
        <span>limit {format(limit)}</span>
        <span>{clock.format(last * 1000)}</span>
      </div>
    </figure>
  )
}
