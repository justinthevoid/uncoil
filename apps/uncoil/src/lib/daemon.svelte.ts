// Connected devices as the control pipe reports them, shared by the device pages.
import { daemon, DaemonError } from './api';
import type { DeviceInfo } from './types';
import { EXPERIMENTAL, EXPERIMENTAL_TEXT } from './checks';

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

/** "Razer BlackWidow V4 Pro 75%" → "BlackWidow V4 Pro 75%"; "… Chroma Extended" → "… Chroma" (tabs, titles). */
export const shortName = (name: string) => name.replace(/^Razer /, '').replace(/ Chroma Extended$/, ' Chroma');

/** The message to show for a failed call (a `DaemonError` carries its code separately, in `code`). */
export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Experimental devices: set up from OpenRazer/OpenRGB data, not yet confirmed on real hardware. */
export const isExperimental = (id: string) => pipe.devices.find((d) => d.id === id)?.support === 'experimental';

/** The page-title tag for an experimental device (null for supported ones). */
export const experimentalBadge = (id: string) => (isExperimental(id) ? { text: EXPERIMENTAL, title: EXPERIMENTAL_TEXT } : null);
