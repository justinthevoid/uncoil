// The effect catalogue for the UI: names, one-line notes, defaults, what input an effect needs, and the
// swatch each effect card shows. The effects themselves live in the engine (uncoil-core effect.rs).
import { hex as toHex } from './effect.ts';
import type { LayerEffect, Rgb } from './types';

export { toHex };

export type LayerKind = LayerEffect['kind'];

export interface EffectInfo {
	kind: LayerKind;
	label: string;
	note: string;
	/** What the effect reacts to, if anything. */
	needs?: 'keys' | 'audio';
	make: () => LayerEffect;
}

export const EFFECTS: EffectInfo[] = [
	{ kind: 'wave', label: 'Wave', note: 'A rainbow moving across the whole desk', make: () => ({ kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false }) },
	{ kind: 'spectrum', label: 'Spectrum', note: 'Every light fades through the rainbow together', make: () => ({ kind: 'spectrum', period_s: 14 }) },
	{ kind: 'breathing', label: 'Breathing', note: 'Fades in and out, in one or two colours', make: () => ({ kind: 'breathing', colors: [[224, 163, 62]], period_s: 6 }) },
	{ kind: 'static', label: 'Static', note: 'One colour everywhere', make: () => ({ kind: 'static', color: [224, 163, 62] }) },
	{ kind: 'starlight', label: 'Starlight', note: 'Random lights twinkle on and off', make: () => ({ kind: 'starlight', colors: [], density: 0.15, twinkle_s: 1.6 }) },
	{ kind: 'fire', label: 'Fire', note: 'Flames rising from the front of the desk', make: () => ({ kind: 'fire', speed: 1, height: 0.6 }) },
	{ kind: 'wheel', label: 'Wheel', note: 'A rainbow turning around the keyboard', make: () => ({ kind: 'wheel', period_s: 6, reverse: false, center: null }) },
	{ kind: 'reactive', label: 'Reactive', note: 'Keys light up when you press them', needs: 'keys', make: () => ({ kind: 'reactive', color: null, fade_s: 1 }) },
	{ kind: 'ripple', label: 'Ripple', note: 'Rings spread across the desk from each key you press', needs: 'keys', make: () => ({ kind: 'ripple', color: null, speed: 14, width: 2, fade_s: 1.5 }) },
	{ kind: 'audio_meter', label: 'Audio meter', note: 'The desk fills with what your PC is playing', needs: 'audio', make: () => ({ kind: 'audio_meter', sensitivity: 1 }) },
	{ kind: 'off', label: 'Off', note: 'Lights stay dark', make: () => ({ kind: 'off' }) }
];

export const effectInfo = (kind: string) => EFFECTS.find((e) => e.kind === kind);
/** The effects that react to key presses, or to what the PC is playing. */
export const kindsNeeding = (input: 'keys' | 'audio'): string[] => EFFECTS.filter((e) => e.needs === input).map((e) => e.kind);

/** Named colours, like a gel book's swatches. */
export const GELS: { name: string; hex: string }[] = [
	{ name: 'Warm white', hex: '#ffd9a8' },
	{ name: 'Cool white', hex: '#e6efff' },
	{ name: 'Straw', hex: '#f3d36b' },
	{ name: 'Amber', hex: '#e0a33e' },
	{ name: 'Primary red', hex: '#d7262e' },
	{ name: 'Rose pink', hex: '#e0559a' },
	{ name: 'Lavender', hex: '#9a7ce0' },
	{ name: 'Congo blue', hex: '#3a2fa0' },
	{ name: 'Steel blue', hex: '#4d7fb8' },
	{ name: 'Cyan', hex: '#26c6da' },
	{ name: 'Teal', hex: '#2a9d8f' },
	{ name: 'Moss green', hex: '#6aa84f' }
];

export const fromHex = (h: string): Rgb => {
	const n = parseInt(h.slice(1), 16);
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

const RAINBOW = '#ff0000, #ff8a00, #ffe600, #2bd94a, #00b3ff, #3a3aff, #b040ff, #ff0060';

/** The swatch an effect card shows: what the effect does, as a small CSS painting. */
export function swatchFor(e: LayerEffect): string {
	switch (e.kind) {
		case 'wave':
			return `linear-gradient(${90 + e.angle_deg}deg, ${RAINBOW})`;
		case 'spectrum':
			return 'linear-gradient(180deg, #ff0060 0 16%, #ff8a00 16% 33%, #ffe600 33% 50%, #2bd94a 50% 66%, #00b3ff 66% 83%, #b040ff 83%)';
		case 'static':
			return toHex(e.color);
		case 'breathing': {
			const a = e.colors[0] ? toHex(e.colors[0]) : '#ff4060';
			const b = e.colors[1] ? toHex(e.colors[1]) : e.colors[0] ? a : '#40a0ff';
			return `linear-gradient(90deg, #111 0%, ${a} 25%, #111 50%, ${b} 75%, #111 100%)`;
		}
		case 'starlight': {
			const dots = ['#fff4c0 22% 30%', '#7ad 64% 22%', '#f8a 40% 70%', '#ffe 82% 60%', '#adf 12% 76%', '#fd7 56% 46%'];
			const c = e.colors.length ? e.colors.map(toHex) : null;
			return (
				dots
					.map((d, i) => {
						const [col, x, y] = d.split(' ');
						return `radial-gradient(circle at ${x} ${y}, ${c ? c[i % c.length] : col} 0 3px, transparent 4px)`;
					})
					.join(', ') + ', #121212'
			);
		}
		case 'fire':
			return 'linear-gradient(0deg, #ffd34d 0%, #ff8a00 28%, #d7262e 55%, #3a0a06 85%, #120604 100%)';
		case 'wheel':
			return `conic-gradient(from 0deg, ${RAINBOW}, #ff0000)`;
		case 'reactive': {
			const c = e.color ? toHex(e.color) : '#7ad3ff';
			return `radial-gradient(circle at 62% 50%, ${c} 0 10px, transparent 11px), radial-gradient(circle at 30% 40%, ${c}88 0 7px, transparent 8px), #141414`;
		}
		case 'ripple': {
			const c = e.color ? toHex(e.color) : '#7ad3ff';
			return `repeating-radial-gradient(circle at 50% 55%, ${c} 0 2px, transparent 3px 14px), #141414`;
		}
		case 'audio_meter':
			return 'linear-gradient(90deg, #2bd94a 0%, #ffe600 45%, #ff3b2f 70%, #1a1a1a 70%)';
		case 'off':
			return 'var(--case)';
	}
}
