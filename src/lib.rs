extern crate structopt;
extern crate xcb;
mod util;

use util::{
    find_escape_keycode, get_window_at_point, get_window_geom, grab_key, grab_pointer_set_cursor,
    set_shape, set_title, ungrab_key, HacksawResult, CURSOR_GRAB_TRIES,
};

fn min_max(a: i16, b: i16) -> (i16, i16) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn build_guides(screen: xcb::Rectangle, pt: xcb::Point, width: u16) -> [xcb::Rectangle; 2] {
    [
        xcb::Rectangle::new(
            pt.x() - width as i16 / 2,
            screen.x(),
            width,
            screen.height(),
        ),
        xcb::Rectangle::new(screen.y(), pt.y() - width as i16 / 2, screen.width(), width),
    ]
}

pub struct HackSawConfig {
    line_width: u16,
    guide_width: u16,
    line_colour: u32,
    format: util::parse_format::Format,
    remove_decorations: u32,
}

impl Default for HackSawConfig {
    fn default() -> Self {
        Self {
            line_width: 1,
            guide_width: 1,
            line_colour: util::parse_args::parse_hex("#7f7f7f").unwrap(),
            format: util::parse_format::parse_format_string("%g").unwrap(),
            remove_decorations: 0,
        }
    }
}

pub fn launch_default(config: Option<HackSawConfig>, no_guides: bool) -> String {
    let opt = match config {
        Some(c) => c,
        None => HackSawConfig::default(),
    };

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();
    let root = screen.root();

    let window = conn.generate_id();

    // TODO fix pointer-grab? bug where hacksaw hangs if mouse held down before run
    if !grab_pointer_set_cursor(&conn, root) {
        return Err(format!(
            "Failed to grab cursor after {} tries, giving up",
            CURSOR_GRAB_TRIES
        ));
    }

    let escape_keycode = find_escape_keycode(&conn);
    grab_key(&conn, root, escape_keycode);

    let screen_rect =
        xcb::Rectangle::new(0, 0, screen.width_in_pixels(), screen.height_in_pixels());

    // TODO event handling for expose/keypress
    let values = [
        // ?RGB. First 4 bytes appear to do nothing
        (xcb::CW_BACK_PIXEL, opt.line_colour),
        (
            xcb::CW_EVENT_MASK,
            xcb::EVENT_MASK_EXPOSURE
            | xcb::EVENT_MASK_KEY_PRESS // we'll need this later
            | xcb::EVENT_MASK_STRUCTURE_NOTIFY
            | xcb::EVENT_MASK_SUBSTRUCTURE_NOTIFY,
        ),
        (xcb::CW_OVERRIDE_REDIRECT, 1u32), // Don't be window managed
    ];

    xcb::create_window(
        &conn,
        xcb::COPY_FROM_PARENT as u8, // usually 32?
        window,
        root,
        screen_rect.x(),
        screen_rect.y(),
        screen_rect.width(),
        screen_rect.height(),
        0,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        screen.root_visual(),
        &values,
    );

    set_title(&conn, window, "hacksaw");

    set_shape(&conn, window, &[xcb::Rectangle::new(0, 0, 0, 0)]);

    xcb::map_window(&conn, window);

    if !no_guides {
        let pointer = xcb::query_pointer(&conn, root).get_reply().unwrap();
        set_shape(
            &conn,
            window,
            &build_guides(
                screen_rect,
                xcb::Point::new(pointer.root_x(), pointer.root_y()),
                opt.guide_width,
            ),
        );
    }

    conn.flush();

    let mut start_pt = xcb::Point::new(0, 0);
    let mut selection = xcb::Rectangle::new(0, 0, 0, 0);

    let mut in_selection = false;
    let mut ignore_next_release = false;

    // TODO draw rectangle around window under cursor
    loop {
        let ev = conn
            .wait_for_event()
            .ok_or_else(|| "Error getting X event, quitting.".to_string())?;

        match ev.response_type() {
            xcb::BUTTON_PRESS => {
                let button_press: &xcb::ButtonPressEvent = unsafe { xcb::cast_event(&ev) };

                let detail = button_press.detail();
                if detail == 3 {
                    return Err("Exiting due to right click".into());
                } else {
                    set_shape(&conn, window, &[]);
                    conn.flush();
                    start_pt = xcb::Point::new(button_press.event_x(), button_press.event_y());

                    in_selection = !(detail == 4 || detail == 5);
                    ignore_next_release = detail == 4 || detail == 5;
                }
            }
            xcb::KEY_PRESS => {
                // This will only happen with an escape key since we only grabbed escape
                return Err("Exiting due to ESC key press".into());
            }
            xcb::MOTION_NOTIFY => {
                let motion: &xcb::MotionNotifyEvent = unsafe { xcb::cast_event(&ev) };

                let (left_x, right_x) = min_max(motion.event_x(), start_pt.x());
                let (top_y, bottom_y) = min_max(motion.event_y(), start_pt.y());
                let width = (right_x - left_x) as u16;
                let height = (bottom_y - top_y) as u16;

                // only save the width and height if we are selecting a
                // rectangle, since we then use these (non-zero width/height)
                // to determine if a selection was made.
                if in_selection {
                    selection = xcb::Rectangle::new(left_x, top_y, width, height);
                } else {
                    selection = xcb::Rectangle::new(left_x, top_y, 0, 0);
                }

                if in_selection {
                    let rects = [
                        // Selection rectangle
                        xcb::Rectangle::new(
                            left_x - opt.line_width as i16,
                            top_y,
                            opt.line_width,
                            height + opt.line_width,
                        ),
                        xcb::Rectangle::new(
                            left_x - opt.line_width as i16,
                            top_y - opt.line_width as i16,
                            width + opt.line_width,
                            opt.line_width,
                        ),
                        xcb::Rectangle::new(
                            right_x,
                            top_y - opt.line_width as i16,
                            opt.line_width,
                            height + opt.line_width,
                        ),
                        xcb::Rectangle::new(left_x, bottom_y, width + opt.line_width, opt.line_width),
                    ];
                    set_shape(&conn, window, &rects);
                } else if !no_guides {
                    let rects = build_guides(
                        screen_rect,
                        xcb::Point::new(motion.event_x(), motion.event_y()),
                        opt.guide_width,
                    );

                    set_shape(&conn, window, &rects);
                }

                conn.flush();
            }
            xcb::BUTTON_RELEASE => {
                let motion: &xcb::ButtonReleaseEvent = unsafe { xcb::cast_event(&ev) };
                let detail = motion.detail();
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

    xcb::ungrab_pointer(&conn, xcb::CURRENT_TIME);
    ungrab_key(&conn, root, escape_keycode);
    xcb::unmap_window(&conn, window);
    xcb::destroy_window(&conn, window);
    conn.flush();

    loop {
        let ev = conn
            .wait_for_event()
            .ok_or_else(|| "Error getting X event, quitting.".to_string())?;

        match ev.response_type() {
            xcb::UNMAP_NOTIFY => {
                break;
            }
            xcb::DESTROY_NOTIFY => {
                break;
            }
            _ => (),
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(40));

    let result;
    if selection.width() == 0 && selection.height() == 0 {
        // Grab window under cursor
        result = match get_window_at_point(&conn, root, start_pt, opt.remove_decorations) {
            Some(r) => r,
            None => get_window_geom(&conn, screen.root()),
        }
    } else {
        result = HacksawResult {
            window: root,
            rect: selection,
        };
    }

    // Now we have taken coordinates, we print them out
    result.fill_format_string(&opt.format)
}
