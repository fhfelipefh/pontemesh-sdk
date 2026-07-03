use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub output: PathBuf,
    pub object_sizes: Vec<u64>,
    pub fragment_sizes: Vec<usize>,
    pub downloaders: Vec<usize>,
    pub runs: usize,
}

impl BenchmarkConfig {
    pub fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Self {
            output: PathBuf::from("target/pontemesh-benchmarks"),
            object_sizes: vec![mib(1), mib(10), mib(100)],
            fragment_sizes: vec![kib(64) as usize, kib(256) as usize, mib(1) as usize],
            downloaders: vec![1, 3, 5, 10],
            runs: 3,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--quick" => {
                    config.object_sizes = vec![mib(1)];
                    config.fragment_sizes = vec![kib(256) as usize];
                    config.downloaders = vec![1, 3];
                    config.runs = 1;
                }
                "--output" => config.output = PathBuf::from(require_value(&mut args, &arg)?),
                "--object-sizes" => {
                    config.object_sizes = parse_size_list(&require_value(&mut args, &arg)?)?
                }
                "--fragment-sizes" => {
                    config.fragment_sizes = parse_size_list(&require_value(&mut args, &arg)?)?
                        .into_iter()
                        .map(|value| value as usize)
                        .collect()
                }
                "--downloaders" => {
                    config.downloaders = parse_usize_list(&require_value(&mut args, &arg)?)?
                }
                "--runs" => {
                    config.runs = require_value(&mut args, &arg)?
                        .parse()
                        .map_err(|_| "invalid --runs value".to_string())?
                }
                "--help" | "-h" => return Err(usage()),
                other => return Err(format!("unknown argument {other}\n{}", usage())),
            }
        }
        if config.runs == 0
            || config.object_sizes.is_empty()
            || config.fragment_sizes.is_empty()
            || config.downloaders.is_empty()
        {
            return Err("benchmark matrix cannot be empty".to_string());
        }
        Ok(config)
    }
}

fn require_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_usize_list(value: &str) -> Result<Vec<usize>, String> {
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse()
                .map_err(|_| format!("invalid integer value {part}"))
        })
        .collect()
}

fn parse_size_list(value: &str) -> Result<Vec<u64>, String> {
    value.split(',').map(parse_size).collect()
}

fn parse_size(value: &str) -> Result<u64, String> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("KiB") {
        (number, kib(1))
    } else if let Some(number) = value.strip_suffix("MiB") {
        (number, mib(1))
    } else if let Some(number) = value.strip_suffix("GiB") {
        (number, 1024 * mib(1))
    } else {
        (value, 1)
    };
    let number: u64 = number
        .parse()
        .map_err(|_| format!("invalid size value {value}"))?;
    Ok(number * multiplier)
}

fn kib(value: u64) -> u64 {
    value * 1024
}

fn mib(value: u64) -> u64 {
    kib(value) * 1024
}

fn usage() -> String {
    "usage: p2p-bench [--quick] [--output DIR] [--object-sizes 1MiB,10MiB] [--fragment-sizes 64KiB,1MiB] [--downloaders 1,3,5] [--runs N]".to_string()
}
