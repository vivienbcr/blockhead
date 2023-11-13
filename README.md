# Blockhead

Blockhead is simple application used to parse blockchains head state and monitor responses from multiple sources. Blockhead expose results via HTTP API and Prometheus metrics.

## Supported blockchains / providers

| Blockchain | Network                  | Endpoint                                                                           | Notes                                                                                                  |
| ---------- | ------------------------ | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Bitcoin    | Mainnet, Testnet         | [RPC](https://developer.bitcoin.org/reference/rpc/)                                |                                                                                                        |
|            |                          | [Blockstream](https://github.com/Blockstream/esplora/blob/master/API.md)           |                                                                                                        |
|            |                          | [Blockcypher](https://www.blockcypher.com/dev/bitcoin/#introduction)               | Tune rate limit to don't reach [429 Errors](https://developer.mozilla.org/fr/docs/Web/HTTP/Status/429) |
| Ethereum   | Mainnet, Goerli, Sepolia | [RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/)                      |                                                                                                        |
| Tezos      | Mainnet, Ghostnet        | [RPC](https://tezos.gitlab.io/active/rpc.html)                                     |                                                                                                        |
|            |                          | [Tzkt](https://api.tzkt.io/)                                                       |                                                                                                        |
|            |                          | [Tzstats](https://tzstats.com/docs/api#tezos-api)                                  |                                                                                                        |
| Ewf        | Mainnet, Volta           | [RPC](https://github.com/energywebfoundation/ewf-rpc)                              |                                                                                                        |
| Polkadot   | Mainnet, Westend, Kusama | [RPC](https://polkadot.js.org/docs/substrate/rpc/)                                 |                                                                                                        |
|            |                          | [Subscan](https://support.subscan.io)                                              |                                                                                                        |
| Moonbeam   | Mainnet, Moonriver       | [RPC](https://docs.moonbeam.network/builders/get-started/eth-compare/rpc-support/) |                                                                                                        |
| Starknet   | Mainnet, Testnet2        | [RPC](https://github.com/starkware-libs/starknet-specs)                            |                                                                                                        |
| Avalanche  | Mainnet, Fuji            | [RPC](https://docs.avax.network/apis/avalanchego/apis/c-chain)                     | For https://api.avax.network/ext/bc/C/rpc use https://api.avax.network                                 |
| Polygon    | Mainnet, Mumbai          | [RPC](https://wiki.polygon.technology)                                             |                                                                                                        |

## Features

- Custom header
- Basic Http auth
- Custom rate limit / retry / delay between requests
- Config file hot reload

## Usage

### Create config file

```yaml
# Path: config.yaml or whatever.yaml
# Global options apply per default to every protocol / network / endpoint
global:
  # head_length is the number of blocks to be fetched from the head of the chain
  networks_options:
    head_length: 5
  # Gobal configuration for all endpoints, if not defined in endpoint, global will be used
  options:
    # Retry define how many time worker will try to call instance if he fail
    retry: 3
    # Delay between every retry
    delay: 1
    # Rate between every scrapping task
    rate: 4
  server:
    # On wich port json rpc will be served
    port: 8080
  metrics:
    # With listenning port metrics will be served
    port: 8081
database:
  # How many block will be kept in database
  keep_history: 100
protocols:
  bitcoin:
    mainnet:
      network_options:
        head_length: 10
      rpc:
        - url: https://sample.bitcoin.mainnet.rpc
          # Options can be defined per endpoint, in this case, global options will be overrided
          options:
            retry: 10
            delay: 1
            rate: 1
            label: mysuperrpcnode
        - url: https://sample2.bitcoin.mainnet.rpc
          options:
            basic-auth:
              username: user
              password: pass
      blockstream:
        url: https://blockstream.info/api
      blockcypher:
        url: https://api.blockcypher.com
          headers:
            X-API-Key: MySuperSecretKey
    testnet: ...
  ethereum:
    mainnet:
      rpc:
        - url: https://sample.eth.mainnet.rpc
    goerli: ...
    sepolia: ...
  tezos:
    mainnet:
      rpc:
        - url: https://sample.tezos.mainnet.rpc
      tzkt:
        url: https://api.tzkt.io
      tzstats:
        url: https://api.tzstats.com
        options:
          headers:
            X-API-KEY: MySuperSecretKey
    ghostnet: ...
```

## Run blockhead

### Docker

From docker hub :

```bash
docker run -v /absolute/path/to/config.yaml:/app/config.yaml vivienbcr/blockhead:latest --config config.yaml
```

From source :

```bash
git clone https://github.com/vivienbcr/blockhead.git
docker build -t blockhead .
docker run -v /absolute/path/to/config.yaml:/app/config.yaml blockhead --config config.yaml
```

Docker compose:

```bash
git clone https://github.com/vivienbcr/blockhead.git
docker-compose up
```

### Standalone

```bash
git clone https://github.com/vivienbcr/blockhead.git
cargo run -- --config config.yaml
```

## API

- API endpoints on : http://localhost:8080/
- Prometheus metrics on : http://localhost:8081/metrics

## Prometheus metrics

Available metrics :

- blockhead_http_response_code (gauge) : Http code returned by endpoints
- blockhead_http_response_time_ms (histogram) : Http response time in ms
- blockhead_endpoint_status (gauge) : Endpoint status (1 = ok, 0 = ko)
- blockhead_blockchain_height (gauge) : Computed blockchain height
- blockhead_blockchain_head_timestamp (gauge) : Computed blockchain head timestamp
- blockhead_blockchain_head_txs (gauge) : Computed blockchain head txs
- blockhead_blockchain_height_endpoint (gauge) : Endpoint blockchain height
