import { FitAddon } from '@xterm/addon-fit'
import { Terminal } from '@xterm/xterm'
import '@xterm/xterm/css/xterm.css'
import { useEffect, useEffectEvent, useRef } from 'react'
import type { Scope } from '@/lib/api/paths'
import { type CommandOptions, type Target, socketUrl } from '@/features/terminal/connect'
import { dataFrame, FrameDecoder, resizeFrame } from '@/features/terminal/protocol'

export type SessionEnd = { kind: 'exit'; code: number } | { kind: 'error'; message: string }

type TerminalSessionProps = {
  scope: Scope
  service: string
  target: Target
  command: string[]
  options: CommandOptions
  onEnd: (end: SessionEnd) => void
}

// one shell; unmounting closes the socket, which makes birdd hang the command up
export function TerminalSession({ scope, service, target, command, options, onEnd }: TerminalSessionProps) {
  const host = useRef<HTMLDivElement>(null)
  const reportEnd = useEffectEvent(onEnd)
  const { project, environment } = scope
  const commandKey = JSON.stringify(command)
  const { machine, skipEntrypoint } = options

  useEffect(() => {
    const element = host.current
    if (!element) return
    const terminal = new Terminal({
      cursorBlink: true,
      fontFamily: '"Geist Mono Variable", ui-monospace, SFMono-Regular, Menlo, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      cursorStyle: 'block',
      // pure black and white, like the rest of the app
      theme: { background: '#000000', foreground: '#ffffff', cursor: '#ffffff', cursorAccent: '#000000', selectionBackground: '#3d3d3d' },
    })
    const fit = new FitAddon()
    terminal.loadAddon(fit)
    terminal.open(element)
    fit.fit()
    terminal.focus()

    const url = socketUrl({ project, environment }, service, target, 'tty', JSON.parse(commandKey) as string[], {
      cols: terminal.cols,
      rows: terminal.rows,
      machine,
      skipEntrypoint,
    })
    const socket = new WebSocket(url)
    socket.binaryType = 'arraybuffer'
    const decoder = new FrameDecoder()
    let ended = false
    const end = (result: SessionEnd) => {
      if (ended) return
      ended = true
      reportEnd(result)
    }

    socket.onmessage = (event: MessageEvent<ArrayBuffer>) => {
      for (const frame of decoder.push(new Uint8Array(event.data))) {
        if (frame.kind === 'data' || frame.kind === 'stderr') terminal.write(frame.bytes)
        else end(frame)
      }
    }
    // a refused upgrade gives no reason to scripts, so the likely ones are named
    socket.onclose = () =>
      end({ kind: 'error', message: 'The connection closed. The service may be stopped, or your session expired.' })

    const input = terminal.onData((data) => {
      if (socket.readyState === WebSocket.OPEN) socket.send(dataFrame(data))
    })
    const resize = terminal.onResize(({ cols, rows }) => {
      if (socket.readyState === WebSocket.OPEN) socket.send(resizeFrame(cols, rows))
    })
    const observer = new ResizeObserver(() => fit.fit())
    observer.observe(element)

    return () => {
      ended = true
      observer.disconnect()
      input.dispose()
      resize.dispose()
      socket.close()
      terminal.dispose()
    }
  }, [project, environment, service, target, commandKey, machine, skipEntrypoint])

  return <div ref={host} className="h-full w-full" />
}
