use crate::errors::{GenericConnectionError, GrabError, KeyboardError, WindowSelectError};
use std::{convert::TryFrom, num::TryFromIntError, ops::Deref};
use x11rb::{
    connection::Connection,
    protocol::{
        shape,
        xproto::{self, GetGeometryReply, QueryPointerReply, Screen, Window},
    },
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
    pub conn: x11rb::rust_connection::RustConnection,
    pub screen: &'a Screen,
    window: u32,
}

impl<'a> Deref for X11Connection<'a> {
    type Target = x11rb::rust_connection::RustConnection;

    fn deref<'b>(&'b self) -> &'b Self::Target {
        &self.conn
    }
}

pub enum SelectionType {
    SelectWindow(xproto::Point, u32),
    SelectFullScreen,
}

pub struct Guides<'a, const SIZE: usize> {
    rects: &'a [xproto::Rectangle; SIZE],
}

impl<'a, const SIZE: usize> From<&'a [xproto::Rectangle; SIZE]> for Guides<'a, SIZE> {
    fn from(val: &'a [xproto::Rectangle; SIZE]) -> Self {
        Self { rects: val }
    }
}

impl TryFrom<(&Screen, xproto::Point, u16)> for Guides<'_, 2> {
    type Error = TryFromIntError;

    fn try_from(guide_values: (&Screen, xproto::Point, u16)) -> Result<Self, Self::Error> {
        Ok(Self {
            rects: &[
                xproto::Rectangle {
                    x: guide_values.1.x - i16::try_from(guide_values.2)? / 2,
                    y: 0,
                    width: guide_values.2,
                    height: guide_values.0.height_in_pixels,
                },
                xproto::Rectangle {
                    x: 0,
                    y: guide_values.1.y - i16::try_from(guide_values.2)? / 2,
                    width: guide_values.0.width_in_pixels,
                    height: guide_values.2,
                },
            ],
        })
    }
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

#[derive(Debug)]
pub struct HackSawResult {
    pub window: u32,
    pub width: u16,
    pub height: u16,
    pub x: i16,
    pub y: i16,
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

    pub fn ungrab_cursor(&self) -> Result<(), GenericConnectionError> {
        xproto::ungrab_pointer(&self.conn, x11rb::CURRENT_TIME)?.check()?;
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

    pub fn ungrab_key(&self, keycode: xproto::Keycode) -> Result<(), GenericConnectionError> {
        for mask in 0..=KEY_GRAB_MASK_MAX {
            xproto::ungrab_key(&self.conn, keycode, self.window.root, mask)?.check()?;
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

        xproto::map_window(&self.conn, self.window)?.check()?;
        Ok(())
    }

    pub fn destory_window(&self) -> Result<(), GenericConnectionError> {
        xproto::unmap_window(&self.conn, self.window)?.check()?;
        xproto::destroy_window(&self.conn, self.window)?.check()?;
    }

    pub fn get_pointer(&self) -> Result<QueryPointerReply, GenericConnectionError> {
        Ok(xproto::query_pointer(&self.conn, self.screen.root)?.reply()?)
    }

    pub fn make_guides<const SIZE: usize>(
        &self,
        guides: Guides<SIZE>,
    ) -> Result<(), GenericConnectionError> {
        shape::rectangles(
            &self.conn,
            shape::SO::SET,
            shape::SK::BOUNDING,
            xproto::ClipOrdering::UNSORTED,
            self.window,
            0,
            0,
            guides.rects,
        )?
        .check()?;

        Ok(())
    }

    fn contains_point(container: GetGeometryReply, containing: xproto::Point) -> bool {
        // TODO negative x/y offsets from bottom or right?
        container.x < containing.x
            && container.y < containing.y
            && containing.x - container.x <= container.width as i16
            && containing.y - container.y <= container.height as i16
    }

    fn relative_to_point(
        geom: GetGeometryReply,
        parent: HackSawResult,
        window: Window,
    ) -> HackSawResult {
        HackSawResult {
            window,
            width: geom.width,
            height: geom.height,
            x: parent.x + geom.x,
            y: parent.y + geom.y,
        }
    }

    pub fn get_window_geometry(
        &self,
        selection: SelectionType,
    ) -> Result<HackSawResult, WindowSelectError> {
        match selection {
            SelectionType::SelectWindow(pt, remove_decorations) => {
                let tree = xproto::query_tree(&self.conn, self.window)?.reply()?;
                let targets = Vec::with_capacity(tree.children_len().into());

                for child in tree.children.iter() {
                    let attrs = xproto::get_window_attributes(&self.conn, *child)?.reply()?;
                    if attrs.map_state == xproto::MapState::VIEWABLE
                        && attrs.class == xproto::WindowClass::INPUT_OUTPUT
                    {
                        let geom = xproto::get_geometry(&self.conn, *child)?.reply()?;
                        if X11Connection::contains_point(geom, pt) {
                            targets.push(HackSawResult {
                                height: geom.height,
                                width: geom.width,
                                x: geom.x,
                                y: geom.y,
                                window: *child,
                            });
                        }
                    }
                }

                if targets.is_empty() {
                    return Err(WindowSelectError::NotFound);
                }

                let mut window = targets[targets.len() - 1];
                for _ in 0..remove_decorations {
                    let tree = xproto::query_tree(&self.conn, window.window)?.reply()?;
                    if tree.children.is_empty() {
                        break;
                    }
                    let firstborn = tree.children[0];
                    let geom = xproto::get_geometry(&self.conn, firstborn)?.reply()?;
                    window = X11Connection::relative_to_point(geom, window, firstborn);
                }

                return Ok(window);
            }
            SelectionType::SelectFullScreen => todo!(),
        }
    }
}
