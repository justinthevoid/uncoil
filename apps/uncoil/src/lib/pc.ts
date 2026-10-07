// The PC's own lighting, as OpenRGB reports it, sorted into the parts of a PC so the PC page can draw each
// where it sits: memory in the DIMM slots, the graphics card under the CPU, the motherboard's own LEDs, fans in
// the front, top and rear, an AIO pump on the CPU, and anything else along the PSU shroud. Sorting is by the
// names OpenRGB gives devices and zones; it is a best guess, and an unknown part still shows, as "other".
import type { OpenRgbDevice } from './types';

export type PartKind = 'ram' | 'gpu' | 'board' | 'fan' | 'pump' | 'radiator' | 'other';

export interface Part {
	kind: PartKind;
	/** The OpenRGB device and the zone this part is. */
	device: string;
	deviceName: string;
	zone: string;
	/** The part's LEDs: indexes into the device's LEDs, in order. */
	leds: number[];
}

const RAM = /\b(ddr\d?|dram|vengeance|dominator|trident|fury|ripjaws|ram|memory)\b/i;
const GPU = /\b(geforce|rtx|gtx|radeon|rx ?\d{3,4}|arc a\d{3}|gpu|graphics)\b/i;
const BOARD = /\b(mainboard|motherboard|aura|mystic|rgb fusion|polychrome|rog|strix|tuf|aorus|mpg|mag|meg|prime|steel legend|taichi|[abxz]\d{3}[a-z]?)\b/i;
const PUMP = /\b(pump|lcd|cpu block|waterblock|kraken|capellix)\b/i;
const RADIATOR = /\b(h\d{2,3}i|radiator|aio)\b/i;
const FAN = /\b(fan|qx|ll|ql|sl|al|uni|ar12|rs|ml|af|sp)\b/i;

function kindOf(device: string, zone: string): PartKind {
	const both = `${device} ${zone}`;
	if (RAM.test(both)) return 'ram';
	if (GPU.test(both)) return 'gpu';
	if (PUMP.test(zone)) return 'pump';
	if (RADIATOR.test(zone)) return 'radiator';
	if (FAN.test(zone)) return 'fan';
	if (BOARD.test(both)) return 'board';
	return 'other';
}

/** Every lit zone of every OpenRGB device, as a PC part. */
export function pcParts(devices: OpenRgbDevice[]): Part[] {
	const parts: Part[] = [];
	for (const d of devices) {
		const zones = d.zones?.length ? d.zones : [{ name: d.name, kind: 'linear' as const, leds: d.leds }];
		let first = 0;
		for (const z of zones) {
			if (z.leds <= 0) continue;
			const leds = Array.from({ length: z.leds }, (_, i) => first + i);
			first += z.leds;
			parts.push({ kind: kindOf(d.name, z.name), device: d.id, deviceName: d.name, zone: z.name, leds });
		}
	}
	return parts;
}

/** Where fans go, in the order they fill: the top (often a radiator), the front, the rear, then the floor. */
export const FAN_SLOTS = ['top 1', 'top 2', 'top 3', 'front 1', 'front 2', 'front 3', 'rear', 'floor 1', 'floor 2', 'floor 3'] as const;
