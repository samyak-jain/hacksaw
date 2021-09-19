mod errors;
mod util;
mod x11;

use std::convert::TryFrom;

use util::{
    find_escape_keycode, get_window_at_point, get_window_geom, grab_key, grab_pointer_set_cursor,
    set_shape, set_title, ungrab_key, HacksawContainer, CURSOR_GRAB_TRIES,
};
use x11::Guides;
use x11rb::connection::Connection;
use x11rb::protocol::{xproto, Event};

fn min_max(a: i16, b: i16) -> (i16, i16) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn build_guides(
    screen: xproto::Rectangle,
    pt: xproto::Point,
    width: u16,
) -> [xproto::Rectangle; 2] {
    [
        xproto::Rectangle {
            x: pt.x - width as i16 / 2,
            y: screen.x,
            width,
            height: screen.height,
        },
        xproto::Rectangle {
            x: screen.y,
            y: pt.y - width as i16 / 2,
            width: screen.width,
            height: width,
        },
    ]
}

pub struct HackSawConfig {
    line_width: u16,
    guide_width: Option<u16>,
    line_colour: u32,
    remove_decorations: u32,
}

impl Default for HackSawConfig {
    fn default() -> Self {
        Self {
            line_width: 1,
            guide_width: Some(1),
            line_colour: util::parse_hex("#7f7f7f").unwrap(),
            remove_decorations: 0,
        }
    }
}

pub fn get_screen() -> Result<HackSawResult, String> {
    let (conn, screen_num) = x11rb::rust_connection::RustConnection::connect(None).unwrap();
    let setup = conn.setup();
    let screen = &setup.roots[screen_num];

    let window = conn.generate_id().unwrap();

    return Ok(HackSawResult {
        window,
        x: 0,
        y: 0,
        width: screen.width_in_pixels,
        height: screen.height_in_pixels,
    });
}

pub fn make_selection(config: Option<HackSawConfig>) -> Result<HackSawResult, String> {
    let opt = match config {
        Some(c) => c,
        None => HackSawConfig::default(),
    };

    let conn = x11::new_connection();
    conn.grab_cursor();
    conn.grab_key(conn.get_escape_keycode().unwrap());

    conn.create_window(opt.line_colour);

    // set_shape(
    //     &conn,
    //     window,
    //     &[xproto::Rectangle {
    //         x: 0,
    //         y: 0,
    //         width: 0,
    //         height: 0,
    //     }],
    // );

    if let Some(guide_width) = opt.guide_width {
        let pointer = conn.get_pointer().unwrap();
        conn.make_guides(
            Guides::try_from((
                conn.screen,
                xproto::Point {
                    x: pointer.root_x,
                    y: pointer.root_y,
                },
                guide_width,
            ))
            .unwrap(),
        );
    }

    conn.flush().unwrap();

    let mut start_pt = xproto::Point { x: 0, y: 0 };
    let mut selection = xproto::Rectangle {
        x: 0,
        y: 0,
        width: 0,
        height: 0,
    };

    let mut in_selection = false;
    let mut ignore_next_release = false;

    // TODO draw rectangle around window under cursor
    loop {
        let ev = conn
            .conn
            .wait_for_event()
            .map_err(|_| "Error getting X event, quitting.".to_string())?;

        match ev {
            Event::ButtonPress(button_press) => {
                let detail = button_press.detail;
                if detail == 3 {
                    return Err("Exiting due to right click".into());
                } else {
                    conn.make_guides((&[] as &[xproto::Rectangle; 0]).into());
                    conn.conn.flush().unwrap();

                    start_pt = xproto::Point {
                        x: button_press.event_x,
                        y: button_press.event_y,
                    };

                    in_selection = !(detail == 4 || detail == 5);
                    ignore_next_release = detail == 4 || detail == 5;
                }
            }
            Event::KeyPress(_) => {
                // This will only happen with an escape key since we only grabbed escape
                return Err("Exiting due to ESC key press".into());
            }
            Event::MotionNotify(motion) => {
                let (left_x, right_x) = min_max(motion.event_x, start_pt.x);
                let (top_y, bottom_y) = min_max(motion.event_y, start_pt.y);
                let width = (right_x - left_x) as u16;
                let height = (bottom_y - top_y) as u16;

                // only save the width and height if we are selecting a
                // rectangle, since we then use these (non-zero width/height)
                // to determine if a selection was made.
                selection = if in_selection {
                    xproto::Rectangle {
                        x: left_x,
                        y: top_y,
                        width,
                        height,
                    }
                } else {
                    xproto::Rectangle {
                        x: left_x,
                        y: top_y,
                        width: 0,
                        height: 0,
                    }
                };

                if in_selection {
                    let rects = [
                        // Selection rectangle
                        xproto::Rectangle {
                            x: left_x - opt.line_width as i16,
                            y: top_y,
                            width: opt.line_width,
                            height: height + opt.line_width,
                        },
                        xproto::Rectangle {
                            x: left_x - opt.line_width as i16,
                            y: top_y - opt.line_width as i16,
                            width: width + opt.line_width,
                            height: opt.line_width,
                        },
                        xproto::Rectangle {
                            x: right_x,
                            y: top_y - opt.line_width as i16,
                            width: opt.line_width,
                            height: height + opt.line_width,
                        },
                        xproto::Rectangle {
                            x: left_x,
                            y: bottom_y,
                            width: width + opt.line_width,
                            height: opt.line_width,
                        },
                    ];

                    conn.make_guides((&rects).into());
                } else if let Some(guide_width) = opt.guide_width {
                    let rects = build_guides(
                        screen_rect,
                        xproto::Point {
                            x: motion.event_x,
                            y: motion.event_y,
                        },
                        guide_width,
                    );

                    conn.make_guides((conn.screen, xproto::Point {
                        x: motion.event_x,
                        y: motion.event_y,
                    }, guide_width));
                }

                conn.flush().unwrap();
            }
            Event::ButtonRelease(button_release) => {
                let detail = button_release.detail;
                if detail == 4 || detail == 5 {
                    continue; // Scroll wheel up/down release
                } else if ignore_next_release {
                    ignore_next_release = false;
                    continue;
                } else {
                    break;
                }
                // Move on after mouse released
            }
            _ => continue,
        };
    }

    conn.destory_window().unwrap();
    conn.ungrab_cursor().unwrap();

    conn.ungrab_key(conn.get_escape_keycode()?).unwrap();
    conn.flush().unwrap();

    loop {
        let ev = conn
            .wait_for_event()
            .map_err(|_| "Error getting X event, quitting.".to_string())?;

        match ev {
            x11rb::protocol::Event::UnmapNotify(_) | x11rb::protocol::Event::DestroyNotify(_) => {
                break;
            }
            _ => (),
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(40));

    let result;
    if selection.width == 0 && selection.height == 0 {
        // Grab window under cursor
        result = match get_window_at_point(&conn, root, start_pt, opt.remove_decorations) {
            Some(r) => r,
            None => get_window_geom(&conn, screen.root),
        }
    } else {
        result = HacksawContainer {
            window: root,
            rect: selection,
        };
    }

    Ok(HackSawResult {
        window: result.window,
        height: result.height(),
        width: result.width(),
        x: result.x(),
        y: result.y(),
    })
}
