use std::collections::BTreeMap;
use std::env;
use web3::signing::keccak256;

use std::fmt::Display;
use std::str::FromStr;

use web3::types::{Address, H160};

fn address_to_mixed_case(address: &H160) -> String {
    let address_str = format!("{:x}", address);
    let hash = keccak256(address_str.as_bytes());
    let mut result = "0x".to_string();

    for (i, char) in address_str.chars().enumerate() {
        if char.is_ascii_hexdigit() {
            let hash_byte = hash[i / 2];
            let is_uppercase = if i % 2 == 0 {
                hash_byte >> 4 > 7
            } else {
                (hash_byte & 0x0f) > 7
            };
            if is_uppercase {
                result.push(char.to_ascii_uppercase());
            } else {
                result.push(char);
            }
        } else {
            result.push(char);
        }
    }

    result
}

#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub enum FancyScoreCategory {
    LeadingZeroes,
    LeadingAny,
    LettersCount,
    NumbersOnly,
    ShortLeadingZeroes,
    ShortLeadingAny,
    SnakeScore,
    LeadingLetters,
    #[default]
    Random,
}

impl Display for FancyScoreCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FancyScoreCategory::LeadingZeroes => write!(f, "leading_zeroes"),
            FancyScoreCategory::LeadingAny => write!(f, "leading_any"),
            FancyScoreCategory::LettersCount => write!(f, "letters_count"),
            FancyScoreCategory::NumbersOnly => write!(f, "numbers_only"),
            FancyScoreCategory::ShortLeadingZeroes => write!(f, "short_leading_zeroes"),
            FancyScoreCategory::ShortLeadingAny => write!(f, "short_leading_any"),
            FancyScoreCategory::SnakeScore => write!(f, "snake_score"),
            FancyScoreCategory::LeadingLetters => write!(f, "leading_letters"),
            FancyScoreCategory::Random => write!(f, "random"),
        }
    }
}

impl FromStr for FancyScoreCategory {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "leading_zeroes" => Ok(FancyScoreCategory::LeadingZeroes),
            "leading_any" => Ok(FancyScoreCategory::LeadingAny),
            "letters_count" => Ok(FancyScoreCategory::LettersCount),
            "numbers_only" => Ok(FancyScoreCategory::NumbersOnly),
            "short_leading_zeroes" => Ok(FancyScoreCategory::ShortLeadingZeroes),
            "short_leading_any" => Ok(FancyScoreCategory::ShortLeadingAny),
            "snake_score" => Ok(FancyScoreCategory::SnakeScore),
            "leading_letters" => Ok(FancyScoreCategory::LeadingLetters),
            "random" => Ok(FancyScoreCategory::Random),
            _ => Err(()),
        }
    }
}

pub fn total_combinations(n: f64) -> f64 {
    16.0f64.powf(n)
}

// n choose k symbol combinations
pub fn combinations(n: f64, k: f64) -> f64 {
    let mut result = 1.0;
    for i in 0..k as i64 {
        result *= (n - i as f64) / (i as f64 + 1.0);
    }
    result
}

//one number is accepted
pub fn exactly_letters_combinations(letters: f64, total: f64) -> f64 {
    if letters == total {
        return 6.0f64.powf(letters);
    }
    6.0f64.powf(letters) * combinations(total, total - letters) * 10f64
}

pub fn exactly_letters_combinations_difficulty(letters: f64, total: f64) -> f64 {
    if letters < 30.0 {
        return 1.0f64;
    }
    total_combinations(total) / exactly_letters_combinations(letters, total)
}

#[test]
fn tx_test() {
    assert_eq!(combinations(40.0, 1.0), 40.0);
    assert_eq!(combinations(40.0, 2.0), 780.0);
    //all letters probability

    let all_combinations = 16.0f64.powf(40.0);
    assert_eq!(all_combinations, 1.461501637330903e48);

    let only_letters_combinations = 6.0f64.powf(40.0);
    assert_eq!(only_letters_combinations, 1.3367494538843734e31);

    let one_number_combinations = 6.0f64.powf(39.0) * combinations(40.0, 1.0) * 10f64.powf(1.0);
    assert_eq!(one_number_combinations, 8.911663025895824e32);

    assert_eq!(
        exactly_letters_combinations(39.0, 40.0),
        8.911663025895824e32
    );
    assert_eq!(
        exactly_letters_combinations(38.0, 40.0),
        2.896290483416142e33
    );

    assert_eq!((6.0f64 / 16.0).powf(40.0), 9.14641092243755e-18);
    //39 letters probability
}

#[derive(Debug, Clone, Default)]
pub struct FancyScore {
    pub address_mixed_case: String,
    pub address_lower_case: String,
    pub address_short_etherscan: String,
    pub scores: BTreeMap<String, FancyScoreEntry>,
    pub total_score: f64,
    pub price_multiplier: f64,
    pub category: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct FancyScoreEntry {
    pub category: FancyScoreCategory,
    pub score: f64,
    pub difficulty: f64,
}

#[allow(dead_code)]
fn get_env_int(key: &str, default: i64) -> i64 {
    env::var(key)
        .map(|s| i64::from_str(&s).unwrap())
        .unwrap_or(default)
}

fn get_env_float(key: &str, default: f64) -> f64 {
    env::var(key)
        .map(|s| f64::from_str(&s).unwrap())
        .unwrap_or(default)
}
pub fn get_base_difficulty() -> f64 {
    get_env_float("BASE_DIFFICULTY", 16.0f64.powf(9f64))
}
pub fn get_min_difficulty() -> f64 {
    get_env_float("MIN_DIFFICULTY", 16.0f64.powf(8f64))
}

#[allow(clippy::vec_init_then_push)]
pub fn score_fancy(address: Address) -> FancyScore {
    let mut score = FancyScore::default();

    score.address_lower_case = format!("{:#x}", address).to_lowercase();
    score.address_mixed_case = address_to_mixed_case(&address);
    score.address_short_etherscan =
        score.address_mixed_case[0..10].to_string() + "..." + &score.address_mixed_case[33..42];

    let mixed_address_str = score.address_mixed_case.trim_start_matches("0x");
    let address_str = format!("{:#x}", address);
    let address_str = address_str.trim_start_matches("0x");
    let short_address_str = score
        .address_short_etherscan
        .trim_start_matches("0x")
        .replace("...", "");
    let mut leading_zeroes = 0;
    for c in address_str.chars() {
        if c == '0' {
            leading_zeroes += 1;
        } else {
            break;
        }
    }

    let char_start = address_str.chars().next().unwrap();
    let mut leading_any = 0;
    for c in address_str.chars() {
        if c == char_start {
            leading_any += 1;
        } else {
            break;
        }
    }

    let mut leading_letters = 0;
    let mixed_char_start = mixed_address_str.chars().next().unwrap();
    if mixed_char_start.is_alphabetic() {
        for c in mixed_address_str.chars() {
            if c == mixed_char_start {
                leading_letters += 1;
            } else {
                break;
            }
        }
    }

    let mut allowed_cipher = 'a';
    let mut letters_only = 0;
    for c in address_str.chars() {
        if c.is_alphabetic() {
            letters_only += 1;
        } else if allowed_cipher == 'a' {
            allowed_cipher = c;
        } else {
            //cipher have to be the same
            if c != allowed_cipher {
                letters_only = 0;
                break;
            }
        }
    }

    let mut numbers_only = 0;
    for c in address_str.chars() {
        if c.is_numeric() {
            numbers_only += 1;
        }
    }

    let mut short_leading_zeroes = 0;
    for c in short_address_str.chars() {
        if c == '0' {
            short_leading_zeroes += 1;
        } else {
            break;
        }
    }

    let mut short_leading_any = 0;
    let char_start = short_address_str.chars().next().unwrap();
    for c in short_address_str.chars() {
        if c == char_start {
            short_leading_any += 1;
        } else {
            break;
        }
    }

    let mut snake_score = 0.0f64;
    let mut prev_char = address_str.chars().next().unwrap();
    for c in address_str.chars() {
        if c == prev_char {
            snake_score += 1.0;
        } else {
            prev_char = c;
        }
    }

    let mut score_entries = Vec::new();

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::Random,
        score: 1.0f64,
        difficulty: 1000.0f64,
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::LeadingZeroes,
        score: leading_zeroes as f64,
        difficulty: 16.0f64.powf(leading_zeroes as f64),
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::LeadingAny,
        score: leading_any as f64 - 1.0_f64,
        difficulty: 16.0f64.powf(leading_any as f64 - (15. / 16.)),
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::LettersCount,
        score: letters_only as f64,
        difficulty: exactly_letters_combinations_difficulty(letters_only as f64, 40.0),
    });

    if numbers_only == 40 {
        let number = address_str.parse::<f64>().unwrap();
        let max_number = 9999999999999999999999999999999999999999f64;
        let difficulty1 =
            total_combinations(40.0) / 10.0f64.powf(numbers_only as f64) / (number / max_number);
        let difficulty2 = total_combinations(40.0)
            / 10.0f64.powf(numbers_only as f64)
            / ((max_number - number) / max_number);
        score_entries.push(FancyScoreEntry {
            category: FancyScoreCategory::NumbersOnly,
            score: numbers_only as f64,
            difficulty: difficulty1.max(difficulty2),
        });
    } else {
        score_entries.push(FancyScoreEntry {
            category: FancyScoreCategory::NumbersOnly,
            score: numbers_only as f64,
            difficulty: 1.0f64,
        });
    }

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::ShortLeadingZeroes,
        score: short_leading_zeroes as f64,
        difficulty: 16.0f64.powf(short_leading_zeroes as f64),
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::ShortLeadingAny,
        score: short_leading_any as f64,
        difficulty: 16.0f64.powf(short_leading_any as f64 - (15. / 16.)),
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::SnakeScore,
        score: snake_score,
        difficulty: 16.0f64.powf(snake_score - 9.0),
    });

    score_entries.push(FancyScoreEntry {
        category: FancyScoreCategory::LeadingLetters,
        score: leading_letters as f64,
        difficulty: 32.0f64.powf(leading_letters as f64 - (15. / 16.)),
    });

    score.scores = score_entries
        .iter()
        .map(|entry| (entry.category.to_string(), entry.clone()))
        .collect();

    let neutral_price_point = get_base_difficulty();

    // This simple method is better than iterator, because of float NaN issues
    let mut biggest_score = score_entries[0].clone();
    for entry in score_entries.iter() {
        if entry.difficulty > biggest_score.difficulty {
            biggest_score = entry.clone();
        }
    }

    let biggest_score_difficulty = biggest_score.difficulty;

    let price_multiplier = if biggest_score_difficulty <= neutral_price_point {
        1.0
    } else {
        biggest_score_difficulty / neutral_price_point
    };

    score.total_score = biggest_score_difficulty;
    score.price_multiplier = price_multiplier;
    score.category = biggest_score.category.to_string();
    score
}
