use gtk::glib;

use std::time::Duration;

/// A boxed [`Duration`]
#[derive(Debug, Default, Clone, Copy, glib::Boxed)]
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
        Self(value.into())
    }
}

impl From<ClockTime> for gst::ClockTime {
    fn from(value: ClockTime) -> Self {
        let nanos = value.0.as_nanos();

        // Note: `std::u64::MAX` is `ClockTime::None`.
        if nanos >= std::u64::MAX as u128 {
            return gst::ClockTime::from_nseconds(std::u64::MAX - 1);
        }

        gst::ClockTime::from_nseconds(nanos as u64)
    }
}
