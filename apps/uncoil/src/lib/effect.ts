// Frontend port of uncoil_core::{color::rainbow, effect::Frame}. Kept line-for-line with the Rust so the
// pulse plot and the browser mock draw exactly what the engine sends; the src-tauri tests guard the mock.
import type { Effect, Rgb } from './types';

const remEuclid = (a: number, b: number) => ((a % b) + b) % b;
const toU8 = (x: number) => Math.trunc(Math.min(255, Math.max(0, x)));
const clamp01 = (x: number) => Math.min(1, Math.max(0, x));

/** FastLED's hsv2rgb_rainbow hue map. */
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

export const scale = ([r, g, b]: Rgb, v: number): Rgb => {
	const k = clamp01(v);
	return [toU8(r * k + 0.5), toU8(g * k + 0.5), toU8(b * k + 0.5)];
};

export const hex = ([r, g, b]: Rgb) => '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join('');

/** Port of `Effect::at(t, sat, val)` + `Frame::color_at(x, y)`. */
export function frame(effect: Effect, t: number, sat: number, val: number): (x: number, y: number) => Rgb {
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

/**
 * The wave's phase at a desk point, 0..1 (where in its rainbow cycle that point is). This is what the
 * pulse plot draws as ridges: angle, band width and speed are all visible in it.
 */
export function phaseAt(effect: Effect, t: number): (x: number, y: number) => number {
	switch (effect.kind) {
		case 'wave': {
			const a = (effect.angle_deg * Math.PI) / 180;
			const ux = Math.cos(a);
			const uy = Math.sin(a);
			const invWl = 1 / Math.max(effect.wavelength, 1);
			const phase = ((effect.reverse ? -1 : 1) * t) / Math.max(effect.period_s, 0.5);
			return (x, y) => remEuclid((x * ux + y * uy) * invWl - phase, 1);
		}
		case 'spectrum': {
			const p = remEuclid(t / Math.max(effect.period_s, 0.5), 1);
			return () => p;
		}
		default:
			return () => 0;
	}
}
