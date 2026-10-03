// Mirrors of the serde types in uncoil-core. Keep in sync with crates/uncoil-core/src/{config,effect,layout}.rs.

/** `color::Rgb` is a tuple struct, so serde writes it as `[r, g, b]`. */
export type Rgb = [number, number, number];

export type WaveEffect = {
	kind: 'wave';
	/** 0 = sweeps left to right, 90 = back to front. */
	angle_deg: number;
	/** Seconds for one colour cycle to pass a point (higher = slower). */
	period_s: number;
	/** Width of one full rainbow, in key units. */
	wavelength: number;
	reverse: boolean;
};
export type SpectrumEffect = { kind: 'spectrum'; period_s: number };
export type StaticEffect = { kind: 'static'; color: Rgb };
export type OffEffect = { kind: 'off' };
export type Effect = WaveEffect | SpectrumEffect | StaticEffect | OffEffect;
export type EffectKind = Effect['kind'];

export interface DisplayPolicy {
	off_when_display_off: boolean;
	dim_level: number;
	fade_s: number;
}

export interface Placement {
	x: number;
	y: number;
}

export interface Config {
	effect: Effect;
	brightness: number;
	saturation: number;
	fps: number;
	display: DisplayPolicy;
	desk: Record<string, Placement>;
	openrgb_hardware_rainbow: boolean;
}

export interface Shape {
	name: string;
	row: number;
	col: number;
	x: number;
	y: number;
	w: number;
	h: number;
	is_key: boolean;
}

export type DeviceKind = 'keyboard' | 'mouse' | 'mousemat' | 'headset' | 'other';

export interface DeskDevice {
	id: string;
	name: string;
	kind: DeviceKind;
	x: number;
	y: number;
	w: number;
	h: number;
	shapes: Shape[];
}

export interface DeviceStatus {
	id: string;
	name: string;
	product_id: number;
	connection: string;
	fps: number;
	busy_retries: number;
	errors: number;
}

export interface Status {
	pid: number;
	version: string;
	started_unix: number;
	updated_unix: number;
	display: string;
	level: number;
	devices: DeviceStatus[];
}
