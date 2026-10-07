// Deliberate differences between the Rust app and the reference (README "Known differences") that
// show in the network layer, masked on both sides so the rest of the layer still has to match.
//
// - Cookies are only sent when they change: Rails re-sends `_campfire_session`, `session_token`
//   and `last_room` on nearly every response, and the Rust app deletes an emptied session cookie.
// - Messages omit Last-Modified: their rendered dependencies can change without record timestamps.
// - The web app manifest is JSON-escaped rather than HTML-escaped, so its body differs.

const SESSION_COOKIES = /^ {2}set-cookie: (_campfire_session|session_token|last_room)\b.*\n/gm

export function maskDeliberateNetworkDifferences(text: string): string {
  return text
    .replace(SESSION_COOKIES, "")
    .replace(/^(GET \/rooms\/[^/?\s]+\/messages(?=[?\s])[^\n]*\n)( {2}headers:[^\n]*) last-modified\b/gm, "$1$2")
    .replace(/^( {2}headers:.*) set-cookie\b/gm, "$1")
    .replace(/^(GET \/webmanifest\.json\b.*\n(?: {2}.*\n)*? {2}body: )sha256:[0-9a-f]+/gm, "$1«manifest»")
}
