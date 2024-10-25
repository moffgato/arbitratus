use {
    fern::Dispatch,
    chrono::Local,
    colored::Colorize,
    log::LevelFilter,
    std::env,
};

pub fn setup_logger(include_timestamp: bool) -> Result<(), Box<dyn std::error::Error>> {

    let log_level = match env::var("RUST_LOG") {
        Ok(level) => level.parse::<LevelFilter>().unwrap_or(LevelFilter::Info),
        Err(_) => LevelFilter::Info,
    };

    let base_dispatch = Dispatch::new()
        .level(log_level)
        .chain(std::io::stdout());

    if include_timestamp {
        base_dispatch
            .format(|out, message, record| {
                let level = format_log_level(record.level());
                let target = record.target();
                out.finish(format_args!(
                    "[{}] {} [{}] {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S"),
                    level,
                    target,
                    message,
                ))
            })
            .apply()?;
    } else {
        base_dispatch
            .format(|out, message, record| {
                let level = format_log_level(record.level());
                let target = record.target();
                out.finish(format_args!(
                    "{} [{}] {}",
                    level,
                    target,
                    message,
                ))
            })
            .apply()?;
    }

    Ok(())
}

fn format_log_level(level: log::Level) -> String {
    match level {
        log::Level::Error => "[ERROR]".red().bold().to_string(),
        log::Level::Warn => "[WARN]".yellow().bold().to_string(),
        log::Level::Info => "[INFO]".green().to_string(),
        log::Level::Debug => "[DEBUG]".blue().to_string(),
        log::Level::Trace => "[TRACE]".red().to_string(),
    }
}


