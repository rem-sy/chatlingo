import { invoke as tauriInvoke } from '@tauri-apps/api/core'

export const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
  tauriInvoke<T>(cmd, args)

export interface EngineConfig {
  baseUrl: string
  apiKey: string
  model: string
}

export interface DeeplConfig {
  apiKey: string
}

export interface AppConfig {
  engine: EngineConfig
  deepl: DeeplConfig
  compareMode: boolean
  quickbar: {
    hotkey: string
    targetLangs: string[]
    defaultTarget: string
    autoCopy: boolean
    autoPaste: boolean
  }
  clipboard: {
    monitor: boolean
    minLength: number
    translateHotkey: string
  }
  outputTarget: string
}

export const LANG_LABELS: Record<string, string> = {
  en: 'EN',
  ja: 'JA',
  ko: 'KO',
  'zh-CN': '中文',
  fr: 'FR',
  de: 'DE',
}
