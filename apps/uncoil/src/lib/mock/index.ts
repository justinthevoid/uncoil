// Browser-only stand-in for the Tauri backend, so `pnpm dev` works in a plain browser for design work.
// Only loaded (as its own chunk) when the page is not running inside Tauri; see ../api.ts.
//
// desk.json is a snapshot of the real `get_desk` output for the default desk, kept honest by the
// `mock_desk_matches_default_desk` test in src-tauri (regenerate: UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-gui).
// The colour maths below is a line-for-line port of uncoil_core::{color::rainbow, effect::Frame::color_at}.
import desk from './desk.json';
import type { Config, DeskDevice, Effect, Rgb, Status } from '../types';

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

/** Rust `f32::rem_euclid`. */
const remEuclid = (a: number, b: number) => ((a % b) + b) % b;
/** Rust `(x + 0.5).clamp(0, 255) as u8` (float-to-int casts truncate toward zero). */
const toU8 = (x: number) => Math.trunc(Math.min(255, Math.max(0, x)));
const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

/** FastLED's hsv2rgb_rainbow hue map; port of `uncoil_core::color::rainbow`. */
export function rainbow(h: number, s: number, v: number): Rgb {
	const h8 = remEuclid(h, 1) * 256;
	const section = Math.trunc(h8 / 32) & 7;
	const off = (h8 % 32) * 8;
	const third = off / 3;
	const two = (off * 2) / 3;
	let r: number, g: number, b: number;
	switch (section) {
		case 0: [r, g, b] = [255 - third, third, 0]; break;
		case 1: [r, g, b] = [171, 85 + third, 0]; break;
		case 2: [r, g, b] = [171 - two, 170 + third, 0]; break;
		case 3: [r, g, b] = [0, 255 - third, third]; break;
		case 4: [r, g, b] = [0, 171 - two, 85 + two]; break;
		case 5: [r, g, b] = [third, 0, 255 - third]; break;
		case 6: [r, g, b] = [85 + third, 0, 171 - third]; break;
		default: [r, g, b] = [170 + third, 0, 85 - third];
	}
	const sc = clamp01(s);
	const vc = clamp01(v);
	const ch = (c: number) => toU8((c * sc + 255 * (1 - sc)) * vc + 0.5);
	return [ch(r), ch(g), ch(b)];
}

const scale = ([r, g, b]: Rgb, v: number): Rgb => {
	const k = clamp01(v);
	return [toU8(r * k + 0.5), toU8(g * k + 0.5), toU8(b * k + 0.5)];
};

const hex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');

/** Port of `Effect::at(t, sat, val)` + `Frame::color_at(x, y)`. */
function frame(effect: Effect, t: number, sat: number, val: number): (x: number, y: number) => Rgb {
	switch (effect.kind) {
		case 'wave': {
			const a = (effect.angle_deg * Math.PI) / 180;
			const ux = Math.cos(a);
			const uy = Math.sin(a);
			const invWl = 1 / Math.max(effect.wavelength, 1);
			const phase = ((effect.reverse ? -1 : 1) * t) / Math.max(effect.period_s, 0.5);
			return (x, y) => rainbow((x * ux + y * uy) * invWl - phase, sat, val);
		}
		case 'spectrum': {
			const c = rainbow(t / Math.max(effect.period_s, 0.5), sat, val);
			return () => c;
		}
		case 'static': {
			const c = scale(effect.color, val);
			return () => c;
		}
		default:
			return () => [0, 0, 0];
	}
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
		default:
			throw new Error(`mock: unknown command ${cmd}`);
	}
}
