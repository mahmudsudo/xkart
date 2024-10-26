use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};
use ic_cdk_macros::*;
use std::collections::HashMap;
use ic_cdk::api::time;

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

// New tokenization structs
#[derive(CandidType, Deserialize, Clone)]
pub struct TokenizationCampaign {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub target_amount: u64,
    pub current_amount: u64,
    pub asset_type: AssetType,
    pub status: CampaignStatus,
    pub investors: HashMap<Principal, u64>,
    pub end_time: u64,
}

#[derive(CandidType, Deserialize, Clone, PartialEq)]
pub enum AssetType {
    Arena,
    Driver,
    Kart,
}

#[derive(CandidType, Deserialize, Clone, PartialEq)]
pub enum CampaignStatus {
    Active,
    Completed,
    Failed,
}

/// Enhanced Race struct with additional fields
#[derive(CandidType, Deserialize, Clone)]
pub struct Race {
    pub id: u64,
    pub name: String,
    pub arena_id: u64,
    pub participants: Vec<RaceParticipant>,
    pub status: RaceStatus,
    pub winner: Option<Principal>,
    pub bets: Vec<Bet>,
    pub start_time: Option<u64>,
    pub end_time: Option<u64>,
    pub entry_fee: u64,
    pub total_prize_pool: u64,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct RaceParticipant {
    pub player: Principal,
    pub kart_id: u64,
    pub driver_id: u64,
    pub current_position: u32,
    pub lap_times: Vec<u64>,
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

/// Enhanced NFT struct with additional metadata
#[derive(CandidType, Deserialize, Clone)]
pub struct NFT {
    pub id: u64,
    pub owner: Principal,
    pub nft_type: NFTType,
    pub metadata: NFTMetadata,
    pub listed_price: Option<u64>,
    pub rarity: Rarity,
    pub creation_time: u64,
    pub transaction_history: Vec<Transaction>,
}

#[derive(CandidType, Deserialize, Clone)]
pub enum NFTType {
    Driver,
    Kart,
    Arena,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct NFTMetadata {
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub attributes: HashMap<String, String>,
}

#[derive(CandidType, Deserialize, Clone)]
pub enum Rarity {
    Common,
    Rare,
    Legendary,
}

#[derive(CandidType, Deserialize, Clone)]
pub struct Transaction {
    pub timestamp: u64,
    pub from: Principal,
    pub to: Principal,
    pub price: Option<u64>,
}

// State variables
thread_local! {
    static BALANCES: std::cell::RefCell<HashMap<Account, u64>> = std::cell::RefCell::new(HashMap::new());
    static TOTAL_SUPPLY: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
    static RACES: std::cell::RefCell<HashMap<u64, Race>> = std::cell::RefCell::new(HashMap::new());
    static NFTS: std::cell::RefCell<HashMap<u64, NFT>> = std::cell::RefCell::new(HashMap::new());
    static TOKENIZATION_CAMPAIGNS: std::cell::RefCell<HashMap<u64, TokenizationCampaign>> = std::cell::RefCell::new(HashMap::new());
    static ADMINS: std::cell::RefCell<Vec<Principal>> = std::cell::RefCell::new(Vec::new());
}

// ICRC-1 Token Implementation

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
        
        Ok(0) 
    })
}

#[query]
fn icrc1_balance_of(account: Account) -> u64 {
    BALANCES.with(|balances| {
        balances.borrow().get(&account).cloned().unwrap_or(0)
    })
}

#[query]
fn icrc1_total_supply() -> u64 {
    TOTAL_SUPPLY.with(|total_supply| *total_supply.borrow())
}

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

// Tokenization System Implementation

#[update]
fn create_tokenization_campaign(
    name: String,
    description: String,
    target_amount: u64,
    asset_type: AssetType,
    duration: u64,
) -> Result<u64, String> {
    let caller = ic_cdk::caller();
    if !is_admin(caller) {
        return Err("Only admins can create tokenization campaigns".to_string());
    }

    let campaign_id = TOKENIZATION_CAMPAIGNS.with(|campaigns| campaigns.borrow().len() as u64 + 1);
    let new_campaign = TokenizationCampaign {
        id: campaign_id,
        name,
        description,
        target_amount,
        current_amount: 0,
        asset_type,
        status: CampaignStatus::Active,
        investors: HashMap::new(),
        end_time: time() + duration,
    };

    TOKENIZATION_CAMPAIGNS.with(|campaigns| {
        campaigns.borrow_mut().insert(campaign_id, new_campaign);
    });

    Ok(campaign_id)
}

#[update]
fn invest_in_campaign(campaign_id: u64, amount: u64) -> Result<(), String> {
    let caller = ic_cdk::caller();
    
    TOKENIZATION_CAMPAIGNS.with(|campaigns| {
        let mut campaigns = campaigns.borrow_mut();
        let campaign = campaigns.get_mut(&campaign_id).ok_or("Campaign not found")?;
        
        if campaign.status != CampaignStatus::Active {
            return Err("Campaign is not active".to_string());
        }
        
        if time() > campaign.end_time {
            campaign.status = CampaignStatus::Failed;
            return Err("Campaign has ended".to_string());
        }

        // Transfer tokens from investor to campaign
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

        // Update campaign state
        *campaign.investors.entry(caller).or_insert(0) += amount;
        campaign.current_amount += amount;

        if campaign.current_amount >= campaign.target_amount {
            campaign.status = CampaignStatus::Completed;
            // Mint NFTs or tokens for investors based on their contribution
            mint_campaign_rewards(campaign_id)?;
        }

        Ok(())
    })
}

// Race System Implementation

#[update]
fn create_race(name: String, arena_id: u64, entry_fee: u64) -> Result<u64, String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can create races".to_string());
    }

    // Verify arena NFT exists
    NFTS.with(|nfts| {
        let nfts = nfts.borrow();
        if let Some(nft) = nfts.get(&arena_id) {
            if nft.nft_type != NFTType::Arena {
                return Err("Invalid arena ID".to_string());
            }
        } else {
            return Err("Arena not found".to_string());
        }
        Ok(())
    })?;

    let race_id = RACES.with(|races| races.borrow().len() as u64 + 1);
    let new_race = Race {
        id: race_id,
        name,
        arena_id,
        participants: Vec::new(),
        status: RaceStatus::Upcoming,
        winner: None,
        bets: Vec::new(),
        start_time: None,
        end_time: None,
        entry_fee,
        total_prize_pool: 0,
    };

    RACES.with(|races| races.borrow_mut().insert(race_id, new_race));
    Ok(race_id)
}

#[update]
fn join_race(race_id: u64, kart_id: u64, driver_id: u64) -> Result<(), String> {
    let caller = get_caller();
    
    // Verify NFT ownership
    verify_nft_ownership(caller, kart_id, NFTType::Kart)?;
    verify_nft_ownership(caller, driver_id, NFTType::Driver)?;

    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        
        if race.status != RaceStatus::Upcoming {
            return Err("Race is not open for joining".to_string());
        }

        // Check if player already joined
        if race.participants.iter().any(|p| p.player == caller) {
            return Err("Already joined this race".to_string());
        }

        // Pay entry fee
        icrc1_transfer(TransferArgs {
            from_subaccount: None,
            to: Account {
                owner: ic_cdk::id(),
                subaccount: None,
            },
            amount: race.entry_fee,
            fee: None,
            memo: None,
            created_at_time: None,
        })?;

        // Add to prize pool
        race.total_prize_pool += race.entry_fee;

        // Add participant
        race.participants.push(RaceParticipant {
            player: caller,
            kart_id,
            driver_id,
            current_position: 0,
            lap_times: Vec::new(),
        });

        Ok(())
    })
}

#[update]
fn update_race_progress(
    race_id: u64,
    participant_positions: Vec<(Principal, u32)>,
    lap_times: Vec<(Principal, u64)>,
) -> Result<(), String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can update race progress".to_string());
    }

    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        
        if race.status != RaceStatus::InProgress {
            return Err("Race is not in progress".to_string());
        }

        // Update positions
        for (player, position) in participant_positions {
            if let Some(participant) = race.participants.iter_mut().find(|p| p.player == player) {
                participant.current_position = position;
            }
        }

        // Update lap times
        for (player, time) in lap_times {
            if let Some(participant) = race.participants.iter_mut().find(|p| p.player == player) {
                participant.lap_times.push(time);
            }
        }

        Ok(())
    })
}

#[update]
fn end_race(race_id: u64) -> Result<(), String> {
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

        // Determine winner based on current positions
        let winner = race.participants.iter()
            .find(|p| p.current_position == 1)
            .map(|p| p.player)
            .ok_or("No winner found")?;

        race.winner = Some(winner);
        race.status = RaceStatus::Completed;
        race.end_time = Some(time());

        // Distribute rewards
        distribute_race_rewards(race_id)?;
        
        Ok(())
    })
}

#[update]
fn distribute_race_rewards(race_id: u64) -> Result<(), String> {
    RACES.with(|races| {
        let mut races = races.borrow_mut();
        let race = races.get_mut(&race_id).ok_or("Race not found")?;
        
        if race.status != RaceStatus::Completed {
            return Err("Race is not completed".to_string());
        }

        let total_prize = race.total_prize_pool;
        
        // Sort participants by position
        let mut participants = race.participants.clone();
        participants.sort_by_key(|p| p.current_position);

        // Distribution scheme: 50% to winner, 30% to second, 20% to third
        let reward_percentages = [(1, 0.5), (2, 0.3), (3, 0.2)];

        for (position, percentage) in reward_percentages {
            if let Some(participant) = participants.iter().find(|p| p.current_position == position) {
                let reward = (total_prize as f64 * percentage) as u64;
                icrc1_transfer(TransferArgs {
                    from_subaccount: None,
                    to: Account {
                        owner: participant.player,
                        subaccount: None,
                    },
                    amount: reward,
                    fee: None,
                    memo: None,
                    created_at_time: None,
                })?;
            }
        }

        Ok(())
    })
}

// NFT System Implementation

#[update]
fn mint_nft(
    name: String,
    description: String,
    image_url: String,
    nft_type: NFTType,
    rarity: Rarity,
    attributes: HashMap<String, String>,
) -> Result<u64, String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can mint NFTs".to_string());
    }

    let nft_id = NFTS.with(|nfts| nfts.borrow().len() as u64 + 1);
    let new_nft = NFT {
        id: nft_id,
        owner: caller,
        nft_type,
        metadata: NFTMetadata {
            name,
            description,
            image_url,
            attributes,
        },
        listed_price: None,
        rarity,
        creation_time: time(),
        transaction_history: vec![],
    };

    NFTS.with(|nfts| nfts.borrow_mut().insert(nft_id, new_nft));
    Ok(nft_id)
}

#[update]
fn transfer_nft(nft_id: u64, to: Principal) -> Result<(), String> {
    let caller = get_caller();
    NFTS.with(|nfts| {
        let mut nfts = nfts.borrow_mut();
        let nft = nfts.get_mut(&nft_id).ok_or("NFT not found")?;
        if nft.owner != caller {
            return Err("Not the owner of this NFT".to_string());
        }

        // Record transaction
        nft.transaction_history.push(Transaction {
            timestamp: time(),
            from: caller,
            to,
            price: None,
        });

        nft.owner = to;
        Ok(())
    })
}

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

#[update]
fn buy_nft(nft_id: u64) -> Result<(), String> {
    let caller = get_caller();
    NFTS.with(|nfts| {
        let mut nfts = nfts.borrow_mut();
        let nft = nfts.get_mut(&nft_id).ok_or("NFT not found")?;
        let price = nft.listed_price.ok_or("NFT not listed for sale")?;
        
        // Transfer payment
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

        // Record transaction
        nft.transaction_history.push(Transaction {
            timestamp: time(),
            from: nft.owner,
            to: caller,
            price: Some(price),
        });

        nft.owner = caller;
        nft.listed_price = None;
        Ok(())
    })
}

// Query Functions

#[query]
fn get_campaign(campaign_id: u64) -> Result<TokenizationCampaign, String> {
    TOKENIZATION_CAMPAIGNS.with(|campaigns| {
        campaigns.borrow()
            .get(&campaign_id)
            .cloned()
            .ok_or("Campaign not found".to_string())
    })
}

#[query]
fn get_active_campaigns() -> Vec<TokenizationCampaign> {
    TOKENIZATION_CAMPAIGNS.with(|campaigns| {
        campaigns.borrow()
            .values()
            .filter(|campaign| campaign.status == CampaignStatus::Active)
            .cloned()
            .collect()
    })
}

#[query]
fn get_race(race_id: u64) -> Result<Race, String> {
    RACES.with(|races| {
        races.borrow()
            .get(&race_id)
            .cloned()
            .ok_or("Race not found".to_string())
    })
}

#[query]
fn get_upcoming_races() -> Vec<Race> {
    RACES.with(|races| {
        races.borrow()
            .values()
            .filter(|race| race.status == RaceStatus::Upcoming)
            .cloned()
            .collect()
    })
}

#[query]
fn get_nft(nft_id: u64) -> Result<NFT, String> {
    NFTS.with(|nfts| {
        nfts.borrow()
            .get(&nft_id)
            .cloned()
            .ok_or("NFT not found".to_string())
    })
}

#[query]
fn get_user_nfts(user: Principal) -> Vec<NFT> {
    NFTS.with(|nfts| {
        nfts.borrow()
            .values()
            .filter(|nft| nft.owner == user)
            .cloned()
            .collect()
    })
}

#[query]
fn get_listed_nfts() -> Vec<NFT> {
    NFTS.with(|nfts| {
        nfts.borrow()
            .values()
            .filter(|nft| nft.listed_price.is_some())
            .cloned()
            .collect()
    })
}

// Helper Functions

fn verify_nft_ownership(owner: Principal, nft_id: u64, expected_type: NFTType) -> Result<(), String> {
    NFTS.with(|nfts| {
        let nfts = nfts.borrow();
        let nft = nfts.get(&nft_id).ok_or("NFT not found")?;
        
        if nft.owner != owner {
            return Err("Not the owner of this NFT".to_string());
        }
        
        if nft.nft_type != expected_type {
            return Err(format!("NFT is not a {:?}", expected_type));
        }
        
        Ok(())
    })
}

fn is_minting_authority(principal: Principal) -> bool {
    ADMINS.with(|admins| admins.borrow().contains(&principal))
}

fn is_admin(principal: Principal) -> bool {
    ADMINS.with(|admins| admins.borrow().contains(&principal))
}

fn get_caller() -> Principal {
    ic_cdk::caller()
}

// Initialize admins
#[init]
fn init() {
    ADMINS.with(|admins| {
        let mut admins = admins.borrow_mut();
        admins.push(ic_cdk::caller()); // Initialize with deployer as admin
    });
}

#[update]
fn add_admin(principal: Principal) -> Result<(), String> {
    let caller = get_caller();
    if !is_admin(caller) {
        return Err("Only admins can add new admins".to_string());
    }
    
    ADMINS.with(|admins| {
        admins.borrow_mut().push(principal);
    });
    
    Ok(())
}