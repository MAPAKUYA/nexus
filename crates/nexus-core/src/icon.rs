//! Иконки приложений: перевод пикселей Windows в PNG и base64 для интерфейса.
//!
//! Всё написано без внешних зависимостей: PNG собирается из «несжатых» блоков zlib
//! (иконка 32×32 занимает около 4 КБ), проверка целостности — CRC32 и Adler32.

/// Самая большая сторона иконки, которую мы принимаем (настоящие — 16…64 px).
pub const MAX_ICON_SIDE: u32 = 128;
/// Верхняя граница размера PNG в базе — защита от мусора.
pub const MAX_ICON_BYTES: usize = 64 * 1024;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}
static CRC_TABLE: [u32; 256] = crc_table();

pub fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |c, &b| CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8))
}

pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// PNG (8 бит, RGBA, без сжатия) из пикселей `rgba` размером `width × height`.
/// `None`, если размеры нулевые, слишком большие или не совпадают с длиной данных.
pub fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > MAX_ICON_SIDE || height > MAX_ICON_SIDE {
        return None;
    }
    let row = width as usize * 4;
    if rgba.len() != row * height as usize {
        return None;
    }
    // Каждая строка начинается с байта фильтра 0 («без фильтра»).
    let mut raw = Vec::with_capacity((row + 1) * height as usize);
    for line in rgba.chunks(row) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    // zlib: заголовок 0x78 0x01, затем блоки «stored» не длиннее 65535 байт, затем Adler32.
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(block) = blocks.next() {
        z.push(if blocks.peek().is_none() { 1 } else { 0 });
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8 бит на канал, RGBA, без интерлейса

    let mut png = PNG_SIGNATURE.to_vec();
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &z);
    push_chunk(&mut png, b"IEND", &[]);
    Some(png)
}

/// Похоже ли содержимое на наш PNG (сигнатура и разумный размер). Проверка перед записью в базу.
pub fn looks_like_png(bytes: &[u8]) -> bool {
    bytes.len() >= 33 && bytes.len() <= MAX_ICON_BYTES && bytes.starts_with(&PNG_SIGNATURE)
}

/// Пиксели Windows (B, G, R, A на точку) → RGBA.
///
/// Старые иконки хранят прозрачность не в альфа-канале, а отдельной маской (`mask`, тоже 32 бита
/// на точку: чёрный — непрозрачно, белый — прозрачно). Если во всём цветном слое альфа нулевая,
/// используется маска; без маски такая иконка считается полностью непрозрачной.
pub fn bgra_to_rgba(bgra: &[u8], mask: Option<&[u8]>) -> Vec<u8> {
    let has_alpha = bgra.chunks_exact(4).any(|p| p[3] != 0);
    let mask = mask.filter(|m| m.len() == bgra.len());
    bgra.chunks_exact(4)
        .enumerate()
        .flat_map(|(i, p)| {
            let a = if has_alpha {
                p[3]
            } else if let Some(m) = mask {
                if m[i * 4] == 0 { 255 } else { 0 }
            } else {
                255
            };
            [p[2], p[1], p[0], a]
        })
        .collect()
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 (RFC 4648, с дополнением `=`) — для вставки PNG в `data:`-адрес.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Минимальный разборщик PNG из наших «stored»-блоков: проверяет сигнатуру, CRC каждого блока,
    /// Adler32 и возвращает (ширина, высота, пиксели RGBA).
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert!(png.starts_with(&PNG_SIGNATURE));
        let (mut pos, mut ihdr, mut idat, mut ended) = (8, Vec::new(), Vec::new(), false);
        while pos < png.len() {
            let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
            let kind = &png[pos + 4..pos + 8];
            let data = &png[pos + 8..pos + 8 + len];
            let crc = u32::from_be_bytes(png[pos + 8 + len..pos + 12 + len].try_into().unwrap());
            assert_eq!(crc, crc32(&png[pos + 4..pos + 8 + len]), "CRC блока {:?}", std::str::from_utf8(kind));
            match kind {
                b"IHDR" => ihdr = data.to_vec(),
                b"IDAT" => idat.extend_from_slice(data),
                b"IEND" => ended = true,
                _ => panic!("лишний блок"),
            }
            pos += 12 + len;
        }
        assert!(ended && pos == png.len(), "IEND последний, мусора после него нет");
        assert_eq!(&ihdr[8..], &[8, 6, 0, 0, 0]);
        let (w, h) = (u32::from_be_bytes(ihdr[0..4].try_into().unwrap()), u32::from_be_bytes(ihdr[4..8].try_into().unwrap()));
        assert_eq!(&idat[..2], &[0x78, 0x01]);
        assert_eq!(((idat[0] as u32) << 8 | idat[1] as u32) % 31, 0, "контрольная сумма заголовка zlib");
        let (mut raw, mut p, mut last) = (Vec::new(), 2, false);
        while !last {
            last = idat[p] & 1 == 1;
            assert_eq!(idat[p] & 0b110, 0, "только stored-блоки");
            let len = u16::from_le_bytes([idat[p + 1], idat[p + 2]]);
            assert_eq!(u16::from_le_bytes([idat[p + 3], idat[p + 4]]), !len);
            raw.extend_from_slice(&idat[p + 5..p + 5 + len as usize]);
            p += 5 + len as usize;
        }
        assert_eq!(u32::from_be_bytes(idat[p..p + 4].try_into().unwrap()), adler32(&raw));
        assert_eq!(p + 4, idat.len());
        let row = w as usize * 4;
        assert_eq!(raw.len(), (row + 1) * h as usize);
        let mut pixels = Vec::new();
        for line in raw.chunks(row + 1) {
            assert_eq!(line[0], 0, "фильтр 0");
            pixels.extend_from_slice(&line[1..]);
        }
        (w, h, pixels)
    }

    #[test]
    fn checksums_match_known_vectors() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // длинные данные: несколько пересчётов по модулю
        assert_eq!(adler32(&vec![0xFF; 100_000]), {
            let (mut a, mut b) = (1u64, 0u64);
            for _ in 0..100_000 {
                a = (a + 255) % 65_521;
                b = (b + a) % 65_521;
            }
            ((b << 16) | a) as u32
        });
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        for (input, want) in [("", ""), ("f", "Zg=="), ("fo", "Zm8="), ("foo", "Zm9v"), ("foob", "Zm9vYg=="), ("fooba", "Zm9vYmE="), ("foobar", "Zm9vYmFy")] {
            assert_eq!(base64_encode(input.as_bytes()), want);
        }
        assert_eq!(base64_encode(&[0xFB, 0xFF, 0xBF]), "+/+/");
    }

    #[test]
    fn png_roundtrip_small_and_icon_sized() {
        let px: Vec<u8> = vec![255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 40];
        let png = encode_png_rgba(2, 2, &px).unwrap();
        assert_eq!(decode(&png), (2, 2, px));
        let big: Vec<u8> = (0..32 * 32 * 4).map(|i| (i * 7 % 251) as u8).collect();
        let png = encode_png_rgba(32, 32, &big).unwrap();
        assert!(png.len() < 5_000, "иконка 32×32 — около 4 КБ, получилось {}", png.len());
        assert!(looks_like_png(&png));
        assert_eq!(decode(&png), (32, 32, big));
    }

    #[test]
    fn png_with_several_stored_blocks() {
        // 120×120×4 = 57 600 байт + фильтры < 65 535, поэтому берём 128×128 (65 536 + 128 > 65 535 → два блока)
        let big: Vec<u8> = (0..128 * 128 * 4).map(|i| (i % 253) as u8).collect();
        let png = encode_png_rgba(128, 128, &big).unwrap();
        assert!(png.len() > 65_535 && !looks_like_png(&png), "слишком большой для базы");
        assert_eq!(decode(&png), (128, 128, big));
    }

    #[test]
    fn png_rejects_bad_input() {
        assert!(encode_png_rgba(0, 4, &[]).is_none());
        assert!(encode_png_rgba(4, 0, &[]).is_none());
        assert!(encode_png_rgba(2, 2, &[0; 15]).is_none());
        assert!(encode_png_rgba(129, 1, &vec![0; 129 * 4]).is_none());
        assert!(!looks_like_png(b"not a png at all, just some bytes of text here!!"));
        assert!(!looks_like_png(&[]));
    }

    #[test]
    fn bgra_is_converted_and_uses_alpha_when_present() {
        // B,G,R,A -> R,G,B,A
        assert_eq!(bgra_to_rgba(&[1, 2, 3, 4, 10, 20, 30, 255], None), vec![3, 2, 1, 4, 30, 20, 10, 255]);
        // альфа есть → маска игнорируется
        assert_eq!(bgra_to_rgba(&[1, 2, 3, 9], Some(&[255, 255, 255, 0])), vec![3, 2, 1, 9]);
    }

    #[test]
    fn old_style_icons_use_the_mask() {
        let bgra = [1, 2, 3, 0, 4, 5, 6, 0]; // альфа везде 0
        let mask = [0, 0, 0, 0, 255, 255, 255, 0]; // 1-я точка непрозрачная, 2-я прозрачная
        assert_eq!(bgra_to_rgba(&bgra, Some(&mask)), vec![3, 2, 1, 255, 6, 5, 4, 0]);
        assert_eq!(bgra_to_rgba(&bgra, None), vec![3, 2, 1, 255, 6, 5, 4, 255], "без маски — непрозрачная");
        assert_eq!(bgra_to_rgba(&bgra, Some(&[0, 0, 0, 0])), vec![3, 2, 1, 255, 6, 5, 4, 255], "маска не той длины игнорируется");
    }
}
