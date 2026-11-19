use anyhow::{Context, Result};
use basinsync::{analogue, recovery, timeline};
use s3::{Bucket, Region, creds::Credentials};
use std::io::{Read, Write};

#[tokio::main]
async fn main() -> Result<()> {
    s3::set_retries(0);
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--generate") => {
            let count: usize = args.get(2).context("missing count")?.parse()?;
            let length: usize = args.get(3).context("missing blob length")?.parse()?;
            let mut state = timeline::Basin::default().into_stager();
            let mut output = std::io::BufWriter::new(std::io::stdout());
            for n in 0..count {
                let blob = vec![(n % 251) as u8; length];
                let marker = basinsync::event::Marker {
                    viewpoint: n.to_string(),
                    sha384: basinsync::event::digest(&blob),
                    blob,
                    edges: Default::default(),
                    context: Default::default(),
                    verified: Default::default(),
                };
                let command = basinsync::event::FlowCommand {
                    markers: std::collections::BTreeMap::from([("f".into(), marker)]),
                };
                output.write_all(state.prepare(vec![command])?.bytes())?;
            }
            output.flush()?;
        }
        Some("--decode") => {
            let mut bytes = vec![];
            std::io::stdin().read_to_end(&mut bytes)?;
            let (index, range) = analogue::footer(&bytes)?;
            analogue::decode(&bytes, &index.order())?;
            println!(
                "{}",
                serde_json::json!({"coordinate":index.coordinate.to_string(),"lowest":index.lowest.to_string(),"highest":index.order().highest.to_string(),"ends":index.ends,"footer":[range.start,range.end]})
            );
        }
        Some("--pack") => {
            let mut bytes = vec![];
            std::io::stdin().read_to_end(&mut bytes)?;
            let mut next: num_bigint::BigUint = args.get(2).context("missing lowest")?.parse()?;
            let mut builder = analogue::Builder::new(next.clone())?;
            for event in basinsync::envbuf::all(&bytes)?
                .into_iter()
                .filter(|event| event.format != basinsync::envbuf::MARGIN)
            {
                builder.append(&next, &bytes[event.start..event.end])?;
                next += 1u8;
            }
            let (_, bytes, _) = builder.finish()?;
            std::io::stdout().write_all(&bytes)?;
        }
        Some("--recover") => {
            let credentials = Credentials::new(
                Some(&std::env::var("AWS_ACCESS_KEY_ID")?),
                Some(&std::env::var("AWS_SECRET_ACCESS_KEY")?),
                None,
                None,
                None,
            )?;
            let bucket = Bucket::new(
                &std::env::var("S3_BUCKET")?,
                Region::Custom {
                    region: "us-east-1".into(),
                    endpoint: std::env::var("S3_ENDPOINT")?,
                },
                credentials,
            )?
            .with_path_style();
            let mut config = recovery::Config::from_env()?;
            if let Some(mode) = args.get(2) {
                config.mode = recovery::Mode::parse(mode)?;
            }
            let started = std::time::Instant::now();
            let state = timeline::recover_with(&bucket, config).await?;
            let elapsed = started.elapsed();
            let database = state.database.context("empty recovery")?;
            let flows: std::collections::BTreeMap<_, _> = database
                .flows()?
                .into_iter()
                .map(|(flow, point)| (flow, point.decimal()))
                .collect();
            println!(
                "{}",
                serde_json::json!({"kernel_sha384":state.kernel_database.as_ref().map(|db|basinsync::event::digest(&db.image)),"crc32c":basinsync::checksum::format(state.crc32c),"coordinate":state.coordinate.to_string(),"elapsed_ms":elapsed.as_secs_f64()*1000.,"milestone":state.milestone.to_string(),"sha384":basinsync::event::digest(&database.image),"flows":flows,"stats":state.recovery_stats})
            );
        }
        _ => anyhow::bail!("expected --generate, --pack, --decode, or --recover"),
    }
    Ok(())
}
