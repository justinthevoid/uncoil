// Keyboards seen from above: what the drawing adds round the keys, measured from each keyboard's top-down
// product photo (key units, from the first key's top-left corner; 1u = one key pitch). The keys themselves,
// and the frames the key groups sit in, come from the device file's layout.
import type { Shape } from '#lib/types.ts';

export interface KeyboardArt {
	/** The case: left, top, right, bottom. */
	case: [number, number, number, number];
	/** Where the front lip starts (a seam across the case), if it has one. */
	lip?: number;
	/** OLED screen: x, y, w, h. */
	oled?: [number, number, number, number];
	/** A dial on the right side of the case: top, bottom (it stands proud of the case's right edge). */
	sideDial?: [number, number];
	/** Small buttons on the right side: top, bottom. */
	sideButtons?: [number, number][];
	/** Key groups that sit in their own frame even where they touch other keys. */
	frames?: string[][];
}

// Cases measured by tools/art/keyboard.py from each keyboard's store photo (./keyboards/<id>.json).
const generated = import.meta.glob<KeyboardArt>('./keyboards/*.json', { eager: true, import: 'default' });

/** Razer's photo of the BlackWidow V4 Pro 75% (full-keyboard-unlit.jpg on its product page), 1u = 100.8 px,
 * measured by hand for its screen and side dial; every other keyboard's case comes from keyboard.py. */
export const KEYBOARD_ART: Record<string, KeyboardArt> = {
	...Object.fromEntries(Object.entries(generated).map(([p, a]) => [p.replace(/^.*\/(.+)\.json$/, '$1'), { case: a.case, lip: a.lip }])),
	'razer-blackwidow-v4-pro-75': {
		case: [-0.42, -0.75, 16.77, 7.81],
		lip: 6.78,
		oled: [13.3, -0.07, 3.0, 1.02],
		sideDial: [-0.07, 0.95],
		sideButtons: [[1.4, 2.0]],
		frames: [['Delete', 'Page Up', 'Page Down', 'Insert']]
	}
};

/**
 * The frames the keys sit in: keys that touch (gaps under `gap` key units) form one group, drawn as the union
 * of their padded rectangles. Returns each group's rectangles (x, y, w, h) in the layout's units.
 */
export function keyFrames(keys: Shape[], own: string[][] = [], pad = 0.1, gap = 0.2): [number, number, number, number][][] {
	const setOf = (s: Shape) => own.findIndex((g) => g.includes(s.name));
	const rect = (s: Shape) => [s.x - s.w / 2, s.y - s.h / 2, s.x + s.w / 2, s.y + s.h / 2];
	const near = (a: Shape, b: Shape) => {
		const [ax0, ay0, ax1, ay1] = rect(a);
		const [bx0, by0, bx1, by1] = rect(b);
		const dx = Math.max(bx0 - ax1, ax0 - bx1, 0);
		const dy = Math.max(by0 - ay1, ay0 - by1, 0);
		return dx < gap && dy < gap && setOf(a) === setOf(b);
	};
	const group = keys.map((_, i) => i);
	const find = (i: number): number => (group[i] === i ? i : (group[i] = find(group[i])));
	for (let i = 0; i < keys.length; i++) for (let j = i + 1; j < keys.length; j++) if (near(keys[i], keys[j])) group[find(i)] = find(j);
	const out = new Map<number, [number, number, number, number][]>();
	keys.forEach((s, i) => {
		const [x0, y0, x1, y1] = rect(s);
		const g = find(i);
		out.set(g, [...(out.get(g) ?? []), [x0 - pad, y0 - pad, x1 - x0 + pad * 2, y1 - y0 + pad * 2]]);
	});
	return [...out.values()];
}

/** The board's drawn extent in desk units: the device box with a margin for the underglow, grown to take in
 * the case, its front lip and anything on its side. */
export function boardExtent(device: { id: string; x: number; y: number; w: number; h: number; shapes: Shape[] }, pad = 0.35) {
	const keys = device.shapes.filter((s) => s.is_key);
	const ox = keys.length ? Math.min(...keys.map((s) => s.x - s.w / 2)) : device.x;
	const oy = keys.length ? Math.min(...keys.map((s) => s.y - s.h / 2)) : device.y;
	let [x0, y0, x1, y1] = [device.x - pad, device.y - pad, device.x + device.w + pad, device.y + device.h + pad];
	const art = KEYBOARD_ART[device.id];
	if (art) {
		const [l, t, r, b] = art.case;
		x0 = Math.min(x0, ox + l - 0.05);
		y0 = Math.min(y0, oy + t - 0.05);
		x1 = Math.max(x1, ox + r + (art.sideDial ? 0.22 : 0.05));
		y1 = Math.max(y1, oy + b + 0.05);
	}
	return { x0, y0, x1, y1, origin: [ox, oy] as [number, number] };
}
