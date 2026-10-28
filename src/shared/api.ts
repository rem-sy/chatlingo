import { invoke as tauriInvoke } from '@tauri-apps/api/core'

export const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
  tauriInvoke<T>(cmd, args)

export interface EngineConfig {
  baseUrl: string
  apiKey: string
  model: string
}

export interface EngineConfig {
  enabled: boolean
  baseUrl: string
  apiKey: string
  model: string
}

export interface DeeplConfig {
  enabled: boolean
  apiKey: string
}

export interface AliyunConfig {
  enabled: boolean
  accessKeyId: string
  accessKeySecret: string
}

export interface GlossaryConfig {
  enabled: boolean
  maxTerms: number
}

export interface GlossaryEntry {
  id: number
  source: string
  target: string
  note: string
  enabled: boolean
  createdAt: number
}

export interface AppConfig {
  engine: EngineConfig
  deepl: DeeplConfig
  aliyun: AliyunConfig
  glossary: GlossaryConfig
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
    visionModel: string
  }
  outputTarget: string
}

/** 全量语言目录：设置页勾选可用语言，面板胶囊从此生成 */
export interface LangDef {
  code: string
  label: string
  name: string
}

export const LANG_CATALOG: LangDef[] = [
  { code: 'en', label: 'EN', name: '英语' },
  { code: 'ja', label: 'JA', name: '日语' },
  { code: 'ko', label: 'KO', name: '韩语' },
  { code: 'zh-CN', label: '中文', name: '简体中文' },
  { code: 'fr', label: 'FR', name: '法语' },
  { code: 'de', label: 'DE', name: '德语' },
  { code: 'es', label: 'ES', name: '西班牙语' },
  { code: 'ru', label: 'RU', name: '俄语' },
  { code: 'pt', label: 'PT', name: '葡萄牙语' },
  { code: 'it', label: 'IT', name: '意大利语' },
  { code: 'th', label: 'TH', name: '泰语' },
  { code: 'vi', label: 'VI', name: '越南语' },
  { code: 'ar', label: 'AR', name: '阿拉伯语' },
  { code: 'id', label: 'ID', name: '印尼语' },
  { code: 'hi', label: 'HI', name: '印地语' },
]

export const langDef = (code: string): LangDef =>
  LANG_CATALOG.find((l) => l.code === code) ?? { code, label: code.toUpperCase(), name: code }

export const langLabel = (code: string): string => langDef(code).label

/** 各渠道支持的目标语言（与 Rust 端 supports_target 保持一致；llm 全量支持） */
export const SUPPORTED_LANGS: Record<'llm' | 'deepl' | 'aliyun', string[]> = {
  llm: LANG_CATALOG.map((l) => l.code),
  deepl: ['en', 'ja', 'ko', 'zh-CN', 'fr', 'de', 'es', 'ru', 'pt', 'it', 'id'],
  aliyun: ['en', 'ja', 'ko', 'zh-CN', 'fr', 'de', 'es', 'ru', 'pt', 'it', 'th', 'vi', 'ar'],
}

