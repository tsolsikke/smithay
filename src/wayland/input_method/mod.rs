//! Utilities for input method support
//!
//! This module provides you with utilities to handle input methods,
//! it must be used in conjunction with the text input module to work.
//!
//! ```
//! use smithay::{
//!     delegate_seat, delegate_input_method_manager, delegate_text_input_manager,
//! #   delegate_compositor,
//! };
//! use smithay::input::{Seat, SeatState, SeatHandler, pointer::CursorImageStatus};
//! # use smithay::wayland::compositor::{CompositorHandler, CompositorState, CompositorClientState};
//! use smithay::wayland::input_method::{InputMethodManagerState, InputMethodHandler, PopupSurface};
//! use smithay::wayland::text_input::TextInputManagerState;
//! use smithay::reexports::wayland_server::{Display, protocol::wl_surface::WlSurface};
//! # use smithay::reexports::wayland_server::Client;
//! use smithay::utils::{Rectangle, Logical};
//!
//! # struct State { seat_state: SeatState<Self> };
//!
//! delegate_seat!(State);
//! # delegate_compositor!(State);
//!
//! impl InputMethodHandler for State {
//!     fn new_popup(&mut self, surface: PopupSurface) {}
//!     fn dismiss_popup(&mut self, surface: PopupSurface) {}
//!     fn popup_repositioned(&mut self, surface: PopupSurface) {}
//!     fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
//!         Rectangle::default()
//!     }
//! }
//!
//! // Delegate input method handling for State to InputMethodManagerState.
//! delegate_input_method_manager!(State);
//!
//! delegate_text_input_manager!(State);
//!
//! # let mut display = wayland_server::Display::<State>::new().unwrap();
//! # let display_handle = display.handle();
//!
//! let mut seat_state = SeatState::<State>::new();
//!
//! // implement the required traits
//! impl SeatHandler for State {
//!     type KeyboardFocus = WlSurface;
//!     type PointerFocus = WlSurface;
//!     type TouchFocus = WlSurface;
//!     fn seat_state(&mut self) -> &mut SeatState<Self> {
//!         &mut self.seat_state
//!     }
//!     fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) { unimplemented!() }
//!     fn cursor_image(&mut self, seat: &Seat<Self>, image: CursorImageStatus) { unimplemented!() }
//! }
//!
//! # impl CompositorHandler for State {
//! #     fn compositor_state(&mut self) -> &mut CompositorState { unimplemented!() }
//! #     fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState { unimplemented!() }
//! #     fn commit(&mut self, surface: &WlSurface) {}
//! # }
//!
//! // Add the seat state to your state and create manager globals
//! InputMethodManagerState::new::<State, _>(&display_handle, |_client| true);
//! // Add text input capabilities, needed for the input method to work
//! TextInputManagerState::new::<State>(&display_handle);
//!
//! ```

use wayland_server::{
    backend::GlobalId, protocol::wl_surface::WlSurface, Client, DataInit, Dispatch, DisplayHandle,
    GlobalDispatch, New,
};

use wayland_protocols_misc::zwp_input_method_v2::server::{
    zwp_input_method_manager_v2::{self, ZwpInputMethodManagerV2},
    zwp_input_method_v2::ZwpInputMethodV2,
};

use crate::{
    input::{Seat, SeatHandler},
    utils::{Logical, Rectangle},
};

pub use input_method_handle::{InputMethodHandle, InputMethodUserData};
pub use input_method_keyboard_grab::{InputMethodKeyboardGrab, InputMethodKeyboardUserData};
pub use input_method_popup_surface::InputMethodPopupSurfaceUserData;

use super::text_input::TextInputHandle;

const MANAGER_VERSION: u32 = 1;

/// The role of the input method popup.
pub const INPUT_POPUP_SURFACE_ROLE: &str = "zwp_input_popup_surface_v2";

mod input_method_handle;
mod input_method_keyboard_grab;
mod input_method_popup_surface;
pub use input_method_popup_surface::{PopupParent, PopupSurface};

/// Adds input method popup to compositor state
pub trait InputMethodHandler {
    /// Add a popup surface to compositor state.
    fn new_popup(&mut self, surface: PopupSurface);

    /// Dismiss a popup surface from the compositor state.
    fn dismiss_popup(&mut self, surface: PopupSurface);

    /// Popup location has changed.
    fn popup_repositioned(&mut self, surface: PopupSurface);

    /// Sets the parent location so the popup surface can be placed correctly
    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical>;
}

/// Extends [Seat] with input method functionality
pub trait InputMethodSeat {
    /// Get an input method associated with this seat
    fn input_method(&self) -> &InputMethodHandle;
}

impl<D: SeatHandler + 'static> InputMethodSeat for Seat<D> {
    fn input_method(&self) -> &InputMethodHandle {
        let user_data = self.user_data();
        user_data.insert_if_missing(InputMethodHandle::default);
        user_data.get::<InputMethodHandle>().unwrap()
    }
}

/// Data associated with a InputMethodManager global.
#[allow(missing_debug_implementations)]
pub struct InputMethodManagerGlobalData {
    filter: Box<dyn for<'c> Fn(&'c Client) -> bool + Send + Sync>,
}

/// State of wp misc input method protocol
#[derive(Debug)]
pub struct InputMethodManagerState {
    global: GlobalId,
}

impl InputMethodManagerState {
    /// Initialize a text input manager global.
    pub fn new<D, F>(display: &DisplayHandle, filter: F) -> Self
    where
        D: GlobalDispatch<ZwpInputMethodManagerV2, InputMethodManagerGlobalData>,
        D: Dispatch<ZwpInputMethodManagerV2, ()>,
        D: Dispatch<ZwpInputMethodV2, InputMethodUserData<D>>,
        D: SeatHandler,
        D: 'static,
        F: for<'c> Fn(&'c Client) -> bool + Send + Sync + 'static,
    {
        let data = InputMethodManagerGlobalData {
            filter: Box::new(filter),
        };
        let global = display.create_global::<D, ZwpInputMethodManagerV2, _>(MANAGER_VERSION, data);

        Self { global }
    }

    /// Get the id of ZwpTextInputManagerV3 global
    pub fn global(&self) -> GlobalId {
        self.global.clone()
    }
}

impl<D> GlobalDispatch<ZwpInputMethodManagerV2, InputMethodManagerGlobalData, D> for InputMethodManagerState
where
    D: GlobalDispatch<ZwpInputMethodManagerV2, InputMethodManagerGlobalData>,
    D: Dispatch<ZwpInputMethodManagerV2, ()>,
    D: Dispatch<ZwpInputMethodV2, InputMethodUserData<D>>,
    D: SeatHandler,
    D: 'static,
{
    fn bind(
        _: &mut D,
        _: &DisplayHandle,
        _: &Client,
        resource: New<ZwpInputMethodManagerV2>,
        _: &InputMethodManagerGlobalData,
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(resource, ());
    }

    fn can_view(client: Client, global_data: &InputMethodManagerGlobalData) -> bool {
        (global_data.filter)(&client)
    }
}

impl<D> Dispatch<ZwpInputMethodManagerV2, (), D> for InputMethodManagerState
where
    D: Dispatch<ZwpInputMethodManagerV2, ()>,
    D: Dispatch<ZwpInputMethodV2, InputMethodUserData<D>>,
    D: SeatHandler + InputMethodHandler,
    D: 'static,
{
    fn request(
        _state: &mut D,
        _client: &Client,
        _: &ZwpInputMethodManagerV2,
        request: zwp_input_method_manager_v2::Request,
        _: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        match request {
            zwp_input_method_manager_v2::Request::GetInputMethod { seat, input_method } => {
                let seat = Seat::<D>::from_resource(&seat).unwrap();

                let user_data = seat.user_data();
                user_data.insert_if_missing(TextInputHandle::default);
                user_data.insert_if_missing(InputMethodHandle::default);
                let handle = user_data.get::<InputMethodHandle>().unwrap();
                let text_input_handle = user_data.get::<TextInputHandle>().unwrap();
                text_input_handle.with_focused_text_input(|ti, surface| {
                    ti.enter(surface);
                });
                let keyboard_handle = seat.get_keyboard().unwrap();
                let instance = data_init.init(
                    input_method,
                    InputMethodUserData {
                        handle: handle.clone(),
                        text_input_handle: text_input_handle.clone(),
                        keyboard_handle,
                        popup_geometry_callback: D::parent_geometry,
                        popup_repositioned: D::popup_repositioned,
                        new_popup: D::new_popup,
                        dismiss_popup: D::dismiss_popup,
                    },
                );
                handle.add_instance(&instance);
            }
            zwp_input_method_manager_v2::Request::Destroy => {
                // Nothing to do
            }
            _ => unreachable!(),
        }
    }
}

#[allow(missing_docs)] // TODO
#[macro_export]
macro_rules! delegate_input_method_manager {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        $crate::reexports::wayland_server::delegate_global_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            $crate::reexports::wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_manager_v2::ZwpInputMethodManagerV2:
            $crate::wayland::input_method::InputMethodManagerGlobalData
        ] => $crate::wayland::input_method::InputMethodManagerState);

        $crate::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            $crate::reexports::wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_manager_v2::ZwpInputMethodManagerV2: ()
        ] => $crate::wayland::input_method::InputMethodManagerState);
        $crate::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            $crate::reexports::wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_v2::ZwpInputMethodV2:
            $crate::wayland::input_method::InputMethodUserData<Self>
        ] => $crate::wayland::input_method::InputMethodManagerState);
        $crate::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            $crate::reexports::wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2:
            $crate::wayland::input_method::InputMethodKeyboardUserData<Self>
        ] => $crate::wayland::input_method::InputMethodManagerState);
        $crate::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            $crate::reexports::wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2:
            $crate::wayland::input_method::InputMethodPopupSurfaceUserData
        ] => $crate::wayland::input_method::InputMethodManagerState);
    };
}
#[cfg(test)]
mod tests {
    //! These tests talk to the compositor side over a socket pair with hand-written wire messages,
    //! so that they need no Wayland client library.

    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
        sync::Arc,
    };

    use wayland_server::{
        backend::{ClientData, ClientId, DisconnectReason},
        Display,
    };

    use super::*;
    use crate::{
        backend::input::{KeyState, Keycode},
        input::{
            keyboard::{FilterResult, XkbConfig},
            SeatState,
        },
        utils::SERIAL_COUNTER,
        wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState},
    };

    struct State {
        compositor_state: CompositorState,
        seat_state: SeatState<Self>,
    }

    impl SeatHandler for State {
        type KeyboardFocus = WlSurface;
        type PointerFocus = WlSurface;
        type TouchFocus = WlSurface;
        fn seat_state(&mut self) -> &mut SeatState<Self> {
            &mut self.seat_state
        }
    }

    impl CompositorHandler for State {
        fn compositor_state(&mut self) -> &mut CompositorState {
            &mut self.compositor_state
        }
        fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
            &client.get_data::<TestClientData>().unwrap().compositor_state
        }
        fn commit(&mut self, _: &WlSurface) {}
    }

    impl InputMethodHandler for State {
        fn new_popup(&mut self, _: PopupSurface) {}
        fn dismiss_popup(&mut self, _: PopupSurface) {}
        fn popup_repositioned(&mut self, _: PopupSurface) {}
        fn parent_geometry(&self, _: &WlSurface) -> Rectangle<i32, Logical> {
            Rectangle::default()
        }
    }

    crate::delegate_compositor!(State);
    crate::delegate_seat!(State);
    crate::delegate_input_method_manager!(State);

    #[derive(Default)]
    struct TestClientData {
        compositor_state: CompositorClientState,
    }
    impl ClientData for TestClientData {
        fn initialized(&self, _: ClientId) {}
        fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
    }

    /// The compositor side: a display with a seat that has a keyboard, and the input method global.
    struct Server {
        display: Display<State>,
        state: State,
        seat: Seat<State>,
    }

    impl Server {
        fn new() -> Self {
            let display = Display::<State>::new().unwrap();
            let dh = display.handle();
            let mut seat_state = SeatState::new();
            let mut seat = seat_state.new_wl_seat(&dh, "seat0");
            seat.add_keyboard(XkbConfig::default(), 200, 25).unwrap();
            InputMethodManagerState::new::<State, _>(&dh, |_| true);
            let state = State {
                compositor_state: CompositorState::new::<State>(&dh),
                seat_state,
            };
            Server { display, state, seat }
        }

        fn connect(&mut self) -> RawClient {
            let (server, client) = UnixStream::pair().unwrap();
            self.display
                .handle()
                .insert_client(server, Arc::new(TestClientData::default()))
                .unwrap();
            client.set_nonblocking(true).unwrap();
            let mut client = RawClient {
                stream: client,
                next_id: 2,
                globals: Vec::new(),
            };
            // wl_display.get_registry
            let registry = client.next_new_id();
            client.send(1, 1, &registry.to_ne_bytes());
            self.roundtrip();
            for (object, opcode, body) in client.events() {
                if (object, opcode) == (2, 0) {
                    let name = u32::from_ne_bytes(body[0..4].try_into().unwrap());
                    let len = u32::from_ne_bytes(body[4..8].try_into().unwrap()) as usize;
                    let interface = String::from_utf8(body[8..8 + len - 1].to_vec()).unwrap();
                    client.globals.push((name, interface));
                }
            }
            client
        }

        fn roundtrip(&mut self) {
            self.display.dispatch_clients(&mut self.state).unwrap();
            self.display.flush_clients().unwrap();
        }

        /// Sends a key press from the seat's keyboard, as the backend would.
        fn press_key(&mut self, keycode: u32) {
            let keyboard = self.seat.get_keyboard().unwrap();
            keyboard.input::<(), _>(
                &mut self.state,
                Keycode::new(keycode),
                KeyState::Pressed,
                SERIAL_COUNTER.next_serial(),
                0,
                |_, _, _| FilterResult::Forward,
            );
            self.display.flush_clients().unwrap();
        }
    }

    /// A client that writes raw wire messages.
    struct RawClient {
        stream: UnixStream,
        next_id: u32,
        globals: Vec<(u32, String)>,
    }

    impl RawClient {
        fn next_new_id(&mut self) -> u32 {
            let id = self.next_id;
            self.next_id += 1;
            id
        }

        fn send(&mut self, object: u32, opcode: u16, args: &[u8]) {
            let size = (8 + args.len()) as u32;
            let mut message = Vec::new();
            message.extend_from_slice(&object.to_ne_bytes());
            message.extend_from_slice(&((size << 16) | opcode as u32).to_ne_bytes());
            message.extend_from_slice(args);
            self.stream.write_all(&message).unwrap();
        }

        /// wl_registry.bind of the global named `interface`. Returns the new object id.
        fn bind(&mut self, interface: &str, version: u32) -> u32 {
            let name = self.globals.iter().find(|(_, i)| i == interface).unwrap().0;
            let id = self.next_new_id();
            let mut args = name.to_ne_bytes().to_vec();
            let mut string = interface.as_bytes().to_vec();
            string.push(0);
            args.extend_from_slice(&(string.len() as u32).to_ne_bytes());
            while !string.len().is_multiple_of(4) {
                string.push(0);
            }
            args.extend_from_slice(&string);
            args.extend_from_slice(&version.to_ne_bytes());
            args.extend_from_slice(&id.to_ne_bytes());
            self.send(2, 0, &args);
            id
        }

        /// The events received so far, as (object, opcode, body).
        fn events(&mut self) -> Vec<(u32, u16, Vec<u8>)> {
            let mut data = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                match self.stream.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => data.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => panic!("{e}"),
                }
            }
            let mut events = Vec::new();
            let mut at = 0;
            while at + 8 <= data.len() {
                let object = u32::from_ne_bytes(data[at..at + 4].try_into().unwrap());
                let word = u32::from_ne_bytes(data[at + 4..at + 8].try_into().unwrap());
                let (size, opcode) = ((word >> 16) as usize, (word & 0xffff) as u16);
                events.push((object, opcode, data[at + 8..at + size].to_vec()));
                at += size;
            }
            events
        }
    }

    /// zwp_input_method_manager_v2.get_input_method(seat, new id). Returns the input method id.
    fn get_input_method(server: &mut Server, client: &mut RawClient, manager: u32, seat: u32) -> u32 {
        let input_method = client.next_new_id();
        let mut args = seat.to_ne_bytes().to_vec();
        args.extend_from_slice(&input_method.to_ne_bytes());
        client.send(manager, 0, &args);
        server.roundtrip();
        input_method
    }

    /// zwp_input_method_v2.grab_keyboard(new id). Returns the keyboard grab id.
    fn grab_keyboard(server: &mut Server, client: &mut RawClient, input_method: u32) -> u32 {
        let grab = client.next_new_id();
        client.send(input_method, 5, &grab.to_ne_bytes());
        server.roundtrip();
        grab
    }

    /// zwp_input_method_v2.unavailable
    const UNAVAILABLE: u16 = 6;
    /// zwp_input_method_v2.destroy
    const DESTROY: u16 = 6;
    /// zwp_input_method_keyboard_grab_v2.release
    const RELEASE: u16 = 0;
    /// zwp_input_method_keyboard_grab_v2.key
    const KEY: u16 = 1;

    #[test]
    fn releasing_the_grab_of_a_destroyed_input_method_keeps_the_new_grab() {
        let mut server = Server::new();
        let mut client = server.connect();
        let seat = client.bind("wl_seat", 1);
        let manager = client.bind("zwp_input_method_manager_v2", 1);

        // The first input method grabs the keyboard, then destroys only its input method object.
        let old_input_method = get_input_method(&mut server, &mut client, manager, seat);
        let old_grab = grab_keyboard(&mut server, &mut client, old_input_method);
        client.send(old_input_method, DESTROY, &[]);
        server.roundtrip();

        // A new input method on the same connection grabs the keyboard.
        let input_method = get_input_method(&mut server, &mut client, manager, seat);
        let grab = grab_keyboard(&mut server, &mut client, input_method);
        assert!(!client
            .events()
            .iter()
            .any(|&(object, opcode, _)| (object, opcode) == (input_method, UNAVAILABLE)));

        // Releasing the old grab object used to release the new grab as well.
        client.send(old_grab, RELEASE, &[]);
        server.roundtrip();
        assert!(
            server.seat.get_keyboard().unwrap().is_grabbed(),
            "the new input method lost its keyboard grab"
        );
        server.press_key(38);
        assert!(
            client
                .events()
                .iter()
                .any(|&(object, opcode, _)| (object, opcode) == (grab, KEY)),
            "the key did not reach the new input method"
        );
    }
}
