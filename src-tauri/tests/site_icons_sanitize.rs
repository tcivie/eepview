// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Site icons, sanitizer side (R16 to R21 of `docs/wiki/site-icons.md`).
//!
//! The inputs are real PNG, ICO, GIF, JPEG, WebP and BMP bytes made with the `image` crate
//! (a dev-dependency), plus hand-made SVG, HTML, truncated, oversize and polyglot files.
//! The output is checked by reading the PNG header and chunks by hand, so the check does
//! not depend on the code under test.

use std::error::Error;
use std::io::Cursor;

use eepview_lib::icons::{
    DATA_URL_PREFIX, Format, LARGE_PX, MAX_SOURCE_PX, SMALL_PX, SanitizeError, data_url, file_stem,
    sanitize, sniff,
};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};

type Res<T> = Result<T, Box<dyn Error>>;

const MARKER: &[u8] = b"SECRET-MARKER-4711";
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// An image filled by `paint(x, y)`.
fn picture(w: u32, h: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
    RgbaImage::from_fn(w, h, |x, y| Rgba(paint(x, y)))
}

/// A noisy opaque image, so truncating its file always breaks it.
fn noisy(w: u32, h: u32) -> RgbaImage {
    picture(w, h, |x, y| {
        let v = (x.wrapping_mul(37) ^ y.wrapping_mul(91)) % 251;
        let b = u8::try_from(v).unwrap_or(0);
        [b, b.wrapping_mul(3), b.wrapping_add(90), 255]
    })
}

/// A solid image of one colour.
fn solid(w: u32, h: u32, colour: [u8; 4]) -> RgbaImage {
    picture(w, h, |_, _| colour)
}

/// The file bytes of `img` in `format` (JPEG gets no alpha channel).
fn encode(img: &RgbaImage, format: ImageFormat) -> Res<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgba8(img.clone())
            .to_rgb8()
            .write_to(&mut out, format)?;
    } else {
        img.write_to(&mut out, format)?;
    }
    Ok(out.into_inner())
}

/// Decodes a PNG made by the code under test.
fn decode(png: &[u8]) -> Res<RgbaImage> {
    Ok(image::load_from_memory_with_format(png, ImageFormat::Png)?.to_rgba8())
}

fn be32(bytes: &[u8], at: usize) -> Res<u32> {
    let raw: [u8; 4] = bytes.get(at..at + 4).ok_or("short file")?.try_into()?;
    Ok(u32::from_be_bytes(raw))
}

/// The chunk types of a PNG file, in order.
fn chunk_types(png: &[u8]) -> Res<Vec<String>> {
    let mut types = Vec::new();
    let mut at = 8;
    while at + 8 <= png.len() {
        let len = usize::try_from(be32(png, at)?)?;
        let kind = png.get(at + 4..at + 8).ok_or("short chunk")?;
        types.push(String::from_utf8_lossy(kind).into_owned());
        at += 12 + len;
    }
    Ok(types)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Checks the stated shape of one output file: a PNG, `px` x `px`, 8 bit RGBA.
fn assert_png_of(png: &[u8], px: u32) -> Res<()> {
    assert_eq!(png.get(..8), Some(&PNG_MAGIC[..]), "output is a PNG");
    assert_eq!(png.get(12..16), Some(&b"IHDR"[..]));
    assert_eq!(be32(png, 16)?, px, "width");
    assert_eq!(be32(png, 20)?, px, "height");
    assert_eq!(png.get(24), Some(&8), "8 bit");
    assert_eq!(png.get(25), Some(&6), "colour type 6 is RGBA");
    Ok(())
}

#[test]
fn r16_sniff_reads_the_magic_bytes_of_the_five_types() {
    // R16: PNG, ICO, GIF (both versions), JPEG, WebP.
    assert_eq!(
        sniff(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0]),
        Some(Format::Png)
    );
    assert_eq!(sniff(&[0x00, 0x00, 0x01, 0x00, 1, 0]), Some(Format::Ico));
    assert_eq!(sniff(b"GIF87a\x01\x00"), Some(Format::Gif));
    assert_eq!(sniff(b"GIF89a\x01\x00"), Some(Format::Gif));
    assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]), Some(Format::Jpeg));
    assert_eq!(sniff(b"RIFF\x24\x00\x00\x00WEBPVP8 "), Some(Format::WebP));
}

#[test]
fn r16_sniff_refuses_every_other_start() {
    // R16, R17: near misses and other types.
    let others: [&[u8]; 12] = [
        b"",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        b"<?xml version=\"1.0\"?><svg/>",
        b"<!DOCTYPE html><html></html>",
        b"BM\x36\x00\x00\x00\x00\x00",
        b"II*\x00\x08\x00\x00\x00",
        b"MM\x00*\x00\x00\x00\x08",
        &[0x00, 0x00, 0x02, 0x00, 1, 0],
        &[0xFF, 0xD8],
        b"RIFF\x24\x00\x00\x00WAVEfmt ",
        b"GIF90a\x01\x00",
        &[0x89, 0x50, 0x4E, 0x47],
    ];
    for bytes in others {
        assert_eq!(sniff(bytes), None, "{bytes:?}");
    }
}

#[test]
fn r16_sniff_names_the_type_of_real_files() -> Res<()> {
    // R16: files made by an encoder, not only the bare magic.
    let img = noisy(16, 16);
    assert_eq!(sniff(&encode(&img, ImageFormat::Png)?), Some(Format::Png));
    assert_eq!(sniff(&encode(&img, ImageFormat::Ico)?), Some(Format::Ico));
    assert_eq!(sniff(&encode(&img, ImageFormat::Gif)?), Some(Format::Gif));
    assert_eq!(sniff(&encode(&img, ImageFormat::Jpeg)?), Some(Format::Jpeg));
    assert_eq!(sniff(&encode(&img, ImageFormat::WebP)?), Some(Format::WebP));
    assert_eq!(sniff(&encode(&img, ImageFormat::Bmp)?), None);
    Ok(())
}

#[test]
fn r20_every_accepted_type_becomes_a_32_and_a_64_pixel_rgba_png() -> Res<()> {
    // R20: PNG, ICO, GIF, JPEG and WebP in, two new PNG files out.
    let img = noisy(48, 48);
    for format in [
        ImageFormat::Png,
        ImageFormat::Ico,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::WebP,
    ] {
        let icon = sanitize(&encode(&img, format)?).map_err(|e| format!("{format:?}: {e:?}"))?;
        assert_png_of(&icon.small, 32)?;
        assert_png_of(&icon.large, 64)?;
    }
    Ok(())
}

#[test]
fn r20_the_stated_sizes_are_32_and_64() {
    // R20: from the requirement text.
    assert_eq!(SMALL_PX, 32);
    assert_eq!(LARGE_PX, 64);
}

#[test]
fn r20_a_small_square_image_is_scaled_to_fill_the_square() -> Res<()> {
    // R20: a 16 x 16 blue image fills the 64 x 64 and the 32 x 32 output.
    let icon = sanitize(&encode(&solid(16, 16, [0, 0, 255, 255]), ImageFormat::Png)?)
        .map_err(|e| format!("{e:?}"))?;
    let large = decode(&icon.large)?;
    for (x, y) in [(2, 2), (32, 32), (61, 61), (2, 61)] {
        let Rgba([r, _, b, a]) = *large.get_pixel(x, y);
        assert!(a == 255 && b > 200 && r < 50, "({x},{y}) is not blue");
    }
    Ok(())
}

#[test]
fn r20_a_wide_image_is_centred_with_transparent_bars() -> Res<()> {
    // R20: 64 x 32 red keeps its shape: opaque in the middle rows, transparent above and below.
    let icon = sanitize(&encode(&solid(64, 32, [255, 0, 0, 255]), ImageFormat::Png)?)
        .map_err(|e| format!("{e:?}"))?;
    let large = decode(&icon.large)?;
    assert_eq!(large.get_pixel(32, 32).0[3], 255, "centre is opaque");
    assert!(large.get_pixel(32, 32).0[0] > 200, "centre is red");
    for y in [0, 5, 10, 53, 58, 63] {
        assert_eq!(large.get_pixel(32, y).0[3], 0, "row {y} is transparent");
    }
    let small = decode(&icon.small)?;
    assert_eq!(small.get_pixel(16, 16).0[3], 255);
    assert_eq!(small.get_pixel(16, 1).0[3], 0);
    assert_eq!(small.get_pixel(16, 30).0[3], 0);
    Ok(())
}

#[test]
fn r20_a_tall_image_is_centred_with_transparent_bars() -> Res<()> {
    // R20: 32 x 64 green keeps its shape: transparent left and right.
    let icon = sanitize(&encode(&solid(32, 64, [0, 255, 0, 255]), ImageFormat::Png)?)
        .map_err(|e| format!("{e:?}"))?;
    let large = decode(&icon.large)?;
    assert_eq!(large.get_pixel(32, 32).0[3], 255);
    for x in [0, 5, 10, 53, 58, 63] {
        assert_eq!(large.get_pixel(x, 32).0[3], 0, "column {x} is transparent");
    }
    Ok(())
}

#[test]
fn r20_transparency_of_the_source_is_kept() -> Res<()> {
    // R20: RGBA output. A fully transparent pixel stays transparent.
    let img = picture(32, 32, |x, _| {
        if x < 16 {
            [255, 0, 0, 0]
        } else {
            [255, 0, 0, 255]
        }
    });
    let icon = sanitize(&encode(&img, ImageFormat::Png)?).map_err(|e| format!("{e:?}"))?;
    let large = decode(&icon.large)?;
    assert_eq!(large.get_pixel(4, 32).0[3], 0);
    assert_eq!(large.get_pixel(60, 32).0[3], 255);
    Ok(())
}

#[test]
fn r20_an_animated_gif_gives_its_first_frame() -> Res<()> {
    // R20: frame 1 red, frame 2 blue. The icon is red.
    let mut out = Cursor::new(Vec::new());
    {
        let mut encoder = GifEncoder::new(&mut out);
        encoder.set_repeat(Repeat::Infinite)?;
        let delay = Delay::from_numer_denom_ms(100, 1);
        for colour in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            encoder.encode_frame(Frame::from_parts(solid(32, 32, colour), 0, 0, delay))?;
        }
    }
    let icon = sanitize(&out.into_inner()).map_err(|e| format!("{e:?}"))?;
    let Rgba([r, _, b, a]) = *decode(&icon.large)?.get_pixel(32, 32);
    assert!(a == 255 && r > 200 && b < 50, "first frame only");
    Ok(())
}

#[test]
fn r19_the_limit_is_1024_pixels() {
    // R19: from the requirement text.
    assert_eq!(MAX_SOURCE_PX, 1024);
}

#[test]
fn r19_an_image_of_1024_pixels_is_accepted() -> Res<()> {
    // R19: 1024 wide and 1024 tall are inside the limit.
    for (w, h) in [(1024, 1), (1, 1024), (1024, 1024)] {
        let bytes = encode(&solid(w, h, [1, 2, 3, 255]), ImageFormat::Png)?;
        assert!(sanitize(&bytes).is_ok(), "{w} x {h} is inside the limit");
    }
    Ok(())
}

#[test]
fn r19_an_image_wider_or_taller_than_1024_is_refused() -> Res<()> {
    // R19: 1025 in either direction, in the formats that can be that large.
    for (w, h) in [(1025, 1), (1, 1025), (1025, 1025), (2000, 10)] {
        for format in [ImageFormat::Png, ImageFormat::Gif, ImageFormat::Jpeg] {
            let bytes = encode(&solid(w, h, [1, 2, 3, 255]), format)?;
            assert_eq!(
                sanitize(&bytes),
                Err(SanitizeError::TooLarge),
                "{w} x {h} {format:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn r19_a_header_that_claims_a_huge_size_is_refused() -> Res<()> {
    // R19: a PNG whose header claims 60 000 x 60 000 pixels. It must be refused, not decoded.
    let mut bytes = encode(&noisy(8, 8), ImageFormat::Png)?;
    bytes[16..20].copy_from_slice(&60_000_u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&60_000_u32.to_be_bytes());
    assert!(sanitize(&bytes).is_err());
    Ok(())
}

#[test]
fn r17_svg_is_always_refused() {
    // R17: every way to start an SVG, also when the sender calls it a PNG.
    let svgs: [&[u8]; 6] = [
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"32\" height=\"32\"><rect/></svg>",
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        b"\n  <svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
        b"\xEF\xBB\xBF<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        b"<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"\"><svg/>",
        b"<svg onload=\"fetch('http://evil.example/')\"/>",
    ];
    for svg in svgs {
        assert_eq!(sanitize(svg), Err(SanitizeError::Format), "{svg:?}");
    }
}

#[test]
fn r17_html_bmp_tiff_text_and_empty_bodies_are_refused() -> Res<()> {
    // R17: other types and the empty body.
    let bmp = encode(&noisy(8, 8), ImageFormat::Bmp)?;
    let others: [&[u8]; 6] = [
        b"<!DOCTYPE html><html><body>not found</body></html>",
        b"<html><link rel=\"icon\" href=\"/x.png\"></html>",
        b"II*\x00\x08\x00\x00\x00\x00\x00",
        b"just some text",
        b"",
        &bmp,
    ];
    for bytes in others {
        assert_eq!(sanitize(bytes), Err(SanitizeError::Format), "{bytes:?}");
    }
    Ok(())
}

#[test]
fn r18_accepted_magic_with_svg_or_html_after_it_does_not_decode() {
    // R18: the start looks right, the rest is not an image.
    let mut png_svg = PNG_MAGIC.to_vec();
    png_svg.extend_from_slice(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    let cases: [Vec<u8>; 5] = [
        png_svg,
        b"GIF89a<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        b"GIF87a<html></html>".to_vec(),
        [&[0xFF, 0xD8, 0xFF][..], b"<html></html>"].concat(),
        [&[0x00, 0x00, 0x01, 0x00][..], b"<svg/>"].concat(),
    ];
    for bytes in cases {
        // Junk after a GIF header can read as a huge size, so TooLarge is a refusal too.
        let got = sanitize(&bytes);
        let refused = matches!(got, Err(SanitizeError::Decode | SanitizeError::TooLarge));
        assert!(refused, "{bytes:?}: {got:?}");
    }
}

#[test]
fn r18_a_riff_file_that_is_not_a_webp_image_does_not_decode() {
    // R18: RIFF, size, WEBP, then junk.
    let bytes = [
        &b"RIFF\x20\x00\x00\x00WEBP"[..],
        b"this is not a bitstream!",
    ]
    .concat();
    assert!(matches!(
        sanitize(&bytes),
        Err(SanitizeError::Decode | SanitizeError::Format)
    ));
    assert_eq!(sniff(&bytes), Some(Format::WebP));
}

#[test]
fn r18_a_truncated_file_of_each_type_is_refused() -> Res<()> {
    // R18: the first 40 bytes of a real file.
    let img = noisy(64, 64);
    for format in [
        ImageFormat::Png,
        ImageFormat::Ico,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::WebP,
    ] {
        let bytes = encode(&img, format)?;
        let cut = bytes.get(..40).ok_or("file shorter than 40 bytes")?;
        assert_eq!(sanitize(cut), Err(SanitizeError::Decode), "{format:?}");
    }
    Ok(())
}

#[test]
fn r18_a_png_cut_in_the_middle_of_its_pixels_is_refused() -> Res<()> {
    // R18: half of a real PNG.
    let bytes = encode(&noisy(64, 64), ImageFormat::Png)?;
    let half = bytes.get(..bytes.len() / 2).ok_or("short")?;
    assert_eq!(sanitize(half), Err(SanitizeError::Decode));
    Ok(())
}

/// The CRC-32 of a PNG chunk (type and data).
fn crc32(parts: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in parts.iter().flat_map(|p| p.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// A PNG chunk with its CRC.
fn chunk(kind: [u8; 4], data: &[u8]) -> Res<Vec<u8>> {
    let mut out = u32::try_from(data.len())?.to_be_bytes().to_vec();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[&kind, data]).to_be_bytes());
    Ok(out)
}

/// `png` with an extra chunk right after IHDR.
fn with_chunk(png: &[u8], extra: &[u8]) -> Res<Vec<u8>> {
    let split = 8 + 12 + 13;
    let mut out = png.get(..split).ok_or("short")?.to_vec();
    out.extend_from_slice(extra);
    out.extend_from_slice(png.get(split..).ok_or("short")?);
    Ok(out)
}

#[test]
fn r21_the_output_has_only_pixel_chunks_and_none_of_the_input_metadata() -> Res<()> {
    // R21: a PNG with a text chunk, a physical size chunk and trailing bytes.
    let plain = encode(&noisy(40, 40), ImageFormat::Png)?;
    let mut text = b"Comment\0".to_vec();
    text.extend_from_slice(MARKER);
    let mut bytes = with_chunk(&plain, &chunk(*b"tEXt", &text)?)?;
    bytes = with_chunk(&bytes, &chunk(*b"pHYs", &[0, 0, 11, 19, 0, 0, 11, 19, 1])?)?;
    bytes.extend_from_slice(MARKER);
    let icon = sanitize(&bytes).map_err(|e| format!("{e:?}"))?;
    for png in [&icon.small, &icon.large] {
        assert!(!contains(png, MARKER), "R21: metadata reached the output");
        for kind in chunk_types(png)? {
            assert!(
                ["IHDR", "IDAT", "IEND"].contains(&kind.as_str()),
                "R21: chunk {kind}"
            );
        }
    }
    Ok(())
}

/// Bytes of a GIF before its first block: header, screen descriptor, global colour table.
fn gif_header_len(gif: &[u8]) -> Res<usize> {
    let packed = *gif.get(10).ok_or("short gif")?;
    let table = if packed & 0x80 == 0 {
        0
    } else {
        3 * (1_usize << ((packed & 7) + 1))
    };
    Ok(13 + table)
}

#[test]
fn r21_a_jpeg_comment_and_a_gif_comment_do_not_reach_the_output() -> Res<()> {
    // R21: a JPEG COM segment and a GIF comment extension.
    let jpeg = encode(&noisy(32, 32), ImageFormat::Jpeg)?;
    let mut com = vec![0xFF, 0xFE];
    com.extend_from_slice(&u16::try_from(MARKER.len() + 2)?.to_be_bytes());
    com.extend_from_slice(MARKER);
    let with_com = [&jpeg[..2], &com, &jpeg[2..]].concat();
    let gif = encode(&noisy(32, 32), ImageFormat::Gif)?;
    let mut comment = vec![0x21, 0xFE, u8::try_from(MARKER.len())?];
    comment.extend_from_slice(MARKER);
    comment.push(0);
    let at = gif_header_len(&gif)?;
    let with_comment = [&gif[..at], &comment, &gif[at..]].concat();
    for bytes in [with_com, with_comment] {
        let icon = sanitize(&bytes).map_err(|e| format!("{e:?}"))?;
        assert!(!contains(&icon.small, MARKER) && !contains(&icon.large, MARKER));
    }
    Ok(())
}

#[test]
fn r21_the_output_never_contains_the_input_bytes() -> Res<()> {
    // R21: new bytes, for every accepted type. 50 px, so the input is no 64 px PNG.
    let img = noisy(50, 50);
    for format in [
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::WebP,
    ] {
        let input = encode(&img, format)?;
        let icon = sanitize(&input).map_err(|e| format!("{e:?}"))?;
        for out in [&icon.small, &icon.large] {
            assert_ne!(out, &input);
            assert!(
                !contains(out, &input),
                "{format:?}: input inside the output"
            );
        }
    }
    Ok(())
}

#[test]
fn r21_a_polyglot_with_html_after_the_png_gives_a_clean_png() -> Res<()> {
    // R21: a valid PNG followed by script text. Only pixels come out.
    let mut bytes = encode(&noisy(32, 32), ImageFormat::Png)?;
    bytes.extend_from_slice(b"<script>alert(document.cookie)</script><svg onload=alert(1)>");
    let icon = sanitize(&bytes).map_err(|e| format!("{e:?}"))?;
    for png in [&icon.small, &icon.large] {
        assert!(!contains(png, b"<script") && !contains(png, b"<svg"));
        assert_eq!(chunk_types(png)?.last().map(String::as_str), Some("IEND"));
    }
    assert_png_of(&icon.large, 64)
}

#[test]
fn r21_a_polyglot_with_a_zip_tail_gives_a_clean_png() -> Res<()> {
    // R21: a GIF followed by a ZIP local header.
    let mut bytes = encode(&noisy(32, 32), ImageFormat::Gif)?;
    bytes.extend_from_slice(b"PK\x03\x04hidden.txt");
    let icon = sanitize(&bytes).map_err(|e| format!("{e:?}"))?;
    assert!(!contains(&icon.large, b"PK\x03\x04") && !contains(&icon.large, b"hidden.txt"));
    Ok(())
}

#[test]
fn r20_data_url_is_the_png_prefix_and_standard_base64_with_padding() {
    // R31: `data:image/png;base64,` plus standard base64 with padding.
    assert_eq!(DATA_URL_PREFIX, "data:image/png;base64,");
    assert_eq!(data_url(&[1, 2, 3]), "data:image/png;base64,AQID");
    assert_eq!(data_url(&[1]), "data:image/png;base64,AQ==");
    assert_eq!(data_url(&[1, 2]), "data:image/png;base64,AQI=");
    assert_eq!(data_url(&[]), "data:image/png;base64,");
    assert_eq!(data_url(&[0xFB, 0xFF, 0xFE]), "data:image/png;base64,+//+");
}

#[test]
fn r22_file_stem_is_the_sha256_of_the_host_in_lower_case_hex() {
    // R22: known SHA-256 values of the host text.
    assert_eq!(
        file_stem("site.i2p"),
        "38b57446e557683adfc2d44100e81fc46bd5a5a5582e2659146d5a3f3e97c1f1"
    );
    assert_eq!(
        file_stem("example.i2p"),
        "d07827620dbf5948d723d83fb93c28495b2a363e2d79720932bc4f94e9d78660"
    );
    assert_eq!(
        file_stem("alpha.i2p"),
        "3cb7d940c289d5f66ec11ccd02197102e5d1ecf80b0ff3bfb8a491910238ba26"
    );
}
