use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

struct IdleState {
    proxy: EventLoopProxy<UserEvent>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for IdleState {
    fn event(
        _state: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(IdleState: ignore wl_seat::WlSeat);
delegate_noop!(IdleState: ignore ExtIdleNotifierV1);

impl Dispatch<ExtIdleNotificationV1, ()> for IdleState {
    fn event(
        state: &mut Self,
        _obj: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => {
                let _ = state.proxy.send_event(UserEvent::Idle);
            }
            ext_idle_notification_v1::Event::Resumed => {
                let _ = state.proxy.send_event(UserEvent::Resumed);
            }
            _ => {}
        }
    }
}

pub fn start(timeout_secs: u32, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let conn =
            Connection::connect_to_env().expect("failed to connect to Wayland for idle notify");
        let (globals, mut event_queue) =
            registry_queue_init::<IdleState>(&conn).expect("failed to init Wayland registry");
        let qh = event_queue.handle();

        let seat: wl_seat::WlSeat = globals
            .bind(&qh, 1..=9, ())
            .expect("compositor missing wl_seat");
        let notifier: ExtIdleNotifierV1 = globals
            .bind(&qh, 1..=1, ())
            .expect("compositor missing ext_idle_notifier_v1");

        let _notification = notifier.get_idle_notification(timeout_secs * 1000, &seat, &qh, ());

        let mut state = IdleState { proxy };
        loop {
            event_queue
                .blocking_dispatch(&mut state)
                .expect("Wayland idle dispatch failed");
        }
    });
}
