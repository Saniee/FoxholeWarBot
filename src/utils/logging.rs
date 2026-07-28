//! Where log records go: a quiet console, a readable file, and a verbose file.
//!
//! The console is what someone watching `docker compose logs` reads, so it
//! carries this crate's own messages and nothing else below a warning.
//! Everything the dependencies say — serenity's HTTP requests and gateway
//! traffic, poise's dispatch, every SQL statement sqlx runs at INFO — goes to
//! the verbose file, where it is available when something needs debugging and
//! invisible when it doesn't. Losing it entirely was never an option: it is the
//! only record of what the bot asked Discord for and what came back.
//!
//! Three sinks, two of them files:
//!
//! | Sink | Default filter | Overridden by |
//! |---|---|---|
//! | stderr | `warn` + this crate at `info` | `RUST_LOG` |
//! | `foxholewarbot.<date>.log` | the same | `LOG_FILE_FILTER` |
//! | `foxholewarbot-verbose.<date>.log` | `debug`, this crate at `trace` | `LOG_VERBOSE_FILTER` |
//!
//! The plain file is deliberately the console's twin rather than a third level:
//! its job is to answer "what did it say last Tuesday" for someone who was not
//! watching the terminal, and a stream that reads differently from the one they
//! know is a worse answer than the same one, kept.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use log::LevelFilter;

/// Written to when `LOG_DIR` is unset. Relative to the working directory, which
/// in the container is `/app` — see `compose.yaml`, which mounts a volume here.
const DEFAULT_DIR: &str = "./logs";

/// `<prefix><date>.log`, rotated daily by `fern`. The verbose file is the same
/// name with a suffix, so the pair sorts together in a directory listing and it
/// is obvious at a glance which is which.
const FILE_PREFIX: &str = "foxholewarbot.";
const VERBOSE_PREFIX: &str = "foxholewarbot-verbose.";
const DATE_SUFFIX: &str = "%Y-%m-%d.log";

/// Days of logs kept by [`prune`]. Long enough to cover "it broke over the
/// weekend and I looked on Monday", short enough that a volume nobody watches
/// doesn't grow forever.
const DEFAULT_RETENTION_DAYS: u64 = 14;

/// Where the log files ended up, for the daily prune job.
#[derive(Debug, Clone)]
pub struct LogFiles {
    pub dir: PathBuf,
    /// `0` means "keep everything", which is what someone shipping logs off the
    /// box with their own rotation wants.
    pub retention_days: u64,
}

/// Installs the logger. Returns where the files are, or `None` when only the
/// console is logging.
///
/// Never fails the process: a log directory that can't be created is a warning
/// on a console that is already working, not a reason to refuse to start a bot
/// whose actual job is elsewhere. Same rule the on-disk cache follows.
pub fn init() -> Option<LogFiles> {
    let root = crate_target();

    // `tracing::span=off` in every filter, including the verbose one. Serenity is
    // instrumented with `tracing`, whose `log` bridge emits a record for every
    // span it opens — `recv;`, `do_heartbeat;`, `recv_event;`, several a second,
    // forever, each carrying no information beyond its own name. They are noise
    // at every verbosity, and they arrive under their own target so dropping
    // them costs none of serenity's real messages.
    let console_spec = spec("RUST_LOG", &format!("warn,{root}=info,tracing::span=off"));
    let file_spec = spec("LOG_FILE_FILTER", &console_spec);
    // The two gateway targets are muted rather than merely lowered, and they are
    // the only ones that are. Measured from a real verbose log: 1,837 lines over
    // 2h05m, of which 1,695 came from these two — and 65 MB of file, an average
    // of 36 KB a line. That is ~750 MB a day, or 10 GB across the default
    // retention window. `serenity` is instrumented with `tracing`, and
    // the bridge records a span's *creation* with its fields inline, so
    // `handle_event; event=Ok(Dispatch(N, GuildCreate(GuildCreateEvent { .. })))`
    // is the whole guild — every channel, role and emoji — Debug-formatted onto
    // one line. Left in, the verbose file is most of a gigabyte a day and the
    // useful lines are needles in it.
    //
    // Muting is the blunt instrument on purpose: the noise here is INFO while
    // the little that's worth keeping under the same targets (`Received a
    // Hello`, `Sending presence update`) is DEBUG, so no *level* separates them.
    // What actually matters about the gateway — reconnects, resumes, failures —
    // is logged at WARN by these targets and by `shard_runner`, `shard_manager`
    // and `shard_queuer`, which are left alone.
    // `serenity::http` is muted for the same reason and by the same measurement.
    // With the gateway silenced the file was still 55.7 MB — from 711 lines, 80 KB
    // each — because `build; self=Request { body: Some([N, N, N, ..` is the request
    // body as a decimal list, one element per byte. A rendered map PNG posted to a
    // webhook is a few megabytes, so a single scheduled tick writes a single log
    // line of a few megabytes. What that line would have told us — method, route,
    // status — is not worth the file it arrives in, and a 429 or a failed request
    // is logged at WARN, which survives the mute.
    let verbose_spec = spec(
        "LOG_VERBOSE_FILTER",
        &format!(
            "debug,{root}=trace,tracing::span=off,\
             serenity::gateway::shard=warn,serenity::gateway::ws=warn,\
             serenity::http=warn"
        ),
    );

    let dir = log_dir();
    let files = dir.as_ref().map(|dir| LogFiles {
        dir: dir.clone(),
        retention_days: retention_days(),
    });

    let console = fern::Dispatch::new()
        .filter(filter(&console_spec))
        .format(format)
        .chain(std::io::stderr());

    let mut dispatch = fern::Dispatch::new()
        // The root has to admit everything the most verbose sink asks for: it
        // sets `log::max_level`, and a record rejected there never reaches a
        // per-sink filter at all.
        .level(max_level(&[&console_spec, &file_spec, &verbose_spec]))
        .chain(console);

    if let Some(dir) = &dir {
        dispatch = dispatch
            .chain(
                fern::Dispatch::new()
                    .filter(filter(&file_spec))
                    .format(format)
                    .chain(fern::DateBased::new(dir.join(FILE_PREFIX), DATE_SUFFIX)),
            )
            .chain(
                fern::Dispatch::new()
                    .filter(filter(&verbose_spec))
                    .format(format)
                    .chain(fern::DateBased::new(dir.join(VERBOSE_PREFIX), DATE_SUFFIX)),
            );
    }

    if let Err(err) = dispatch.apply() {
        // Only reachable if something already installed a logger, which nothing
        // does. `eprintln!` because the thing that would have carried this
        // message is what just failed.
        eprintln!("could not install the logger: {err}");
        return None;
    }

    match &files {
        Some(files) => log::info!(
            "logging to {} (and {VERBOSE_PREFIX}<date>.log for the verbose stream)",
            files.dir.display()
        ),
        None => log::info!("logging to the console only"),
    }

    files
}

/// Deletes rotated log files older than the retention window.
///
/// Rotation on its own only decides how the same unbounded pile is named. This
/// is what keeps a mounted volume from filling with a year of gateway traffic.
/// Returns how many files went.
pub fn prune(dir: &Path, retention_days: u64) -> std::io::Result<usize> {
    if retention_days == 0 {
        return Ok(0);
    }

    let cutoff = SystemTime::now() - Duration::from_secs(retention_days * 24 * 60 * 60);
    let mut removed = 0;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        // Only files this module writes. The directory may be a bind mount to
        // somewhere with other things in it, and deleting by age alone in a
        // directory we don't own is how a log pruner eats something else.
        let ours = name.starts_with(FILE_PREFIX) || name.starts_with(VERBOSE_PREFIX);

        if !ours || !name.ends_with(".log") {
            continue;
        }

        // Modification time, not the date in the name: the file being written
        // right now is young by both measures, and an mtime is one parse fewer
        // to get wrong.
        let modified = entry.metadata().and_then(|meta| meta.modified());

        match modified {
            Ok(modified) if modified < cutoff => match fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(err) => log::warn!("could not remove the old log {name}: {err}"),
            },
            Ok(_) => {}
            Err(err) => log::warn!("could not read the age of {name}: {err}"),
        }
    }

    Ok(removed)
}

/// [`prune`], with its outcome logged. What both callers — the startup pass and
/// the nightly job — actually want, kept in one place so they can't drift.
pub fn prune_and_report(logs: &LogFiles) {
    match prune(&logs.dir, logs.retention_days) {
        Ok(0) => {}
        Ok(removed) => log::info!("removed {removed} log file(s) past the retention window"),
        Err(err) => log::warn!("could not tidy {}: {err}", logs.dir.display()),
    }
}

/// The directory to write log files in, creating it if needed.
///
/// `LOG_DIR=""` turns file logging off — the one way to say "console only" that
/// doesn't need a second variable to mean it.
fn log_dir() -> Option<PathBuf> {
    let dir = dotenv::var("LOG_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());

    if dir.trim().is_empty() {
        return None;
    }

    let dir = PathBuf::from(dir.trim());

    if let Err(err) = fs::create_dir_all(&dir) {
        eprintln!(
            "could not create the log directory {}: {err} — logging to the console only",
            dir.display()
        );
        return None;
    }

    // Absolute from here on. `./logs` is only meaningful next to the working
    // directory it was read in, and "it says it's logging but the folder is
    // empty" is almost always two different folders — a container writing to
    // its own filesystem, or a process started from somewhere else.
    let dir = fs::canonicalize(&dir).unwrap_or(dir);

    // Prove the directory is writable now, rather than discovering it isn't
    // when the first record is dropped. `fern::DateBased` opens its file lazily
    // on the first write and has nowhere to report a failure to, so a
    // permissions problem on a mounted volume is otherwise perfectly silent:
    // the console keeps working and the directory stays empty.
    if let Err(err) = probe(&dir) {
        eprintln!(
            "the log directory {} is not writable: {err} — logging to the console only",
            dir.display()
        );
        return None;
    }

    Some(dir)
}

/// Creates and removes a file in `dir`, to find out whether the log files will
/// be writable before anything depends on them being written.
fn probe(dir: &Path) -> std::io::Result<()> {
    let path = dir.join(".write-probe");

    fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)?;

    // Best-effort: a probe file left behind is untidy, not broken, and it must
    // not turn a writable directory into an unwritable one.
    let _ = fs::remove_file(&path);

    Ok(())
}

fn retention_days() -> u64 {
    // Empty counts as unset: `compose.yaml` passes these through as
    // `${LOG_RETENTION_DAYS:-}`, so an unset host variable arrives as "".
    let Some(value) = dotenv::var("LOG_RETENTION_DAYS")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return DEFAULT_RETENTION_DAYS;
    };

    value.trim().parse().unwrap_or_else(|_| {
        eprintln!("LOG_RETENTION_DAYS is not a number, using {DEFAULT_RETENTION_DAYS}");
        DEFAULT_RETENTION_DAYS
    })
}

/// This crate's log target, so the filters don't have to name it in a string
/// literal that a rename would leave pointing at nothing.
fn crate_target() -> &'static str {
    module_path!().split("::").next().unwrap_or("FoxholeWarBot")
}

fn spec(var: &str, default: &str) -> String {
    dotenv::var(var)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn filter(spec: &str) -> impl Fn(&log::Metadata) -> bool + Send + Sync + 'static {
    let filter = env_filter::Builder::new().parse(spec).build();

    move |metadata| filter.enabled(metadata)
}

fn max_level(specs: &[&str]) -> LevelFilter {
    specs
        .iter()
        .map(|spec| env_filter::Builder::new().parse(spec).build().filter())
        .max()
        .unwrap_or(LevelFilter::Info)
}

fn format(out: fern::FormatCallback, message: &std::fmt::Arguments, record: &log::Record) {
    out.finish(format_args!(
        "{} {:<5} {} > {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        record.level(),
        record.target(),
        message
    ))
}
