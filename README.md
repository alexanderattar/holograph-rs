# Introduction

In an evolving multichain landscape, the seamless transfer of assets between blockchains is essential. Holograph, launched in January 2023, introducing a new crypto primitive in holographic assets. These exist natively across multiple chains, ensuring native interoperability and eliminating asset lock-in. Rather than the traditional "lock and mint" approach, which can lead to asset risks and complexities, Holograph offers a unique solution that decouples value from the application and messaging layers, facilitating asset movement between networks. As a result, holographic assets stand out for their preservation of data across networks, ensuring consistent contract addresses and token IDs. This innovation, coupled with tools provided by Holograph, positions holographic assets as the key to secure cross-chain value transfer, paving the way for a frictionless multichain experience.

## Problem: UX Friction

When a token is bridged from one blockchain to another, it's locked on the origin chain and a new token is minted on the destination chain. This necessitates at least 4 transactions, 2 network additions to a wallet, and 2 different gas tokens for fee payments, culminating in an often cumbersome user experience.

Here's a breakdown of the current process:

- **On Chain A:**
  - Tx 1: Approve tokens to be spent (gas fee on Chain A)
  - Tx 2: Lock the token (gas fee on Chain A)
- **User switches network to Chain B:**
  - Tx 3: Approve tokens to be spent (gas fee on Chain B)
  - Tx 4: Mint the token (gas fee on Chain B)

## Solution: Holograph Operator Network

Holograph employs the use of Operators, which is a network of ecosystem participants who execute bridge transactions for users. This reduces user friction when bridging tokens by (i) reducing the number of transactions needed to complete the process, (ii) removing the need to switch networks, and (iii) removing the need to manage multiple gas tokens. The right to perform work on Holograph is made possible through the use of the protocol’s native token, HLG. Operators bond HLG to earn the right to perform work. Operators who fail to perform this work will have their bonded tokens slashed.

## Operator CLI

The Operator CLI is designed to listen for, select, and execute bridge transactions. An operator must configure the CLI on every blockchain they want to execute jobs.

To operate, participants must:

1. Run the Operator CLI (or a custom-built client)
2. Select a blockchain
3. Deposit the selected blockchain's native gas token
4. Bond HLG to a selected pod
5. Execute bridge transactions when selected

## Operator Job Selection

To execute bridge transactions, operators must join a _pod_. Each pod provides a different probability of selection, based on increasingly higher minimum bond requirements. The probability of job selection is based on a specific pod being joined, and the number of operators bonded in the specific pod. To move to a different pod, an operator must withdraw and re-bond HLG. Operators who withdraw HLG will be charged a 0.1% fee, the proceeds of which will be burned or returned to the Treasury.

Pods are created in the **HolographOperator** smart contract. To join a pod, operators must bond a minimum amount of HLG, based on the pod they want to join. Lower number pods cost less to bond and accept more operators. Higher number pods cost more to bond and accept less operators. Operators can freely choose which pod they want to join.

If a pod operator limit is reached, the bond amount increases with each new operator. There are no limits to the number of pods that can be available. Furthermore, an operator can only participate in one pod per network at any given time.

Each time a new job is made available, one pod is randomly selected. Once selected, one random operator is selected to finalize the job. When an operator is selected for a job, they are temporarily removed from the pod, until they complete the job. If an operator successfully finalizes a job, they earn a reward and are placed back into their selected pod.

If an operator fails to complete the job, a percentage of their bond is slashed and their balance is checked to see if there is enough HLG left to re-bond back to their selected pod. If there’s enough HLG to re-bond, the operator is placed back into their selected pod. If there’s not enough HLG to re-bond, the operator is returned the remainder of their bonded tokens (if any), and they are permanently removed from pod selection. A removed operator can choose to join a new pod at any time in the future, granted they have enough HLG to bond.

In addition to the primary operator, five backup operators are also selected in case the primary operator fails to complete the job. Backup operators are allowed to try and finalize the job in sequence of selection. For example, backup operator `n` can only try to finalize a job if `t * n` amount of time has passed since the job was posted. This mechanic allows backup operators to earn the primary operator’s slashed token amount on top of the reward (see the “Rewards” section for more details).

Operator jobs are given specific gas limits. This is meant to prevent gas spike abuse (e.g., as a form of DoS attack), bad code, or smart contract reverts from penalizing good-faith operators. If an operator is late to finalize a job and another operator steps in to take its place, if the gas price is above the set limit, the selected operator will not get slashed. A job is considered successful if it does not revert, or if it reverts but gas limits were followed correctly. Failed jobs can be re-done (for an additional fee), can be returned to origin chain (for an additional fee), or left untouched entirely. This shifts the financial responsibility towards users, rather than operators.

Gas limits are embedded in the cross-chain transaction. When bridging NFTs, users can specify the max gas limit and gas price and pay the equivalent in native gas tokens. When a job is executed, the portion that executes the bridge request is wrapped by a non-reverting function that sets max gas limit to that of the user’s settings. If the transaction fails due to an out of gas error, or any other errors inside of that kind, it will not revert but instead be logged as failed job. Gas price is used to check the transaction and see if current gas price is over the set maximum gas price. If the gas is higher, slashing does not occur.

To prevent operator abuse by un-bonding while processing a job, the bond amount is put in escrow and only added back when the job is successfully completed. In other words, if an operator tries to un-bond while a job has been assigned to them, they would leave only with the remaining tokens and not the ones that were needed for the job.

The following represents a simple _OperatorJob_ object.

```typescript
struct OperatorJob {
  // which pod was selected for this job
  pod: number;

  // how much time is given to finish the job
  // can be different for different networks
  blockTimes: number;

  // the address of operator that was selected to finalize the job
  operator: address;

  // the block number when the job was published
  // used for easy lookup on-chain
  startBlock: number;

  // the timestamp of when job was published
  // all counters start from that time
  startTimestamp: number;

  // array of backup operators from selected pod
  // referenced by position index in pod
  fallbackOperators: number[5];
}
```

### Randomness

In order to make pod and operator selection mathematically random, a random number is generated by running:

`sha3(sha3(jobPayload) + jobNonce + blockNumber + blockTimestamp)`

The \*\*`jobNonce` is a unique chain-specific number that gets incremented each time a new job request is submitted to the chain. The `blockNumber` is the number of the mined block in which the transaction was included. The `blockTimestamp` is the UNIX timestamp that is assigned to each block the moment a miner has mined it. The `jobNonce`, `blockNumber`, `blockTimestamp`, and `jobPayload` are combined into a single hash, providing enough randomization to make it impossible to calculate, predict, or manipulate the selection process.

To select a pod, this random number is then reduced against the total number of pods available, to choose a pod randomly, expressed as follows:

`randomNumber % pods.length`

To select an operator, the same random number is then reduced against the total number of operators available in a pod, to choose an operator randomly, expressed as follows:

`randomNumber % pods[x].length`

Once a pod and an operator have been selected, the random number is used in conjunction with the block hash of a previously mined block, expressed as follows:

`unchecked{ random + blockHash(block.number - x) }`

To select backup operators, 5 new random numbers are generated inside of the same pod.

### Gas Optimizations

Storing data on the blockchain can get expensive really fast. To minimize gas consumption, operator selection logic is calculated in memory when a cross-chain message is received. The resulting calculations are then tightly packed into a 32 byte sized storage slot (the smallest available) using [bitwise operations](https://en.wikipedia.org/wiki/Bitwise_operation). This data is then saved in a mapping, with the job hash `sha3(jobPayload)` as the lookup key. This method makes the smart contract code more complex, but results in very efficient gas usage.

### Operator Bond & Pod Calculations

_Minimum Bond Amounts_ are dynamically calculated based on some initial variables. These variables can be changed or set differently for each blockchain.

- baseBondAmount = `100 * (10^18)`
- podMultiplier = `2`
- operatorThreshold = `1000`
- operatorThresholdStep = `10`
- operatorThresholdMultiplier = `0.01`

_Pod Threshold_ is calculated by running this formula with a pod number in question:

`operatorThreshold / (2^pod)`

Using the above variables and selecting for `Pod 1` (or `0` in array language), this would result in the number `1000`.

$$
\frac{1000}{2^0}
$$

_Minimum Bond_ _Amounts_ are calculated by running this formula with a pod number in question: `baseBondAmount * (podMultiplier^pod)`.

$$
(100*10^{18}) * 2^0
$$

Using the above variables and selecting for `Pod 1` (or `0` in array language), this would result in the number `100000000000000000000`, or `100 HLG` (using 18 decimal places).

_Current Bond Amount_ is calculated by running the _Minimum Bond_ _Amount_ formula above. If the current number of operators in a specific pod is greater than the _Pod Threshold,_ then the _Minimum Bond_ _Amount_ needs to be added with `(bondAmount * operatorThresholdMultiplier) * ((position - threshold) / operatorThresholdStep)`

Using the above variables and selecting for `Pod 1` (or `0` in array language) and getting into position `1500`, this would result in the number `150000000000000000000`, or `150 HLG`.

$$
(100*10^{18}) * 2^0 + (((100*10^{18}) * 2^0) * 0.01)(\frac{1}{10}(1500 - \frac{1000}{2^0}))
$$

## Slashing

If an operator acts maliciously, an amount of their bonded HLG will get slashed. Misbehavior includes (i) downtime, (ii) double-signing transactions, and (iii) abusing transaction speeds. 50% of the slashed HLG will be rewarded to the next operator to execute the transaction, and the remaining 50% will be burned or returned to the Treasury.

## Burning

Up to 2,000,000,000 HLG (20% of the total supply) will be subject to burning. <!-- Burning will apply to the rewards allocated to the first 20 networks added to the protocol. In other words, for each of the first 20 networks, 50% of the operator rewards (100,000,000 HLG) will be subject to burning.--> After the cap is reached, tokens that meet the burn conditions will instead be returned to the Treasury.

HLG will be burned programmatically based on the following conditions:

- Unclaimed HLG from public distributions
- Withdrawal fees when unbonding from a pod
- 50% of operator slash amounts

<!--
| Number of Slashes  | Percentage of Bond |
|--------------------|--------------------|
| 1                  | 4%                 |
| 2                  | 16%                |
| 3                  | 36%                |
| 4                  | 64%                |
| 5                  | 100%               |
-->

<!-- ## Rewards

 A capped supply of 4,000,000,000 HLG (40% of the total supply) will be allocated in tranches to operators as a reward for executing bridge transactions. In other words, each of the first 20 blockchains will be allocated a total of 200,000,000 HLG in rewards, the supply of which will reduce in tranches, based on bridge transaction milestones, as outlined in the table below (there will be no additional rewards allocated after the first 20 blockchains). Operators will be rewarded HLG per job executed. For example, during Reward Era 1, each job executed will net 1,000 HLG, during Reward Era 2, each job executed will net 50 HLG, and so on. -->

<!--
| Reward Era | Rewards Per Era | Transactions  | Rewards Per 1,000 Tx | Supply Reduction |
|------------|-----------------|---------------|----------------------|------------------|
| 1          | 100,000,000     | 100,000       | 1,000,000            | 50%              |
| 2          | 50,000,000      | 1,000,000     | 50,000               | 25%              |
| 3          | 25,000,000      | 10,000,000    | 2,500                | 12.5%            |
| 4          | 12,500,000      | 100,000,000   | 125                  | 6.25%            |
| 5          | 12,500,000      | 1,000,000,000 | 12.5                 | 6.25%            |
-->

<!-- ## Fees

The gas a user pays on the origin chain is calculated to cover the cost of (i) burning the NFT on Chain A, (ii) relaying a messaging to Chain B, and (iii) minting the NFT on Chain B. Additionally, Holograph will charge 1% of this fee, which goes to the Treasury. Any gas that is left over after executing these transactions goes to operators.

The table below itemizes an example of the protocol’s fee structure using arbitrary gas unit denominations. Note that gas fees are variable and that the amounts in this example are for illustrative purposes only.

| Fee Type                   | Gas Amount      |
|----------------------------|-----------------|
| Bridge Request tx fee      | 100 gas units   |
| Cross-chain Message tx fee | 50 gas units    |
| Bridge Execution tx fee    | 100 gas units   |
| Base tx fee                | 250 gas units   |
| Protocol tx fee            | 2.5 gas units   |
| Estimated Base tx fee      | 277.5 gas units |

You will notice that the Estimated Base transaction fee is 25 gas units over the sum of the first 3 transactions. This is to account the gas fee variability caused by factors such as network traffic, supply of validators, and demand for transaction verification to ensure that the transaction will always be executed.
-->
