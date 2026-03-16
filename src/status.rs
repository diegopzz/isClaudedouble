use time::{Date, Duration, OffsetDateTime, PrimitiveDateTime, UtcOffset, Weekday};

const PROMO_START_UNIX: i64 = 1_773_374_400;
const PROMO_END_UNIX: i64 = 1_774_670_400;
const PEAK_START_HOUR_UTC: u8 = 12;
const PEAK_END_HOUR_UTC: u8 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionState {
    BeforeStart,
    TwoX,
    Standard,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub state: PromotionState,
    pub is_active: bool,
    pub next_transition_utc: Option<OffsetDateTime>,
}

pub fn status_at(now_utc: OffsetDateTime) -> StatusSnapshot {
    let now_utc = now_utc.to_offset(UtcOffset::UTC);
    let state = classify(now_utc);
    let next_transition_utc = next_transition_for(now_utc, state);

    StatusSnapshot {
        state,
        is_active: matches!(state, PromotionState::TwoX),
        next_transition_utc,
    }
}

pub fn is_weekend_eastern(now_utc: OffsetDateTime) -> bool {
    matches!(
        now_utc.to_offset(eastern_offset()).weekday(),
        Weekday::Saturday | Weekday::Sunday
    )
}

pub fn active_window_ends_at(now_utc: OffsetDateTime) -> Option<OffsetDateTime> {
    let snapshot = status_at(now_utc);
    snapshot
        .is_active
        .then_some(snapshot.next_transition_utc)
        .flatten()
}

pub fn next_two_x_starts_at(now_utc: OffsetDateTime) -> Option<OffsetDateTime> {
    let now_utc = now_utc.to_offset(UtcOffset::UTC);

    match classify(now_utc) {
        PromotionState::BeforeStart => Some(promo_start()),
        PromotionState::Standard => Some(cap_at_promo_end(peak_end_for_day(now_utc))),
        PromotionState::TwoX => {
            let active_end = cap_at_promo_end(find_next_weekday_peak_start(now_utc));
            if active_end >= promo_end() {
                None
            } else {
                Some(cap_at_promo_end(peak_end_for_day(active_end)))
            }
        }
        PromotionState::Ended => None,
    }
}

fn classify(now_utc: OffsetDateTime) -> PromotionState {
    let unix = now_utc.unix_timestamp();

    if unix <= PROMO_START_UNIX {
        return PromotionState::BeforeStart;
    }

    if unix >= PROMO_END_UNIX {
        return PromotionState::Ended;
    }

    if is_weekend_eastern(now_utc) || !is_peak_hour(now_utc) {
        PromotionState::TwoX
    } else {
        PromotionState::Standard
    }
}

fn next_transition_for(now_utc: OffsetDateTime, state: PromotionState) -> Option<OffsetDateTime> {
    match state {
        PromotionState::BeforeStart => Some(promo_start()),
        PromotionState::Ended => None,
        PromotionState::Standard => Some(cap_at_promo_end(peak_end_for_day(now_utc))),
        PromotionState::TwoX => Some(cap_at_promo_end(find_next_weekday_peak_start(now_utc))),
    }
}

fn cap_at_promo_end(candidate: OffsetDateTime) -> OffsetDateTime {
    candidate.min(promo_end())
}

fn is_peak_hour(now_utc: OffsetDateTime) -> bool {
    let hour = now_utc.hour();
    (PEAK_START_HOUR_UTC..PEAK_END_HOUR_UTC).contains(&hour)
}

fn find_next_weekday_peak_start(now_utc: OffsetDateTime) -> OffsetDateTime {
    let base = peak_start_for_day(now_utc);

    for day_offset in 0..=7 {
        let candidate = base + Duration::days(day_offset);
        if candidate > now_utc && !is_weekend_eastern(candidate) {
            return candidate;
        }
    }

    promo_end()
}

fn peak_start_for_day(now_utc: OffsetDateTime) -> OffsetDateTime {
    utc_datetime(now_utc.date(), PEAK_START_HOUR_UTC)
}

fn peak_end_for_day(now_utc: OffsetDateTime) -> OffsetDateTime {
    utc_datetime(now_utc.date(), PEAK_END_HOUR_UTC)
}

fn utc_datetime(date: Date, hour: u8) -> OffsetDateTime {
    PrimitiveDateTime::new(date, time::Time::from_hms(hour, 0, 0).expect("valid hour")).assume_utc()
}

fn promo_start() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(PROMO_START_UNIX).expect("valid promo start")
}

fn promo_end() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(PROMO_END_UNIX).expect("valid promo end")
}

fn eastern_offset() -> UtcOffset {
    UtcOffset::from_hms(-4, 0, 0).expect("valid EDT offset")
}

#[cfg(test)]
mod tests {
    use super::{
        PromotionState, active_window_ends_at, is_weekend_eastern, next_two_x_starts_at, status_at,
    };
    use time::macros::datetime;

    #[test]
    fn inactive_before_promo_start() {
        let snapshot = status_at(datetime!(2026-03-13 03:59:59 UTC));

        assert_eq!(snapshot.state, PromotionState::BeforeStart);
        assert!(!snapshot.is_active);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-13 04:00:00 UTC))
        );
    }

    #[test]
    fn friday_off_peak_is_active() {
        let snapshot = status_at(datetime!(2026-03-13 11:59:59 UTC));

        assert_eq!(snapshot.state, PromotionState::TwoX);
        assert!(snapshot.is_active);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-13 12:00:00 UTC))
        );
    }

    #[test]
    fn exact_peak_start_is_inactive() {
        let snapshot = status_at(datetime!(2026-03-13 12:00:00 UTC));

        assert_eq!(snapshot.state, PromotionState::Standard);
        assert!(!snapshot.is_active);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-13 18:00:00 UTC))
        );
    }

    #[test]
    fn exact_peak_end_is_active() {
        let snapshot = status_at(datetime!(2026-03-13 18:00:00 UTC));

        assert_eq!(snapshot.state, PromotionState::TwoX);
        assert!(snapshot.is_active);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-16 12:00:00 UTC))
        );
    }

    #[test]
    fn weekend_is_always_active_during_promo() {
        let snapshot = status_at(datetime!(2026-03-14 15:30:00 UTC));

        assert_eq!(snapshot.state, PromotionState::TwoX);
        assert!(snapshot.is_active);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-16 12:00:00 UTC))
        );
    }

    #[test]
    fn inactive_at_and_after_promo_end() {
        let snapshot = status_at(datetime!(2026-03-28 04:00:00 UTC));

        assert_eq!(snapshot.state, PromotionState::Ended);
        assert!(!snapshot.is_active);
        assert_eq!(snapshot.next_transition_utc, None);
    }

    #[test]
    fn eastern_weekend_boundary_matches_fixed_offset() {
        assert!(!is_weekend_eastern(datetime!(2026-03-14 03:59:59 UTC)));
        assert!(is_weekend_eastern(datetime!(2026-03-14 04:00:00 UTC)));
        assert!(is_weekend_eastern(datetime!(2026-03-16 03:59:59 UTC)));
        assert!(!is_weekend_eastern(datetime!(2026-03-16 04:00:00 UTC)));
    }

    #[test]
    fn standard_hours_transition_same_day() {
        let snapshot = status_at(datetime!(2026-03-17 15:00:00 UTC));

        assert_eq!(snapshot.state, PromotionState::Standard);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-17 18:00:00 UTC))
        );
    }

    #[test]
    fn final_active_period_transitions_to_promo_end() {
        let snapshot = status_at(datetime!(2026-03-28 03:59:59 UTC));

        assert_eq!(snapshot.state, PromotionState::TwoX);
        assert_eq!(
            snapshot.next_transition_utc,
            Some(datetime!(2026-03-28 04:00:00 UTC))
        );
    }

    #[test]
    fn promo_window_uses_exact_same_start_boundary_as_site_script() {
        let snapshot = status_at(datetime!(2026-03-13 04:00:00 UTC));

        assert_eq!(snapshot.state, PromotionState::BeforeStart);
        assert!(!snapshot.is_active);
    }

    #[test]
    fn active_window_end_matches_next_transition_when_two_x_is_live() {
        assert_eq!(
            active_window_ends_at(datetime!(2026-03-13 11:59:59 UTC)),
            Some(datetime!(2026-03-13 12:00:00 UTC))
        );
    }

    #[test]
    fn next_two_x_start_after_standard_hours_is_same_day_peak_end() {
        assert_eq!(
            next_two_x_starts_at(datetime!(2026-03-17 15:00:00 UTC)),
            Some(datetime!(2026-03-17 18:00:00 UTC))
        );
    }

    #[test]
    fn next_two_x_start_while_active_skips_to_after_the_next_standard_window() {
        assert_eq!(
            next_two_x_starts_at(datetime!(2026-03-13 11:59:59 UTC)),
            Some(datetime!(2026-03-13 18:00:00 UTC))
        );
    }

    #[test]
    fn next_two_x_start_is_none_after_promo_end() {
        assert_eq!(
            next_two_x_starts_at(datetime!(2026-03-28 04:00:00 UTC)),
            None
        );
    }
}
