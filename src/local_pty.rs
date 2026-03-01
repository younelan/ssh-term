use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};

pub struct LocalPty {
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub reader: Box<dyn Read + Send>,
    pub writer: Box<dyn Write + Send>,
}

pub fn spawn_local_shell(cols: u16, rows: u16) -> Result<LocalPty, Box<dyn std::error::Error + Send + Sync>> {
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let shell = if cfg!(target_os = "windows") {
        "powershell.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };

    let mut cmd = CommandBuilder::new(shell);
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd);
    }
    // On macOS/Linux, we might want to start as a login shell
    #[cfg(not(target_os = "windows"))]
    {
        if std::env::var("SHELL").is_ok() {
            cmd.arg("-l");
        }
    }

    let _child = pair.slave.spawn_command(cmd)?;
    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;

    Ok(LocalPty {
        master: pair.master,
        reader,
        writer,
    })
}
