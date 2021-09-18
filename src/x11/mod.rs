use crate::errors::{GenericConnectionError, GrabError, KeyboardError};
use std::convert::TryFrom;
use x11rb::{
    connection::Connection,
    protocol::xproto::{self, Screen},
};

pub const CURSOR_GRAB_TRIES: i32 = 5;
const ESC_KEYSYM: u32 = 0xff1b;

/// Since MOD_MASK_ANY is apparently bug-ridden, we instead exploit the fact
/// that the modifier masks NONE to MOD_MASK_5 are 0, 1, 2, 4, 8, ... 128.
/// Then we grab on every possible combination of these masks by iterating
/// through all the integers 0 to 255. This allows us to grab Esc, Shift+Esc,
/// CapsLock+Shift+Esc, or any other combination.
const KEY_GRAB_MASK_MAX: u16 = (u16::from(xproto::ModMask::M5) * 2) - 1;

pub struct X11Connection<'a> {
    conn: x11rb::rust_connection::RustConnection,
    screen: &'a Screen,
    window: u32,
}

pub fn new_connection<'a>() -> X11Connection<'a> {
    let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None).unwrap();
    let setup = conn.setup();
    let screen = &setup.roots[screen_num];
    let window = conn.generate_id().unwrap();

    X11Connection {
        conn,
        screen,
        window,
    }
}

impl X11Connection<'_> {
    pub fn grab_cursor(&self) -> Result<(), GrabError> {
        let font_id = self.conn.generate_id()?;
        xproto::open_font(&self.conn, font_id, b"cursor")?.check()?;

        // TODO: create cursor with a Pixmap
        // https://stackoverflow.com/questions/40578969/how-to-create-a-cursor-in-x11-from-raw-data-c
        let cursor = self.conn.generate_id()?;
        xproto::create_glyph_cursor(
            &self.conn, cursor, font_id, font_id, 0, 30, 0, 0, 0, 0, 0, 0,
        )?
        .check()?;

        for i in 0..CURSOR_GRAB_TRIES {
            let reply = xproto::grab_pointer(
                &self.conn,
                true,
                self.screen.root,
                u32::from(
                    xproto::EventMask::BUTTON_RELEASE
                        | xproto::EventMask::BUTTON_PRESS
                        | xproto::EventMask::BUTTON_MOTION
                        | xproto::EventMask::POINTER_MOTION,
                ) as u16,
                xproto::GrabMode::ASYNC,
                xproto::GrabMode::ASYNC,
                x11rb::NONE,
                cursor,
                x11rb::CURRENT_TIME,
            )?
            .reply()?;

            if reply.status == xproto::GrabStatus::SUCCESS {
                return Ok(());
            } else if i < CURSOR_GRAB_TRIES - 1 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        return Err(GrabError::TooManyRetriesError);
    }

    pub fn get_escape_keycode(&self) -> Result<xproto::Keycode, KeyboardError> {
        // https://stackoverflow.com/questions/18689863/obtain-keyboard-layout-and-keysyms-with-xcb
        let setup = self.conn.setup();
        let reply = xproto::get_keyboard_mapping(
            &self.conn,
            setup.min_keycode,
            setup.max_keycode - setup.min_keycode + 1,
        )?
        .reply()?;

        let escape_index = reply
            .keysyms
            .iter()
            .position(|&keysym| keysym == ESC_KEYSYM)
            .ok_or(KeyboardError::NotFound)?;

        match u8::try_from(escape_index / usize::from(reply.keysyms_per_keycode)) {
            Ok(escape) => return Ok(escape + setup.min_keycode),
            Err(_) => return Err(KeyboardError::NotFound),
        }
    }

    pub fn grab_key(&self, keycode: xproto::Keycode) -> Result<(), KeyboardError> {
        for mask in 0..=KEY_GRAB_MASK_MAX {
            xproto::grab_key(
                &self.conn,
                true,
                self.screen.root,
                mask,
                keycode,
                xproto::GrabMode::ASYNC,
                xproto::GrabMode::ASYNC,
            )?
            .check()?;
        }

        Ok(())
    }

    pub fn create_window(&self, line_colour: u32) -> Result<(), GenericConnectionError> {
        // TODO event handling for expose/keypress
        let value_list = xproto::CreateWindowAux::new()
            .background_pixel(line_colour)
            .event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::STRUCTURE_NOTIFY
                    | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            )
            .override_redirect(1);

        xproto::create_window(
            &self.conn,
            x11rb::COPY_DEPTH_FROM_PARENT,
            self.window,
            self.screen.root,
            0,
            0,
            self.screen.width_in_pixels,
            self.screen.height_in_pixels,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            self.screen.root_visual,
            &value_list,
        )?
        .check()?;

        let title = "hacksaw";
        xproto::change_property(
            &self.conn,
            xproto::PropMode::REPLACE,
            self.window,
            xproto::AtomEnum::WM_NAME,
            xproto::AtomEnum::STRING,
            8,
            title.len() as u32,
            title.as_bytes(),
        )?
        .check()?;

        Ok(())
    }
}
