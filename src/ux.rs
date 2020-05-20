/*
 * Set of utilities to convert or format data to make them user-friendlier
 */
use std::time::Duration;

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

const FILLER: char = '…';

/// Truncates a string to be at most `width` in terms of display width.
/// If the string is longer, it is truncated from left end and ellipsis
/// is added to its end. Width of the result string that includes
/// ellipsis does not exceed `width`.
///
/// # Examples
///
/// let s = fade_str_left("abc", 1);
/// assert_eq!("…".to_string(), s);
/// let s = fade_str_left("abc", 2);
/// assert_eq!("…c".to_string(), s);
pub(crate) fn fade_str_left(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut curr_width = s.width();
    if curr_width <= width {
        return s.to_string();
    }

    curr_width += 1;
    let mut found = false;
    for (bidx, c) in s.char_indices() {
        curr_width -= c.width().unwrap_or(0);
        if found {
            return format!("{}{}", FILLER, s.get(bidx..).unwrap());
        }
        if curr_width <= width {
            // mark position and move to the next character
            found = true;
        }
    }
    FILLER.to_string()
}

// Converts value in KB to string of maximum length of 4 characters.
// Do its best to display as much info as possible.
pub(crate) fn format_bytes(val: u64) -> String {
    if val < 1000 {
        return format!("{}K", val);
    }
    const COEFF: [char; 4] = ['M', 'G', 'T', 'P'];
    let mut idx = 0usize;
    let mut val = val as f32 / 1024.0;
    while idx < 4 {
        if val < 9.5 {
            return format!("{:.2}{}", val, COEFF[idx]);
        }
        if val < 99.5 {
            return format!("{:.1}{}", val, COEFF[idx]);
        }
        if val < 999.5 {
            return format!("{:.0}{}", val, COEFF[idx]);
        }
        val /= 1024.0;
        idx += 1
    }
    "!!!!!".to_string()
}

pub(crate) fn round_to_hundred(v: u64) -> u64 {
    if v % 100 == 0 {
        return v;
    }
    let h = v / 100 + 1;
    h * 100
}

pub(crate) fn format_duration(d: Duration) -> String {
    let sec = d.as_secs();
    if sec < 60 {
        return format!("{}s", sec);
    }
    let m = sec / 60;
    let sec = sec % 60;
    if m < 60 {
        if sec == 0 {
            return format!("{}m", m);
        } else {
            return format!("{}m{}s", m, sec);
        }
    }
    let h = m / 60;
    let m = m % 60;
    if h < 24 {
        if m == 0 {
            return format!("{}h", h);
        } else {
            return format!("{}h{}m", h, m);
        }
    }
    let days = h / 24;
    let h = h % 24;
    if h == 0 {
        format!("{}d", days)
    } else {
        format!("{}d{}h", days, h)
    }
}

// Converts difference in KB to string..
pub(crate) fn format_diff(val: i64) -> String {
    let sgn = if val < 0 { '-' } else { '+' };
    let val = if val < 0 { -val } else { val };
    if val == 0 {
        return "0K".to_string();
    }
    if val < 1000 {
        return format!("{}{}K", sgn, val);
    }
    const COEFF: [char; 4] = ['M', 'G', 'T', 'P'];
    let mut idx = 0usize;
    let mut val = val as f32 / 1024.0;
    while idx < 4 {
        if val < 999.5 {
            return format!("{}{}{}", sgn, val.round() as u64, COEFF[idx]);
        }
        val /= 1024.0;
        idx += 1
    }
    format!("{}!!!!", sgn)
}

pub(crate) fn short_round(val: u64, down: bool) -> (u64, u64) {
    if val < 1024 {
        return (val, 1);
    }
    let mut coef = 1024;
    let delta = if val % 1024 == 0 { 0 } else { 1 };
    let mut val = val / 1024;
    if !down && delta != 0 {
        val += 1;
    }
    while val >= 1024 {
        coef *= 1024;
        let delta = if val % 1024 == 0 { 0 } else { 1 };
        val /= 1024;
        if !down {
            val += delta
        };
    }
    (val, coef)
}

// Converts value in KB to string of maximum length of 4 characters.
// Do its best to display as much info as possible.
pub(crate) fn format_mem(val: u64) -> String {
    if val < 1024 {
        return format!("{}K", val);
    }
    const COEFF: [char; 4] = ['M', 'G', 'T', 'P'];
    let mut idx = 0usize;
    let mut val = val / 1024;
    while idx < 4 {
        if val < 1024 {
            return format!("{}{}", val, COEFF[idx]);
        }
        val /= 1024;
        idx += 1
    }
    "!!!!!".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_left() {
        let s = fade_str_left("", 10);
        assert_eq!("".to_string(), s);
        let s = fade_str_left("a", 10);
        assert_eq!("a".to_string(), s);
        let s = fade_str_left("abc", 1);
        assert_eq!("…".to_string(), s);
        let s = fade_str_left("thañispa", 3);
        assert_eq!("…pa".to_string(), s);
        let s = fade_str_left("thañispa", 6);
        assert_eq!("…ñispa".to_string(), s);
        let s = fade_str_left("thañispa", 8);
        assert_eq!("thañispa".to_string(), s);
    }

    #[test]
    fn bytes_fmt() {
        let vals: [u64; 10] = [0, 67, 876, 1_000, 1_056, 2_048, 7_865, 784_670, 2_200_900, 7_777_555_444_222_333];
        let ress: [&str; 10] = ["0K", "67K", "876K", "0.98M", "1.03M", "2.00M", "7.68M", "766M", "2.10G", "!!!!!"];
        for idx in 0..10usize {
            let r = format_bytes(vals[idx]);
            assert_eq!(&r, ress[idx]);
        }
    }

    #[test]
    fn duration_fmt() {
        let vals: [u64; 12] = [0, 23, 60, 176, 360, 7200, 7320, 7345, 345_600, 352_800, 352_860, 352_869];
        let ress: [&str; 12] = ["0s", "23s", "1m", "2m56s", "6m", "2h", "2h2m", "2h2m", "4d", "4d2h", "4d2h", "4d2h"];
        for idx in 0..12usize {
            let r = format_duration(Duration::from_secs(vals[idx]));
            assert_eq!(ress[idx], &r);
        }
    }

    #[test]
    fn round_100() {
        let vals: [u64; 5] = [0, 30, 110, 200, 399];
        let ress: [u64; 5] = [0, 100, 200, 200, 400];
        for (idx, v) in vals.iter().enumerate() {
            let r = round_to_hundred(*v);
            assert_eq!(r, ress[idx]);
        }
    }

    #[test]
    fn diff_fmt() {
        let vals: [i64; 10] = [0, 67, 876, 1_000, 1_056, 2_048, 7_865, 784_670, 2_200_900, 7_777_555_444_222_333];
        let resp: [&str; 10] = ["0K", "+67K", "+876K", "+1M", "+1M", "+2M", "+8M", "+766M", "+2G", "+!!!!"];
        let resn: [&str; 10] = ["0K", "-67K", "-876K", "-1M", "-1M", "-2M", "-8M", "-766M", "-2G", "-!!!!"];
        for idx in 0..10usize {
            let r = format_diff(vals[idx]);
            assert_eq!(&r, resp[idx]);
            let r = format_diff(-vals[idx]);
            assert_eq!(&r, resn[idx]);
        }
    }

    #[test]
    fn short_round_test() {
        let vals: [u64; 9] = [0, 67, 876, 1_000, 1_056, 2_048, 7_865, 784_670, 2_200_900];
        let ress_v: [u64; 9] = [0, 67, 876, 1_000, 1, 2, 7, 766, 2];
        let ress_v1: [u64; 9] = [0, 67, 876, 1_000, 2, 2, 8, 767, 3];
        let ress_m: [u64; 9] = [1, 1, 1, 1, 1024, 1024, 1024, 1024, 1024 * 1024];
        for idx in 0..9usize {
            let (v, m) = short_round(vals[idx], true);
            let (v1, m1) = short_round(vals[idx], false);
            assert_eq!(v, ress_v[idx]);
            assert_eq!(m, ress_m[idx]);
            assert_eq!(v1, ress_v1[idx]);
            assert_eq!(m1, ress_m[idx]);
        }
    }
}
