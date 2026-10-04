// uncoil's default effect. Its own module (no desk data) so the in-browser animation stays tiny.
import type { Effect } from '$uncoil/types';

/** The default effect, exactly as uncoil ships it (uncoil_core::effect::Effect::default()). */
export const EFFECT: Effect = { kind: 'wave', angle_deg: 35, period_s: 14, wavelength: 26, reverse: false };

/** The moment drawn as the still frame (no JavaScript, reduced motion, paused). */
export const STILL_T = 6;
