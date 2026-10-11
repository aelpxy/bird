const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })
const absolute = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' })

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['week', 7 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60],
]

// birdd sends unix seconds
export function timeAgo(unixSeconds: number, now = Date.now()): string {
  const seconds = unixSeconds - Math.round(now / 1000)
  for (const [unit, size] of UNITS) {
    if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit)
  }
  return 'just now'
}

export function dateTime(unixSeconds: number): string {
  return absolute.format(new Date(unixSeconds * 1000))
}

export function bytes(value: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = value
  let unit = 0
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit += 1
  }
  return `${size.toFixed(size < 10 && unit > 0 ? 1 : 0)} ${units[unit]}`
}

export function mebibytes(value: number): string {
  return value >= 1024 && value % 1024 === 0 ? `${value / 1024} GB` : `${value} MB`
}

export function cores(millicores: number): string {
  const value = millicores / 1000
  return `${Number(value.toFixed(2))} ${value === 1 ? 'core' : 'cores'}`
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`
}

// "1 day", "12 hours": the largest whole unit
export function interval(seconds: number): string {
  if (seconds % 86_400 === 0) return plural(seconds / 86_400, 'day')
  if (seconds % 3_600 === 0) return plural(seconds / 3_600, 'hour')
  return plural(Math.round(seconds / 60), 'minute')
}
