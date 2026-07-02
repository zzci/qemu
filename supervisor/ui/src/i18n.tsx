import { createContext, useContext, useState, type ReactNode } from 'react'

export type Lang = 'zh' | 'en'

// UI string table. `t(key)` looks up the active language; `ts(raw)` translates a server/state word
// (running/paused/connecting/…) by the same table, falling back to the raw value.
const dict = {
  zh: {
    vcpu: 'vCPU',
    mem: '内存',
    disk: '磁盘',
    accel: '加速',
    cores: '核',
    diskPath: '磁盘路径',
    webPort: 'Web 端口',
    tpm: 'TPM',
    enabled: '启用',
    disabled: '禁用',
    openVnc: '打开 VNC',
    openSerial: '打开控制台',
    powerOn: '开机',
    shutdown: '关机',
    restart: '重启',
    forceOff: '强制关闭',
    confirmShutdown: '关机 (ACPI)？',
    confirmRestart: '重启 (reset)？',
    confirmForceOff: '强制关闭 (quit)？',
    portForward: '端口转发',
    launchCmd: '启动命令',
    details: '详情',
    none: '无',
    back: '返回',
    serial: '控制台',
    grab: '抓取键盘',
    failed: '失败',
    // run / connection states (also used via ts())
    running: '运行中',
    paused: '已暂停',
    off: '已关机',
    unknown: '未知',
    unreachable: '无法连接',
    connecting: '连接中',
    connected: '已连接',
    disconnected: '已断开',
    closed: '已关闭',
  },
  en: {
    vcpu: 'vCPU',
    mem: 'Memory',
    disk: 'Disk',
    accel: 'Accel',
    cores: 'cores',
    diskPath: 'Disk path',
    webPort: 'Web port',
    tpm: 'TPM',
    enabled: 'Enabled',
    disabled: 'Disabled',
    openVnc: 'Open VNC',
    openSerial: 'Open Console',
    powerOn: 'Power On',
    shutdown: 'Shut Down',
    restart: 'Restart',
    forceOff: 'Force Off',
    confirmShutdown: 'Shut down (ACPI)?',
    confirmRestart: 'Restart (reset)?',
    confirmForceOff: 'Force off (quit)?',
    portForward: 'Port forwards',
    launchCmd: 'Launch command',
    details: 'Details',
    none: 'None',
    back: 'Back',
    serial: 'Console',
    grab: 'Grab keyboard',
    failed: 'failed',
    running: 'running',
    paused: 'paused',
    off: 'off',
    unknown: 'unknown',
    unreachable: 'unreachable',
    connecting: 'connecting',
    connected: 'connected',
    disconnected: 'disconnected',
    closed: 'closed',
  },
} as const

export type MsgKey = keyof (typeof dict)['en']

function detect(): Lang {
  const saved = localStorage.getItem('vmd_lang')
  if (saved === 'zh' || saved === 'en') return saved
  return navigator.language?.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

interface I18n {
  lang: Lang
  setLang: (l: Lang) => void
  t: (k: MsgKey) => string
  ts: (raw: string) => string
}

const Ctx = createContext<I18n | null>(null)

export function LangProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detect)
  const setLang = (l: Lang) => {
    localStorage.setItem('vmd_lang', l)
    document.documentElement.lang = l === 'zh' ? 'zh-CN' : 'en'
    setLangState(l)
  }
  const t = (k: MsgKey): string => dict[lang][k] ?? dict.en[k] ?? k
  const ts = (raw: string): string => (dict[lang] as Record<string, string>)[raw] ?? raw
  return <Ctx.Provider value={{ lang, setLang, t, ts }}>{children}</Ctx.Provider>
}

export function useI18n(): I18n {
  const c = useContext(Ctx)
  if (!c) throw new Error('useI18n must be used within LangProvider')
  return c
}

/** A compact toggle that switches to the other language (label shows the target). */
export function LangSwitch() {
  const { lang, setLang } = useI18n()
  return (
    <button
      className="lang-switch"
      title={lang === 'zh' ? 'Switch to English' : '切换到中文'}
      onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}
    >
      {lang === 'zh' ? 'EN' : '中文'}
    </button>
  )
}
