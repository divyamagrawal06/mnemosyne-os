use std::{
    error::Error,
    io::{self, Write},
};

fn main() -> Result<(), Box<dyn Error>> {
    let report = mnemosyne_replay_lab::demo()?;
    serde_json::to_writer_pretty(io::stdout().lock(), &report)?;
    writeln!(io::stdout().lock())?;
    if !report.passed {
        return Err("replay demo did not meet its invariants".into());
    }
    Ok(())
}
