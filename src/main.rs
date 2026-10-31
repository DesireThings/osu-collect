use osu_collect::app::run_app;
use osu_collect::auto_update::cleanup_stale_artifacts;
use osu_collect::cli;
use osu_collect::config::{LogLevel, LoggingConfig, load_config_or_default, migrate_legacy_dirs};
use osu_collect::realm_bridge::ffi::set_realm_debug_logging;
use osu_collect::utils;
#[cfg(windows)]
use osu_collect::windows_init;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(windows)]
    windows_init::enable_ansi_support();
    #[cfg(windows)]
    windows_init::widen_window_if_needed();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    let cmd = match cli::parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    migrate_legacy_dirs();

    let config = load_config_or_default();

    match cmd {
        Some(cli::CliCommand::UpdateCollections(args)) => {
            // For the CLI path, set up a simple stderr logger if verbose
            let logging_config = if args.verbose {
                LoggingConfig {
                    enabled: true,
                    level: LogLevel::Debug,
                    ..Default::default()
                }
            } else {
                LoggingConfig {
                    enabled: true,
                    level: LogLevel::Info,
                    ..Default::default()
                }
            };
            init_stderr_logging(&logging_config);
            if let Err(e) = cli::run_update_collections(args).await {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        Some(cli::CliCommand::LazerImport(args)) => {
            init_stderr_logging(&LoggingConfig {
                enabled: true,
                level: LogLevel::Error,
                ..Default::default()
            });
            if let Err(e) = cli::run_lazer_import(args, config.recent.directory.clone()).await {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            // TUI mode — original behavior
            let realm_debug = config.logging.enabled
                && matches!(config.logging.level, LogLevel::Debug | LogLevel::Trace);
            set_realm_debug_logging(realm_debug);
            let _logging_guard = utils::init_logging(&config.logging)?;
            cleanup_stale_artifacts();
            run_app(config, None).await?;
        }
    }

    Ok(())
}

fn init_stderr_logging(config: &LoggingConfig) {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{EnvFilter, fmt};

    let level = match config.level {
        LogLevel::Error => LevelFilter::ERROR,
        LogLevel::Warn => LevelFilter::WARN,
        LogLevel::Info => LevelFilter::INFO,
        LogLevel::Debug => LevelFilter::DEBUG,
        LogLevel::Trace => LevelFilter::TRACE,
    };

    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let _ = fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .try_init();
}
