#!/usr/bin/env node
// Shared functional gate; keep screenshot inventories in this implementation.
import path from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"
const root = process.env.VERIFICATION_ROOT || path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../once-campfire-verification")
await import(pathToFileURL(path.join(root, "browser/smoke.mjs")).href)
