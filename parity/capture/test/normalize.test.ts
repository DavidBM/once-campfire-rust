import { test } from "node:test"
import assert from "node:assert/strict"
import { closeReferenceTransferForm, maskText, normalizeDocument } from "../normalize.ts"

const seedTime = Date.parse("2026-03-02T16:00:00Z")

test("transfer comparison permits only the explicit closing tag on its empty PUT form", () => {
  const before = '<main><form data-controller="auto-submit" action="/session/transfers/example" method="post"><input name="_method" value="put">'
  const after = '\n<footer id="footer"></footer></main>'
  const frozen = before + after
  const closed = before + "</form>" + after
  assert.equal(closeReferenceTransferForm(frozen), closed)
  assert.equal(closeReferenceTransferForm(closed), closed)
  assert.equal(normalizeDocument(frozen), normalizeDocument(closed))
  assert.notEqual(normalizeDocument(frozen), normalizeDocument(closed.replace('value="put"', 'value="patch"')))
  for (const other of [frozen.replace("/session/transfers/", "/join/"),
                       frozen.replace('data-controller="auto-submit"', 'data-controller="other"'),
                       frozen + before,
                       frozen.replace("\n<footer", "<button>Unexpected content</button>\n<footer")]) {
    assert.equal(closeReferenceTransferForm(other), other)
  }
})

test("decodes signed ids behind a URL path", () => {
  const url = "/rails/active_storage/blobs/redirect/eyJfcmFpbHMiOnsiZGF0YSI6NSwicHVyIjoiYmxvYl9pZCJ9fQ==--4ceb3a7460a929db324ca5fd0dffee9c8527bfad/moon.jpg"
  assert.equal(maskText(url, { seedTime }), "/rails/active_storage/blobs/redirect/«signed_id:blob_id:5»/moon.jpg")
})

test("decodes avatar tokens", () => {
  const token = "eyJfcmFpbHMiOnsiZGF0YSI6MTI3MzI2MTQxLCJwdXIiOiJ1c2VyL2F2YXRhciJ9fQ--0ac8233a4786ee6c413408416bb81e7107534db5702d351533dc2ecd3876e97f"
  assert.equal(maskText(`/users/${token}/avatar`, { seedTime }), "/users/«signed_id:user/avatar:127326141»/avatar")
})

test("decodes turbo signed stream names", () => {
  const name = "IloybGtPaTh2WTJGdGNHWnBjbVV2VW05dmJYTTZPa05zYjNObFpDODJOVFEyTXpJNE56WTptZXNzYWdlcyI=--02b7b2332eb7e12ec765aa332a46ce0a08a11c822d2ebd26710899f01d4392bd"
  assert.equal(maskText(name, { seedTime }), "«stream:Rooms::Closed#654632876:messages»")
})

test("a token for another record still differs", () => {
  const a = maskText("eyJfcmFpbHMiOnsiZGF0YSI6NSwicHVyIjoiYmxvYl9pZCJ9fQ==--4ceb3a7460a929db324ca5fd0dffee9c8527bfad", { seedTime })
  const b = maskText("eyJfcmFpbHMiOnsiZGF0YSI6NiwicHVyIjoiYmxvYl9pZCJ9fQ==--4ceb3a7460a929db324ca5fd0dffee9c8527bfad", { seedTime })
  assert.notEqual(a, b)
})

test("normalizes attributes, whitespace and timestamps, and drops csrf tags", () => {
  const html = `<!DOCTYPE html><html><head><meta name="csrf-token" content="abc"></head><body>
    <div  id="x"   class="b a" data-message-timestamp="1772463600000">  hello
      world </div><time datetime="2026-03-02T15:00:00Z"></time><pre>  keep\n me</pre></body></html>`
  assert.equal(normalizeDocument(html, { seedTime }), [
    "<!DOCTYPE html>",
    "<html>",
    "  <head>",
    "  </head>",
    "  <body>",
    `    <div class="b a" data-message-timestamp="«epochms-3600s»" id="x">`,
    "      hello world",
    "    </div>",
    `    <time datetime="«t-3600s»">`,
    "    </time>",
    "    <pre>",
    `      "  keep\\n me"`,
    "    </pre>",
    "  </body>",
    "</html>",
    "",
  ].join("\n"))
})

test("decodes consecutive tokens in one URL", () => {
  const url = "/rails/active_storage/representations/redirect/eyJfcmFpbHMiOnsiZGF0YSI6NSwicHVyIjoiYmxvYl9pZCJ9fQ==--4ceb3a7460a929db324ca5fd0dffee9c8527bfad/eyJfcmFpbHMiOnsiZGF0YSI6eyJmb3JtYXQiOiJqcGciLCJyZXNpemVfdG9fbGltaXQiOlsxMjAwLDgwMF19LCJwdXIiOiJ2YXJpYXRpb24ifX0=--28426ca1e33b0fea71b8b10b7f52a844de5886cf/moon.jpg"
  assert.equal(maskText(url, { seedTime }), `/rails/active_storage/representations/redirect/«signed_id:blob_id:5»/«signed_id:variation:{"format":"jpg","resize_to_limit":[1200,800]}»/moon.jpg`)
})

test("compares a message's copy link by its path", () => {
  const button = (attr: string) => `<button title="Copy link" data-controller="copy-to-clipboard" ${attr}></button>`
  const rails = normalizeDocument(button(`data-copy-to-clipboard-content-value="http://reference.test:3000/rooms/1/@2"`))
  const port = normalizeDocument(button(`data-copy-to-clipboard-url-value="/rooms/1/@2"`))
  assert.equal(rails, port)
  assert.match(port, /data-copy-to-clipboard-url-value="\/rooms\/1\/@2"/)
  const invite = normalizeDocument(`<button title="Copy" data-copy-to-clipboard-content-value="http://a.test/join/x"></button>`)
  assert.match(invite, /content-value="http:\/\/a.test\/join\/x"/)
})
