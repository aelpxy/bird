import { Download } from 'lucide-react'
import { useEffect, useEffectEvent, useState } from 'react'
import { buttonClass } from '@/components/ui/button-styles'
import { Eyebrow } from '@/components/ui/eyebrow'
import type { Scope } from '@/lib/api/paths'
import { bytes } from '@/lib/format'
import { type CommandOptions, socketUrl, type Target } from '@/features/terminal/connect'
import { bytesFrame, eofFrame, FrameDecoder } from '@/features/terminal/protocol'
import type { SessionEnd } from './terminal-session'

// well under birdd's frame limit, and small enough to keep the socket's buffer short
const CHUNK_BYTES = 256 * 1024
const BUFFER_HIGH_WATER = 4 * 1024 * 1024

type PipeSessionProps = {
  scope: Scope
  service: string
  target: Target
  command: string[]
  options: CommandOptions
  input: File | null
  downloadName: string
  onEnd: (end: SessionEnd) => void
}

// bytes in and out exactly: a file becomes stdin, stdout becomes a download
export function PipeSession({ scope, service, target, command, options, input, downloadName, onEnd }: PipeSessionProps) {
  const [sent, setSent] = useState(0)
  const [received, setReceived] = useState(0)
  const [stderr, setStderr] = useState('')
  const [download, setDownload] = useState<string | null>(null)
  const reportEnd = useEffectEvent(onEnd)
  const { project, environment } = scope
  const commandKey = JSON.stringify(command)
  const { machine, skipEntrypoint } = options

  useEffect(() => {
    const socket = new WebSocket(socketUrl({ project, environment }, service, target, 'pipe', JSON.parse(commandKey) as string[], { machine, skipEntrypoint }))
    socket.binaryType = 'arraybuffer'
    const decoder = new FrameDecoder()
    const text = new TextDecoder()
    const stdout: Uint8Array<ArrayBuffer>[] = []
    let ended = false
    let url: string | null = null
    const end = (result: SessionEnd) => {
      if (ended) return
      ended = true
      url = URL.createObjectURL(new Blob(stdout))
      setDownload(url)
      reportEnd(result)
    }

    socket.onopen = () => {
      void (async () => {
        if (input) {
          for (let offset = 0; offset < input.size && socket.readyState === WebSocket.OPEN; offset += CHUNK_BYTES) {
            while (socket.bufferedAmount > BUFFER_HIGH_WATER) await new Promise((resolve) => setTimeout(resolve, 20))
            socket.send(bytesFrame(new Uint8Array(await input.slice(offset, offset + CHUNK_BYTES).arrayBuffer())))
            setSent(Math.min(input.size, offset + CHUNK_BYTES))
          }
        }
        if (socket.readyState === WebSocket.OPEN) socket.send(eofFrame())
      })()
    }
    socket.onmessage = (event: MessageEvent<ArrayBuffer>) => {
      for (const frame of decoder.push(new Uint8Array(event.data))) {
        if (frame.kind === 'data') {
          stdout.push(new Uint8Array(frame.bytes))
          setReceived((total) => total + frame.bytes.length)
        } else if (frame.kind === 'stderr') {
          const chunk = text.decode(frame.bytes, { stream: true })
          setStderr((current) => (current + chunk).slice(-64 * 1024))
        } else end(frame)
      }
    }
    socket.onclose = () => end({ kind: 'error', message: 'The connection closed before the command finished.' })

    return () => {
      ended = true
      socket.close()
      if (url) URL.revokeObjectURL(url)
    }
  }, [project, environment, service, target, commandKey, machine, skipEntrypoint, input])

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3 text-sm">
      <p className="tabular font-mono text-xs text-fg-muted" role="status">
        {input && <>Sent {bytes(sent)} of {bytes(input.size)} · </>}
        Received {bytes(received)}
      </p>
      {download && (
        <a href={download} download={downloadName || 'output'} className={`${buttonClass('primary', 'sm')} self-start`}>
          <Download aria-hidden className="size-4" />
          Download {downloadName || 'output'} ({bytes(received)})
        </a>
      )}
      {stderr && (
        <div className="flex flex-col gap-1">
          <Eyebrow>stderr</Eyebrow>
          <pre className="max-h-48 overflow-auto border border-line bg-canvas p-2 font-mono text-xs whitespace-pre-wrap text-bad">{stderr}</pre>
        </div>
      )}
    </div>
  )
}
