import { streamJsonLines, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type LogEntry = Schemas['LogEntry']

// recent lines first, then new ones as machines write them, also from machines a deploy starts
export function followLogs(
  scope: Scope,
  service: string,
  tail: number,
  onEntry: (entry: LogEntry) => void,
  signal: AbortSignal,
) {
  return streamJsonLines<LogEntry>(
    servicePath(scope, service, `/logs?follow=true&tail=${tail}`),
    onEntry,
    signal,
  )
}
