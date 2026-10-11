// birdd's bird-tty frames: a kind byte, a big-endian u32 length, then the payload
const HEADER_BYTES = 5

const DATA = 0
const RESIZE = 1
const EXIT = 2
const ERROR = 3
const STDERR = 4
const EOF = 5

export type ServerFrame =
  | { kind: 'data'; bytes: Uint8Array }
  | { kind: 'stderr'; bytes: Uint8Array }
  | { kind: 'exit'; code: number }
  | { kind: 'error'; message: string }

function frame(kind: number, payload: Uint8Array): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(HEADER_BYTES + payload.length)
  const view = new DataView(out.buffer)
  view.setUint8(0, kind)
  view.setUint32(1, payload.length)
  out.set(payload, HEADER_BYTES)
  return out
}

const encoder = new TextEncoder()

export function dataFrame(text: string): Uint8Array<ArrayBuffer> {
  return frame(DATA, encoder.encode(text))
}

export function bytesFrame(bytes: Uint8Array): Uint8Array<ArrayBuffer> {
  return frame(DATA, bytes)
}

// the client's input ended, so the command reads end of file
export function eofFrame(): Uint8Array<ArrayBuffer> {
  return frame(EOF, new Uint8Array(0))
}

export function resizeFrame(cols: number, rows: number): Uint8Array<ArrayBuffer> {
  const payload = new Uint8Array(4)
  const view = new DataView(payload.buffer)
  view.setUint16(0, cols)
  view.setUint16(2, rows)
  return frame(RESIZE, payload)
}

// frames may arrive split across or joined within websocket messages
export class FrameDecoder {
  private buffer = new Uint8Array(0)
  private readonly text = new TextDecoder()

  push(chunk: Uint8Array): ServerFrame[] {
    const joined = new Uint8Array(this.buffer.length + chunk.length)
    joined.set(this.buffer)
    joined.set(chunk, this.buffer.length)
    this.buffer = joined

    const frames: ServerFrame[] = []
    while (this.buffer.length >= HEADER_BYTES) {
      const view = new DataView(this.buffer.buffer, this.buffer.byteOffset)
      const kind = view.getUint8(0)
      const length = view.getUint32(1)
      if (this.buffer.length < HEADER_BYTES + length) break
      const payload = this.buffer.slice(HEADER_BYTES, HEADER_BYTES + length)
      this.buffer = this.buffer.slice(HEADER_BYTES + length)
      if (kind === DATA) frames.push({ kind: 'data', bytes: payload })
      else if (kind === STDERR) frames.push({ kind: 'stderr', bytes: payload })
      else if (kind === EXIT && payload.length === 4) {
        frames.push({ kind: 'exit', code: new DataView(payload.buffer).getInt32(0) })
      } else if (kind === ERROR) frames.push({ kind: 'error', message: this.text.decode(payload) })
    }
    return frames
  }
}
