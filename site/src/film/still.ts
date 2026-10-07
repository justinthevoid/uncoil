// Build-time stills for the landing page: what the page shows with no JavaScript, and the
// facts the film draws from. Every colour here is the engine's own wave at t = 6 s, at each LED's real position.
import { leds, devices, paint, WAVE, hex } from '../lib/desk';
import { deskGeometry } from '../art/geometry';
import { deskSvg as artSvg } from '../art/svg';

export const GEO = deskGeometry(devices);

const STILL_T = 6;
const r3 = (n: number) => +n.toFixed(3);


/** The desk at rest, in the app's device art (../art), lit with the wave's still frame. */
export function deskSvg(): string {
	const c = paint(WAVE, STILL_T).map(hex);
	return artSvg(GEO, { colors: c, id: 'still-desk' });
}

/** The Fn layer as the keyboard stores it, with each changed key's gel tab. */
export const FN_KEYS: { key: string; sub: string; action: string; gel: Gel }[] = [
	{ key: 'Escape', sub: 'Eco', action: 'Low power mode', gel: 'system' },
	{ key: 'F9', sub: 'Macro', action: 'Macro record', gel: 'media' },
	{ key: 'F10', sub: 'Game', action: 'Game mode', gel: 'media' },
	{ key: 'F11', sub: 'Lit −', action: 'Backlight down', gel: 'light' },
	{ key: 'F12', sub: 'Lit +', action: 'Backlight up', gel: 'light' },
	{ key: 'Delete', sub: 'Sleep', action: 'System sleep', gel: 'system' },
	{ key: 'P', sub: 'PrtSc', action: 'Print Screen', gel: 'yours' },
];
export type Gel = 'yours' | 'media' | 'light' | 'system';
export const GEL: Record<Gel, { name: string; hex: string; job: string }> = {
	yours: { name: 'Your change', hex: '#58b06c', job: 'keys and buttons you remapped' },
	media: { name: 'Media & macros', hex: '#e0a33e', job: 'media keys, macro record, game mode' },
	light: { name: 'Lighting', hex: '#cf5aa0', job: 'backlight brighter and dimmer' },
	system: { name: 'System', hex: '#5b84e0', job: 'sleep, low power, profiles and DPI' },
};

/** The keyboard alone, Fn held: dark caps, gel tabs on the keys Fn changes, Fn and P marked. */
export function fnSvg(): string {
	const kb = GEO.devices.find((d) => d.kind === 'keyboard')!;
	const pad = 0.3;
	return artSvg(GEO, {
		id: 'still-fn',
		view: { x: kb.box.x - pad, y: kb.box.y - pad, w: kb.box.w + pad * 2, h: kb.box.h + pad * 2 },
		gels: new Map(FN_KEYS.map((f) => [f.key, GEL[f.gel].hex])),
		marks: new Map([['Right Fn', '#c9a46a'], ['P', '#c9a46a']]),
	});
}

/** Seventeen dim coils and one bright line: the measured comparison as a still. */
export function shedSvg(): string {
	const c = paint(WAVE, STILL_T);
	const order = leds.map((l, i) => ({ i, p: l.x * Math.cos(0.6109) + l.y * Math.sin(0.6109) })).sort((a, b) => a.p - b.p);
	const stops = [0, 0.25, 0.5, 0.75, 1].map((f) => {
		const o = order[Math.min(order.length - 1, Math.round(f * (order.length - 1)))];
		return `<stop offset="${f}" stop-color="${hex(c[o.i])}"/>`;
	});
	const coils: string[] = [];
	for (let j = 0; j < 17; j++) {
		const a = j * 2.39996;
		const rr = 9 * Math.sqrt((j + 0.5) / 17);
		const x = 30 + rr * Math.cos(a) * 1.5;
		const y = 17 + rr * Math.sin(a);
		coils.push(`<use href="#u-coil" transform="translate(${r3(x - 6)} ${r3(y - 6)}) rotate(${(j * 47) % 360} 6 6) scale(0.5)"/>`);
	}
	return `<svg viewBox="0 0 60 44" xmlns="http://www.w3.org/2000/svg"><defs><path id="u-coil" d="M12 12a2 2 0 1 1 2-2 4 4 0 1 1-4-4 6 6 0 1 1-6 6 8 8 0 1 1 8 8" fill="none" stroke="#c9a46a" stroke-opacity="0.32" stroke-width="1.6" stroke-linecap="round"/><linearGradient id="u-line">${stops.join('')}</linearGradient></defs>${coils.join('')}<rect x="6" y="38" width="48" height="0.7" rx="0.35" fill="url(#u-line)"/></svg>`;
}

/** The Fn+P write as a whole 90-byte feature report, laid out per docs/PROTOCOL.md ("Transport"). */
export type Field = 'status' | 'txn' | 'header' | 'command' | 'args' | 'crc' | 'reserved';
export const FIELDS: { from: number; to: number; field: Field; name: string; note: string }[] = [
	{ from: 0, to: 0, field: 'status', name: 'Status', note: '00 new · 01 busy · 02 ok · 03 fail · 04 no answer · 05 unsupported' },
	{ from: 1, to: 1, field: 'txn', name: 'Transaction id', note: '1F keyboard and mouse · 3F mat' },
	{ from: 2, to: 5, field: 'header', name: 'Header', note: 'remaining packets, protocol type, data size' },
	{ from: 6, to: 7, field: 'command', name: 'Command', note: 'class and id; high bit set = a read' },
	{ from: 8, to: 87, field: 'args', name: 'Arguments', note: 'up to 80 bytes' },
	{ from: 88, to: 88, field: 'crc', name: 'CRC', note: 'XOR of bytes 2–87' },
	{ from: 89, to: 89, field: 'reserved', name: 'Reserved', note: '' },
];
export const report: { b: number; field: Field }[] = (() => {
	const bytes = new Array(90).fill(0);
	bytes[1] = 0x1f;
	bytes[5] = 0x50; // data size byte for 02/0D, as PROTOCOL.md records it
	bytes[6] = 0x02;
	bytes[7] = 0x0d;
	[1, 26, 1, 2, 2, 0, 0x46].forEach((v, i) => (bytes[8 + i] = v));
	let crc = 0;
	for (let i = 2; i <= 87; i++) crc ^= bytes[i];
	bytes[88] = crc;
	return bytes.map((b, i) => ({ b, field: FIELDS.find((f) => i >= f.from && i <= f.to)!.field }));
})();
export const hex2 = (n: number) => n.toString(16).toUpperCase().padStart(2, '0');
