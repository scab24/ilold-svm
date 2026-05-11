# Contract Commands

Contract commands inspect the structure of the loaded contracts without modifying the session.

## functions

`f` or `functions`

Lists the callable functions in the active contract with their access level and tags.

```
ilold[Staking]> f

  [P] deposit           writes state, external calls
  [P] withdraw          writes state, external calls
  [P] claimRewards      writes state, external calls
  [R] setRewardRate     writes state
  [R] pause             writes state
  [R] unpause           writes state
  [P] rewardPerToken    view
  [P] earned            view
```

Badges: `[P]` public/external, `[R]` restricted (admin-gated), `[I]` internal, `[S]` special. Tags indicate `writes state`, `external calls`, or `view` (read-only, no external calls).

**Returns:** `FunctionList { functions: [FunctionEntry { name, access, writes_state, has_external_calls, is_read_only }] }`.

## funcs-all

`fa` or `funcs-all`

Lists all accessible functions including those inherited from parent contracts.

```
ilold[Staking]> fa

  [P] deposit           writes state, external calls
  [P] withdraw          writes state, external calls
  [P] claimRewards      writes state, external calls
  [R] setRewardRate     writes state
  [R] pause             writes state
  [R] unpause           writes state
  [P] rewardPerToken    view
  [P] earned            view

  inherited:
  [P] owner              from Ownable
  [P] transferOwnership  from Ownable
```

Inherited functions are listed separately with their origin contract.

**Returns:** `FunctionListAll { functions: [AccessibleFunctionEntry { name, access, writes_state, has_external_calls, is_read_only, origin, is_inherited }] }`.

## vars

`v` or `vars`

Lists the state variables of the active contract with their type and mutability tag.

```
ilold[Staking]> v

  mutable   owner                   address
  mutable   paused                  bool
  mutable   rewardRate              uint256
  mutable   lastUpdateTime          uint256
  mutable   rewardPerTokenStored    uint256
  mutable   balances                mapping(address => uint256)
  mutable   userRewardPerTokenPaid  mapping(address => uint256)
  mutable   rewards                 mapping(address => uint256)
  mutable   totalStaked             uint256
```

Tags are `mutable`, `const`, or `immutable`.

## vars-all

`va` or `vars-all`

Lists all accessible state variables including inherited ones.

```
ilold[Staking]> va

  mutable   owner                   address
  mutable   paused                  bool
  mutable   rewardRate              uint256
  ...
```

**Returns:** `StateVarListAll { state_vars: [AccessibleStateVarEntry { name, type_name, is_constant, is_immutable, origin, is_inherited }] }`. Inherited entries print under an `inherited:` section with `from <origin>`.

## contracts

`ct` or `contracts`

Lists all contracts in the loaded project with their type badge, function count, state variable count, and `inherits` clause when present.

```
ilold[Staking]> ct

  [I] IERC20    3 functions, 0 state vars
  [C] Staking   9 functions, 11 state vars  ← current
```

Type badges: `[C]` contract, `[I]` interface, `[L]` library, `[A]` abstract.

## use

`use <contract>`

Switches the active contract. Clears the current session steps.

```
ilold[Staking]> use Ownable

  ✓ Now using: Ownable
  Cleared 2 step(s) from previous contract
```

After switching, all session and analysis commands operate on the new contract.
