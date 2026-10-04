// Frontend port of uncoil_core::{color::rainbow, effect} (crates/uncoil-core/src/effect.rs). Kept line-for-line
// with the Rust so the browser mock draws what the engine sends; scripts/check-mirror.mjs (part of `pnpm check`)
// compares it with the engine's own colours in mock/fixtures.json. The integer hashes are bit-exact; float maths
// runs in f64 here (f32 in Rust), so a colour can differ by a step at most.
import type { DeskDevice, Effect, LayerEffect, Mask, PreviewPress, Rgb } from './types';

const remEuclid = (a: number, b: number) => ((a % b) + b) % b;
const toU8 = (x: number) => Math.trunc(Math.min(255, Math.max(0, x)));
const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));
const BLACK: Rgb = [0, 0, 0];

/** FastLED's hsv2rgb_rainbow hue map. */
function rainbow(h: number, s: number, v: number): Rgb {
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

export const hex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');

// ---- inputs ----

/** The desk's extent in key units (`maxY` is the front edge, toward the user). */
export interface Bounds {
	minX: number;
	minY: number;
	maxX: number;
	maxY: number;
}

/** `Bounds::DEFAULT`: the default desk (keyboard, mouse, extended mat). */
const DEFAULT_BOUNDS: Bounds = { minX: -2, minY: -1.5, maxX: 24.25, maxY: 8.25 };
const DEFAULT_WHEEL_CENTER: [number, number] = [8.1, 3.1];
const REACTIVE_RADIUS = 0.6;

/** Port of `effect::Inputs`. */
export interface Inputs {
	presses?: PreviewPress[];
	/** Audio peak level 0..1. */
	audio?: number;
	bounds?: Bounds | null;
	keyboardCenter?: [number, number] | null;
}

/** `layout::desk_bounds`: union of every device body. */
function deskBounds(desk: DeskDevice[]): Bounds | null {
	if (desk.length === 0) return null;
	return desk.reduce<Bounds>(
		(a, d) => ({
			minX: Math.min(a.minX, d.x),
			minY: Math.min(a.minY, d.y),
			maxX: Math.max(a.maxX, d.x + d.w),
			maxY: Math.max(a.maxY, d.y + d.h)
		}),
		{ minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity }
	);
}

/** Centre of the first keyboard's body (what a wheel with `center: null` turns around). */
function keyboardCenter(desk: DeskDevice[]): [number, number] | null {
	const kb = desk.find((d) => d.kind === 'keyboard');
	return kb ? [kb.x + kb.w / 2, kb.y + kb.h / 2] : null;
}

/** Inputs for a whole desk: bounds and keyboard centre from the layout, plus simulated presses / audio. */
export const deskInputs = (desk: DeskDevice[], presses: PreviewPress[] = [], audio = 0): Inputs => ({
	presses,
	audio,
	bounds: deskBounds(desk),
	keyboardCenter: keyboardCenter(desk)
});

// ---- deterministic noise (bit-exact with the Rust) ----

/** lowbias32 finaliser. */
function mix(h: number): number {
	h >>>= 0;
	h ^= h >>> 16;
	h = Math.imul(h, 0x7feb352d);
	h ^= h >>> 15;
	h = Math.imul(h, 0x846ca68b);
	return (h ^ (h >>> 16)) >>> 0;
}

function hash3(a: number, b: number, c: number): number {
	return mix(Math.imul(a, 0x9e3779b1) ^ mix(Math.imul(b, 0x85ebca77) ^ mix(c)));
}

const unit = (h: number) => (h >>> 8) / 16777216;
const quant = (v: number) => Math.floor(v * 64 + 0.5) | 0;

const f32buf = new Float32Array(1);
const u32buf = new Uint32Array(f32buf.buffer);
/** `f32::to_bits`. */
function f32Bits(x: number): number {
	f32buf[0] = x;
	return u32buf[0];
}

const pressHue = (t: number) => unit(mix(f32Bits(t)));

function vnoise(x: number, y: number, z: number): number {
	const xf = Math.floor(x), yf = Math.floor(y), zf = Math.floor(z);
	const i = xf | 0, j = yf | 0, k = zf | 0;
	const s = (f: number) => f * f * (3 - 2 * f);
	const u = s(x - xf), v = s(y - yf), w = s(z - zf);
	const c = (di: number, dj: number, dk: number) => unit(hash3(i + di, j + dj, k + dk));
	const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
	const x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
	const x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
	const x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
	const x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
	return lerp(lerp(x00, x10, v), lerp(x01, x11, v), w);
}

const FIRE: [number, Rgb][] = [
	[0, [0, 0, 0]],
	[0.3, [110, 0, 0]],
	[0.55, [210, 40, 0]],
	[0.8, [255, 130, 0]],
	[1, [255, 210, 60]]
];

function firePalette(heat: number): Rgb {
	let i = 1;
	while (i < FIRE.length - 1 && heat > FIRE[i][0]) i++;
	const [t0, a] = FIRE[i - 1];
	const [t1, b] = FIRE[i];
	const f = clamp01((heat - t0) / (t1 - t0));
	const ch = (k: number) => toU8(a[k] + (b[k] - a[k]) * f + 0.5);
	return [ch(0), ch(1), ch(2)];
}

function meterColor(u: number): Rgb {
	return u < 0.6 ? [toU8((255 * u) / 0.6 + 0.5), 255, 0] : [255, toU8(255 * (1 - (u - 0.6) / 0.4) + 0.5), 0];
}

// ---- effects ----

/** Colour + alpha at a desk point. */
type Sampler = (x: number, y: number) => [Rgb, number];

interface Spot {
	x: number;
	y: number;
	r: number;
	strength: number;
	color: Rgb;
}

function spots(presses: PreviewPress[], t: number, fadeS: number, speed: number, color: Rgb | null, sat: number, val: number): Spot[] {
	const fade = Math.max(fadeS, 0.05);
	const out: Spot[] = [];
	for (const p of presses) {
		const age = t - p.t;
		if (!(age >= 0 && age < fade)) continue;
		const c = color ? scale(color, val) : rainbow(pressHue(p.t), sat, val);
		out.push({ x: p.x, y: p.y, r: age * speed, strength: 1 - age / fade, color: c });
	}
	return out;
}

function strongest(list: Spot[], f: (s: Spot) => number): [Rgb, number] {
	let best: [Rgb, number] = [BLACK, 0];
	for (const s of list) {
		const a = f(s);
		if (a > best[1]) best = [s.color, a];
	}
	return best;
}

const solid = (c: Rgb): Sampler => () => [c, 1];

/** Port of `prepare` + `Kind::eval` for one (non-studio) effect. */
function prepare(e: LayerEffect | Effect, t: number, sat: number, val: number, inp: Inputs): Sampler {
	const b = inp.bounds ?? DEFAULT_BOUNDS;
	switch (e.kind) {
		case 'wave': {
			const a = (e.angle_deg * Math.PI) / 180;
			const ux = Math.cos(a), uy = Math.sin(a);
			const invWl = 1 / Math.max(e.wavelength, 1);
			const phase = ((e.reverse ? -1 : 1) * t) / Math.max(e.period_s, 0.5);
			return (x, y) => [rainbow((x * ux + y * uy) * invWl - phase, sat, val), 1];
		}
		case 'spectrum':
			return solid(rainbow(t / Math.max(e.period_s, 0.5), sat, val));
		case 'static':
			return solid(scale(e.color, val));
		case 'breathing': {
			const ph = t / Math.max(e.period_s, 0.5);
			const n = Math.floor(ph);
			const br = 0.5 - 0.5 * Math.cos(2 * Math.PI * (ph - n));
			const v = val * br * br;
			const len = e.colors.length;
			return solid(len === 0 ? rainbow(n / 6, sat, v) : scale(e.colors[remEuclid(n, len)], v));
		}
		case 'starlight': {
			const density = clamp01(e.density);
			const tw = Math.max(e.twinkle_s, 0.1);
			const colors = e.colors;
			return (x, y) => {
				const qx = quant(x), qy = quant(y);
				const tt = t / tw + unit(hash3(qx, qy, 0x5eed));
				const bucket = Math.floor(tt);
				const h = hash3(qx, qy, bucket | 0);
				if (unit(h) >= density) return [BLACK, 0];
				const pick = mix(h ^ 0x9e3779b9);
				const c = colors.length === 0 ? rainbow(unit(pick), sat, val) : scale(colors[pick % colors.length], val);
				return [c, Math.max(0, Math.sin(Math.PI * (tt - bucket)))];
			};
		}
		case 'fire': {
			const depth = Math.max(b.maxY - b.minY, 1);
			const invH = 1 / Math.max(clamp(e.height, 0.05, 1) * depth, 0.5);
			const tt = t * clamp(e.speed, 0.25, 3);
			const front = b.maxY;
			return (x, y) => {
				const d = Math.max((front - y) * invH, 0);
				const rise = y + tt * 2.2;
				const n = 0.65 * vnoise(x * 0.6, rise * 0.6, tt * 0.5) + 0.35 * vnoise(x * 1.3, rise * 1.3, tt * 0.9 + 17);
				const heat = clamp01(1 - d + (n - 0.5) * 1.2);
				return [scale(firePalette(heat), val), 1];
			};
		}
		case 'wheel': {
			const [cx, cy] = e.center ?? inp.keyboardCenter ?? DEFAULT_WHEEL_CENTER;
			const phase = ((e.reverse ? -1 : 1) * t) / Math.max(e.period_s, 0.5);
			return (x, y) => [rainbow(Math.atan2(y - cy, x - cx) / (2 * Math.PI) - phase, sat, val), 1];
		}
		case 'reactive': {
			const list = spots(inp.presses ?? [], t, e.fade_s, 0, e.color, sat, val);
			const r2 = REACTIVE_RADIUS * REACTIVE_RADIUS;
			return (x, y) =>
				strongest(list, (s) => {
					const d2 = (x - s.x) ** 2 + (y - s.y) ** 2;
					return d2 >= r2 ? 0 : s.strength * (1 - d2 / r2);
				});
		}
		case 'ripple': {
			const list = spots(inp.presses ?? [], t, e.fade_s, Math.max(e.speed, 0), e.color, sat, val);
			const half = Math.max(e.width * 0.5, 0.1);
			return (x, y) =>
				strongest(list, (s) => {
					const ring = 1 - Math.abs(Math.hypot(x - s.x, y - s.y) - s.r) / half;
					return ring <= 0 ? 0 : s.strength * ring;
				});
		}
		case 'audio_meter': {
			const level = clamp01((inp.audio ?? 0) * Math.max(e.sensitivity, 0));
			const w = Math.max(b.maxX - b.minX, 1);
			const fill = level > 0 ? b.minX + level * w : -Infinity;
			return (x) => (x > fill ? [BLACK, 0] : [scale(meterColor(clamp01((x - b.minX) / w)), val), 1]);
		}
		default:
			// off, and a studio nested inside a studio
			return solid(BLACK);
	}
}

function covers(m: Mask, device: string, shape: string): boolean {
	switch (m.kind) {
		case 'all':
			return true;
		case 'devices':
			return m.ids.includes(device);
		case 'keys':
			return m.device === device && m.shapes.includes(shape);
	}
}

/** Colour of one LED: device id, desk layout shape name, desk position. */
export type LedSampler = (device: string, shape: string, x: number, y: number) => Rgb;

/** Port of `Effect::at_with(t, sat, val, inputs)` + `Frame::color_led`, studio masks and blending included. */
export function frameWith(effect: Effect, t: number, sat: number, val: number, inputs: Inputs = {}): LedSampler {
	if (effect.kind !== 'studio') {
		const f = prepare(effect, t, sat, val, inputs);
		return (_d, _s, x, y) => {
			const [c, a] = f(x, y);
			return a >= 1 ? c : scale(c, a);
		};
	}
	const layers = effect.layers
		.filter((l) => l.enabled && l.opacity > 0 && (l.effect as Effect).kind !== 'studio')
		.map((l) => ({ f: prepare(l.effect, t, sat, val, inputs), opacity: Math.min(l.opacity, 1), mask: l.mask }));
	return (device, shape, x, y) => {
		let r = 0, g = 0, b = 0;
		for (const l of layers) {
			if (!covers(l.mask, device, shape)) continue;
			const [c, a0] = l.f(x, y);
			const a = Math.min(a0 * l.opacity, 1);
			if (a <= 0) continue;
			r += (c[0] - r) * a;
			g += (c[1] - g) * a;
			b += (c[2] - b) * a;
		}
		return [toU8(r + 0.5), toU8(g + 0.5), toU8(b + 0.5)];
	};
}
