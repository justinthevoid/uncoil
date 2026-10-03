// Browser-only stand-in for uncoild's control pipe, so the Keys and Hardware screens work in `pnpm dev`.
// Seeded from real responses of `uncoild --fake` (captured with `uncoil --json --pipe \\.\pipe\uncoil-fake …`
// into ./daemon/*.json), then kept in memory so writes, reads and resets behave like the daemon's.
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
import type { Capabilities, DeviceInfo, DialState, KeyMapping, Layer, OledState } from '../types';
import { describeFunction } from '../keys';

const KB = 'razer-blackwidow-v4-pro-75';
const MOUSE = 'razer-basilisk-v3-pro';

const state = {
	devices: structuredClone(devices) as DeviceInfo[],
	caps: { [KB]: capsKeyboard, [MOUSE]: capsMouse } as unknown as Record<string, Capabilities>,
	keymap: {
		[KB]: { normal: structuredClone(kbNormal), hypershift: structuredClone(kbFn) },
		[MOUSE]: { normal: structuredClone(mouseNormal), hypershift: structuredClone(mouseFn) }
	} as Record<string, Record<Layer, KeyMapping[]>>,
	/** What each key held before the first write (like the daemon's journal). */
	original: new Map<string, string>(),
	profiles: { [KB]: profilesKeyboard, [MOUSE]: profilesMouse } as Record<string, unknown>,
	dial: structuredClone(dialState) as DialState,
	oled: structuredClone(oledState) as OledState
};

// The maintainer's board has Fn+P remapped to Print Screen (docs/PROTOCOL.md); show that, with its factory
// value as the recorded original, so Restore has something to restore.
{
	const p = state.keymap[KB].hypershift.find((k) => k.name === 'P');
	if (p) {
		state.original.set(`${KB}/hypershift/${p.key}`, p.function);
		p.function = 'key PRINT_SCREEN';
		p.description = 'Print screen';
	}
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function resolve(query: string | null): string {
	if (!query) throw new Error('this command needs a device');
	const q = query.toLowerCase();
	const hit = state.devices.find((d) => d.id === q || d.kind === q || d.name.toLowerCase().includes(q));
	if (!hit) throw new Error(`no connected device matches \`${query}\``);
	return hit.id;
}

function needs(dev: string, feature: string) {
	if (!state.devices.find((d) => d.id === dev)?.features.includes(feature as never)) {
		throw new Error(`${dev} does not support ${feature}`);
	}
}

function needWrite(args: Record<string, unknown>, name: string) {
	if (args.write !== true) throw new Error(`this writes ${name}'s onboard memory; repeat with write=true (CLI: --write)`);
}

function findKey(dev: string, layer: Layer, key: string): KeyMapping {
	const rows = state.keymap[dev][layer];
	const k = key.startsWith('#') ? rows.find((r) => r.key === Number(key.slice(1))) : rows.find((r) => r.name === key.toUpperCase());
	if (!k) throw new Error(`no key \`${key}\``);
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
			const k = findKey(dev, layer, args.key as string);
			const id = `${dev}/${layer}/${k.key}`;
			const before = structuredClone(k);
			const target = cmd === 'keymap.set' ? (args.function as string) : (state.original.get(id) ?? k.function);
			if (!state.original.has(id)) state.original.set(id, k.function);
			if (cmd === 'keymap.reset') state.original.delete(id);
			if (target === k.function) return { before, after: before, verified: true, unchanged: true };
			k.function = target;
			k.description = describeFunction(target);
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
		default:
			throw new Error(`mock daemon: unknown command ${cmd}`);
	}
}
