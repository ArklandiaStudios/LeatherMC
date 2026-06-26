//! Minimal NBT support — enough to write the "network NBT" the modern protocol
//! uses (since 1.20.2 the root tag is **nameless**).
//!
//! We only need *writing* for now (sending registry data and text components).
//! Values are represented by the [`Nbt`] tree and serialized with
//! [`write_network_nbt`].

/// An NBT value.
///
/// `Compound` keeps insertion order (a `Vec` of pairs) so output is
/// deterministic — handy for tests and reproducible registry blobs.
#[derive(Debug, Clone, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Nbt {
    /// The NBT tag id used in the wire format.
    pub fn tag_id(&self) -> u8 {
        match self {
            Nbt::Byte(_) => 1,
            Nbt::Short(_) => 2,
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::Float(_) => 5,
            Nbt::Double(_) => 6,
            Nbt::ByteArray(_) => 7,
            Nbt::String(_) => 8,
            Nbt::List(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::IntArray(_) => 11,
            Nbt::LongArray(_) => 12,
        }
    }
}

/// Tag id for an empty list's element type (TAG_End).
const TAG_END: u8 = 0;

/// Writes a value in **network NBT** form: the root's tag id, then its payload,
/// with no root name (the modern, nameless-root format).
pub fn write_network_nbt(buf: &mut Vec<u8>, root: &Nbt) {
    buf.push(root.tag_id());
    write_payload(buf, root);
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
}

fn write_payload(buf: &mut Vec<u8>, value: &Nbt) {
    match value {
        Nbt::Byte(v) => buf.push(*v as u8),
        Nbt::Short(v) => buf.extend_from_slice(&v.to_be_bytes()),
        Nbt::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
        Nbt::Long(v) => buf.extend_from_slice(&v.to_be_bytes()),
        Nbt::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
        Nbt::Double(v) => buf.extend_from_slice(&v.to_be_bytes()),
        Nbt::ByteArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for b in v {
                buf.push(*b as u8);
            }
        }
        Nbt::String(v) => write_string(buf, v),
        Nbt::List(items) => {
            // Element type is taken from the first item; an empty list uses
            // TAG_End, matching vanilla.
            let elem_id = items.first().map_or(TAG_END, Nbt::tag_id);
            buf.push(elem_id);
            buf.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for item in items {
                write_payload(buf, item);
            }
        }
        Nbt::Compound(entries) => {
            for (name, val) in entries {
                buf.push(val.tag_id());
                write_string(buf, name);
                write_payload(buf, val);
            }
            buf.push(TAG_END);
        }
        Nbt::IntArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for n in v {
                buf.extend_from_slice(&n.to_be_bytes());
            }
        }
        Nbt::LongArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for n in v {
                buf.extend_from_slice(&n.to_be_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nameless_root_string() {
        // Equivalent to the disconnect text component we send elsewhere.
        let mut buf = Vec::new();
        write_network_nbt(&mut buf, &Nbt::String("hi".to_string()));
        assert_eq!(buf, vec![0x08, 0x00, 0x02, b'h', b'i']);
    }

    #[test]
    fn compound_with_mixed_types() {
        let mut buf = Vec::new();
        let nbt = Nbt::Compound(vec![
            ("a".to_string(), Nbt::Int(1)),
            ("b".to_string(), Nbt::Byte(1)),
        ]);
        write_network_nbt(&mut buf, &nbt);
        assert_eq!(
            buf,
            vec![
                0x0A, // root: compound
                0x03, 0x00, 0x01, b'a', 0x00, 0x00, 0x00, 0x01, // int a = 1
                0x01, 0x00, 0x01, b'b', 0x01, // byte b = 1
                0x00, // end
            ]
        );
    }

    #[test]
    fn empty_list_uses_tag_end() {
        let mut buf = Vec::new();
        write_network_nbt(&mut buf, &Nbt::List(vec![]));
        assert_eq!(buf, vec![0x09, TAG_END, 0x00, 0x00, 0x00, 0x00]);
    }
}
