// a short, readable name for where a session signed in, like "Firefox on Linux"
export function describeAgent(agent: string | null | undefined): string {
  if (!agent) return 'Unknown device'
  if (agent.startsWith('bird/')) return 'bird CLI'
  if (agent.startsWith('curl/')) return 'curl'
  const browser = /Edg\//.test(agent)
    ? 'Edge'
    : /Firefox\//.test(agent)
      ? 'Firefox'
      : /Chrome\//.test(agent)
        ? 'Chrome'
        : /Safari\//.test(agent)
          ? 'Safari'
          : 'Browser'
  const system = /Android/.test(agent)
    ? 'Android'
    : /iPhone|iPad/.test(agent)
      ? 'iOS'
      : /Mac OS X/.test(agent)
        ? 'macOS'
        : /Windows/.test(agent)
          ? 'Windows'
          : /Linux/.test(agent)
            ? 'Linux'
            : undefined
  return system ? `${browser} on ${system}` : browser
}
