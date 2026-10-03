// Connected devices as the control pipe reports them, shared by the Keys and Hardware screens.
import { daemon, DaemonError } from './api';
import type { DeviceInfo, Feature } from './types';

export const pipe = $state({
	devices: [] as DeviceInfo[],
	loaded: false,
	loading: false,
	/** uncoild isn't answering on the pipe (not running, or a build from before the control pipe). */
	unreachable: false,
	error: null as string | null
});

export async function loadDevices() {
	pipe.loading = true;
	try {
		pipe.devices = await daemon<DeviceInfo[]>('devices');
		pipe.unreachable = false;
		pipe.error = null;
	} catch (e) {
		pipe.devices = [];
		pipe.unreachable = e instanceof DaemonError && e.unreachable;
		pipe.error = String(e instanceof Error ? e.message : e);
	} finally {
		pipe.loading = false;
		pipe.loaded = true;
	}
}

const ORDER: Record<string, number> = { keyboard: 0, mouse: 1, mousemat: 2, headset: 3 };

/** Connected devices with a feature, keyboard first. */
export const withFeature = (f: Feature) =>
	pipe.devices.filter((d) => d.features.includes(f)).sort((a, b) => (ORDER[a.kind] ?? 9) - (ORDER[b.kind] ?? 9));

/** "Razer BlackWidow V4 Pro 75%" → "BlackWidow V4 Pro 75%". */
export const shortName = (name: string) => name.replace(/^Razer /, '');

export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));
