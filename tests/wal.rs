use anyhow::Result;
use basinsync::{
    event::{self, FlowCommand, Marker},
    lexidecimal::Viewpoint,
    timeline::Datashed,
    turso::{State, wal::Writer},
};
use num_bigint::BigUint;
use std::collections::BTreeMap;
fn command(flow: &str, vp: &str) -> FlowCommand {
    FlowCommand {
        markers: BTreeMap::from([(
            flow.into(),
            Marker {
                context: Default::default(),
                edges: Default::default(),
                verified: Default::default(),
                viewpoint: vp.into(),
                sha384: event::digest(&[0, 255, 128]),
                blob: vec![0, 255, 128],
            },
        )]),
    }
}

#[test]
fn unpublished_state_is_not_readable_and_bootstrap_is_verified() -> Result<()> {
    let mut state = Datashed::default();
    let prepared = state.prepare_wal(vec![command("f", "0")])?;
    assert!(state.latest("f").is_err());
    assert!(state.prepare_wal(vec![command("f", "1")]).is_err());
    let bytes = prepared.bytes().to_vec();
    let mut recovered = Datashed::default();
    recovered.apply_entry(1u8.into(), &bytes)?;
    assert_eq!(recovered.latest("f")?.unwrap().decimal(), "0");
    let event::Entry::Wal {
        commands,
        mut delta,
        ..
    } = event::decode_entry(&bytes)?
    else {
        panic!()
    };
    delta.logical.base.as_mut().unwrap().0[0] ^= 1;
    let mut empty = Datashed::default();
    assert!(
        empty
            .apply_entry(1u8.into(), &event::encode_wal(commands, *delta)?)
            .is_err()
    );
    assert!(empty.latest("f")?.is_none());
    Ok(())
}

#[test]
fn concatenated_events_keep_boundaries_alignment_ranges_and_recovery() -> Result<()> {
    let mut stager = Datashed::default().into_stager();
    let mut events = Vec::new();
    for n in 0..3 {
        events.push(
            stager
                .prepare(vec![command("f", &n.to_string())])?
                .bytes()
                .to_vec(),
        );
    }
    let mut feed = vec![];
    basinsync::envbuf::append(&mut feed, &[1], basinsync::envbuf::MARGIN)?;
    for bytes in &events {
        feed.extend_from_slice(bytes);
    }
    let mut recovered = Datashed::default();
    let frames = basinsync::envbuf::all(&feed)?;
    assert_eq!(frames.len(), 7);
    let mut end = feed.len();
    for frame in frames.iter().rev() {
        assert_eq!(*frame, basinsync::envbuf::before(&feed, end)?);
        end = frame.start;
    }
    assert_eq!(end, 0);
    for (n, frame) in frames
        .iter()
        .filter(|frame| frame.format == basinsync::envbuf::NESTED)
        .enumerate()
    {
        assert_eq!(frame.format, basinsync::envbuf::NESTED);
        let bytes = &feed[frame.start..frame.end];
        let borrowed = basinsync::envbuf::nested_message(bytes)?;
        assert_eq!(borrowed.as_ptr(), frame.content.as_ptr());
        for inner in basinsync::envbuf::all(borrowed)? {
            assert_eq!((frame.content_start + inner.content_start) % 8, 0);
        }
        assert_eq!(
            event::entry_digest(bytes)?,
            event::entry_digest(&events[n])?
        );
        for range in event::blob_ranges(bytes)? {
            let start = frame.start + usize::try_from(range.offset)?;
            let blob = &feed[start..start + usize::try_from(range.length)?];
            assert_eq!(blob, &[0, 255, 128]);
        }
        recovered.apply_entry((n + 1).into(), bytes)?;
    }
    assert_eq!(recovered.prefix(), stager.prefix());
    assert_eq!(recovered.latest("f")?.unwrap().decimal(), "2");
    assert!(event::decode_entry(&feed).is_err());
    assert!(event::decode_entry(event::envelope(&events[0])?.content).is_err());
    Ok(())
}

#[test]
fn retry_margin_preserves_prepared_successors_and_locations() -> Result<()> {
    let mut stager = Datashed::default().into_stager();
    let first = stager.prepare(vec![command("f", "0")])?;
    let second = stager.prepare(vec![command("f", "1")])?;
    let digest = event::entry_digest(first.bytes())?;
    let locations = event::blob_ranges(first.bytes())?;
    let manifest = event::manifest_range(first.bytes())?;
    let mut retried = first.bytes().to_vec();
    for counter in [0u64, 1, 256, u64::MAX] {
        let mut margin = vec![];
        basinsync::envbuf::append(
            &mut margin,
            &counter.to_le_bytes(),
            basinsync::envbuf::MARGIN,
        )?;
        let start = event::envelope(first.bytes())?.end;
        retried[start..start + margin.len()].copy_from_slice(&margin);
        assert_eq!(retried.len(), first.bytes().len());
        assert_eq!(event::entry_digest(&retried)?, digest);
        assert_ne!(event::digest(&retried), digest);
        assert_eq!(event::blob_ranges(&retried)?, locations);
        assert_eq!(event::manifest_range(&retried)?, manifest);
    }
    let mut recovered = Datashed::default();
    recovered.apply_entry(1u8.into(), &retried)?;
    recovered.apply_entry(2u8.into(), second.bytes())?;
    assert_eq!(recovered.prefix(), stager.prefix());
    assert_eq!(recovered.latest("f")?.unwrap().decimal(), "1");
    Ok(())
}

#[test]
fn legacy_history_can_be_extended_with_wal_batches() -> Result<()> {
    let mut reader = Datashed::default();
    let (_, legacy) = reader.prepare(command("f", "0").markers)?;
    let bytes = event::encode(legacy)?;
    reader.apply_entry(1u8.into(), &bytes)?;
    let prepared = reader.prepare_wal(vec![
        command("f", "1"),
        command("f", "2"),
        command("g", "0"),
    ])?;
    let mut recovered = Datashed::default();
    recovered.apply_entry(1u8.into(), &bytes)?;
    recovered.apply_entry(2u8.into(), prepared.bytes())?;
    assert_eq!(recovered.latest("f")?.unwrap().decimal(), "2");
    assert_eq!(recovered.latest("g")?.unwrap().decimal(), "0");
    assert_eq!(recovered.head, Some(event::entry_digest(prepared.bytes())?));
    Ok(())
}

#[test]
fn corrupt_frames_lineage_order_and_versions_fail_atomically() -> Result<()> {
    let mut writer = Writer::open(&State::genesis()?.image)?;
    let commands = vec![command("f", "0")];
    let delta = writer.prepare(&0u8.into(), None, &commands)?;
    let first = event::encode_wal(commands, delta)?;
    let mut reader = Datashed::default();
    reader.apply_entry(1u8.into(), &first)?;
    let commands = vec![command("f", "1"), command("f", "2")];
    let delta = writer.prepare(&1u8.into(), Some(&event::entry_digest(&first)?), &commands)?;
    for fault in 0..7 {
        let mut bad = delta.clone();
        let mut inputs = commands.clone();
        match fault {
            0 => bad.version = "unknown".into(),
            1 => bad.physical.parent = Some("00".repeat(48)),
            2 => bad.physical.segments[0].0[56] ^= 1,
            3 => {
                bad.physical.segments[0].0.pop();
            }
            4 => bad.logical.frames[0].0[0] ^= 1,
            5 => inputs.reverse(),
            _ => bad.physical.format = "unknown".into(),
        }
        assert!(
            reader
                .apply_entry(2u8.into(), &event::encode_wal(inputs, bad)?)
                .is_err(),
            "fault {fault}"
        );
        assert_eq!(reader.milestone, 1u8.into());
        assert_eq!(reader.latest("f")?.unwrap().decimal(), "0");
        assert_eq!(reader.head, Some(event::entry_digest(&first)?));
    }
    reader.apply_entry(2u8.into(), &event::encode_wal(commands, delta)?)?;
    assert_eq!(reader.latest("f")?.unwrap().decimal(), "2");
    Ok(())
}

#[test]
fn wal_preserves_counters_above_machine_precision() -> Result<()> {
    let huge = BigUint::from(1u8) << 256usize;
    let (base, _) = State::genesis()?.stage(
        &huge.to_string(),
        &command("f", &huge.to_string()).markers,
        true,
    )?;
    let mut writer = Writer::open(&base.image)?;
    let next = (&huge + 1u8).to_string();
    let inputs = vec![command("f", &next)];
    let delta = writer.prepare(&huge, Some("prior-record"), &inputs)?;
    let image = delta
        .physical
        .ingest(&base.image, &huge, Some("prior-record"))?;
    let recovered = State::from_image(image)?;
    assert_eq!(recovered.milestone()?, next);
    assert_eq!(recovered.latest("f")?.unwrap().decimal(), next);
    let location = recovered.location("f", &Viewpoint::parse(&next)?)?.unwrap();
    assert_eq!(location.upstream, &huge + 1u8);
    let bytes = event::encode_wal(inputs, delta)?;
    assert_eq!(
        &bytes[location.offset as usize..(location.offset + location.length) as usize],
        &[0, 255, 128]
    );
    Ok(())
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(32))]
    #[test]
    fn persistent_wal_batches_reconstruct_at_every_prefix(batches in proptest::collection::vec(proptest::collection::vec(0u16..64,1..20),2..30)){
        let mut before=State::genesis().unwrap().image;
        let mut writer=Writer::open(&before).unwrap();let mut reader=Datashed::default();
        let mut latest:BTreeMap<String,Viewpoint>=BTreeMap::new();
        for (index,batch) in batches.into_iter().enumerate(){
            let mut commands=Vec::new();
            for id in batch {let flow=format!("{id}-snow");let vp=latest.get(&flow).map_or_else(Viewpoint::zero,Viewpoint::next);commands.push(command(&flow,&vp.decimal()));latest.insert(flow,vp);}
            let delta=writer.prepare(&reader.milestone,reader.head.as_deref(),&commands).unwrap();
            let bytes=event::encode_wal(commands,delta.clone()).unwrap();
            reader.apply_entry((index+1).into(),&bytes).unwrap();
            let image=&reader.database.as_ref().unwrap().image;
            let logical=State::ingest_logical(&before,&delta.logical).unwrap();
            proptest::prop_assert_eq!(image,&writer.image().unwrap());proptest::prop_assert_eq!(image,&logical.image);
            proptest::prop_assert_eq!(reader.database.as_ref().unwrap().flows().unwrap(),latest.clone());
            before=image.clone();
        }
    }
}

#[test]
fn byte_coordinates_are_digest_bound_contiguous_and_ignore_retry_margins() -> Result<()> {
    let mut stager = Datashed::default().into_stager();
    let first = stager.prepare(vec![command("f", "0")])?;
    let second = stager.prepare(vec![command("f", "1")])?;
    assert_eq!(event::coordinate(first.bytes())?, Some(0u8.into()));
    let mut reframed = first.bytes().to_vec();
    basinsync::envbuf::append_counter_margin(&mut reframed, 0)?;
    assert_eq!(
        event::entry_digest(&reframed)?,
        event::entry_digest(first.bytes())?
    );
    let mut unmodified = Datashed::default();
    unmodified.apply_entry(1u8.into(), &reframed)?;
    unmodified.apply_entry(2u8.into(), second.bytes())?;
    assert_eq!(unmodified.prefix(), stager.prefix());
    assert_eq!(
        event::coordinate(second.bytes())?,
        Some(event::coordinate_length(first.bytes())?.into())
    );
    assert_eq!(
        stager.prefix().coordinate(),
        &BigUint::from(
            event::coordinate_length(first.bytes())? + event::coordinate_length(second.bytes())?
        )
    );
    let event::Entry::Wal {
        commands, delta, ..
    } = event::decode_entry(first.bytes())?
    else {
        panic!()
    };
    for wrong in [1u8.into(), BigUint::from(1u8) << 256usize] {
        let bad = event::encode_wal_at(commands.clone(), *delta.clone(), &wrong)?;
        assert_eq!(event::coordinate(&bad)?, Some(wrong));
        assert_ne!(
            event::entry_digest(&bad)?,
            event::entry_digest(first.bytes())?
        );
        let mut recovered = Datashed::default();
        assert!(recovered.apply_entry(1u8.into(), &bad).is_err());
        assert_eq!(recovered.milestone, 0u8.into());
        assert!(recovered.database.is_none());
    }
    Ok(())
}

#[test]
fn external_retry_margins_leave_the_entire_event_unchanged_and_reject_other_suffixes() -> Result<()>
{
    let mut stager = Datashed::default().into_stager();
    let first = stager.prepare(vec![command("f", "0")])?;
    let second = stager.prepare(vec![command("f", "1")])?;
    let original = first.bytes();
    let outer = event::envelope(original)?;
    let bytes = event::coordinate_bytes(original)?;
    assert_eq!(outer.end, bytes.len());
    assert_eq!(
        basinsync::envbuf::all(original)?
            .iter()
            .map(|f| f.format)
            .collect::<Vec<_>>(),
        vec![4, 6]
    );
    assert_eq!(
        basinsync::envbuf::before(outer.content, outer.content.len())?.format,
        basinsync::envbuf::FLEXBUFFERS
    );
    assert_eq!(event::entry_digest(original)?, event::digest(bytes));
    assert_eq!(
        basinsync::checksum::event(0, original)?,
        basinsync::checksum::append(0, bytes)
    );
    for count in 0..=3 {
        let mut stored = bytes.to_vec();
        for n in 0..count {
            basinsync::envbuf::append(
                &mut stored,
                &vec![n as u8; n * 9],
                basinsync::envbuf::MARGIN,
            )?;
        }
        assert_eq!(event::coordinate_bytes(&stored)?, bytes);
        assert_eq!(
            event::entry_digest(&stored)?,
            event::entry_digest(original)?
        );
        let mut recovered = Datashed::default();
        recovered.apply_entry(1u8.into(), &stored)?;
        recovered.apply_entry(2u8.into(), second.bytes())?;
        assert_eq!(recovered.prefix(), stager.prefix());
    }
    for format in [basinsync::envbuf::NESTED, basinsync::envbuf::FLEXBUFFERS, 0] {
        let mut stored = original.to_vec();
        basinsync::envbuf::append(&mut stored, b"unexpected", format)?;
        assert!(event::decode_entry(&stored).is_err());
        assert!(event::coordinate_bytes(&stored).is_err());
    }
    let mut truncated = original.to_vec();
    truncated.pop();
    assert!(event::decode_entry(&truncated).is_err());
    // Margins within the command feed belong in its manifest; they cannot trail it.
    let mut internal = vec![0; 8];
    internal.extend_from_slice(outer.content);
    basinsync::envbuf::append_counter_margin(&mut internal, 0)?;
    let internal = basinsync::envbuf::finish_nested(internal)?;
    assert!(event::decode_entry(&internal).is_err());
    Ok(())
}
