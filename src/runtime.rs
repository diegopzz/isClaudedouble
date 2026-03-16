use std::{
    sync::{
        Arc,
        atomic::AtomicBool,
    },
    thread,
    time::Duration as StdDuration,
};

use anyhow::{Context, Result};
#[cfg(target_os = "macos")]
use tray_icon::menu::Submenu;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::WindowId,
};

use crate::{
    config::AppConfig,
    icon::{IconImage, active_icon, inactive_icon},
    notify::NotificationEngine,
    popup,
    status::{PromotionState, active_window_ends_at, next_two_x_starts_at, status_at},
};

const STATUS_MENU_ID: &str = "status";
const TIME_LEFT_MENU_ID: &str = "time-left";
const NEXT_TWO_X_MENU_ID: &str = "next-two-x";
const QUIT_MENU_ID: &str = "quit";
const TOOLTIP_ACTIVE: &str = "Claude 2x: active";
const TOOLTIP_INACTIVE: &str = "Claude 2x: inactive";

#[derive(Debug, Clone)]
pub enum UserEvent {
    Tick,
    Menu(MenuEvent),
    TrayClick,
}

pub fn run() -> Result<()> {
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    let event_loop = builder.build().context("failed to create event loop")?;
    let proxy = event_loop.create_proxy();

    install_menu_event_bridge(proxy.clone());
    install_tray_click_bridge(proxy.clone());
    start_ticker(proxy);

    let config = AppConfig::load().unwrap_or_default();
    let mut app = TrayApplication::new(config)?;
    event_loop
        .run_app(&mut app)
        .context("failed to run tray event loop")
}

fn build_tray_icon(source: IconImage) -> Result<Icon> {
    Icon::from_rgba(source.rgba, source.width, source.height)
        .context("failed to convert RGBA data into tray icon")
}

fn install_menu_event_bridge(proxy: EventLoopProxy<UserEvent>) {
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));
}

fn install_tray_click_bridge(proxy: EventLoopProxy<UserEvent>) {
    TrayIconEvent::set_event_handler(Some(move |event| {
        if let TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        } = event
        {
            let _ = proxy.send_event(UserEvent::TrayClick);
        }
    }));
}

fn start_ticker(proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        loop {
            thread::sleep(StdDuration::from_secs(30));
            if proxy.send_event(UserEvent::Tick).is_err() {
                break;
            }
        }
    });
}

struct TrayApplication {
    tray_icon: Option<TrayIcon>,
    status_item: Option<MenuItem>,
    time_left_item: Option<MenuItem>,
    next_two_x_item: Option<MenuItem>,
    active_icon: Icon,
    inactive_icon: Icon,
    current_state: Option<PromotionState>,
    notification_engine: NotificationEngine,
    popup_open: Arc<AtomicBool>,
}

impl TrayApplication {
    fn new(config: AppConfig) -> Result<Self> {
        Ok(Self {
            tray_icon: None,
            status_item: None,
            time_left_item: None,
            next_two_x_item: None,
            active_icon: build_tray_icon(active_icon())?,
            inactive_icon: build_tray_icon(inactive_icon())?,
            current_state: None,
            notification_engine: NotificationEngine::new(config),
            popup_open: Arc::new(AtomicBool::new(false)),
        })
    }

    fn initialize_tray(&mut self) -> Result<()> {
        let now_utc = time::OffsetDateTime::now_utc();
        let initial_snapshot = status_at(now_utc);
        let (menu, status_item, time_left_item, next_two_x_item) =
            build_menu(now_utc, initial_snapshot.state).context("failed to build tray menu")?;
        let initial_icon = self.icon_for_state(initial_snapshot.state);
        let tooltip = tooltip_for_state(initial_snapshot.state);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(initial_icon)
            .with_tooltip(tooltip)
            .with_icon_as_template(false)
            .with_menu_on_left_click(false)
            .build()
            .context("failed to create tray icon")?;

        self.current_state = Some(initial_snapshot.state);
        self.status_item = Some(status_item);
        self.time_left_item = Some(time_left_item);
        self.next_two_x_item = Some(next_two_x_item);
        self.tray_icon = Some(tray_icon);
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        let now_utc = time::OffsetDateTime::now_utc();
        let snapshot = status_at(now_utc);
        let tray_icon = self
            .tray_icon
            .as_ref()
            .context("tray icon not initialized")?;
        let status_item = self
            .status_item
            .as_ref()
            .context("status menu item not initialized")?;
        let time_left_item = self
            .time_left_item
            .as_ref()
            .context("time-left menu item not initialized")?;
        let next_two_x_item = self
            .next_two_x_item
            .as_ref()
            .context("next-2x menu item not initialized")?;

        let next_state = snapshot.state;
        if self.current_state != Some(next_state) {
            tray_icon
                .set_icon(Some(self.icon_for_state(next_state)))
                .context("failed to update tray icon")?;
            self.current_state = Some(next_state);
        }

        tray_icon
            .set_tooltip(Some(tooltip_for_state(next_state)))
            .context("failed to update tray tooltip")?;
        status_item.set_text(status_label(next_state));
        time_left_item.set_text(time_left_label(now_utc, next_state));
        next_two_x_item.set_text(next_two_x_label(now_utc, next_state));

        // Pick up any config changes from the popup subprocess.
        if let Ok(latest_config) = AppConfig::load() {
            self.notification_engine.update_config(latest_config);
        }

        // Fire notifications if thresholds are hit.
        self.notification_engine.check_and_notify(now_utc);

        Ok(())
    }

    fn handle_user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tick => {
                if let Err(error) = self.refresh() {
                    eprintln!("refresh error: {error:#}");
                    event_loop.exit();
                }
            }
            UserEvent::Menu(event) if event.id.as_ref() == QUIT_MENU_ID => {
                event_loop.exit();
            }
            UserEvent::Menu(_) => {}
            UserEvent::TrayClick => {
                popup::show_popup(Arc::clone(&self.popup_open));
            }
        }
    }

    fn icon_for_state(&self, state: PromotionState) -> Icon {
        match state {
            PromotionState::TwoX => self.active_icon.clone(),
            PromotionState::BeforeStart | PromotionState::Standard | PromotionState::Ended => {
                self.inactive_icon.clone()
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for TrayApplication {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Wait);

        if self.tray_icon.is_some() {
            return;
        }

        if let Err(error) = self.initialize_tray().and_then(|_| self.refresh()) {
            eprintln!("startup error: {error:#}");
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        self.handle_user_event(event_loop, event);
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}

fn build_menu(
    now_utc: time::OffsetDateTime,
    initial_state: PromotionState,
) -> Result<(Menu, MenuItem, MenuItem, MenuItem)> {
    let status_item = MenuItem::with_id(STATUS_MENU_ID, status_label(initial_state), false, None);
    let time_left_item = MenuItem::with_id(
        TIME_LEFT_MENU_ID,
        time_left_label(now_utc, initial_state),
        false,
        None,
    );
    let next_two_x_item = MenuItem::with_id(
        NEXT_TWO_X_MENU_ID,
        next_two_x_label(now_utc, initial_state),
        false,
        None,
    );
    let quit_item = MenuItem::with_id(QUIT_MENU_ID, "Quit", true, None);

    #[cfg(target_os = "macos")]
    {
        let root_menu = Menu::new();
        let submenu = Submenu::with_items(
            "Claude 2x",
            true,
            &[&status_item, &time_left_item, &next_two_x_item, &quit_item],
        )
        .context("failed to build macOS tray submenu")?;
        root_menu
            .append(&submenu)
            .context("failed to append macOS tray submenu")?;
        Ok((root_menu, status_item, time_left_item, next_two_x_item))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let menu = Menu::with_items(&[&status_item, &time_left_item, &next_two_x_item, &quit_item])
            .context("failed to build tray menu")?;
        Ok((menu, status_item, time_left_item, next_two_x_item))
    }
}

fn tooltip_for_state(state: PromotionState) -> &'static str {
    match state {
        PromotionState::TwoX => TOOLTIP_ACTIVE,
        PromotionState::BeforeStart | PromotionState::Standard | PromotionState::Ended => {
            TOOLTIP_INACTIVE
        }
    }
}

fn status_label(state: PromotionState) -> &'static str {
    match state {
        PromotionState::TwoX => "Status: active",
        PromotionState::BeforeStart | PromotionState::Standard | PromotionState::Ended => {
            "Status: inactive"
        }
    }
}

fn time_left_label(now_utc: time::OffsetDateTime, state: PromotionState) -> String {
    match state {
        PromotionState::TwoX => active_window_ends_at(now_utc)
            .map(|ends_at| format!("2x left: {}", format_duration(ends_at - now_utc)))
            .unwrap_or_else(|| "2x left: unknown".to_string()),
        PromotionState::BeforeStart | PromotionState::Standard => next_two_x_starts_at(now_utc)
            .map(|starts_at| format!("2x starts in: {}", format_duration(starts_at - now_utc)))
            .unwrap_or_else(|| "2x starts in: unavailable".to_string()),
        PromotionState::Ended => "2x left: promotion ended".to_string(),
    }
}

fn next_two_x_label(now_utc: time::OffsetDateTime, state: PromotionState) -> String {
    match state {
        PromotionState::TwoX => next_two_x_starts_at(now_utc)
            .map(|starts_at| format!("2x again: {}", format_local_timestamp(starts_at)))
            .unwrap_or_else(|| "2x again: active until promo end".to_string()),
        PromotionState::BeforeStart | PromotionState::Standard => next_two_x_starts_at(now_utc)
            .map(|starts_at| format!("2x again: {}", format_local_timestamp(starts_at)))
            .unwrap_or_else(|| "2x again: unavailable".to_string()),
        PromotionState::Ended => "2x again: promotion ended".to_string(),
    }
}

fn format_duration(duration: time::Duration) -> String {
    let total_seconds = duration.whole_seconds().max(0);
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {minutes:02}m")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

fn format_local_timestamp(timestamp_utc: time::OffsetDateTime) -> String {
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local_timestamp = timestamp_utc.to_offset(local_offset);
    let month = match local_timestamp.month() {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    };
    let hour_24 = local_timestamp.hour();
    let minute = local_timestamp.minute();
    let (hour_12, meridiem) = match hour_24 {
        0 => (12, "AM"),
        1..=11 => (hour_24, "AM"),
        12 => (12, "PM"),
        _ => (hour_24 - 12, "PM"),
    };

    format!(
        "{month} {} {:02}:{:02} {meridiem}",
        local_timestamp.day(),
        hour_12,
        minute
    )
}
