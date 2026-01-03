use anyhow::Result;
use clap::Parser;
use mdscreensnap::runtime::RealRuntime;
use mdscreensnap::{run, Args};

fn main() -> Result<()> {
    let args = Args::parse();
    let runtime = RealRuntime;
    run(args, &runtime)
}
