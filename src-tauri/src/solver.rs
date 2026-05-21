use sha1::{Digest, Sha1};

pub struct Challenge {
    pub token: String,
    pub check_offset: usize,
}

pub fn parse_challenge(html: &str) -> Option<Challenge> {
    let token = extract_js_array(html, 2)?;
    if token.len() < 2 {
        return None;
    }
    let check_offset = usize::from_str_radix(&token[0..1], 16).ok()?;
    Some(Challenge {
        token,
        check_offset,
    })
}

pub fn solve(challenge: &Challenge) -> u64 {
    let mut i: u64 = 0;
    loop {
        let input = format!("{}{}", challenge.token, i);
        let digest = Sha1::digest(input.as_bytes());
        if digest[challenge.check_offset] == 0xB0
            && digest[challenge.check_offset + 1] == 0x0B
        {
            return i;
        }
        i += 1;
    }
}

pub fn build_challenge_cookie(challenge: &Challenge, solution: u64, elapsed_ms: u64) -> String {
    format!(
        "c_token={}{}; c_time={:.3}; remix_userkey=a097500143c397d1c09c8c4c459bb142; remix_userid=35246529; selectedSiteMode=books",
        challenge.token,
        solution,
        elapsed_ms as f64 / 1000.0
    )
}

fn extract_js_array(html: &str, index: usize) -> Option<String> {
    let marker = "const a0_0x2a54=['";
    let start = html.find(marker)? + marker.len();
    let remaining = &html[start..];
    let array_end = remaining.find("'];")?;
    let array_str = &remaining[..array_end];

    let items: Vec<&str> = array_str
        .split("','")
        .map(|s| s.trim_matches('\''))
        .collect();

    if items.len() != 3 {
        return None;
    }

    let shuffle_str = &remaining[array_end..];
    let shuffle_count = parse_shuffle_count(shuffle_str)?;

    let rotated_index = (index + shuffle_count) % 3;
    Some(items[rotated_index].to_string())
}

fn parse_shuffle_count(s: &str) -> Option<usize> {
    let marker = "_0x4457dc(++_0x2a548e);}(a0_0x2a54,";
    let pos = s.find(marker)?;
    let hex_str = &s[pos + marker.len()..];
    let end = hex_str.find(')')?;
    let hex_val = &hex_str[..end].trim().trim_start_matches("0x");
    usize::from_str_radix(hex_val, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_solve_from_html() {
        let html = include_str!("../challenge_full.html");
        let challenge = parse_challenge(&html).expect("parse challenge");
        eprintln!("token={}, offset={}", challenge.token, challenge.check_offset);

        let solution = solve(&challenge);
        eprintln!("solution={}", solution);

        let input = format!("{}{}", challenge.token, solution);
        let digest = sha1::Sha1::digest(input.as_bytes());
        eprintln!("digest[{}]={:02x}, digest[{}]={:02x}",
            challenge.check_offset, digest[challenge.check_offset],
            challenge.check_offset + 1, digest[challenge.check_offset + 1]);

        assert_eq!(digest[challenge.check_offset], 0xB0);
        assert_eq!(digest[challenge.check_offset + 1], 0x0B);
    }

    #[test]
    fn test_js_array_extract() {
        let html = include_str!("../challenge_full.html");

        let token = extract_js_array(&html, 0).expect("idx 0");
        let array_method = extract_js_array(&html, 1).expect("idx 1");
        let token_hex = extract_js_array(&html, 2).expect("idx 2");

        eprintln!("idx0={token}");
        eprintln!("idx1={array_method}");
        eprintln!("idx2={token_hex}");

        assert_eq!(token, "c_token=");
        assert_eq!(array_method, "array");
        assert!(!token_hex.is_empty());
        assert!(token_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }
}