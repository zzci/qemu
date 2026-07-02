import { useEffect, useRef } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { wsUrl } from './api'
import type { ConnState } from './VncPanel'

interface ConsolePanelProps {
  onState: (state: ConnState) => void
}

export default function ConsolePanel({ onState }: ConsolePanelProps) {
  const surface = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!surface.current) return
    onState('connecting')
    const term = new Terminal({
      convertEol: true,
      cursorBlink: true,
      fontSize: 14,
      // no fontFamily override: use xterm's built-in monospace default
      theme: { background: '#000000', foreground: '#e8e8e8' },
    })
    const fit = new FitAddon()
    term.loadAddon(fit)
    term.open(surface.current)
    fit.fit()

    const ws = new WebSocket(wsUrl('/console'))
    ws.binaryType = 'arraybuffer'
    const dec = new TextDecoder()
    const enc = new TextEncoder()
    ws.onopen = () => onState('connected')
    ws.onclose = () => onState('closed')
    ws.onmessage = (e) =>
      term.write(typeof e.data === 'string' ? e.data : dec.decode(new Uint8Array(e.data)))
    term.onData((d) => ws.readyState === WebSocket.OPEN && ws.send(enc.encode(d)))

    // Re-fit on any container size change (window resize or toolbar show/hide).
    const ro = new ResizeObserver(() => fit.fit())
    ro.observe(surface.current)
    return () => {
      ro.disconnect()
      try {
        ws.close()
      } catch {
        /* ignore */
      }
      term.dispose()
    }
    // onState is a stable setState updater from the parent.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return <div ref={surface} className="term-surface" />
}
