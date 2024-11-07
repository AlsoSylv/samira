//! Constants, as well as the schema for the lock file can be found here
//! <https://hextechdocs.dev/getting-started-with-the-lcu-api/>

//! This module also contains a list of constants for the different names
//! of the processes for `OSX`, and `Windows`

use irelia_encoder::Encoder;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::num::ParseIntError;
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

// Linux is unplayable, the constants here are only defined so the docs build
#[cfg(target_os = "windows")]
pub const CLIENT_PROCESS_NAME: &str = "LeagueClientUx.exe";
#[cfg(target_os = "macos")]
pub const CLIENT_PROCESS_NAME: &str = "LeagueClientUx";

#[cfg(target_os = "windows")]
pub const GAME_PROCESS_NAME: &str = "League of Legends.exe";
#[cfg(target_os = "macos")]
pub const GAME_PROCESS_NAME: &str = "League of Legends";

/// const copy of the encoder
pub(crate) const ENCODER: Encoder = Encoder::new();

#[cfg(all(docsrs, target_os = "linux"))]
pub const GAME_PROCESS_NAME: &str = "";
#[cfg(all(docsrs, target_os = "linux"))]
pub const CLIENT_PROCESS_NAME: &str = "";

const NOT_RUNNING: Error = Error::new(
    ErrorKind::NotRunning,
    "neither the game or client process were running",
);

const PORT_NOT_FOUND: Error = Error::new(ErrorKind::PortNotFound, "port was not found");

const AUTH_NOT_FOUND: Error = Error::new(ErrorKind::AuthTokenNotFound, "auth token was not found");

const LOCK_FILE_NOT_FOUND: Error = Error::new(
    ErrorKind::LockFileNotFound,
    "Did not follow the typical install structure",
)
.set_lockfile_error(true);

/// Gets the port and auth for the client via the process id
/// This is done to avoid needing to find the lock file, but
/// a fallback could be implemented in theory using the fact
/// that you can get the exe location, and go backwards.
///
/// # Errors
/// This will return an error if the LCU is truly not running,
/// or the lock file is inaccessibly for some reason.
/// If it returns an error for any other reason, this code
/// likely needs the client and game process names updated.
///
/// # Panics
/// Panics if the lockfile length is greater than `usize::MAX`, but this should be impossible
pub fn get_running_client(
    client_process_name: &str,
    game_process_name: &str,
    force_lock_file: bool,
) -> Result<(SocketAddrV4, String), Error> {
    // If we always read the lock file, we never need to get the command line of the process
    let cmd = if force_lock_file {
        sysinfo::UpdateKind::Never
    } else {
        sysinfo::UpdateKind::OnlyIfNotSet
    };
    // No matter what, the path to the process is required
    let refresh_kind = ProcessRefreshKind::new()
        .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
        .with_cmd(cmd);

    // Get the current list of processes
    let system = System::new_with_specifics(
        // This creates a new instance of `system` every time, so this only
        //  needs to be updated if it's not set
        RefreshKind::new().with_processes(refresh_kind),
    );

    // Is the client running, or is it the game?
    let mut client = false;

    // Iterate through all the processes, using .values() because
    // We don't need the PID. Look for a process with the same name
    // as the constant for that platform, otherwise return an error.
    let process = system
        .processes()
        .values()
        .find(|process| {
            // If it matches the name of the client,
            // set the flag, and return it
            client = process.name() == client_process_name;
            client | (process.name() == game_process_name)
        })
        .ok_or(NOT_RUNNING)?;

    // The size of the lock file is typically 53kb, but I am overallocating to stay cautious
    let mut lock_file = [0; 60];
    let [port, auth] = if client && !force_lock_file {
        // The port and auth should always be ASCII, as they are a number and a B64 buffer
        let cmd = process.cmd().iter().filter_map(|os_str| os_str.to_str());
        // Use a variable in a higher scope to make sure that port and auth get initialized
        let mut scoped_auth = None;
        let mut scoped_port = None;

        // Iterate through the command args, updating the scoped values as we go
        for s in cmd {
            if scoped_auth.is_none() {
                scoped_auth = s.strip_prefix("--remoting-auth-token=");
            }

            if scoped_port.is_none() {
                scoped_port = s.strip_prefix("--app-port=");
            }

            if scoped_auth.is_some() && scoped_port.is_some() {
                break;
            }
        }

        // Check that we found a port and auth key, otherwise error
        [
            scoped_port.ok_or(PORT_NOT_FOUND)?,
            scoped_auth.ok_or(AUTH_NOT_FOUND)?,
        ]
    } else {
        // We have to walk back twice to get the path of the lock file relative to the path of the game
        // This can only be None on Linux according to the docs, so we should be fine everywhere else
        let path = process.exe().ok_or(LOCK_FILE_NOT_FOUND)?;

        let mut dir = path.parent().ok_or(LOCK_FILE_NOT_FOUND)?;
        // Sadly, we're relying on how the client structures things here
        // Walking back a whole folder in order to get the lock file
        if !client {
            // If we're looking at the game and not the client, we need to walk back once more
            dir = dir.parent().ok_or(LOCK_FILE_NOT_FOUND)?;
        };

        let mut file = std::fs::File::open(dir.join("lockfile"))?;
        // This len shouldn't be more than a few bytes
        let len = file
            .metadata()?
            .len()
            .try_into()
            .expect("This file is always ~60 bytes");

        // Read the file initially
        let mut read = file.read(&mut lock_file)?;

        // Make sure the entire file was read, though it is so small I can't imagine it wouldn't be
        while read != len {
            read += file.read(&mut lock_file[read..])?;
        }

        // Make sure that we're not over reading into 0's
        let lock_file = std::str::from_utf8(&lock_file[..len])?;

        // Split the lock file on `:` which separates the different fields
        // Because lock_file is from a higher scope, we can split the string here
        // and return two string references later on
        let mut split = lock_file.split(':');

        [
            // Get the 3rd field, which should be the port
            split
                .nth(2)
                .ok_or(PORT_NOT_FOUND.set_lockfile_error(true))?,
            // We moved the cursor, so the fourth element is the very next one
            // Which should be the auth string
            split
                .next()
                .ok_or(AUTH_NOT_FOUND.set_lockfile_error(true))?,
        ]
    };

    // Format the header without
    let mut needs_encoding = String::with_capacity(5 + auth.len());
    needs_encoding.push_str("riot:");
    needs_encoding.push_str(auth);

    let auth_header_len = needs_encoding.len().div_ceil(3) * 4;
    let mut auth_header_buffer: &mut [u8] = if auth_header_len > 36 { &mut vec![b'='; auth_header_len].into_boxed_slice() } else { &mut [b'='; 36] };

    // The auth header has to be base64 encoded, so that's happens here
    ENCODER.internal_encode(needs_encoding.as_bytes(), &mut auth_header_buffer);

    let auth_header = std::str::from_utf8(&auth_header_buffer[..auth_header_len])
        .expect("The buffer is always valid utf-8");

    let port: u16 = port.parse().map_err(|err: ParseIntError| {
        Error::new_string(ErrorKind::PortNotFound, err.to_string())
    })?;

    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);

    let mut formatted_auth = String::with_capacity(6 + auth_header_len);
    formatted_auth.push_str("Basic ");
    formatted_auth.push_str(&auth_header[..auth_header_len]);

    // Format the port and header so that they can be used as headers
    // For the LCU API
    Ok((addr, formatted_auth))
}

#[derive(Debug, Clone)]
/// Error retaining to getting the auth key and url for the LCU
pub struct Error {
    kind: ErrorKind,
    message: std::borrow::Cow<'static, str>,
    lock_file_error: bool,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl Error {
    const fn new(kind: ErrorKind, message: &'static str) -> Self {
        Self {
            kind,
            message: std::borrow::Cow::Borrowed(message),
            lock_file_error: false,
        }
    }

    const fn new_string(kind: ErrorKind, message: String) -> Self {
        Self {
            kind,
            message: std::borrow::Cow::Owned(message),
            lock_file_error: false,
        }
    }

    const fn set_lockfile_error(mut self, lock_fie_error: bool) -> Self {
        self.lock_file_error = lock_fie_error;
        self
    }

    #[must_use]
    pub const fn is_lockfile_error(&self) -> bool {
        self.lock_file_error
    }

    #[must_use]
    /// Returns true if it's an IO error, false otherwise
    pub const fn is_io_error(&self) -> bool {
        matches!(self.kind, ErrorKind::Io(_))
    }

    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind.clone()
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
/// What caused the error
pub enum ErrorKind {
    Io(std::io::ErrorKind),
    LockFileNotFound,
    AuthTokenNotFound,
    PortNotFound,
    NotRunning,
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Io(value.kind()),
            message: value.to_string().into(),
            lock_file_error: true,
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(_: std::str::Utf8Error) -> Self {
        const {
            Self::new(
                ErrorKind::Io(std::io::ErrorKind::InvalidData),
                "stream did not contain valid UTF-8",
            )
            .set_lockfile_error(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{get_running_client, CLIENT_PROCESS_NAME, GAME_PROCESS_NAME};
    use sysinfo::{ProcessRefreshKind, RefreshKind, System};

    #[ignore = "This is only needed for testing, and doesn't need to be run all the time"]
    #[test]
    fn test_process_info() {
        let (port, pass) =
            get_running_client(CLIENT_PROCESS_NAME, GAME_PROCESS_NAME, true).unwrap();
        println!("{port} {pass}");
    }

    #[ignore = "This is only needed for testing, and doesn't need to be run all the time"]
    #[test]
    fn test_process_args() {
        // No matter what, the path to the process is required
        let refresh_kind = ProcessRefreshKind::new()
            .with_cwd(sysinfo::UpdateKind::OnlyIfNotSet)
            .with_root(sysinfo::UpdateKind::OnlyIfNotSet)
            .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
            .with_cmd(sysinfo::UpdateKind::OnlyIfNotSet);

        // Get the current list of processes
        let system = System::new_with_specifics(
            // This creates a new instance of `system` every time, so this only
            //  needs to be updated if it's not set
            RefreshKind::new().with_processes(refresh_kind),
        );

        let process = system
            .processes()
            .values()
            .find(|process| process.name() == GAME_PROCESS_NAME)
            .unwrap();

        println!("{:?}", process.exe());
        println!("{:?}", process.root());
        println!("{:?}", process.cmd());
        println!("{:?}", process.cwd());
        println!("{:?}", process.environ());

        let parent = process.parent().unwrap();

        let process = system.process(parent).unwrap();

        println!("{process:?}");
    }
}
