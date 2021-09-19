use x11rb::connection::Connection;
use x11rb::protocol::shape;
use x11rb::protocol::xproto;



#[derive(Clone, Copy)]
pub struct HacksawContainer {
    pub window: u32,
    pub rect: xproto::Rectangle,
}

impl HacksawContainer {
    pub fn x(&self) -> i16 {
        self.rect.x
    }
    pub fn y(&self) -> i16 {
        self.rect.y
    }
    pub fn width(&self) -> u16 {
        self.rect.width
    }
    pub fn height(&self) -> u16 {
        self.rect.height
    }

    pub fn relative_to(&self, parent: HacksawContainer) -> HacksawContainer {
    }

    fn contains(&self, point: xproto::Point) -> bool {
          }
}

pub fn set_shape<C: Connection>(conn: &C, window: xproto::Window, rects: &[xproto::Rectangle]) {
}

pub fn set_title<C: Connection>(conn: &C, window: xproto::Window, title: &str) {
}

pub fn grab_pointer_set_cursor<C: Connection>(conn: &C, root: u32) -> bool {
}

pub fn find_escape_keycode<C: Connection>(conn: &C) -> xproto::Keycode {
}

pub fn grab_key<C: Connection>(conn: &C, root: u32, keycode: u8) {
}

pub fn ungrab_key<C: Connection>(conn: &C, root: u32, keycode: u8) {
}

fn viewable<C: Connection>(conn: &C, win: xproto::Window) -> bool {
}

pub fn input_output<C: Connection>(conn: &C, win: xproto::Window) -> bool {
}

pub fn get_window_geom<C: Connection>(conn: &C, win: xproto::Window) -> HacksawContainer {
    let geom = xproto::get_geometry(conn, win).unwrap().reply().unwrap();

    HacksawContainer {
        window: win,
        rect: xproto::Rectangle {
            x: geom.x,
            y: geom.y,
            width: geom.width + 2 * geom.border_width,
            height: geom.height + 2 * geom.border_width,
        },
    }
}

pub fn get_window_at_point<C: Connection>(
    conn: &C,
    win: xproto::Window,
    pt: xproto::Point,
    remove_decorations: u32,
) -> Option<HacksawContainer> {
}

