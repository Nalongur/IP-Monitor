/**
 * Core data contract — mirrors spec §7.2.
 *
 * Every field name crossing the Vue component boundary is camelCase.
 * The Rust side serializes with `#[serde(rename_all = "camelCase")]`,
 * so these names are the wire format as well.
 */

/** How the outbound request reaches ipleak.net (spec §2.3). */
export type DetectionMode = 'direct' | 'system_proxy' | 'custom_proxy'

/** Per-field outcome, independent for IPv4 and IPv6 (spec §7.2). */
export type FieldStatus = 'success' | 'unavailable' | 'timeout' | 'error'

/** Normalized IPv4 lookup payload. Missing values stay `null`. */
export interface IpInfo {
  ip: string | null
  countryCode: string | null
  countryName: string | null
  cityName: string | null
  ispName: string | null
  asNumber: number | null
  asNetwork: string | null
  timeZone: string | null
}

/** One completed detection attempt (spec §7.2). */
export interface DetectionResult {
  mode: DetectionMode
  ipv4: IpInfo | null
  ipv4Status: FieldStatus
  ipv6: string | null
  ipv6Status: FieldStatus
  /** Full IPv6 record; the contract's ipv6 above stays just the address. */
  ipv6Info?: IpInfo | null
  /** ISO 8601 timestamp of when the request was sent. */
  checkedAt: string
  durationMs: number
  /** Credential-free description of where the request went. */
  route?: string
  /** Failure reason from the backend; `null` when the run produced an address. */
  detail?: string | null
}

/** Card-level status shown in the header pill (spec §8.2). */
export type ConnectionStatus =
  | 'idle'
  | 'checking'
  | 'connected'
  | 'partial'
  | 'error'
  | 'stale'

/** Single-reader state shape (spec §7.3). */
export interface MonitorState {
  mode: DetectionMode
  result: DetectionResult | null
  isLoading: boolean
  error: string | null
  modeMenuOpen: boolean
  lastUpdated: Date | null
}

/** One address family's place line; `tag` only appears when both are shown. */
export interface PlaceRow {
  tag: 'v4' | 'v6' | null
  /** Resolved flag asset URL, or `null` when the country is unknown. */
  flag: string | null
  text: string
}

/** Derived cell for the 2×2 information grid (view model, not wire data). */
export interface IpInfoItem {
  label: string
  value: string
  /** True when the value is a placeholder rather than reported data. */
  empty: boolean
  /** Present on cells that stack one line per address family. */
  rows?: PlaceRow[]
}

/** One row of the detection-mode popover. */
export interface DetectionModeOption {
  value: DetectionMode
  label: string
  /** Static hint only; no global shortcut is bound in Phase 1. */
  hint: string
}

/** Wire protocol for a hand-configured proxy (spec §2.3 C). */
export type ProxyProtocol = 'http' | 'https' | 'socks5'

/** What the settings panel sends back; password may be null to keep the session one. */
export interface CustomProxyConfig {
  protocol: ProxyProtocol
  host: string
  port: number
  username: string | null
  password: string | null
}

/** What the backend is willing to hand out: never the password itself. */
export interface CustomProxyView {
  protocol: ProxyProtocol
  host: string
  port: number
  username: string | null
  hasPassword: boolean
}
