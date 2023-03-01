pub trait ClockTimeExt {
    /// Creates a new `ClockTime` from a f64 of seconds.
    ///
    /// Note: This returns a `ClockTime::ZERO` if the value is <= 0
    /// or `ClockTime::MAX` if the computed value is equal to `u64::MAX`.
    fn from_seconds_f64(secs: f64) -> Self;

    /// Converts a `ClockTime` to a f64 of seconds.
    fn seconds_f64(self) -> f64;

    /// Format into a `MM∶SS` string with padding for SS.
    fn to_minute_sec_str(self) -> String;
}

impl ClockTimeExt for gst::ClockTime {
    fn from_seconds_f64(seconds: f64) -> Self {
        let nseconds = (seconds * (Self::SECOND.nseconds() as f64)).round() as u64;

        // Note: `u64::MAX` is `ClockTime::None`.
        if nseconds == u64::MAX {
            return Self::MAX;
        }

        Self::from_nseconds(nseconds)
    }

    fn seconds_f64(self) -> f64 {
        (self.nseconds() as f64) / (Self::SECOND.nseconds() as f64)
    }

    fn to_minute_sec_str(self) -> String {
        let seconds = self.seconds();

        let minutes_display = seconds / 60;
        let seconds_display = seconds % 60;
        format!("{}∶{:02}", minutes_display, seconds_display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seconds_f64() {
        let res = gst::ClockTime::from_seconds_f64(-2.0);
        assert_eq!(res, gst::ClockTime::ZERO);
        let res = gst::ClockTime::from_seconds_f64(0.0);
        assert_eq!(res, gst::ClockTime::ZERO);
        let res = gst::ClockTime::from_seconds_f64(1e-20);
        assert_eq!(res, gst::ClockTime::ZERO);
        let res = gst::ClockTime::from_seconds_f64(4.2e-7);
        assert_eq!(res, gst::ClockTime::from_nseconds(420));
        let res = gst::ClockTime::from_seconds_f64(2.7);
        assert_eq!(res, gst::ClockTime::from_nseconds(2_700_000_000));
        let res = gst::ClockTime::from_seconds_f64((u64::MAX - 1) as f64);
        assert_eq!(res, gst::ClockTime::MAX);
        let res = gst::ClockTime::from_seconds_f64(f64::MAX);
        assert_eq!(res, gst::ClockTime::MAX);

        // this float represents exactly 976562.5e-9
        let val = f64::from_bits(0x3F50_0000_0000_0000);
        let res = gst::ClockTime::from_seconds_f64(val);
        assert_eq!(res, gst::ClockTime::from_nseconds(976_563));

        // this float represents exactly 2929687.5e-9
        let val = f64::from_bits(0x3F68_0000_0000_0000);
        let res = gst::ClockTime::from_seconds_f64(val);
        assert_eq!(res, gst::ClockTime::from_nseconds(2_929_688));

        // this float represents exactly 1.000_976_562_5
        let val = f64::from_bits(0x3FF0_0400_0000_0000);
        let res = gst::ClockTime::from_seconds_f64(val);
        assert_eq!(res, gst::ClockTime::from_nseconds(1_000_976_563));

        // this float represents exactly 1.002_929_687_5
        let val = f64::from_bits(0x3FF0_0C00_0000_0000);
        let res = gst::ClockTime::from_seconds_f64(val);
        assert_eq!(res, gst::ClockTime::from_nseconds(1_002_929_688));
    }

    #[test]
    fn seconds_f64() {
        let ct = gst::ClockTime::from_nseconds(0);
        assert_eq!(ct.seconds_f64(), 0.0);

        let ct = gst::ClockTime::from_nseconds(2_700_000_000);
        assert_eq!(ct.seconds_f64(), 2.7);

        let ct = gst::ClockTime::MAX;
        assert_eq!(ct.seconds_f64(), 18_446_744_073.709_553);
    }

    #[test]
    fn to_minute_sec_str() {
        assert_eq!(gst::ClockTime::ZERO.to_minute_sec_str(), "0∶00");
        assert_eq!(gst::ClockTime::from_seconds(31).to_minute_sec_str(), "0∶31");
        assert_eq!(
            gst::ClockTime::from_seconds(59 * 60 + 59).to_minute_sec_str(),
            "59∶59"
        );

        assert_eq!(
            gst::ClockTime::from_seconds(60 * 60).to_minute_sec_str(),
            "60∶00"
        );
        assert_eq!(
            gst::ClockTime::from_seconds(100 * 60 + 20).to_minute_sec_str(),
            "100∶20"
        );
        assert_eq!(gst::ClockTime::MAX.to_minute_sec_str(), "307445734∶33");
    }
}
