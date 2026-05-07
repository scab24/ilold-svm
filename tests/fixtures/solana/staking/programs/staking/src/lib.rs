use anchor_lang::prelude::*;

declare_id!("AQjgX8bsAU8CvvEoP9q36vAbT83dRxdjK4zGEhhn6SFc");

#[program]
pub mod staking {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>, reward_rate: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        pool.admin = ctx.accounts.admin.key();
        pool.total_staked = 0;
        pool.total_rewards = 0;
        pool.reward_rate = reward_rate;
        msg!("Pool initialized by {} with reward_rate={}", pool.admin, reward_rate);
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::ZeroAmount);
        let pool = &mut ctx.accounts.pool;
        let user_stake = &mut ctx.accounts.user_stake;
        if user_stake.user == Pubkey::default() {
            user_stake.user = ctx.accounts.user.key();
            user_stake.amount = 0;
            user_stake.claimed_rewards = 0;
        }
        user_stake.amount = user_stake.amount.checked_add(amount).ok_or(StakingError::Overflow)?;
        pool.total_staked = pool.total_staked.checked_add(amount).ok_or(StakingError::Overflow)?;
        msg!("{} staked {} (user_total={}, pool_total={})", user_stake.user, amount, user_stake.amount, pool.total_staked);
        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let user_stake = &mut ctx.accounts.user_stake;
        require!(user_stake.user == ctx.accounts.user.key(), StakingError::WrongUser);
        require!(amount > 0, StakingError::ZeroAmount);
        require!(user_stake.amount >= amount, StakingError::InsufficientBalance);
        user_stake.amount -= amount;
        pool.total_staked -= amount;
        msg!("{} unstaked {} (user_remaining={}, pool_total={})", user_stake.user, amount, user_stake.amount, pool.total_staked);
        Ok(())
    }

    pub fn add_rewards(ctx: Context<AddRewards>, amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        require!(pool.admin == ctx.accounts.admin.key(), StakingError::WrongAdmin);
        pool.total_rewards = pool.total_rewards.checked_add(amount).ok_or(StakingError::Overflow)?;
        msg!("Admin added {} rewards (total_rewards={})", amount, pool.total_rewards);
        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let pool = &mut ctx.accounts.pool;
        let user_stake = &mut ctx.accounts.user_stake;
        require!(user_stake.user == ctx.accounts.user.key(), StakingError::WrongUser);
        require!(pool.total_staked > 0, StakingError::EmptyPool);
        let share = (pool.total_rewards as u128)
            .checked_mul(user_stake.amount as u128)
            .ok_or(StakingError::Overflow)?
            .checked_div(pool.total_staked as u128)
            .ok_or(StakingError::Overflow)?;
        let pending = (share as u64).saturating_sub(user_stake.claimed_rewards);
        user_stake.claimed_rewards = user_stake.claimed_rewards.checked_add(pending).ok_or(StakingError::Overflow)?;
        msg!("{} claimed {} rewards (lifetime={})", user_stake.user, pending, user_stake.claimed_rewards);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(init, payer = admin, space = 8 + 32 + 8 + 8 + 8)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(init_if_needed, payer = user, space = 8 + 32 + 8 + 8)]
    pub user_stake: Account<'info, UserStake>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user_stake: Account<'info, UserStake>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct AddRewards<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub user_stake: Account<'info, UserStake>,
    pub user: Signer<'info>,
}

#[account]
pub struct Pool {
    pub admin: Pubkey,
    pub total_staked: u64,
    pub total_rewards: u64,
    pub reward_rate: u64,
}

#[account]
pub struct UserStake {
    pub user: Pubkey,
    pub amount: u64,
    pub claimed_rewards: u64,
}

#[error_code]
pub enum StakingError {
    #[msg("Amount must be greater than zero")] ZeroAmount,
    #[msg("Arithmetic overflow")] Overflow,
    #[msg("Insufficient staked balance")] InsufficientBalance,
    #[msg("Wrong user for this UserStake account")] WrongUser,
    #[msg("Wrong admin for this pool")] WrongAdmin,
    #[msg("Pool has no stakers")] EmptyPool,
}
