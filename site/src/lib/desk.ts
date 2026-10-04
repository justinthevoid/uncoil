// The maintainer's real desk (the app's snapshot, apps/uncoil/src/lib/mock/desk.json) and the geometry helpers
// the build-time renders share. Build time only; the browser gets positions from data attributes.
import desk from '$uncoil/mock/desk.json';
import type { DeskDevice } from '$uncoil/types';

export const devices = desk as unknown as DeskDevice[];

export const ledCount = devices.reduce((n, d) => n + d.shapes.length, 0);

/** Every device's outline, plus a little air around it (as the app's desk preview frames it). */
export const deskBounds = (() => {
	const xs = devices.flatMap((d) => [d.x, d.x + d.w]);
	const ys = devices.flatMap((d) => [d.y, d.y + d.h]);
	const x0 = Math.min(...xs) - 0.4;
	const y0 = Math.min(...ys) - 0.4;
	return { x0, y0, w: Math.max(...xs) + 0.4 - x0, h: Math.max(...ys) + 0.4 - y0 };
})();

const pct = (n: number) => `${+(n * 100).toFixed(3)}%`;

/** CSS box (left/top/width/height in %) of a desk-unit rectangle inside the desk bounds. */
export function deskBox(r: { x: number; y: number; w: number; h: number }) {
	const b = deskBounds;
	return `left:${pct((r.x - b.x0) / b.w)};top:${pct((r.y - b.y0) / b.h)};width:${pct(r.w / b.w)};height:${pct(r.h / b.h)}`;
}

/** Keyboard case padding around the key field, in key units (as the app draws it). */
export const CASE_PAD = 0.35;

/** CSS box of a key or LED (centre x/y, size w/h) inside a keyboard's case. */
export function capBox(kb: DeskDevice, x: number, y: number, w: number, h: number) {
	const W = kb.w + CASE_PAD * 2;
	const H = kb.h + CASE_PAD * 2;
	return `left:${pct((x - w / 2 - kb.x + CASE_PAD) / W)};top:${pct((y - h / 2 - kb.y + CASE_PAD) / H)};width:${pct(w / W)};height:${pct(h / H)}`;
}

/** Keycap legends for the layout's key names (the app's Keyboard.svelte uses the same words). */
const LEGEND: Record<string, string> = {
	Escape: 'Esc',
	Delete: 'Del',
	Insert: 'Ins',
	'Page Up': 'PgUp',
	'Page Down': 'PgDn',
	'Caps Lock': 'Caps',
	'Left Shift': 'Shift',
	'Right Shift': 'Shift',
	'Left Control': 'Ctrl',
	'Right Control': 'Ctrl',
	'Left Windows': 'Win',
	'Left Alt': 'Alt',
	'Right Alt': 'Alt',
	'Right Fn': 'Fn',
	'Up Arrow': '↑︎',
	'Down Arrow': '↓︎',
	'Left Arrow': '←︎',
	'Right Arrow': '→︎',
	Space: '',
};
export const legendFor = (name: string) => LEGEND[name] ?? name;
