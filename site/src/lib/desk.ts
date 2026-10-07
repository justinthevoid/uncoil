// The maintainer's real desk as flat LED records, usable at build time and in the browser. The landing page draws
// from this and from the app's own effect maths, so whatever lights up on the page is what uncoild would send.
import desk from '$uncoil/mock/desk.json';
import { frameWith, hex } from '$uncoil/effect';
import type { DeskDevice, Effect, Rgb } from '$uncoil/types';

export { frameWith, hex };
export type { Effect, Rgb };

export const devices = desk as unknown as DeskDevice[];

/** One LED: which device, its layout name, its measured centre and size in key units (1u = 19.05 mm). */
export interface Led {
	device: string;
	kind: DeskDevice['kind'];
	name: string;
	x: number;
	y: number;
	w: number;
	h: number;
	key: boolean;
}

export const leds: Led[] = devices.flatMap((d) =>
	d.shapes.map((s) => ({ device: d.id, kind: d.kind, name: s.name, x: s.x, y: s.y, w: s.w, h: s.h, key: s.is_key })),
);

/** Union of the device bodies, in key units. */
export const bounds = (() => {
	const xs = devices.flatMap((d) => [d.x, d.x + d.w]);
	const ys = devices.flatMap((d) => [d.y, d.y + d.h]);
	const x0 = Math.min(...xs);
	const y0 = Math.min(...ys);
	return { x0, y0, w: Math.max(...xs) - x0, h: Math.max(...ys) - y0 };
})();

/** uncoil's shipped default: an angled rainbow wave. */
export const WAVE: Effect = { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false };

/** Colour every LED for one moment of an effect. Returns [r,g,b] per LED, in `leds` order. */
export function paint(effect: Effect, t: number, sat = 1, val = 1): Rgb[] {
	const f = frameWith(effect, t, sat, val);
	return leds.map((l) => f(l.device, l.name, l.x, l.y));
}

/** Measured on the maintainer's PC (README.md, PRODUCT.md). The only numbers the site may show. */
export const MEASURED = {
	binaryBytes: 1_402_880,
	binary: '1.4 MB',
	ram: '~3 MB',
	cpu: 'under 1% of one core',
	processes: 1,
	synapseProcesses: 17,
	synapseRam: '~1.4 GB at start, leaking to several GB over days',
	synapseCpu: '~7% of one core',
	synapseInstall: '~500 MB',
	logsMined: '130 MB of Synapse logs',
	commands: 30,
	inputEvents: 116,
	reportBytes: 90,
	note: 'Measured on the maintainer’s PC. Memory and CPU on an earlier build. One machine; yours will differ.',
};
