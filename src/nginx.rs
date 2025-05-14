use anyhow::{bail, Context, Result};
use std::process::Command;

/// Reloads nginx, checking that the current site configurations are valid and then restarting.
/// Returns any error which occurs, otherwise returns a closure which reloads nginx again.
pub fn reload() -> Result<Box<dyn FnOnce() -> Result<String>>> {
    // TODO: add handling for running in a snap
    do_reload()?;
    let restore = || {
        do_reload()?;
        Ok(String::from("Reloaded nginx"))
    };
    Ok(Box::new(restore))
}

fn do_reload() -> Result<()> {
    if !Command::new("systemctl")
        .arg("reload")
        .arg("nginx.service") // TODO: is this portable?
        .status()
        .context("Failed to execute systemctl")?
        .success()
    {
        let _ = Command::new("journalctl") // just dump whatever journalctl returns
            .arg("-xeu")
            .arg("nginx.service")
            .status();
        bail!("Error when reloading nginx")
    }
    Ok(())
}

// /// Restarts nginx, returning any error which occurs. Returns a closure which restarts nginx again.
// pub fn restart() -> Result<impl FnOnce() -> Result<String>> {
//     // TODO: add handling for running in a snap
//     do_restart()?;
//     let restore = || {
//         do_restart()?;
//         Ok(String::from("Restarted nginx"))
//     };
//     Ok(restore)
// }

// fn do_restart() -> Result<()> {
//     if !Command::new("systemctl")
//         .arg("restart")
//         .arg("nginx.service") // TODO: is this portable?
//         .status()
//         .context("Failed to execute systemctl")?
//         .success()
//     {
//         let _ = Command::new("journalctl") // just dump whatever journalctl returns
//             .arg("-xeu")
//             .arg("nginx.service")
//             .status();
//         "Error when restarting nginx"
//     );
//     Ok(())
// }
