use crate::score::{get_min_difficulty, score_fancy};
use alloy_primitives::{hex, Address, FixedBytes};
use itertools::chain;
use ocl::{Buffer, Context, Device, MemFlags, Platform, ProQue, Program, Queue};
use rand::{thread_rng, Rng};
use std::fs::OpenOptions;
use std::io::Write;
use std::{
    fmt::Write as _,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

pub mod cli;
mod score;

const PROXY_CHILD_CODEHASH: [u8; 32] = [
    33, 195, 93, 190, 27, 52, 74, 36, 136, 207, 51, 33, 214, 206, 84, 47, 142, 159, 48, 85, 68,
    255, 9, 228, 153, 58, 98, 49, 154, 73, 124, 31,
];

static KERNEL_SRC: &str = include_str!("./kernels/keccak256.cl");

pub enum CreateXVariant {
    Create2 { init_code_hash: [u8; 32] },
    Create3,
}

pub enum RewardVariant {
    LeadingAny {
        group: u64,
        leading: u64,
        ones: u64,
        ints: u64,
    },
    LeadingZeros {
        zeros_threshold: u8,
    },
    TotalZeros {
        zeros_threshold: u8,
    },
    LeadingAndTotalZeros {
        leading_zeros_threshold: u8,
        total_zeros_threshold: u8,
    },
    LeadingOrTotalZeros {
        leading_zeros_threshold: u8,
        total_zeros_threshold: u8,
    },
    Matching {
        pattern: Box<str>,
    },
}

#[derive(Clone, Copy)]
pub enum SaltVariant {
    CrosschainSender {
        chain_id: [u8; 32],
        calling_address: [u8; 20],
    },
    Crosschain {
        chain_id: [u8; 32],
    },
    Sender {
        calling_address: [u8; 20],
    },
    Random,
}

pub struct Config<'a> {
    pub gpu_device: u8,
    pub work_size: usize,
    pub result_buffer_size: usize,
    pub sleep_for: f64,
    pub factory_address: [u8; 20],
    pub salt_variant: SaltVariant,
    pub create_variant: CreateXVariant,
    pub reward: RewardVariant,
    pub output: &'a str,
}

impl<'a> Config<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gpu_device: u8,
        work_size: usize,
        result_buffer_size: usize,
        sleep_for: f64,
        factory_address_str: &str,
        calling_address_str: Option<&str>,
        chain_id: Option<u64>,
        init_code_hash: Option<&str>,
        reward: RewardVariant,
        output: &'a str,
    ) -> Result<Self, &'static str> {
        // convert main arguments from hex string to vector of bytes
        let factory_address_vec =
            hex::decode(factory_address_str).expect("could not decode factory address argument");
        let calling_address_vec = calling_address_str.map(|calling_address| {
            hex::decode(calling_address).expect("could not decode calling address argument")
        });
        let init_code_hash_vec = init_code_hash.map(|init_code_hash| {
            hex::decode(init_code_hash).expect("could not decode init code hash argument")
        });

        // convert from vector to fixed array
        let factory_address = TryInto::<[u8; 20]>::try_into(factory_address_vec)
            .expect("invalid length for factory address argument");
        let calling_address = calling_address_vec.map(|calling_address_vec| {
            TryInto::<[u8; 20]>::try_into(calling_address_vec)
                .expect("invalid length for calling address argument")
        });
        let init_code_hash = init_code_hash_vec.map(|init_code_hash_vec| {
            TryInto::<[u8; 32]>::try_into(init_code_hash_vec)
                .expect("invalid length for init code hash argument")
        });
        let chain_id = chain_id.map(|chain_id| {
            let mut arr = [0u8; 32];
            arr[24..].copy_from_slice(&chain_id.to_be_bytes());
            arr
        });

        let create_variant = match init_code_hash {
            Some(init_code_hash) => CreateXVariant::Create2 { init_code_hash },
            None => CreateXVariant::Create3 {},
        };

        match &reward {
            RewardVariant::LeadingZeros { zeros_threshold }
            | RewardVariant::TotalZeros { zeros_threshold } => {
                validate_zeros_threshold(zeros_threshold)?;
            }
            RewardVariant::LeadingOrTotalZeros {
                leading_zeros_threshold,
                total_zeros_threshold,
            }
            | RewardVariant::LeadingAndTotalZeros {
                leading_zeros_threshold,
                total_zeros_threshold,
            } => {
                validate_zeros_threshold(leading_zeros_threshold)?;
                validate_zeros_threshold(total_zeros_threshold)?;
            }
            RewardVariant::Matching { pattern } => {
                if pattern.len() != 40 {
                    return Err("matching pattern must be 40 characters long");
                }
                if !pattern.chars().all(|c| c == 'X' || c.is_ascii_hexdigit()) {
                    return Err("matching pattern must only contain 'X' or hex characters");
                }
            }
            RewardVariant::LeadingAny { .. } => {}
        }

        fn validate_zeros_threshold(threhsold: &u8) -> Result<(), &'static str> {
            if threhsold == &0u8 {
                return Err("threshold must be greater than 0");
            }
            if threhsold > &20u8 {
                return Err("threshold must be less than 20");
            }

            Ok(())
        }

        let salt_variant = match (chain_id, calling_address) {
            (Some(chain_id), Some(calling_address)) if calling_address != [0u8; 20] => {
                SaltVariant::CrosschainSender {
                    chain_id,
                    calling_address,
                }
            }
            (Some(chain_id), None) => SaltVariant::Crosschain { chain_id },
            (None, Some(calling_address)) if calling_address != [0u8; 20] => {
                SaltVariant::Sender { calling_address }
            }
            _ => SaltVariant::Random,
        };

        if factory_address_str.chars().any(|c| c.is_uppercase()) {
            let factory_address_str = match factory_address_str.strip_prefix("0x") {
                Some(_) => factory_address_str.to_string(),
                None => format!("0x{}", factory_address_str),
            };
            match Address::parse_checksummed(factory_address_str, None) {
                Ok(_) => {}
                Err(_) => {
                    return Err("factory address uses invalid checksum");
                }
            }
        }

        if calling_address.is_some() {
            let calling_address_str = calling_address_str.unwrap();
            if calling_address_str.chars().any(|c| c.is_uppercase()) {
                let calling_address_str = match calling_address_str.strip_prefix("0x") {
                    Some(_) => calling_address_str.to_string(),
                    None => format!("0x{}", calling_address_str),
                };
                match Address::parse_checksummed(calling_address_str, None) {
                    Ok(_) => {}
                    Err(_) => {
                        return Err("caller address uses invalid checksum");
                    }
                }
            };
        };

        Ok(Self {
            gpu_device,
            sleep_for,
            work_size,
            result_buffer_size,
            factory_address,
            salt_variant,
            create_variant,
            reward,
            output,
        })
    }
}

/// Adapted from https://github.com/0age/create2crunch
///
pub fn gpu(config: Config) -> ocl::Result<()> {
    // set up a platform to use
    let platform = Platform::new(ocl::core::default_platform()?);

    //make sure output directory exists

    if std::fs::metadata("output").is_err() {
        log::info!("Creating output directory");
        std::fs::create_dir("output")?;
    }
    // set up the device to use
    let device = Device::by_idx_wrap(platform, config.gpu_device as usize)?;

    println!(
        "Using device: {}",
        device.name().unwrap_or("Unknown device".to_string())
    );
    // set up the context to use
    let context = Context::builder()
        .platform(platform)
        .devices(device)
        .build()?;

    // set up the program to use
    let program = Program::builder()
        .devices(device)
        .src(mk_kernel_src(&config))
        .build(&context)?;

    // set up the queue to use
    let queue = Queue::new(&context, device, None)?;

    // set up the "proqueue" (or amalgamation of various elements) to use
    let ocl_pq = ProQue::new(context, queue, program, Some(config.work_size));

    // create a random number generator
    let mut rng = thread_rng();

    // the last work duration in milliseconds
    let mut work_duration_millis: u64 = 0;

    let mut number_found = 0;

    let mut total_processed = 0;

    // begin searching for addresses
    loop {
        // reset nonce & create a buffer to view it in little-endian
        // for more uniformly distributed nonces, we shall initialize it to a random value
        let mut nonce: [u32; 1] = rng.gen();

        // build a corresponding buffer for passing the nonce to the kernel
        let mut nonce_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(1)
            .copy_host_slice(&nonce)
            .build()?;

        // establish a buffer for nonces that result in desired addresses
        let mut solutions: Vec<u64> = vec![0; 4 * config.result_buffer_size];
        let solutions_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().write_only())
            .len(4 * config.result_buffer_size)
            .copy_host_slice(&solutions)
            .build()?;
        // construct the 4-byte message to hash, leaving last 8 of salt empty
        let mut salt;

        // repeatedly enqueue kernel to search for new addresses
        'middle: loop {
            salt = FixedBytes::<4>::random();

            // build a corresponding buffer for passing the message to the kernel
            let message_buffer = Buffer::builder()
                .queue(ocl_pq.queue().clone())
                .flags(MemFlags::new().read_only())
                .len(4)
                .copy_host_slice(&salt[..])
                .build()?;

            // build the kernel and define the type of each buffer
            let kern = ocl_pq
                .kernel_builder("hashMessage")
                .arg_named("message", None::<&Buffer<u8>>)
                .arg_named("nonce", None::<&Buffer<u32>>)
                .arg_named("solutions", None::<&Buffer<u64>>)
                .build()?;

            // set each buffer
            kern.set_arg("message", Some(&message_buffer))?;
            kern.set_arg("nonce", Some(&nonce_buffer))?;
            kern.set_arg("solutions", &solutions_buffer)?;

            // enqueue the kernel
            unsafe { kern.enq()? };

            // calculate the current time
            let mut now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

            // record the start time of the work
            let work_start_time_millis = now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000;

            //if work_duration_millis > 0  {
            total_processed += config.work_size as u64;
            println!(
                "Processed: {:.1}GH, message {}, nonce {}, last {} took {}ms. Avg {:.1}Mh/s",
                total_processed as f64 / 1.0E9,
                hex::encode(salt),
                nonce[0] as u16,
                config.work_size,
                work_duration_millis,
                config.work_size as f64 / work_duration_millis as f64 / 1000.0
            );
            //}

            thread::sleep(std::time::Duration::from_secs_f64(config.sleep_for));

            // read the solutions from the device
            solutions_buffer.read(&mut solutions).enq()?;

            // record the end time of the work and compute how long the work took
            now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            work_duration_millis = (now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1000000)
                - work_start_time_millis;

            // if at least one solution is found, end the loop

            for i in 0..config.result_buffer_size {
                // get the salt that results from the hash
                let solution = solutions[i * 4];
                if solution != 0 {
                    break 'middle;
                }
            }

            // if no solution has yet been found, increment the nonce
            nonce[0] += 1;

            // update the nonce buffer with the incremented nonce value
            nonce_buffer = Buffer::builder()
                .queue(ocl_pq.queue().clone())
                .flags(MemFlags::new().read_write())
                .len(1)
                .copy_host_slice(&nonce)
                .build()?;
        }
        let mut solution_count_rej = 0;
        let mut solution_count_acc = 0;

        let res_buffer_size = config.result_buffer_size;
        let salt_variant = config.salt_variant;
        //spawn new thread
        std::thread::spawn(move || {
            for i in 0..res_buffer_size {
                // get the salt that results from the hash
                let solution = solutions[i * 4];
                if solution == 0 {
                    continue;
                }
                let solution = solution.to_le_bytes();

                let mined_salt = chain!(salt, solution[..7].iter().copied());

                let salt: Vec<u8> = match salt_variant {
                    SaltVariant::CrosschainSender {
                        chain_id: _,
                        calling_address,
                    } => chain!(calling_address, [1u8], mined_salt).collect(),
                    SaltVariant::Crosschain { chain_id: _ } => {
                        chain!([0u8; 20], [1u8], mined_salt).collect()
                    }
                    SaltVariant::Sender { calling_address } => {
                        chain!(calling_address, [0u8], mined_salt).collect()
                    }
                    SaltVariant::Random => chain!(mined_salt, [0u8; 21]).collect(),
                };

                // get the address that results from the hash
                let address = solutions[i * 4 + 1]
                    .to_be_bytes()
                    .into_iter()
                    .chain(solutions[i * 4 + 2].to_be_bytes())
                    .chain(solutions[i * 4 + 3].to_be_bytes()[..4].to_vec())
                    .collect::<Vec<u8>>();

                let score = score_fancy(web3::types::Address::from_slice(address.as_slice()));

                if score.total_score < get_min_difficulty() {
                    solution_count_rej += 1;
                } else {
                    solution_count_acc += 1;
                    log::info!(
                        "Found accepted solution: address: {}, score: {}, category: {}",
                        score.address_mixed_case,
                        score.total_score,
                        score.category
                    );
                    //get cargo version
                    let version = env!("CARGO_PKG_VERSION");
                    let output = format!(
                        "0x{},0x{},0x{},{}_{}",
                        hex::encode(salt),
                        hex::encode(&address),
                        hex::encode(config.factory_address),
                        version,
                        total_processed / 1000000000
                    );

                    number_found += 1;
                    println!("{}", output);
                    let mut file = OpenOptions::new()
                        .write(true)
                        .append(true)
                        .create(true)
                        .open(format!("output/addr_{}.csv", hex::encode(&address)))
                        .unwrap();

                    // Write lines to the file
                    writeln!(file, "{}", output).unwrap();
                }
            }

            log::info!(
                "Found {} solutions, {} accepted, {} rejected",
                number_found,
                solution_count_acc,
                solution_count_rej
            );
        });
    }
}

/// Creates the OpenCL kernel source code by populating the template with the
/// values from the Config object.
pub fn mk_kernel_src(config: &Config) -> String {
    let mut src = String::with_capacity(2048 + KERNEL_SRC.len());

    let (caller, chain_id) = match config.salt_variant {
        SaltVariant::CrosschainSender {
            chain_id,
            calling_address,
        } => {
            writeln!(src, "#define GENERATE_SEED() SENDER_XCHAIN()").unwrap();
            (calling_address, Some(chain_id))
        }
        SaltVariant::Crosschain { chain_id } => {
            writeln!(src, "#define GENERATE_SEED() XCHAIN()").unwrap();
            ([0u8; 20], Some(chain_id))
        }
        SaltVariant::Sender { calling_address } => {
            writeln!(src, "#define GENERATE_SEED() SENDER()").unwrap();
            (calling_address, None)
        }
        SaltVariant::Random => {
            writeln!(src, "#define GENERATE_SEED() RANDOM()").unwrap();
            ([0u8; 20], None)
        }
    };
    writeln!(
        src,
        "#define RESULT_BUFFER_SIZE {}",
        config.result_buffer_size
    )
    .unwrap();

    match &config.reward {
        RewardVariant::LeadingAny {
            group,
            leading,
            ones,
            ints,
        } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define LEADING {leading}").unwrap();
            writeln!(src, "#define GROUP {group}").unwrap();
            writeln!(src, "#define ONES {ones}").unwrap();
            writeln!(src, "#define INTS {ints}").unwrap();

            writeln!(src, "#define SUCCESS_CONDITION() hasLeadingAny(digest)").unwrap();
        }
        RewardVariant::LeadingZeros { zeros_threshold } => {
            writeln!(src, "#define LEADING_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasLeading(digest)").unwrap();
        }
        RewardVariant::TotalZeros { zeros_threshold } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasTotal(digest)").unwrap();
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasLeading(digest) && hasTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasLeading(digest) || hasTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::Matching { pattern } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define PATTERN() \"{pattern}\"").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() isMatching(digest)").unwrap();
        }
    };

    let init_code_hash = match config.create_variant {
        CreateXVariant::Create2 { init_code_hash } => {
            writeln!(src, "#define CREATE3()").unwrap();
            init_code_hash
        }
        CreateXVariant::Create3 => {
            writeln!(src, "#define CREATE3() RUN_CREATE3()").unwrap();
            PROXY_CHILD_CODEHASH
        }
    };

    let caller = caller.iter();
    let chain_id = chain_id
        .iter()
        .flatten()
        .enumerate()
        .map(|(i, x)| (i + 20, x));
    caller.enumerate().chain(chain_id).for_each(|(i, x)| {
        writeln!(src, "#define S1_{} {}u", i + 12, x).unwrap();
    });

    let factory = config.factory_address.iter();
    let hash = init_code_hash.iter();
    let hash = hash.enumerate().map(|(i, x)| (i + 52, x));

    for (i, x) in factory.enumerate().chain(hash) {
        writeln!(src, "#define S2_{} {}u", i + 1, x).unwrap();
    }

    src.push_str(KERNEL_SRC);

    src
}
