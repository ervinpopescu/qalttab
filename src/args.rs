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
    fn parses_with_no_args() {
        assert!(Args::try_parse_from(["qalttab"]).is_ok());
    }

    #[test]
    fn rejects_unknown_flag() {
        assert!(Args::try_parse_from(["qalttab", "--nonexistent"]).is_err());
    }

    #[test]
    fn rejects_unknown_positional() {
        assert!(Args::try_parse_from(["qalttab", "extra"]).is_err());
    }

    #[test]
    fn help_flag_exits_with_display_help() {
        let err = Args::try_parse_from(["qalttab", "--help"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn version_flag_exits_with_display_version() {
        let err = Args::try_parse_from(["qalttab", "--version"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
    }
}
