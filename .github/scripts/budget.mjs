// The panel's JavaScript budget (SPEC XC-03): every script the build ships, gzipped, against a limit in kilobytes of
// 1,000 bytes.
//
//   node .github/scripts/budget.mjs <build directory> <limit in KB>

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';

const [dir, limit] = process.argv.slice(2);
if (!dir || !(Number(limit) > 0)) {
  console.error('usage: budget.mjs <build directory> <limit in KB>');
  process.exit(2);
}

const files = readdirSync(dir, { recursive: true })
  .filter((file) => file.endsWith('.js'))
  .map((file) => ({ file, bytes: gzipSync(readFileSync(join(dir, file)), { level: 9 }).length }))
  .sort((a, b) => b.bytes - a.bytes);
const total = files.reduce((sum, { bytes }) => sum + bytes, 0);
const kb = (bytes) => (bytes / 1000).toFixed(1);

for (const { file, bytes } of files.slice(0, 5)) console.log(`${kb(bytes).padStart(7)} KB  ${file}`);
console.log(`${kb(total)} KB of JavaScript in ${files.length} files, gzipped; the budget is ${limit} KB.`);
if (total > Number(limit) * 1000) {
  console.error(`Over the budget by ${kb(total - Number(limit) * 1000)} KB.`);
  process.exit(1);
}
