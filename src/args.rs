use clap::Parser;

/// Qtile alttab window
#[derive(Parser, Debug, Clone, Default)]
#[command(version, about, long_about = None)]
pub struct Args {}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_args_parse() {
        // Since Args is empty, any or no arguments should work
        let args = Args::try_parse_from(["qalttab"]);
        assert!(args.is_ok());
    }

    #[test]
    fn test_args_help() {
        let args = Args::try_parse_from(["qalttab", "--help"]);
        assert!(args.is_err()); // help returns an error in try_parse_from
    }

    #[test]
    fn test_args_version() {
        let args = Args::try_parse_from(["qalttab", "--version"]);
        assert!(args.is_err());
    }
}
