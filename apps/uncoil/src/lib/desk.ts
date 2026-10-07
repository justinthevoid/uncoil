// Moving devices on the desk: where they sit is `config.desk` (each device's origin, in key units), which the
// daemon hot-reloads, so the effects follow the desk as it really is.
import type { Config, DeskDevice } from './types';

/** Snap to a quarter key, or to a twentieth with `fine`. */
export const snap = (v: number, fine = false) => (fine ? Math.round(v * 20) / 20 : Math.round(v * 4) / 4);

/** Move devices by (dx, dy) key units from where they sit now. */
export function moveOnDesk(config: Config, desk: DeskDevice[], ids: string[], dx: number, dy: number) {
	if (!dx && !dy) return;
	const next = { ...(config.desk ?? {}) };
	for (const id of ids) {
		const d = desk.find((x) => x.id === id);
		if (!d) continue;
		next[id] = { x: Math.round((d.at.x + dx) * 1000) / 1000, y: Math.round((d.at.y + dy) * 1000) / 1000 };
	}
	config.desk = next;
}

/** Every device back where uncoil places it. */
export function resetDesk(config: Config) {
	config.desk = {};
}

/** The PC's devices (driven through OpenRGB) move together, as one PC. */
export const isPc = (id: string) => id.startsWith('openrgb:');
