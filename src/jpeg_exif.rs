//! Copy JPEG APP1 EXIF (and other `APPn`) segments into re-encoded JPEGs.

const SOI: [u8; 2] = [0xFF, 0xD8];

/// Copies `APPn` metadata from `source_jpeg` into `encoded_jpeg` after SOI / JFIF.
///
/// Encoder output typically has a JFIF APP0 and no EXIF. This inserts source APP
/// segments (except a duplicate JFIF APP0) so archival cameras keep EXIF.
///
/// # Arguments
///
/// * `source_jpeg` - Original JPEG bytes
/// * `encoded_jpeg` - Freshly encoded JPEG bytes
///
/// # Examples
///
/// ```
/// use pictures4096::jpeg_exif::merge_jpeg_metadata;
///
/// let tiny = &[0xFF, 0xD8, 0xFF, 0xD9];
/// assert_eq!(merge_jpeg_metadata(tiny, tiny), tiny);
/// ```
#[must_use]
pub fn merge_jpeg_metadata(source_jpeg: &[u8], encoded_jpeg: &[u8]) -> Vec<u8> {
    if !is_jpeg(source_jpeg) || !is_jpeg(encoded_jpeg) {
        return encoded_jpeg.to_vec();
    }

    let source_apps = collect_app_segments(source_jpeg);
    if source_apps.is_empty() {
        return encoded_jpeg.to_vec();
    }

    let (dest_prefix_end, dest_has_jfif) = dest_insert_index(encoded_jpeg);
    let filtered: Vec<&[u8]> = source_apps
        .into_iter()
        .filter(|segment| !(dest_has_jfif && is_jfif(segment)))
        .collect();
    if filtered.is_empty() {
        return encoded_jpeg.to_vec();
    }

    let extra: usize = filtered.iter().map(|segment| segment.len()).sum();
    let mut out = Vec::with_capacity(encoded_jpeg.len() + extra);
    out.extend_from_slice(&encoded_jpeg[..dest_prefix_end]);
    for segment in filtered {
        out.extend_from_slice(segment);
    }
    out.extend_from_slice(&encoded_jpeg[dest_prefix_end..]);
    out
}

/// Returns true when `data` starts with a JPEG SOI marker.
///
/// # Examples
///
/// ```
/// use pictures4096::jpeg_exif::is_jpeg;
///
/// assert!(is_jpeg(&[0xFF, 0xD8, 0xFF, 0xD9]));
/// assert!(!is_jpeg(&[0x89, 0x50]));
/// ```
#[must_use]
pub fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == SOI[0] && data[1] == SOI[1]
}

fn collect_app_segments(data: &[u8]) -> Vec<&[u8]> {
    let mut apps = Vec::new();
    let mut index = 2;
    while index + 3 < data.len() {
        if data[index] != 0xFF {
            break;
        }
        let marker = data[index + 1];
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if marker == 0x00 || marker == 0xFF {
            index += 1;
            continue;
        }
        if (0xD0..=0xD7).contains(&marker) {
            index += 2;
            continue;
        }
        if marker == 0x01 {
            index += 2;
            continue;
        }
        if index + 4 > data.len() {
            break;
        }
        let length = u16::from_be_bytes([data[index + 2], data[index + 3]]) as usize;
        let end = index.saturating_add(2).saturating_add(length);
        if length < 2 || end > data.len() {
            break;
        }
        if (0xE0..=0xEF).contains(&marker) {
            apps.push(&data[index..end]);
        }
        index = end;
    }
    apps
}

fn dest_insert_index(data: &[u8]) -> (usize, bool) {
    let mut index = 2;
    let mut has_jfif = false;
    if data.len() >= 6 && data[2] == 0xFF && data[3] == 0xE0 {
        let length = u16::from_be_bytes([data[4], data[5]]) as usize;
        let end = 4usize.saturating_add(length);
        if length >= 2 && end <= data.len() {
            has_jfif = is_jfif(&data[2..end]);
            if has_jfif {
                return (end, true);
            }
        }
    }
    while index + 1 < data.len() && data[index] == 0xFF && data[index + 1] == 0xFF {
        index += 1;
    }
    (index, has_jfif)
}

fn is_jfif(segment: &[u8]) -> bool {
    segment.len() >= 9 && segment[1] == 0xE0 && &segment[4..8] == b"JFIF"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app1_exif() -> Vec<u8> {
        let mut segment = vec![0xFF, 0xE1, 0x00, 0x08];
        segment.extend_from_slice(b"Exif");
        segment.extend_from_slice(&[0x00, 0x00]);
        let len = u16::try_from(segment.len() - 2).expect("test APP1 segment fits in u16");
        segment[2..4].copy_from_slice(&len.to_be_bytes());
        segment
    }

    #[test]
    fn inserts_exif_after_soi() {
        let mut source = Vec::from(SOI);
        source.extend_from_slice(&app1_exif());
        source.extend_from_slice(&[0xFF, 0xD9]);

        let dest = vec![0xFF, 0xD8, 0xFF, 0xD9];
        let merged = merge_jpeg_metadata(&source, &dest);
        assert!(merged.windows(4).any(|window| window == b"Exif"));
        assert_eq!(&merged[..2], &SOI);
        assert_eq!(&merged[merged.len() - 2..], &[0xFF, 0xD9]);
    }
}
