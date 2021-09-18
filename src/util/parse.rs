fn parse_hex_slice(slice: &str) -> Result<u32, ParseHexError> {
    u32::from_str_radix(slice, 16).map_err(|err| ParseHexError {
        reason: err.to_string(),
        source: slice,
    })
}

/// Parse an HTML-color-like hex input
pub fn parse_hex(hex: &str) -> Result<u32, ParseHexError> {
    let hex = hex.trim_start_matches('#');
    let mut color;

    match hex.len() {
        3 | 4 => {
            color = 0x11 * parse_hex_slice(&hex[2..3])?
                + 0x11_00 * parse_hex_slice(&hex[1..2])?
                + 0x11_00_00 * parse_hex_slice(&hex[0..1])?;

            if hex.len() == 4 {
                color |= 0x11_00_00_00 * parse_hex_slice(&hex[3..4])?
            } else {
                color |= 0xFF_00_00_00;
            }
        }

        6 | 8 => {
            color = parse_hex_slice(&hex)?;

            if hex.len() == 6 {
                color |= 0xFF_00_00_00;
            }
        }

        _ => {
            return Err(ParseHexError {
                reason: "Hex colour should have length 3, 4, 6, or 8".to_owned(),
                source: hex,
            })
        }
    }

    Ok(color)
}
