#!/usr/bin/env node
/**
 * stage-dist.mjs —— 生成自包含 Windows 分发目录：exe 与 assets 同级，双击即可运行。
 *
 * 用法：cargo build --release && node scripts/stage-dist.mjs
 * 输出：dist-win/dsh-pet-rust.exe + dist-win/assets/（含 config.jsonc/webm/fonts/pic）
 */
import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url)).replace(/[\\/]$/, '');
const OUT = join(ROOT, 'dist-win');

const releaseExeCandidates = [
  process.env.CARGO_TARGET_DIR ? join(process.env.CARGO_TARGET_DIR, 'release', 'dsh-pet-rust.exe') : null,
  join(ROOT, 'target', 'release', 'dsh-pet-rust.exe'),
].filter(Boolean);

const exe = releaseExeCandidates.find((p) => existsSync(p));
if (!exe) {
  console.error('[stage] 未找到 release exe，请先 cargo build --release');
  process.exit(1);
}

rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });
cpSync(exe, join(OUT, 'dsh-pet-rust.exe'));
cpSync(join(ROOT, 'assets'), join(OUT, 'assets'), { recursive: true });

console.log(`[stage] ✓ 分发目录：${OUT}`);
console.log('[stage] 双击 dist-win/dsh-pet-rust.exe 即可运行（assets 同级自动识别）');
