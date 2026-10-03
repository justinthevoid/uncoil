// Browser-only stand-in for the Tauri backend, so `pnpm dev` works in a plain browser for design work.
// Only loaded (as its own chunk) when the page is not running inside Tauri; see ../api.ts.
//
// desk.json is a snapshot of the real `get_desk` output for the default desk, kept honest by the
// `mock_desk_matches_default_desk` test in src-tauri (regenerate: UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-gui).
// The colour maths is the shared port in ../effect.ts (uncoil_core::{color::rainbow, effect::Frame}).
import desk from './desk.json';
import type { Config, DeskDevice, Status } from '../types';
import { frame, hex } from '../effect';

const defaultConfig = (): Config => ({
	effect: { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false },
	brightness: 1,
	saturation: 1,
	fps: 30,
	display: { off_when_display_off: true, dim_level: 0.35, fade_s: 1.2 },
	desk: {},
	openrgb_hardware_rainbow: true
});

let stored: Config | null = null;

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
			{ id: 'razer-goliathus-chroma-extended', name: 'Razer Goliathus Chroma Extended', product_id: 0x0c02, connection: 'wired', fps: 30.0, busy_retries: 0, errors: 0 }
		]
	};
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
			return desk as unknown as T;
		case 'preview_frame': {
			const c = config!;
			const at = frame(c.effect, args.t as number, c.saturation, c.brightness);
			return (desk as DeskDevice[]).map((d) => d.shapes.map((s) => hex(at(s.x, s.y)))) as T;
		}
		case 'get_status':
			return status() as T;
		case 'daemon': {
			const { mockDaemon } = await import('./daemon');
			return (await mockDaemon(args.cmd as string, args.device as string | null, (args.args as Record<string, unknown>) ?? {})) as T;
		}
		default:
			throw new Error(`mock: unknown command ${cmd}`);
	}
}
