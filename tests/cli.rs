mod util;

use capcover::index::CapCover;
use capcover::vector::Vectors;
use std::fs;
use std::process::Command;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_capcover"))
}

#[test]
fn cli_round_trip() {
    let directory = util::temp_path("cli");
    fs::create_dir_all(&directory).unwrap();
    let vectors = directory.join("v.f32");
    let queries = directory.join("q.f32");
    let index = directory.join("c.ccv");
    let bitmap = directory.join("bits.bin");

    let status = binary()
        .args([
            "gen",
            "--kind",
            "clustered",
            "--n",
            "200",
            "--dim",
            "16",
            "--queries",
            "20",
            "--seed",
            "3",
            "--out",
        ])
        .arg(&vectors)
        .arg("--queries-out")
        .arg(&queries)
        .status()
        .unwrap();
    assert!(status.success());

    let status = binary()
        .args(["build", "--vectors"])
        .arg(&vectors)
        .args(["--dim", "16", "--budget", "10%", "--seed", "3", "--out"])
        .arg(&index)
        .status()
        .unwrap();
    assert!(status.success());

    let output = binary()
        .args(["verify", "--index"])
        .arg(&index)
        .arg("--vectors")
        .arg(&vectors)
        .arg("--full")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("uncovered=0"));

    let output = binary()
        .args(["check", "--index"])
        .arg(&index)
        .arg("--queries")
        .arg(&queries)
        .args(["--similarity", "0.95", "--format", "bitmap", "--out"])
        .arg(&bitmap)
        .output()
        .unwrap();
    assert!(output.status.success());
    let bits = fs::read(&bitmap).unwrap();
    assert_eq!(bits.len(), 20);
    let opened = CapCover::open(&index).unwrap();
    let query_vectors = Vectors::load_queries(&queries, opened.dim()).unwrap();
    for (query, bit) in bits.iter().enumerate() {
        let decision = opened.may_contain(query_vectors.row(query), 0.95).unwrap();
        assert_eq!(*bit, u8::from(decision));
    }

    let output = binary()
        .args(["check", "--index"])
        .arg(&index)
        .arg("--queries")
        .arg(&queries)
        .args(["--format", "text", "--out"])
        .arg(directory.join("bad.txt"))
        .output()
        .unwrap();
    assert!(!output.status.success());

    let output = binary()
        .args(["info", "--index"])
        .arg(&index)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("magic CCV1"));

    let output = binary()
        .args(["bench", "--index"])
        .arg(&index)
        .arg("--vectors")
        .arg(&vectors)
        .arg("--queries")
        .arg(&queries)
        .args(["--similarity", "0.95"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"fn\": 0"));

    let output = binary().args(["selftest"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("selftest ok"));

    fs::remove_dir_all(&directory).unwrap();
}
