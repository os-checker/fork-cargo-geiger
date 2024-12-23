use crate::args::Verbosity::{Normal, Quiet, Verbose};
use crate::format::print_config::OutputFormat;

use cargo::core::shell::ColorChoice;
use cargo::{CliResult, GlobalContext};
use pico_args::Arguments;
use std::path::PathBuf;

/// Constant `&str` containing help text
pub const HELP: &str =
    "Detects usage of unsafe Rust in a Rust crate and its dependencies.

USAGE:
    cargo geiger [OPTIONS]

OPTIONS:
    -p, --package <SPEC>          Package to be used as the root of the tree.
        --features <FEATURES>     Space-separated list of features to activate.
        --all-features            Activate all available features.
        --no-default-features     Do not activate the `default` feature.
        --target <TARGET>         Set the target triple.
        --all-targets             Return dependencies for all targets. By
                                  default only the host target is matched.
        --manifest-path <PATH>    Path to Cargo.toml.
    -i, --invert                  Invert the tree direction.
        --no-indent               Display the dependencies as a list (rather
                                  than a tree).
        --prefix-depth            Display the dependencies as a list (rather
                                  than a tree), but prefixed with the depth.
    -a, --all                     Don't truncate dependencies that have already
                                  been displayed.
    --format <FORMAT>             Format string used for printing dependencies
                                  [default: {p}].
    --output-format               Output format for the report: Ascii, GitHubMarkdown,
                                  Json, Utf8, Ratio [default: Utf8]
    --update-readme               Writes output to ./README.md. Looks for a Safety
                                  Report section, replaces if found, adds if not.
                                  Throws an error if no README.md exists.
        --readme-path <PATH>      Path of README.md file to be written to.
        --section-name <NAME>     The section name in the README.md to be written
                                  to.
    -v, --verbose                 Use verbose output (-vv very verbose/build.rs
                                  output).
    -q, --quiet                   No output printed to stdout other than the
                                  tree.
        --color <WHEN>            Coloring: auto, always, never.
        --frozen                  Require Cargo.lock and cache are up to date.
        --locked                  Require Cargo.lock is up to date.
        --offline                 Run without accessing the network.
    -Z \"<FLAG>...\"                Unstable (nightly-only) flags to Cargo.
        --include-tests           Count unsafe usage in tests.
        --build-dependencies      Also analyze build dependencies.
        --dev-dependencies        Also analyze dev dependencies.
        --all-dependencies        Analyze all dependencies, including build and
                                  dev.
        --forbid-only             Don't build or clean anything, only scan
                                  entry point .rs source files for.
                                  forbid(unsafe_code) flags. This is
                                  significantly faster than the default
                                  scanning mode. TODO: Add ability to combine
                                  this with a whitelist for use in CI.
    -h, --help                    Prints help information.
    -V, --version                 Prints version information.
";

#[derive(Default)]
pub struct Args {
    pub all: bool,
    pub color: Option<String>,
    pub deps_args: DepsArgs,
    pub features_args: FeaturesArgs,
    pub forbid_only: bool,
    pub format: String,
    pub frozen: bool,
    pub help: bool,
    pub include_tests: bool,
    pub invert: bool,
    pub locked: bool,
    pub manifest_path: Option<PathBuf>,
    pub no_indent: bool,
    pub offline: bool,
    pub output_format: OutputFormat,
    pub package: Option<String>,
    pub prefix_depth: bool,
    pub quiet: bool,
    pub readme_args: ReadmeArgs,
    pub target_args: TargetArgs,
    pub unstable_flags: Vec<String>,
    pub verbosity: Verbosity,
    pub version: bool,
}

impl Args {
    /// Construct `Args` struct from `pico_args::Arguments` loaded from command line arguments
    /// provided by the user
    /// ```
    /// # use cargo_geiger::args::Args;
    /// let pico_arguments = pico_args::Arguments::from_env();
    /// let args = Args::parse_args(pico_arguments);
    /// ```
    pub fn parse_args(
        mut raw_args: Arguments,
    ) -> Result<Args, Box<dyn std::error::Error>> {
        let mut args = Args {
            all: raw_args.contains(["-a", "--all"]),
            color: raw_args.opt_value_from_str("--color")?,
            deps_args: DepsArgs {
                all_deps: raw_args.contains("--all-dependencies"),
                build_deps: raw_args.contains("--build-dependencies"),
                dev_deps: raw_args.contains("--dev-dependencies"),
            },
            features_args: FeaturesArgs {
                all_features: raw_args.contains("--all-features"),
                features: parse_features(
                    raw_args.opt_value_from_str("--features")?,
                ),
                no_default_features: raw_args.contains("--no-default-features"),
            },
            forbid_only: raw_args.contains(["-f", "--forbid-only"]),
            format: raw_args
                .opt_value_from_str("--format")?
                .unwrap_or_else(|| "{p}".to_string()),
            frozen: raw_args.contains("--frozen"),
            help: raw_args.contains(["-h", "--help"]),
            include_tests: raw_args.contains("--include-tests"),
            invert: raw_args.contains(["-i", "--invert"]),
            locked: raw_args.contains("--locked"),
            manifest_path: raw_args.opt_value_from_str("--manifest-path")?,
            no_indent: raw_args.contains("--no-indent"),
            offline: raw_args.contains("--offline"),
            package: raw_args.opt_value_from_str(["-p", "--package"])?,
            prefix_depth: raw_args.contains("--prefix-depth"),
            quiet: raw_args.contains(["-q", "--quiet"]),
            readme_args: ReadmeArgs {
                readme_path: raw_args.opt_value_from_str("--readme-path")?,
                section_name: raw_args.opt_value_from_str("--section-name")?,
                update_readme: raw_args.contains("--update-readme"),
            },
            target_args: TargetArgs {
                all_targets: raw_args.contains("--all-targets"),
                target: raw_args.opt_value_from_str("--target")?,
            },
            unstable_flags: raw_args
                .opt_value_from_str("-Z")?
                .map(|s: String| s.split(' ').map(|s| s.to_owned()).collect())
                .unwrap_or_else(Vec::new),

            version: raw_args.contains(["-V", "--version"]),
            verbosity: match (
                raw_args.contains("-vv"),
                raw_args.contains(["-v", "--verbose"]),
            ) {
                (false, false) => Quiet,
                (false, true) => Normal,
                (true, _) => Verbose,
            },
            output_format: raw_args
                .opt_value_from_str("--output-format")?
                .unwrap_or(OutputFormat::Utf8),
        };

        if args.readme_args.update_readme
            && args.output_format != OutputFormat::GitHubMarkdown
        {
            eprintln!(
                "OutputFormat has been specified as {:?}, but the `--update-readme` flag has also been provided. \
                To ensure the report written to the README.md is correct, a reduced charset will be used.",
                args.output_format
            );
            args.output_format = OutputFormat::GitHubMarkdown
        }

        Ok(args)
    }

    /// Update `cargo::util::Config` with values from `Args` struct, and set the shell
    /// colour choice
    /// ```
    /// # use cargo::Config;
    /// # use cargo_geiger::args::Args;
    /// let args = Args::parse_args(
    ///     pico_args::Arguments::from_env()
    /// ).unwrap();
    /// let mut config = Config::default().unwrap();
    /// args.update_config(&mut config);
    /// ```
    pub fn update_config(&self, config: &mut GlobalContext) -> CliResult {
        let target_dir = None; // Doesn't add any value for cargo-geiger.
        let cargo_config_verbosity = match self.verbosity {
            Quiet => 0,
            Normal => 1,
            Verbose => 2,
        };

        config.configure(
            cargo_config_verbosity,
            self.quiet,
            self.color.as_deref(),
            self.frozen,
            self.locked,
            self.offline,
            &target_dir,
            &self.unstable_flags,
            &[], // Some cargo API change, TODO: Look closer at this later.
        )?;

        match config.shell().color_choice() {
            ColorChoice::Always => colored::control::set_override(true),
            ColorChoice::Never => colored::control::set_override(false),
            ColorChoice::CargoAuto => {}
        }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct DepsArgs {
    pub all_deps: bool,
    pub build_deps: bool,
    pub dev_deps: bool,
}

#[derive(Debug, Default)]
pub struct FeaturesArgs {
    pub all_features: bool,
    pub features: Vec<String>,
    pub no_default_features: bool,
}

#[derive(Debug, Default)]
pub struct TargetArgs {
    pub all_targets: bool,
    pub target: Option<String>,
}

#[derive(Debug, Default)]
pub struct ReadmeArgs {
    pub readme_path: Option<PathBuf>,
    pub section_name: Option<String>,
    pub update_readme: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

impl Default for Verbosity {
    fn default() -> Self {
        Verbose
    }
}

fn parse_features(raw_features: Option<String>) -> Vec<String> {
    raw_features
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .split(' ')
        .map(str::to_owned)
        .filter(|f| !f.is_empty())
        .collect::<Vec<String>>()
}

#[cfg(test)]
pub mod args_tests {
    use super::*;

    use cargo::core::shell::ColorChoice;
    use cargo::core::Verbosity as CargoCoreVerbosity;
    use rstest::*;
    use std::ffi::OsString;

    #[rstest(
        input_argument_vector,
        expected_all,
        expected_output_format,
        expected_verbosity,
        case(
            vec![],
            false,
            OutputFormat::Utf8,
            Quiet
        ),
        case(
            vec![OsString::from("--all")],
            true,
            OutputFormat::Utf8,
            Quiet,
        ),
        case(
            vec![OsString::from("--output-format"), OsString::from("Ascii")],
            false,
            OutputFormat::Ascii,
            Quiet
        ),
        case(
            vec![OsString::from("-v")],
            false,
            OutputFormat::Utf8,
            Normal
        ),
        case(
            vec![OsString::from("-vv")],
            false,
            OutputFormat::Utf8,
            Verbose
        ),
        case(
            vec![OsString::from("--update-readme")],
            false,
            OutputFormat::GitHubMarkdown,
            Quiet
        ),
        case(
            vec![OsString::from("--update-readme"), OsString::from("--output-format"), OsString::from("Ascii")],
            false,
            OutputFormat::GitHubMarkdown,
            Quiet
        )
    )]
    fn parse_args_test(
        input_argument_vector: Vec<OsString>,
        expected_all: bool,
        expected_output_format: OutputFormat,
        expected_verbosity: Verbosity,
    ) {
        let args_result =
            Args::parse_args(Arguments::from_vec(input_argument_vector));

        assert!(args_result.is_ok());

        let args = args_result.unwrap();
        assert_eq!(args.all, expected_all);
        assert_eq!(args.output_format, expected_output_format);
        assert_eq!(args.verbosity, expected_verbosity)
    }

    #[rstest(
        input_raw_features,
        expected_features,
        case(
            Some(String::from("test some features")),
            vec![
                String::from("test"),
                String::from("some"),
                String::from("features")
            ]
        ),
        case(
            Some(String::from("test")),
            vec![String::from("test")]
        ),
        case(
            Some(String::from("")),
            vec![]
        ),
        case(
            None,
            vec![]
        )
    )]
    fn parse_features_test(
        input_raw_features: Option<String>,
        expected_features: Vec<String>,
    ) {
        assert_eq!(parse_features(input_raw_features), expected_features);
    }

    #[rstest(
        input_quiet,
        input_verbosity,
        expected_extra_verbose,
        expected_shell_verbosity,
        case(true, Quiet, false, CargoCoreVerbosity::Quiet),
        case(false, Quiet, false, CargoCoreVerbosity::Normal),
        case(false, Normal, false, CargoCoreVerbosity::Verbose),
        case(false, Verbose, true, CargoCoreVerbosity::Verbose)
    )]
    fn update_config_test_verbosity(
        input_quiet: bool,
        input_verbosity: Verbosity,
        expected_extra_verbose: bool,
        expected_shell_verbosity: CargoCoreVerbosity,
    ) {
        let offline = rand::random();
        let args = Args {
            offline,
            quiet: input_quiet,
            verbosity: input_verbosity,
            ..Default::default()
        };
        let mut config = GlobalContext::default().unwrap();
        let update_config_result = args.update_config(&mut config);

        assert!(update_config_result.is_ok());
        assert_eq!(config.extra_verbose(), expected_extra_verbose);
        assert_eq!(config.shell().verbosity(), expected_shell_verbosity);
        assert_eq!(config.offline(), offline);
        assert!(config.target_dir().unwrap().is_none());
    }

    #[rstest(
        input_color,
        expected_shell_color_choice,
        case(Some(String::from("always")), ColorChoice::Always),
        case(Some(String::from("auto")), ColorChoice::CargoAuto),
        case(Some(String::from("never")), ColorChoice::Never),
        case(None, ColorChoice::CargoAuto)
    )]
    fn update_config_test_color_choice(
        input_color: Option<String>,
        expected_shell_color_choice: ColorChoice,
    ) {
        let offline = rand::random();
        let args = Args {
            color: input_color,
            offline,
            ..Default::default()
        };
        let mut config = GlobalContext::default().unwrap();
        let update_config_result = args.update_config(&mut config);

        assert!(update_config_result.is_ok());
        assert_eq!(config.shell().color_choice(), expected_shell_color_choice);
        assert_eq!(config.offline(), offline);
        assert!(config.target_dir().unwrap().is_none());
    }

    #[rstest(
        input_frozen,
        input_locked,
        expected_frozen,
        expected_lock_update_allowed,
        case(true, true, true, false),
        case(true, false, true, false),
        case(false, true, false, false),
        case(false, false, false, true)
    )]
    fn update_config_test_frozen_locked(
        input_frozen: bool,
        input_locked: bool,
        expected_frozen: bool,
        expected_lock_update_allowed: bool,
    ) {
        let offline = rand::random();
        let args = Args {
            frozen: input_frozen,
            locked: input_locked,
            offline,
            ..Default::default()
        };
        let mut config = GlobalContext::default().unwrap();
        let update_config_result = args.update_config(&mut config);

        assert!(update_config_result.is_ok());
        assert_eq!(config.frozen(), expected_frozen);
        assert_eq!(config.lock_update_allowed(), expected_lock_update_allowed);
        assert_eq!(config.offline(), offline);
        assert!(config.target_dir().unwrap().is_none());
    }
}
