use clap::{command, ArgAction, Args, Parser, Subcommand};

#[derive(Parser)]
#[command(arg_required_else_help = true)]
#[clap(name = "createXcrunch", version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Args)]
pub struct CliArgs {
    #[arg(
        id = "factory",
        long,
        short,
        default_value = "0x9e3f8eae49e442a323ef2094f277bf62752e6995",
        long_help = "Set the factory address.",
        help_heading = "Crunching options"
    )]
    pub factory: String,

    #[arg(
        id = "work-size",
        long,
        short,
        default_value = "1000000000",
        long_help = "Work size at once in GPU.",
        help_heading = "Crunching options"
    )]
    pub work_size: usize,

    #[arg(
        id = "sleep-for",
        long,
        short,
        long_help = "Sleep every kernel run (override default behaviour)",
        help_heading = "Crunching options",
        default_value = "0.0"
    )]
    pub sleep_for: f64,

    #[arg(
        id = "gpu-device-id",
        long,
        short,
        default_value = "0",
        long_help = "Set the GPU device ID.",
        help_heading = "Crunching options"
    )]
    pub gpu_device_id: u8,

    #[arg(
        id = "caller",
        long,
        short,
        long_help = "Set the caller address in hex format for a permissioned deployment.",
        help_heading = "Crunching options"
    )]
    pub caller: Option<String>,

    #[arg(
        id = "chain-id",
        long = "crosschain",
        short = 'x',
        long_help = "Set whether or not to enable crosschain deployment protection.",
        help_heading = "Crunching options",
        visible_alias = "crp"
    )]
    pub chain_id: Option<u64>,

    #[arg(
        id = "zeros",
        short = 'z',
        group = "search-criteria",
        long_help = "Minimum number of leading zero bytes. Cannot be used in combination with --matching.\n\nExample: --leading 4.",
        help_heading = "Crunching options"
    )]
    pub zeros: Option<u8>,

    #[arg(
        id = "total",
        long = "total",
        short = 't',
        group = "search-criteria",
        long_help = "Total number of zero bytes. If used in conjunction with --leading, search criteria will be both thresholds. Pass --either to search for either threshold.\n\nExample: --total 32.",
        help_heading = "Crunching options"
    )]
    pub total: Option<u8>,

    #[arg(
        id = "either",
        long = "either",
        long_help = "Search for either threshold. Must be used with --leading and --total.",
        requires_all = &["zeros", "total"],
        action = ArgAction::SetTrue,
        help_heading = "Crunching options"
    )]
    pub either: bool,

    #[arg(
        id = "pattern",
        long = "matching",
        short = 'm',
        group = "search-criteria",
        long_help = "Matching pattern for the contract address. Cannot be used in combination with --leading.\n\nExample: --matching ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed.",
        help_heading = "Crunching options",
        conflicts_with_all = &["zeros", "total"]
    )]
    pub pattern: Option<Box<str>>,

    #[arg(long = "group", default_value = "24")]
    pub group: u64,

    #[arg(long = "leading", default_value = "7")]
    pub leading: u64,

    #[arg(long = "ones", default_value = "9")]
    pub ones: u64,

    #[arg(long = "ints", default_value = "4")]
    pub ints: u64,

    #[arg(
        id = "output",
        long,
        short,
        default_value = "output.txt",
        long_help = "Output file name.",
        help_heading = "Output options"
    )]
    pub output: String,

    #[arg(
        long = "result-buffer-size",
        long_help = "Set the result buffer size.",
        help_heading = "Crunching options",
        default_value = "20000"
    )]
    pub result_buffer_size: usize,
}

#[derive(Args)]
pub struct Create2Args {
    #[clap(flatten)]
    pub cli_args: CliArgs,

    #[arg(
        long = "code-hash",
        visible_alias = "ch",
        long_help = "Set the init code hash in hex format.",
        help_heading = "Crunching options",
        required = true,
        visible_alias = "ch"
    )]
    pub init_code_hash: String,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Mine for a CREATE3 deployment address.")]
    Create3(CliArgs),
    #[command(about = "Mine for a CREATE2 deployment address.")]
    Create2(Create2Args),
}
