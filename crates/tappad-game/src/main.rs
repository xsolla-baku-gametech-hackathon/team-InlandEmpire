//! Binary entry point; all logic lives in the library crate.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    tappad_game::run()?;
    Ok(())
}
