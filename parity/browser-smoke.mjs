#!/usr/bin/env node
// Cross-implementation browser flows, adapted from once-campfire-go/bin/check-browser.mjs.
// Run against a disposable, freshly installed production server: node parity/browser-smoke.mjs --base URL.
// This functional gate complements the screenshot inventory.
import { mkdtemp, rm } from "node:fs/promises"
import { createRequire } from "node:module"
import { fileURLToPath } from "node:url"
import path from "node:path"

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
const require = createRequire(import.meta.url)
const { chromium } = require(path.join(root, "parity/node_modules/playwright"))
const baseIndex = process.argv.indexOf("--base")
const base = baseIndex < 0 ? null : process.argv[baseIndex + 1]
if (!base || !/^https?:\/\//.test(base))
  throw new Error("--base must specify a disposable fresh server")
const work = await mkdtemp("/tmp/campfire-browser-smoke-")
let browser
try {
  let ready = false
  for (let i = 0; i < 100; i++) {
    try {
      ready = (await fetch(`${base}/up`)).ok
    } catch {}
    if (ready) break
    await new Promise((resolve) => setTimeout(resolve, 50))
  }
  if (!ready) throw new Error(`Server did not become ready: ${base}`)
  browser = await chromium.launch({
    headless: true,
    env: { ...process.env, TMPDIR: work },
  })
  const context = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  })
  const errors = []
  function watch(page) {
    if (process.env.DEBUG_BROWSER) {
      page.on("response", async (res) => {
        if (
          res.request().method() !== "GET" ||
          new URL(res.url()).pathname.startsWith("/rooms/") ||
          res.status() >= 400
        )
          console.error(
            res.status(),
            res.url(),
            (await res.text().catch(() => "")).slice(0, 500),
          )
      })
      page.on("websocket", (ws) =>
        ws.on("framereceived", ({ payload }) =>
          console.error("WS", String(payload).slice(0, 700)),
        ),
      )
    }
    page.on("pageerror", (error) => {
      errors.push(String(error))
      if (process.env.DEBUG_BROWSER) console.error(error)
    })
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text())
        if (process.env.DEBUG_BROWSER) console.error(msg.text())
      }
    })
    return new Promise((resolve) =>
      page.on("websocket", (socket) =>
        socket.on("framereceived", ({ payload }) => {
          const frame = JSON.parse(String(payload))
          if (
            frame.type === "confirm_subscription" &&
            JSON.parse(frame.identifier).channel === "RoomMessagesChannel"
          )
            resolve()
        }),
      ),
    )
  }
  const first = await context.newPage()
  const firstSubscribed = watch(first)
  await first.goto(`${base}/first_run`)
  await first.locator('[name="user[name]"]').fill("Browser User")
  await first
    .locator('[name="user[email_address]"]')
    .fill("browser@example.test")
  await first.locator('[name="user[password]"]').fill("browser test password")
  await Promise.all([
    first.waitForURL("**/rooms/*"),
    first
      .locator('form:has(input[name="user[name]"]) button[type="submit"]')
      .click(),
  ])
  const roomURL = first.url()
  const second = await context.newPage()
  const secondSubscribed = watch(second)
  await second.goto(first.url())
  await Promise.race([
    Promise.all([firstSubscribed, secondSubscribed]),
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error("subscription timeout")),
        10000,
      ).unref(),
    ),
  ])
  await first
    .getByRole("textbox", { name: "Write a message", exact: true })
    .fill("Hello from the first tab")
  await first.getByRole("button", { name: "Send Message", exact: true }).click()
  await second
    .locator(".messages > .message[data-message-id]")
    .filter({ hasText: "Hello from the first tab" })
    .waitFor()
  await second
    .getByRole("textbox", { name: "Write a message", exact: true })
    .fill("Reply from the second tab")
  await second
    .getByRole("button", { name: "Send Message", exact: true })
    .click()
  await first
    .locator(".messages > .message[data-message-id]")
    .filter({ hasText: "Reply from the second tab" })
    .waitFor()
  await first.locator("lexxy-editor#message_body").evaluate((el) => {
    el.value =
      '<img src=x onerror="window.injected=true"><b>safe browser text</b>'
  })
  await first.getByRole("button", { name: "Send Message", exact: true }).click()
  await second
    .locator(".messages > .message[data-message-id]")
    .filter({ hasText: "safe browser text" })
    .waitFor()
  for (const page of [first, second]) {
    if (
      (await page.locator(".messages > .message[data-message-id]").count()) !==
      3
    )
      throw new Error("duplicate or missing message")
    if (await page.evaluate(() => window.injected === true))
      throw new Error("stored markup executed")
  }
  const messageID = await first
    .locator(".messages > .message[data-message-id]")
    .filter({ hasText: "Hello from the first tab" })
    .getAttribute("id")
  const message = first.locator(`[id="${messageID}"]`)
  await message.locator("summary").click()
  await message.getByRole("button", { name: "Copy link", exact: true }).click()
  const copiedLink = await first.evaluate(() => navigator.clipboard.readText())
  const numericID = await message.getAttribute("data-message-id")
  if (copiedLink !== `${roomURL}/@${numericID}`)
    throw new Error(`Incorrect message permalink: ${copiedLink}`)
  await message.getByRole("link", { name: "Edit", exact: true }).click()
  await message
    .getByRole("textbox", { name: "Edit message", exact: true })
    .fill("Edited in the browser")
  await message
    .getByRole("button", { name: "Save changes", exact: true })
    .click()
  await second
    .locator(".messages > .message[data-message-id]")
    .filter({ hasText: "Edited in the browser" })
    .waitFor()
  await first.goto(`${base}/searches?q=Edited`)
  await first
    .locator("#search-results > .message")
    .filter({ hasText: "Edited in the browser" })
    .waitFor()
  await first.goto(`${base}/users/me/profile`)
  await first.locator('[name="user[bio]"]').fill("Browser profile update")
  await first.getByRole("button", { name: "Save changes", exact: true }).click()
  await first.waitForFunction(
    () =>
      document.querySelector('[name="user[bio]"]').value ===
      "Browser profile update",
  )
  const transferURL = await first.locator("#session_transfer_url").inputValue()
  if (!transferURL.startsWith(`${base}/session/transfers/`))
    throw new Error("Missing transfer URL")
  const qrURL = new URL(
    await first
      .getByRole("link", { name: "Show auto-login QR code" })
      .getAttribute("href"),
    base,
  ).href
  const qr = await first.evaluate(async (url) => {
    const response = await fetch(url)
    const blob = await response.blob()
    const image = new Image()
    const source = URL.createObjectURL(blob)
    try {
      image.src = source
      await image.decode()
      return {
        status: response.status,
        type: blob.type,
        width: image.naturalWidth,
        height: image.naturalHeight,
      }
    } finally {
      URL.revokeObjectURL(source)
    }
  }, qrURL)
  if (
    qr.status !== 200 ||
    !["image/svg+xml", "image/png"].includes(qr.type) ||
    qr.width <= 0 ||
    qr.height <= 0
  )
    throw new Error(`QR code failed: ${JSON.stringify(qr)}`)
  await first.goto(`${base}/account/edit`)
  await first.locator('[name="account[name]"]').fill("Browser Campfire")
  await first.getByRole("button", { name: "Save changes", exact: true }).click()
  await first.waitForFunction(
    () =>
      document.querySelector('[name="account[name]"]').value ===
      "Browser Campfire",
  )
  await first.goto(`${base}/rooms/opens/new`)
  await first
    .getByRole("textbox", { name: "Name this room", exact: true })
    .fill("Browser room")
  await Promise.all([
    first.waitForURL(/\/rooms\/\d+$/),
    first.getByRole("button", { name: "Save", exact: true }).click(),
  ])
  await second
    .locator("#shared_rooms a")
    .filter({ hasText: "Browser room" })
    .waitFor()
  const newRoomURL = first.url()
  await first
    .getByRole("link", { name: /^(Settings for this room|Room settings)$/ })
    .click()
  await first.waitForFunction(
    () =>
      document.querySelector('[name="room[name]"]') ||
      Array.from(document.querySelectorAll("a")).some(
        (node) => node.textContent === "Edit room",
      ),
  )
  if (await first.getByRole("link", { name: "Edit room", exact: true }).count())
    await first.getByRole("link", { name: "Edit room", exact: true }).click()
  await first
    .getByRole("textbox", { name: "Name this room", exact: true })
    .fill("Renamed browser room")
  await Promise.all([
    first.waitForURL(newRoomURL),
    first.getByRole("button", { name: "Save", exact: true }).click(),
  ])
  await second
    .locator("#shared_rooms a")
    .filter({ hasText: "Renamed browser room" })
    .waitFor()
  await first.goto(`${base}/account/bots/new`)
  await first
    .getByPlaceholder("Name the bot", { exact: true })
    .fill("Browser bot")
  await Promise.all([
    first.waitForURL(`${base}/account/bots`),
    first.getByRole("button", { name: "Save changes", exact: true }).click(),
  ])
  const command = await first
    .getByRole("textbox", { name: "curl command for posting messages" })
    .first()
    .inputValue()
  const botURL = command.slice(command.lastIndexOf(" ") + 1)
  const posted = await fetch(botURL, {
    method: "POST",
    body: "Hello from browser bot",
  })
  if (!posted.ok) throw new Error(`Bot post failed: ${posted.status}`)
  await first
    .getByRole("link", { name: "Edit Browser bot", exact: true })
    .click()
  await first.locator('[name="user[name]"]').fill("Renamed browser bot")
  await Promise.all([
    first.waitForURL(`${base}/account/bots`),
    first.getByRole("button", { name: "Save changes", exact: true }).click(),
  ])
  await first
    .getByRole("link", { name: "Edit Renamed browser bot", exact: true })
    .waitFor()
  await first.goto(`${base}/account/custom_styles/edit`)
  await first
    .locator('[name="account[custom_styles]"]')
    .fill("body { --browser-test: verified; }")
  await first
    .locator('form:has(textarea[name="account[custom_styles]"]) button')
    .click()
  await first.waitForFunction(
    () =>
      getComputedStyle(document.body)
        .getPropertyValue("--browser-test")
        .trim() === "verified",
  )
  await first.goto(`${base}/account/edit`)
  if (
    (
      await first.evaluate(() =>
        getComputedStyle(document.body).getPropertyValue("--browser-test"),
      )
    ).trim() !== "verified"
  )
    throw new Error("Custom styles missing")
  const invitation = first.locator("#invite_url")
  const inviteURL = (await invitation.count())
    ? await invitation.inputValue()
    : new URL(
        await first.locator('a[href^="/join/"]').getAttribute("href"),
        base,
      ).href
  const transferContext = await browser.newContext()
  const transferred = await transferContext.newPage()
  watch(transferred)
  await transferred.goto(transferURL)
  await transferred.waitForURL(/\/rooms\/\d+$/)
  await transferred.goto(`${base}/users/me/profile`)
  if (
    (await transferred.locator('[name="user[email_address]"]').inputValue()) !==
    "browser@example.test"
  )
    throw new Error("Transfer signed in the wrong user")
  await transferContext.close()
  const joinedContext = await browser.newContext()
  const joined = await joinedContext.newPage()
  watch(joined)
  await joined.goto(inviteURL)
  await joined.locator('[name="user[name]"]').fill("Joined browser user")
  await joined
    .locator('[name="user[email_address]"]')
    .fill("joined@example.test")
  await joined.locator('[name="user[password]"]').fill("joining test password")
  await Promise.all([
    joined.waitForURL(/\/rooms\/\d+$/),
    joined
      .locator('form:has(input[name="user[name]"]) button[type="submit"]')
      .click(),
  ])
  await joined.goto(`${base}/account/edit`)
  if (await joined.getByRole("link", { name: "Set up chat bots" }).count())
    throw new Error("Member sees admin controls")
  await first.goto(roomURL)
  await first.getByRole("link", { name: "New Ping" }).click()
  const autocomplete = first.locator('[data-autocomplete-target="input"]')
  await autocomplete.fill("Joined browser")
  await first
    .locator("suggestion-option")
    .getByText("Joined browser user", { exact: true })
    .click()
  await first.waitForFunction(
    () =>
      document.querySelector('[data-autocomplete-target="select"]')
        ?.selectedOptions.length === 1,
  )
  if (process.env.DEBUG_BROWSER)
    console.error(
      await first
        .locator('[data-autocomplete-target="select"]')
        .evaluate((el) => ({
          html: el.outerHTML,
          value: el.value,
          valid: el.validity.valid,
          form: el.form?.outerHTML,
        })),
    )
  await Promise.all([
    first.waitForURL(
      (url) => /\/rooms\/\d+$/.test(url.pathname) && url.href !== roomURL,
    ),
    first.getByRole("button", { name: "Start Ping", exact: true }).click(),
  ])
  await first
    .locator("#nav")
    .getByRole("heading", {
      name: "Ping with Joined browser user",
      exact: true,
    })
    .waitFor()
  await joinedContext.close()
  if (errors.length) throw new Error(errors.join("\n"))
  console.log(
    "PASS: setup, two-tab live messaging, duplicate suppression, and stored-markup safety, copying message permalinks, editing, search, profile/account updates, QR codes, live room creation/renaming, bots, custom styles, session transfers, joining, and autocomplete-started direct pings in Chromium.",
  )
} catch (error) {
  for (const page of browser?.contexts()[0]?.pages() || []) {
    console.error(await page.locator("body").innerText())
    console.error(
      await page
        .locator("turbo-cable-stream-source")
        .evaluateAll((nodes) => nodes.map((node) => node.outerHTML)),
    )
    console.error(
      await page
        .locator(".messages")
        .evaluateAll((nodes) =>
          nodes.map((node) => node.outerHTML.slice(0, 4000)),
        ),
    )
  }
  throw error
} finally {
  await browser?.close()
  await rm(work, { recursive: true, force: true })
}
