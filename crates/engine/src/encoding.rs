//! Turning a text file's bytes into text, whatever its encoding (EXT-2).

use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};

/// How much of a file the legacy-encoding guess reads. Plenty to tell
/// Arabic from Cyrillic or Western European text; reading more only costs.
const GUESS_FROM: usize = 1 << 20;

/// Text from bytes in whatever encoding they are in. In order: a byte-order
/// mark decides; then UTF-16 without a mark, told by its zero bytes; then
/// UTF-8, if the bytes are valid UTF-8; then the likeliest legacy encoding,
/// such as Windows-1252 or Windows-1256, guessed from the bytes.
/// Bytes that do not fit the encoding become U+FFFD; nothing is refused.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some((encoding, mark)) = Encoding::for_bom(bytes) {
        return encoding
            .decode_without_bom_handling(&bytes[mark..])
            .0
            .into_owned();
    }
    // Before UTF-8: zero bytes are valid UTF-8, so UTF-16 holding only
    // ASCII letters would pass for it.
    if let Some(encoding) = utf16_without_mark(bytes) {
        return encoding.decode_without_bom_handling(bytes).0.into_owned();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    // ISO-2022-JP is considered, as chardetng always did before 1.0: its
    // warning is for web pages that run scripts, and document text is shown
    // as plain text only.
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Allow);
    let sample = &bytes[..bytes.len().min(GUESS_FROM)];
    detector.feed(sample, sample.len() == bytes.len());
    // Not UTF-8: that was ruled out above.
    let encoding = detector.guess(None, Utf8Detection::Deny);
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

/// UTF-16 written without a byte-order mark: text in Latin script has a
/// zero in every other byte. Little-endian puts it second, big-endian first.
fn utf16_without_mark(bytes: &[u8]) -> Option<&'static Encoding> {
    let sample = &bytes[..bytes.len().min(4096) & !1];
    let pairs = sample.len() / 2;
    if pairs < 8 {
        return None;
    }
    let zeros_at = |offset: usize| {
        sample
            .iter()
            .skip(offset)
            .step_by(2)
            .filter(|&&byte| byte == 0)
            .count()
    };
    let (first, second) = (zeros_at(0), zeros_at(1));
    // Most pairs hold a zero on one side and almost none on the other.
    if second * 10 >= pairs * 3 && first * 20 <= pairs {
        Some(UTF_16LE)
    } else if first * 10 >= pairs * 3 && second * 20 <= pairs {
        Some(UTF_16BE)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use encoding_rs::{WINDOWS_1251, WINDOWS_1252, WINDOWS_1256};

    const FRENCH: &str = "Le préavis de résiliation est de trois mois. Le locataire \
        doit envoyer une lettre recommandée avant la fin du trimestre, et le \
        propriétaire répond dans les quinze jours. Les clés sont rendues à l'état \
        des lieux, où l'on vérifie les dégâts éventuels.";
    const ARABIC: &str = "مدة الإشعار لإنهاء عقد الإيجار ثلاثة أشهر. يجب على المستأجر \
        إرسال رسالة مسجلة قبل نهاية الربع، ويرد المالك خلال خمسة عشر يوما. تعاد \
        المفاتيح عند معاينة الشقة.";
    const RUSSIAN: &str = "Срок уведомления о расторжении договора аренды составляет \
        три месяца. Арендатор должен отправить заказное письмо до конца квартала, \
        а владелец отвечает в течение пятнадцати дней.";

    #[test]
    fn utf8_with_or_without_a_mark_is_read_as_it_is() {
        assert_eq!(decode_text(FRENCH.as_bytes()), FRENCH);
        let marked = [b"\xEF\xBB\xBF".as_slice(), ARABIC.as_bytes()].concat();
        assert_eq!(decode_text(&marked), ARABIC);
    }

    #[test]
    fn utf16_is_read_with_or_without_a_mark() {
        let little: Vec<u8> = FRENCH.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let big: Vec<u8> = FRENCH.encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert_eq!(
            decode_text(&[b"\xFF\xFE".as_slice(), &little].concat()),
            FRENCH
        );
        assert_eq!(
            decode_text(&[b"\xFE\xFF".as_slice(), &big].concat()),
            FRENCH
        );
        // Without a mark, as some tools write it; plain ASCII too.
        assert_eq!(decode_text(&little), FRENCH);
        assert_eq!(decode_text(&big), FRENCH);
        let ascii: Vec<u8> = "plain notes, nothing more"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(decode_text(&ascii), "plain notes, nothing more");
    }

    #[test]
    fn legacy_encodings_are_recognised() {
        for (text, encoding) in [
            (FRENCH, WINDOWS_1252),
            (ARABIC, WINDOWS_1256),
            (RUSSIAN, WINDOWS_1251),
        ] {
            let (bytes, _, lossy) = encoding.encode(text);
            assert!(!lossy, "{}", encoding.name());
            assert_eq!(decode_text(&bytes), text, "{}", encoding.name());
        }
    }

    #[test]
    fn odd_bytes_never_fail() {
        assert_eq!(decode_text(b""), "");
        let text = decode_text(&[0xFF, 0x00, 0xC3, 0x28, 0x80]);
        assert!(!text.is_empty());
    }
}
