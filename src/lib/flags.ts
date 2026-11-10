import type { IpInfo } from '../types/ip-monitor'

/**
 * Country flags, bundled rather than fetched.
 *
 * A network-diagnosis tool must not depend on a CDN to draw its own card: the
 * moment the network is the thing under test, remote icons are exactly what
 * stops rendering. The whole set (271 flags, 4:3) ships inside the bundle.
 */
const modules = import.meta.glob('../assets/flags/*.svg', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>

const byCode = new Map<string, string>()
for (const [path, url] of Object.entries(modules)) {
  const match = /\/([a-z]{2})\.svg$/.exec(path)
  if (match) byCode.set(match[1], url)
}

/**
 * Resolve a flag asset for an ISO 3166-1 alpha-2 code.
 *
 * The code is matched against a fixed table, so a malformed or hostile value
 * from the API can never be turned into a path.
 */
export function flagUrl(code: string | null | undefined): string | null {
  if (!code || !/^[A-Za-z]{2}$/.test(code)) return null
  return byCode.get(code.toLowerCase()) ?? null
}

/** `City, Country` as the card has always rendered it. */
export function placeText(info: IpInfo | null | undefined): string {
  if (!info) return 'Unknown'
  const parts = [info.cityName, info.countryName ?? info.countryCode].filter(
    (part): part is string => Boolean(part),
  )
  return parts.length > 0 ? parts.join(', ') : 'Unknown'
}
