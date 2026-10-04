// @ts-nocheck -- plain Node script; the files it imports are type-checked by svelte-check
// Checks the TypeScript mirrors of the engine against what the engine itself answers
// (src/lib/mock/fixtures.json, written by `ts_mirror_fixtures` in crates/uncoil-core/src/effect.rs):
//   effect.ts    every effect kind and a studio composition, sampled on the desk at several times (±1 step)
//   effects.ts   the catalogue lists every effect kind, each with the engine's fields
//   keys.ts      every key name the picker offers is one the engine knows
//   mock config  the browser mock's config.json is Config::default()
// Plain Node (it strips the TypeScript types itself), no dependencies. Run by `pnpm check`.
import { readFileSync } from 'node:fs';

const fx = JSON.parse(readFileSync(new URL('../src/lib/mock/fixtures.json', import.meta.url), 'utf8'));
const { frameWith } = await import('../src/lib/effect.ts');
const { EFFECTS } = await import('../src/lib/effects.ts');
const { KEY_GROUPS } = await import('../src/lib/keys.ts');
const { defaultConfig } = await import('../src/lib/mock/config.ts');

const problems = [];

// effect.ts: colours within one step per channel
const inputs = { presses: fx.inputs.presses, audio: fx.inputs.audio, bounds: fx.inputs.bounds, keyboardCenter: fx.inputs.keyboard_center };
let samples = 0;
for (const s of fx.samples) {
	fx.times.forEach((t, i) => {
		const at = frameWith(s.effect, t, fx.sat, fx.val, inputs);
		fx.points.forEach(([device, shape, x, y], j) => {
			const got = at(device, shape, x, y);
			const want = s.colors[i][j];
			samples++;
			if (got.some((c, k) => Math.abs(c - want[k]) > 1)) {
				problems.push(`effect.ts: ${s.name} at t=${t}, ${shape || `(${x}, ${y})`}: [${got}] but the engine gives [${want}]`);
			}
		});
	});
}

// effects.ts: one catalogue entry per kind, with the engine's fields
const fields = (o) => Object.keys(o).sort().join(', ');
for (const [kind, engine] of Object.entries(fx.effect_defaults)) {
	const info = EFFECTS.find((e) => e.kind === kind);
	if (!info) problems.push(`effects.ts: no entry for the ${kind} effect`);
	else if (fields(info.make()) !== fields(engine)) problems.push(`effects.ts: ${kind} makes {${fields(info.make())}}, the engine has {${fields(engine)}}`);
}
for (const e of EFFECTS) if (!(e.kind in fx.effect_defaults)) problems.push(`effects.ts: ${e.kind} is not an engine effect`);

// keys.ts: every key the picker offers parses in the engine
const known = new Set(fx.usage_names);
for (const g of KEY_GROUPS) for (const k of g.keys) if (!known.has(k)) problems.push(`keys.ts: ${g.label} offers ${k}, which the engine does not know`);

// mock config: Config::default(), numbers compared as the engine's f32
const same = (a, b) =>
	typeof a === 'number' && typeof b === 'number'
		? Math.fround(a) === Math.fround(b)
		: a && b && typeof a === 'object' && typeof b === 'object'
			? fields(a) === fields(b) && Object.keys(a).every((k) => same(a[k], b[k]))
			: a === b;
if (!same(defaultConfig(), fx.config_default)) {
	problems.push(`mock/config.ts: ${JSON.stringify(defaultConfig())} is not Config::default() ${JSON.stringify(fx.config_default)}`);
}

if (problems.length) {
	console.error(`TypeScript mirrors differ from the engine (regenerate fixtures with UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-core ts_mirror if the engine changed on purpose):\n  ${problems.join('\n  ')}`);
	process.exit(1);
}
console.log(`mirror check: ${samples} colour samples, ${EFFECTS.length} effects, ${known.size} key names and the default config match the engine`);
