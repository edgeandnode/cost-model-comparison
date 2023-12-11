# Cost Model Comparison

Simple command line utility for fetching cost models & comparing fees

## Building

```
cargo build --release
```

Executable is placed in `./target/release/cost-models`

## Usage

Examples use [QmeBPZyEeaHyZAiFS2Q7cT3CESS49hhgGuT3E9S8RYoHNm](https://thegraph.com/explorer/subgraphs/GyijYxW9yiSRcEd5u2gfquSvneQKi5QuvU3WZgFyfFSn?view=Overview&chain=arbitrum-one).

- Fetch cost models

  ```sh
  cost-models fetch \
    --deployment QmeBPZyEeaHyZAiFS2Q7cT3CESS49hhgGuT3E9S8RYoHNm \
    --network-subgraph https://api.thegraph.com/subgraphs/name/graphprotocol/graph-network-arbitrum \
    | tee cost-models-QmeBPZyEeaHyZAiFS2Q7cT3CESS49hhgGuT3E9S8RYoHNm.json
  ```

- Execute cost models

  ```sh
  cost-models fees \
    --cost-models "$(cat cost-models-QmeBPZyEeaHyZAiFS2Q7cT3CESS49hhgGuT3E9S8RYoHNm.json)" \
    --query '{ _meta { block { number } } }'
  ```

  output as CSV:

  ```sh
  cost-models fees \
    --cost-models "$(cat cost-models-QmeBPZyEeaHyZAiFS2Q7cT3CESS49hhgGuT3E9S8RYoHNm.json)" \
    --query '{ _meta { block { number } } }' \
    | jq -r 'to_entries | .[] | [.key, .value] | @csv'
  ```
