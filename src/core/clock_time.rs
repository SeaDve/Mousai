use gtk::glib;

use std::time::Duration;

/// A boxed [`Duration`]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "MsaiClockTime", nullable)]
pub struct ClockTime(Duration);

impl ClockTime {
    pub const ZERO: Self = Self(Duration::ZERO);

    pub fn from_secs_f64(secs: f64) -> Self {
        Self(Duration::from_secs_f64(secs))
    }

    pub const fn from_secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    pub const fn from_micros(micros: u64) -> Self {
        Self(Duration::from_micros(micros))
    }

    pub fn as_secs_f64(&self) -> f64 {
        self.0.as_secs_f64()
    }

    pub const fn as_secs(&self) -> u64 {
        self.0.as_secs()
    }

    pub const fn as_micros(&self) -> u128 {
        self.0.as_micros()
    }
}

impl From<gst::ClockTime> for ClockTime {
    fn from(value: gst::ClockTime) -> Self {
        Self(Duration::from(value))
    }
}

impl From<ClockTime> for gst::ClockTime {
    fn from(value: ClockTime) -> Self {
        let nanos = value.0.as_nanos();

        // Note: `u64::MAX` is `ClockTime::None`.
        if nanos >= u64::MAX as u128 {
            return gst::ClockTime::MAX;
        }

        gst::ClockTime::from_nseconds(nanos as u64)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! assert_eq_gst {
        ($this:expr, $gst:expr) => {
            assert_eq!($this.0.as_nanos(), $gst.nseconds() as u128);
        };
    }

    #[test]
    fn zero() {
        assert_eq_gst!(ClockTime::ZERO, gst::ClockTime::ZERO);
    }

    #[test]
    fn gst_conversion() {
        let std = Duration::from_nanos(123);
        let this = ClockTime(std);
        let gst = gst::ClockTime::try_from(std).unwrap();

        assert_eq_gst!(this, gst);
        assert_eq_gst!(ClockTime::from(gst), gst::ClockTime::from(this));
    }

    #[test]
    fn gst_conversion_max_handling() {
        let this_max = ClockTime(Duration::from_nanos(u64::MAX));
        let gst_from_this_max = gst::ClockTime::from(this_max);

        assert_eq!(gst::ClockTime::MAX, gst_from_this_max);
        assert_eq_gst!(
            ClockTime(Duration::from_nanos(u64::MAX - 1)),
            gst_from_this_max
        );
    }
}
