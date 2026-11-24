//! Interoperability with the reference Rust `flexbuffers` crate: its reader
//! accepts our encoding, and our decoder accepts its builder's output.

use basinsync_codec::flexbuffers::{decode, encode, Value};
use flexbuffers::{Builder, FlexBufferType, Reader};

#[test]
fn reference_reader_reads_ours() {
    let v = Value::Vector(vec![
        Value::species(),
        Value::String("sync.memoremix.com".into()),
        Value::Map(vec![
            ("blob".into(), Value::Blob(vec![1, 2, 3])),
            ("n".into(), Value::Int(-300)),
            ("ends".into(), Value::IntVector(vec![488, 1360, 1976])),
            ("f".into(), Value::Float(0.1)),
            ("b".into(), Value::Bool(true)),
            ("u".into(), Value::UInt(1 << 40)),
        ]),
    ]);
    let bytes = encode(&v).unwrap();
    let r = Reader::get_root(bytes.as_slice()).unwrap();
    let vec = r.as_vector();
    assert_eq!(vec.len(), 3);
    let sym = vec.idx(0);
    assert_eq!(sym.flexbuffer_type(), FlexBufferType::VectorUInt);
    let tv = sym.as_vector();
    let s: Vec<u8> = (0..tv.len()).map(|i| tv.idx(i).as_u8()).collect();
    assert_eq!(s, b"#species");
    assert_eq!(vec.idx(1).as_str(), "sync.memoremix.com");
    let m = vec.idx(2).as_map();
    assert_eq!(m.idx("blob").as_blob().0, &[1, 2, 3]);
    assert_eq!(m.idx("n").as_i64(), -300);
    assert_eq!(m.idx("ends").flexbuffer_type(), FlexBufferType::VectorInt);
    assert_eq!(m.idx("ends").as_vector().idx(2).as_i64(), 1976);
    assert_eq!(m.idx("f").as_f64(), 0.1);
    assert!(m.idx("b").as_bool());
    assert_eq!(m.idx("u").as_u64(), 1 << 40);
}

#[test]
fn we_read_reference_builder_output() {
    let mut b = Builder::default();
    {
        let mut v = b.start_vector();
        v.push("hello");
        v.push(-5i8);
        v.push(70000u32);
        v.push(2.5f64);
        v.push(flexbuffers::Blob(&[9u8, 8, 7][..]));
        {
            let mut m = v.start_map();
            m.push("zeta", 1i64);
            m.push("alpha", "a");
            m.end_map();
        }
        v.push(&[1i16, 2, 3][..]);
        v.end_vector();
    }
    let got = decode(b.view()).unwrap();
    assert_eq!(
        got,
        Value::Vector(vec![
            Value::String("hello".into()),
            Value::Int(-5),
            Value::UInt(70000),
            Value::Float(2.5),
            Value::Blob(vec![9, 8, 7]),
            Value::Map(vec![("alpha".into(), Value::String("a".into())), ("zeta".into(), Value::Int(1))]),
            Value::IntVector(vec![1, 2, 3]),
        ])
    );
}

#[test]
fn reference_typed_uint_vector_is_an_extension_slot() {
    // An ordinary unsigned array written as VT_VECTOR_UINT by another writer is
    // not a valid extension here: '\x01' is not a known prefix.
    let mut b = Builder::default();
    b.build_singleton(&[1u16, 2, 3, 4, 5][..]);
    assert_eq!(Reader::get_root(b.view()).unwrap().flexbuffer_type(), FlexBufferType::VectorUInt);
    assert!(decode(b.view()).is_err());
    // A valid extension written by the reference builder decodes.
    let mut b = Builder::default();
    b.build_singleton(&[b'x' as u16, 1, 0, 0, 0][..]);
    assert_eq!(decode(b.view()).unwrap(), Value::BigInt(16777216.into()));
}

/// Docs round 5: a value must end at or before the slot that refers to it.
#[test]
fn overlapping_layouts_are_rejected() {
    use basinsync_codec::flexbuffers::{decode, decode_extension};
    for wire in [&[106u8, 2, 120, 1, 48, 1][..], &[0, 2, 120, 1, 48, 1], &[3, 120, 1, 48, 1]] {
        assert!(decode_extension(wire).is_err(), "{wire:?}");
        assert!(decode(wire).is_err(), "{wire:?}");
    }
    assert!(decode_extension(&[2, 120, 1, 2, 48, 1]).is_ok());
}
