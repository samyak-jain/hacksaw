use x11rb::{connection::Connection, protocol::xproto};
use crate::errors::XError;

pub const CURSOR_GRAB_TRIES: i32 = 5;

pub struct X11Connection {
    conn: x11rb::rust_connection::RustConnection,
}

impl X11Connection {
    fn grab_cursor(&self, root: u32) -> Result<(), XError> {
        let font_id = self.conn.generate_id()?;
        xproto::open_font(&self.conn, font_id, b"cursor")?.check()?;

        // TODO: create cursor with a Pixmap
        // https://stackoverflow.com/questions/40578969/how-to-create-a-cursor-in-x11-from-raw-data-c
        let cursor = self.conn.generate_id()?;
        xproto::create_glyph_cursor(&self.conn, cursor, font_id, font_id, 0, 30, 0, 0, 0, 0, 0, 0)?
            .check()?;

        for i in 0..CURSOR_GRAB_TRIES {
            let reply = xproto::grab_pointer(
                &self.conn,
                true,
                root,
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

        return Err(XError::TooManyRetriesError);
    }
}
