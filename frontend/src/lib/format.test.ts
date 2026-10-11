import { describe, expect, it } from 'vitest'
import { bytes, cores, interval, mebibytes, plural, timeAgo } from './format'

describe('format', () => {
  it('sizes bytes', () => {
    expect(bytes(512)).toBe('512 B')
    expect(bytes(1536)).toBe('1.5 KB')
    expect(bytes(14_315_520)).toBe('14 MB')
  })

  it('names limits the way people set them', () => {
    expect(mebibytes(512)).toBe('512 MB')
    expect(mebibytes(2048)).toBe('2 GB')
    expect(cores(1000)).toBe('1 core')
    expect(cores(500)).toBe('0.5 cores')
  })

  it('uses the largest whole unit for intervals', () => {
    expect(interval(86_400)).toBe('1 day')
    expect(interval(6 * 3600)).toBe('6 hours')
    expect(interval(90)).toBe('2 minutes')
    expect(plural(1, 'copy', 'copies')).toBe('1 copy')
  })

  it('says how long ago', () => {
    const now = 1_800_000_000_000
    expect(timeAgo(now / 1000, now)).toBe('just now')
    expect(timeAgo(now / 1000 - 7200, now)).toMatch(/2 hours ago/)
  })
})
