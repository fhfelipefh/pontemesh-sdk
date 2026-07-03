mod config;
mod metrics;
mod object_factory;
mod report;
mod runner;
mod scenario;
mod table;

use std::process::ExitCode;

use config::BenchmarkConfig;
use runner::run_benchmark;

fn main() -> ExitCode {
    match BenchmarkConfig::from_args(std::env::args().skip(1)).and_then(run_benchmark) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("p2p-bench failed: {error}");
            ExitCode::FAILURE
        }
    }
}
