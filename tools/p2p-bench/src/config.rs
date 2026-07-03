use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub profile: BenchmarkProfile,
    pub output: PathBuf,
    pub object_sizes: Vec<u64>,
    pub fragment_sizes: Vec<usize>,
    pub downloaders: Vec<usize>,
    pub runs: usize,
    pub force: bool,
    pub concurrent_downloaders: bool,
    pub scenario_timeout: Duration,
    pub benchmark_timeout: Duration,
    pub fragment_request_timeout: Duration,
}

impl BenchmarkConfig {
    pub fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut config = Self::for_profile(BenchmarkProfile::Production);
        config.output = PathBuf::from("target/pontemesh-benchmarks");
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--profile" => {
                    let output = config.output.clone();
                    let force = config.force;
                    let profile = BenchmarkProfile::parse(&require_value(&mut args, &arg)?)?;
                    config = Self::for_profile(profile);
                    config.output = output;
                    config.force = force;
                }
                "--quick" => {
                    let output = config.output.clone();
                    let force = config.force;
                    config = Self::for_profile(BenchmarkProfile::Quick);
                    config.output = output;
                    config.force = force;
                }
                "--force" => config.force = true,
                "--concurrent-downloaders" => {
                    config.concurrent_downloaders = parse_bool(&require_value(&mut args, &arg)?)?
                }
                "--scenario-timeout-secs" => {
                    config.scenario_timeout =
                        Duration::from_secs(parse_u64(&require_value(&mut args, &arg)?)?)
                }
                "--benchmark-timeout-secs" => {
                    config.benchmark_timeout =
                        Duration::from_secs(parse_u64(&require_value(&mut args, &arg)?)?)
                }
                "--fragment-request-timeout-secs" => {
                    config.fragment_request_timeout =
                        Duration::from_secs(parse_u64(&require_value(&mut args, &arg)?)?)
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

    fn for_profile(profile: BenchmarkProfile) -> Self {
        match profile {
            BenchmarkProfile::Quick => Self {
                profile,
                output: PathBuf::from("target/pontemesh-benchmarks-quick"),
                object_sizes: vec![mib(1)],
                fragment_sizes: vec![kib(256) as usize],
                downloaders: vec![1, 3],
                runs: 1,
                force: false,
                concurrent_downloaders: true,
                scenario_timeout: Duration::from_secs(300),
                benchmark_timeout: Duration::from_secs(900),
                fragment_request_timeout: Duration::from_secs(10),
            },
            BenchmarkProfile::Ci => Self {
                profile,
                output: PathBuf::from("target/pontemesh-benchmarks-ci"),
                object_sizes: vec![mib(1), mib(10)],
                fragment_sizes: vec![kib(256) as usize, mib(1) as usize],
                downloaders: vec![1, 3],
                runs: 1,
                force: false,
                concurrent_downloaders: true,
                scenario_timeout: Duration::from_secs(600),
                benchmark_timeout: Duration::from_secs(3_600),
                fragment_request_timeout: Duration::from_secs(10),
            },
            BenchmarkProfile::Production => Self {
                profile,
                output: PathBuf::from("target/pontemesh-benchmarks-production"),
                object_sizes: vec![mib(1), mib(10), mib(100)],
                fragment_sizes: vec![kib(64) as usize, kib(256) as usize, mib(1) as usize],
                downloaders: vec![1, 3, 5, 10],
                runs: 3,
                force: false,
                concurrent_downloaders: true,
                scenario_timeout: Duration::from_secs(1_800),
                benchmark_timeout: Duration::from_secs(28_800),
                fragment_request_timeout: Duration::from_secs(10),
            },
            BenchmarkProfile::Stress => Self {
                profile,
                output: PathBuf::from("target/pontemesh-benchmarks-stress"),
                object_sizes: vec![mib(100), mib(250), mib(500)],
                fragment_sizes: vec![kib(256) as usize, mib(1) as usize, mib(4) as usize],
                downloaders: vec![5, 10, 25],
                runs: 3,
                force: false,
                concurrent_downloaders: true,
                scenario_timeout: Duration::from_secs(3_600),
                benchmark_timeout: Duration::from_secs(86_400),
                fragment_request_timeout: Duration::from_secs(10),
            },
            BenchmarkProfile::Soak => Self {
                profile,
                output: PathBuf::from("target/pontemesh-benchmarks-soak"),
                object_sizes: vec![mib(100)],
                fragment_sizes: vec![mib(1) as usize],
                downloaders: vec![10],
                runs: 30,
                force: false,
                concurrent_downloaders: true,
                scenario_timeout: Duration::from_secs(1_800),
                benchmark_timeout: Duration::from_secs(7_200),
                fragment_request_timeout: Duration::from_secs(10),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkProfile {
    Quick,
    Ci,
    Production,
    Stress,
    Soak,
}

impl BenchmarkProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Ci => "ci",
            Self::Production => "production",
            Self::Stress => "stress",
            Self::Soak => "soak",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "quick" => Ok(Self::Quick),
            "ci" => Ok(Self::Ci),
            "production" => Ok(Self::Production),
            "stress" => Ok(Self::Stress),
            "soak" => Ok(Self::Soak),
            other => Err(format!("invalid profile {other}")),
        }
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

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid integer value {value}"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        other => Err(format!("invalid boolean value {other}")),
    }
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
    "usage: p2p-bench [--profile quick|ci|production|stress|soak] [--quick] [--force] [--output DIR] [--object-sizes 1MiB,10MiB] [--fragment-sizes 64KiB,1MiB] [--downloaders 1,3,5] [--runs N]".to_string()
}
