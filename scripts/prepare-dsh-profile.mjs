#!/usr/bin/env node

/**
 * Build the DeepSeek Harness bridge into the profile directory the desktop
 * bundle ships (`packages/dsh-acp/dist-profile`).
 *
 * BitFun launches DeepSeek Harness as `dsh --profile bitfun-acp`, and copies
 * that directory into the user's own dsh install on first use — see
 * `src/crates/interfaces/acp/src/client/dsh_profile.rs`. The build has to
 * happen here rather than in Cargo because it is a TypeScript compile.
 *
 * `packages/dsh-acp` is deliberately NOT a pnpm workspace member: it pins the
 * whole harness `0.1.0-rc.6` train, and making every contributor's
 * `pnpm install` fetch that is a poor trade when only the desktop bundle needs
 * it. So this installs the package from its own lockfile, here, once — unless
 * a developer already linked a local harness checkout, in which case that
 * checkout wins and no install runs.
 *
 * A failure here fails the build. An app that silently ships no bridge looks
 * exactly like an app that ships a working one until a user tries to start a
 * DeepSeek session, and that is the wrong place to find out. Set
 * `BITFUN_SKIP_DSH_PROFILE=1` to opt out deliberately.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const PACKAGE_DIR = path.join(ROOT_DIR, 'packages', 'dsh-acp');
const OUT_DIR = path.join(PACKAGE_DIR, 'dist-profile');

/** One installed harness package is enough to tell a populated tree from a bare one. */
const DEPS_PROBE = path.join(PACKAGE_DIR, 'node_modules', '@deepseek-ai', 'dsh-app-boot');

/** Explains an empty dist-profile to whoever finds one in a build tree. */
const PLACEHOLDER = `This BitFun build ships no DeepSeek Harness bridge.

BITFUN_SKIP_DSH_PROFILE was set when this build ran, so packages/dsh-acp was
never compiled. BitFun's profile resolver rejects this unstamped directory and
reports that the bridge is missing from the build.

To include it: run \`pnpm run prepare:dsh-profile\` without that variable.
`;

/**
 * Run a command in the bridge package directory.
 * @param command - the executable to run.
 * @param args - its arguments.
 * @returns the exit status, with a missing status treated as a failure.
 */
function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: PACKAGE_DIR,
    shell: process.platform === 'win32',
    stdio: 'inherit',
    env: process.env,
  });
  return result.status ?? 1;
}

/**
 * Fail the build, saying what to do about it.
 * @param message - what went wrong.
 * @param status - the exit status to propagate.
 */
function fail(message, status) {
  process.stderr.write(`[dsh-profile] ${message}\n`);
  process.stderr.write(
    '[dsh-profile] set BITFUN_SKIP_DSH_PROFILE=1 to build without the DeepSeek bridge\n',
  );
  process.exit(status === 0 ? 1 : status);
}

if (process.env.BITFUN_SKIP_DSH_PROFILE === '1') {
  process.stdout.write('[dsh-profile] skipped (this build ships no DeepSeek bridge)\n');
  mkdirSync(OUT_DIR, { recursive: true });
  writeFileSync(path.join(OUT_DIR, 'NOT-BUILT.md'), PLACEHOLDER);
  process.exit(0);
}

if (!existsSync(DEPS_PROBE)) {
  process.stdout.write('[dsh-profile] installing the harness toolchain from package-lock.json\n');
  const installed = run('npm', ['ci', '--no-audit', '--no-fund']);
  if (installed !== 0) fail('npm ci failed', installed);
}

const compiled = run('npm', ['run', 'build']);
if (compiled !== 0) fail('tsc failed', compiled);

const packaged = run('node', ['scripts/build-profile.mjs']);
if (packaged !== 0) fail('profile packaging failed', packaged);
