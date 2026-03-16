use std::collections::HashSet;

use time::OffsetDateTime;

use crate::{
    config::AppConfig,
    status::{PromotionState, status_at},
};

/// Key: (transition_unix_timestamp, threshold_minutes).
/// Prevents the same notification from firing more than once.
type FiredKey = (i64, u32);

pub struct NotificationEngine {
    config: AppConfig,
    fired: HashSet<FiredKey>,
    last_transition_unix: Option<i64>,
}

impl NotificationEngine {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            fired: HashSet::new(),
            last_transition_unix: None,
        }
    }

    pub fn update_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    pub fn check_and_notify(&mut self, now_utc: OffsetDateTime) {
        if !self.config.notifications.enabled {
            return;
        }

        let snapshot = status_at(now_utc);

        let transition_utc = match snapshot.next_transition_utc {
            Some(t) => t,
            None => return,
        };

        let transition_unix = transition_utc.unix_timestamp();

        // Reset fired set when the transition target changes.
        if self.last_transition_unix != Some(transition_unix) {
            self.fired.clear();
            self.last_transition_unix = Some(transition_unix);
        }

        let seconds_until = (transition_utc - now_utc).whole_seconds().max(0);
        let minutes_until = (seconds_until + 59) / 60; // ceiling

        let (thresholds, title, body_template) = match snapshot.state {
            PromotionState::TwoX => (
                &self.config.notifications.before_end_minutes,
                "Claude 2x Ending Soon",
                "2x rate ends in about {} minutes",
            ),
            PromotionState::Standard | PromotionState::BeforeStart => (
                &self.config.notifications.before_start_minutes,
                "Claude 2x Starting Soon",
                "2x rate starts in about {} minutes",
            ),
            PromotionState::Ended => return,
        };

        for &threshold in thresholds {
            let key: FiredKey = (transition_unix, threshold);
            if minutes_until <= i64::from(threshold) && !self.fired.contains(&key) {
                self.fired.insert(key);
                let body = body_template.replace("{}", &threshold.to_string());
                fire_toast(title, &body, self.config.notifications.sound);
            }
        }
    }
}

#[cfg(windows)]
fn fire_toast(title: &str, body: &str, with_sound: bool) {
    use winrt_notification::{Duration as ToastDuration, Sound, Toast};

    let sound = if with_sound { Some(Sound::SMS) } else { None };

    let result = Toast::new(Toast::POWERSHELL_APP_ID)
        .title(title)
        .text1(body)
        .duration(ToastDuration::Short)
        .sound(sound)
        .show();

    if let Err(e) = result {
        eprintln!("failed to show notification: {e}");
    }
}

#[cfg(not(windows))]
fn fire_toast(title: &str, body: &str, _with_sound: bool) {
    eprintln!("[notification] {title}: {body}");
}
