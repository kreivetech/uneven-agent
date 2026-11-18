/**
 * Uneven TypeScript public API types
 */

export interface LocalBrainConfig {
  /** Number of CPU threads to use (default: 4) */
  threads?: number
  /** Number of GPU layers to offload (default: 32) */
  gpuLayers?: number
  /** Time in minutes to keep model in RAM after last use (default: 10). Set to 0 to never unload. */
  unloadTimeout?: number
}

export interface BrainConfig {
  /** Optional ID for routing (required if using multiple brains) */
  id?: string
  provider: 'local' | 'openai' | 'claude' | 'gemini' | 'ollama' | 'deepseek' | 'mistral' | 'groq'
  model: string
  apiKey?: string
  temperature?: number
  maxTokens?: number
  /** Hardware/resource settings for local inference */
  local?: LocalBrainConfig
}

export interface DbConfig {
  url?: string
  /** Explicit list of tables allowed for database analysis and SQL generation (Mandatory for CLI compatibility) */
  tables?: string[]
  /** Alias for tables */
  allowedTables?: string[]
}

export interface KnowledgeConfig {
  dirs?: string[]
  files?: string[]
  db?: DbConfig
  urls?: string[]
  externalProject?: string
}

export interface BruteforceConfig {
  enabled?: boolean
  endpoints?: string[]
  wordlist?: string
  detectRateLimit?: boolean
  jwtWeak?: boolean
  sessionExpiry?: boolean
  rateLimit?: number
}

export interface FirewallConfig {
  enabled?: boolean
  portScan?: boolean
  cors?: boolean
  csp?: boolean
  ssl?: boolean
  hsts?: boolean
  xFrameOptions?: boolean
}

export interface ActiveConfig {
  sqlInjection?: boolean
  xssReflected?: boolean
  idor?: boolean
  directoryTraversal?: boolean
  hiddenRoutes?: boolean
}

export interface NetworkConfig {
  timeout?: number
  followRedirects?: boolean
  rateLimit?: number
}

export interface PentesterConfig {
  enabled?: boolean
  mode?: 'static' | 'active'
  target?: string
  realtime?: boolean
  bruteforce?: BruteforceConfig
  firewall?: FirewallConfig
  static?: {
    owasp?: boolean
    secrets?: boolean
    dependencies?: boolean
    injections?: boolean
    headers?: boolean
  }
  active?: ActiveConfig
  network?: NetworkConfig
  severity?: 'low' | 'medium' | 'high' | 'critical'
}

export interface WatchConfig {
  terminal?: string
  terminals?: string[]
  ignorePatterns?: string[]
  autoFix?: boolean
  confirmBeforeFix?: boolean
  alertOnly?: boolean
  dirs?: string[]
  onlyUseStrategies?: boolean
  lowCpuMode?: boolean
}

export interface ProactiveConfig {
  /** Enable proactive fixing when files are saved (default: false) */
  enabled?: boolean
  /** Automatically apply fixes without confirmation (default: false) */
  autoFix?: boolean
  /** Show diff and wait for user approval before applying (default: true) */
  confirmBeforeFix?: boolean
  /** Directories to watch for file-save triggers (default: same as knowledge.dirs) */
  dirs?: string[]
  /** Patterns to ignore for proactive fixing (e.g. ["*.test.ts", "**/vendor/**"]) */
  ignore?: string[]
  /** Patterns to ignore for proactive fixing (alias for ignore) */
  vendor?: string[]
  onlyUseStrategies?: boolean
  lowCpuMode?: boolean
}

/**
 * Configuration for the Native Zero-Day Finder feature.
 */
export interface ZeroDayConfig {
  /** Enable or disable the native Zero-Day scanning engine */
  enabled?: boolean
  /** Target directories to scan for zero-day vulnerabilities (e.g. ['./src']) */
  dirs?: string[]
  /** Sleep for 5ms every 50 files scanned to reduce CPU utilization */
  lowCpuMode?: boolean
}



export interface LogConfig {
  path?: string
  append?: boolean
  includeTimestamp?: boolean
  includeDiff?: boolean
}

export interface NotificationsConfig {
  /** Slack or Discord incoming webhook URL */
  webhookUrl?: string
  /** Minimum severity to notify (default: 'error') */
  minSeverity?: 'error' | 'warning'
  /** 
   * Maximum notifications per minute (default: 10). 
   * Prevents spamming webhooks in case of rapid events.
   */
  rateLimit?: number
}

export interface RemoteShellHttpsConfig {
  /** Path to TLS certificate file (PEM format) */
  cert: string
  /** Path to TLS private key file (PEM format) */
  key: string
}

export interface RemoteShellConfig {
  /** Local port to listen on (e.g. 4242) */
  port: number
  /** Bearer token the caller must send in Authorization header. */
  secret?: string
  /** Enable HTTPS by providing cert and key paths. Omit for plain HTTP. */
  https?: RemoteShellHttpsConfig
  /** 
   * Rate limiting settings (default: 30 requests/min with burst of 5). 
   */
  rateLimit?: {
    requestsPerMinute?: number
    burst?: number
  }
  /** 
   * CORS settings. Omit to block all cross-origin requests.
   */
  cors?: {
    allowedOrigins?: string[]
  }
}

export interface DataSecurityConfig {
  /** Column name patterns (case-insensitive) that are masked with ***REDACTED*** in results. */
  mask?: string[]
  /** Table names (exact, case-insensitive) whose entire schema is hidden from the AI. */
  block?: string[]
  /** Legacy field for blocked columns. */
  blockedColumns?: string[]
  /** Legacy field for blocked tables. */
  blockedTables?: string[]
  /** Privacy masking for reports. */
  reportRedactions?: boolean
}

export interface UnevenConfig {
  /**
   * Root directory of the project.
   * All relative paths (log.path, knowledge.dirs, etc.) are resolved relative to this root.
   */
  projectRoot?: string
  /** The primary brain used for the current command session. */
  brain: BrainConfig
  /** 
   * Available brains for hybrid routing (Pro/Team feature). 
   * When multiple brains are defined, the loader selects one to put into the 'brain' property.
   */
  brains?: BrainConfig[]
  /** 
   * Explicit routing of commands to specific brain IDs.
   * Example: { analyze: "fast-brain", review: "pro-brain" }
   */
  routing?: Record<string, string>
  knowledge?: KnowledgeConfig
  watch?: WatchConfig
  pentester?: PentesterConfig
  log?: LogConfig
  notifications?: NotificationsConfig
  /**
   * Data privacy and masking policies for DB, CSV, and Excel sources.
   */
  security?: DataSecurityConfig
  /**
   * Exposes a local HTTP/HTTPS endpoint so external tools (Discord bots,
   * Slack bots, custom scripts) can send natural-language messages to Uneven
   * and receive the result as JSON — same pipeline as the interactive shell.
   */
  remoteShell?: RemoteShellConfig
  /**
   * Independent proactive fixing service settings (observes file saves).
   */
  proactive?: ProactiveConfig
  zeroday?: ZeroDayConfig
  skills?: string
}


export interface ErrorEvent {
  file: string
  line: number
  column: number
  message: string
  stackTrace: string
}

export interface FixSuggestion {
  file: string
  line: number
  before: string
  after: string
  confidence: number
  explanation: string
}

export interface SecurityFinding {
  severity: 'critical' | 'high' | 'medium' | 'low' | 'info'
  type: string
  file?: string
  line?: number
  message: string
  recommendation: string
  cvss?: number
  match?: string
}

export interface IndexStats {
  totalDocuments: number
  diskSize: number
}

export interface RetrievalResult {
  documents: string[]
  scores: number[]
}

export type EventType =
  | 'error-detected'
  | 'fix-suggested'
  | 'fix-applied'
  | 'fix-reverted'
  | 'fix-pending'
  | 'pentest-finding'
  | 'indexed'
  | 'warning'
  | 'info'
  | 'brain-ready'
  | 'watch-started'
  | 'stopped'
  | 'initialized'
  | 'file-changed'
  | 'file-added'
  | 'file-removed'

export interface UnevenEvent {
  type: EventType
  timestamp: Date
  data: unknown
}

export interface UnevenEventHandler {
  (event: UnevenEvent): void | Promise<void>
}

// ─── Chunker ──────────────────────────────────────────────────────────────────

export interface Chunk {
  id: string
  content: string
  startLine: number
  endLine: number
  metadata: Record<string, string>
}

// ─── Database Loader ──────────────────────────────────────────────────────────

export interface DatabaseDocument {
  id: string
  path: string
  content: string
  metadata: Record<string, string>
}

// ─── Web Scraper ──────────────────────────────────────────────────────────────

export interface WebDocument {
  id: string
  url: string
  content: string
  title: string
  metadata: Record<string, string>
}

// ─── Process Watcher ──────────────────────────────────────────────────────────

export interface WatcherOptions {
  command: string
  args?: string[]
  cwd?: string
  env?: Record<string, string>
  autoRestart?: boolean
  maxRestarts?: number
}

export interface WatcherEvent {
  type: 'start' | 'output' | 'error' | 'exit' | 'restart'
  timestamp: Date
  data: unknown
}

// ─── Error Parser ─────────────────────────────────────────────────────────────

export interface ParsedError {
  type: 'compiler' | 'runtime' | 'linter' | 'test' | 'unknown'
  severity: 'error' | 'warning' | 'info'
  file: string
  line: number
  column: number
  message: string
  code: string
  context: string[]
  fullOutput: string
}

// ─── Security Analyzer (SecurityFinding defined above, SecurityAuditFinding is an alias) ──
export type SecurityAuditFinding = SecurityFinding

// ─── External Providers ───────────────────────────────────────────────────────

export interface ProviderResponse {
  content: string
  tokensUsed: number
  model: string
  provider: string
}

export type ExternalProvider = 'openai' | 'claude' | 'gemini' | 'ollama' | 'deepseek' | 'mistral' | 'groq'

// ─── LLM Response ─────────────────────────────────────────────────────────────

export interface LlmResponse {
  content: string
  tokensUsed: number
  model: string
}
