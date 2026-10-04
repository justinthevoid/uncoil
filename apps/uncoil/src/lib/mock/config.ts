// The browser mock's config.json: `Config::default()` (checked against fixtures.json by scripts/check-mirror.mjs).
import type { Config } from '../types';

export const defaultConfig = (): Config => ({
	effect: { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false },
	brightness: 1,
	saturation: 1,
	fps: 30,
	display: { off_when_display_off: true, dim_level: 0.35, fade_s: 1.2 },
	desk: {},
	openrgb_hardware_rainbow: false,
	openrgb: { devices: [] }
});
