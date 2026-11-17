use anyhow::Result;
use basinsync::{
    analogue::{self, Builder, Order},
    event::{self, FlowCommand, Marker},
    recovery::{Candidate, Coverage},
    timeline::Datashed,
};
use num_bigint::BigUint;
use std::collections::BTreeMap;

fn events() -> Result<Vec<Vec<u8>>> {
    let mut state = Datashed::default().into_stager();
    (0..4)
        .map(|n| {
            let blob = vec![n as u8; n * 100 + 1];
            Ok(state
                .prepare(vec![FlowCommand {
                    markers: BTreeMap::from([(
                        "f".into(),
                        Marker {
                            viewpoint: n.to_string(),
                            sha384: event::digest(&blob),
                            blob,
                            edges: Default::default(),
                            context: BTreeMap::new(),
                            verified: BTreeMap::new(),
                        },
                    )]),
                }])?
                .bytes()
                .to_vec())
        })
        .collect()
}

#[test]
fn bulk_orders_preserve_event_bytes_ranges_identity_and_physical_recovery() -> Result<()> {
    let events = events()?;
    let mut builder = Builder::new(1u8.into())?;
    for (n, bytes) in events.iter().enumerate() {
        builder.append(&(n + 1).into(), bytes)?;
    }
    let (order, bytes, range) = builder.finish()?;
    let index = analogue::decode(&bytes, &order)?;
    assert_eq!(index.lowest, 1u8.into());
    assert_eq!(order.highest, 5u8.into());
    assert_eq!(index.offsets.last(), Some(&range.start));
    assert_eq!(analogue::footer(&bytes)?.1, range);
    let mut actual = Datashed::default();
    let mut expected = Datashed::default();
    for (n, original) in events.iter().enumerate() {
        let m: BigUint = (n + 1).into();
        let selected = index.range(&m, &(&m + 1u8))?;
        assert_eq!(&bytes[selected.clone()], original);
        actual.apply_entry(m.clone(), &bytes[selected])?;
        expected.apply_entry(m, original)?;
    }
    assert_eq!(
        actual.database.unwrap().image,
        expected.database.unwrap().image
    );
    assert_eq!(
        index.range(&2u8.into(), &4u8.into())?,
        index.offsets[1]..index.offsets[3]
    );
    let mut bad = bytes.clone();
    bad[index.offsets[1] - 1] ^= 1;
    assert!(analogue::decode(&bad, &order).is_err());
    assert!(analogue::decode(&bytes[..range.start], &order).is_err());
    assert!(
        analogue::decode(
            &bytes,
            &Order {
                lowest: 2u8.into(),
                highest: 6u8.into()
            }
        )
        .is_err()
    );
    let mut bad = bytes.clone();
    bad[range.end - 1] ^= 1;
    assert!(analogue::decode(&bad, &order).is_err());
    let mut margins = bytes.clone();
    let inner = basinsync::envbuf::nested_message(&events[0])?;
    let margin = basinsync::envbuf::before(inner, inner.len())?;
    margins[8 + margin.content_start] = 7;
    analogue::verify_same_events(&margins, &bytes, &order)?;
    let marker = event::blob_ranges(&events[0])?.remove(0);
    margins[usize::try_from(marker.offset)?] ^= 1;
    assert!(analogue::verify_same_events(&margins, &bytes, &order).is_err());
    Ok(())
}

#[test]
fn lowest_extensions_are_canonical_at_short_and_widened_vector_lengths() -> Result<()> {
    let events = events()?;
    for lowest in [
        1u8.into(),
        255u16.into(),
        65535u32.into(),
        16777215u32.into(),
        BigUint::from(1u8) << 256usize,
        BigUint::from(1u8) << 2048usize,
    ] {
        for count in 1..=4 {
            let mut builder = Builder::new(lowest.clone())?;
            for (n, event) in events.iter().take(count).enumerate() {
                builder.append(&(&lowest + n), event)?;
            }
            let (order, bytes, range) = builder.finish()?;
            let index = analogue::decode(&bytes, &order)?;
            assert_eq!(index.lowest, lowest);
            assert_eq!(index.offsets.len(), count + 1);
            let frame = basinsync::envbuf::at(&bytes, range.start)?;
            let query = flexbuffers::Reader::get_root(frame.content)?
                .get_vector()?
                .index(3)?
                .get_map()?;
            assert_eq!(
                query.index("lowest")?.flexbuffer_type(),
                flexbuffers::FlexBufferType::VectorUInt
            );
            assert_ne!(
                query.index("offsets")?.flexbuffer_type(),
                flexbuffers::FlexBufferType::VectorUInt
            );
        }
    }
    Ok(())
}

#[test]
fn keys_seek_descending_bounds_and_coverage_detects_contained_intervals() -> Result<()> {
    let mut orders = vec![(1u8, 10u8), (2, 3), (2, 8), (1, 20)]
        .into_iter()
        .map(|(a, b)| Order {
            lowest: a.into(),
            highest: b.into(),
        })
        .collect::<Vec<_>>();
    orders.sort_by_key(|o| o.key(16..32).unwrap());
    assert_eq!(
        orders
            .iter()
            .map(|o| (o.lowest.to_string(), o.highest.to_string()))
            .collect::<Vec<_>>(),
        vec![
            ("2".into(), "8".into()),
            ("2".into(), "3".into()),
            ("1".into(), "20".into()),
            ("1".into(), "10".into())
        ]
    );
    let mut coverage = Coverage::default();
    for order in &orders {
        let key = order.key(16..32)?;
        assert_eq!(Order::from_key(&key)?, *order);
        assert_eq!(analogue::manifest_range(&key)?, 16..32);
        assert!(key.ends_with("/bytes=16-31"));
        coverage.observe(&Candidate {
            key,
            order: order.clone(),
            size: 100,
        })?;
    }
    assert_eq!(coverage.covered.len(), 3);
    assert!(orders[0].key(16..32)? > analogue::seek(&2u8.into()));
    assert!(
        orders
            .iter()
            .all(|o| o.key(16..32).unwrap() > analogue::seek(&3u8.into()))
    );
    for bad in [
        "analogue/0yfff/0yffe",
        "analogue/0yffe/0yffe",
        "analogue/0z001/0z002",
        "analogue/0yffe/0yffd/extra",
    ] {
        assert!(Order::from_key(bad).is_err());
    }
    for suffix in [
        "bytes=016-31",
        "bytes=16-32",
        "bytes=32-15",
        "bytes=-1-31",
        "bytes=0-18446744073709551615",
    ] {
        assert!(Order::from_key(&format!("analogue/0yffe/0yffd/{suffix}")).is_err());
    }
    assert!(Builder::new(0u8.into()).is_err());
    assert!(Builder::new(1u8.into())?.finish().is_err());
    let mut builder = Builder::new(1u8.into())?;
    assert!(builder.append(&2u8.into(), &events()?[0]).is_err());
    Ok(())
}
