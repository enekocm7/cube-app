//! Links that open a solve on [alg.cubing.net](https://alg.cubing.net).
//!
//! The site rebuilds the solve from the query string: the scramble in `setup`,
//! the solution in `alg`, a heading in `title`. It only has cubes, so other
//! events get no link. Nothing is sent anywhere until someone opens the link.

use std::fmt::Write as _;

use crate::event::WcaEvent;
use crate::solve::Solve;

const SITE: &str = "https://alg.cubing.net/";

/// Returns the site's name for `event`, if it has that puzzle.
pub const fn puzzle(event: WcaEvent) -> Option<&'static str> {
    match event {
        WcaEvent::Cube3x3OneHanded | WcaEvent::Cube3x3Blind => Some(WcaEvent::Cube3x3.name()),
        WcaEvent::Cube4x4Blind => Some(WcaEvent::Cube4x4.name()),
        WcaEvent::Cube5x5Blind => Some(WcaEvent::Cube5x5.name()),
        WcaEvent::Cube2x2
        | WcaEvent::Cube3x3
        | WcaEvent::Cube4x4
        | WcaEvent::Cube5x5
        | WcaEvent::Cube6x6
        | WcaEvent::Cube7x7 => Some(event.name()),
        WcaEvent::Megaminx
        | WcaEvent::Pyraminx
        | WcaEvent::Fto
        | WcaEvent::Skewb
        | WcaEvent::Square1
        | WcaEvent::Clock => None,
    }
}

/// Builds the link of `solve`: its scramble and, when it has them, its
/// solution and comment.
///
/// `None` when the site has no such puzzle or there is no scramble to show.
pub fn url(solve: &Solve) -> Option<String> {
    let puzzle = puzzle(solve.event())?;
    let scramble = solve.scramble().trim();
    if scramble.is_empty() {
        return None;
    }
    let comment = solve.comment().trim();
    let title = if comment.is_empty() {
        solve.to_string()
    } else {
        format!("{solve} - {comment}")
    };
    let mut link = format!(
        "{SITE}?title={}&puzzle={}&type=reconstruction&setup={}",
        percent_encode(&title),
        percent_encode(puzzle),
        percent_encode(&escape_alg(scramble)),
    );
    let solution = solve.solution().trim();
    if !solution.is_empty() {
        let _ = write!(link, "&alg={}", percent_encode(&escape_alg(solution)));
    }
    Some(link)
}

/// Rewrites moves the way the site's `escape_alg` does, in the same order.
///
/// `'` becomes `-` and a space `_`, which keeps links readable, so a real `-`,
/// `_` or `+` has to be spelled out or it would come back as something else.
/// That matters in `//` comments: moves themselves have none of the three.
/// The site reads only `setup` and `alg` this way, never `title`.
fn escape_alg(alg: &str) -> String {
    alg.replace("\r\n", "\n")
        .replace('_', "&#95;")
        .replace(' ', "_")
        .replace('+', "&#2b;")
        .replace('-', "&#45;")
        .replace('\'', "-")
}

/// Encodes every byte a query value cannot carry as it is, so `&`, `#`, `%`,
/// newlines and non-ASCII text all arrive unchanged.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::Modifier;

    fn solve(event: WcaEvent, scramble: &str, comment: &str, solution: &str) -> Solve {
        let mut solve = Solve::new_with_meta(
            12_340,
            event,
            scramble.to_owned(),
            1,
            Modifier::None,
            comment.to_owned(),
        );
        solve.set_solution(solution.to_owned());
        solve
    }

    fn percent_decode(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut decoded = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'%' {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap();
                decoded.push(u8::from_str_radix(hex, 16).unwrap());
                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
        String::from_utf8(decoded).unwrap()
    }

    /// What the site's `unescape_alg` does, statement by statement.
    fn unescape_alg(text: &str) -> String {
        text.replace('-', "'")
            .replace("&#45;", "-")
            .replace('+', " ")
            .replace("&#2b;", "+")
            .replace('_', " ")
            .replace("&#95;", "_")
    }

    /// Reads one parameter back the way the site's router does.
    fn param(link: &str, name: &str) -> Option<String> {
        let (_, query) = link.split_once('?').unwrap();
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
            .map(percent_decode)
    }

    #[test]
    fn a_scramble_keeps_the_sites_readable_shape() {
        let link = url(&solve(WcaEvent::Cube3x3, "R U' F2", "", "")).unwrap();
        assert_eq!(
            link,
            "https://alg.cubing.net/?title=12.340&puzzle=3x3x3&type=reconstruction&setup=R_U-_F2"
        );
    }

    #[test]
    fn every_cube_has_a_link_and_nothing_else_does() {
        for event in WcaEvent::ALL {
            let link = url(&solve(event, "R U", "", ""));
            // The site's name for the puzzle the event is played on.
            let site = match event {
                WcaEvent::Cube2x2 => Some(WcaEvent::Cube2x2),
                WcaEvent::Cube3x3 | WcaEvent::Cube3x3OneHanded | WcaEvent::Cube3x3Blind => {
                    Some(WcaEvent::Cube3x3)
                }
                WcaEvent::Cube4x4 | WcaEvent::Cube4x4Blind => Some(WcaEvent::Cube4x4),
                WcaEvent::Cube5x5 | WcaEvent::Cube5x5Blind => Some(WcaEvent::Cube5x5),
                WcaEvent::Cube6x6 => Some(WcaEvent::Cube6x6),
                WcaEvent::Cube7x7 => Some(WcaEvent::Cube7x7),
                _ => None,
            };
            assert_eq!(link.is_some(), site.is_some(), "{event:?}");
            if let (Some(link), Some(site)) = (link, site) {
                assert_eq!(param(&link, "puzzle").as_deref(), Some(site.name()));
            }
        }
    }

    #[test]
    fn a_blank_scramble_has_no_link() {
        assert_eq!(url(&solve(WcaEvent::Cube3x3, "  ", "", "R U")), None);
    }

    #[test]
    fn wide_moves_reach_the_site_unchanged() {
        let scramble = "3Rw2 Uw' 3Fw D2";
        let link = url(&solve(WcaEvent::Cube7x7, scramble, "", "")).unwrap();
        assert_eq!(unescape_alg(&param(&link, "setup").unwrap()), scramble);
    }

    #[test]
    fn a_solution_with_comments_comes_back_exactly() {
        let solution = "y x' // inspection_1 + a-b\nR U R' // 100% & \"done\" #1?\nU'";
        let link = url(&solve(WcaEvent::Cube3x3, "R", "", solution)).unwrap();
        assert_eq!(unescape_alg(&param(&link, "alg").unwrap()), solution);
    }

    #[test]
    fn windows_line_ends_in_a_solution_become_newlines() {
        let link = url(&solve(WcaEvent::Cube3x3, "R", "", "R // a\r\nU")).unwrap();
        assert_eq!(unescape_alg(&param(&link, "alg").unwrap()), "R // a\nU");
    }

    #[test]
    fn an_awkward_comment_stays_in_the_title() {
        let comment = "a&b=c #1 100% + 'q' _x_ -y- /z? ¿ñ? 世界 مرحبا\nline";
        let link = url(&solve(WcaEvent::Cube3x3, "R", comment, "")).unwrap();
        // The title is not an alg: the site shows it as it arrives.
        assert_eq!(
            param(&link, "title").unwrap(),
            format!("12.340 - {comment}")
        );
        assert_eq!(param(&link, "setup").as_deref(), Some("R"));
    }

    #[test]
    fn the_link_is_plain_ascii_with_one_query() {
        let link = url(&solve(WcaEvent::Cube3x3, "R", "? # & 世界", "U // é")).unwrap();
        assert!(link.is_ascii());
        assert!(!link.contains(['#', ' ', '\n']));
        assert_eq!(link.matches('?').count(), 1);
        assert_eq!(link.matches('&').count(), 4);
    }

    #[test]
    fn the_title_shows_the_penalty() {
        let mut plus_two = solve(WcaEvent::Cube3x3, "R", "", "");
        let mut after = plus_two.snapshot();
        after.modifier = Modifier::PlusTwo;
        plus_two.edit(after);
        let link = url(&plus_two).unwrap();
        assert_eq!(param(&link, "title").as_deref(), Some("14.340+"));
    }
}
