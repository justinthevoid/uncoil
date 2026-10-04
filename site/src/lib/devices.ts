// Device lists, read at build time from the same files the daemon compiles in: devices/*.toml (supported) and
// devices/experimental/*.toml (experimental). Adding a TOML there adds a row here; nothing is typed by hand.
import { parse } from 'smol-toml';

type UsbEndpoint = { product_id: number; connection?: string };
type DeviceToml = {
	id: string;
	name: string;
	kind: string;
	vendor_id: number;
	support?: 'supported' | 'experimental';
	features?: string[];
	usb: UsbEndpoint[];
	quirks?: { ack_every_report?: boolean; custom_mode_once?: boolean };
	// Optional: devices without RGB have neither.
	matrix?: { rows: number; cols: number; names: string[][] };
	layout?: { type: string };
};

export interface DeviceRow {
	id: string;
	name: string;
	kind: string;
	file: string;
	leds: number;
	lighting: string;
	usb: { pid: string; connection: string }[];
	quirks: string[];
}

const files = import.meta.glob('../../../devices/*.toml', {
	query: '?raw',
	import: 'default',
	eager: true,
}) as Record<string, string>;

const experimentalFiles = import.meta.glob('../../../devices/experimental/*.toml', {
	query: '?raw',
	import: 'default',
	eager: true,
}) as Record<string, string>;

const hex4 = (n: number) => n.toString(16).toUpperCase().padStart(4, '0');

/** "Strip 1".."Strip 11" -> "Strip ×11"; keyboards -> "84 keys + 18 underglow". */
function lighting(def: DeviceToml, names: string[]): string {
	if (names.length === 0) return 'no lighting';
	if (names.length === 1) return '1 zone';
	if (def.kind === 'keyboard') {
		const glow = names.filter((n) => /^(LU|RU)\d+$/.test(n)).length;
		const rest = names.filter((n) => /^WR\d+$/.test(n)).length;
		const keys = names.length - glow - rest;
		return [`${keys} keys`, glow && `${glow} underglow`, rest && `${rest} wrist rest`].filter(Boolean).join(' + ');
	}
	const groups = new Map<string, number>();
	for (const n of names) {
		const g = n.replace(/\s*\d+$/, '');
		groups.set(g, (groups.get(g) ?? 0) + 1);
	}
	return [...groups].map(([g, c]) => (c > 1 ? `${g.toLowerCase()} ×${c}` : g.toLowerCase())).join(', ');
}

export const devices: DeviceRow[] = Object.entries(files)
	.map(([path, src]) => {
		const def = parse(src) as unknown as DeviceToml;
		const names = (def.matrix?.names ?? []).flat().filter((n) => n !== '');
		const quirks: string[] = [];
		if (def.quirks?.ack_every_report) quirks.push('reads every reply');
		if (def.quirks?.custom_mode_once ?? true) quirks.push('custom mode once');
		return {
			id: def.id,
			name: def.name,
			kind: def.kind,
			file: `devices/${path.split('/').pop()}`,
			leds: names.length,
			lighting: lighting(def, names),
			usb: def.usb.map((u) => ({ pid: `${hex4(def.vendor_id)}:${hex4(u.product_id)}`, connection: u.connection ?? '' })),
			quirks,
		};
	})
	.sort((a, b) => ['keyboard', 'mouse', 'mousemat'].indexOf(a.kind) - ['keyboard', 'mouse', 'mousemat'].indexOf(b.kind));

export const totalLeds = devices.reduce((n, d) => n + d.leds, 0);

const KIND_ORDER = ['keyboard', 'mouse', 'mousemat', 'headset', 'other'];

export interface ExperimentalRow {
	id: string;
	name: string;
	kind: string;
	file: string;
	lighting: string;
	/** Firmware effects only (no per-LED frames) when the sources disagree about per-LED lighting. */
	effectsOnly: boolean;
	usb: { pid: string; connection: string }[];
}

/** Experimental devices: built from OpenRazer / OpenRGB data, not yet confirmed on real hardware. */
export const experimentalDevices: ExperimentalRow[] = Object.entries(experimentalFiles)
	.map(([path, src]) => {
		const def = parse(src) as unknown as DeviceToml;
		const names = (def.matrix?.names ?? []).flat().filter((n) => n !== '');
		const features = def.features ?? ['lighting'];
		return {
			id: def.id,
			name: def.name,
			kind: def.kind,
			file: `devices/experimental/${path.split('/').pop()}`,
			lighting: lighting(def, names),
			effectsOnly: names.length > 0 && !features.includes('lighting'),
			usb: def.usb.map((u) => ({ pid: `${hex4(def.vendor_id)}:${hex4(u.product_id)}`, connection: u.connection ?? '' })),
		};
	})
	.sort((a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) || a.name.localeCompare(b.name));
