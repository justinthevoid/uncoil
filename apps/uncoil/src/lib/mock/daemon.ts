// Browser-only stand-in for uncoild's control pipe, so the device pages work in `pnpm dev`.
// ./daemon/*.json are the fake daemon's answers, written and checked by the `gui_mock_from_fake_answers` test in
// apps/uncoild/src/control.rs (regenerate: UNCOIL_UPDATE_MOCK=1 cargo test -p uncoild gui_mock). They are kept
// in memory here so writes, reads and resets behave like the daemon's.
import devices from './daemon/devices.json';
import capsKeyboard from './daemon/caps-keyboard.json';
import capsMouse from './daemon/caps-mouse.json';
import kbNormal from './daemon/keymap-keyboard-normal.json';
import kbFn from './daemon/keymap-keyboard-hypershift.json';
import mouseNormal from './daemon/keymap-mouse-normal.json';
import mouseFn from './daemon/keymap-mouse-hypershift.json';
import profilesKeyboard from './daemon/profiles-keyboard.json';
import profilesMouse from './daemon/profiles-mouse.json';
import dialState from './daemon/dial.json';
import oledState from './daemon/oled.json';
import capsDeathAdder from './daemon/caps-deathadder.json';
import deathAdderNormal from './daemon/keymap-deathadder-normal.json';
import deathAdderFn from './daemon/keymap-deathadder-hypershift.json';
import profilesDeathAdder from './daemon/profiles-deathadder.json';
import perfMouse from './daemon/performance-mouse.json';
import perfDeathAdder from './daemon/performance-deathadder.json';
import powerMouse from './daemon/power-mouse.json';
import powerDeathAdder from './daemon/power-deathadder.json';
import type { Capabilities, DeviceInfo, DialState, Dpi, Feature, FeatureCheck, KeyMapping, Layer, OledState, PerformanceState, PowerState } from '../types';
import { describeFunction } from '../keys';
import type { DaemonFailure, ErrorCode } from '../api';

const KB = 'razer-blackwidow-v4-pro-75';
const MOUSE = 'razer-basilisk-v3-pro';
/** Experimental: no lighting, checks start untested. */
const DA = 'razer-deathadder-v3-pro';

const state = {
	devices: structuredClone(devices) as DeviceInfo[],
	caps: { [KB]: structuredClone(capsKeyboard), [MOUSE]: structuredClone(capsMouse), [DA]: structuredClone(capsDeathAdder) } as unknown as Record<string, Capabilities>,
	keymap: {
		[KB]: { normal: structuredClone(kbNormal), hypershift: structuredClone(kbFn) },
		[MOUSE]: { normal: structuredClone(mouseNormal), hypershift: structuredClone(mouseFn) },
		[DA]: { normal: structuredClone(deathAdderNormal), hypershift: structuredClone(deathAdderFn) }
	} as Record<string, Partial<Record<Layer, KeyMapping[]>>>,
	performance: { [MOUSE]: structuredClone(perfMouse), [DA]: structuredClone(perfDeathAdder) } as Record<string, PerformanceState>,
	power: { [MOUSE]: structuredClone(powerMouse), [DA]: structuredClone(powerDeathAdder) } as unknown as Record<string, PowerState>,
	/** What each key held before the first write (like the daemon's journal). */
	original: new Map<string, string>(),
	profiles: { [KB]: profilesKeyboard, [MOUSE]: profilesMouse, [DA]: profilesDeathAdder } as Record<string, unknown>,
	dial: structuredClone(dialState) as DialState,
	oled: structuredClone(oledState) as OledState
};

// The maintainer's board has Fn+P remapped to Print Screen (docs/PROTOCOL.md); show that, with its factory
// value as the recorded original, so Restore has something to restore.
{
	const p = state.keymap[KB].hypershift!.find((k) => k.name === 'P');
	if (p) {
		state.original.set(`${KB}/hypershift/${p.key}`, p.function);
		p.function = 'key PRINT_SCREEN';
		p.description = 'Print screen';
	}
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** A failed call, in the shape the Tauri `daemon` command rejects with. */
const fail = (message: string, code: ErrorCode | null = null): DaemonFailure => ({ message, code, unreachable: false });

function resolve(query: string | null): string {
	if (!query) throw fail('this command needs a device');
	const q = query.toLowerCase();
	const hit = state.devices.find((d) => d.id === q || d.kind === q || d.name.toLowerCase().includes(q));
	if (!hit) throw fail(`no connected device matches \`${query}\``);
	return hit.id;
}

function needs(dev: string, feature: string) {
	const d = state.devices.find((x) => x.id === dev);
	if (!d?.features.includes(feature as never)) {
		throw fail(`${d?.name ?? dev} does not support ${feature}`, 'not_supported');
	}
}

function needWrite(args: Record<string, unknown>, dev: string) {
	const name = state.devices.find((d) => d.id === dev)?.name ?? dev;
	if (args.write !== true) throw fail(`this writes ${name}'s onboard memory; repeat with write=true (CLI: --write)`);
}

/** `?fail=keymap` (or dpi, poll_rate, power) in the dev URL makes that check fail, to see the failed state. */
const failing = () => (typeof location !== 'undefined' ? new URLSearchParams(location.search).get('fail') : null);

const DETAIL: Partial<Record<Feature, string>> = {
	keymap: 'Button 1 read back as button 3 (expected left click).',
	dpi: 'The DPI read back as 0, outside 100 to 30000.',
	poll_rate: 'The polling rate reply had an unknown code.',
	power: 'The sleep timer read back as 0 seconds.'
};

/** One read-only check, as `check.run` and a first write run it. */
function runCheck(c: FeatureCheck): FeatureCheck {
	if (c.state === 'not_needed') return c;
	if (c.feature === failing()) return { ...c, state: 'failed', detail: DETAIL[c.feature] ?? 'The reply was not what uncoil expected.' };
	return { ...c, state: 'passed', detail: null };
}

/** Like the daemon: an untested check runs on the first write that needs it; a failed one refuses the write. */
function gate(dev: string, feature: Feature) {
	const caps = state.caps[dev];
	const i = caps?.checks?.findIndex((x) => x.feature === feature) ?? -1;
	if (i < 0) return;
	if (caps.checks[i].state === 'untested') caps.checks[i] = runCheck(caps.checks[i]);
	const c = caps.checks[i];
	if (c.state === 'failed') {
		const name = state.devices.find((d) => d.id === dev)?.name ?? dev;
		throw fail(`uncoil couldn't confirm ${name} answers the way it expects, so it won't change its ${feature} settings (${c.detail}).`, 'check_failed');
	}
}

const write = <T>(before: T, after: T) => ({ before, after, verified: true, unchanged: JSON.stringify(before) === JSON.stringify(after) });

function findKey(dev: string, layer: Layer, key: string): KeyMapping {
	const rows = state.keymap[dev][layer];
	if (!rows) throw fail(`this device has no ${layer} layer`);
	const k = key.startsWith('#') ? rows.find((r) => r.key === Number(key.slice(1))) : rows.find((r) => r.name === key.toUpperCase());
	if (!k) throw fail(`no key \`${key}\``);
	return k;
}

export async function mockDaemon(cmd: string, device: string | null, args: Record<string, unknown> = {}): Promise<unknown> {
	await sleep(120 + Math.random() * 120);
	const layer = ((args.layer as string) === 'fn' ? 'hypershift' : (args.layer as Layer)) ?? 'normal';
	switch (cmd) {
		case 'devices':
			return structuredClone(state.devices);
		case 'capabilities':
			return structuredClone(state.caps[resolve(device)]);
		case 'keymap.dump': {
			const dev = resolve(device);
			needs(dev, 'keymap');
			if (!state.keymap[dev][layer]) throw fail(`this device has no ${layer} layer`);
			return structuredClone(state.keymap[dev][layer]);
		}
		case 'keymap.get': {
			const dev = resolve(device);
			return structuredClone(findKey(dev, layer, args.key as string));
		}
		case 'keymap.set':
		case 'keymap.reset': {
			const dev = resolve(device);
			needs(dev, 'keymap');
			needWrite(args, dev);
			gate(dev, 'keymap');
			const k = findKey(dev, layer, args.key as string);
			const id = `${dev}/${layer}/${k.key}`;
			const before = structuredClone(k);
			const target = cmd === 'keymap.set' ? (args.function as string) : (state.original.get(id) ?? k.function);
			// Left-click guard (every mouse): never leave the normal layer without a button that left-clicks.
			const isMouse = state.devices.find((d) => d.id === dev)?.kind === 'mouse';
			if (isMouse && layer === 'normal' && target !== 'button 1' && !state.keymap[dev].normal!.some((r) => r.key !== k.key && r.function === 'button 1')) {
				throw fail('This would leave no button that left-clicks. Map another button to left click first.', 'left_click_guard');
			}
			if (!state.original.has(id)) state.original.set(id, k.function);
			if (cmd === 'keymap.reset') state.original.delete(id);
			if (target === k.function) return { before, after: before, verified: true, unchanged: true };
			k.function = target;
			k.description = describeFunction(target, dev);
			return { before, after: structuredClone(k), verified: true, unchanged: false };
		}
		case 'profile.list':
			return structuredClone(state.profiles[resolve(device)]);
		case 'dial.get':
			needs(resolve(device), 'dial');
			return structuredClone(state.dial);
		case 'dial.set': {
			const dev = resolve(device);
			needWrite(args, dev);
			const before = structuredClone(state.dial);
			if (before.mode === args.mode) return { before, after: before, verified: true, unchanged: true };
			const modes = state.caps[KB].dial_modes;
			state.dial = { ...state.dial, mode: args.mode as string, mode_id: modes.indexOf(args.mode as string) };
			return { before, after: structuredClone(state.dial), verified: true, unchanged: false };
		}
		case 'oled.get':
			needs(resolve(device), 'oled');
			return structuredClone(state.oled);
		case 'oled.set': {
			const dev = resolve(device);
			needWrite(args, dev);
			const before = state.oled.brightness;
			const after = Math.min(100, Number(args.brightness));
			state.oled.brightness = after;
			return { before, after, verified: true, unchanged: before === after };
		}
		case 'effect.hw': {
			const dev = resolve(device);
			const d = state.devices.find((x) => x.id === dev)!;
			d.hw_effect = args.effect as string;
			return { effect: d.hw_effect, storage: (args.storage as string) ?? 'session' };
		}
		case 'effect.software': {
			const d = state.devices.find((x) => x.id === resolve(device))!;
			d.hw_effect = null;
			return { effect: null, storage: 'session' };
		}
		case 'check.run': {
			const dev = resolve(device);
			const caps = state.caps[dev];
			caps.checks = caps.checks.map(runCheck);
			await sleep(500);
			return structuredClone(caps.checks);
		}
		case 'performance.get': {
			const dev = resolve(device);
			if (!state.performance[dev]) throw fail(`${dev} does not support dpi or poll_rate`, 'not_supported');
			return structuredClone(state.performance[dev]);
		}
		case 'performance.set': {
			const dev = resolve(device);
			const p = state.performance[dev];
			if (!p) throw fail(`${dev} does not support dpi or poll_rate`, 'not_supported');
			const inRange = (d: Dpi) => d.x >= p.dpi_min! && d.x <= p.dpi_max! && d.y >= p.dpi_min! && d.y <= p.dpi_max!;
			if (args.stages === undefined && args.poll_hz === undefined) {
				// Live DPI: not stored, like pressing the DPI button.
				// It still waits for the DPI check, like the daemon's.
				gate(dev, 'dpi');
				const dpi = args.dpi as Dpi;
				if (!dpi || !inRange(dpi)) throw fail(`DPI must be between ${p.dpi_min} and ${p.dpi_max}`);
				p.dpi = { x: Math.round(dpi.x), y: Math.round(dpi.y) };
				return structuredClone(p);
			}
			needWrite(args, dev);
			const before = structuredClone(p);
			if (args.stages !== undefined) {
				gate(dev, 'dpi');
				const st = args.stages as { active: number; list: Dpi[] };
				if (!st.list.length || st.list.length > p.stages_max) throw fail(`Between 1 and ${p.stages_max} DPI stages, please.`);
				if (!st.list.every(inRange)) throw fail(`Every stage must be between ${p.dpi_min} and ${p.dpi_max} DPI.`);
				const active = Math.min(Math.max(1, st.active), st.list.length);
				p.stages = { active, list: st.list.map((d) => ({ x: Math.round(d.x), y: Math.round(d.y) })) };
				p.dpi = { ...p.stages.list[active - 1] };
			}
			if (args.poll_hz !== undefined) {
				gate(dev, 'poll_rate');
				if (!p.poll_rates.includes(args.poll_hz as number)) throw fail(`The polling rate must be one of ${p.poll_rates.join(', ')} Hz.`);
				p.poll_hz = args.poll_hz as number;
			}
			return write(before, structuredClone(p));
		}
		case 'power.get': {
			const dev = resolve(device);
			needs(dev, 'power');
			return structuredClone(state.power[dev]);
		}
		case 'power.set': {
			const dev = resolve(device);
			needs(dev, 'power');
			needWrite(args, dev);
			gate(dev, 'power');
			const p = state.power[dev];
			const before = structuredClone(p);
			if (args.idle_s !== undefined) {
				const [lo, hi] = p.idle_range!;
				const v = Number(args.idle_s);
				if (v < lo || v > hi) throw fail(`The sleep timer must be between ${lo} and ${hi} seconds.`);
				p.idle_s = v;
			}
			if (args.low_battery_pct !== undefined) {
				const [lo, hi] = p.low_battery_range!;
				const v = Number(args.low_battery_pct);
				if (v < lo || v > hi) throw fail(`The low battery warning must be between ${lo} and ${hi} %.`);
				// Stored as a fraction of 255 on the device, so it reads back rounded.
				p.low_battery_pct = Math.round((Math.round((v / 100) * 255) / 255) * 100);
			}
			return write(before, structuredClone(p));
		}
		default:
			throw fail(`mock daemon: unknown command ${cmd}`);
	}
}
