use serde::{Deserialize, Serialize};

/// The legal values of a control as reported by the camera driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawRange")]
pub struct ControlRange {
    min: i32,
    max: i32,
    step: i32,
    default: i32,
}

#[derive(Deserialize)]
struct RawRange {
    min: i32,
    max: i32,
    step: i32,
    default: i32,
}

impl TryFrom<RawRange> for ControlRange {
    type Error = RangeError;

    fn try_from(raw: RawRange) -> Result<Self, Self::Error> {
        Self::new(raw.min, raw.max, raw.step, raw.default)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    #[error("minimum {min} is greater than maximum {max}")]
    Inverted { min: i32, max: i32 },
    #[error("step must be positive, got {0}")]
    NonPositiveStep(i32),
}

impl ControlRange {
    /// Builds a range, accepting driver defaults that fall outside the range or off the step grid
    /// by snapping them, because many UVC firmwares report such values.
    pub fn new(min: i32, max: i32, step: i32, default: i32) -> Result<Self, RangeError> {
        if min > max {
            return Err(RangeError::Inverted { min, max });
        }
        if step <= 0 {
            return Err(RangeError::NonPositiveStep(step));
        }
        let mut range = Self { min, max, step, default: min };
        range.default = range.snap(default);
        Ok(range)
    }

    #[must_use]
    pub const fn min(&self) -> i32 {
        self.min
    }

    #[must_use]
    pub const fn max(&self) -> i32 {
        self.max
    }

    #[must_use]
    pub const fn step(&self) -> i32 {
        self.step
    }

    #[must_use]
    pub const fn default(&self) -> i32 {
        self.default
    }

    /// Clamps `value` into the range and rounds it to the nearest step from `min`.
    #[must_use]
    pub fn snap(&self, value: i32) -> i32 {
        let clamped = i64::from(value.clamp(self.min, self.max));
        let min = i64::from(self.min);
        let step = i64::from(self.step);
        let steps = (clamped - min + step / 2) / step;
        let snapped = (min + steps * step).min(i64::from(self.highest_reachable()));
        i32::try_from(snapped).expect("snapped value stays within i32 bounds")
    }

    #[must_use]
    pub fn contains(&self, value: i32) -> bool {
        self.snap(value) == value
    }

    /// Position of `value` on the slider track, from 0.0 to 1.0.
    #[must_use]
    pub fn ratio(&self, value: i32) -> f32 {
        if self.min == self.max {
            return 0.0;
        }
        let span = f64::from(self.max) - f64::from(self.min);
        #[allow(clippy::cast_possible_truncation)]
        let ratio = ((f64::from(self.snap(value)) - f64::from(self.min)) / span) as f32;
        ratio
    }

    /// The value at a slider position from 0.0 to 1.0, snapped to the step grid.
    #[must_use]
    pub fn value_at(&self, ratio: f32) -> i32 {
        let ratio = f64::from(ratio.clamp(0.0, 1.0));
        let span = f64::from(self.max) - f64::from(self.min);
        #[allow(clippy::cast_possible_truncation)]
        let raw = (f64::from(self.min) + span * ratio).round() as i64;
        self.snap(i32::try_from(raw).expect("interpolated value stays within the range"))
    }

    /// Moves `value` by `steps` increments, saturating at the range bounds.
    #[must_use]
    pub fn nudge(&self, value: i32, steps: i32) -> i32 {
        let target = i64::from(self.snap(value)) + i64::from(steps) * i64::from(self.step);
        let clamped = target.clamp(i64::from(self.min), i64::from(self.max));
        self.snap(i32::try_from(clamped).expect("clamped value fits in i32"))
    }

    fn highest_reachable(&self) -> i32 {
        let min = i64::from(self.min);
        let step = i64::from(self.step);
        let reachable = min + (i64::from(self.max) - min) / step * step;
        i32::try_from(reachable).expect("reachable maximum stays within i32 bounds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: i32, max: i32, step: i32, default: i32) -> ControlRange {
        ControlRange::new(min, max, step, default).unwrap()
    }

    #[test]
    fn rejects_invalid_ranges() {
        assert_eq!(ControlRange::new(10, 0, 1, 5), Err(RangeError::Inverted { min: 10, max: 0 }));
        assert_eq!(ControlRange::new(0, 10, 0, 5), Err(RangeError::NonPositiveStep(0)));
        assert_eq!(ControlRange::new(0, 10, -2, 5), Err(RangeError::NonPositiveStep(-2)));
        assert_eq!(
            RangeError::Inverted { min: 10, max: 0 }.to_string(),
            "minimum 10 is greater than maximum 0"
        );
        assert_eq!(RangeError::NonPositiveStep(0).to_string(), "step must be positive, got 0");
    }

    #[test]
    fn exposes_its_bounds() {
        let r = range(-64, 64, 1, 0);
        assert_eq!((r.min(), r.max(), r.step(), r.default()), (-64, 64, 1, 0));
    }

    #[test]
    fn snaps_out_of_range_defaults() {
        assert_eq!(range(0, 255, 1, 300).default(), 255);
        assert_eq!(range(2800, 6500, 10, 4005).default(), 4010);
    }

    #[test]
    fn snap_clamps_and_rounds_to_step() {
        let r = range(2800, 6500, 10, 4600);
        assert_eq!(r.snap(1000), 2800);
        assert_eq!(r.snap(9000), 6500);
        assert_eq!(r.snap(4604), 4600);
        assert_eq!(r.snap(4605), 4610);
    }

    #[test]
    fn snap_never_exceeds_the_last_reachable_step() {
        let r = range(0, 10, 4, 0);
        assert_eq!(r.snap(10), 8);
        assert_eq!(r.snap(7), 8);
        assert!(!r.contains(10));
        assert!(r.contains(8));
    }

    #[test]
    fn snap_handles_extreme_bounds() {
        let r = range(i32::MIN, i32::MAX, 1, 0);
        assert_eq!(r.snap(i32::MIN), i32::MIN);
        assert_eq!(r.snap(i32::MAX), i32::MAX);
        let coarse = range(i32::MIN, i32::MAX, i32::MAX, 0);
        assert_eq!(coarse.snap(i32::MAX), i32::MAX - 1);
    }

    #[test]
    fn ratio_maps_values_onto_the_track() {
        let r = range(0, 200, 1, 100);
        assert!((r.ratio(0) - 0.0).abs() < f32::EPSILON);
        assert!((r.ratio(100) - 0.5).abs() < f32::EPSILON);
        assert!((r.ratio(500) - 1.0).abs() < f32::EPSILON);
        assert!((range(5, 5, 1, 5).ratio(5)).abs() < f32::EPSILON);
    }

    #[test]
    fn value_at_inverts_ratio() {
        let r = range(-7, 7, 1, 0);
        assert_eq!(r.value_at(0.0), -7);
        assert_eq!(r.value_at(0.5), 0);
        assert_eq!(r.value_at(1.0), 7);
        assert_eq!(r.value_at(-3.0), -7);
        assert_eq!(r.value_at(9.0), 7);
        for value in -7..=7 {
            assert_eq!(r.value_at(r.ratio(value)), value);
        }
    }

    #[test]
    fn nudge_moves_by_steps_and_saturates() {
        let r = range(0, 100, 5, 50);
        assert_eq!(r.nudge(50, 1), 55);
        assert_eq!(r.nudge(50, -3), 35);
        assert_eq!(r.nudge(98, 10), 100);
        assert_eq!(r.nudge(2, -10), 0);
        assert_eq!(r.nudge(52, 0), 50);
    }

    #[test]
    fn serializes_as_plain_table() {
        let r = range(0, 10, 2, 4);
        let encoded = toml::to_string(&r).unwrap();
        assert_eq!(toml::from_str::<ControlRange>(&encoded).unwrap(), r);
    }

    #[test]
    fn deserialization_validates() {
        let err = toml::from_str::<ControlRange>("min = 5\nmax = 1\nstep = 1\ndefault = 2").unwrap_err();
        assert!(err.to_string().contains("minimum 5 is greater than maximum 1"));
    }
}
