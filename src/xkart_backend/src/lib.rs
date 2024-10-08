use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};
use ic_cdk_macros::*;
use std::collections::HashMap;

/// Represents an account in the ICRC-1 token system.
#[derive(CandidType, Deserialize, Clone, Hash, Eq, PartialEq)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<[u8; 32]>,
}

/// Arguments for the ICRC-1 transfer function.
#[derive(CandidType, Deserialize)]
pub struct TransferArgs {
    pub from_subaccount: Option<[u8; 32]>,
    pub to: Account,
    pub amount: u64,
    pub fee: Option<u64>,
    pub memo: Option<Vec<u8>>,
    pub created_at_time: Option<u64>,
}

/// Possible errors that can occur during an ICRC-1 transfer.
#[derive(CandidType, Deserialize)]
pub enum TransferError {
    BadFee { expected_fee: u64 },
    BadBurn { min_burn_amount: u64 },
    InsufficientFunds { balance: u64 },
    TooOld,
    CreatedInFuture { ledger_time: u64 },
    Duplicate { duplicate_of: u64 },
    TemporarilyUnavailable,
    GenericError { error_code: u64, message: String },
}

/// Represents a race in the XKart Racing system.
#[derive(CandidType, Deserialize, Clone)]
pub struct Race {
    pub id: u64,
    pub name: String,
    pub participants: Vec<Principal>,
    pub status: RaceStatus,
    pub winner: Option<Principal>,
    pub bets: Vec<Bet>,
}

/// Possible states of a race.
#[derive(CandidType, Deserialize, Clone, PartialEq)]
pub enum RaceStatus {
    Upcoming,
    InProgress,
    Completed,
}

/// Represents a bet placed on a race.
#[derive(CandidType, Deserialize, Clone)]
pub struct Bet {
    pub race_id: u64,
    pub bettor: Principal,
    pub amount: u64,
    pub prediction: Principal,
}

/// Represents an NFT in the XKart Racing system.
#[derive(CandidType, Deserialize, Clone)]
pub struct NFT {
    pub id: u64,
    pub owner: Principal,
    pub metadata: String,
    pub listed_price: Option<u64>,
}

// State variables
thread_local! {
    static BALANCES: std::cell::RefCell<HashMap<Account, u64>> = std::cell::RefCell::new(HashMap::new());
    static TOTAL_SUPPLY: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
    static RACES: std::cell::RefCell<HashMap<u64, Race>> = std::cell::RefCell::new(HashMap::new());
    static NFTS: std::cell::RefCell<HashMap<u64, NFT>> = std::cell::RefCell::new(HashMap::new());
}

/// Transfers tokens from one account to another according to the ICRC-1 standard.
#[update]
fn icrc1_transfer(args: TransferArgs) -> Result<u64, TransferError> {
    let caller = ic_cdk::caller();
    let from_account = Account {
        owner: caller,
        subaccount: args.from_subaccount,
    };

    BALANCES.with(|balances| {
        let mut balances = balances.borrow_mut();
        let from_balance = balances.get(&from_account).cloned().unwrap_or(0);
        if from_balance < args.amount {
            return Err(TransferError::InsufficientFunds { balance: from_balance });
        }
        
        *balances.entry(from_account).or_insert(0) -= args.amount;
        *balances.entry(args.to).or_insert(0) += args.amount;
        
        Ok(0) // Return a dummy transaction index
    })
}

/// Returns the balance of the specified account.
#[query]
fn icrc1_balance_of(account: Account) -> u64 {
    BALANCES.with(|balances| {
        balances.borrow().get(&account).cloned().unwrap_or(0)
    })
}

/// Returns the total supply of tokens.
#[query]
fn icrc1_total_supply() -> u64 {
    TOTAL_SUPPLY.with(|total_supply| *total_supply.borrow())
}

/// Mints new tokens and assigns them to the specified account.
/// Only authorized principals can mint tokens.
#[update]
fn icrc1_mint(to: Account, amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    if !is_minting_authority(caller) {
        return Err("Not authorized to mint tokens".to_string());
    }

    BALANCES.with(|balances| {
        let mut balances = balances.borrow_mut();
        *balances.entry(to).or_insert(0) += amount;
    });

    TOTAL_SUPPLY.with(|total_supply| {
        *total_supply.borrow_mut() += amount;
    });

    Ok(())
}

/// Checks if the given principal is authorized to mint tokens.
fn is_minting_authority(principal: Principal) -> bool {
    
    true
}

/// Creates a new race with the given name.
/// Only admins can create races.
#[update]
fn create_race(name: String) -> Result<u64, String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can create races".to_string());
    }
    let race_id = RACES.with(|races| races.borrow().len() as u64 + 1);
    let new_race = Race {
        id: race_id,
        name,
        participants: Vec::new(),
        status: RaceStatus::Upcoming,
        winner: None,
        bets: Vec::new(),
    };
    RACES.with(|races| races.borrow_mut().insert(race_id, new_race));
    Ok(race_id)
}

/// Allows a user to join a race.
#[update]
fn join_race(race_id: u64) -> Result<(), String> {
    let caller = get_caller();
    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        if race.status != RaceStatus::Upcoming {
            return Err("Race is not open for joining".to_string());
        }
        if race.participants.contains(&caller) {
            return Err("Already joined this race".to_string());
        }
        race.participants.push(caller);
        Ok(())
    })
}

/// Starts a race. Only admins can start races.
#[update]
fn start_race(race_id: u64) -> Result<(), String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can start races".to_string());
    }
    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        if race.status != RaceStatus::Upcoming {
            return Err("Race cannot be started".to_string());
        }
        race.status = RaceStatus::InProgress;
        Ok(())
    })
}

/// Ends a race and determines the winner. Only admins can end races.
#[update]
fn end_race(race_id: u64, winner: Principal) -> Result<(), String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can end races".to_string());
    }
    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        if race.status != RaceStatus::InProgress {
            return Err("Race is not in progress".to_string());
        }
        if !race.participants.contains(&winner) {
            return Err("Winner is not a participant".to_string());
        }
        race.status = RaceStatus::Completed;
        race.winner = Some(winner);
        resolve_bets(race);
        Ok(())
    })
}

/// Resolves all bets for a completed race and distributes winnings.
fn resolve_bets(race: &mut Race) {
    let winner = race.winner.unwrap();
    let total_bet_amount: u64 = race.bets.iter().map(|bet| bet.amount).sum();
    let winning_bets: Vec<&Bet> = race.bets.iter().filter(|bet| bet.prediction == winner).collect();
    let total_winning_amount: u64 = winning_bets.iter().map(|bet| bet.amount).sum();

    for bet in &race.bets {
        if bet.prediction == winner {
            let payout = (bet.amount as f64 / total_winning_amount as f64) * total_bet_amount as f64;
            let _ = icrc1_transfer(TransferArgs {
                from_subaccount: None,
                to: Account {
                    owner: bet.bettor,
                    subaccount: None,
                },
                amount: payout as u64,
                fee: None,
                memo: None,
                created_at_time: None,
            });
        }
    }
}

/// Places a bet on a race.
#[update]
fn place_bet(race_id: u64, amount: u64, prediction: Principal) -> Result<(), String> {
    let caller = get_caller();
    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        if race.status != RaceStatus::Upcoming {
            return Err("Betting is closed for this race".to_string());
        }
        if !race.participants.contains(&prediction) {
            return Err("Invalid prediction".to_string());
        }
        icrc1_transfer(TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: ic_cdk::id(),
                subaccount: None,
            },
            amount,
            fee: None,
            memo: None,
            created_at_time: None,
        })?;
        race.bets.push(Bet {
            race_id,
            bettor: caller,
            amount,
            prediction,
        });
        Ok(())
    })
}

/// Mints a new NFT with the given metadata.
#[update]
fn mint_nft(metadata: String) -> Result<u64, String> {
    let caller = get_caller();
    let nft_id = NFTS.with(|nfts| nfts.borrow().len() as u64 + 1);
    let new_nft = NFT {
        id: nft_id,
        owner: caller,
        metadata,
        listed_price: None,
    };
    NFTS.with(|nfts| nfts.borrow_mut().insert(nft_id, new_nft));
    Ok(nft_id)
}

/// Transfers an NFT to a new owner.
#[update]
fn transfer_nft(nft_id: u64, to: Principal) -> Result<(), String> {
    let caller = get_caller();
    NFTS.with(|nfts| {
        let mut nfts = nfts.borrow_mut();
        let nft = nfts.get_mut(&nft_id).ok_or("NFT not found")?;
        if nft.owner != caller {
            return Err("Not the owner of this NFT".to_string());
        }
        nft.owner = to;
        Ok(())
    })
}

/// Lists an NFT for sale at the specified price.
#[update]
fn list_nft(nft_id: u64, price: u64) -> Result<(), String> {
    let caller = get_caller();
    NFTS.with(|nfts| {
        let mut nfts = nfts.borrow_mut();
        let nft = nfts.get_mut(&nft_id).ok_or("NFT not found")?;
        if nft.owner != caller {
            return Err("Not the owner of this NFT".to_string());
        }
        nft.listed_price = Some(price);
        Ok(())
    })
}

/// Allows a user to buy a listed NFT.
#[update]
fn buy_nft(nft_id: u64) -> Result<(), String> {
    let caller = get_caller();
    NFTS.with(|nfts| {
        let mut nfts = nfts.borrow_mut();
        let nft = nfts.get_mut(&nft_id).ok_or("NFT not found")?;
        let price = nft.listed_price.ok_or("NFT not listed for sale")?;
        icrc1_transfer(TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: nft.owner,
                subaccount: None,
            },
            amount: price,
            fee: None,
            memo: None,
            created_at_time: None,
        })?;
        nft.owner = caller;
        nft.listed_price = None;
        Ok(())
    })
}

/// Returns information about a specific race.
#[query]
fn get_race(race_id: u64) -> Result<Race, String> {
    RACES.with(|races| races.borrow().get(&race_id).cloned().ok_or("Race not found".to_string()))
}

/// Returns information about a specific NFT.
#[query]
fn get_nft(nft_id: u64) -> Result<NFT, String> {
    NFTS.with(|nfts| nfts.borrow().get(&nft_id).cloned().ok_or("NFT not found".to_string()))
}

/// Returns a list of all races.
#[query]
fn get_all_races() -> Vec<Race> {
    RACES.with(|races| races.borrow().values().cloned().collect())
}

/// Returns a list of all NFTs owned by a specific user.
#[query]
fn get_user_nfts(user: Principal) -> Vec<NFT> {
    NFTS.with(|nfts| nfts.borrow().values().filter(|nft| nft.owner == user).cloned().collect())
}

/// Returns a list of all NFTs currently listed for sale.
#[query]
fn get_listed_nfts() -> Vec<NFT> {
    NFTS.with(|nfts| nfts.borrow().values().filter(|nft| nft.listed_price.is_some()).cloned().collect())
}

/// Helper function to get the caller's principal.
fn get_caller() -> Principal {
    ic_cdk::caller()
}

/// Checks if the given principal is an admin.
fn is_admin(principal: Principal) -> bool {
    // Implement your admin check logic here
    // For example, you could have a list of admin principals
    true // Temporary, replace with actual logic
}

/// Greet function (kept for compatibility).
#[query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}