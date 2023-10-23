# Tagger Api

REST Api for natural language processing of Qwant map user queries.


## Prerequisites

The address tagger is hidden behind the `address` cargo feature to avoid installing libpostal on each CI/CD pipeline run.

If you need to test this feature locally you will need to install [libpostal](https://github.com/openvenues/libpostal). 
 
Note that if libpostal linking fail at runtime, you will need to export the following library path: `LD_LIBRARY_PATH=/usr/local/lib`. 


### Run

```sh
cargo run --package tagger-api --features address -- --debug
```

### Run with docker

1. **build the image:**
    ```sh
   docker build -f docker/tagger/Dockerfile -t tagger  .
   ```
2. **run:**
    ```sh
   docker run -p 3000:3000 tagger
   ```
   
## Documentation

The OpenApi documentation can be found at `/docs`
