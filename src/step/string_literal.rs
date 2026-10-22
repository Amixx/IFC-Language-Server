//! Decoding of the content of STEP string literals (ISO 10303-21 §6.4.3).

/// The upper halves (0xA0–0xFF) of ISO 8859-2 to 8859-9, which `\PB\` to `\PI\` select;
/// unassigned codes are U+FFFD.
const PAGES: [&str; 8] = [
    // B: ISO 8859-2
    "\u{a0}Ą˘Ł¤ĽŚ§¨ŠŞŤŹ\u{ad}ŽŻ°ą˛ł´ľśˇ¸šşťź˝žżŔÁÂĂÄĹĆÇČÉĘËĚÍÎĎĐŃŇÓÔŐÖ×ŘŮÚŰÜÝŢßŕáâăäĺćçčéęëěíîďđńňóôőö÷řůúűüýţ˙",
    // C: ISO 8859-3
    "\u{a0}Ħ˘£¤\u{fffd}Ĥ§¨İŞĞĴ\u{ad}\u{fffd}Ż°ħ²³´µĥ·¸ışğĵ½\u{fffd}żÀÁÂ\u{fffd}ÄĊĈÇÈÉÊËÌÍÎÏ\u{fffd}ÑÒÓÔĠÖ×ĜÙÚÛÜŬŜßàáâ\u{fffd}äċĉçèéêëìíîï\u{fffd}ñòóôġö÷ĝùúûüŭŝ˙",
    // D: ISO 8859-4
    "\u{a0}ĄĸŖ¤ĨĻ§¨ŠĒĢŦ\u{ad}Ž¯°ą˛ŗ´ĩļˇ¸šēģŧŊžŋĀÁÂÃÄÅÆĮČÉĘËĖÍÎĪĐŅŌĶÔÕÖ×ØŲÚÛÜŨŪßāáâãäåæįčéęëėíîīđņōķôõö÷øųúûüũū˙",
    // E: ISO 8859-5
    "\u{a0}ЁЂЃЄЅІЇЈЉЊЋЌ\u{ad}ЎЏАБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдежзийклмнопрстуфхцчшщъыьэюя№ёђѓєѕіїјљњћќ§ўџ",
    // F: ISO 8859-6
    "\u{a0}\u{fffd}\u{fffd}\u{fffd}¤\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}،\u{ad}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}؛\u{fffd}\u{fffd}\u{fffd}؟\u{fffd}ءآأؤإئابةتثجحخدذرزسشصضطظعغ\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}ـفقكلمنهوىي\u{64b}\u{64c}\u{64d}\u{64e}\u{64f}\u{650}\u{651}\u{652}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}",
    // G: ISO 8859-7
    "\u{a0}‘’£€₯¦§¨©ͺ«¬\u{ad}\u{fffd}―°±²³΄΅Ά·ΈΉΊ»Ό½ΎΏΐΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡ\u{fffd}ΣΤΥΦΧΨΩΪΫάέήίΰαβγδεζηθικλμνξοπρςστυφχψωϊϋόύώ\u{fffd}",
    // H: ISO 8859-8
    "\u{a0}\u{fffd}¢£¤¥¦§¨©×«¬\u{ad}®¯°±²³´µ¶·¸¹÷»¼½¾\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}‗אבגדהוזחטיךכלםמןנסעףפץצקרשת\u{fffd}\u{fffd}\u{200e}\u{200f}\u{fffd}",
    // I: ISO 8859-9
    "\u{a0}¡¢£¤¥¦§¨©ª«¬\u{ad}®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏĞÑÒÓÔÕÖ×ØÙÚÛÜİŞßàáâãäåæçèéêëìíîïğñòóôõö÷øùúûüışÿ",
];

/// Decodes the content between a string literal's quotes: `''`, `\\`, `\S\`, `\P?\`, `\X\hh`,
/// `\X2\…\X0\` and `\X4\…\X0\`. Malformed directives are kept as written.
pub fn decode_string(content: &str) -> String {
    let mut decoded = String::with_capacity(content.len());
    let mut page = b'A';
    let mut rest = content;
    while let Some(index) = rest.find(['\\', '\'']) {
        decoded.push_str(&rest[..index]);
        rest = &rest[index..];
        let consumed = decode_escape(rest, &mut page, &mut decoded).unwrap_or_else(|| {
            decoded.push_str(&rest[..1]);
            1
        });
        rest = &rest[consumed..];
    }
    decoded.push_str(rest);
    decoded
}

/// Decodes the escape at the start of `text` into `decoded`, returning its length in bytes.
fn decode_escape(text: &str, page: &mut u8, decoded: &mut String) -> Option<usize> {
    let bytes = text.as_bytes();
    if text.starts_with("''") {
        decoded.push('\'');
        Some(2)
    } else if text.starts_with(r"\\") {
        decoded.push('\\');
        Some(2)
    } else if let Some(rest) = text.strip_prefix(r"\S\") {
        let code = *rest.as_bytes().first()?;
        decoded.push(page_char(*page, code)?);
        Some(4)
    } else if text.starts_with(r"\P") && bytes.get(3) == Some(&b'\\') {
        let selected = bytes[2];
        if !(b'A'..=b'I').contains(&selected) {
            return None;
        }
        *page = selected;
        Some(4)
    } else if let Some(rest) = text.strip_prefix(r"\X2\") {
        let (units, length) = hex_groups(rest, 4)?;
        let units = units.into_iter().map(|unit| unit as u16);
        decoded.extend(char::decode_utf16(units).map(|unit| unit.unwrap_or('\u{fffd}')));
        Some(4 + length)
    } else if let Some(rest) = text.strip_prefix(r"\X4\") {
        let (units, length) = hex_groups(rest, 8)?;
        let chars: Option<Vec<char>> = units.into_iter().map(char::from_u32).collect();
        decoded.extend(chars?);
        Some(4 + length)
    } else if let Some(rest) = text.strip_prefix(r"\X\") {
        let code = u8::from_str_radix(rest.get(..2)?, 16).ok()?;
        decoded.push(char::from(code));
        Some(5)
    } else {
        None
    }
}

/// The character `\S\` followed by `code` stands for in the ISO 8859 part `page` selects.
fn page_char(page: u8, code: u8) -> Option<char> {
    let index = usize::from(code.checked_sub(0x20)?);
    if index >= 96 {
        return None;
    }
    match page {
        b'A' => Some(char::from(code + 0x80)),
        _ => PAGES[usize::from(page - b'B')].chars().nth(index),
    }
}

/// The values of the `digits`-digit hex groups at the start of `text` up to the `\X0\` that ends
/// them, with the length of the groups and terminator in bytes.
fn hex_groups(text: &str, digits: usize) -> Option<(Vec<u32>, usize)> {
    let mut values = Vec::new();
    let mut offset = 0;
    loop {
        let rest = &text[offset..];
        if rest.starts_with(r"\X0\") {
            return Some((values, offset + 4));
        }
        let group = rest.get(..digits)?;
        if !group.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        values.push(u32::from_str_radix(group, 16).ok()?);
        offset += digits;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_doubled_apostrophes_and_backslashes() {
        assert_eq!(decode_string(r"it''s a \\ b"), r"it's a \ b");
    }

    #[test]
    fn decodes_latin_1_by_default() {
        assert_eq!(decode_string(r"D\S\|sseldorf \S\\"), "Düsseldorf Ü");
        assert_eq!(decode_string(r"M\X\FCnchen"), "München");
    }

    #[test]
    fn page_directives_select_the_iso_8859_part() {
        assert_eq!(decode_string(r"\PE\\S\?\S\`\PA\\S\D"), "ПрÄ");
        assert_eq!(decode_string(r"\PB\\S\9"), "š");
        assert_eq!(decode_string(r"\PG\\S\a"), "α");
    }

    #[test]
    fn decodes_two_and_four_byte_groups() {
        assert_eq!(decode_string(r"M\X2\00FC\X0\nchen"), "München");
        assert_eq!(decode_string(r"\X2\041F04400438\X0\"), "При");
        assert_eq!(decode_string(r"\X2\D83DDE00\X0\"), "😀");
        assert_eq!(decode_string(r"\X4\0001F600\X0\"), "😀");
    }

    #[test]
    fn keeps_malformed_directives() {
        for text in [
            r"a\b",
            r"\X2\00F\X0\",
            r"\X2\00FC",
            r"\X\G1",
            r"\PZ\",
            r"\S\",
            r"\X4\FFFFFFFF\X0\",
        ] {
            assert_eq!(decode_string(text), text);
        }
    }
}
