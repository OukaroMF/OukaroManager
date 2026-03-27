import { mkdirSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const webrootDir = path.resolve(scriptDir, '../../module/webroot')

rmSync(webrootDir, { force: true, recursive: true })
mkdirSync(webrootDir, { recursive: true })
