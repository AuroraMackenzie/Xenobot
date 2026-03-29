use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PointerState {
    pub target_x: f32,
    pub target_y: f32,
    pub current_x: f32,
    pub current_y: f32,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            target_x: 0.0,
            target_y: 0.0,
            current_x: 0.0,
            current_y: 0.0,
        }
    }
}

impl PointerState {
    pub fn eased(self, responsiveness: f32) -> Self {
        let gain = responsiveness.clamp(0.0, 1.0);
        Self {
            current_x: self.current_x + (self.target_x - self.current_x) * gain,
            current_y: self.current_y + (self.target_y - self.current_y) * gain,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BlackHoleConfig {
    pub radius: f32,
    pub width: f32,
    pub height: f32,
    pub reduced_motion: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Orientation {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub sky_yaw: f32,
    pub sky_pitch: f32,
    pub sky_roll: f32,
    pub precession: f32,
    pub disc_spin: f32,
    pub jitter: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DiscRotation {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BlackHoleAnchor {
    pub x: f32,
    pub y: f32,
}

fn pointer_energy(pointer: PointerState) -> f32 {
    (pointer.current_x.hypot(pointer.current_y)).clamp(0.0, 1.0)
}

pub fn scene_orientation(
    time_seconds: f32,
    pointer: PointerState,
    reduced_motion: bool,
) -> Orientation {
    let motion_scale = if reduced_motion { 0.18 } else { 1.0 };
    let energy = pointer_energy(pointer);
    let yaw = (time_seconds * 0.13).sin() * 0.06 + pointer.current_x * 0.46 * motion_scale;
    let pitch = (time_seconds * 0.1).cos() * 0.04 + pointer.current_y * 0.34 * motion_scale;
    let roll = (time_seconds * 0.09).sin() * 0.06
        + (pointer.current_x * 0.18 - pointer.current_y * 0.12) * motion_scale;
    let precession = time_seconds * (0.16 + energy * 0.025) * motion_scale.max(0.18);
    let disc_spin = time_seconds * (0.92 + energy * 0.08) * motion_scale;
    let jitter = if reduced_motion {
        0.0
    } else {
        ((time_seconds * 7.8).sin()
            + (time_seconds * 5.1 + 0.6).cos() * 0.7
            + (time_seconds * 11.4 + 1.9).sin() * 0.32)
            * 0.0046
    };

    Orientation {
        yaw,
        pitch,
        roll,
        sky_yaw: yaw * 0.82 + precession * 0.24,
        sky_pitch: pitch * 0.72,
        sky_roll: roll * 0.68 + precession * 0.1,
        precession,
        disc_spin,
        jitter,
    }
}

pub fn disc_rotation(orientation: Orientation) -> DiscRotation {
    DiscRotation {
        pitch: 0.66 + orientation.pitch * 0.96,
        yaw: orientation.yaw * 1.04 + orientation.precession.sin() * 0.085,
        roll: orientation.roll * 0.92 + (orientation.precession * 0.68).cos() * 0.036,
    }
}

pub fn scene_anchor(
    config: BlackHoleConfig,
    time_seconds: f32,
    pointer: PointerState,
    orientation: Orientation,
) -> BlackHoleAnchor {
    let center_x = config.width * 0.5;
    let center_y = config.height * 0.5;
    let layout_bias_x = (config.width * 0.11).min(180.0);
    let layout_bias_y = (config.height * 0.045).min(64.0);
    let orbital_radius = if config.reduced_motion {
        config.radius * 0.018
    } else {
        config.radius * 0.072
    };
    let drift_x = ((time_seconds * 0.53 + 0.4).sin()
        + (time_seconds * 1.31 + 1.8).sin() * 0.54
        + (time_seconds * 2.47 + 0.1).cos() * 0.28)
        * orbital_radius;
    let drift_y = ((time_seconds * 0.41 + 0.9).cos()
        + (time_seconds * 1.74 + 0.5).sin() * 0.46
        + (time_seconds * 2.89 + 1.4).cos() * 0.24)
        * orbital_radius
        * 0.58;
    let motion_scale = if config.reduced_motion { 0.18 } else { 1.0 };
    let pointer_x = pointer.current_x * config.radius * 0.11 * motion_scale;
    let pointer_y = pointer.current_y * config.radius * 0.08 * motion_scale;
    let jitter_radius = config.radius * 0.028 * motion_scale;
    let chaotic_x = ((time_seconds * 9.6 + orientation.precession * 4.2).sin()
        + (time_seconds * 6.4 + 0.7).cos() * 0.62
        + (time_seconds * 13.8 + 1.4).sin() * 0.24)
        * jitter_radius;
    let chaotic_y = ((time_seconds * 8.8 + 1.1).cos()
        + (time_seconds * 5.3 + orientation.roll * 8.2).sin() * 0.58
        + (time_seconds * 12.1 + 2.2).cos() * 0.22)
        * jitter_radius
        * 0.75;

    BlackHoleAnchor {
        x: center_x + layout_bias_x + drift_x + pointer_x + chaotic_x,
        y: center_y + layout_bias_y + drift_y + pointer_y + chaotic_y,
    }
}

fn halton(mut index: u32, base: u32) -> f32 {
    let mut result = 0.0f32;
    let mut fraction = 1.0f32 / base as f32;
    while index > 0 {
        result += fraction * (index % base) as f32;
        index /= base;
        fraction /= base as f32;
    }
    result
}

pub fn taa_jitter(frame_index: u32, strength: f32) -> (f32, f32) {
    let wrapped = frame_index % 16;
    let x = (halton(wrapped + 1, 2) - 0.5) * strength;
    let y = (halton(wrapped + 1, 3) - 0.5) * strength;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_easing_moves_toward_target() {
        let state = PointerState {
            target_x: 1.0,
            target_y: -1.0,
            current_x: 0.0,
            current_y: 0.0,
        };
        let eased = state.eased(0.25);
        assert!(eased.current_x > 0.0);
        assert!(eased.current_y < 0.0);
    }

    #[test]
    fn orientation_stays_bounded() {
        let pointer = PointerState {
            current_x: 0.8,
            current_y: -0.6,
            ..PointerState::default()
        };
        let orientation = scene_orientation(14.0, pointer, false);
        assert!(orientation.yaw.abs() < 1.0);
        assert!(orientation.pitch.abs() < 1.0);
        assert!(orientation.roll.abs() < 1.0);
    }

    #[test]
    fn anchor_biases_toward_main_content() {
        let pointer = PointerState::default();
        let orientation = scene_orientation(4.0, pointer, false);
        let anchor = scene_anchor(
            BlackHoleConfig {
                radius: 120.0,
                width: 1440.0,
                height: 900.0,
                reduced_motion: false,
            },
            4.0,
            pointer,
            orientation,
        );
        assert!(anchor.x > 720.0);
        assert!(anchor.y > 450.0);
    }

    #[test]
    fn taa_sequence_is_stable_and_centered() {
        let (x1, y1) = taa_jitter(0, 1.0);
        let (x2, y2) = taa_jitter(16, 1.0);
        assert_eq!((x1, y1), (x2, y2));
        assert!(x1.abs() <= 0.5);
        assert!(y1.abs() <= 0.5);
    }
}
