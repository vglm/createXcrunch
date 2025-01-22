use clap::Parser;
use createxcrunch::{
    cli::{Cli, Commands},
    gpu, Config, RewardVariant,
};
use std::env;

fn main() {
    let cli = Cli::parse();

    env::set_var(
        "RUST_LOG",
        env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
    );

    env_logger::init();

    match cli.command {
        Commands::Create2(args) => {
            let gpu_device_id = args.cli_args.gpu_device_id;
            let factory = args.cli_args.factory;
            let caller = args.cli_args.caller;
            let chain_id = args.cli_args.chain_id;
            let init_code_hash = args.init_code_hash;
            let reward = RewardVariant::LeadingAny {
                group: args.cli_args.group,
                leading: args.cli_args.leading,
                ones: args.cli_args.ones,
                ints: args.cli_args.ints,
            };
            let output = args.cli_args.output;

            match Config::new(
                gpu_device_id,
                args.cli_args.work_size,
                args.cli_args.result_buffer_size,
                args.cli_args.sleep_for,
                &factory,
                caller.as_deref(),
                chain_id,
                Some(&init_code_hash),
                reward,
                &output,
            ) {
                Ok(config) => match gpu(config) {
                    Ok(_) => (),
                    Err(e) => panic!("{}", e),
                },
                Err(e) => panic!("{}", e),
            };
        }
        Commands::Create3(args) => {
            let gpu_device_id = args.gpu_device_id;
            let factory = args.factory;
            let caller = args.caller;
            let chain_id = args.chain_id;
            let reward = RewardVariant::LeadingAny {
                group: args.group,
                leading: args.leading,
                ones: args.ones,
                ints: args.ints,
            };
            let output = args.output;

            match Config::new(
                gpu_device_id,
                args.work_size,
                args.result_buffer_size,
                args.sleep_for,
                &factory,
                caller.as_deref(),
                chain_id,
                None,
                reward,
                &output,
            ) {
                Ok(config) => match gpu(config) {
                    Ok(_) => (),
                    Err(e) => panic!("{}", e),
                },
                Err(e) => panic!("{}", e),
            };
        }
    }
}
