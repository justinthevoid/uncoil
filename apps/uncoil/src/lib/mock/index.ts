// Browser-only stand-in for the Tauri backend, so `pnpm dev` works in a plain browser for design work.
// Only loaded (as its own chunk) when the page is not running inside Tauri; see ../api.ts.
//
// desk.json is a snapshot of the real `get_desk` output for the default desk, kept honest by the
// `mock_desk_matches_default_desk` test in src-tauri (regenerate: UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-gui).
// The colour maths is the shared port in ../effect.ts (uncoil_core::{color::rainbow, effect}), studio masks,
// simulated key presses and audio level included.
import desk from './desk.json';
import type { AppSettings, Config, Conflict, DeskDevice, OpenRgbDevice, OpenRgbStatus, PreviewPress, Shape, Status } from '../types';
import { deskInputs, frameWith, hex } from '../effect';
import { defaultConfig } from './config';

let stored: Config | null = null;
let appSettings: AppSettings = { close_to_tray: false, start_in_tray: false, battery_notifications: true, battery_threshold: 20 };

const param = (name: string) => (typeof location !== 'undefined' ? new URLSearchParams(location.search).get(name) : null);

/** `?conflict=1` in the dev URL shows Synapse running, to see the notice. */
function conflicts(): Conflict[] {
	if (!param('conflict')) return [];
	return [
		{
			app: 'Razer Synapse',
			detail: 'Razer Synapse is running. Two programs driving the same devices fight over them; quit Synapse or turn off its start-up entry.'
		}
	];
}

/** Live OpenRGB follows the stored config: connected with a board, two sticks of RAM, a GPU and a fan hub
 * (four fans and an AIO pump) when the mode is live, or with `?openrgb=live` in the dev URL. `?icue=1` has
 * iCUE running, which holds the Corsair devices (as `uncoil_core::owners` decides: Corsair names and RAM). */
function openrgb(): OpenRgbStatus {
	const config = stored ?? defaultConfig();
	if (param('openrgb') !== 'live' && config.openrgb?.mode !== 'live') return { state: 'off', detail: null, devices: [], held: [], ours: false };
	// `openrgb.live.exclude`, as the daemon reads it: part of the name, any case
	const exclude = (config.openrgb?.live?.exclude ?? []).map((e) => e.trim().toLowerCase()).filter(Boolean);
	const devices: OpenRgbDevice[] = [
		{ id: 'openrgb:asus-rog-strix-b550-f-gaming', name: 'ASUS ROG STRIX B550-F GAMING', leds: 8, zones: [{ name: 'Aura Mainboard', kind: 'linear', leds: 8 }, { name: 'Addressable RGB Header 1', kind: 'linear', leds: 0 }] },
		{ id: 'openrgb:corsair-vengeance-pro-rgb', name: 'Corsair Vengeance Pro RGB', leds: 10, zones: [{ name: 'Corsair DRAM', kind: 'linear', leds: 10 }] },
		{ id: 'openrgb:corsair-vengeance-pro-rgb-2', name: 'Corsair Vengeance Pro RGB', leds: 10, zones: [{ name: 'Corsair DRAM', kind: 'linear', leds: 10 }] },
		{ id: 'openrgb:nvidia-geforce-rtx-3070', name: 'NVIDIA GeForce RTX 3070', leds: 4, zones: [{ name: 'GPU Zone', kind: 'linear', leds: 4 }] },
		{
			id: 'openrgb:corsair-icue-link-system-hub',
			name: 'Corsair iCUE Link System Hub',
			leds: 160,
			zones: [
				...[1, 2, 3, 4].map(() => ({ name: 'iCUE LINK QX RGB', kind: 'linear' as const, leds: 34 })),
				{ name: 'iCUE LINK COOLER PUMP LCD', kind: 'linear', leds: 24 }
			]
		}
	];
	const listed = devices.filter((d) => !exclude.some((e) => d.name.toLowerCase().includes(e)));
	const icue = (d: OpenRgbDevice) => !!param('icue') && d.name.toLowerCase().includes('corsair');
	return {
		state: 'connected',
		detail: null,
		devices: listed.filter((d) => !icue(d)),
		held: listed.filter(icue).map((d) => ({ name: d.name, by: 'Corsair iCUE' })),
		ours: true
	};
}

function status(): Status {
	const now = Math.floor(Date.now() / 1000);
	return {
		pid: 14872,
		version: '0.1.0',
		started_unix: now - 3 * 3600 - 17 * 60,
		updated_unix: now,
		display: 'on',
		level: 1,
		memory_bytes: 3_145_728 + Math.round(Math.random() * 40_000),
		cpu_percent: 0.6 + Math.random() * 0.4,
		exe_bytes: 666_624,
		devices: [
			{ id: 'razer-blackwidow-v4-pro-75', name: 'Razer BlackWidow V4 Pro 75%', product_id: 0x02b3, connection: 'wired', fps: 29.8, busy_retries: 12, errors: 0 },
			{ id: 'razer-basilisk-v3-pro', name: 'Razer Basilisk V3 Pro', product_id: 0x00aa, connection: 'wired', fps: 30.0, busy_retries: 3, errors: 1 },
			{ id: 'razer-goliathus-chroma-extended', name: 'Razer Goliathus Chroma Extended', product_id: 0x0c02, connection: 'wired', fps: 30.0, busy_retries: 0, errors: 0 },
			{ id: 'razer-deathadder-v3-pro', name: 'Razer DeathAdder V3 Pro', product_id: 0x00b6, connection: 'wired', fps: 0, busy_retries: 0, errors: 0 }
		],
		unknown_devices: [{ product_id: 0x0ffe, interfaces: [0, 1, 2] }],
		conflicts: conflicts(),
		openrgb: openrgb()
	};
}

/** The default desk moved as the config places it, plus OpenRGB's devices as the PC column left of the
 * keyboard (a rough port of uncoil_core::layout::{external_def, column_place}). */
function mockDesk(config: Config | undefined, external: OpenRgbDevice[] = []): DeskDevice[] {
	const placed = config?.desk ?? {};
	const shift = (d: DeskDevice, dx: number, dy: number): DeskDevice =>
		!dx && !dy ? d : { ...d, at: { x: d.at.x + dx, y: d.at.y + dy }, x: d.x + dx, y: d.y + dy, shapes: d.shapes.map((s) => ({ ...s, x: s.x + dx, y: s.y + dy })) };
	const out = (desk as unknown as DeskDevice[]).map((d) => {
		const p = placed[d.id];
		return p ? shift(d, p.x - d.at.x, p.y - d.at.y) : d;
	});
	const kb = out.find((d) => d.kind === 'keyboard');
	const base = [...out];
	let top = kb?.y ?? -0.3;
	for (const e of external.filter((e) => e.id.startsWith('openrgb:'))) {
		const zones = (e.zones?.length ? e.zones : [{ name: e.name, kind: 'linear' as const, leds: e.leds }]).filter((z) => z.leds > 0);
		const inner = Math.min(3, Math.max(0.6, ...zones.map((z) => z.leds * Math.min(0.3, 3 / z.leds))));
		const w = zones.length * 0.6 + 0.6, h = inner + 0.6;
		const x = Math.min(kb?.x ?? -0.3, ...base.map((d) => d.x)) - 1 - w, y = top;
		top += h + 0.5;
		const shapes: Shape[] = [];
		zones.forEach((z, c) => {
			const step = z.leds > 1 ? Math.min(0.3, 3 / z.leds) : 0.3;
			for (let i = 0; i < z.leds; i++) {
				const name = z.leds === 1 ? z.name : `${z.name} ${i + 1}`;
				shapes.push({ name: shapes.some((s) => s.name === name) ? `${name} (${c + 1})` : name, row: 0, col: shapes.length, x: x + 0.3 + c * 0.6 + 0.3, y: y + 0.3 + step * (i + 0.5), w: 0.3, h: 0.3, is_key: false });
			}
		});
		const d: DeskDevice = { id: e.id, name: e.name, kind: 'other', at: { x: x + w / 2, y: y + h / 2 }, x, y, w, h, shapes };
		const p = placed[e.id];
		out.push(p ? shift(d, p.x - d.at.x, p.y - d.at.y) : d);
	}
	return out;
}

export async function mockInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
	const config = args.config as Config | undefined;
	switch (cmd) {
		case 'get_config':
			return structuredClone(stored ?? defaultConfig()) as T;
		case 'save_config':
			stored = structuredClone(config!);
			return undefined as T;
		case 'get_desk':
			return mockDesk(config, args.external as OpenRgbDevice[] | undefined) as T;
		case 'preview_frame': {
			const c = config!;
			const devices = mockDesk(c, args.external as OpenRgbDevice[] | undefined);
			const inputs = deskInputs(devices, (args.presses as PreviewPress[] | undefined) ?? [], (args.audio as number | undefined) ?? 0);
			const at = frameWith(c.effect, args.t as number, c.saturation, c.brightness, inputs);
			return devices.map((d) => d.shapes.map((s) => hex(at(d.id, s.name, s.x, s.y)))) as T;
		}
		case 'get_status':
			return status() as T;
		case 'get_app_settings':
			return structuredClone(appSettings) as T;
		case 'save_app_settings':
			appSettings = structuredClone(args.settings as AppSettings);
			return undefined as T;
		case 'daemon': {
			const { mockDaemon } = await import('./daemon');
			return (await mockDaemon(args.cmd as string, args.device as string | null, (args.args as Record<string, unknown>) ?? {})) as T;
		}
		default:
			throw new Error(`mock: unknown command ${cmd}`);
	}
}
