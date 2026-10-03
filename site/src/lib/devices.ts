// Supported devices, read at build time from the same devices/*.toml the daemon compiles in.
// Adding a TOML there adds a row here; nothing on the site is typed by hand.
import { parse } from 'smol-toml';

type UsbEndpoint = { product_id: number; connection?: string };
type DeviceToml = {
	id: string;
	name: string;
	kind: string;
	vendor_id: number;
	usb: UsbEndpoint[];
	quirks?: { ack_every_report?: boolean; custom_mode_once?: boolean };
	matrix: { rows: number; cols: number; names: string[][] };
	layout: { type: string };
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

const hex4 = (n: number) => n.toString(16).toUpperCase().padStart(4, '0');

/** "Strip 1".."Strip 11" -> "Strip ×11"; keyboards -> "84 keys + 18 underglow". */
function lighting(def: DeviceToml, names: string[]): string {
	if (names.length === 1) return '1 zone';
	if (def.kind === 'keyboard') {
		const glow = names.filter((n) => /^(LU|RU)\d+$/.test(n)).length;
		const keys = names.length - glow;
		return glow ? `${keys} keys + ${glow} underglow` : `${keys} keys`;
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
		const names = def.matrix.names.flat().filter((n) => n !== '');
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
