use capcover::bench::run as run_bench;
use capcover::build::{build, BuildConfig};
use capcover::error::{Error, Result};
use capcover::fixture::{generate, write_f32, FixtureKind};
use capcover::index::CapCover;
use capcover::vector::Vectors;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "capcover",
    version,
    about = "Zero-false-negative semantic pre-filter"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build {
        #[arg(long)]
        vectors: PathBuf,
        #[arg(long)]
        dim: Option<usize>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "2%")]
        budget: String,
        #[arg(long, default_value_t = 0)]
        seed: u64,
    },
    Check {
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value_t = 0.95)]
        similarity: f32,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Verify {
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        vectors: PathBuf,
        #[arg(long)]
        full: bool,
    },
    Bench {
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        vectors: PathBuf,
        #[arg(long)]
        queries: PathBuf,
        #[arg(long, default_value_t = 0.95)]
        similarity: f32,
        #[arg(long, value_enum, default_value_t = Backend::Brute)]
        backend: Backend,
        #[arg(long)]
        baseline_ms: Option<f64>,
        #[arg(long, default_value_t = 10)]
        top_k: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Info {
        #[arg(long)]
        index: PathBuf,
    },
    Selftest,
    Gen {
        #[arg(long, value_enum)]
        kind: FixtureKindArg,
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long, default_value_t = 32)]
        dim: usize,
        #[arg(long, default_value_t = 100)]
        queries: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        #[arg(long, default_value_t = 8)]
        clusters: usize,
        #[arg(long, default_value_t = 0.1)]
        sigma: f64,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        queries_out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Text,
    Jsonl,
    Bitmap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Backend {
    Brute,
    Usearch,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FixtureKindArg {
    Clustered,
    Duplicates,
    Uniform,
    Edge,
}

impl FixtureKindArg {
    fn into_kind(self) -> FixtureKind {
        match self {
            FixtureKindArg::Clustered => FixtureKind::Clustered,
            FixtureKindArg::Duplicates => FixtureKind::Duplicates,
            FixtureKindArg::Uniform => FixtureKind::Uniform,
            FixtureKindArg::Edge => FixtureKind::Edge,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("capcover: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Build {
            vectors,
            dim,
            out,
            budget,
            seed,
        } => {
            let vectors = Vectors::load(&vectors, dim)?;
            let budget_bytes = parse_budget(&budget, vectors.n, vectors.dim)?;
            let built = build(&vectors, &BuildConfig { budget_bytes, seed })?;
            built.save(&out)?;
            let info = CapCover::open(&out)?.info();
            println!(
                "built n={} d={} k={} file_bytes={} budget_bytes={}",
                info.n, info.dim, info.k, info.file_bytes, info.budget_bytes
            );
            Ok(())
        }
        Command::Check {
            index,
            queries,
            similarity,
            format,
            out,
        } => {
            let index = CapCover::open(&index)?;
            let queries = Vectors::load_queries(&queries, index.dim())?;
            let mut decisions = vec![false; queries.n];
            index.may_contain_batch(&queries, similarity, &mut decisions)?;
            let accepted = decisions.iter().filter(|value| **value).count();
            if out.is_some() && format != OutputFormat::Bitmap {
                return Err(Error::InvalidData(
                    "--out is only valid with --format bitmap".into(),
                ));
            }
            match format {
                OutputFormat::Text => {
                    let stdout = std::io::stdout();
                    let mut lock = stdout.lock();
                    for decision in &decisions {
                        writeln!(lock, "{decision}")?;
                    }
                }
                OutputFormat::Jsonl => {
                    for (query, decision) in decisions.iter().enumerate() {
                        println!("{{\"query\":{query},\"may_contain\":{decision}}}");
                    }
                }
                OutputFormat::Bitmap => {
                    let path = out
                        .ok_or_else(|| Error::InvalidData("bitmap output requires --out".into()))?;
                    let bytes: Vec<u8> = decisions
                        .iter()
                        .map(|decision| u8::from(*decision))
                        .collect();
                    fs::write(path, bytes)?;
                }
            }
            eprintln!(
                "queries={} accepted={} rejected={}",
                queries.n,
                accepted,
                queries.n - accepted
            );
            Ok(())
        }
        Command::Verify {
            index,
            vectors,
            full,
        } => {
            let index = CapCover::open(&index)?;
            let vectors = Vectors::load(&vectors, Some(index.dim()))?;
            let report = index.verify(&vectors, full)?;
            println!(
                "verify n={} uncovered={} mode={}",
                report.n,
                report.uncovered,
                if report.full { "full" } else { "assignment" }
            );
            if report.uncovered > 0 {
                if let Some(index) = report.first_uncovered {
                    eprintln!("first uncovered vector: {index}");
                }
                std::process::exit(2);
            }
            Ok(())
        }
        Command::Bench {
            index,
            vectors,
            queries,
            similarity,
            backend,
            baseline_ms,
            top_k,
            out,
        } => {
            let _ = top_k;
            let index = CapCover::open(&index)?;
            let vectors = Vectors::load(&vectors, Some(index.dim()))?;
            let queries = Vectors::load_queries(&queries, index.dim())?;
            let json = match backend {
                Backend::Brute => serde_json::to_string_pretty(&run_bench(
                    &index,
                    &vectors,
                    &queries,
                    similarity,
                    baseline_ms,
                )?)
                .map_err(|error| Error::InvalidData(error.to_string()))?,
                Backend::Usearch => {
                    #[cfg(feature = "usearch")]
                    {
                        serde_json::to_string_pretty(&capcover::usearch::run(
                            &index, &vectors, &queries, similarity, top_k,
                        )?)
                        .map_err(|error| Error::InvalidData(error.to_string()))?
                    }
                    #[cfg(not(feature = "usearch"))]
                    {
                        return Err(Error::InvalidData(
                            "usearch backend requires --features usearch".into(),
                        ));
                    }
                }
            };
            match out {
                Some(path) => fs::write(path, json)?,
                None => println!("{json}"),
            }
            Ok(())
        }
        Command::Info { index } => {
            let info = CapCover::open(&index)?.info();
            println!("magic CCV1");
            println!("version 1");
            println!("dim {}", info.dim);
            println!("n {}", info.n);
            println!("k {}", info.k);
            println!("seed {}", info.seed);
            println!("encoding {}", info.encoding);
            println!("budget_bytes {}", info.budget_bytes);
            println!("file_bytes {}", info.file_bytes);
            println!("header_bytes {}", info.header_bytes);
            println!("direction_bytes {}", info.direction_bytes);
            println!("angle_bytes {}", info.angle_bytes);
            println!("angle_padding {}", info.angle_padding);
            println!("assignment_bytes {}", info.assignment_bytes);
            println!(
                "bytes_per_vector {:.3}",
                info.file_bytes as f64 / info.n as f64
            );
            println!("checksum {:016x}", info.checksum);
            Ok(())
        }
        Command::Selftest => selftest(),
        Command::Gen {
            kind,
            n,
            dim,
            queries,
            seed,
            clusters,
            sigma,
            out,
            queries_out,
        } => {
            let fixture = generate(kind.into_kind(), n, dim, seed, queries, clusters, sigma)?;
            write_f32(&out, &fixture.vectors)?;
            write_f32(&queries_out, &fixture.queries)?;
            println!(
                "wrote vectors n={} d={} and queries n={} d={}",
                fixture.vectors.n, fixture.vectors.dim, fixture.queries.n, fixture.queries.dim
            );
            Ok(())
        }
    }
}

fn parse_budget(value: &str, n: usize, dim: usize) -> Result<u64> {
    let raw_bytes = (n as u64)
        .checked_mul(dim as u64)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| Error::InvalidData("input size overflows u64".into()))?;
    if let Some(percent) = value.strip_suffix('%') {
        let percent: f64 = percent
            .parse()
            .map_err(|_| Error::InvalidData(format!("invalid budget percentage: {value}")))?;
        if !percent.is_finite() || percent <= 0.0 || percent > 100.0 {
            return Err(Error::InvalidData(format!(
                "budget percentage must be in (0, 100]: {value}"
            )));
        }
        let bytes = (raw_bytes as f64 * percent / 100.0).round();
        if bytes < 1.0 {
            return Err(Error::InvalidData("budget rounds to zero".into()));
        }
        Ok(bytes as u64)
    } else {
        let bytes: u64 = value
            .parse()
            .map_err(|_| Error::InvalidData(format!("invalid budget: {value}")))?;
        if bytes == 0 {
            return Err(Error::InvalidData("budget must be positive".into()));
        }
        Ok(bytes)
    }
}

fn selftest() -> Result<()> {
    let fixture = generate(FixtureKind::Clustered, 256, 16, 7, 64, 8, 0.08)?;
    let budget_bytes = (fixture.vectors.n * fixture.vectors.dim * 4 / 5) as u64;
    let built = build(
        &fixture.vectors,
        &BuildConfig {
            budget_bytes,
            seed: 7,
        },
    )?;
    let path = std::env::temp_dir().join(format!("capcover-selftest-{}.ccv", std::process::id()));
    built.save(&path)?;
    let index = CapCover::open(&path)?;
    let report = index.verify(&fixture.vectors, true)?;
    let mut decisions = vec![false; fixture.vectors.n];
    index.may_contain_batch(&fixture.vectors, 0.9, &mut decisions)?;
    let all_accepted = decisions.iter().all(|decision| *decision);
    let _ = fs::remove_file(&path);
    if report.uncovered != 0 {
        return Err(Error::InvalidData(format!(
            "selftest found {} uncovered vectors",
            report.uncovered
        )));
    }
    if !all_accepted {
        return Err(Error::InvalidData(
            "selftest rejected a stored vector".into(),
        ));
    }
    println!(
        "selftest ok n={} d={} k={} uncovered=0",
        index.n(),
        index.dim(),
        index.k()
    );
    Ok(())
}
