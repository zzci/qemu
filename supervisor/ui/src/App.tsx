import { useEffect, useRef, useState } from 'react'
import VncPanel, { type ConnState, type VncHandle } from './VncPanel'
import ConsolePanel from './ConsolePanel'
import { info, power, type PowerAction, type VmInfo } from './api'
import { LangSwitch, useI18n } from './i18n'

type View = 'home' | 'vnc' | 'serial'

export default function App() {
  const { t, ts } = useI18n()
  const [view, setView] = useState<View>('home')
  const [vm, setVm] = useState<VmInfo | null>(null)
  const [vmState, setVmState] = useState('…')
  const [toolbarVisible, setToolbarVisible] = useState(true)
  const [conn, setConn] = useState<ConnState>('connecting')
  const [msg, setMsg] = useState('')
  const vncRef = useRef<VncHandle>(null)
  const hideTimer = useRef<number | undefined>(undefined)

  // The toolbar floats over the console and auto-hides after inactivity, so it never resizes the
  // VNC/terminal canvas (no jump). Pointer movement reveals it; hovering it keeps it up.
  const revealToolbar = () => {
    setToolbarVisible(true)
    window.clearTimeout(hideTimer.current)
    hideTimer.current = window.setTimeout(() => setToolbarVisible(false), 2500)
  }
  const keepToolbar = () => window.clearTimeout(hideTimer.current)
  useEffect(() => () => window.clearTimeout(hideTimer.current), [])

  // One /info poll drives both the live run-state and the static VM facts.
  useEffect(() => {
    let live = true
    const tick = async () => {
      const i = await info()
      if (!live) return
      if (i) {
        setVm(i)
        setVmState(i.status)
      } else {
        setVmState('unreachable')
      }
    }
    tick()
    const id = setInterval(tick, 2000)
    return () => {
      live = false
      clearInterval(id)
    }
  }, [])

  // Auto-dismiss the power-action toast.
  useEffect(() => {
    if (!msg) return
    const id = setTimeout(() => setMsg(''), 3000)
    return () => clearTimeout(id)
  }, [msg])

  const act = async (action: PowerAction, confirmMsg?: string) => {
    if (confirmMsg && !window.confirm(confirmMsg)) return
    setMsg(`${action}…`)
    try {
      setMsg(await power(action))
    } catch {
      setMsg(`${action} ${t('failed')}`)
    }
  }

  const open = (target: 'vnc' | 'serial') => {
    setConn('connecting')
    setView(target)
    revealToolbar()
  }

  if (view === 'home') {
    const running = vmState === 'running'
    const warn = vmState === 'paused' || vmState === 'unreachable'
    const pillCls = running ? 'running' : warn ? 'warn' : 'stopped'

    const dash = '—'
    const stats = [
      { label: t('vcpu'), value: vm ? `${vm.cpus} ${t('cores')}` : dash },
      { label: t('mem'), value: vm?.ram ?? dash },
      { label: t('disk'), value: vm?.disk_size ?? dash },
      { label: t('accel'), value: vm ? `${vm.accel.toUpperCase()} · ${vm.cpu}` : dash },
    ]
    const meta = [
      { k: t('diskPath'), v: vm?.disk ?? dash },
      { k: 'MAC', v: vm?.mac ?? dash },
      { k: 'UUID', v: vm?.uuid ?? dash },
      { k: t('webPort'), v: vm ? String(vm.web_port) : dash },
      { k: t('tpm'), v: vm ? (vm.tpm ? t('enabled') : t('disabled')) : dash },
    ]

    return (
      <div className="home">
        <div className="home-head">
          <div className="name-row">
            <div className="icon-box">🖥️</div>
            <span className="vm-name">{vm?.name || location.hostname || 'vm'}</span>
          </div>
          <div className="head-right">
            <div className={`pill ${pillCls}`}>
              <span className="pill-dot" />
              <span className="pill-text">{ts(vmState)}</span>
            </div>
            <LangSwitch />
          </div>
        </div>

        <div className="stats">
          {stats.map((s) => (
            <div className="stat" key={s.label}>
              <span className="stat-label">{s.label}</span>
              <span className="stat-value">{s.value}</span>
            </div>
          ))}
        </div>

        <div className="actions">
          <div className="primary-row">
            <button className="btn btn-vnc" onClick={() => open('vnc')}>
              <span className="emoji">🖥️</span> {t('openVnc')}
            </button>
            {(vm?.console ?? true) && (
              <button className="btn btn-serial" onClick={() => open('serial')}>
                <span className="emoji">⌨️</span> {t('openSerial')}
              </button>
            )}
          </div>

          <div className="divider" />

          <div className="power-row">
            <button className="btn-power p-start" onClick={() => act('start')}>
              <span className="emoji">⏻</span> {t('powerOn')}
            </button>
            <button className="btn-power p-neutral" onClick={() => act('shutdown', t('confirmShutdown'))}>
              <span className="emoji">⏼</span> {t('shutdown')}
            </button>
            <button className="btn-power p-neutral" onClick={() => act('reset', t('confirmRestart'))}>
              <span className="emoji">⟳</span> {t('restart')}
            </button>
            <button className="btn-power p-danger" onClick={() => act('poweroff', t('confirmForceOff'))}>
              <span className="emoji">⚠</span> {t('forceOff')}
            </button>
          </div>
        </div>

        <div className="details">
          <div className="detail-block">
            <span className="detail-title">{t('portForward')}</span>
            {vm && vm.port_forwards.length > 0 ? (
              <div className="fwd-list">
                {vm.port_forwards.map((f) => (
                  <span className="fwd-chip" key={`${f.proto}-${f.host}-${f.guest}`}>
                    {f.host} <span className="arrow">→</span> {f.guest}
                    <span className="proto">{f.proto}</span>
                  </span>
                ))}
              </div>
            ) : (
              <span className="fwd-empty">{t('none')}</span>
            )}
          </div>

          <div className="detail-block">
            <span className="detail-title">{t('launchCmd')}</span>
            <pre className="cmd">{vm?.command || dash}</pre>
          </div>

          <div className="detail-block">
            <span className="detail-title">{t('details')}</span>
            <div className="meta">
              {meta.map((m) => (
                <div className="meta-row" key={m.k}>
                  <span className="meta-k">{m.k}</span>
                  <span className="meta-v" title={m.v}>
                    {m.v}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>

        {msg && <div className="toast">{msg}</div>}
      </div>
    )
  }

  const kind = view === 'vnc' ? 'VNC' : t('serial')
  const connCls = conn === 'connected' ? 'ok' : conn === 'connecting' ? 'wait' : 'err'

  return (
    <div className="console" onMouseMove={revealToolbar}>
      <div
        className={`toolbar${toolbarVisible ? '' : ' hidden'}`}
        onMouseEnter={keepToolbar}
        onMouseLeave={revealToolbar}
      >
        <button className="tbtn back" onClick={() => setView('home')}>
          ← {t('back')}
        </button>
        <div className="conn">
          <span className="kind">{kind}</span>
          <span className={`conn-dot ${connCls}`} />
          <span className="conn-text">{ts(conn)}</span>
        </div>
        <div className="spacer" />
        {view === 'vnc' && (
          <>
            <button className="tbtn" onClick={() => vncRef.current?.grab()}>
              {t('grab')}
            </button>
            <button className="tbtn" onClick={() => vncRef.current?.cad()}>
              Ctrl-Alt-Del
            </button>
          </>
        )}
        <LangSwitch />
      </div>

      {view === 'vnc' ? (
        <VncPanel ref={vncRef} onState={setConn} />
      ) : (
        <ConsolePanel onState={setConn} />
      )}
    </div>
  )
}
