# Blockhead

Blockhead is simple application used to parse blockchains head state and monitor responses from multiple sources. Blockhead expose results via HTTP API and Prometheus metrics.

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
        - url: https://sample2.bitcoin.mainnet.rpc
      blockstream:
        url: https://blockstream.info/api
      blockcypher:
        url: https://api.blockcypher.com
  ethereum:
    mainnet:
      rpc:
        - url: https://sample.eth.mainnet.rpc
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
