// a minimal ustar writer: files only, with pax headers for long paths. Parts are kept as Blobs so
// file contents are never copied into memory before upload.
const BLOCK = 512
// the tar crate's deterministic timestamp, so unchanged files hit the build cache as with the CLI
const FIXED_MTIME = 1_153_704_088
const encoder = new TextEncoder()

export type TarEntry = { path: string; file: Blob; executable: boolean }

export function tarBlob(entries: TarEntry[]): Blob {
  const parts: BlobPart[] = []
  for (const entry of entries) {
    const name = encoder.encode(entry.path)
    if (name.length > 100) {
      const record = paxRecord('path', entry.path)
      parts.push(header('PaxHeader', record.length, 0o644, 'x'), record, padding(record.length))
    }
    parts.push(header(entry.path, entry.file.size, entry.executable ? 0o755 : 0o644, '0'), entry.file, padding(entry.file.size))
  }
  parts.push(new Uint8Array(BLOCK * 2))
  return new Blob(parts, { type: 'application/x-tar' })
}

function header(path: string, size: number, mode: number, type: '0' | 'x'): Uint8Array<ArrayBuffer> {
  const block = new Uint8Array(BLOCK)
  write(block, 0, encoder.encode(path).slice(0, 100))
  octal(block, 100, 8, mode)
  octal(block, 108, 8, 0)
  octal(block, 116, 8, 0)
  octal(block, 124, 12, size)
  octal(block, 136, 12, FIXED_MTIME)
  block.fill(0x20, 148, 156)
  block[156] = type.charCodeAt(0)
  write(block, 257, encoder.encode('ustar\u000000'))
  const sum = block.reduce((total, byte) => total + byte, 0)
  write(block, 148, encoder.encode(`${sum.toString(8).padStart(6, '0')}\u0000 `))
  return block
}

// "<length> path=<value>\n", where the length counts its own digits
function paxRecord(key: string, value: string): Uint8Array<ArrayBuffer> {
  const body = ` ${key}=${value}\n`
  const bodyBytes = encoder.encode(body).length
  let length = bodyBytes + String(bodyBytes).length
  if (String(length).length !== String(bodyBytes).length) length = bodyBytes + String(length).length
  return encoder.encode(`${length}${body}`)
}

function padding(size: number): Uint8Array<ArrayBuffer> {
  return new Uint8Array((BLOCK - (size % BLOCK)) % BLOCK)
}

function write(block: Uint8Array, offset: number, bytes: Uint8Array) {
  block.set(bytes, offset)
}

function octal(block: Uint8Array, offset: number, width: number, value: number) {
  write(block, offset, encoder.encode(value.toString(8).padStart(width - 1, '0')))
}
