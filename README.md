# jup-carrot-implementation

Carrot Protocol Integration with Jupiter Router

## What is Carrot?

Carrot is the easiest way to passively benefit from DeFi on Solana. We've crafted a single token that allows you to accrue the highest average yield across the Solana DeFi landscape. By continually monitoring every lending protocol, we rebalance funds whenever there is a more optimal rate available. Carrot ensures your stablecoins capture higher APRs quicker, optimizing your returns efficiently and effectively.

![Carrot Overview](carrot-overview.png)

### TODO

- strategy integration
- clean up code, dont use unwrap, collapse into more functions
- return error? or 0 if not enough tokens for redemption, like we have no pyusd
- testing:
  - management, performance fee

#### Questions

- can i use multiple ixns
- my program does not init the ATA if they dont have, client side we detect and add that ix if necessary
- what do I put for Swap type in SwapAndAccountMetas
