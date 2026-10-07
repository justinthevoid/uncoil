// The desk's device art, as pure geometry in desk units (1u = 19.05 mm): a port of the desktop app's drawing
// (DeskPreview.svelte, Keyboard.svelte, MouseArt.svelte, MatArt.svelte), built from the app's own data
// (lib/art/keyboards.ts, lib/art/mice.ts, lib/art/basilisk.ts). No DOM here, so it runs at build time (the SVG
// still) and in the browser (the canvas renderer). Match the app; don't re-invent it.
import { boardExtent, isStripLed, KEYBOARD_ART, keyFrames } from '$uncoil/art/keyboards';
import type { MouseArt } from '$uncoil/art/mice';
import { ledRoles, siteMouseArt, stripParts } from './mice';
import type { DeskDevice } from '$uncoil/types';

/** The app's device finish (app.css `.device-finish`): real black hardware on a cloth mat. */
export const FINISH = {
	cap: '#34302c',
	capEdge: 'rgba(0,0,0,0.5)',
	capSkirt: '#1b1917',
	case: '#0f0e0d',
	caseEdge: 'rgba(255,255,255,0.13)',
	shellTop: '#1f1d1b',
	ledOff: '#2c2926',
	seam: 'rgba(255,255,255,0.17)',
	plate: '#090808',
	cloth: '#1d1c1a',
} as const;

export interface Rect {
	x: number;
	y: number;
	w: number;
	h: number;
}
export interface RRect extends Rect {
	r: number;
}
export interface CapGeo extends RRect {
	led: number;
	name: string;
	legend: string;
	/** The keycap's top face, set back from its front edge. */
	top: RRect;
}
export interface BarGeo extends Rect {
	vertical: boolean;
	/** Global LED indices along the bar, in order. */
	leds: number[];
}
export interface DotGeo {
	led: number;
	x: number;
	y: number;
	r: number;
}
export interface KeyboardGeo {
	kind: 'keyboard';
	id: string;
	box: Rect;
	/** Board width: the app sizes radii in `cqw` of it. */
	W: number;
	shell: RRect;
	lip?: { y: number; x0: number; x1: number; bottom: number };
	frames: Rect[];
	oled?: RRect;
	dial?: RRect;
	buttons: RRect[];
	rest?: RRect;
	caps: CapGeo[];
	bars: BarGeo[];
	dots: DotGeo[];
}
export interface MouseGeo {
	kind: 'mouse';
	id: string;
	box: Rect;
	art: MouseArt;
	/** Photo pixels to desk units: desk = px * u + (tx, ty). */
	u: number;
	tx: number;
	ty: number;
	/** Strip parts (photo px) and their LEDs, front left first. */
	strip: { led: number; d: string }[];
	wheel?: number;
	logo?: number;
	/** The spiral at the logo LED: photo px = SPIRAL * s + (x, y). */
	logoAt: { s: number; x: number; y: number };
	/** Side buttons drawn as keys. */
	side: string[];
	/** Sizes scale with the photo (the Basilisk V3 Pro's body is 496 px long). */
	S: number;
}
export interface MatGeo {
	kind: 'mat';
	id: string;
	box: Rect;
	r: number;
	hub: RRect;
	/** The lit band's width. */
	E: number;
	/** One LED lighting the whole edge, or bands in order (absolute desk units). */
	ring?: number;
	bands: { led: number; pts: [number, number][] }[];
}
export interface PlainGeo {
	kind: 'plain';
	id: string;
	box: Rect;
	dots: DotGeo[];
}
export type DeviceGeo = KeyboardGeo | MouseGeo | MatGeo | PlainGeo;

export interface DeskGeo {
	/** Drawing order: mats first, so what sits on them is on top. */
	devices: DeviceGeo[];
	/** Every device's drawn box plus the app's 0.6u margin. */
	bounds: Rect;
	ledCount: number;
	/** Where each LED is drawn (x, y per LED, global order = devices' shapes in desk order). */
	anchors: Float32Array;
	/** Index into `devices` for each LED. */
	ledDevice: Int16Array;
}

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
	'Up Arrow': '↑',
	'Down Arrow': '↓',
	'Left Arrow': '←',
	'Right Arrow': '→',
	Space: '',
};
export const legendFor = (name: string) => LEGEND[name] ?? name;

const nums = (d: string) => [...d.matchAll(/-?\d+(?:\.\d+)?/g)].map((m) => +m[0]);
function pathBox(d: string): Rect {
	const n = nums(d);
	const xs = n.filter((_, i) => i % 2 === 0);
	const ys = n.filter((_, i) => i % 2 === 1);
	const x = Math.min(...xs);
	const y = Math.min(...ys);
	return { x, y, w: Math.max(...xs) - x, h: Math.max(...ys) - y };
}
function polyMid(d: string): [number, number] {
	const n = nums(d);
	const pts: [number, number][] = [];
	for (let i = 0; i + 1 < n.length; i += 2) pts.push([n[i], n[i + 1]]);
	let total = 0;
	const seg = pts.slice(1).map((p, i) => {
		const l = Math.hypot(p[0] - pts[i][0], p[1] - pts[i][1]);
		total += l;
		return l;
	});
	let half = total / 2;
	for (let i = 0; i < seg.length; i++) {
		if (half <= seg[i]) {
			const t = seg[i] ? half / seg[i] : 0;
			return [pts[i][0] + (pts[i + 1][0] - pts[i][0]) * t, pts[i][1] + (pts[i + 1][1] - pts[i][1]) * t];
		}
		half -= seg[i];
	}
	return pts[pts.length - 1] ?? [0, 0];
}

function keyboardGeo(d: DeskDevice, base: number, anchors: Float32Array): KeyboardGeo {
	const ext = boardExtent(d);
	const W = ext.x1 - ext.x0;
	const cq = W / 100; // the app's `cqw`
	const [ox, oy] = ext.origin;
	const at = (x: number, y: number, w: number, h: number): Rect => ({ x: ox + x, y: oy + y, w, h });
	const art = KEYBOARD_ART[d.id];
	const keys = d.shapes.map((s, i) => ({ s, i })).filter(({ s }) => s.is_key && !isStripLed(s.name));
	const shell: RRect = art
		? { ...at(art.case[0], art.case[1], art.case[2] - art.case[0], art.case[3] - art.case[1]), r: 1.4 * cq }
		: { x: d.x, y: d.y, w: d.w, h: d.h, r: 1.4 * cq };
	const caps: CapGeo[] = keys.map(({ s, i }) => {
		const w = s.w - 0.14;
		const h = s.h - 0.14;
		const x = s.x - w / 2;
		const y = s.y - h / 2;
		anchors[(base + i) * 2] = s.x;
		anchors[(base + i) * 2 + 1] = s.y;
		return {
			led: base + i,
			name: s.name,
			legend: legendFor(s.name),
			x,
			y,
			w,
			h,
			r: 0.75 * cq,
			top: { x: x + 0.36 * cq, y: y + 0.2 * cq, w: w - 0.72 * cq, h: h - 0.82 * cq, r: 0.55 * cq },
		};
	});
	// LEDs in a line along one side become one light bar; anything else stays a point (Keyboard.svelte).
	const leds = d.shapes.map((s, i) => ({ s, i })).filter(({ s }) => !s.is_key || isStripLed(s.name));
	const bars: BarGeo[] = [];
	const used = new Set<number>();
	for (const vertical of [true, false]) {
		const groups = new Map<number, typeof leds>();
		for (const l of leds) {
			if (used.has(l.i)) continue;
			const k = Math.round((vertical ? l.s.x : l.s.y) * 10);
			groups.set(k, [...(groups.get(k) ?? []), l]);
		}
		for (const g of groups.values()) {
			if (g.length < 3) continue;
			g.sort((a, b) => (vertical ? a.s.y - b.s.y : a.s.x - b.s.x));
			const along = g.map((l) => (vertical ? l.s.y : l.s.x));
			const step = (along[along.length - 1] - along[0]) / (g.length - 1);
			const from = along[0] - step / 2;
			const len = along[along.length - 1] - along[0] + step;
			const across = vertical ? g[0].s.x : g[0].s.y;
			const t = 0.22;
			const r: Rect = vertical ? { x: across - t / 2, y: from, w: t, h: len } : { x: from, y: across - t / 2, w: len, h: t };
			bars.push({ ...r, vertical, leds: g.map((l) => base + l.i) });
			g.forEach((l, n) => {
				used.add(l.i);
				anchors[(base + l.i) * 2] = vertical ? across : from + (n + 0.5) * (len / g.length);
				anchors[(base + l.i) * 2 + 1] = vertical ? from + (n + 0.5) * (len / g.length) : across;
			});
		}
	}
	const dots: DotGeo[] = leds
		.filter((l) => !used.has(l.i))
		.map((l) => {
			anchors[(base + l.i) * 2] = l.s.x;
			anchors[(base + l.i) * 2 + 1] = l.s.y;
			return { led: base + l.i, x: l.s.x, y: l.s.y, r: 0.11 };
		});
	const frames = keyFrames(
		keys.map(({ s }) => s),
		art?.frames,
	).flatMap((g) => g.map(([x, y, w, h]) => ({ x, y, w, h })));
	return {
		kind: 'keyboard',
		id: d.id,
		box: { x: ext.x0, y: ext.y0, w: W, h: ext.y1 - ext.y0 },
		W,
		shell,
		lip: art?.lip !== undefined ? { y: oy + art.lip, x0: ox + art.case[0], x1: ox + art.case[2], bottom: oy + art.case[3] } : undefined,
		frames,
		oled: art?.oled ? { ...at(...art.oled), r: 0.5 * cq } : undefined,
		dial: art?.sideDial ? { ...at(art.case[2] - 0.3, art.sideDial[0], 0.48, art.sideDial[1] - art.sideDial[0]), r: 0.4 * cq } : undefined,
		buttons: (art?.sideButtons ?? []).map(([t, b]) => ({ ...at(art!.case[2] - 0.05, t, 0.12, b - t), r: 0.3 * cq })),
		rest: art?.rest ? { ...at(art.rest[0], art.rest[1] + 0.08, art.rest[2] - art.rest[0], art.rest[3] - art.rest[1] - 0.08), r: 1.1 * cq } : undefined,
		caps,
		bars,
		dots,
	};
}

function mouseGeo(d: DeskDevice, art: MouseArt, base: number, anchors: Float32Array): MouseGeo {
	// DeskPreview.frame(): the body scaled to the device's depth, centred on its box.
	const u = d.h / art.body_box.h;
	const cx = art.body_box.x + art.body_box.w / 2;
	const cy = art.body_box.y + art.body_box.h / 2;
	const fx = d.x + d.w / 2 - (cx - art.view.x) * u;
	const fy = d.y + d.h / 2 - (cy - art.view.y) * u;
	const tx = fx - art.view.x * u;
	const ty = fy - art.view.y * u;
	const names = d.shapes.map((s) => s.name);
	const roles = ledRoles(names);
	const parts = stripParts(art, roles.strip.length);
	const S = art.body_box.h / 496;
	const idx = (n: string | undefined) => (n === undefined ? undefined : base + names.indexOf(n));
	const strip = parts.map((p, k) => ({ led: base + names.indexOf(roles.strip[k]), d: p }));
	const put = (led: number | undefined, px: number, py: number) => {
		if (led === undefined) return;
		anchors[led * 2] = px * u + tx;
		anchors[led * 2 + 1] = py * u + ty;
	};
	for (const s of strip) put(s.led, ...polyMid(s.d));
	const wb = pathBox(art.region.WHEEL_CLICK);
	put(idx(roles.wheel), wb.x + wb.w / 2, wb.y + wb.h / 2);
	put(idx(roles.logo), art.logo[0], art.logo[1]);
	return {
		kind: 'mouse',
		id: d.id,
		box: { x: fx, y: fy, w: art.view.w * u, h: art.view.h * u },
		art,
		u,
		tx,
		ty,
		strip,
		wheel: idx(roles.wheel),
		logo: idx(roles.logo),
		logoAt: { s: 1.7 * S, x: art.logo[0] - 12 * 1.7 * S, y: art.logo[1] - 12 * 1.7 * S },
		side: ['FORWARD', 'BACK', 'CLUTCH', 'SCROLL_MODE', 'DPI_BUTTON'].filter((k) => art.region[k]),
		S,
	};
}

function matGeo(d: DeskDevice, base: number, anchors: Float32Array): MatGeo {
	const W = d.w;
	const H = d.h;
	const R = Math.min(0.6, H * 0.06);
	const E = 0.18;
	const n = d.shapes.length;
	const bands: MatGeo['bands'] = [];
	if (n > 1) {
		const i = E / 2;
		const pts: [number, number][] = [
			[i, i + 0.4],
			[i, H - i],
			[W - i, H - i],
			[W - i, i + 0.4],
		];
		const seg = pts.slice(1).map((p, k) => Math.hypot(p[0] - pts[k][0], p[1] - pts[k][1]));
		const total = seg.reduce((a, b) => a + b, 0);
		const at = (dd: number): [number, number] => {
			for (let k = 0; k < seg.length; k++) {
				if (dd <= seg[k] || k === seg.length - 1) {
					const t = Math.min(dd / seg[k], 1);
					return [d.x + pts[k][0] + (pts[k + 1][0] - pts[k][0]) * t, d.y + pts[k][1] + (pts[k + 1][1] - pts[k][1]) * t];
				}
				dd -= seg[k];
			}
			return [d.x + pts[3][0], d.y + pts[3][1]];
		};
		for (let k = 0; k < n; k++) {
			const a = (total * k) / n + 0.06;
			const b = (total * (k + 1)) / n - 0.06;
			const p = Array.from({ length: 7 }, (_, s) => at(a + ((b - a) * s) / 6));
			bands.push({ led: base + k, pts: p });
			const m = p[3];
			anchors[(base + k) * 2] = m[0];
			anchors[(base + k) * 2 + 1] = m[1];
		}
	} else if (n === 1) {
		// one LED lights the whole edge: its point rests on the front edge
		anchors[base * 2] = d.x + W / 2;
		anchors[base * 2 + 1] = d.y + H - E / 2;
	}
	return {
		kind: 'mat',
		id: d.id,
		box: { x: d.x, y: d.y, w: W, h: H },
		r: R,
		hub: { x: d.x + 1.1, y: d.y - 0.24, w: 1.5, h: 0.5, r: 0.16 },
		E,
		ring: n === 1 ? base : undefined,
		bands,
	};
}

/** Build the desk once. LED indices follow the devices' shapes in desk order (as `paint()` does). */
export function deskGeometry(desk: DeskDevice[], mouseArt: (id: string) => MouseArt | undefined = siteMouseArt): DeskGeo {
	const ledCount = desk.reduce((n, d) => n + d.shapes.length, 0);
	const anchors = new Float32Array(ledCount * 2);
	const ledDevice = new Int16Array(ledCount);
	const built: { geo: DeviceGeo; mat: boolean }[] = [];
	let base = 0;
	for (const d of desk) {
		let geo: DeviceGeo;
		const art = d.kind === 'mouse' ? mouseArt(d.id) : undefined;
		if (d.kind === 'keyboard') geo = keyboardGeo(d, base, anchors);
		else if (d.kind === 'mousemat') geo = matGeo(d, base, anchors);
		else if (art) geo = mouseGeo(d, art, base, anchors);
		else {
			const dots = d.shapes.map((s, i) => {
				anchors[(base + i) * 2] = s.x;
				anchors[(base + i) * 2 + 1] = s.y;
				return { led: base + i, x: s.x, y: s.y, r: 0.16 };
			});
			geo = { kind: 'plain', id: d.id, box: { x: d.x, y: d.y, w: d.w, h: d.h }, dots };
		}
		built.push({ geo, mat: d.kind === 'mousemat' });
		base += d.shapes.length;
	}
	built.sort((a, b) => Number(b.mat) - Number(a.mat));
	const devices = built.map((b) => b.geo);
	// LED -> drawing index
	let b0 = 0;
	for (const d of desk) {
		const k = devices.findIndex((g) => g.id === d.id);
		for (let i = 0; i < d.shapes.length; i++) ledDevice[b0 + i] = k;
		b0 += d.shapes.length;
	}
	const boxes = devices.map((g) => g.box);
	const x0 = Math.min(...boxes.map((b) => b.x)) - 0.6;
	const y0 = Math.min(...boxes.map((b) => b.y)) - 0.6;
	const bounds = { x: x0, y: y0, w: Math.max(...boxes.map((b) => b.x + b.w)) + 0.6 - x0, h: Math.max(...boxes.map((b) => b.y + b.h)) + 0.6 - y0 };
	return { devices, bounds, ledCount, anchors, ledDevice };
}
