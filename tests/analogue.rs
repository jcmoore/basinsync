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
fn bulk_orders_trim_margins_and_preserve_messages_digests_ranges_and_recovery() -> Result<()> {
    let events = events()?;
    let mut builder = Builder::new(1u8.into())?;
    for (n, bytes) in events.iter().enumerate() {
        builder.append(&(n + 1).into(), bytes)?;
    }
    let (order, bytes, range) = builder.finish()?;
    let index = analogue::decode(&bytes, &order)?;
    assert_eq!(index.lowest, 1u8.into());
    assert_eq!(index.coordinate, 0u8.into());
    assert_eq!(order.highest, 5u8.into());
    assert_eq!(index.ends.last(), Some(&range.start));
    assert_eq!(analogue::footer(&bytes)?.1, range);
    let mut actual = Datashed::default();
    let mut expected = Datashed::default();
    for (n, original) in events.iter().enumerate() {
        let m: BigUint = (n + 1).into();
        let selected = index.range(&m, &(&m + 1u8))?;
        assert_eq!(
            &bytes[selected.clone()],
            event::trim_retry_margins(original)?.as_ref()
        );
        assert_eq!(
            event::entry_digest(&bytes[selected.clone()])?,
            event::entry_digest(original)?
        );
        actual.apply_entry(m.clone(), &bytes[selected])?;
        expected.apply_entry(m, original)?;
    }
    assert_eq!(
        actual.database.unwrap().image,
        expected.database.unwrap().image
    );
    assert_eq!(
        index.range(&2u8.into(), &4u8.into())?,
        index.ends[0]..index.ends[2]
    );
    let mut bad = bytes.clone();
    bad[index.ends[0] - 1] ^= 1;
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
    let margin = basinsync::envbuf::before(&margins, margins.len())?;
    let start = margin.content_start;
    margins[start] = 7;
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
            assert_eq!(index.ends.len(), count);
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
                query.index("ends")?.flexbuffer_type(),
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
    orders.sort_by_key(|o| o.key(16, &0u8.into()).unwrap());
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
        let key = order.key(16, &0u8.into())?;
        assert_eq!(Order::from_key(&key)?, *order);
        assert_eq!(analogue::manifest_start(&key)?, 16);
        assert!(key.ends_with("/v0/16/0"));
        coverage.observe(&Candidate {
            key,
            order: order.clone(),
            size: 100,
        })?;
    }
    assert_eq!(coverage.covered.len(), 3);
    assert!(orders[0].key(16, &0u8.into())? > analogue::seek(&2u8.into()));
    assert!(
        orders
            .iter()
            .all(|o| o.key(16, &0u8.into()).unwrap() > analogue::seek(&3u8.into()))
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
        "v0/016",
        "v0/17",
        "v0/0",
        "v0/-16",
        "v1/16",
        "v0/18446744073709551616",
    ] {
        assert!(Order::from_key(&format!("analogue/0yffe/0yffd/{suffix}/0")).is_err());
    }
    assert!(Builder::new(0u8.into()).is_err());
    assert!(Builder::new(1u8.into())?.finish().is_err());
    let mut builder = Builder::new(1u8.into())?;
    assert!(builder.append(&2u8.into(), &events()?[0]).is_err());
    Ok(())
}

#[test]
fn retry_margins_are_trimmed_and_ends_share_logical_and_physical_spans() -> Result<()> {
    let originals = events()?;
    let mut variants = vec![];
    let mut builder = Builder::new(1u8.into())?;
    let mut logical = 0;
    let mut physical = 0;
    let mut expected_ends = vec![];
    let mut expected = Datashed::default();
    for (i, original) in originals.iter().enumerate() {
        let mut changed = event::coordinate_bytes(original)?.to_vec();
        for n in 0..i {
            basinsync::envbuf::append(
                &mut changed,
                &vec![n as u8; n * 9],
                basinsync::envbuf::MARGIN,
            )?;
        }
        assert_eq!(
            event::entry_digest(&changed)?,
            event::entry_digest(original)?
        );
        assert_eq!(
            event::coordinate_length(&changed)?,
            event::coordinate_length(original)?
        );
        if i == 0 {
            assert_eq!(event::coordinate_length(&changed)?, changed.len());
        }
        builder.append(&(i + 1).into(), &changed)?;
        logical += event::coordinate_length(original)?;
        physical += changed.len();
        expected_ends.push(logical);
        expected.apply_entry((i + 1).into(), original)?;
        variants.push(changed);
        // Format/alignment affects the logical extent and must therefore be authenticated.
        let mut aligned = vec![];
        basinsync::envbuf::append_with_alignment(
            &mut aligned,
            event::envelope(original)?.content,
            basinsync::envbuf::NESTED,
            4,
        )?;
        assert_ne!(
            event::entry_digest(&aligned)?,
            event::entry_digest(original)?
        );
        let trimmed = event::trim_retry_margins(&aligned)?;
        assert_eq!(trimmed.len(), event::coordinate_length(&aligned)?);
        assert_eq!(&trimmed[..4], &aligned[..4]);
        assert_eq!(
            event::entry_digest(&trimmed)?,
            event::entry_digest(&aligned)?
        );
    }
    let (order, bytes, range) = builder.finish()?;
    let index = analogue::decode(&bytes, &order)?;
    let mut from_originals = Builder::new(1u8.into())?;
    for (i, event) in originals.iter().enumerate() {
        from_originals.append(&(i + 1).into(), event)?;
    }
    let (_, same, same_range) = from_originals.finish()?;
    assert_eq!(
        bytes, same,
        "transport margins cannot change the canonical BulkOrder"
    );
    assert_eq!(range, same_range);
    assert_eq!(index.ends, expected_ends);
    assert_eq!(index.ends.len(), originals.len());
    let mut recovered = Datashed::default();
    for (i, variant) in variants.iter().enumerate() {
        let selected = index.range(&(i + 1).into(), &(i + 2).into())?;
        assert_eq!(
            &bytes[selected.clone()],
            event::trim_retry_margins(variant)?.as_ref()
        );
        assert!(matches!(
            event::trim_retry_margins(&bytes[selected.clone()])?,
            std::borrow::Cow::Borrowed(_)
        ));
        recovered.apply_entry((i + 1).into(), &bytes[selected])?;
    }
    assert_eq!(recovered.prefix(), expected.prefix());
    assert_eq!(recovered.coordinate, logical.into());
    assert_eq!(index.coordinate_at(&order.highest)?, recovered.coordinate);
    assert_eq!(range.start, logical);
    assert_ne!(physical, logical);
    assert_eq!(
        recovered.database.unwrap().image,
        expected.database.unwrap().image
    );
    Ok(())
}

#[test]
fn crc32c_accumulates_exactly_the_margin_free_coordinate_run() -> Result<()> {
    let originals = events()?;
    let mut accumulated = vec![];
    let mut before = 0;
    let mut state = Datashed::default();
    for (i, original) in originals.iter().enumerate() {
        let trimmed = event::trim_retry_margins(original)?;
        accumulated.extend_from_slice(&trimmed);
        let after = basinsync::checksum::event(before, original)?;
        assert_eq!(after, crc32c::crc32c(&accumulated));
        assert_eq!(after, basinsync::checksum::event(before, &trimmed)?);
        let headers = basinsync::timeline::entry_headers(original, before)?;
        assert_eq!(
            headers["x-amz-meta-crc32c"],
            basinsync::checksum::format(after)
        );
        assert_eq!(
            headers["x-amz-meta-base-crc32c"],
            basinsync::checksum::format(before)
        );
        state.apply_entry((i + 1).into(), original)?;
        assert_eq!(state.crc32c, after);
        before = after;
        for power in 0..=5 {
            let mut aligned = vec![];
            basinsync::envbuf::append_with_alignment(
                &mut aligned,
                event::envelope(original)?.content,
                basinsync::envbuf::NESTED,
                power,
            )?;
            let normalized = event::trim_retry_margins(&aligned)?;
            assert_eq!(
                basinsync::checksum::event(0, &aligned)?,
                crc32c::crc32c(&normalized)
            );
        }
    }
    Ok(())
}
