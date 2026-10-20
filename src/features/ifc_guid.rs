//! IFC GlobalId generation shared by scaffold and interactive completions.

use uuid::Uuid;

pub(crate) const IFC_GUID_ALPHABET: &[u8; 64] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// Returns a fresh UUID v4 compressed into the 22-character IFC GlobalId representation.
pub(crate) fn new_guid() -> String {
    compress_uuid(Uuid::new_v4().as_u128())
}

fn compress_uuid(mut value: u128) -> String {
    let mut output = [b'0'; 22];
    for character in output.iter_mut().rev() {
        let index = (value & 0b11_1111) as usize;
        *character = IFC_GUID_ALPHABET[index];
        value >>= 6;
    }

    String::from_utf8(output.to_vec()).expect("IFC GUID alphabet should be ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_guids_have_ifc_shape() {
        let guid = compress_uuid(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);

        assert_eq!(guid.len(), 22);
        assert!(guid.bytes().all(|byte| IFC_GUID_ALPHABET.contains(&byte)));
    }

    #[test]
    fn fresh_guids_are_distinct() {
        assert_ne!(new_guid(), new_guid());
    }
}
