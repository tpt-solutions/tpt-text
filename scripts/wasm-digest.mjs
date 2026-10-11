// Runs the tpt-wasm-check layouts under WebAssembly and writes their digests.
//
// Usage (from the repository root, after building the wasm target):
//   node scripts/wasm-digest.mjs target/wasm32-unknown-unknown/release/tpt_wasm_check.wasm \
//        crates/tpt-wasm-check/golden/digests.json
//
// Case names must match CASES in crates/tpt-wasm-check/src/lib.rs, in order.
import { readFileSync, writeFileSync } from 'node:fs';

const [wasmPath, outPath] = process.argv.slice(2);
if (!wasmPath || !outPath) {
  console.error('usage: node scripts/wasm-digest.mjs <module.wasm> <out.json>');
  process.exit(2);
}

const NAMES = [
  'latin_wrap',
  'latin_justify',
  'arabic_numbers_right',
  'mixed_fallback_center',
  'long_paragraph',
];

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const e = instance.exports;

if (e.tpt_case_count() !== NAMES.length) {
  console.error(`case count mismatch: wasm has ${e.tpt_case_count()}, script has ${NAMES.length}`);
  process.exit(1);
}

const fontDir = 'crates/tpt-text/tests/fonts';
const latin = readFileSync(`${fontDir}/NotoSans-Regular.ttf`);
const arabic = readFileSync(`${fontDir}/NotoSansArabic-Regular.ttf`);

// Allocate both buffers before taking any view, since memory may grow.
const latinPtr = e.tpt_alloc(latin.length);
const arabicPtr = e.tpt_alloc(arabic.length);
new Uint8Array(e.memory.buffer, latinPtr, latin.length).set(latin);
new Uint8Array(e.memory.buffer, arabicPtr, arabic.length).set(arabic);

const digests = {};
NAMES.forEach((name, index) => {
  const digest = BigInt.asUintN(64, e.tpt_digest(index, latinPtr, latin.length, arabicPtr, arabic.length));
  if (digest === 0xffffffffffffffffn) {
    console.error(`layout failed for ${name}`);
    process.exit(1);
  }
  digests[name] = digest.toString(16).padStart(16, '0');
});

writeFileSync(outPath, JSON.stringify(digests, null, 2) + '\n');
console.log(`wrote ${Object.keys(digests).length} digests to ${outPath}`);
