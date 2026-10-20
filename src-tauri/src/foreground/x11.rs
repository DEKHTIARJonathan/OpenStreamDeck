use super::*;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask, Window,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::{DefaultStream, PollMode, RustConnection, Stream};
use x11rb::utils::RawFdContainer;

// A stalled X server must not block preference changes or process shutdown.
struct CancellableStream {
    inner: DefaultStream,
    stop: Arc<AtomicBool>,
}
impl Stream for CancellableStream {
    fn poll(&self, mode: PollMode) -> std::io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if self.stop.load(Ordering::Acquire) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "Application detection stopped",
                ));
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "X11 application detection timed out",
                ));
            }
            let mut descriptor = libc::pollfd {
                fd: self.inner.as_raw_fd(),
                events: (if mode.readable() { libc::POLLIN } else { 0 })
                    | (if mode.writable() { libc::POLLOUT } else { 0 }),
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut descriptor, 1, 20) };
            if result > 0 {
                return Ok(());
            }
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
    fn read(&self, bytes: &mut [u8], fds: &mut Vec<RawFdContainer>) -> std::io::Result<usize> {
        self.inner.read(bytes, fds)
    }
    fn write(&self, bytes: &[u8], fds: &mut Vec<RawFdContainer>) -> std::io::Result<usize> {
        self.inner.write(bytes, fds)
    }
}
type XConnection = RustConnection<CancellableStream>;

fn connection_wait(stop: &AtomicBool, deadline: Instant) -> std::io::Result<()> {
    if stop.load(Ordering::Acquire) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "Application detection stopped",
        ));
    }
    if Instant::now() >= deadline {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "X11 application connection timed out",
        ));
    }
    Ok(())
}

/// Connect without entering the blocking UnixStream::connect used by x11rb.
/// A stopped server with a full listen backlog must remain cancellable too.
fn connect_socket(
    path: &[u8],
    abstract_socket: bool,
    stop: &AtomicBool,
    deadline: Instant,
) -> std::io::Result<UnixStream> {
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as _;
    let offset = usize::from(abstract_socket);
    if path.is_empty() || path.contains(&0) || path.len() + 1 > address.sun_path.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid X11 socket path",
        ));
    }
    for (destination, source) in address.sun_path[offset..].iter_mut().zip(path) {
        *destination = *source as _;
    }
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + path.len() + 1) as libc::socklen_t;
    loop {
        connection_wait(stop, deadline)?;
        #[cfg(target_os = "linux")]
        let flags = libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
        #[cfg(not(target_os = "linux"))]
        let flags = libc::SOCK_STREAM;
        let fd = unsafe { libc::socket(libc::AF_UNIX, flags, 0) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // Own the descriptor before any operation that can fail.
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        #[cfg(not(target_os = "linux"))]
        {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
            stream.set_nonblocking(true)?;
        }
        let result =
            unsafe { libc::connect(fd, (&address as *const libc::sockaddr_un).cast(), length) };
        if result == 0 {
            return Ok(stream);
        }
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::WouldBlock {
            // Linux returns EAGAIN for a full Unix listen backlog without
            // starting a connection. Retry with a fresh descriptor.
            drop(stream);
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if error.raw_os_error() != Some(libc::EINPROGRESS)
            && error.kind() != std::io::ErrorKind::Interrupted
        {
            return Err(error);
        }
        loop {
            connection_wait(stop, deadline)?;
            let mut descriptor = libc::pollfd {
                fd,
                events: libc::POLLOUT,
                revents: 0,
            };
            let result = unsafe { libc::poll(&mut descriptor, 1, 20) };
            if result > 0 {
                if let Some(error) = stream.take_error()? {
                    return Err(error);
                }
                return Ok(stream);
            }
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
}

fn connect_local(
    path: &std::path::Path,
    stop: &AtomicBool,
) -> std::io::Result<(DefaultStream, (x11rb_protocol::xauth::Family, Vec<u8>))> {
    use std::os::unix::ffi::OsStrExt;
    let path = path.as_os_str().as_bytes();
    #[cfg(target_os = "linux")]
    if let Ok(stream) = connect_socket(path, true, stop, Instant::now() + Duration::from_secs(2)) {
        return DefaultStream::from_unix_stream(stream);
    }
    let stream = connect_socket(path, false, stop, Instant::now() + Duration::from_secs(2))?;
    DefaultStream::from_unix_stream(stream)
}

fn connect(stop: Arc<AtomicBool>) -> Result<(XConnection, usize), String> {
    use x11rb_protocol::parse_display::{parse_display, ConnectAddress};
    let display =
        parse_display(None).map_err(|e| format!("X11 application detection unavailable: {e}"))?;
    let mut error = "Automatic application pages require a local X11 display".to_owned();
    for address in display.connect_instruction() {
        let ConnectAddress::Socket(path) = address else {
            continue;
        };
        match connect_local(path.as_ref(), &stop) {
            Ok((stream, (family, address))) => {
                let (auth_name, auth_data) =
                    x11rb_protocol::xauth::get_auth(family, &address, display.display)
                        .map_err(|_| "Could not read X11 authentication metadata")?
                        .unwrap_or_default();
                let connection = XConnection::connect_to_stream_with_auth_info(
                    CancellableStream {
                        inner: stream,
                        stop,
                    },
                    display.screen as usize,
                    auth_name,
                    auth_data,
                )
                .map_err(|e| format!("X11 application detection unavailable: {e}"))?;
                return Ok((connection, display.screen as usize));
            }
            Err(reason) => {
                error = format!("X11 application detection unavailable: {reason}");
            }
        }
    }
    Err(error)
}

fn atom(connection: &XConnection, name: &[u8]) -> Result<Atom, String> {
    connection
        .intern_atom(false, name)
        .map_err(|e| e.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|e| e.to_string())
}
fn query(connection: &XConnection, root: Window, active: Atom, class: Atom) -> Observation {
    let reply = connection
        .get_property(false, root, active, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let Some(window) = reply
        .value32()
        .and_then(|mut values| values.next())
        .filter(|value| *value != 0)
    else {
        return Ok(None);
    };
    let reply = connection
        .get_property(false, window, class, AtomEnum::STRING, 0, 1024)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    let value = x11_identity(&reply.value)?;
    // A window activation can race the WM_CLASS reply. Never publish metadata
    // for a handle that is already obsolete; its queued event triggers a retry.
    let latest = connection
        .get_property(false, root, active, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    if latest.value32().and_then(|mut values| values.next()) != Some(window) {
        return Ok(None);
    }
    Ok(Some(ForegroundApplication {
        name: value.trim_start_matches("wmclass:").into(),
        identity: ApplicationIdentity {
            platform: "linux".into(),
            value,
        },
    }))
}
fn session(stop: &Arc<AtomicBool>, callback: &Callback) -> Result<(), String> {
    let (connection, screen) = connect(stop.clone())?;
    let root = connection.setup().roots[screen].root;
    let active = atom(&connection, b"_NET_ACTIVE_WINDOW")?;
    let class = atom(&connection, b"WM_CLASS")?;
    let supported = atom(&connection, b"_NET_SUPPORTED")?;
    let support = connection
        .get_property(false, root, supported, AtomEnum::ATOM, 0, 4096)
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?;
    if !support
        .value32()
        .is_some_and(|mut values| values.any(|value| value == active))
    {
        return Err(
            "This X11 window manager does not expose the active application (_NET_ACTIVE_WINDOW)"
                .into(),
        );
    }
    connection
        .change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    connection.flush().map_err(|e| e.to_string())?;
    let mut dirty = true;
    let origin = Instant::now();
    let mut schedule = QuerySchedule::default();
    while !stop.load(Ordering::Acquire) {
        while let Some(event) = connection
            .poll_for_event()
            .map_err(|e| format!("X11 connection lost: {e}"))?
        {
            if matches!(event, Event::PropertyNotify(event) if event.window == root && event.atom == active)
            {
                dirty = true;
            }
        }
        if schedule.due(dirty, origin.elapsed().as_millis() as u64) {
            dirty = false;
            let result = query(&connection, root, active, class);
            schedule.observed(&result, origin.elapsed().as_millis() as u64);
            callback(result);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
pub(super) fn observe(stop: Arc<AtomicBool>, callback: Callback) {
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .is_ok_and(|value| value.eq_ignore_ascii_case("wayland"))
    {
        callback(Err("Automatic application pages are unavailable on Wayland. Use manual pages or an X11 session; XWayland cannot observe all foreground applications.".into()));
        return;
    }
    while !stop.load(Ordering::Acquire) {
        if let Err(error) = session(&stop, &callback) {
            callback(Err(error));
        }
        for _ in 0..100 {
            if stop.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    fn socket_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
        directory.path().join("x11.sock")
    }

    #[cfg(target_os = "linux")]
    fn saturate_backlog(
        path: &std::path::Path,
    ) -> (std::os::unix::net::UnixListener, Vec<UnixStream>) {
        let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
        assert_eq!(unsafe { libc::listen(listener.as_raw_fd(), 1) }, 0);
        let mut clients = vec![];
        for _ in 0..64 {
            match connect_socket(
                path.as_os_str().as_bytes(),
                false,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_millis(80),
            ) {
                Ok(stream) => clients.push(stream),
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                    return (listener, clients);
                }
                Err(error) => panic!("Could not fill fixture socket backlog: {error}"),
            }
        }
        panic!("Fixture socket backlog did not fill");
    }

    #[test]
    fn x11_socket_connection_handles_success_and_missing_server() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let path = socket_path(&directory);
        assert!(connect_local(&path, &AtomicBool::new(false)).is_err());
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let (stream, _) = connect_local(&path, &AtomicBool::new(false)).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        use std::io::Read;
        assert_eq!(stream.write(b"ready", &mut vec![]).unwrap(), 5);
        let mut bytes = [0; 5];
        server.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ready");
    }

    #[test]
    fn x11_socket_connection_checks_cancellation_and_deadline_before_connecting() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let path = socket_path(&directory);
        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let error = connect_socket(
            path.as_os_str().as_bytes(),
            false,
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionAborted);
        let error = connect_socket(
            path.as_os_str().as_bytes(),
            false,
            &AtomicBool::new(false),
            Instant::now(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn full_x11_socket_backlog_times_out_and_cancels_without_a_display_server() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let path = socket_path(&directory);
        let (_listener, _clients) = saturate_backlog(&path);
        let error = connect_socket(
            path.as_os_str().as_bytes(),
            false,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_millis(80),
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (started, wait_started) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            connect_socket(
                path.as_os_str().as_bytes(),
                false,
                &worker_stop,
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap_err()
            .kind()
        });
        wait_started.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(40));
        let cancellation_started = Instant::now();
        stop.store(true, Ordering::Release);
        assert_eq!(
            worker.join().unwrap(),
            std::io::ErrorKind::ConnectionAborted
        );
        assert!(cancellation_started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn x11_socket_connection_preserves_abstract_address_precedence() {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::{SocketAddr, UnixListener};
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let path = socket_path(&directory);
        let address = SocketAddr::from_abstract_name(path.as_os_str().as_bytes()).unwrap();
        let listener = UnixListener::bind_addr(&address).unwrap();
        let (_stream, _) = connect_local(&path, &AtomicBool::new(false)).unwrap();
        assert!(listener.accept().is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn cancelled_x11_transport_stops_before_waiting_for_a_reply() {
        let (socket, _server) = std::os::unix::net::UnixStream::pair().unwrap();
        let (inner, _) = DefaultStream::from_unix_stream(socket).unwrap();
        let stream = CancellableStream {
            inner,
            stop: Arc::new(AtomicBool::new(true)),
        };
        assert_eq!(
            stream.poll(PollMode::Readable).unwrap_err().kind(),
            std::io::ErrorKind::ConnectionAborted
        );
    }
    #[test]
    fn x11_transport_reads_real_socket_data_without_a_display_server() {
        use std::io::Write;
        let (socket, mut server) = std::os::unix::net::UnixStream::pair().unwrap();
        let (inner, _) = DefaultStream::from_unix_stream(socket).unwrap();
        let stream = CancellableStream {
            inner,
            stop: Arc::new(AtomicBool::new(false)),
        };
        server.write_all(b"fixture").unwrap();
        stream.poll(PollMode::Readable).unwrap();
        let mut data = [0u8; 7];
        assert_eq!(stream.read(&mut data, &mut vec![]).unwrap(), 7);
        assert_eq!(&data, b"fixture");
    }
}
