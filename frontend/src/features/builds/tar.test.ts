import { describe, expect, it } from 'vitest'
import { tarBlob } from './tar'

async function bytes(blob: Blob) {
  return new Uint8Array(await blob.arrayBuffer())
}

const text = (data: Uint8Array, from: number, to: number) =>
  new TextDecoder().decode(data.slice(from, to)).replace(/\0+$/, '')

describe('tarBlob', () => {
  it('writes a ustar header, the content padded to a block, and the end marker', async () => {
    const data = await bytes(tarBlob([{ path: 'app/main.sh', file: new Blob(['#!/bin/sh\n']), executable: true }]))
    expect(data.length).toBe(512 + 512 + 1024)
    expect(text(data, 0, 100)).toBe('app/main.sh')
    expect(text(data, 100, 107)).toBe('0000755')
    expect(parseInt(text(data, 124, 135), 8)).toBe(10)
    expect(text(data, 257, 263)).toBe('ustar')
    const stored = parseInt(text(data, 148, 154), 8)
    const header = data.slice(0, 512)
    header.fill(0x20, 148, 156)
    expect(header.reduce((total, byte) => total + byte, 0)).toBe(stored)
    expect(text(data, 512, 522)).toBe('#!/bin/sh\n')
  })

  it('puts long paths in a pax header first', async () => {
    const path = `${'deep/'.repeat(30)}file.txt`
    const data = await bytes(tarBlob([{ path, file: new Blob(['x']), executable: false }]))
    expect(String.fromCharCode(data[156])).toBe('x')
    const size = parseInt(text(data, 124, 135), 8)
    const record = text(data, 512, 512 + size)
    expect(record).toBe(`${size} path=${path}\n`)
  })
})
