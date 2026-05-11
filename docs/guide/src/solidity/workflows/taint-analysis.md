# Taint Analysis -- Tracing User Input

Forward slicing traces how a variable's value propagates through a function body. When the starting variable is a user-controlled parameter, the slice acts as a taint analysis: it reveals every state variable, local variable, and external call that the attacker-controlled input can reach.

## Identifying entry points

List functions and note which ones are externally callable:

```
Staking > f
  deposit          External   writes state
  withdraw         External   writes state
  claimRewards     External   writes state
  setRewardRate    External   writes state   onlyOwner
  pause            External   writes state   onlyOwner
  unpause          External   writes state   onlyOwner
  rewardPerToken   Public     read-only
  earned           Public     read-only
```

Functions marked `External` accept parameters from untrusted callers. The `onlyOwner` annotation means the function has an access-control modifier, but the parameter values themselves are still caller-supplied. For taint analysis, focus on functions where user-controlled parameters flow into state writes or external calls.

In this contract, `deposit(uint256 amount)` and `withdraw(uint256 amount)` both take an `amount` parameter from the caller and write state.

## Forward-slicing amount in deposit

Use the forward slice to trace where `amount` goes:

```
Staking > sl deposit amount --forward
  Forward slice (amount -> sinks):
    [0] require(amount > 0, "Cannot stake 0")
    [1] stakingToken.transferFrom(msg.sender, address(this), amount)
    [2] balances[msg.sender] += amount
    [3] totalStaked += amount
```

The slice shows four statements that depend on `amount`:

1. A `require` check validates that `amount` is positive.
2. An external call to `stakingToken.transferFrom` uses `amount` directly.
3. `balances[msg.sender]` is incremented by `amount`.
4. `totalStaked` is incremented by `amount`.

The user-controlled value reaches two state variables (`balances`, `totalStaked`) and one external call (`transferFrom`). The `require` at line 0 is the only validation gate.

## Forward-slicing amount in withdraw

```
Staking > sl withdraw amount --forward
  Forward slice (amount -> sinks):
    [0] require(amount > 0, "Cannot withdraw 0")
    [1] require(balances[msg.sender] >= amount, "Insufficient balance")
    [2] balances[msg.sender] -= amount
    [3] totalStaked -= amount
    [4] stakingToken.transfer(msg.sender, amount)
```

The `withdraw` slice is similar but has an additional check: `balances[msg.sender] >= amount`. This prevents withdrawing more than deposited. The subtraction operations mirror the additions in `deposit`.

Compare the two slices side by side:

| Statement type | deposit | withdraw |
|---|---|---|
| Validation | `amount > 0` | `amount > 0`, `balances >= amount` |
| External call | `transferFrom(sender, this, amount)` | `transfer(sender, amount)` |
| State writes | `balances += amount`, `totalStaked += amount` | `balances -= amount`, `totalStaked -= amount` |

The asymmetry in validation is expected here: `deposit` relies on the ERC-20 `transferFrom` to enforce that the caller actually has the tokens, while `withdraw` must check the internal balance explicitly.

## What the slice reveals about state variable control

The forward slice of a user parameter tells you which state variables are directly controlled by external input. From the two slices above:

- `balances` is written in both functions using `amount` directly. Any arithmetic error in the `+=` or `-=` operations would let an attacker manipulate their balance.
- `totalStaked` is written in both functions using `amount` directly. Since `rewardPerToken` reads `totalStaked` (visible via `who totalStaked`), a corrupted `totalStaked` would affect reward calculations for all users.
- Neither `rewardPerTokenStored` nor `rewards` appear in the forward slice of `amount`. These variables are written by the `updateReward` modifier, which derives values from `rewardPerToken()` and `earned()` -- not from the caller's `amount` parameter directly.

## Mapping to vulnerability patterns

Forward slice results map to common vulnerability classes:

**Unchecked arithmetic.** If `balances[msg.sender] += amount` or `totalStaked += amount` can overflow, the attacker controls the input that triggers it. For Solidity 0.8+, the compiler inserts overflow checks automatically. For older versions, look for SafeMath usage in the slice.

**Missing validation.** If the slice shows a state write or external call with no preceding `require` that bounds the parameter, the input flows unchecked. In `deposit`, the only check is `amount > 0` -- there is no upper bound. Whether this is a problem depends on the token's `transferFrom` behavior.

**External call with user input.** Both slices show external calls (`transferFrom`, `transfer`) that use `amount`. If the token contract is untrusted or implements callbacks (e.g., ERC-777), the attacker controls the value passed to a potentially re-entrant call. Check whether the state writes happen before or after the external call (CEI pattern).

**Cross-function impact.** Use `who` to find all readers of a tainted state variable, then forward-slice the reader to see downstream effects. For example, `rewardPerToken` reads `totalStaked`, so a manipulated `totalStaked` propagates into every user's reward calculation.

## Practical workflow

1. Run `f` to list entry points.
2. For each external function with parameters, run `sl <func> <param> --forward`.
3. Note which state variables appear in each forward slice.
4. Run `who <var>` on each affected state variable to find cross-function readers.
5. Run `sl <reader> <var> --forward` to trace second-order propagation.
6. Record findings with `fi` when a tainted path reaches a sensitive sink without adequate validation.

## Related pages

- [Full Audit Walkthrough](./audit-walkthrough.md)
- [Known Limitations](../limitations.md) -- forward slice caveats
