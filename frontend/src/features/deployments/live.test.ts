import { describe, expect, it } from 'vitest'
import type { DeployLogLine } from './api'
import { dismissLive, endLive, liveDeploy, pushLive, startLive } from './live'

const line = (seq: number): DeployLogLine => ({ seq, at: 0, kind: 'bird', text: `step ${seq}` })

describe('live deploys', () => {
  it('collects lines, forgets a deploy that worked and keeps one that failed', () => {
    const key = 'shop/production/web'
    startLive(key)
    pushLive(key, line(0))
    pushLive(key, line(1))
    expect(liveDeploy(key)?.lines.map((seen) => seen.seq)).toEqual([0, 1])
    endLive(key)
    expect(liveDeploy(key)).toBeUndefined()

    startLive(key)
    pushLive(key, line(0))
    endLive(key, 'app did not answer')
    expect(liveDeploy(key)).toMatchObject({ running: false, error: 'app did not answer' })
    dismissLive(key)
    expect(liveDeploy(key)).toBeUndefined()
  })
})
