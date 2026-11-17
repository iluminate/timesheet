// Empaqueta el CLI en un solo archivo: dist/timesheet.mjs
import { createHash } from 'node:crypto';
import { chmodSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const outfile = join(root, 'dist', 'timesheet.mjs');

mkdirSync(join(root, 'dist'), { recursive: true });

await build({
  entryPoints: [join(root, 'bin', 'timesheet.mjs')],
  outfile,
  bundle: true,
  platform: 'node',
  format: 'esm',
  target: 'node20',
  external: ['playwright', 'playwright-core'],
  define: { __VERSION__: JSON.stringify(pkg.version) },
  legalComments: 'none',
  logLevel: 'warning',
});

chmodSync(outfile, 0o755);

const contents = readFileSync(outfile);
const hash = createHash('sha256').update(contents).digest('hex');
writeFileSync(`${outfile}.sha256`, `${hash}  timesheet.mjs\n`);

console.log(`dist/timesheet.mjs  ${(contents.length / 1024).toFixed(0)} KB  v${pkg.version}`);
console.log(`sha256 ${hash}`);
