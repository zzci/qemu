import { forwardRef, useEffect, useImperativeHandle, useRef } from 'react'
// @novnc/novnc ships no types; its `exports` maps the package root to core/rfb.js (the RFB client).
// @ts-expect-error - no bundled type declarations
import RFB from '@novnc/novnc'
import { wsUrl } from './api'

export type ConnState = 'connecting' | 'connected' | 'disconnected' | 'closed'

export interface VncHandle {
  grab: () => void
  cad: () => void
}

interface VncPanelProps {
  onState: (state: ConnState) => void
}

const VncPanel = forwardRef<VncHandle, VncPanelProps>(function VncPanel({ onState }, ref) {
  const surface = useRef<HTMLDivElement>(null)
  const rfbRef = useRef<any>(null)

  useImperativeHandle(ref, () => ({
    grab: () => rfbRef.current?.focus(),
    cad: () => rfbRef.current?.sendCtrlAltDel(),
  }))

  useEffect(() => {
    if (!surface.current) return
    onState('connecting')
    const rfb = new RFB(surface.current, wsUrl('/websockify'), {})
    rfb.scaleViewport = true
    rfb.clipViewport = true
    rfb.addEventListener('connect', () => onState('connected'))
    rfb.addEventListener('disconnect', () => onState('disconnected'))
    rfbRef.current = rfb
    return () => {
      try {
        rfb.disconnect()
      } catch {
        /* ignore */
      }
    }
    // onState is a stable setState updater from the parent.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return <div ref={surface} className="vnc-surface" />
})

export default VncPanel
