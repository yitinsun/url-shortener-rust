const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
const BASE: u64 = 62;

/// Encode a non-negative id. `encode(0)` is `"0"`.
pub fn encode(id: u64) -> String {
    if id == 0 {
        return "0".to_string();
    }
    let mut value = id;
    let mut bytes = Vec::new();
    while value > 0 {
        let remainder = (value % BASE) as usize;
        bytes.push(ALPHABET[remainder]);
        value /= BASE;
    }
    bytes.reverse();
    String::from_utf8(bytes).expect("alphabet is ASCII")
}

/// Decode a Base62 string. Fails on an empty value or a character outside the alphabet.
pub fn decode(short_code: &str) -> Result<u64, String> {
    if short_code.is_empty() {
        return Err("shortCode must not be blank".to_string());
    }
    let mut total: u128 = 0;
    for ch in short_code.chars() {
        let index = ALPHABET
            .iter()
            .position(|byte| *byte as char == ch)
            .ok_or_else(|| format!("Invalid Base62 character: {ch}"))?;
        total = total
            .checked_mul(BASE as u128)
            .and_then(|value| value.checked_add(index as u128))
            .ok_or_else(|| "Base62 value overflows u64".to_string())?;
        if total > u64::MAX as u128 {
            return Err("Base62 value overflows u64".to_string());
        }
    }
    Ok(total as u64)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn pivot_id_is_five_to_seven_alphanumeric_chars() {
        let encoded = encode(56_800_235);
        assert!(encoded.len() >= 5 && encoded.len() <= 7, "{encoded}");
        assert!(encoded
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric()));
        assert!(!encoded.contains('+') && !encoded.contains('/'));
        assert_eq!(decode(&encoded).unwrap(), 56_800_235);
    }

    #[test]
    fn alphabet_excludes_base64_symbols() {
        for id in [1_u64, 61, 62, 63, 1_000_000, 56_800_235] {
            let encoded = encode(id);
            assert!(!encoded.contains('+'));
            assert!(!encoded.contains('/'));
        }
    }

    #[test]
    fn max_i64_round_trips_without_overflow() {
        let id = i64::MAX as u64;
        let encoded = encode(id);
        assert_eq!(decode(&encoded).unwrap(), id);
    }

    #[test]
    fn random_positive_ids_round_trip() {
        let mut state = 0x1234_5678_9abc_def0_u64;
        for _ in 0..32 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let id = state & (i64::MAX as u64);
            assert_eq!(decode(&encode(id)).unwrap(), id);
        }
    }

    #[test]
    fn zero_encodes_as_zero() {
        assert_eq!(encode(0), "0");
        assert_eq!(decode("0").unwrap(), 0);
    }

    #[test]
    fn decode_rejects_blank_unknown_and_overflowing_values() {
        assert!(decode("").is_err());
        assert!(decode("abc-").is_err());
        assert!(decode(&"z".repeat(40)).is_err());
    }
}
