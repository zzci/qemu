// vmd HTTP/WS endpoints (same origin as this page in production).

export type PowerAction = 'start' | 'shutdown' | 'reset' | 'poweroff'

export async function power(action: PowerAction): Promise<string> {
  const r = await fetch(`/power/${action}`, { method: 'POST' })
  return (await r.text()).trim() || `${r.status}`
}

export async function status(): Promise<string> {
  try {
    const r = await fetch('/status')
    return (await r.text()).trim()
  } catch {
    return 'unreachable'
  }
}

export interface PortForward {
  proto: string
  host: number
  guest: number
}

// Mirrors the supervisor's /info payload: live run-state plus the static VM facts.
export interface VmInfo {
  status: string
  name: string
  accel: string
  cpu: string
  cpus: number
  ram: string
  disk: string
  disk_size: string
  uuid: string
  mac: string
  tpm: boolean
  web_port: number
  port_forwards: PortForward[]
  command: string
}

export async function info(): Promise<VmInfo | null> {
  try {
    const r = await fetch('/info')
    if (!r.ok) return null
    return (await r.json()) as VmInfo
  } catch {
    return null
  }
}

export function wsUrl(path: string): string {
  const proto = location.protocol === 'https:' ? 'wss' : 'ws'
  return `${proto}://${location.host}${path}`
}
