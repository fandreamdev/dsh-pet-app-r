#!/usr/bin/env node
/**
 * sync-from-siblings.mjs —— 开发期便利工具：从上游本地副本刷新素材与纯逻辑产物。
 *
 * 本仓库自包含：即使不运行本脚本也能编译/运行。本脚本仅用于想跟随上游更新时手动同步。
 *
 * 默认来源：../dsh-pet-app（可用 UPSTREAM=... 指定仓库根，需含 assets/ 与 renderer/）。
 * 复制范围（白名单，绝不覆盖本仓库自行维护的适配文件）：
 *   - assets/（config.jsonc + webm + fonts + pic）
 *   - frontend/shared-core.js（纯逻辑构建产物，与上游一致）
 *
 * 不会覆盖：frontend/sprite.js、constants.js、bridge.js、renderer.js、settings.*（本仓库维护）。
 */
import { cpSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url)).replace(/[\\/]$/, '');
const UP = process.env.UPSTREAM || join(ROOT, '..', 'dsh-pet-app');

const ITEMS = [
  ['assets', 'assets'],
  ['renderer/shared-core.js', 'frontend/shared-core.js'],
];

if (!existsSync(join(UP, 'assets'))) {
  console.error(`[sync] 未找到上游副本：${UP}（可用 UPSTREAM= 指定，或跳过本脚本——仓库已自带素材）`);
  process.exit(1);
}
for (const [from, to] of ITEMS) {
  const f = join(UP, from);
  const t = join(ROOT, to);
  if (!existsSync(f)) {
    console.warn(`[sync] 跳过（${from} 不存在）`);
    continue;
  }
  mkdirSync(dirname(t), { recursive: true });
  cpSync(f, t, { recursive: true });
  console.log(`[sync] ✓ ${from} -> ${to}`);
}
console.log('[sync] 完成（注意：本仓库自维护的 sprite/bridge/constants/renderer/settings 不会被覆盖）');
