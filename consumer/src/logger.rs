use {
    fern::Dispatch,
    chrono::Local,
    colored::Colorize,
};

pub fn setup_logger(include_timestamp: bool) -> Result<(), Box<dyn std::error::Error>> {

    let base_dispatch = Dispatch::new()
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout());

    if include_timestamp {
        base_dispatch
            .format(|out, message, record| {
                let level = format_log_level(record.level());
                out.finish(format_args!(
                    "[{}] {} {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S"),
                    level,
                    message,
                ))
            })
            .apply()?;
    } else {
        base_dispatch
            .format(|out, message, record| {
                let level = format_log_level(record.level());
                out.finish(format_args!(
                    "{} {}",
                    level,
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


