// Studio masks as the app edits them: which lights a layer covers, in words, and free selection across the
// desk. A "keys" mask (one device, from older configs) reads as the same lights as a "lights" mask.
import type { DeskDevice, Mask } from './types';

export type Light = [device: string, shape: string];

/** The lights a picking mask covers (keys or lights); empty for whole-desk and device masks. */
export function lightsOf(m: Mask): Light[] {
	if (m.kind === 'lights') return m.lights;
	if (m.kind === 'keys') return m.shapes.map((s): Light => [m.device, s]);
	return [];
}

/** "deviceId/shape" for each picked light, for the desk preview's outlines. */
export const markedOf = (m: Mask): Set<string> => new Set(lightsOf(m).map(([d, s]) => `${d}/${s}`));

/** Toggle-paint one light: `add` decides (taken from the first light of a drag). Returns a lights mask. */
export function paint(m: Mask, light: Light, add: boolean): Mask {
	const list = lightsOf(m);
	const has = list.some(([d, s]) => d === light[0] && s === light[1]);
	if (add === has) return m.kind === 'lights' ? m : { kind: 'lights', lights: list };
	return { kind: 'lights', lights: add ? [...list, light] : list.filter(([d, s]) => !(d === light[0] && s === light[1])) };
}

export const hasLight = (m: Mask, light: Light) => lightsOf(m).some(([d, s]) => d === light[0] && s === light[1]);

/** What a mask covers, in plain words. */
export function coverage(m: Mask, name: (id: string) => string): string {
	if (m.kind === 'all') return 'Whole desk';
	if (m.kind === 'devices') return m.ids.length ? m.ids.map(name).join(', ') : 'No devices yet';
	const list = lightsOf(m);
	if (!list.length) return 'No lights picked yet';
	const devices = [...new Set(list.map(([d]) => d))];
	const where = devices.length > 2 ? `${devices.length} devices` : devices.map(name).join(' and ');
	return `${list.length} ${list.length === 1 ? 'light' : 'lights'} on ${where}`;
}

/** Every light of the given devices. */
export const allLightsOf = (desk: DeskDevice[], ids: string[]): Light[] =>
	desk.filter((d) => ids.includes(d.id)).flatMap((d) => d.shapes.map((s): Light => [d.id, s.name]));
